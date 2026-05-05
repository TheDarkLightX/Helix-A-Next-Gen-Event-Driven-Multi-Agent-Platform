#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "[release] formal core"
bash "$ROOT_DIR/scripts/verify_formal_core.sh"

echo "[release] formal deterministic agents"
bash "$ROOT_DIR/scripts/verify_formal_agents.sh"

echo "[release] helix-core tests"
cargo test --manifest-path "$ROOT_DIR/crates/helix-core/Cargo.toml"

echo "[release] helix-api tests"
cargo test --manifest-path "$ROOT_DIR/crates/helix-api/Cargo.toml"

echo "[release] Lean proofs"
(
  cd "$ROOT_DIR/formal/lean"
  lake build
)

echo "[release] UI build"
(
  cd "$ROOT_DIR/ui"
  npm run build
)

echo "[release] AssemblyScript SDK"
(
  cd "$ROOT_DIR/helix-ts-sdk"
  npm run asbuild
  npm test
)

echo "[release] complete"
