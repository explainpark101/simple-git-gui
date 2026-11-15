#!/usr/bin/env bash
# Build a Windows NSIS installer (.exe) into ./dist via cargo-packager.
# Prefer cargo-packager (NSIS) over Inno Setup: one Rust toolchain for
# Windows/macOS/Linux packages, and the custom NSIS template registers the
# Explorer context menu ("여기서 sgg 열기") on install / removes it on uninstall.
#
# CI also copies target/release/sgg.exe into dist/ as a portable binary:
#   sgg_portable.exe       (x86_64)
#   sgg_portable_arm64.exe (aarch64 / Snapdragon)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=packaging/common.sh
source "$ROOT/packaging/common.sh"

require_host windows
run_packager nsis

echo "==> Windows installer(s) in $DIST"
ls -lh "$DIST"/*-setup.exe 2>/dev/null || ls -lh "$DIST"/*.exe 2>/dev/null || ls -lh "$DIST"
