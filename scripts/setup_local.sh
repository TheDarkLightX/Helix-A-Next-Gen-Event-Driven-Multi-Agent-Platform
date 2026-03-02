#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "[setup] missing required command: $cmd" >&2
    exit 1
  fi
}

echo "[setup] validating toolchain"
require_cmd cargo
require_cmd rustc
require_cmd node
require_cmd npm
require_cmd python3

echo "[setup] rust toolchain"
rustc --version
cargo --version

echo "[setup] node toolchain"
node --version
npm --version

if [[ -d "$ROOT_DIR/ui" && -f "$ROOT_DIR/ui/package.json" ]]; then
  echo "[setup] installing UI dependencies"
  (cd "$ROOT_DIR/ui" && npm install)
fi

echo "[setup] complete"
echo "[setup] next steps:"
echo "  1) ./scripts/run_local.sh"
echo "  2) open UI: http://127.0.0.1:5173"
echo "  3) API health: http://127.0.0.1:3000/health"
