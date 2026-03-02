// Copyright 2026 DarkLightX
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Deterministic policy engine composing high-ROI control machines.

use crate::deterministic_agents::{
    AllowlistDecision, AllowlistInput, AllowlistPolicyMachine, ApprovalDecision,
    ApprovalGateMachine, ApprovalInput, BackpressureDecision, BackpressureInput,
    BackpressureMachine, BreakerDecision, BreakerInput, CircuitBreakerMachine, DedupDecision,
    DedupInput, DedupMachine, DlqBudgetMachine, DlqDecision, DlqInput, FeeBiddingMachine,
    FeeDecision, FeeInput, FinalityDecision, FinalityGuardMachine, FinalityInput, NonceDecision,
    NonceInput, NonceManagerMachine, RateLimitDecision, RateLimitInput, RateLimiterMachine,
    RetryBudgetMachine, RetryDecision, RetryInput, SlaDecision, SlaDeadlineMachine, SlaInput,
};
use serde::{Deserialize, Serialize};

/// Configuration for deterministic policy engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeterministicPolicyConfig {
    /// Dedup logical window.
    pub dedup_window_ticks: u64,
    /// Token bucket capacity.
    pub rate_max_tokens: u16,
    /// Tokens added per tick.
    pub rate_refill_per_tick: u16,
    /// Consecutive failures before open.
    pub breaker_failure_threshold: u8,
    /// Open duration in ticks.
    pub breaker_open_duration_ticks: u8,
    /// Retry budget per cycle.
    pub retry_budget: u8,
    /// Approval quorum.
    pub approval_quorum: u16,
    /// Total reviewers.
    pub approval_reviewers: u16,
    /// Queue depth where throttling starts.
    pub backpressure_soft_limit: u16,
    /// Queue depth where shedding starts.
    pub backpressure_hard_limit: u16,
    /// SLA deadline in logical ticks.
    pub sla_deadline_ticks: u16,
    /// Consecutive failures before DLQ route.
    pub dlq_max_consecutive_failures: u8,
    /// Initial nonce cursor.
    pub nonce_start: u64,
    /// Maximum tracked in-flight nonces.
    pub nonce_max_in_flight: u16,
    /// Initial base fee for deterministic quoting.
    pub fee_base_fee: u64,
    /// Priority fee used in quotes.
    pub fee_priority_fee: u64,
    /// Fee bump in basis points per rejection.
    pub fee_bump_bps: u16,
    /// Hard cap for quoted max fee.
    pub fee_max_fee_cap: u64,
    /// Required chain confirmation depth.
    pub finality_required_depth: u16,
    /// Allowed chain id for policy guard.
    pub allowlist_chain_id: u32,
    /// Allowed contract tag for policy guard.
    pub allowlist_contract_tag: u64,
    /// Allowed method tag for policy guard.
    pub allowlist_method_tag: u32,
}

impl Default for DeterministicPolicyConfig {
    fn default() -> Self {
        Self {
            dedup_window_ticks: 2,
            rate_max_tokens: 10,
            rate_refill_per_tick: 2,
            breaker_failure_threshold: 3,
            breaker_open_duration_ticks: 3,
            retry_budget: 2,
            approval_quorum: 2,
            approval_reviewers: 3,
            backpressure_soft_limit: 4,
            backpressure_hard_limit: 7,
            sla_deadline_ticks: 4,
            dlq_max_consecutive_failures: 3,
            nonce_start: 0,
            nonce_max_in_flight: 64,
            fee_base_fee: 100,
            fee_priority_fee: 2,
            fee_bump_bps: 500,
            fee_max_fee_cap: 10_000,
            finality_required_depth: 12,
            allowlist_chain_id: 1,
            allowlist_contract_tag: 55,
            allowlist_method_tag: 0xdeadbeef,
        }
    }
}

