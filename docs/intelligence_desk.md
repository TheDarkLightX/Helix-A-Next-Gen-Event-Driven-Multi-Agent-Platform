---
title: intelligence_desk
type: note
permalink: helix-platform-review/docs/intelligence-desk
---

# Helix Intelligence Desk

## Purpose

Helix 2.0 turns the platform into a self-hosted personal intelligence agency with reference workflows for OSINT and market intelligence.

The system is designed around one constraint: non-deterministic models may propose, but deterministic kernels decide.

## Product Surface

The intelligence desk currently exposes these first-class records:

- `SourceDefinition`: collection adapter metadata, profile boundary, optional vaulted credential reference, trust score, cadence, and tags
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
4. Use the market-intel priority breakdowns to rank work by escalation state, severity, corroboration, freshness, trust, and signal density.
5. Attach a deterministic brief to a market case when it is ready for handoff or escalation.
6. Reuse the same evidence, claim, and case substrate to drive competitor monitoring.

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
- `GET /api/v1/market-intel/cases/:case_id/export`

The market-intel overview includes deterministic priority breakdowns for theme cards, tracked companies, and case briefs. The ranking function is bounded, replayable, and independent of wall-clock time.

The corroboration component of that ranking is no longer a raw count bucket. It is derived from a deterministic credibility-fusion model that accumulates support signals, attenuates them with rejection signals, and exposes the resulting `credibility_bps` in the priority breakdown.

### Sources
- `GET /api/v1/sources`
- `POST /api/v1/sources`
- `POST /api/v1/sources/collect-due`
- `POST /api/v1/sources/:source_id/collect`
- `POST /api/v1/sources/:source_id/webhook`
- `POST /api/v1/sources/:source_id/import`

Pull-collection sources support `credential_id`, `credential_header_name`, and
`credential_header_prefix`. When configured, collection resolves the credential
inside the source's profile, decrypts it just in time, sends it as the configured
HTTP header, and records only redacted credential metadata in audit events.

Scheduled collection is explicit and replayable. `POST /api/v1/sources/collect-due`
accepts `observed_at`, `tick_minute`, optional `max_items_per_source`, and optional
`source_ids`; Helix derives a stable phase from each source and its cadence, then
collects only due RSS, JSON API, and website sources. No hidden server clock is
consulted.

Webhook sources are push-only. `POST /api/v1/sources/:source_id/webhook` accepts
a single evidence item, an array, or an envelope with explicit `observed_at` and
`items`. The endpoint requires `source.kind = webhook_ingest`, never reads server
time, caps one payload at 50 items, then runs the same evidence normalization,
watchlist matching, case update, persistence, and audit path as pull collection.

File-import sources are also push-only. `POST /api/v1/sources/:source_id/import`
accepts UTF-8 file content with explicit `file_name` and `observed_at`, rejects
empty or oversized content, and records the import through the same evidence,
watchlist, case, persistence, and audit path.

### Watchlists
- `GET /api/v1/watchlists`
- `POST /api/v1/watchlists`

### Evidence
- `GET /api/v1/evidence`
- `POST /api/v1/evidence/ingest`

The evidence endpoint returns a deterministic ranked queue with filters for `source_id`, `tag`, `entity`, `linked_status`, `min_trust`, semantic `q` / `semantic_query`, and bounded `limit`.

### Claims
- `GET /api/v1/claims`
- `POST /api/v1/claims/:claim_id/review`

The claim endpoint returns a deterministic ranked queue with filters for `review_status`, `predicate`, `subject`, `linked_status`, `min_confidence_bps`, semantic `q` / `semantic_query`, and bounded `limit`.

### Cases
- `GET /api/v1/cases`
- `POST /api/v1/cases/:case_id/transition`

The case endpoint returns a deterministic priority-ranked queue with explicit priority breakdowns and stable tie-breaks, so dossier ordering is consistent across refreshes and operators. It also accepts deterministic filters for `status`, `severity`, `watchlist_id`, `primary_entity`, and bounded `limit`.

### Autopilot Review
- `GET /api/v1/autopilot/review-queue`
- `GET /api/v1/autopilot/review-queue/export`
- `POST /api/v1/autopilot/review-queue/propose`

The autopilot review queue merges non-closed cases, non-rejected claims, and evidence into one deterministic proposal worklist. It accepts bounded `kind` and `limit` filters, and it preserves stable tie-breaks across refreshes.

The review-propose endpoint lets an operator draft a proposal directly from a ranked review item. It reuses the deterministic item context and the same guarded autopilot proposal path used by manual proposal requests.

The review-export and market-brief export endpoints return deterministic JSON packets built from ranked desk state, linked evidence, linked claims, and stable IDs. They are export surfaces, not new sources of truth.

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

The evidence screen handles ingestion, latest watchlist hits, latest case updates, and claim review decisions. It now also renders ranked evidence and claim queues directly from the shared deterministic triage kernel instead of ad hoc reverse-chronological lists, and it persists local-only saved views plus active filters across reloads. The market-intel screen renders deterministic theme cards, tracked companies, case briefs, priority breakdowns, reference playbooks, and export packets on top of the same desk state. The cases screen now renders the ranked queue directly from the shared deterministic triage kernel, exposes queue filters before lifecycle transitions, and persists local-only saved views plus active filters across reloads. The autopilot screen now renders the deterministic review queue so operators and proposer models are steered by the same ranked worklist, can draft a policy proposal directly from a selected review item, can export a deterministic review packet, and persists local saved views plus draft workspace state across reloads. The shell itself also persists local-only operator preferences for desktop sidebar collapse and default landing route selection.

## Autopilot Role

Autopilot is an execution interface, not a trust boundary.

- LLMs can draft actions or plans.
- The deterministic guard previews or denies them.
- Human confirmation remains configurable.
- High-risk actions can require dry-run and manual confirmation.

## Verification Posture

Helix ships verification scripts and replayable tests for the core logic. Public documentation describes the entrypoints and operator expectations, while private implementation details of the verification stack stay out of the public product narrative.
