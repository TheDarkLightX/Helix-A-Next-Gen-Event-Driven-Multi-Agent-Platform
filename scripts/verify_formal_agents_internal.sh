#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODEL_DIR="$ROOT_DIR/formal/models/roi_agents"
OUT_DIR="$ROOT_DIR/runs/formal_roi_agents_verify"
PYTHON_BIN="${PYTHON_BIN:-python3}"
FORMAL_MODULE="${HELIX_FORMAL_MODULE:-formal_verifier}"

check_formal_module() {
  FORMAL_MODULE="$FORMAL_MODULE" "$PYTHON_BIN" - <<'PY' >/dev/null 2>&1
import importlib
import os

importlib.import_module(os.environ["FORMAL_MODULE"])
PY
}

mkdir -p "$OUT_DIR"

if ! check_formal_module; then
  if [[ -n "${HELIX_FORMAL_PYTHONPATH:-}" ]]; then
    export PYTHONPATH="${HELIX_FORMAL_PYTHONPATH}:${PYTHONPATH:-}"
  fi
fi

if ! check_formal_module; then
  echo "[formal] private formal verification module not found." >&2
  echo "[formal] set HELIX_FORMAL_MODULE and HELIX_FORMAL_PYTHONPATH before running." >&2
  exit 1
fi

models=(
  "dedup_window.yaml"
  "token_bucket.yaml"
  "circuit_breaker.yaml"
  "retry_budget.yaml"
  "approval_gate.yaml"
  "backpressure.yaml"
  "sla_deadline.yaml"
  "dlq_budget.yaml"
  "nonce_manager.yaml"
  "fee_bidding.yaml"
  "finality_guard.yaml"
  "allowlist_guard.yaml"
)

for model in "${models[@]}"; do
  model_path="$MODEL_DIR/$model"
  model_id="${model%.yaml}"
  model_out="$OUT_DIR/$model_id"
  mkdir -p "$model_out"

  echo "[formal][$model_id] validate"
  "$PYTHON_BIN" -m "$FORMAL_MODULE" validate "$model_path" > "$model_out/validate.json"

  echo "[formal][$model_id] verify"
  "$PYTHON_BIN" -m "$FORMAL_MODULE" verify "$model_path" \
    --reference "$model_path" \
    --timeout-ms 10000 \
    --output "$model_out/verify" \
    --write-report \
    > "$model_out/verify.json"

  echo "[formal][$model_id] verify-multi"
  "$PYTHON_BIN" -m "$FORMAL_MODULE" verify-multi "$model_path" \
    --reference "$model_path" \
    --timeout-ms 10000 \
    --solvers z3 \
    --determinism-trials 2 \
    --output "$model_out/verify_multi" \
    --write-report \
    > "$model_out/verify_multi.json"
done

echo "[formal] ROI agent verification completed: $OUT_DIR"
