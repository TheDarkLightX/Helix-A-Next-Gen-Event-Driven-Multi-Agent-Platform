#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODEL_DIR="$ROOT_DIR/formal/esso/roi_agents"
OUT_DIR="$ROOT_DIR/runs/esso_roi_agents_verify"
PYTHON_BIN="${PYTHON_BIN:-python3}"

mkdir -p "$OUT_DIR"

if ! "$PYTHON_BIN" -c "import ESSO" >/dev/null 2>&1; then
  if [[ -n "${HELIX_ESSO_PYTHONPATH:-}" ]]; then
    export PYTHONPATH="${HELIX_ESSO_PYTHONPATH}:${PYTHONPATH:-}"
  fi
fi

if ! "$PYTHON_BIN" -c "import ESSO" >/dev/null 2>&1; then
  echo "[esso] ESSO Python module not found." >&2
  echo "[esso] install ESSO in your Python environment or set HELIX_ESSO_PYTHONPATH." >&2
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

  echo "[esso][$model_id] validate"
  "$PYTHON_BIN" -m ESSO validate "$model_path" > "$model_out/validate.json"

  echo "[esso][$model_id] verify"
  "$PYTHON_BIN" -m ESSO verify "$model_path" \
    --reference "$model_path" \
    --timeout-ms 10000 \
    --output "$model_out/verify" \
    --write-report \
    > "$model_out/verify.json"

  echo "[esso][$model_id] verify-multi"
  "$PYTHON_BIN" -m ESSO verify-multi "$model_path" \
    --reference "$model_path" \
    --timeout-ms 10000 \
    --solvers z3 \
    --determinism-trials 2 \
    --output "$model_out/verify_multi" \
    --write-report \
    > "$model_out/verify_multi.json"
done

echo "[esso] ROI agent verification completed: $OUT_DIR"
