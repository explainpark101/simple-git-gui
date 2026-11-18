#!/usr/bin/env bash
# Sign macOS .app bundles (and optionally a .dmg) in ./dist.
#
# Prefer Developer ID when available (MACOS_SIGNING_IDENTITY / keychain),
# otherwise fall back to ad-hoc codesign (no hardened runtime).
#
# Env:
#   MACOS_SIGNING_IDENTITY  — e.g. "Developer ID Application: Name (TEAMID)"
#                             or a certificate SHA-1 hash. Empty → auto-detect
#                             Developer ID Application, else ad-hoc.
set -euo pipefail

if [[ "$(uname -s 2>/dev/null || echo unknown)" != "Darwin" ]]; then
  exit 0
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="${ROOT}/dist"
ENTITLEMENTS="${ROOT}/packaging/macos/entitlements.plist"

resolve_identity() {
  if [[ -n "${MACOS_SIGNING_IDENTITY:-}" ]]; then
    printf '%s' "${MACOS_SIGNING_IDENTITY}"
    return 0
  fi
  # Prefer a Developer ID Application identity already in the keychain.
  local line
  line="$(
    security find-identity -v -p codesigning 2>/dev/null \
      | grep -F 'Developer ID Application:' \
      | head -n 1 \
      || true
  )"
  if [[ -n "${line}" ]]; then
    # Extract quoted identity name from:  1) HASH "Developer ID Application: ..."
    sed -n 's/.*"\(Developer ID Application: .*\)".*/\1/p' <<<"${line}"
    return 0
  fi
  printf ''
}

sign_app() {
  local app="$1"
  local identity="$2"

  if [[ -n "${identity}" ]]; then
    echo "==> Developer ID codesign ${app}"
    echo "    identity: ${identity}"
    if [[ ! -f "${ENTITLEMENTS}" ]]; then
      echo "error: missing ${ENTITLEMENTS}" >&2
      exit 1
    fi
    # Inside-out via --deep; hardened runtime + secure timestamp required for notarization.
    codesign \
      --force \
      --deep \
      --options runtime \
      --timestamp \
      --entitlements "${ENTITLEMENTS}" \
      --sign "${identity}" \
      "${app}"
  else
    echo "==> ad-hoc codesign ${app}"
    # No hardened runtime — ad-hoc + runtime is rejected without notarization.
    codesign --force --deep --sign - --timestamp=none "${app}"
  fi

  codesign --verify --verbose=2 "${app}"
  codesign -dv --verbose=2 "${app}" 2>&1 | head -n 20 || true
}

sign_dmg() {
  local dmg="$1"
  local identity="$2"

  if [[ -n "${identity}" ]]; then
    echo "==> Developer ID codesign ${dmg}"
    codesign --force --sign "${identity}" --timestamp "${dmg}"
  else
    echo "==> ad-hoc codesign ${dmg}"
    codesign --force --sign - --timestamp=none "${dmg}"
  fi
  codesign --verify --verbose=2 "${dmg}" || true
}

IDENTITY="$(resolve_identity)"
if [[ -n "${IDENTITY}" ]]; then
  echo "==> using Developer ID signing identity"
else
  echo "==> no Developer ID identity — ad-hoc sign (Gatekeeper will warn on download)"
fi

# Optional: sign only a DMG (used by build-dmg.sh after rebuild).
if [[ "${1:-}" == "--dmg" && -n "${2:-}" ]]; then
  sign_dmg "$2" "${IDENTITY}"
  if [[ -n "${IDENTITY}" ]]; then
    printf '%s' "${IDENTITY}" > "${DIST}/.macos-signing-identity"
  else
    rm -f "${DIST}/.macos-signing-identity"
  fi
  exit 0
fi

shopt -s nullglob
apps=("${DIST}"/*.app)
shopt -u nullglob

if [[ ${#apps[@]} -eq 0 ]]; then
  echo "error: macos-sign: no .app found in ${DIST}" >&2
  ls -lah "${DIST}" 2>/dev/null || true
  exit 1
fi

for app in "${apps[@]}"; do
  sign_app "${app}" "${IDENTITY}"
done

# Export for callers (notarize step).
if [[ -n "${IDENTITY}" ]]; then
  printf '%s' "${IDENTITY}" > "${DIST}/.macos-signing-identity"
else
  rm -f "${DIST}/.macos-signing-identity"
fi
