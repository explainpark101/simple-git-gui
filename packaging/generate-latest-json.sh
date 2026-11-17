#!/usr/bin/env bash
# Write release-assets/latest.json for GitHub Releases (auto-update manifest).
# Only includes download keys for files that actually exist in the out dir.
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

# Resolve a known asset filename to its latest.json download key (empty if unknown).
key_for_name() {
  case "$1" in
    "simple-git-gui_${VERSION}_aarch64.dmg") echo "macos-aarch64" ;;
    "simple-git-gui_${VERSION}_x64.dmg") echo "macos-x64" ;;
    "sgg_${VERSION}_x64-setup.exe") echo "windows-x64" ;;
    "sgg_${VERSION}_arm64-setup.exe") echo "windows-arm64" ;;
    "sgg_${VERSION}_x86_64.AppImage") echo "linux-x64" ;;
    "sgg_portable.exe") echo "windows-x64-portable" ;;
    "sgg_portable_arm64.exe") echo "windows-arm64-portable" ;;
    "linux_sgg_portable") echo "linux-x64-portable" ;;
    *) echo "" ;;
  esac
}

entries=""
count=0
# Portable directory listing (no mapfile / assoc arrays — works on bash 3 + Git Bash).
# shellcheck disable=SC2045
for path in "${OUT_DIR}"/*; do
  [[ -e "${path}" ]] || continue
  [[ -f "${path}" ]] || continue
  name="$(basename "${path}")"
  [[ "${name}" == "latest.json" ]] && continue
  key="$(key_for_name "${name}")"
  if [[ -z "${key}" ]]; then
    echo "note: skipping unrecognized asset: ${name}" >&2
    continue
  fi
  entry="$(printf '    "%s": { "name": "%s", "url": "%s/%s" }' \
    "${key}" "${name}" "${BASE}" "${name}")"
  if [[ -n "${entries}" ]]; then
    entries="${entries},
${entry}"
  else
    entries="${entry}"
  fi
  count=$((count + 1))
done

if [[ "${count}" -eq 0 ]]; then
  echo "error: no recognized release assets in ${OUT_DIR}" >&2
  ls -lah "${OUT_DIR}" >&2 || true
  exit 1
fi

cat > "${OUT}" <<EOF
{
  "version": "${VERSION}",
  "tag": "${TAG}",
  "name": "simple-git-gui",
  "notes": "https://github.com/${REPO}/releases/tag/${TAG}",
  "published_at": "${PUBLISHED_AT}",
  "downloads": {
${entries}
  }
}
EOF

echo "==> wrote ${OUT} (${count} assets)"
cat "${OUT}"
