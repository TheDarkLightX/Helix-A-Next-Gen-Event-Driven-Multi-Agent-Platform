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

//! Deterministic, high-ROI agent kernels.
//!
//! These machines are pure: no network, clock, or storage side effects.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Action emitted by a dedup agent step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DedupDecision {
    /// Event is new and should continue in the pipeline.
    Emit,
    /// Event is a duplicate in the current window.
    DropDuplicate,
}

/// Input to dedup agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DedupInput {
    /// Observe one event fingerprint.
    Observe { fingerprint: u64 },
    /// Advance one logical time unit.
    Tick,
}

/// Finite-window dedup state machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DedupMachine {
    tick: u64,
    window_ticks: u64,
    entries: VecDeque<(u64, u64)>, // (fingerprint, expires_at)
}

impl DedupMachine {
    /// Creates a dedup machine.
    pub fn new(window_ticks: u64) -> Self {
        Self {
            tick: 0,
            window_ticks: window_ticks.max(1),
            entries: VecDeque::new(),
        }
    }

    fn purge(&mut self) {
        while let Some((_, expires_at)) = self.entries.front().copied() {
            if expires_at <= self.tick {
                let _ = self.entries.pop_front();
            } else {
                break;
            }
        }
    }

    /// Applies one input deterministically.
    pub fn step(&mut self, input: DedupInput) -> Option<DedupDecision> {
        match input {
            DedupInput::Tick => {
                self.tick = self.tick.saturating_add(1);
                self.purge();
                None
            }
            DedupInput::Observe { fingerprint } => {
                self.purge();
                let duplicate = self
                    .entries
                    .iter()
                    .any(|(fp, exp)| *fp == fingerprint && *exp > self.tick);
                if duplicate {
                    Some(DedupDecision::DropDuplicate)
                } else {
                    self.entries
                        .push_back((fingerprint, self.tick.saturating_add(self.window_ticks)));
                    Some(DedupDecision::Emit)
                }
            }
        }
    }
}

/// Action emitted by the rate limiter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RateLimitDecision {
    /// Request allowed.
    Allow,
    /// Request denied due to insufficient tokens.
    Deny,
}

/// Input to token bucket limiter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RateLimitInput {
    /// Refill one tick.
    Tick,
    /// Consume a request cost.
    Request { cost: u16 },
}

/// Deterministic token bucket limiter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimiterMachine {
    tokens: u16,
    max_tokens: u16,
    refill_per_tick: u16,
}

impl RateLimiterMachine {
    /// Creates a limiter with full bucket.
    pub fn new(max_tokens: u16, refill_per_tick: u16) -> Self {
        Self {
            tokens: max_tokens,
            max_tokens,
            refill_per_tick,
        }
    }

    /// Current available tokens.
    pub fn tokens(self) -> u16 {
        self.tokens
    }

    /// Applies one limiter input.
    pub fn step(&mut self, input: RateLimitInput) -> Option<RateLimitDecision> {
        match input {
            RateLimitInput::Tick => {
                self.tokens = self.tokens.saturating_add(self.refill_per_tick).min(self.max_tokens);
                None
            }
            RateLimitInput::Request { cost } => {
                if cost == 0 {
                    return Some(RateLimitDecision::Allow);
                }
                if self.tokens >= cost {
                    self.tokens -= cost;
                    Some(RateLimitDecision::Allow)
                } else {
                    Some(RateLimitDecision::Deny)
                }
            }
        }
    }
}

/// Circuit breaker phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakerPhase {
    /// Healthy path.
    Closed,
    /// Fast-fail path.
    Open,
    /// Probe path.
    HalfOpen,
}

/// Input to circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakerInput {
    /// Logical clock tick.
    Tick,
    /// Check if request can proceed.
    Request,
    /// Observe downstream success.
    Success,
    /// Observe downstream failure.
    Failure,
}

/// Decision from circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakerDecision {
    /// Request can proceed.
    Allow,
    /// Request rejected while breaker open.
    DenyOpen,
    /// No request decision for this input.
    Noop,
}

/// Deterministic circuit breaker machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CircuitBreakerMachine {
    phase: BreakerPhase,
    failure_count: u8,
    failure_threshold: u8,
    open_ticks_left: u8,
    open_duration_ticks: u8,
    half_open_probe_in_flight: bool,
}

