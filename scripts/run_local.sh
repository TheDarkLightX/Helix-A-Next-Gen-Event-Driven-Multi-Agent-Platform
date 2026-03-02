#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUN_DIR="$ROOT_DIR/runs/local"
mkdir -p "$RUN_DIR"

API_LOG="$RUN_DIR/api.log"
UI_LOG="$RUN_DIR/ui.log"

echo "[run] starting Helix API (logs: $API_LOG)"
(
  cd "$ROOT_DIR"
  cargo run --manifest-path crates/helix-api/Cargo.toml >"$API_LOG" 2>&1
) &
API_PID=$!

echo "[run] starting Helix UI (logs: $UI_LOG)"
(
  cd "$ROOT_DIR/ui"
  npm run dev -- --host 127.0.0.1 --port 5173 >"$UI_LOG" 2>&1
) &
UI_PID=$!

cleanup() {
  echo "[run] stopping processes"
  kill "$API_PID" "$UI_PID" >/dev/null 2>&1 || true
}
trap cleanup INT TERM EXIT

echo "[run] API: http://127.0.0.1:3000"
echo "[run] UI:  http://127.0.0.1:5173"
echo "[run] press Ctrl+C to stop"

wait -n "$API_PID" "$UI_PID"
