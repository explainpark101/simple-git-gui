#!/usr/bin/env bash
# Write release-assets/latest.json for GitHub Releases (auto-update manifest).
#
# Usage:
#   VERSION=1.4.2 TAG=v1.4.2 REPO=explainpark101/simple-git-gui \
#     bash packaging/generate-latest-json.sh [out-dir]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${1:-${ROOT}/release-assets}"
VERSION="${VERSION:?VERSION required}"
TAG="${TAG:-v${VERSION}}"
REPO="${REPO:-explainpark101/simple-git-gui}"
BASE="https://github.com/${REPO}/releases/download/${TAG}"
PUBLISHED_AT="${PUBLISHED_AT:-$(date -u +"%Y-%m-%dT%H:%M:%SZ")}"

mkdir -p "${OUT_DIR}"
OUT="${OUT_DIR}/latest.json"

# Asset names must match cargo-packager / build-*.sh / CI portable copies.
cat > "${OUT}" <<EOF
{
  "version": "${VERSION}",
  "tag": "${TAG}",
  "name": "simple-git-gui",
  "notes": "https://github.com/${REPO}/releases/tag/${TAG}",
  "published_at": "${PUBLISHED_AT}",
  "downloads": {
    "macos-aarch64": {
      "name": "simple-git-gui_${VERSION}_aarch64.dmg",
      "url": "${BASE}/simple-git-gui_${VERSION}_aarch64.dmg"
    },
    "macos-x64": {
      "name": "simple-git-gui_${VERSION}_x64.dmg",
      "url": "${BASE}/simple-git-gui_${VERSION}_x64.dmg"
    },
    "windows-x64": {
      "name": "sgg_${VERSION}_x64-setup.exe",
      "url": "${BASE}/sgg_${VERSION}_x64-setup.exe"
    },
    "windows-arm64": {
      "name": "sgg_${VERSION}_arm64-setup.exe",
      "url": "${BASE}/sgg_${VERSION}_arm64-setup.exe"
    },
    "linux-x64": {
      "name": "sgg_${VERSION}_x86_64.AppImage",
      "url": "${BASE}/sgg_${VERSION}_x86_64.AppImage"
    },
    "windows-x64-portable": {
      "name": "sgg_portable.exe",
      "url": "${BASE}/sgg_portable.exe"
    },
    "windows-arm64-portable": {
      "name": "sgg_portable_arm64.exe",
      "url": "${BASE}/sgg_portable_arm64.exe"
    },
    "linux-x64-portable": {
      "name": "linux_sgg_portable",
      "url": "${BASE}/linux_sgg_portable"
    }
  }
}
EOF

echo "==> wrote ${OUT}"
cat "${OUT}"