impl CircuitBreakerMachine {
    /// Creates a breaker in closed state.
    pub fn new(failure_threshold: u8, open_duration_ticks: u8) -> Self {
        Self {
            phase: BreakerPhase::Closed,
            failure_count: 0,
            failure_threshold: failure_threshold.max(1),
            open_ticks_left: 0,
            open_duration_ticks: open_duration_ticks.max(1),
            half_open_probe_in_flight: false,
        }
    }

    /// Current phase.
    pub fn phase(self) -> BreakerPhase {
        self.phase
    }

    /// Applies one breaker input.
    pub fn step(&mut self, input: BreakerInput) -> BreakerDecision {
        match input {
            BreakerInput::Tick => {
                if self.phase == BreakerPhase::Open && self.open_ticks_left > 0 {
                    self.open_ticks_left -= 1;
                    if self.open_ticks_left == 0 {
                        self.phase = BreakerPhase::HalfOpen;
                        self.half_open_probe_in_flight = false;
                    }
                }
                BreakerDecision::Noop
            }
            BreakerInput::Request => match self.phase {
                BreakerPhase::Closed => BreakerDecision::Allow,
                BreakerPhase::Open => BreakerDecision::DenyOpen,
                BreakerPhase::HalfOpen => {
                    if self.half_open_probe_in_flight {
                        BreakerDecision::DenyOpen
                    } else {
                        self.half_open_probe_in_flight = true;
                        BreakerDecision::Allow
                    }
                }
            },
            BreakerInput::Success => {
                match self.phase {
                    BreakerPhase::Closed => self.failure_count = 0,
                    BreakerPhase::HalfOpen => {
                        self.phase = BreakerPhase::Closed;
                        self.failure_count = 0;
                        self.half_open_probe_in_flight = false;
                    }
                    BreakerPhase::Open => {}
                }
                BreakerDecision::Noop
            }
            BreakerInput::Failure => {
                match self.phase {
                    BreakerPhase::Closed => {
                        self.failure_count = self.failure_count.saturating_add(1);
                        if self.failure_count >= self.failure_threshold {
                            self.phase = BreakerPhase::Open;
                            self.open_ticks_left = self.open_duration_ticks;
                        }
                    }
                    BreakerPhase::HalfOpen => {
                        self.phase = BreakerPhase::Open;
                        self.open_ticks_left = self.open_duration_ticks;
                        self.half_open_probe_in_flight = false;
                    }
                    BreakerPhase::Open => {}
                }
                BreakerDecision::Noop
            }
        }
    }
}

/// Retry decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetryDecision {
    /// Retry permitted.
    Retry,
    /// Retry denied.
    Exhausted,
    /// No decision for this input.
    Noop,
}

/// Retry input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetryInput {
    /// Start a new cycle and reset budget.
    ResetCycle,
    /// Ask for one retry token.
    ConsumeRetry,
}

/// Deterministic retry budget machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryBudgetMachine {
    max_retries: u8,
    remaining: u8,
}

impl RetryBudgetMachine {
    /// Creates a retry budget.
    pub fn new(max_retries: u8) -> Self {
        Self {
            max_retries,
            remaining: max_retries,
        }
    }

    /// Remaining retries.
    pub fn remaining(self) -> u8 {
        self.remaining
    }

    /// Applies one retry input.
    pub fn step(&mut self, input: RetryInput) -> RetryDecision {
        match input {
            RetryInput::ResetCycle => {
                self.remaining = self.max_retries;
                RetryDecision::Noop
            }
            RetryInput::ConsumeRetry => {
                if self.remaining > 0 {
                    self.remaining -= 1;
                    RetryDecision::Retry
                } else {
                    RetryDecision::Exhausted
                }
            }
        }
    }
}

/// Approval decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalDecision {
    /// Not enough votes yet.
    Pending,
    /// Approved by quorum.
    Approved,
    /// Rejected by impossible quorum.
    Rejected,
}

/// Approval input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalInput {
    /// Positive vote.
    Approve,
    /// Negative vote.
    Reject,
    /// Reset votes.
    Reset,
}

/// Deterministic quorum approval machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalGateMachine {
    approvals: u16,
    rejects: u16,
    quorum: u16,
    reviewers: u16,
}

