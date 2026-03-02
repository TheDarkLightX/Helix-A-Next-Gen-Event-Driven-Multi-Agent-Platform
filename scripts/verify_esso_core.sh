#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODEL_DIR="$ROOT_DIR/formal/esso"
OUT_DIR="$ROOT_DIR/runs/esso_core_verify"
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
  "helix_execution_kernel.yaml"
  "onchain_tx_intent.yaml"
)

for model in "${models[@]}"; do
  model_path="$MODEL_DIR/$model"
  model_id="${model%.yaml}"
  model_out="$OUT_DIR/$model_id"
  mkdir -p "$model_out"

  echo "[esso][$model_id] guide (verify, ci)"
  "$PYTHON_BIN" -m ESSO guide \
    --input "$model_path" \
    --goal verify \
    --profile ci \
    > "$model_out/guide.json"

  echo "[esso][$model_id] validate"
  "$PYTHON_BIN" -m ESSO validate "$model_path" \
    > "$model_out/validate.json"

  echo "[esso][$model_id] verify (self-reference equivalence)"
  "$PYTHON_BIN" -m ESSO verify "$model_path" \
    --reference "$model_path" \
    --timeout-ms 10000 \
    --output "$model_out/verify" \
    --write-report \
    > "$model_out/verify.json"

  echo "[esso][$model_id] verify-multi (determinism + cross-check)"
  "$PYTHON_BIN" -m ESSO verify-multi "$model_path" \
    --reference "$model_path" \
    --timeout-ms 10000 \
    --solvers z3 \
    --determinism-trials 2 \
    --output "$model_out/verify_multi" \
    --write-report \
    > "$model_out/verify_multi.json"
done

echo "[esso] completed: artifacts in $OUT_DIR"
