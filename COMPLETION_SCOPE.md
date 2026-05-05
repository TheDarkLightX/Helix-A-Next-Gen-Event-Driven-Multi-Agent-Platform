# Helix Completion Scope

This document defines the current release target for Helix so completion work has
a concrete boundary.

## Current Release Target

Helix 2.0 ships as a self-hosted intelligence desk with guarded autopilot:

- deterministic policy simulation and guardrail evaluation
- deterministic agent catalog and guard simulation
- OSINT and market-intel source, watchlist, evidence, claim, and case APIs
- deterministic evidence, claim, case, market, and autopilot review queues
- symbolic, expert, neuro-risk, and neuro-symbolic reasoning gates
- onchain dry-run and guarded EVM transaction shell
- operator UI for dashboard, sources, watchlists, evidence, cases, policy,
  agents, onchain, market intel, and autopilot
- formal ESSO models and Lean proof artifacts for the currently modeled kernels
- AssemblyScript SDK scaffold for WASM agents

## Release Gates

The current release is not complete unless these pass in the current workspace:

```bash
bash scripts/verify_formal_core.sh
bash scripts/verify_formal_agents.sh
cargo test --manifest-path crates/helix-core/Cargo.toml
cargo test --manifest-path crates/helix-api/Cargo.toml
cd formal/lean && lake build
cd ui && npm run build
cd helix-ts-sdk && npm run asbuild && npm test
```

`cargo test --workspace` is a desired hardening gate, but it currently depends on
enough local disk for `trybuild` to compile an isolated dependency graph.

## Deferred Platform Backlog

The following are not required for the Helix 2.0 release target above unless the
product scope is explicitly expanded:

- full Rete-style rule engine and rule CRUD API
- Cedar-backed security policy management and storage
- production authentication and authorization
- persistent storage wiring for every intelligence-desk API surface
- commercial LLM provider parity beyond the OpenAI-compatible autopilot path
- local GGUF model orchestration and model resource scheduling
- RAG indexing, embedding generation, vector storage, and knowledge graphs
- production zkVM proving and verification backends
- full end-to-end browser automation suite
- plugin marketplace or ten-plugin OSS catalog target

## Known Release Cleanup

- keep the worktree clean and commit the current release slice intentionally
- make the ESSO verifier available from a tracked dependency, submodule, or
  public setup script before adding CI for the formal release gates
- resolve Rust warnings in touched crates before cutting a stable release tag
- decide whether deleted runtime integration tests are obsolete or need a
  replacement under the current agent-registry smoke-test model
