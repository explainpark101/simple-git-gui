#!/usr/bin/env bash
# Install the latest (or a given tag) macOS DMG from GitHub Releases via curl.
#
# Browser downloads set com.apple.quarantine and trigger Gatekeeper for
# ad-hoc signed builds. curl does not, so this path avoids that dialog when
# the release is not yet notarized.
#
# Usage:
#   bash packaging/macos/install-from-github.sh
#   bash packaging/macos/install-from-github.sh v1.4.1
#   curl -fsSL https://raw.githubusercontent.com/<owner>/<repo>/main/packaging/macos/install-from-github.sh | bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "error: macOS only" >&2
  exit 1
fi

REPO="${SGG_GITHUB_REPO:-explainpark101/simple-git-gui}"
TAG="${1:-}"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/sgg-install.XXXXXX")"
cleanup() { rm -rf "${TMP}"; }
trap cleanup EXIT

case "$(uname -m)" in
  arm64|aarch64) ARCH="aarch64" ;;
  x86_64) ARCH="x64" ;;
  *)
    echo "error: unsupported arch $(uname -m)" >&2
    exit 1
    ;;
esac

if [[ -z "${TAG}" ]]; then
  echo "==> resolve latest release for ${REPO}"
  TAG="$(
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
      | sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' \
      | head -n 1
  )"
fi
if [[ -z "${TAG}" ]]; then
  echo "error: could not resolve release tag" >&2
  exit 1
fi

VERSION="${TAG#v}"
DMG_NAME="simple-git-gui_${VERSION}_${ARCH}.dmg"
URL="https://github.com/${REPO}/releases/download/${TAG}/${DMG_NAME}"
DMG_PATH="${TMP}/${DMG_NAME}"

echo "==> download ${URL}"
curl -fL --retry 3 --retry-delay 2 -o "${DMG_PATH}" "${URL}"

# Extra safety if some tools stamped quarantine anyway.
xattr -cr "${DMG_PATH}" 2>/dev/null || true

echo "==> attach DMG"
ATTACH_OUT="$(hdiutil attach "${DMG_PATH}" -nobrowse -readonly)"
MOUNT="$(
  sed -n 's/.*\(\/Volumes\/[^[:space:]]*\).*/\1/p' <<<"${ATTACH_OUT}" | tail -n 1
)"
if [[ -z "${MOUNT}" || ! -d "${MOUNT}" ]]; then
  echo "error: could not mount DMG" >&2
  echo "${ATTACH_OUT}" >&2
  exit 1
fi
detach() { hdiutil detach "${MOUNT}" -quiet || hdiutil detach "${MOUNT}" -force || true; }
trap 'detach; cleanup' EXIT

APP_SRC="${MOUNT}/simple-git-gui.app"
INSTALL_CMD="${MOUNT}/Install sgg.command"
APP_DST="/Applications/simple-git-gui.app"

if [[ ! -d "${APP_SRC}" ]]; then
  echo "error: ${APP_SRC} missing in DMG" >&2
  ls -la "${MOUNT}" >&2 || true
  exit 1
fi

if [[ -x "${INSTALL_CMD}" ]]; then
  echo "==> run Install sgg.command"
  # Run from mount so the helper finds the sibling .app.
  /bin/bash "${INSTALL_CMD}"
else
  echo "==> copy app to ${APP_DST}"
  rm -rf "${APP_DST}"
  cp -R "${APP_SRC}" "${APP_DST}"
  xattr -cr "${APP_DST}" 2>/dev/null || true
  BIN="${APP_DST}/Contents/MacOS/sgg"
  if [[ -x "${BIN}" ]]; then
    "${BIN}" install-shell
  fi
fi

echo "==> installed ${TAG} → ${APP_DST}"
echo "    open a new terminal and run: sgg"