impl ApprovalGateMachine {
    /// Creates a quorum gate.
    pub fn new(quorum: u16, reviewers: u16) -> Self {
        Self {
            approvals: 0,
            rejects: 0,
            quorum,
            reviewers: reviewers.max(1),
        }
    }

    fn decide(self) -> ApprovalDecision {
        if self.approvals >= self.quorum {
            return ApprovalDecision::Approved;
        }
        let remaining_votes = self.reviewers.saturating_sub(self.approvals + self.rejects);
        if self.approvals + remaining_votes < self.quorum {
            ApprovalDecision::Rejected
        } else {
            ApprovalDecision::Pending
        }
    }

    /// Applies one approval input and returns current decision.
    pub fn step(&mut self, input: ApprovalInput) -> ApprovalDecision {
        match input {
            ApprovalInput::Approve => {
                if self.approvals + self.rejects < self.reviewers {
                    self.approvals += 1;
                }
            }
            ApprovalInput::Reject => {
                if self.approvals + self.rejects < self.reviewers {
                    self.rejects += 1;
                }
            }
            ApprovalInput::Reset => {
                self.approvals = 0;
                self.rejects = 0;
            }
        }
        self.decide()
    }
}

/// Admission decision from backpressure controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackpressureDecision {
    /// Queue depth is healthy.
    Accept,
    /// Queue depth is elevated, caller should slow down.
    Throttle,
    /// Queue depth is critical, caller should shed load.
    Shed,
}

/// Input to backpressure controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackpressureInput {
    /// Add queued work.
    Enqueue { count: u16 },
    /// Remove queued work.
    Dequeue { count: u16 },
}

/// Deterministic queue-depth backpressure controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackpressureMachine {
    queue_depth: u16,
    soft_limit: u16,
    hard_limit: u16,
}

impl BackpressureMachine {
    /// Creates a controller with bounded thresholds.
    pub fn new(soft_limit: u16, hard_limit: u16) -> Self {
        let hard = hard_limit.max(1);
        let soft = soft_limit.clamp(1, hard);
        Self {
            queue_depth: 0,
            soft_limit: soft,
            hard_limit: hard,
        }
    }

    /// Current queue depth.
    pub fn queue_depth(self) -> u16 {
        self.queue_depth
    }

    fn classify(self) -> BackpressureDecision {
        if self.queue_depth >= self.hard_limit {
            BackpressureDecision::Shed
        } else if self.queue_depth >= self.soft_limit {
            BackpressureDecision::Throttle
        } else {
            BackpressureDecision::Accept
        }
    }

    /// Applies one queue-depth update.
    pub fn step(&mut self, input: BackpressureInput) -> BackpressureDecision {
        match input {
            BackpressureInput::Enqueue { count } => {
                self.queue_depth = self.queue_depth.saturating_add(count);
            }
            BackpressureInput::Dequeue { count } => {
                self.queue_depth = self.queue_depth.saturating_sub(count);
            }
        }
        self.classify()
    }
}

/// SLA deadline decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlaDecision {
    /// No externally visible decision for this input.
    Noop,
    /// Work remains in budget.
    Pending,
    /// Active work exceeded deadline.
    Expired,
    /// Work completed before expiry.
    CompletedOnTime,
    /// Work completed after expiry.
    CompletedLate,
}

/// Input to SLA deadline controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlaInput {
    /// Start a new SLA window.
    StartWindow,
    /// Advance one logical clock tick.
    Tick,
    /// Mark active work complete.
    Complete,
    /// Clear all progress and return to idle.
    Reset,
}

/// Deterministic SLA deadline controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlaDeadlineMachine {
    deadline_ticks: u16,
    remaining_ticks: u16,
    active: bool,
    expired: bool,
}

impl SlaDeadlineMachine {
    /// Creates a deadline controller.
    pub fn new(deadline_ticks: u16) -> Self {
        Self {
            deadline_ticks: deadline_ticks.max(1),
            remaining_ticks: 0,
            active: false,
            expired: false,
        }
    }

    /// Returns true if a deadline window is active.
    pub fn active(self) -> bool {
        self.active
    }

    /// Returns remaining ticks in current window.
    pub fn remaining_ticks(self) -> u16 {
        self.remaining_ticks
    }

    /// Returns true if active work has expired.
    pub fn expired(self) -> bool {
        self.expired
    }

