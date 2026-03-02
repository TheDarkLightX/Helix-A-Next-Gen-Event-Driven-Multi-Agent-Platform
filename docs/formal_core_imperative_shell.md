# Helix: Formal Core + Imperative Shell

This repository now includes a correctness-first execution kernel flow:

1. `helix-core` contains a pure transition kernel:
   `crates/helix-core/src/execution_kernel.rs`
2. `helix-runtime` contains side-effect adapters:
   `crates/helix-runtime/src/imperative_shell.rs`
3. ESSO model mirrors the same finite state machine:
   `formal/esso/helix_execution_kernel.yaml`
4. On-chain transaction intent has its own pure kernel:
   `crates/helix-core/src/onchain_intent.rs`
5. On-chain ESSO model mirrors the intent lifecycle:
   `formal/esso/onchain_tx_intent.yaml`

## Contract

The execution kernel is finite and deterministic:

- `ExecutionPhase`: `Idle | Running | Succeeded | Failed`
- Inputs: `Start | AgentCompleted | AgentFailed | Reset`
- Invariants:
  - `Idle => remaining=0 && failed=false`
  - `Running => remaining>0 && failed=false`
  - `Succeeded => remaining=0 && failed=false`
  - `Failed => remaining=0 && failed=true`

The runtime shell applies effects emitted by the kernel instead of mutating
state ad hoc.

The same pattern is used for blockchain operations:

- Core intent machine emits `SubmitRawTransaction` / `PollReceipt` effects.
- Imperative shell performs EVM JSON-RPC calls.
- Results are fed back as deterministic inputs (`SubmitAccepted`, `Receipt*`).

## Verification Commands

Run ESSO checks:

```bash
./scripts/verify_esso_core.sh
```

Run Rust unit tests for the kernel:

```bash
cargo test --manifest-path crates/helix-core/Cargo.toml execution_kernel
```

## Why this pattern

- Pure kernel enables exhaustive reasoning and deterministic replay.
- Imperative shell isolates IO (events, DB, messaging, wasm runtime calls).
- ESSO model gives a fail-closed formal artifact in CI.
