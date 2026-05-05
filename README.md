---
title: README
type: note
permalink: helix-platform-review/readme
---

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
- Shared intelligence triage kernel with replayable mixed-radix scoring across case queues and market-intel projections
- Deterministic market-intel priority scoring with mixed-radix ranking over escalation, severity, corroboration, freshness, trust, and signal density
- Deterministic case-queue ranking so dossier triage is ordered by bounded priority math instead of insertion order
- Deterministic evidence and claim queues with bounded ranking/filtering at the API boundary
- Deterministic credibility fusion for corroboration, exposed as bounded `credibility_bps` in priority breakdowns
- Deterministic autopilot review queue that merges cases, claims, and evidence into one bounded proposal worklist
- Onchain EVM transaction shell with dry-run support and receipt polling
- Autopilot control plane with `off`, `assist`, and `auto` modes
- Operator UI for dashboard, sources, watchlists, evidence, cases, policy, agents, credentials, automation, audit, onchain, and autopilot
- Effect-powered UI transport layer for typed browser-side timeouts, retries, and HTTP error normalization
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

Containerized local run with Postgres:

```bash
./scripts/run_compose.sh
```

Prebuilt release archives install a `helix` launcher with the UI bundled; see
[`docs/install.md`](docs/install.md).

Default local addresses:

- API: `http://127.0.0.1:3000`
- UI: `http://127.0.0.1:5173`
- Compose UI/API: `http://127.0.0.1:3000`

## Public Environment Configuration

See [`.env.example`](.env.example) for the public runtime surface.

Important variables:

- `HELIX_AUTH_REQUIRED`
- `HELIX_API_TOKEN`
- `HELIX_API_ADDR`
- `DATABASE_URL`
- `HELIX_AUTO_MIGRATE`
- `HELIX_ENCRYPTION_KEY`
- `HELIX_AUTOPILOT_MODE`
- `HELIX_AUTOPILOT_ALLOW_ONCHAIN`
- `HELIX_AUTOPILOT_REQUIRE_ONCHAIN_CONFIRMATION`
- `HELIX_AUTOPILOT_REQUIRE_DRY_RUN`
- `HELIX_AUTOPILOT_MAX_POLICY_COMMANDS`
- `HELIX_AUTOPILOT_LLM_MODEL`
- `LLM_API_KEY`
- `LLM_BASE_URL`

When `DATABASE_URL` is set, the API loads and saves intelligence desk sources,
watchlists, evidence, claims, cases, recipes, automation rules, rule evaluations,
recipe runs, redacted credential metadata, policy config snapshots, and autopilot
guard snapshots in Postgres. Mutating operator actions also write durable audit
records. Without it, Helix uses seeded in-memory state for local development.

Set `HELIX_AUTO_MIGRATE=true` to let the API apply the bundled idempotent
Postgres schema at startup. This is enabled by the Compose path and is useful
for installed binaries pointed at an empty database.

Credential vault writes also require `HELIX_ENCRYPTION_KEY` to contain a
hex-encoded 32-byte AES-GCM key:

```bash
export HELIX_ENCRYPTION_KEY="$(openssl rand -hex 32)"
```

### API Auth

By default, local development keeps `/api/v1/*` open on `127.0.0.1`. For shared
or production deployments, require bearer-token auth:

```bash
export HELIX_AUTH_REQUIRED=true
export HELIX_API_TOKEN="$(openssl rand -hex 32)"
```

Authenticated API calls must include:

```bash
Authorization: Bearer $HELIX_API_TOKEN
```

`/health` and the static UI fallback remain public so process health checks and
the installed UI shell can load before an operator supplies API credentials.

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
3. Review deterministic market-intel summaries, priority-scored case briefs, and tracked-company coverage.
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

### Automation
- `GET /api/v1/recipes`
- `POST /api/v1/recipes`
- `GET /api/v1/recipe-runs`
- `GET /api/v1/rules`
- `POST /api/v1/rules`
- `GET /api/v1/rules/evaluations`
- `POST /api/v1/rules/evaluate`
- `POST /api/v1/rules/trigger-plans/run`

The API recipe runner registers deterministic built-in native agents for local automation:
`record_state` writes a bounded state snapshot, `emit_event` emits a typed event, and `noop`
remains available only for compatibility. Recipe run history persists started agents,
emitted events, state snapshots, status, and errors when Postgres is enabled.

### Intelligence Desk
- `GET /api/v1/intel/overview`
- `GET /api/v1/market-intel/overview`
- `POST /api/v1/market-intel/cases/:case_id/brief`
- `GET /api/v1/market-intel/cases/:case_id/export`
- `GET /api/v1/sources`
- `POST /api/v1/sources`
- `POST /api/v1/sources/:source_id/collect`
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

### Audit
- `GET /api/v1/audit`

### Credentials
- `GET /api/v1/credentials?profile_id=:profile_id`
- `POST /api/v1/credentials`
- `DELETE /api/v1/credentials/:profile_id/:credential_id`