    /// Applies one SLA input.
    pub fn step(&mut self, input: SlaInput) -> SlaDecision {
        match input {
            SlaInput::StartWindow => {
                self.active = true;
                self.expired = false;
                self.remaining_ticks = self.deadline_ticks;
                SlaDecision::Pending
            }
            SlaInput::Tick => {
                if !self.active {
                    return SlaDecision::Noop;
                }
                if self.remaining_ticks > 0 {
                    self.remaining_ticks -= 1;
                    if self.remaining_ticks == 0 {
                        self.expired = true;
                        SlaDecision::Expired
                    } else {
                        SlaDecision::Pending
                    }
                } else {
                    self.expired = true;
                    SlaDecision::Expired
                }
            }
            SlaInput::Complete => {
                if !self.active {
                    return SlaDecision::Noop;
                }
                self.active = false;
                if self.expired || self.remaining_ticks == 0 {
                    SlaDecision::CompletedLate
                } else {
                    SlaDecision::CompletedOnTime
                }
            }
            SlaInput::Reset => {
                self.active = false;
                self.expired = false;
                self.remaining_ticks = 0;
                SlaDecision::Noop
            }
        }
    }
}

/// DLQ routing decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DlqDecision {
    /// Continue normal execution path.
    Continue,
    /// Route failing workload to dead-letter queue.
    RouteToDlq,
}

/// Input to DLQ routing budget machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DlqInput {
    /// Observe one successful execution.
    Success,
    /// Observe one failed execution.
    Failure,
    /// Reset failure counter.
    Reset,
}

/// Deterministic consecutive-failure DLQ budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DlqBudgetMachine {
    max_consecutive_failures: u8,
    consecutive_failures: u8,
}

impl DlqBudgetMachine {
    /// Creates a DLQ failure budget.
    pub fn new(max_consecutive_failures: u8) -> Self {
        Self {
            max_consecutive_failures: max_consecutive_failures.max(1),
            consecutive_failures: 0,
        }
    }

    /// Returns current consecutive failure count.
    pub fn consecutive_failures(self) -> u8 {
        self.consecutive_failures
    }

    /// Applies one DLQ budget input.
    pub fn step(&mut self, input: DlqInput) -> DlqDecision {
        match input {
            DlqInput::Success | DlqInput::Reset => {
                self.consecutive_failures = 0;
                DlqDecision::Continue
            }
            DlqInput::Failure => {
                if self.consecutive_failures < self.max_consecutive_failures {
                    self.consecutive_failures += 1;
                }
                if self.consecutive_failures >= self.max_consecutive_failures {
                    DlqDecision::RouteToDlq
                } else {
                    DlqDecision::Continue
                }
            }
        }
    }
}

/// Decision emitted by nonce manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NonceDecision {
    /// Reserved a nonce for a new transaction.
    Reserved { nonce: u64 },
    /// Confirmed nonce was observed on-chain.
    Confirmed { nonce: u64 },
    /// Nonce was not in tracked in-flight set.
    Unknown { nonce: u64 },
    /// Reconciled local cursor against chain view.
    Reconciled { next_nonce: u64 },
}

/// Input to nonce manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NonceInput {
    /// Reserve one nonce.
    Reserve,
    /// Confirm one nonce from chain.
    Confirm { nonce: u64 },
    /// Reconcile to observed chain next nonce.
    Reconcile { chain_next_nonce: u64 },
}

/// Deterministic nonce tracker for blockchain transaction submission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NonceManagerMachine {
    next_nonce: u64,
    in_flight: VecDeque<u64>,
    max_in_flight: u16,
}

impl NonceManagerMachine {
    /// Creates a nonce manager.
    pub fn new(start_nonce: u64, max_in_flight: u16) -> Self {
        Self {
            next_nonce: start_nonce,
            in_flight: VecDeque::new(),
            max_in_flight: max_in_flight.max(1),
        }
    }

    /// Next nonce cursor.
    pub fn next_nonce(&self) -> u64 {
        self.next_nonce
    }

    /// Count of tracked in-flight nonces.
    pub fn in_flight_len(&self) -> usize {
        self.in_flight.len()
    }

