# Helix Platform 2.0

Helix is a self-hosted personal intelligence agency platform for operators who want deterministic control, replayable evidence, and optional autopilot.

Core rule: LLMs can propose actions, summaries, and claims. Deterministic kernels decide what is accepted, denied, escalated, or executed.

## Product Thesis

Helix is not positioned as a generic automation clone. The main product surface is a self-hosted intelligence desk built on a functional core and imperative shell:

- Sources with explicit trust scores and bounded collection cadence
- Evidence with provenance, timestamps, entities, and claim proposals
- Watchlists with deterministic keyword/entity matching and severity
- Case files with deterministic lifecycle transitions
- First-party verticals for OSINT and market intelligence
- Compiled symbolic, expert, neural-risk, and neuro-symbolic reasoning backends with fail-closed decisions
- Guarded autopilot for LLM-proposed actions

## What Ships

- Deterministic policy engine with replayable simulations
- Deterministic agent catalog and deployment templates
- Reasoning backends: indexed symbolic closure, compiled expert matching, neural-risk, and neuro-symbolic
- Intelligence desk APIs for sources, evidence, claims, watchlists, cases, and market-intel projections
- Onchain EVM transaction shell with dry-run support and receipt polling
- Autopilot control plane with `off`, `assist`, and `auto` modes
- Operator UI for dashboard, sources, watchlists, evidence, cases, policy, agents, onchain, and autopilot
- GitHub Pages marketing site under `site/`

## Architecture

Helix uses a functional core / imperative shell design:

- Pure kernels own policy, guardrails, reasoning decisions, and case lifecycle
- Adapters own HTTP, storage, JSON-RPC, webhooks, and LLM calls
- Non-deterministic systems can propose inputs, but they do not mutate trusted state directly
- Every risky path is designed to fail closed on ambiguity, invalid input, timeout, or unknown state

## Quickstart

```bash
./scripts/setup_local.sh
./scripts/run_local.sh
```

Default local addresses:

- API: `http://127.0.0.1:3000`
- UI: `http://127.0.0.1:5173`

## Public Environment Configuration

See [`.env.example`](.env.example) for the public runtime surface.

Important variables:

- `HELIX_AUTOPILOT_MODE`
- `HELIX_AUTOPILOT_ALLOW_ONCHAIN`
- `HELIX_AUTOPILOT_REQUIRE_ONCHAIN_CONFIRMATION`
- `HELIX_AUTOPILOT_REQUIRE_DRY_RUN`
- `HELIX_AUTOPILOT_MAX_POLICY_COMMANDS`
- `HELIX_AUTOPILOT_LLM_MODEL`
- `LLM_API_KEY`
- `LLM_BASE_URL`

Self-hosted LLM example:

```bash
export HELIX_AUTOPILOT_LLM_MODEL="qwen-3.5"
export LLM_BASE_URL="http://127.0.0.1:8000/v1"
export LLM_API_KEY="local"
```

## Reference Workflows

### OSINT Desk

1. Register trust-scored sources.
2. Define deterministic watchlists.
3. Ingest evidence with provenance and claim proposals.
4. Review claims and corroborate or reject them.
5. Let watchlist hits open or update case files.
6. Move cases through monitoring, brief-ready, escalation, closure, and reopen flows.
7. Use autopilot only as a guarded proposer.

### Market Intelligence

1. Register competitor, pricing, launch, partner, and hiring sources.
2. Define watchlists for pricing moves, product launches, partnerships, and expansion signals.
3. Review deterministic market-intel summaries, case briefs, and tracked-company coverage.
4. Attach deterministic briefs to market cases through an explicit case transition.
5. Use the same evidence, claim, and case substrate for competitor monitoring.

## Core API

### Health
- `GET /health`

### Policy
- `GET /api/v1/policy/config`
- `PUT /api/v1/policy/config`
- `POST /api/v1/policy/simulate`

### Agent Catalog
- `GET /api/v1/agents`
- `GET /api/v1/agents/quality`
- `POST /api/v1/agents/guards/simulate`
- `GET /api/v1/agents/templates`
- `GET /api/v1/agents/templates/:template_id`
- `POST /api/v1/agents/templates/:template_id`

