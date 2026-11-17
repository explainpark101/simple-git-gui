#!/usr/bin/env bash
# Build a macOS .app + .dmg into ./dist via cargo-packager.
# Ad-hoc codesigns the .app before the DMG is assembled (Apple Silicon needs
# at least an ad-hoc signature to launch). After install, Finder Services can
# be registered with: sgg install-shell
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=packaging/common.sh
source "$ROOT/packaging/common.sh"

require_host macos
# `app` must be listed so packager keeps the bundle and runs the before-each
# hook for `dmg` after the .app exists (see packaging/macos-adhoc-sign.sh).
run_packager app,dmg

# Ad-hoc sign the DMG itself (no hardened runtime).
shopt -s nullglob
dmgs=("${DIST}"/*.dmg)
shopt -u nullglob
for dmg in "${dmgs[@]}"; do
  echo "==> ad-hoc codesign ${dmg}"
  codesign --force --sign - --timestamp=none "${dmg}"
  codesign --verify --verbose=2 "${dmg}" || true
done

echo "==> macOS package(s) in $DIST"
ls -lh "$DIST"/*.dmg 2>/dev/null || ls -lh "$DIST"
echo "note: run 'sgg install-shell' once after install for Finder Services."
echo "note: .app/.dmg use ad-hoc codesign (no Apple Developer ID / notarization)."