    /// Applies one nonce manager input.
    pub fn step(&mut self, input: NonceInput) -> NonceDecision {
        match input {
            NonceInput::Reserve => {
                if self.in_flight.len() >= usize::from(self.max_in_flight) {
                    if let Some(existing) = self.in_flight.front().copied() {
                        return NonceDecision::Reserved { nonce: existing };
                    }
                }
                let nonce = self.next_nonce;
                self.next_nonce = self.next_nonce.saturating_add(1);
                self.in_flight.push_back(nonce);
                NonceDecision::Reserved { nonce }
            }
            NonceInput::Confirm { nonce } => {
                if let Some(idx) = self.in_flight.iter().position(|n| *n == nonce) {
                    self.in_flight.remove(idx);
                    NonceDecision::Confirmed { nonce }
                } else {
                    NonceDecision::Unknown { nonce }
                }
            }
            NonceInput::Reconcile { chain_next_nonce } => {
                if chain_next_nonce > self.next_nonce {
                    self.next_nonce = chain_next_nonce;
                }
                while let Some(front) = self.in_flight.front().copied() {
                    if front < chain_next_nonce {
                        self.in_flight.pop_front();
                    } else {
                        break;
                    }
                }
                NonceDecision::Reconciled {
                    next_nonce: self.next_nonce,
                }
            }
        }
    }
}

/// Decision emitted by fee bidding machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeeDecision {
    /// Produced a deterministic fee quote.
    Quote {
        /// Effective max fee per gas.
        max_fee: u64,
        /// Max priority fee per gas.
        max_priority_fee: u64,
        /// Current rejection count used in bumping.
        rejection_count: u8,
    },
    /// No externally-visible quote for this input.
    Noop,
}

/// Input to fee bidding machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeeInput {
    /// Update observed base fee.
    UpdateBaseFee { base_fee: u64 },
    /// Produce quote for next transaction.
    Quote { urgent: bool },
    /// Track that last quote failed to land.
    MarkRejected,
    /// Track that transaction landed.
    MarkConfirmed,
}

/// Deterministic EIP-1559 style fee quote machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeeBiddingMachine {
    base_fee: u64,
    priority_fee: u64,
    bump_bps: u16,
    max_fee_cap: u64,
    rejection_count: u8,
}

impl FeeBiddingMachine {
    /// Creates a fee bidding machine.
    pub fn new(base_fee: u64, priority_fee: u64, bump_bps: u16, max_fee_cap: u64) -> Self {
        Self {
            base_fee: base_fee.max(1),
            priority_fee: priority_fee.max(1),
            bump_bps: bump_bps.max(1),
            max_fee_cap: max_fee_cap.max(1),
            rejection_count: 0,
        }
    }

    /// Current rejection count.
    pub fn rejection_count(self) -> u8 {
        self.rejection_count
    }

    fn compute_quote(self, urgent: bool) -> (u64, u64) {
        let base = self.base_fee.saturating_add(self.priority_fee);
        let rejection_bump = u64::from(self.rejection_count).saturating_mul(u64::from(self.bump_bps));
        let urgency_bump = if urgent { u64::from(self.bump_bps) } else { 0 };
        let multiplier = 10_000_u64
            .saturating_add(rejection_bump)
            .saturating_add(urgency_bump);
        let bumped = base.saturating_mul(multiplier) / 10_000_u64;
        let max_fee = bumped.max(self.priority_fee).min(self.max_fee_cap);
        (max_fee, self.priority_fee)
    }

    /// Applies one fee input.
    pub fn step(&mut self, input: FeeInput) -> FeeDecision {
        match input {
            FeeInput::UpdateBaseFee { base_fee } => {
                self.base_fee = base_fee.max(1);
                FeeDecision::Noop
            }
            FeeInput::MarkRejected => {
                self.rejection_count = self.rejection_count.saturating_add(1);
                FeeDecision::Noop
            }
            FeeInput::MarkConfirmed => {
                self.rejection_count = 0;
                FeeDecision::Noop
            }
            FeeInput::Quote { urgent } => {
                let (max_fee, max_priority_fee) = self.compute_quote(urgent);
                FeeDecision::Quote {
                    max_fee,
                    max_priority_fee,
                    rejection_count: self.rejection_count,
                }
            }
        }
    }
}

/// Finality status decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinalityDecision {
    /// Waiting for more confirmations.
    Pending {
        /// Remaining confirmation depth required.
        remaining_depth: u16,
    },
    /// Required confirmation depth reached.
    Finalized,
    /// Reorg risk detected.
    ReorgDetected,
}

