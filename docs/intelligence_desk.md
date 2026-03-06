# Helix Intelligence Desk

## Purpose

Helix 2.0 turns the platform into a self-hosted personal intelligence agency with reference workflows for OSINT and market intelligence.

The system is designed around one constraint: non-deterministic models may propose, but deterministic kernels decide.

## Product Surface

The intelligence desk currently exposes these first-class records:

- `SourceDefinition`: collection adapter metadata, trust score, cadence, and tags
- `EvidenceItem`: immutable normalized artifact with provenance hash, source, entities, and timestamps
- `ClaimRecord`: bounded assertion linked to evidence and review state
- `Watchlist`: deterministic keywords, entities, trust floor, severity, and enablement state
- `CaseFile`: dossier with evidence links, claim links, lifecycle status, and optional briefing summary

## Workflows

### OSINT Desk

1. Register a source.
2. Assign a trust score and collection cadence.
3. Define watchlists with explicit keyword and entity conditions.
4. Ingest evidence with provenance and claim proposals.
5. Review claims as `needs_review`, `corroborated`, or `rejected`.
6. Let deterministic matching open or update case files.
7. Move cases through `open`, `monitoring`, `brief_ready`, `escalated`, `closed`, and `reopen` transitions.
8. Use autopilot as a guarded proposer for follow-up actions.

### Market Intelligence

1. Register pricing, launch, partnership, and hiring sources.
2. Define deterministic watchlists for competitor motion.
3. Review market-intel theme coverage, tracked-company summaries, and deterministic case briefs.
4. Attach a deterministic brief to a market case when it is ready for handoff or escalation.
5. Reuse the same evidence, claim, and case substrate to drive competitor monitoring.

## Deterministic Boundaries

Helix keeps trusted state inside the functional core.

- Watchlist evaluation is deterministic.
- Case lifecycle transitions are deterministic.
- Claim review state is explicit and bounded.
- Unknown or malformed actions fail closed.
- Onchain execution stays behind explicit guardrails.

## API Surface

### Overview
- `GET /api/v1/intel/overview`
- `GET /api/v1/market-intel/overview`
- `POST /api/v1/market-intel/cases/:case_id/brief`

### Sources
- `GET /api/v1/sources`
- `POST /api/v1/sources`

### Watchlists
- `GET /api/v1/watchlists`
- `POST /api/v1/watchlists`

### Evidence
- `GET /api/v1/evidence`
- `POST /api/v1/evidence/ingest`

### Claims
- `GET /api/v1/claims`
- `POST /api/v1/claims/:claim_id/review`

### Cases
- `GET /api/v1/cases`
- `POST /api/v1/cases/:case_id/transition`

## UI Surface

The operator console includes these desk-oriented screens:

- Dashboard
- Market Intel
- Sources
- Watchlists
- Evidence
- Cases
- Policies
- Agents
- Onchain
- Autopilot

The evidence screen handles ingestion, latest watchlist hits, latest case updates, and claim review decisions. The market-intel screen renders deterministic theme cards, tracked companies, case briefs, and reference playbooks on top of the same desk state. The cases screen manages dossier lifecycle transitions.

## Autopilot Role

Autopilot is an execution interface, not a trust boundary.

- LLMs can draft actions or plans.
- The deterministic guard previews or denies them.
- Human confirmation remains configurable.
- High-risk actions can require dry-run and manual confirmation.

## Verification Posture

Helix ships verification scripts and replayable tests for the core logic. Public documentation describes the entrypoints and operator expectations, while private implementation details of the verification stack stay out of the public product narrative.
