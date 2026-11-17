#!/usr/bin/env bash
# Build sgg (release) and install the binary into ./dist
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

DIST="$ROOT/dist"
mkdir -p "$DIST"

echo "==> cargo build -p sgg --release"
cargo build -p sgg --release

BIN_SRC=""
if [[ -f "$ROOT/target/release/sgg.exe" ]]; then
  BIN_SRC="$ROOT/target/release/sgg.exe"
  BIN_DST="$DIST/sgg.exe"
elif [[ -f "$ROOT/target/release/sgg" ]]; then
  BIN_SRC="$ROOT/target/release/sgg"
  BIN_DST="$DIST/sgg"
else
  echo "error: release binary not found under target/release/" >&2
  exit 1
fi

cp -f "$BIN_SRC" "$BIN_DST"
chmod +x "$BIN_DST" 2>/dev/null || true

echo "==> installed: $BIN_DST"
ls -lh "$BIN_DST"
