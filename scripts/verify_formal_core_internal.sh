#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODEL_DIR="$ROOT_DIR/formal/models"
OUT_DIR="$ROOT_DIR/runs/formal_core_verify"
PYTHON_BIN="${PYTHON_BIN:-python3}"
FORMAL_MODULE="${HELIX_FORMAL_MODULE:-formal_verifier}"
FORMAL_MODULE_CANDIDATE="$FORMAL_MODULE"
FORMAL_MODULE_HINT="${HELIX_FORMAL_FALLBACK_MODULE:-}"
VENDORED_ROOT="${HELIX_FORMAL_VENDORED_ROOT:-$ROOT_DIR/external}"

check_formal_module() {
  FORMAL_MODULE_CHECK="$1" "$PYTHON_BIN" - <<'PY' >/dev/null 2>&1
import importlib
import os

importlib.import_module(os.environ["FORMAL_MODULE_CHECK"])
PY
}

prepend_pythonpath() {
  if [[ -n "${PYTHONPATH:-}" ]]; then
    export PYTHONPATH="$1:$PYTHONPATH"
  else
    export PYTHONPATH="$1"
  fi
}

resolve_formal_module() {
  if [[ -n "${HELIX_FORMAL_PYTHONPATH:-}" ]]; then
    prepend_pythonpath "${HELIX_FORMAL_PYTHONPATH}"
  fi

  if check_formal_module "$FORMAL_MODULE_CANDIDATE"; then
    FORMAL_MODULE="$FORMAL_MODULE_CANDIDATE"
    return 0
  fi

  if [[ -n "$FORMAL_MODULE_HINT" ]] && check_formal_module "$FORMAL_MODULE_HINT"; then
    FORMAL_MODULE="$FORMAL_MODULE_HINT"
    return 0
  fi

  if [[ -d "$VENDORED_ROOT" ]]; then
    for vendored_repo in "$VENDORED_ROOT"/*; do
      [[ -d "$vendored_repo" ]] || continue
      local module_name
      module_name="$(basename "$vendored_repo")"
      [[ -f "$vendored_repo/$module_name/__init__.py" ]] || continue
      prepend_pythonpath "$vendored_repo"
      if check_formal_module "$module_name"; then
        FORMAL_MODULE="$module_name"
        return 0
      fi
    done
  fi

  return 1
}

mkdir -p "$OUT_DIR"

if ! resolve_formal_module; then
  echo "[formal] verification module not found." >&2
  echo "[formal] tried HELIX_FORMAL_MODULE='$FORMAL_MODULE_CANDIDATE', HELIX_FORMAL_FALLBACK_MODULE, and vendored backends under '$VENDORED_ROOT'." >&2
  echo "[formal] set HELIX_FORMAL_MODULE and HELIX_FORMAL_PYTHONPATH before running." >&2
  exit 1
fi

echo "[formal] using configured verification backend"

models=(
  "helix_execution_kernel.yaml"
  "onchain_tx_intent.yaml"
  "reasoning/symbolic_reasoning_gate.yaml"
  "reasoning/expert_system_gate.yaml"
  "reasoning/neuro_risk_gate.yaml"
  "reasoning/neuro_symbolic_fusion_gate.yaml"
)

for model in "${models[@]}"; do
  model_path="$MODEL_DIR/$model"
  model_id="${model%.yaml}"
  model_out="$OUT_DIR/$model_id"
  mkdir -p "$model_out"

  echo "[formal][$model_id] guide (verify, ci)"
  "$PYTHON_BIN" -m "$FORMAL_MODULE" guide \
    --input "$model_path" \
    --goal verify \
    --profile ci \
    > "$model_out/guide.json"

  echo "[formal][$model_id] validate"
  "$PYTHON_BIN" -m "$FORMAL_MODULE" validate "$model_path" \
    > "$model_out/validate.json"

  echo "[formal][$model_id] verify (self-reference equivalence)"
  "$PYTHON_BIN" -m "$FORMAL_MODULE" verify "$model_path" \
    --reference "$model_path" \
    --timeout-ms 10000 \
    --output "$model_out/verify" \
    --write-report \
    > "$model_out/verify.json"

  echo "[formal][$model_id] verify-multi (determinism + cross-check)"
  "$PYTHON_BIN" -m "$FORMAL_MODULE" verify-multi "$model_path" \
    --reference "$model_path" \
    --timeout-ms 10000 \
    --solvers z3 \
    --determinism-trials 2 \
    --output "$model_out/verify_multi" \
    --write-report \
    > "$model_out/verify_multi.json"
done

echo "[formal] completed: artifacts in $OUT_DIR"