/// Finality guard input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinalityInput {
    /// Observe latest confirmation depth.
    ObserveDepth { depth: u16 },
    /// Explicitly mark reorg detection.
    MarkReorg,
    /// Reset state for next transaction.
    Reset,
}

/// Finality guard phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinalityPhase {
    /// Waiting for sufficient confirmations.
    Pending,
    /// Confirmation depth reached.
    Finalized,
    /// Reorg signal observed.
    ReorgDetected,
}

/// Deterministic finality/reorg guard machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalityGuardMachine {
    required_depth: u16,
    observed_depth: u16,
    phase: FinalityPhase,
}

impl FinalityGuardMachine {
    /// Creates a finality guard.
    pub fn new(required_depth: u16) -> Self {
        Self {
            required_depth: required_depth.max(1),
            observed_depth: 0,
            phase: FinalityPhase::Pending,
        }
    }

    /// Current observed depth.
    pub fn observed_depth(self) -> u16 {
        self.observed_depth
    }

    /// True when finalized.
    pub fn is_finalized(self) -> bool {
        self.phase == FinalityPhase::Finalized
    }

    /// True when reorg is detected.
    pub fn reorg_detected(self) -> bool {
        self.phase == FinalityPhase::ReorgDetected
    }

    /// Applies one finality input.
    pub fn step(&mut self, input: FinalityInput) -> FinalityDecision {
        match input {
            FinalityInput::ObserveDepth { depth } => {
                if self.phase == FinalityPhase::ReorgDetected {
                    return FinalityDecision::ReorgDetected;
                }
                self.observed_depth = self.observed_depth.max(depth);
                if self.observed_depth >= self.required_depth {
                    self.phase = FinalityPhase::Finalized;
                    FinalityDecision::Finalized
                } else {
                    self.phase = FinalityPhase::Pending;
                    FinalityDecision::Pending {
                        remaining_depth: self.required_depth - self.observed_depth,
                    }
                }
            }
            FinalityInput::MarkReorg => {
                self.phase = FinalityPhase::ReorgDetected;
                FinalityDecision::ReorgDetected
            }
            FinalityInput::Reset => {
                self.phase = FinalityPhase::Pending;
                self.observed_depth = 0;
                FinalityDecision::Pending {
                    remaining_depth: self.required_depth,
                }
            }
        }
    }
}

/// Allowlist policy decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AllowlistDecision {
    /// Request is allowed.
    Allow,
    /// Request denied because policy is paused.
    DenyPaused,
    /// Request denied because tuple is not in allowlist.
    DenyNotAllowed,
}

/// Allowlist policy input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AllowlistInput {
    /// Evaluate one (chain, contract, method) tuple.
    Evaluate {
        /// EVM chain id.
        chain_id: u32,
        /// Application contract tag/hash.
        contract_tag: u64,
        /// Method selector tag/hash.
        method_tag: u32,
    },
    /// Pause policy.
    Pause,
    /// Resume policy.
    Resume,
}

/// Deterministic allowlist guard machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllowlistPolicyMachine {
    allowed_chain_id: u32,
    allowed_contract_tag: u64,
    allowed_method_tag: u32,
    paused: bool,
}

impl AllowlistPolicyMachine {
    /// Creates allowlist guard.
    pub fn new(allowed_chain_id: u32, allowed_contract_tag: u64, allowed_method_tag: u32) -> Self {
        Self {
            allowed_chain_id,
            allowed_contract_tag,
            allowed_method_tag,
            paused: false,
        }
    }

    /// Returns true when paused.
    pub fn paused(self) -> bool {
        self.paused
    }

