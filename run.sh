#!/usr/bin/env bash
# Run sgg from ./dist (builds first if the binary is missing)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

BIN=""
if [[ -f "$ROOT/dist/sgg.exe" ]]; then
  BIN="$ROOT/dist/sgg.exe"
elif [[ -f "$ROOT/dist/sgg" ]]; then
  BIN="$ROOT/dist/sgg"
fi

if [[ -z "$BIN" ]]; then
  echo "==> dist binary missing; running ./build.sh"
  "$ROOT/build.sh"
  if [[ -f "$ROOT/dist/sgg.exe" ]]; then
    BIN="$ROOT/dist/sgg.exe"
  else
    BIN="$ROOT/dist/sgg"
  fi
fi

exec "$BIN" "$@"