/// One command in deterministic simulation or live policy evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PolicyCommand {
    /// Advance one logical tick.
    Tick,
    /// Evaluate a request through dedup, breaker, and rate limits.
    Request {
        /// Deterministic event fingerprint.
        fingerprint: u64,
        /// Rate-limit cost.
        cost: u16,
    },
    /// Record successful downstream execution.
    Success,
    /// Record downstream failure.
    Failure,
    /// Consume one retry token.
    Retry,
    /// Reset retry budget.
    ResetRetry,
    /// Add approval vote.
    Approve,
    /// Add reject vote.
    Reject,
    /// Reset approval votes.
    ResetApprovals,
    /// Start SLA deadline window.
    StartSlaWindow,
    /// Complete active SLA deadline window.
    CompleteSlaWindow,
    /// Reset SLA deadline state.
    ResetSlaWindow,
    /// Add queued work to backpressure controller.
    EnqueueBackpressure { count: u16 },
    /// Remove queued work from backpressure controller.
    DequeueBackpressure { count: u16 },
    /// Reset DLQ budget.
    ResetDlq,
    /// Reserve next nonce.
    NonceReserve,
    /// Confirm nonce from chain receipt.
    NonceConfirm {
        /// Confirmed nonce.
        nonce: u64,
    },
    /// Reconcile nonce cursor to chain next nonce.
    NonceReconcile {
        /// Observed chain next nonce.
        chain_next_nonce: u64,
    },
    /// Update observed base fee.
    FeeUpdateBaseFee {
        /// New base fee value.
        base_fee: u64,
    },
    /// Produce fee quote.
    FeeQuote {
        /// True to apply one extra urgency bump.
        urgent: bool,
    },
    /// Mark rejected bid (not landed).
    FeeRejected,
    /// Mark confirmed bid (landed).
    FeeConfirmed,
    /// Observe current confirmation depth.
    FinalityObserveDepth {
        /// Current confirmation depth.
        depth: u16,
    },
    /// Mark reorg detection.
    FinalityMarkReorg,
    /// Reset finality state.
    FinalityReset,
    /// Evaluate allowlist tuple.
    AllowlistEvaluate {
        /// Chain id.
        chain_id: u32,
        /// Contract tag.
        contract_tag: u64,
        /// Method tag.
        method_tag: u32,
    },
    /// Pause allowlist policy.
    AllowlistPause,
    /// Resume allowlist policy.
    AllowlistResume,
}

/// Deterministic decision emitted by policy engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PolicyDecision {
    /// Request admitted.
    RequestAccepted,
    /// Request denied with reason.
    RequestDenied {
        /// Denial reason.
        reason: String,
    },
    /// Retry decision.
    Retry {
        /// True if retry allowed.
        allowed: bool,
    },
    /// Approval decision.
    Approval {
        /// Current quorum state.
        decision: ApprovalDecision,
    },
    /// Backpressure classification.
    Backpressure {
        /// Admission state after queue update.
        decision: BackpressureDecision,
    },
    /// SLA deadline state transition.
    Sla {
        /// Current SLA status.
        status: SlaDecision,
    },
    /// DLQ routing state.
    Dlq {
        /// Current DLQ route decision.
        route: DlqDecision,
    },
    /// Nonce manager decision.
    Nonce {
        /// Decision outcome identifier.
        outcome: String,
        /// Reserved/confirmed nonce when applicable.
        nonce: Option<u64>,
        /// Reconciled next nonce when applicable.
        next_nonce: Option<u64>,
    },
    /// Fee bidding decision.
    Fee {
        /// True when a quote is emitted.
        quoted: bool,
        /// Quoted max fee.
        max_fee: Option<u64>,
        /// Quoted max priority fee.
        max_priority_fee: Option<u64>,
        /// Current rejection count.
        rejection_count: u8,
    },
    /// Finality guard state transition.
    Finality {
        /// Finality state label.
        state: String,
        /// Remaining depth for pending state.
        remaining_depth: Option<u16>,
    },
    /// Allowlist decision.
    Allowlist {
        /// Allowlist outcome label.
        decision: String,
    },
    /// No externally visible decision.
    Noop,
}

