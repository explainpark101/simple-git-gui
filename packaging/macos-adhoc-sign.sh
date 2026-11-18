#!/usr/bin/env bash
# Back-compat wrapper: prefer packaging/macos-sign.sh.
# Forces ad-hoc only when MACOS_SIGNING_IDENTITY is unset and no Developer ID
# is in the keychain (same as macos-sign.sh default path).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec bash "$ROOT/packaging/macos-sign.sh" "$@"
