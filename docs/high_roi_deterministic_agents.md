# High-ROI Deterministic Agents

This document tracks deterministic agent kernels with the best risk/reward profile for Helix.

Source: `crates/helix-core/src/deterministic_agents.rs`

## Implemented

1. Dedup Window Agent
- ROI: drops duplicate bursts early to reduce downstream cost.
- Deterministic state: `(tick, window, entries)`.
- Inputs: `Observe`, `Tick`.
- Decisions: `Emit | DropDuplicate`.

2. Token Bucket Rate Limiter
- ROI: protects runtime and external dependencies from overload.
- Deterministic state: `(tokens, max_tokens, refill_per_tick)`.
- Inputs: `Tick`, `Request{cost}`.
- Decisions: `Allow | Deny`.

3. Circuit Breaker Agent
- ROI: fail-fast on unstable dependencies, controlled recovery.
- Deterministic state: `(phase, failure_count, timers, probe flag)`.
- Inputs: `Request`, `Success`, `Failure`, `Tick`.
- Decisions: `Allow | DenyOpen | Noop`.

4. Retry Budget Agent
- ROI: bounds retry cascades and queue blowups.
- Deterministic state: `(max_retries, remaining)`.
- Inputs: `ConsumeRetry`, `ResetCycle`.
- Decisions: `Retry | Exhausted | Noop`.

5. Approval Gate Agent
- ROI: strong deterministic guardrail for privileged actions.
- Deterministic state: `(approvals, rejects, quorum, reviewers)`.
- Inputs: `Approve`, `Reject`, `Reset`.
- Decisions: `Pending | Approved | Rejected`.

6. Backpressure Controller Agent
- ROI: deterministic throttle/shed behavior under queue pressure.
- Deterministic state: `(queue_depth, soft_limit, hard_limit)`.
- Inputs: `Enqueue{count}`, `Dequeue{count}`.
- Decisions: `Accept | Throttle | Shed`.

7. SLA Deadline Agent
- ROI: deterministic control for timeout and late-completion handling.
- Deterministic state: `(active, remaining_ticks, expired)`.
- Inputs: `StartWindow`, `Tick`, `Complete`, `Reset`.
- Decisions: `Pending | Expired | CompletedOnTime | CompletedLate | Noop`.

8. DLQ Budget Agent
- ROI: reroutes repeated failures to dead-letter path before system-wide impact.
- Deterministic state: `(max_consecutive_failures, consecutive_failures)`.
- Inputs: `Failure`, `Success`, `Reset`.
- Decisions: `Continue | RouteToDlq`.

9. Onchain Transaction Intent Agent
- ROI: deterministic transaction lifecycle control before chain side effects.
- Deterministic state: `(phase, tx_hash, poll_rounds, max_poll_rounds)`.
- Inputs: `StartBroadcast`, `SubmitAccepted`, `Receipt*`, `Reset`.
- Effects: `SubmitRawTransaction`, `PollReceipt` (executed by imperative shell only).

10. Nonce Manager Agent
- ROI: prevents nonce collision and stale nonce drift in parallel transaction flows.
- Deterministic state: `(next_nonce, in_flight, max_in_flight)`.
- Inputs: `Reserve`, `Confirm`, `Reconcile`.
- Decisions: `Reserved | Confirmed | Unknown | Reconciled`.

11. Fee Bidding Agent
- ROI: deterministic EIP-1559 bidding policy with bounded bumping after rejection.
- Deterministic state: `(base_fee, priority_fee, bump_bps, max_cap, rejection_count)`.
- Inputs: `UpdateBaseFee`, `Quote`, `MarkRejected`, `MarkConfirmed`.
- Decisions: `Quote | Noop`.

12. Finality Reorg Guard Agent
- ROI: prevents premature settlement and detects reorg hazard deterministically.
- Deterministic state: `(required_depth, observed_depth, phase)`.
- Inputs: `ObserveDepth`, `MarkReorg`, `Reset`.
- Decisions: `Pending | Finalized | ReorgDetected`.

13. Allowlist Policy Guard Agent
- ROI: blocks unauthorized chain/contract/method calls with explicit pause control.
- Deterministic state: `(allowed_tuple, paused)`.
- Inputs: `Evaluate`, `Pause`, `Resume`.
- Decisions: `Allow | DenyPaused | DenyNotAllowed`.

## Why these first

- They are cross-cutting controls used in most event-driven systems.
- They reduce outage blast radius before adding feature complexity.
- They compose cleanly with the formal execution kernel and ESSO flow.

## Formal Verification Artifacts

ESSO models for each machine:

- `formal/esso/roi_agents/dedup_window.yaml`
- `formal/esso/roi_agents/token_bucket.yaml`
- `formal/esso/roi_agents/circuit_breaker.yaml`
- `formal/esso/roi_agents/retry_budget.yaml`
- `formal/esso/roi_agents/approval_gate.yaml`
- `formal/esso/roi_agents/backpressure.yaml`
- `formal/esso/roi_agents/sla_deadline.yaml`
- `formal/esso/roi_agents/dlq_budget.yaml`
- `formal/esso/roi_agents/nonce_manager.yaml`
- `formal/esso/roi_agents/fee_bidding.yaml`
- `formal/esso/roi_agents/finality_guard.yaml`
- `formal/esso/roi_agents/allowlist_guard.yaml`
- `formal/esso/onchain_tx_intent.yaml`

Run all checks:

```bash
./scripts/verify_esso_roi_agents.sh
```
