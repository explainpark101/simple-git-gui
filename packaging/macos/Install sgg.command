#!/bin/bash
# Double-click from the DMG: copy the app to /Applications and register `sgg` on PATH
# (plus Finder Services via `sgg install-shell`).
set -euo pipefail

DIR="$(cd "$(dirname "$0")" && pwd)"
APP_SRC="${DIR}/simple-git-gui.app"
APP_DST="/Applications/simple-git-gui.app"
BIN="${APP_DST}/Contents/MacOS/sgg"

dialog() {
  local msg="$1"
  local icon="${2:-note}"
  /usr/bin/osascript -e "display dialog \"${msg}\" buttons {\"OK\"} default button 1 with icon ${icon}" >/dev/null || true
}

clear_quarantine() {
  local path="$1"
  if [[ -e "${path}" ]]; then
    /usr/bin/xattr -cr "${path}" 2>/dev/null || true
  fi
}

if [[ ! -d "${APP_SRC}" ]]; then
  dialog "simple-git-gui.app was not found next to this installer." stop
  exit 1
fi

# Browser-downloaded DMGs set com.apple.quarantine; strip it so Gatekeeper
# does not block the installed copy after the user opens this installer.
clear_quarantine "${APP_SRC}"
clear_quarantine "${DIR}/Install sgg.command"

echo "==> Installing to ${APP_DST}"
rm -rf "${APP_DST}"
cp -R "${APP_SRC}" "${APP_DST}"
clear_quarantine "${APP_DST}"

if [[ ! -x "${BIN}" ]]; then
  dialog "Installed the app, but could not find ${BIN}" stop
  exit 1
fi

echo "==> Registering sgg on PATH + Finder Services"
"${BIN}" install-shell

dialog "simple-git-gui installed to Applications.

The sgg command is on your PATH — open a new terminal tab and run: sgg

Finder: right-click a folder → Services → Open with sgg
(You may need to enable it once under System Settings → Keyboard → Shortcuts → Services.)" note

echo "Done."
