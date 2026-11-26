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
        gsub(/["\r]/, "", $3)
        print $3
        exit
      }
    ' "$ROOT/Cargo.toml"
  )"
  # Cargo.toml may use CRLF on Windows checkouts; strip CR/whitespace.
  v="${v//$'\r'/}"
  v="${v#"${v%%[![:space:]]*}"}"
  v="${v%"${v##*[![:space:]]}"}"
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

# Fail if the macOS binary still loads Homebrew OpenSSL (absolute Cellar/opt
# paths). Those break on machines without the same brew formula (libssl.4 etc.).
assert_no_homebrew_openssl() {
  local bin="$1"
  if [[ ! -f "$bin" ]]; then
    echo "error: assert_no_homebrew_openssl: missing binary: $bin" >&2
    exit 1
  fi
  if ! command -v otool >/dev/null 2>&1; then
    echo "warn: otool not found — skip Homebrew OpenSSL dylib check" >&2
    return 0
  fi
  local deps
  deps="$(otool -L "$bin" 2>/dev/null || true)"
  if printf '%s\n' "$deps" | grep -E '/opt/homebrew/.*/lib(ssl|crypto)\.|/usr/local/opt/openssl' >/dev/null; then
    echo "error: $bin links Homebrew OpenSSL — enable git2 vendored-openssl" >&2
    printf '%s\n' "$deps" | grep -E 'lib(ssl|crypto)\.' >&2 || true
    exit 1
  fi
  echo "==> OK: no Homebrew OpenSSL load commands in $(basename "$bin")"
}
