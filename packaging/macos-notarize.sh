#!/usr/bin/env bash
# Notarize + staple a macOS .dmg (or .app.zip) with notarytool.
#
# Skips cleanly when Apple credentials are not configured (local/ad-hoc builds).
#
# Auth (either set):
#   App Store Connect API key (preferred):
#     APPLE_API_KEY_PATH  — path to AuthKey_XXX.p8
#     APPLE_API_KEY_ID
#     APPLE_API_ISSUER    — issuer UUID (team keys)
#   Or Apple ID:
#     APPLE_ID
#     APPLE_APP_SPECIFIC_PASSWORD
#     APPLE_TEAM_ID
#
# Usage: macos-notarize.sh <path-to.dmg>
set -euo pipefail

if [[ "$(uname -s 2>/dev/null || echo unknown)" != "Darwin" ]]; then
  exit 0
fi

TARGET="${1:-}"
if [[ -z "${TARGET}" || ! -e "${TARGET}" ]]; then
  echo "error: macos-notarize: pass an existing .dmg/.pkg/.zip path" >&2
  exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="${ROOT}/dist"

# Only notarize Developer ID–signed artifacts.
if [[ ! -f "${DIST}/.macos-signing-identity" ]]; then
  echo "==> skip notarize: ad-hoc signed (set MACOS_SIGNING_IDENTITY / import Developer ID cert)"
  exit 0
fi

has_api_key=0
has_apple_id=0
if [[ -n "${APPLE_API_KEY_PATH:-}" && -n "${APPLE_API_KEY_ID:-}" && -n "${APPLE_API_ISSUER:-}" ]]; then
  has_api_key=1
fi
if [[ -n "${APPLE_ID:-}" && -n "${APPLE_APP_SPECIFIC_PASSWORD:-}" && -n "${APPLE_TEAM_ID:-}" ]]; then
  has_apple_id=1
fi

if [[ "${has_api_key}" -eq 0 && "${has_apple_id}" -eq 0 ]]; then
  echo "==> skip notarize: no Apple notary credentials in env"
  echo "    set APPLE_API_KEY_PATH + APPLE_API_KEY_ID + APPLE_API_ISSUER"
  echo "    or APPLE_ID + APPLE_APP_SPECIFIC_PASSWORD + APPLE_TEAM_ID"
  exit 0
fi

if [[ "${has_api_key}" -eq 1 && ! -f "${APPLE_API_KEY_PATH}" ]]; then
  echo "error: APPLE_API_KEY_PATH not found: ${APPLE_API_KEY_PATH}" >&2
  exit 1
fi

echo "==> submit for notarization: ${TARGET}"
if [[ "${has_api_key}" -eq 1 ]]; then
  xcrun notarytool submit "${TARGET}" \
    --key "${APPLE_API_KEY_PATH}" \
    --key-id "${APPLE_API_KEY_ID}" \
    --issuer "${APPLE_API_ISSUER}" \
    --wait
else
  xcrun notarytool submit "${TARGET}" \
    --apple-id "${APPLE_ID}" \
    --password "${APPLE_APP_SPECIFIC_PASSWORD}" \
    --team-id "${APPLE_TEAM_ID}" \
    --wait
fi

echo "==> staple ${TARGET}"
xcrun stapler staple "${TARGET}"
xcrun stapler validate "${TARGET}"

echo "==> Gatekeeper assessment"
spctl --assess --type open --context context:primary-signature -v "${TARGET}" || true

echo "==> notarization complete"
