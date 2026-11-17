#!/usr/bin/env bash
# Build a macOS .app + .dmg into ./dist via cargo-packager.
#
# cargo-packager's before-each-package-command runs on every OS via cmd/sh, so
# macOS signing must NOT live there (it breaks Windows NSIS CI). Instead:
#   1) pack app + dmg
#   2) ad-hoc codesign the .app
#   3) rebuild the DMG from the signed .app + Install helper (hdiutil)
#   4) ad-hoc codesign the DMG
#
# DMG contents:
#   - simple-git-gui.app
#   - Applications (symlink)
#   - Install sgg.command  → copies to /Applications + PATH + Finder Services
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=packaging/common.sh
source "$ROOT/packaging/common.sh"

require_host macos
run_packager app,dmg

bash "$ROOT/packaging/macos-adhoc-sign.sh"

VERSION="$(sgg_version)"
case "$(uname -m)" in
  arm64|aarch64) ARCH="aarch64" ;;
  x86_64) ARCH="x64" ;;
  *) ARCH="$(uname -m)" ;;
esac

APP_NAME="simple-git-gui.app"
APP_PATH="${DIST}/${APP_NAME}"
DMG_NAME="simple-git-gui_${VERSION}_${ARCH}.dmg"
DMG_PATH="${DIST}/${DMG_NAME}"
INSTALL_CMD_SRC="${ROOT}/packaging/macos/Install sgg.command"

if [[ ! -d "${APP_PATH}" ]]; then
  echo "error: expected ${APP_PATH} after packager" >&2
  ls -lah "${DIST}" || true
  exit 1
fi
if [[ ! -f "${INSTALL_CMD_SRC}" ]]; then
  echo "error: missing ${INSTALL_CMD_SRC}" >&2
  exit 1
fi

echo "==> rebuild DMG from signed .app → ${DMG_PATH}"
STAGE="$(mktemp -d "${TMPDIR:-/tmp}/sgg-dmg.XXXXXX")"
cleanup_stage() {
  rm -rf "${STAGE}"
}
trap cleanup_stage EXIT

# Packager may leave an unsigned DMG; replace it with one that embeds the signed app.
rm -f "${DIST}"/*.dmg
cp -R "${APP_PATH}" "${STAGE}/${APP_NAME}"
ln -s /Applications "${STAGE}/Applications"
cp "${INSTALL_CMD_SRC}" "${STAGE}/Install sgg.command"
chmod +x "${STAGE}/Install sgg.command"

# UDZO = compressed read-only image (same role as create-dmg output).
hdiutil create \
  -volname "simple-git-gui" \
  -srcfolder "${STAGE}" \
  -ov \
  -format UDZO \
  "${DMG_PATH}"

echo "==> ad-hoc codesign ${DMG_PATH}"
codesign --force --sign - --timestamp=none "${DMG_PATH}"
codesign --verify --verbose=2 "${DMG_PATH}" || true

echo "==> macOS package(s) in $DIST"
ls -lh "$DIST"/*.dmg 2>/dev/null || ls -lh "$DIST"
echo "note: open the DMG and double-click 'Install sgg.command' (or drag .app then launch once)."
echo "note: that registers the sgg CLI on PATH; Finder Services via install-shell."
echo "note: .app/.dmg use ad-hoc codesign (no Apple Developer ID / notarization)."
