#!/usr/bin/env bash
# Ad-hoc codesign macOS .app bundles in ./dist.
#
# Used by build-dmg.sh after cargo-packager creates the .app (and again before
# the final DMG is assembled). Uses `codesign --sign -` WITHOUT
# `--options runtime` — packager's built-in signing-identity="-" enables
# Hardened Runtime, which rejects ad-hoc bundles without notarization.
set -euo pipefail

if [[ "$(uname -s 2>/dev/null || echo unknown)" != "Darwin" ]]; then
  exit 0
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="${ROOT}/dist"

shopt -s nullglob
apps=("${DIST}"/*.app)
shopt -u nullglob

if [[ ${#apps[@]} -eq 0 ]]; then
  echo "error: macos-adhoc-sign: no .app found in ${DIST}" >&2
  ls -lah "${DIST}" 2>/dev/null || true
  exit 1
fi

for app in "${apps[@]}"; do
  echo "==> ad-hoc codesign ${app}"
  # Inside-out via --deep; no hardened runtime / no Apple timestamp server.
  codesign --force --deep --sign - --timestamp=none "${app}"
  codesign --verify --verbose=2 "${app}"
  codesign -dv --verbose=2 "${app}" 2>&1 | head -n 20 || true
done
