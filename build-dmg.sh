#!/usr/bin/env bash
# Build a macOS .app + .dmg into ./dist via cargo-packager.
# After install, Finder Services can be registered with: sgg install-shell
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=packaging/common.sh
source "$ROOT/packaging/common.sh"

require_host macos
run_packager dmg

echo "==> macOS package(s) in $DIST"
ls -lh "$DIST"/*.dmg 2>/dev/null || ls -lh "$DIST"
echo "note: run 'sgg install-shell' once after install for Finder Services."
