# Helix Platform 1.0

Helix is a correctness-first, event-driven multi-agent platform with:

- Formal functional core (pure deterministic state machines)
- Imperative shell (all side effects at explicit boundaries)
- Operator UI control plane
- LLM-operable autopilot API with fail-closed guardrails

## What Ships In 1.0

- Deterministic policy engine with replayable simulations
- Deterministic high-ROI agent catalog
- On-chain EVM transaction shell (`send_raw` + receipt polling + dry-run)
- Autopilot guard for LLM-driven operation (`off` / `assist` / `auto`)

## Repository Layout

- `crates/helix-core`: deterministic kernels and policy/autopilot guard logic
- `crates/helix-api`: HTTP API and imperative adapters (onchain JSON-RPC)
- `crates/helix-runtime`: runtime shell and messaging
- `ui/`: React + TypeScript control plane
- `formal/`: formal model specs for core agents
- `scripts/`: setup, run, and verification scripts

## Fast Setup

```bash
./scripts/setup_local.sh
./scripts/run_local.sh
```

Endpoints after startup:

- API: `http://127.0.0.1:3000`
- UI: `http://127.0.0.1:5173`

## Quint Translator

`helix-llm` includes a CLI translator from plain English to [Quint](https://quint-lang.org) specs.

```bash
OPENAI_API_KEY=your_key_here \
cargo run -p helix-llm --bin quint_translator -- "describe a simple counter that increments"
```

You can also:

- Set `--model`, `--temperature`, `--output`, and `--base-url`
- Use `--prompt-file spec.txt`
- Use `OPENROUTER_API_KEY`, `OPENAI_API_KEY`, or `LLM_API_KEY`

The translator runs a lightweight intent-facet pass and surfaces clarifying questions when prompts are ambiguous.

## Core API

### Health
- `GET /health`

### Policy
- `GET /api/v1/policy/config`
- `PUT /api/v1/policy/config`
- `POST /api/v1/policy/simulate`

### Agent Catalog
- `GET /api/v1/agents`
- `GET /api/v1/agents/templates`
- `GET /api/v1/agents/templates/:template_id`
- `POST /api/v1/agents/templates/:template_id`

### Onchain (EVM)
- `POST /api/v1/onchain/send_raw`
- `POST /api/v1/onchain/receipt`

### Autopilot (LLM Operable)
- `GET /api/v1/autopilot/status`
- `PUT /api/v1/autopilot/config`
- `POST /api/v1/autopilot/execute`

## UI Routes

- `/` Dashboard
- `/policies` Policy workbench
- `/agents` Deterministic agent catalog
- `/onchain` Onchain shell
- `/autopilot` Autopilot control panel

## Autopilot Model

Autopilot is designed so an LLM can operate Helix the way a human operator would, with deterministic constraints:

- `off`: deny autonomous actions
- `assist`: require `confirmed_by_human=true` for every action
- `auto`: permit autonomous execution within guardrails

Guardrails include:

- Max policy command batch size
- On-chain enable/disable switch
- Optional mandatory `dry_run` for on-chain actions
- Denial reason codes for deterministic auditing

Environment controls (see `.env.example`):

- `HELIX_AUTOPILOT_MODE`
- `HELIX_AUTOPILOT_ALLOW_ONCHAIN`
- `HELIX_AUTOPILOT_REQUIRE_DRY_RUN`
- `HELIX_AUTOPILOT_MAX_POLICY_COMMANDS`

## Deterministic Agent Set (1.0)

Helix now ships **73 deterministic agent classes**:

- 13 foundational high-ROI kernels (dedup, token bucket, circuit breaker, retry budget, approval gate, backpressure, SLA, DLQ, nonce, fee bidding, finality/reorg, allowlist, onchain intent).
- 60 expanded guard classes spanning ingress integrity, SLO/incident control, tenant/compliance boundaries, payment/risk controls, and onchain execution safety.

Query the full live catalog:

```bash
curl -s http://127.0.0.1:3000/api/v1/agents | jq '.agents | length'
```

## Template-Driven Setup

Helix ships deterministic deployment templates so operators (or LLM autopilot flows) can apply proven policy profiles without manual tuning.

Example: inspect templates

```bash
curl -s http://127.0.0.1:3000/api/v1/agents/templates | jq
```

Example: apply secure onchain executor template and run bootstrap simulation

```bash
curl -s -X POST \
  http://127.0.0.1:3000/api/v1/agents/templates/secure_onchain_executor \
  -H 'content-type: application/json' \
  -d '{"run_bootstrap_simulation": true}' | jq
```

## Verification

### Rust tests

```bash
cargo test --manifest-path crates/helix-core/Cargo.toml --lib deterministic_agents
cargo test --manifest-path crates/helix-core/Cargo.toml --lib deterministic_policy
cargo test --manifest-path crates/helix-core/Cargo.toml --lib onchain_intent
cargo test --manifest-path crates/helix-api/Cargo.toml
cd ui && npm run build
```

### Formal checks

```bash
./scripts/verify_formal_core.sh
./scripts/verify_formal_agents.sh
```

If the private formal verifier is not in your default Python environment, set:

```bash
export HELIX_FORMAL_PYTHONPATH=/path/to/private/formal_verifier
export HELIX_FORMAL_MODULE=formal_verifier
```

## Privacy/Sanitization Notes

- Private formal-verifier integration is treated as internal operational tooling.
- Private/local verification artifacts are kept out of git (`/external`, `/runs`, local env files).
- Public docs describe verification entrypoints, not private internal infrastructure.
