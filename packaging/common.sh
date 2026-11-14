#!/usr/bin/env bash
# Shared helpers for packaging scripts (sourced, not executed).

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DIST="$ROOT/dist"
PACKAGER_BIN="${CARGO_HOME:-$HOME/.cargo}/bin/cargo-packager"

sgg_version() {
  # Prefer workspace.package.version from the root Cargo.toml.
  local v
  v="$(
    awk '
      /^\[workspace\.package\]/ { in_pkg=1; next }
      /^\[/ { in_pkg=0 }
      in_pkg && /^version[[:space:]]*=/ {
        gsub(/"/, "", $3)
        print $3
        exit
      }
    ' "$ROOT/Cargo.toml"
  )"
  if [[ -z "${v:-}" ]]; then
    echo "error: could not read workspace.package.version from Cargo.toml" >&2
    exit 1
  fi
  printf '%s' "$v"
}

ensure_cargo_packager() {
  if command -v cargo-packager >/dev/null 2>&1; then
    return 0
  fi
  if [[ -x "$PACKAGER_BIN" ]]; then
    export PATH="$(dirname "$PACKAGER_BIN"):$PATH"
    return 0
  fi
  echo "==> installing cargo-packager (Rust NSIS/DMG/AppImage packager)"
  cargo install cargo-packager --locked
  export PATH="$(dirname "$PACKAGER_BIN"):$PATH"
}

require_host() {
  local expected="$1"
  local actual
  actual="$(uname -s 2>/dev/null || echo unknown)"
  case "$expected" in
    windows)
      case "$actual" in
        MINGW*|MSYS*|CYGWIN*|Windows_NT) return 0 ;;
      esac
      if [[ "${OS:-}" == "Windows_NT" ]]; then
        return 0
      fi
      echo "error: build-exe.sh must run on Windows (Git Bash / MSYS2). Host=$actual" >&2
      exit 1
      ;;
    macos)
      if [[ "$actual" == "Darwin" ]]; then
        return 0
      fi
      echo "error: build-dmg.sh must run on macOS. Host=$actual" >&2
      exit 1
      ;;
    linux)
      if [[ "$actual" == "Linux" ]]; then
        return 0
      fi
      echo "error: build-appimage.sh must run on Linux. Host=$actual" >&2
      exit 1
      ;;
    *)
      echo "error: unknown host requirement: $expected" >&2
      exit 1
      ;;
  esac
}

prepare_dist() {
  mkdir -p "$DIST"
}

run_packager() {
  local format="$1"
  ensure_cargo_packager
  prepare_dist
  echo "==> cargo packager --release --formats $format --packages sgg"
  # Config lives in crates/sgg/Cargo.toml [package.metadata.packager]
  cargo packager --release --formats "$format" --packages sgg
}