/// Snapshot of all machine states.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicySnapshot {
    /// Remaining rate limiter tokens.
    pub rate_tokens: u16,
    /// Breaker phase.
    pub breaker_phase: String,
    /// Remaining retry budget.
    pub retry_remaining: u8,
    /// Current queue depth tracked by backpressure controller.
    pub queue_depth: u16,
    /// Current consecutive failure count for DLQ route.
    pub dlq_consecutive_failures: u8,
    /// True if SLA deadline window is active.
    pub sla_active: bool,
    /// Remaining ticks in SLA window.
    pub sla_remaining_ticks: u16,
    /// True when SLA window is expired.
    pub sla_expired: bool,
    /// Next nonce cursor.
    pub nonce_next: u64,
    /// Count of tracked in-flight nonces.
    pub nonce_in_flight: u16,
    /// Current fee rejection count.
    pub fee_rejection_count: u8,
    /// Current observed finality depth.
    pub finality_observed_depth: u16,
    /// True when finality is reached.
    pub finality_finalized: bool,
    /// True when reorg is detected.
    pub finality_reorg_detected: bool,
    /// True when allowlist guard is paused.
    pub allowlist_paused: bool,
}

/// Step result with post-state snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyStepResult {
    /// Applied command.
    pub command: PolicyCommand,
    /// Decision emitted by command.
    pub decision: PolicyDecision,
    /// Snapshot after command.
    pub snapshot: PolicySnapshot,
}

/// Composed deterministic policy engine.
pub struct DeterministicPolicyEngine {
    dedup: DedupMachine,
    rate: RateLimiterMachine,
    breaker: CircuitBreakerMachine,
    retry: RetryBudgetMachine,
    approval: ApprovalGateMachine,
    backpressure: BackpressureMachine,
    sla: SlaDeadlineMachine,
    dlq: DlqBudgetMachine,
    nonce: NonceManagerMachine,
    fee: FeeBiddingMachine,
    finality: FinalityGuardMachine,
    allowlist: AllowlistPolicyMachine,
}

impl DeterministicPolicyEngine {
    /// Constructs engine from config.
    pub fn new(config: DeterministicPolicyConfig) -> Self {
        Self {
            dedup: DedupMachine::new(config.dedup_window_ticks),
            rate: RateLimiterMachine::new(config.rate_max_tokens, config.rate_refill_per_tick),
            breaker: CircuitBreakerMachine::new(
                config.breaker_failure_threshold,
                config.breaker_open_duration_ticks,
            ),
            retry: RetryBudgetMachine::new(config.retry_budget),
            approval: ApprovalGateMachine::new(config.approval_quorum, config.approval_reviewers),
            backpressure: BackpressureMachine::new(
                config.backpressure_soft_limit,
                config.backpressure_hard_limit,
            ),
            sla: SlaDeadlineMachine::new(config.sla_deadline_ticks),
            dlq: DlqBudgetMachine::new(config.dlq_max_consecutive_failures),
            nonce: NonceManagerMachine::new(config.nonce_start, config.nonce_max_in_flight),
            fee: FeeBiddingMachine::new(
                config.fee_base_fee,
                config.fee_priority_fee,
                config.fee_bump_bps,
                config.fee_max_fee_cap,
            ),
            finality: FinalityGuardMachine::new(config.finality_required_depth),
            allowlist: AllowlistPolicyMachine::new(
                config.allowlist_chain_id,
                config.allowlist_contract_tag,
                config.allowlist_method_tag,
            ),
        }
    }