    /// Applies one allowlist input.
    pub fn step(&mut self, input: AllowlistInput) -> AllowlistDecision {
        match input {
            AllowlistInput::Pause => {
                self.paused = true;
                AllowlistDecision::DenyPaused
            }
            AllowlistInput::Resume => {
                self.paused = false;
                AllowlistDecision::Allow
            }
            AllowlistInput::Evaluate {
                chain_id,
                contract_tag,
                method_tag,
            } => {
                if self.paused {
                    return AllowlistDecision::DenyPaused;
                }
                if chain_id == self.allowed_chain_id
                    && contract_tag == self.allowed_contract_tag
                    && method_tag == self.allowed_method_tag
                {
                    AllowlistDecision::Allow
                } else {
                    AllowlistDecision::DenyNotAllowed
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_suppresses_duplicates_in_window() {
        let mut m = DedupMachine::new(2);
        assert_eq!(
            m.step(DedupInput::Observe { fingerprint: 42 }),
            Some(DedupDecision::Emit)
        );
        assert_eq!(
            m.step(DedupInput::Observe { fingerprint: 42 }),
            Some(DedupDecision::DropDuplicate)
        );
        let _ = m.step(DedupInput::Tick);
        let _ = m.step(DedupInput::Tick);
        assert_eq!(
            m.step(DedupInput::Observe { fingerprint: 42 }),
            Some(DedupDecision::Emit)
        );
    }

    #[test]
    fn rate_limiter_refill_is_deterministic() {
        let mut m = RateLimiterMachine::new(3, 1);
        assert_eq!(
            m.step(RateLimitInput::Request { cost: 2 }),
            Some(RateLimitDecision::Allow)
        );
        assert_eq!(m.tokens(), 1);
        assert_eq!(
            m.step(RateLimitInput::Request { cost: 2 }),
            Some(RateLimitDecision::Deny)
        );
        let _ = m.step(RateLimitInput::Tick);
        assert_eq!(m.tokens(), 2);
        assert_eq!(
            m.step(RateLimitInput::Request { cost: 2 }),
            Some(RateLimitDecision::Allow)
        );
    }

    #[test]
    fn circuit_breaker_opens_and_recovers() {
        let mut m = CircuitBreakerMachine::new(2, 2);
        assert_eq!(m.step(BreakerInput::Request), BreakerDecision::Allow);
        let _ = m.step(BreakerInput::Failure);
        let _ = m.step(BreakerInput::Failure);
        assert_eq!(m.phase(), BreakerPhase::Open);
        assert_eq!(m.step(BreakerInput::Request), BreakerDecision::DenyOpen);
        let _ = m.step(BreakerInput::Tick);
        let _ = m.step(BreakerInput::Tick);
        assert_eq!(m.phase(), BreakerPhase::HalfOpen);
        assert_eq!(m.step(BreakerInput::Request), BreakerDecision::Allow);
        let _ = m.step(BreakerInput::Success);
        assert_eq!(m.phase(), BreakerPhase::Closed);
    }

    #[test]
    fn retry_budget_enforces_cap() {
        let mut m = RetryBudgetMachine::new(2);
        assert_eq!(m.step(RetryInput::ConsumeRetry), RetryDecision::Retry);
        assert_eq!(m.step(RetryInput::ConsumeRetry), RetryDecision::Retry);
        assert_eq!(m.step(RetryInput::ConsumeRetry), RetryDecision::Exhausted);
        assert_eq!(m.remaining(), 0);
        let _ = m.step(RetryInput::ResetCycle);
        assert_eq!(m.remaining(), 2);
    }

    #[test]
    fn approval_gate_reaches_terminal_states() {
        let mut m = ApprovalGateMachine::new(2, 3);
        assert_eq!(m.step(ApprovalInput::Approve), ApprovalDecision::Pending);
        assert_eq!(m.step(ApprovalInput::Approve), ApprovalDecision::Approved);

        let mut m2 = ApprovalGateMachine::new(3, 3);
        assert_eq!(m2.step(ApprovalInput::Reject), ApprovalDecision::Rejected);
        assert_eq!(m2.step(ApprovalInput::Reject), ApprovalDecision::Rejected);
    }

    #[test]
    fn backpressure_machine_classifies_queue_levels() {
        let mut m = BackpressureMachine::new(3, 5);
        assert_eq!(
            m.step(BackpressureInput::Enqueue { count: 2 }),
            BackpressureDecision::Accept
        );
        assert_eq!(
            m.step(BackpressureInput::Enqueue { count: 2 }),
            BackpressureDecision::Throttle
        );
        assert_eq!(
            m.step(BackpressureInput::Enqueue { count: 2 }),
            BackpressureDecision::Shed
        );
        assert_eq!(
            m.step(BackpressureInput::Dequeue { count: 4 }),
            BackpressureDecision::Accept
        );
    }

    #[test]
    fn sla_deadline_expires_and_completes_late() {
        let mut m = SlaDeadlineMachine::new(2);
        assert_eq!(m.step(SlaInput::StartWindow), SlaDecision::Pending);
        assert_eq!(m.step(SlaInput::Tick), SlaDecision::Pending);
        assert_eq!(m.step(SlaInput::Tick), SlaDecision::Expired);
        assert_eq!(m.step(SlaInput::Complete), SlaDecision::CompletedLate);
        assert!(!m.active());
    }

    #[test]
    fn dlq_budget_routes_after_failures() {
        let mut m = DlqBudgetMachine::new(2);
        assert_eq!(m.step(DlqInput::Failure), DlqDecision::Continue);
        assert_eq!(m.step(DlqInput::Failure), DlqDecision::RouteToDlq);
        assert_eq!(m.consecutive_failures(), 2);
        assert_eq!(m.step(DlqInput::Success), DlqDecision::Continue);
        assert_eq!(m.consecutive_failures(), 0);
    }

    #[test]
    fn nonce_manager_reserves_confirms_and_reconciles() {
        let mut m = NonceManagerMachine::new(10, 4);
        assert_eq!(m.step(NonceInput::Reserve), NonceDecision::Reserved { nonce: 10 });
        assert_eq!(m.step(NonceInput::Reserve), NonceDecision::Reserved { nonce: 11 });
        assert_eq!(m.next_nonce(), 12);
        assert_eq!(m.in_flight_len(), 2);

        assert_eq!(
            m.step(NonceInput::Confirm { nonce: 10 }),
            NonceDecision::Confirmed { nonce: 10 }
        );
        assert_eq!(m.in_flight_len(), 1);
        assert_eq!(
            m.step(NonceInput::Reconcile {
                chain_next_nonce: 15
            }),
            NonceDecision::Reconciled { next_nonce: 15 }
        );
        assert_eq!(m.next_nonce(), 15);
    }

    #[test]
    fn fee_bidding_bumps_after_rejections() {
        let mut m = FeeBiddingMachine::new(100, 10, 500, 5000);
        let first = m.step(FeeInput::Quote { urgent: false });
        let base_quote = match first {
            FeeDecision::Quote { max_fee, .. } => max_fee,
            FeeDecision::Noop => panic!("expected quote"),
        };
        let _ = m.step(FeeInput::MarkRejected);
        let second = m.step(FeeInput::Quote { urgent: false });
        let bumped_quote = match second {
            FeeDecision::Quote { max_fee, .. } => max_fee,
            FeeDecision::Noop => panic!("expected quote"),
        };
        assert!(bumped_quote >= base_quote);
        let _ = m.step(FeeInput::MarkConfirmed);
        assert_eq!(m.rejection_count(), 0);
    }

    #[test]
    fn finality_guard_finalizes_and_reorgs() {
        let mut m = FinalityGuardMachine::new(3);
        assert_eq!(
            m.step(FinalityInput::ObserveDepth { depth: 1 }),
            FinalityDecision::Pending { remaining_depth: 2 }
        );
        assert_eq!(
            m.step(FinalityInput::ObserveDepth { depth: 3 }),
            FinalityDecision::Finalized
        );
        assert!(m.is_finalized());
        assert_eq!(m.step(FinalityInput::MarkReorg), FinalityDecision::ReorgDetected);
        assert!(m.reorg_detected());
    }

    #[test]
    fn allowlist_policy_honors_pause_and_tuple_match() {
        let mut m = AllowlistPolicyMachine::new(1, 55, 0xdeadbeef);
        assert_eq!(
            m.step(AllowlistInput::Evaluate {
                chain_id: 1,
                contract_tag: 55,
                method_tag: 0xdeadbeef
            }),
            AllowlistDecision::Allow
        );
        assert_eq!(m.step(AllowlistInput::Pause), AllowlistDecision::DenyPaused);
        assert_eq!(
            m.step(AllowlistInput::Evaluate {
                chain_id: 1,
                contract_tag: 55,
                method_tag: 0xdeadbeef
            }),
            AllowlistDecision::DenyPaused
        );
        assert_eq!(m.step(AllowlistInput::Resume), AllowlistDecision::Allow);
        assert_eq!(
            m.step(AllowlistInput::Evaluate {
                chain_id: 1,
                contract_tag: 99,
                method_tag: 0xdeadbeef
            }),
            AllowlistDecision::DenyNotAllowed
        );
    }
}