### Intelligence Desk
- `GET /api/v1/intel/overview`
- `GET /api/v1/market-intel/overview`
- `POST /api/v1/market-intel/cases/:case_id/brief`
- `GET /api/v1/sources`
- `POST /api/v1/sources`
- `GET /api/v1/watchlists`
- `POST /api/v1/watchlists`
- `GET /api/v1/evidence`
- `POST /api/v1/evidence/ingest`
- `GET /api/v1/claims`
- `POST /api/v1/claims/:claim_id/review`
- `GET /api/v1/cases`
- `POST /api/v1/cases/:case_id/transition`

### Reasoning
- `POST /api/v1/reasoning/evaluate`

Reasoning requests are evaluated by deterministic compiled kernels:

- Symbolic mode uses indexed closure over normalized facts, rules, and triples.
- Symbolic responses include a deterministic support graph for replay and explanation.
- Symbolic traces report whether the closure saturated or hit the round bound, along with remaining ready rules.
- Symbolic traces include a minimal query-support slice and a stable compiled-program fingerprint for replay.
- Contradictory symbolic closures deny automatically and return both sides of the contradiction.
- Expert mode uses compiled threshold matching with deterministic fail-closed voting.
- Neuro-symbolic mode still requires symbolic justification before neural confidence can permit execution.

### Onchain
- `POST /api/v1/onchain/send_raw`
- `POST /api/v1/onchain/receipt`

### Autopilot
- `GET /api/v1/autopilot/status`
- `PUT /api/v1/autopilot/config`
- `POST /api/v1/autopilot/propose`
- `POST /api/v1/autopilot/execute`

## UI Routes

- `/` Dashboard
- `/market-intel` Market intelligence dashboard
- `/sources` Source registry
- `/watchlists` Watchlist management
- `/evidence` Evidence ingest and claim review
- `/cases` Case queue and lifecycle controls
- `/policies` Policy workbench
- `/agents` Deterministic agent catalog
- `/onchain` Onchain shell
- `/autopilot` Autopilot control panel

## Autopilot Model

Autopilot treats model output as untrusted input.

- `off`: deny autonomous actions
- `assist`: require `confirmed_by_human=true`
- `auto`: allow bounded execution within the configured guardrails

Default guardrails include:

- bounded policy command batch size
- onchain enable or disable switch
- optional required human confirmation for onchain actions
- optional required `dry_run` for onchain actions
- stable denial reasons for audit and replay

`POST /api/v1/autopilot/propose` calls an OpenAI-compatible endpoint to draft a typed action and then previews how the deterministic guard would evaluate it with and without human confirmation.

## GitHub Pages Site

The public marketing site lives in `site/` and deploys through `.github/workflows/pages.yml`.

Preview locally:

```bash
python3 -m http.server -d site 8080
```

## Documentation

- [`docs/intelligence_desk.md`](docs/intelligence_desk.md): product model, OSINT + market-intel workflows, APIs, and operator surfaces
- [`docs/formal_core_imperative_shell.md`](docs/formal_core_imperative_shell.md): functional core / imperative shell rationale
- [`docs/high_roi_deterministic_agents.md`](docs/high_roi_deterministic_agents.md): deterministic agent set and deployment posture

## Additional Component

`helix-llm` includes a CLI translator from plain English to [Quint](https://quint-lang.org) specs.

```bash
OPENAI_API_KEY=your_key_here \
cargo run -p helix-llm --bin quint_translator -- "describe a simple counter that increments"
```

The translator supports:

- `--model`
- `--temperature`
- `--output`
- `--base-url`
- `--prompt-file`

## Verification

Run the project verification gates before release work:

```bash
bash scripts/verify_formal_core.sh
bash scripts/verify_formal_agents.sh
cargo test --manifest-path crates/helix-core/Cargo.toml
cargo test --manifest-path crates/helix-api/Cargo.toml
cd ui && npm run build
```

Public docs intentionally describe the verification entrypoints, not private internal tooling details.