    /// Applies one policy command.
    pub fn apply(&mut self, command: PolicyCommand) -> PolicyStepResult {
        let decision = match command {
            PolicyCommand::Tick => {
                let _ = self.dedup.step(DedupInput::Tick);
                let _ = self.rate.step(RateLimitInput::Tick);
                let _ = self.breaker.step(BreakerInput::Tick);
                match self.sla.step(SlaInput::Tick) {
                    SlaDecision::Expired => PolicyDecision::Sla {
                        status: SlaDecision::Expired,
                    },
                    _ => PolicyDecision::Noop,
                }
            }
            PolicyCommand::Request { fingerprint, cost } => {
                if matches!(
                    self.dedup.step(DedupInput::Observe { fingerprint }),
                    Some(DedupDecision::DropDuplicate)
                ) {
                    PolicyDecision::RequestDenied {
                        reason: "duplicate".to_string(),
                    }
                } else if matches!(self.breaker.step(BreakerInput::Request), BreakerDecision::DenyOpen)
                {
                    PolicyDecision::RequestDenied {
                        reason: "circuit_open".to_string(),
                    }
                } else if matches!(
                    self.rate.step(RateLimitInput::Request { cost }),
                    Some(RateLimitDecision::Deny)
                ) {
                    PolicyDecision::RequestDenied {
                        reason: "rate_limited".to_string(),
                    }
                } else {
                    PolicyDecision::RequestAccepted
                }
            }
            PolicyCommand::Success => {
                let _ = self.breaker.step(BreakerInput::Success);
                let _ = self.dlq.step(DlqInput::Success);
                PolicyDecision::Noop
            }
            PolicyCommand::Failure => {
                let _ = self.breaker.step(BreakerInput::Failure);
                PolicyDecision::Dlq {
                    route: self.dlq.step(DlqInput::Failure),
                }
            }
            PolicyCommand::Retry => {
                let allowed = matches!(self.retry.step(RetryInput::ConsumeRetry), RetryDecision::Retry);
                PolicyDecision::Retry { allowed }
            }
            PolicyCommand::ResetRetry => {
                let _ = self.retry.step(RetryInput::ResetCycle);
                PolicyDecision::Noop
            }
            PolicyCommand::Approve => PolicyDecision::Approval {
                decision: self.approval.step(ApprovalInput::Approve),
            },
            PolicyCommand::Reject => PolicyDecision::Approval {
                decision: self.approval.step(ApprovalInput::Reject),
            },
            PolicyCommand::ResetApprovals => PolicyDecision::Approval {
                decision: self.approval.step(ApprovalInput::Reset),
            },
            PolicyCommand::StartSlaWindow => PolicyDecision::Sla {
                status: self.sla.step(SlaInput::StartWindow),
            },
            PolicyCommand::CompleteSlaWindow => PolicyDecision::Sla {
                status: self.sla.step(SlaInput::Complete),
            },
            PolicyCommand::ResetSlaWindow => PolicyDecision::Sla {
                status: self.sla.step(SlaInput::Reset),
            },
            PolicyCommand::EnqueueBackpressure { count } => PolicyDecision::Backpressure {
                decision: self.backpressure.step(BackpressureInput::Enqueue { count }),
            },
            PolicyCommand::DequeueBackpressure { count } => PolicyDecision::Backpressure {
                decision: self.backpressure.step(BackpressureInput::Dequeue { count }),
            },
            PolicyCommand::ResetDlq => PolicyDecision::Dlq {
                route: self.dlq.step(DlqInput::Reset),
            },
            PolicyCommand::NonceReserve => match self.nonce.step(NonceInput::Reserve) {
                NonceDecision::Reserved { nonce } => PolicyDecision::Nonce {
                    outcome: "reserved".to_string(),
                    nonce: Some(nonce),
                    next_nonce: None,
                },
                NonceDecision::Confirmed { nonce } => PolicyDecision::Nonce {
                    outcome: "confirmed".to_string(),
                    nonce: Some(nonce),
                    next_nonce: None,
                },
                NonceDecision::Unknown { nonce } => PolicyDecision::Nonce {
                    outcome: "unknown".to_string(),
                    nonce: Some(nonce),
                    next_nonce: None,
                },
                NonceDecision::Reconciled { next_nonce } => PolicyDecision::Nonce {
                    outcome: "reconciled".to_string(),
                    nonce: None,
                    next_nonce: Some(next_nonce),
                },
            },
            PolicyCommand::NonceConfirm { nonce } => {
                match self.nonce.step(NonceInput::Confirm { nonce }) {
                    NonceDecision::Reserved { nonce } => PolicyDecision::Nonce {
                        outcome: "reserved".to_string(),
                        nonce: Some(nonce),
                        next_nonce: None,
                    },
                    NonceDecision::Confirmed { nonce } => PolicyDecision::Nonce {
                        outcome: "confirmed".to_string(),
                        nonce: Some(nonce),
                        next_nonce: None,
                    },
                    NonceDecision::Unknown { nonce } => PolicyDecision::Nonce {
                        outcome: "unknown".to_string(),
                        nonce: Some(nonce),
                        next_nonce: None,
                    },
                    NonceDecision::Reconciled { next_nonce } => PolicyDecision::Nonce {
                        outcome: "reconciled".to_string(),
                        nonce: None,
                        next_nonce: Some(next_nonce),
                    },
                }
            }
            PolicyCommand::NonceReconcile { chain_next_nonce } => {
                match self.nonce.step(NonceInput::Reconcile { chain_next_nonce }) {
                    NonceDecision::Reserved { nonce } => PolicyDecision::Nonce {
                        outcome: "reserved".to_string(),
                        nonce: Some(nonce),
                        next_nonce: None,
                    },
                    NonceDecision::Confirmed { nonce } => PolicyDecision::Nonce {
                        outcome: "confirmed".to_string(),
                        nonce: Some(nonce),
                        next_nonce: None,
                    },
                    NonceDecision::Unknown { nonce } => PolicyDecision::Nonce {
                        outcome: "unknown".to_string(),
                        nonce: Some(nonce),
                        next_nonce: None,
                    },
                    NonceDecision::Reconciled { next_nonce } => PolicyDecision::Nonce {
                        outcome: "reconciled".to_string(),
                        nonce: None,
                        next_nonce: Some(next_nonce),
                    },
                }
            }
            PolicyCommand::FeeUpdateBaseFee { base_fee } => {
                let _ = self.fee.step(FeeInput::UpdateBaseFee { base_fee });
                PolicyDecision::Noop
            }
            PolicyCommand::FeeQuote { urgent } => match self.fee.step(FeeInput::Quote { urgent }) {
                FeeDecision::Quote {
                    max_fee,
                    max_priority_fee,
                    rejection_count,
                } => PolicyDecision::Fee {
                    quoted: true,
                    max_fee: Some(max_fee),
                    max_priority_fee: Some(max_priority_fee),
                    rejection_count,
                },
                FeeDecision::Noop => PolicyDecision::Fee {
                    quoted: false,
                    max_fee: None,
                    max_priority_fee: None,
                    rejection_count: self.fee.rejection_count(),
                },
            },
            PolicyCommand::FeeRejected => {
                let _ = self.fee.step(FeeInput::MarkRejected);
                PolicyDecision::Noop
            }
            PolicyCommand::FeeConfirmed => {
                let _ = self.fee.step(FeeInput::MarkConfirmed);
                PolicyDecision::Noop
            }
            PolicyCommand::FinalityObserveDepth { depth } => {
                match self.finality.step(FinalityInput::ObserveDepth { depth }) {
                    FinalityDecision::Pending { remaining_depth } => PolicyDecision::Finality {
                        state: "pending".to_string(),
                        remaining_depth: Some(remaining_depth),
                    },
                    FinalityDecision::Finalized => PolicyDecision::Finality {
                        state: "finalized".to_string(),
                        remaining_depth: None,
                    },
                    FinalityDecision::ReorgDetected => PolicyDecision::Finality {
                        state: "reorg_detected".to_string(),
                        remaining_depth: None,
                    },
                }
            }
            PolicyCommand::FinalityMarkReorg => {
                match self.finality.step(FinalityInput::MarkReorg) {
                    FinalityDecision::Pending { remaining_depth } => PolicyDecision::Finality {
                        state: "pending".to_string(),
                        remaining_depth: Some(remaining_depth),
                    },
                    FinalityDecision::Finalized => PolicyDecision::Finality {
                        state: "finalized".to_string(),
                        remaining_depth: None,
                    },
                    FinalityDecision::ReorgDetected => PolicyDecision::Finality {
                        state: "reorg_detected".to_string(),
                        remaining_depth: None,
                    },
                }
            }
            PolicyCommand::FinalityReset => match self.finality.step(FinalityInput::Reset) {
                FinalityDecision::Pending { remaining_depth } => PolicyDecision::Finality {
                    state: "pending".to_string(),
                    remaining_depth: Some(remaining_depth),
                },
                FinalityDecision::Finalized => PolicyDecision::Finality {
                    state: "finalized".to_string(),
                    remaining_depth: None,
                },
                FinalityDecision::ReorgDetected => PolicyDecision::Finality {
                    state: "reorg_detected".to_string(),
                    remaining_depth: None,
                },
            },
            PolicyCommand::AllowlistEvaluate {
                chain_id,
                contract_tag,
                method_tag,
            } => match self.allowlist.step(AllowlistInput::Evaluate {
                chain_id,
                contract_tag,
                method_tag,
            }) {
                AllowlistDecision::Allow => PolicyDecision::Allowlist {
                    decision: "allow".to_string(),
                },
                AllowlistDecision::DenyPaused => PolicyDecision::Allowlist {
                    decision: "deny_paused".to_string(),
                },
                AllowlistDecision::DenyNotAllowed => PolicyDecision::Allowlist {
                    decision: "deny_not_allowed".to_string(),
                },
            },
            PolicyCommand::AllowlistPause => match self.allowlist.step(AllowlistInput::Pause) {
                AllowlistDecision::Allow => PolicyDecision::Allowlist {
                    decision: "allow".to_string(),
                },
                AllowlistDecision::DenyPaused => PolicyDecision::Allowlist {
                    decision: "deny_paused".to_string(),
                },
                AllowlistDecision::DenyNotAllowed => PolicyDecision::Allowlist {
                    decision: "deny_not_allowed".to_string(),
                },
            },
            PolicyCommand::AllowlistResume => match self.allowlist.step(AllowlistInput::Resume) {
                AllowlistDecision::Allow => PolicyDecision::Allowlist {
                    decision: "allow".to_string(),
                },
                AllowlistDecision::DenyPaused => PolicyDecision::Allowlist {
                    decision: "deny_paused".to_string(),
                },
                AllowlistDecision::DenyNotAllowed => PolicyDecision::Allowlist {
                    decision: "deny_not_allowed".to_string(),
                },
            },
        };

        PolicyStepResult {
            command,
            decision,
            snapshot: self.snapshot(),
        }
    }