Reasoning requests are evaluated by deterministic compiled kernels:

- Symbolic mode uses indexed closure over normalized facts, rules, and triples.
- Symbolic responses include a deterministic support graph for replay and explanation.
- Symbolic requests accept `consistency_scope` with strict `global` default and `query_support` for intelligence-style localized contradiction handling.
- Symbolic traces report whether the closure saturated or hit the round bound, along with remaining ready rules.
- Symbolic traces include a minimal query-support slice, a stable compiled-program fingerprint, and the subset of contradictions that actually blocked the verdict.
- Contradictory symbolic closures deny automatically and return both sides of the contradiction.
- Expert mode uses compiled threshold matching with deterministic fail-closed voting.
- Neuro-symbolic mode still requires symbolic justification before neural confidence can permit execution.

`GET /api/v1/market-intel/overview` now includes deterministic priority breakdowns for themes, tracked companies, and case briefs so operator ordering is driven by bounded math instead of incidental count order.

`GET /api/v1/cases` now returns a priority-ranked queue with stable tie-breaks (`latest_signal_at`, then `case_id`) so case triage stays replayable across operators and restarts. The queue accepts deterministic filters for `status`, `severity`, `watchlist_id`, `primary_entity`, and bounded `limit`.

`GET /api/v1/evidence` and `GET /api/v1/claims` now return ranked queue entries instead of raw arrays. Both endpoints support bounded filter params and keep ordering deterministic rather than leaving prioritization to the browser.

Both endpoints also accept `q` or `semantic_query` for deterministic local semantic retrieval. Helix embeds evidence/claim text with a bounded lexical feature hash, returns `semantic_score_bps`, and ranks semantic matches ahead of the normal priority tie-breaks without calling external model services.

The corroboration axis now comes from a bounded evidence-fusion model instead of raw count thresholds. Helix computes a deterministic `credibility_bps` score from proposal, corroboration, and rejection signals using fixed-point noisy-or accumulation and fail-closed attenuation.

`GET /api/v1/autopilot/review-queue` merges non-closed cases, non-rejected claims, and evidence into one deterministic proposal queue. It accepts bounded `kind` and `limit` filters and keeps tie-breaks stable across refreshes.

`POST /api/v1/autopilot/review-queue/propose` lets the operator ask for a proposal directly from a ranked review item. The endpoint uses deterministic item context to build the proposer goal and then runs through the same guarded proposal path as manual requests.

`GET /api/v1/autopilot/review-queue/export` and `GET /api/v1/market-intel/cases/:case_id/export` return deterministic JSON packets for review items and market briefs. The packets are replayable exports built from ranked state, linked claims, linked evidence, and stable IDs.

`POST /api/v1/sources/:source_id/collect` fetches a configured RSS, JSON API, or website source, normalizes it into evidence, runs watchlist matching, and persists resulting claims/case updates. The request supplies an explicit `observed_at` fallback so collection does not depend on hidden server time. Sources can reference a vaulted `credential_id` plus an explicit HTTP header mapping; collection decrypts the credential just in time and never returns the secret in API responses or audit records.

`GET /api/v1/audit` returns the latest durable audit events for policy, autopilot, source, evidence, claim, case, automation, and credential mutations when Postgres persistence is enabled.

`POST /api/v1/credentials` stores credential secrets only as AES-GCM ciphertext in Postgres. List and upsert responses return redacted metadata only; plaintext secrets and encrypted blobs are not echoed in API responses or audit records. The endpoint fails closed unless both `DATABASE_URL` and a valid `HELIX_ENCRYPTION_KEY` are configured.

### Onchain
- `POST /api/v1/onchain/send_raw`
- `POST /api/v1/onchain/receipt`

### Autopilot
- `GET /api/v1/autopilot/status`
- `PUT /api/v1/autopilot/config`
- `GET /api/v1/autopilot/review-queue`
- `GET /api/v1/autopilot/review-queue/export`
- `POST /api/v1/autopilot/review-queue/propose`
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

The autopilot screen also renders the deterministic review queue directly from the API, can draft a policy proposal straight from a ranked item without manual copy/paste of the goal hint, can export deterministic review packets, and persists local review views plus draft workspace state across reloads.

The cases and evidence screens also persist local-only saved views and active filters across reloads, so analysts can keep deterministic queue slices without introducing new trusted backend state.

The shell now also keeps local-only operator preferences for desktop sidebar collapse and default landing route selection. Those preferences live in browser storage and do not create backend control state.

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
- [`COMPLETION_SCOPE.md`](COMPLETION_SCOPE.md): current release boundary, deferred backlog, and release gates

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
bash scripts/verify_release.sh
```

Run the UI golden-path browser test when changing operator workflows:

```bash
cd ui
PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH=/usr/bin/google-chrome npm run test:e2e
```

Public docs intentionally describe the verification entrypoints, not private internal tooling details.
