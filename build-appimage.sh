#!/usr/bin/env bash
# Build a Linux AppImage into ./dist via cargo-packager.
# After first run, file-manager integration: sgg install-shell
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=packaging/common.sh
source "$ROOT/packaging/common.sh"

require_host linux
run_packager appimage

echo "==> AppImage(s) in $DIST"
ls -lh "$DIST"/*.AppImage 2>/dev/null || ls -lh "$DIST"
echo "note: run 'sgg install-shell' once for Nautilus / file-manager actions."