    /// Runs a deterministic simulation over commands.
    pub fn simulate(&mut self, commands: &[PolicyCommand]) -> Vec<PolicyStepResult> {
        commands.iter().copied().map(|c| self.apply(c)).collect()
    }

    /// Returns current state snapshot.
    pub fn snapshot(&self) -> PolicySnapshot {
        PolicySnapshot {
            rate_tokens: self.rate.tokens(),
            breaker_phase: format!("{:?}", self.breaker.phase()),
            retry_remaining: self.retry.remaining(),
            queue_depth: self.backpressure.queue_depth(),
            dlq_consecutive_failures: self.dlq.consecutive_failures(),
            sla_active: self.sla.active(),
            sla_remaining_ticks: self.sla.remaining_ticks(),
            sla_expired: self.sla.expired(),
            nonce_next: self.nonce.next_nonce(),
            nonce_in_flight: self.nonce.in_flight_len().min(usize::from(u16::MAX)) as u16,
            fee_rejection_count: self.fee.rejection_count(),
            finality_observed_depth: self.finality.observed_depth(),
            finality_finalized: self.finality.is_finalized(),
            finality_reorg_detected: self.finality.reorg_detected(),
            allowlist_paused: self.allowlist.paused(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_is_denied_as_duplicate() {
        let mut engine = DeterministicPolicyEngine::new(DeterministicPolicyConfig::default());
        let first = engine.apply(PolicyCommand::Request {
            fingerprint: 7,
            cost: 1,
        });
        assert!(matches!(first.decision, PolicyDecision::RequestAccepted));
        let second = engine.apply(PolicyCommand::Request {
            fingerprint: 7,
            cost: 1,
        });
        assert!(matches!(
            second.decision,
            PolicyDecision::RequestDenied { reason } if reason == "duplicate"
        ));
    }

    #[test]
    fn breaker_denies_when_open() {
        let mut cfg = DeterministicPolicyConfig::default();
        cfg.breaker_failure_threshold = 1;
        let mut engine = DeterministicPolicyEngine::new(cfg);
        let _ = engine.apply(PolicyCommand::Failure);
        let request = engine.apply(PolicyCommand::Request {
            fingerprint: 100,
            cost: 1,
        });
        assert!(matches!(
            request.decision,
            PolicyDecision::RequestDenied { reason } if reason == "circuit_open"
        ));
    }

    #[test]
    fn approval_reaches_approved() {
        let mut engine = DeterministicPolicyEngine::new(DeterministicPolicyConfig::default());
        let first = engine.apply(PolicyCommand::Approve);
        assert!(matches!(
            first.decision,
            PolicyDecision::Approval {
                decision: ApprovalDecision::Pending
            }
        ));
        let second = engine.apply(PolicyCommand::Approve);
        assert!(matches!(
            second.decision,
            PolicyDecision::Approval {
                decision: ApprovalDecision::Approved
            }
        ));
    }

    #[test]
    fn dlq_routes_after_consecutive_failures() {
        let mut cfg = DeterministicPolicyConfig::default();
        cfg.dlq_max_consecutive_failures = 2;
        let mut engine = DeterministicPolicyEngine::new(cfg);
        let first = engine.apply(PolicyCommand::Failure);
        assert!(matches!(
            first.decision,
            PolicyDecision::Dlq {
                route: DlqDecision::Continue
            }
        ));
        let second = engine.apply(PolicyCommand::Failure);
        assert!(matches!(
            second.decision,
            PolicyDecision::Dlq {
                route: DlqDecision::RouteToDlq
            }
        ));
    }

    #[test]
    fn backpressure_reports_shed_at_hard_limit() {
        let mut cfg = DeterministicPolicyConfig::default();
        cfg.backpressure_soft_limit = 2;
        cfg.backpressure_hard_limit = 3;
        let mut engine = DeterministicPolicyEngine::new(cfg);
        let _ = engine.apply(PolicyCommand::EnqueueBackpressure { count: 2 });
        let result = engine.apply(PolicyCommand::EnqueueBackpressure { count: 1 });
        assert!(matches!(
            result.decision,
            PolicyDecision::Backpressure {
                decision: BackpressureDecision::Shed
            }
        ));
        assert_eq!(result.snapshot.queue_depth, 3);
    }

    #[test]
    fn sla_expiration_is_observable_on_tick() {
        let mut cfg = DeterministicPolicyConfig::default();
        cfg.sla_deadline_ticks = 1;
        let mut engine = DeterministicPolicyEngine::new(cfg);
        let _ = engine.apply(PolicyCommand::StartSlaWindow);
        let tick = engine.apply(PolicyCommand::Tick);
        assert!(matches!(
            tick.decision,
            PolicyDecision::Sla {
                status: SlaDecision::Expired
            }
        ));
        let done = engine.apply(PolicyCommand::CompleteSlaWindow);
        assert!(matches!(
            done.decision,
            PolicyDecision::Sla {
                status: SlaDecision::CompletedLate
            }
        ));
    }

    #[test]
    fn nonce_reserve_is_monotonic() {
        let mut cfg = DeterministicPolicyConfig::default();
        cfg.nonce_start = 7;
        let mut engine = DeterministicPolicyEngine::new(cfg);
        let a = engine.apply(PolicyCommand::NonceReserve);
        let b = engine.apply(PolicyCommand::NonceReserve);
        assert!(matches!(
            a.decision,
            PolicyDecision::Nonce {
                outcome,
                nonce: Some(7),
                next_nonce: None
            } if outcome == "reserved"
        ));
        assert!(matches!(
            b.decision,
            PolicyDecision::Nonce {
                outcome,
                nonce: Some(8),
                next_nonce: None
            }
            if outcome == "reserved"
        ));
        assert_eq!(b.snapshot.nonce_next, 9);
        assert_eq!(b.snapshot.nonce_in_flight, 2);
    }

    #[test]
    fn fee_quote_grows_after_rejection() {
        let mut cfg = DeterministicPolicyConfig::default();
        cfg.fee_base_fee = 100;
        cfg.fee_priority_fee = 2;
        cfg.fee_bump_bps = 500;
        let mut engine = DeterministicPolicyEngine::new(cfg);
        let first = engine.apply(PolicyCommand::FeeQuote { urgent: false });
        let first_fee = match first.decision {
            PolicyDecision::Fee {
                quoted: true,
                max_fee: Some(max_fee),
                max_priority_fee: _,
                rejection_count: _,
            } => max_fee,
            _ => panic!("expected fee quote"),
        };
        let _ = engine.apply(PolicyCommand::FeeRejected);
        let second = engine.apply(PolicyCommand::FeeQuote { urgent: false });
        let second_fee = match second.decision {
            PolicyDecision::Fee {
                quoted: true,
                max_fee: Some(max_fee),
                max_priority_fee: _,
                rejection_count: _,
            } => max_fee,
            _ => panic!("expected fee quote"),
        };
        assert!(second_fee >= first_fee);
        assert_eq!(second.snapshot.fee_rejection_count, 1);
    }

    #[test]
    fn finality_reaches_finalized_state() {
        let mut cfg = DeterministicPolicyConfig::default();
        cfg.finality_required_depth = 2;
        let mut engine = DeterministicPolicyEngine::new(cfg);
        let p = engine.apply(PolicyCommand::FinalityObserveDepth { depth: 1 });
        assert!(matches!(
            p.decision,
            PolicyDecision::Finality {
                state,
                remaining_depth: Some(1)
            } if state == "pending"
        ));
        let f = engine.apply(PolicyCommand::FinalityObserveDepth { depth: 2 });
        assert!(matches!(
            f.decision,
            PolicyDecision::Finality {
                state,
                remaining_depth: None
            } if state == "finalized"
        ));
        assert!(f.snapshot.finality_finalized);
    }

    #[test]
    fn allowlist_denies_unknown_tuple() {
        let mut cfg = DeterministicPolicyConfig::default();
        cfg.allowlist_chain_id = 1;
        cfg.allowlist_contract_tag = 55;
        cfg.allowlist_method_tag = 0xdeadbeef;
        let mut engine = DeterministicPolicyEngine::new(cfg);
        let allowed = engine.apply(PolicyCommand::AllowlistEvaluate {
            chain_id: 1,
            contract_tag: 55,
            method_tag: 0xdeadbeef,
        });
        assert!(matches!(
            allowed.decision,
            PolicyDecision::Allowlist { decision } if decision == "allow"
        ));
        let denied = engine.apply(PolicyCommand::AllowlistEvaluate {
            chain_id: 1,
            contract_tag: 99,
            method_tag: 0xdeadbeef,
        });
        assert!(matches!(
            denied.decision,
            PolicyDecision::Allowlist { decision } if decision == "deny_not_allowed"
        ));
    }
}
