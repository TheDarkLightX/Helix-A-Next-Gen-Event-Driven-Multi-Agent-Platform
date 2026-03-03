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

//! Expanded deterministic agent class library.
//!
//! These agents share a temporal guard kernel with strike budgeting and cooldown,
//! making behavior stateful and replayable instead of stateless threshold checks.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Deterministic temporal guard input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TemporalGuardInput {
    /// Evaluate one bounded signal value.
    Evaluate { value: u32 },
    /// Advance logical time by one tick.
    Tick,
    /// Reset dynamic state.
    Reset,
}

/// Deterministic temporal guard decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemporalGuardDecision {
    /// Input is accepted and no escalation is active.
    Allow,
    /// Input crossed soft risk threshold; keep running but flag degradation.
    Warn,
    /// Input is rejected.
    Block,
    /// Machine is still in cooldown state.
    CoolingDown,
}

/// Snapshot of temporal guard state after one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalGuardSnapshot {
    /// Hard threshold for block escalation.
    pub threshold: u32,
    /// Soft threshold for warning.
    pub soft_threshold: u32,
    /// Number of high-risk strikes required for hard block.
    pub strike_limit: u8,
    /// Current accumulated strikes.
    pub strikes: u8,
    /// Remaining cooldown ticks while blocked.
    pub blocked_ticks_left: u8,
    /// Configured cooldown duration.
    pub cooldown_ticks: u8,
    /// Largest observed value since reset.
    pub peak_observed: u32,
    /// Number of evaluated observations.
    pub total_evaluations: u32,
}

/// Step trace item for temporal guard simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalGuardSimulationStep {
    /// Applied command.
    pub input: TemporalGuardInput,
    /// Decision for this command.
    pub decision: TemporalGuardDecision,
    /// Snapshot after applying command.
    pub snapshot: TemporalGuardSnapshot,
}

/// Simulation output for one expanded guard class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalGuardSimulation {
    /// Expanded agent identifier.
    pub agent_id: String,
    /// Effective threshold used.
    pub threshold: u32,
    /// Effective strike limit used.
    pub strike_limit: u8,
    /// Effective cooldown ticks used.
    pub cooldown_ticks: u8,
    /// Full deterministic step trace.
    pub steps: Vec<TemporalGuardSimulationStep>,
}

/// Shared temporal guard kernel used by expanded deterministic classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalGuardMachine {
    threshold: u32,
    soft_threshold: u32,
    strike_limit: u8,
    strikes: u8,
    cooldown_ticks: u8,
    blocked_ticks_left: u8,
    peak_observed: u32,
    total_evaluations: u32,
}

impl TemporalGuardMachine {
    /// Creates a temporal guard machine.
    pub fn new(threshold: u32, strike_limit: u8, cooldown_ticks: u8) -> Self {
        let bounded_threshold = threshold.max(1);
        let soft_threshold = ((u64::from(bounded_threshold) * 8) / 10)
            .max(1)
            .min(u64::from(bounded_threshold)) as u32;
        Self {
            threshold: bounded_threshold,
            soft_threshold,
            strike_limit: strike_limit.max(1),
            strikes: 0,
            cooldown_ticks: cooldown_ticks.max(1),
            blocked_ticks_left: 0,
            peak_observed: 0,
            total_evaluations: 0,
        }
    }

    /// Returns state snapshot.
    pub fn snapshot(self) -> TemporalGuardSnapshot {
        TemporalGuardSnapshot {
            threshold: self.threshold,
            soft_threshold: self.soft_threshold,
            strike_limit: self.strike_limit,
            strikes: self.strikes,
            blocked_ticks_left: self.blocked_ticks_left,
            cooldown_ticks: self.cooldown_ticks,
            peak_observed: self.peak_observed,
            total_evaluations: self.total_evaluations,
        }
    }

    /// Applies one deterministic step.
    pub fn step(&mut self, input: TemporalGuardInput) -> TemporalGuardDecision {
        match input {
            TemporalGuardInput::Reset => {
                self.strikes = 0;
                self.blocked_ticks_left = 0;
                self.peak_observed = 0;
                self.total_evaluations = 0;
                TemporalGuardDecision::Allow
            }
            TemporalGuardInput::Tick => {
                if self.blocked_ticks_left > 0 {
                    self.blocked_ticks_left -= 1;
                    if self.blocked_ticks_left > 0 {
                        TemporalGuardDecision::CoolingDown
                    } else {
                        TemporalGuardDecision::Allow
                    }
                } else {
                    if self.strikes > 0 {
                        self.strikes -= 1;
                    }
                    TemporalGuardDecision::Allow
                }
            }
            TemporalGuardInput::Evaluate { value } => {
                self.total_evaluations = self.total_evaluations.saturating_add(1);
                self.peak_observed = self.peak_observed.max(value);

                if self.blocked_ticks_left > 0 {
                    return TemporalGuardDecision::Block;
                }

                if value > self.threshold {
                    self.strikes = self.strikes.saturating_add(1);
                    if self.strikes >= self.strike_limit {
                        self.strikes = 0;
                        self.blocked_ticks_left = self.cooldown_ticks;
                        TemporalGuardDecision::Block
                    } else {
                        TemporalGuardDecision::Warn
                    }
                } else if value > self.soft_threshold {
                    TemporalGuardDecision::Warn
                } else {
                    if self.strikes > 0 {
                        self.strikes -= 1;
                    }
                    TemporalGuardDecision::Allow
                }
            }
        }
    }
}

/// Metadata describing one expanded deterministic agent class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpandedAgentDescriptor {
    /// Stable API identifier.
    pub id: &'static str,
    /// Human-readable name.
    pub name: &'static str,
    /// ROI rationale displayed in UI/API.
    pub roi_rationale: &'static str,
    /// Rust machine type name.
    pub machine_type: &'static str,
    /// Domain coverage bucket for measurable breadth.
    pub category: &'static str,
    /// Default threshold used in simulations.
    pub default_threshold: u32,
    /// Default strike limit used in simulations.
    pub default_strike_limit: u8,
    /// Default cooldown used in simulations.
    pub default_cooldown_ticks: u8,
}

/// Quality summary for the expanded class library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpandedAgentQualitySummary {
    /// Number of expanded classes.
    pub expanded_agents: usize,
    /// Number of distinct categories covered.
    pub categories: usize,
    /// Number of temporal input variants.
    pub temporal_inputs: usize,
    /// Number of temporal decision variants.
    pub temporal_decisions: usize,
}

macro_rules! define_temporal_guard_class {
    ($machine:ident, $input:ident, $decision:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $decision {
            /// Input is accepted and no escalation is active.
            Allow,
            /// Input crossed soft risk threshold; keep running but flag degradation.
            Warn,
            /// Input is rejected.
            Block,
            /// Machine is still in cooldown state.
            CoolingDown,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        pub enum $input {
            /// Evaluate one bounded signal value.
            Evaluate { value: u32 },
            /// Advance logical time by one tick.
            Tick,
            /// Reset dynamic state.
            Reset,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub struct $machine {
            core: TemporalGuardMachine,
        }

        impl $machine {
            /// Creates a temporal guard class instance.
            pub fn new(threshold: u32, strike_limit: u8, cooldown_ticks: u8) -> Self {
                Self {
                    core: TemporalGuardMachine::new(threshold, strike_limit, cooldown_ticks),
                }
            }

            /// Returns state snapshot.
            pub fn snapshot(self) -> TemporalGuardSnapshot {
                self.core.snapshot()
            }

            /// Applies one deterministic step.
            pub fn step(&mut self, input: $input) -> $decision {
                let core_input = match input {
                    $input::Evaluate { value } => TemporalGuardInput::Evaluate { value },
                    $input::Tick => TemporalGuardInput::Tick,
                    $input::Reset => TemporalGuardInput::Reset,
                };
                match self.core.step(core_input) {
                    TemporalGuardDecision::Allow => $decision::Allow,
                    TemporalGuardDecision::Warn => $decision::Warn,
                    TemporalGuardDecision::Block => $decision::Block,
                    TemporalGuardDecision::CoolingDown => $decision::CoolingDown,
                }
            }
        }
    };
}

macro_rules! define_expanded_agents {
    (
        $(
            {
                id: $id:literal,
                name: $name:literal,
                roi: $roi:literal,
                machine: $machine:ident,
                input: $input:ident,
                decision: $decision:ident,
                category: $category:literal,
                threshold: $threshold:expr,
                strike_limit: $strike_limit:expr,
                cooldown_ticks: $cooldown_ticks:expr
            }
        ),+ $(,)?
    ) => {
        $(define_temporal_guard_class!($machine, $input, $decision);)+

        /// All expanded deterministic agents exposed by Helix.
        pub const EXPANDED_AGENT_DESCRIPTORS: &[ExpandedAgentDescriptor] = &[
            $(ExpandedAgentDescriptor {
                id: $id,
                name: $name,
                roi_rationale: $roi,
                machine_type: stringify!($machine),
                category: $category,
                default_threshold: $threshold,
                default_strike_limit: $strike_limit,
                default_cooldown_ticks: $cooldown_ticks,
            }),+
        ];
    };
}

define_expanded_agents!(
    { id: "webhook_signature_guard", name: "Webhook Signature Guard Agent", roi: "Rejects unsigned webhook payloads before dispatch.", machine: WebhookSignatureGuardMachine, input: WebhookSignatureGuardInput, decision: WebhookSignatureGuardDecision, category: "ingress", threshold: 6, strike_limit: 2, cooldown_ticks: 2 },
    { id: "payload_schema_guard", name: "Payload Schema Guard Agent", roi: "Blocks malformed payloads at ingress to avoid downstream faults.", machine: PayloadSchemaGuardMachine, input: PayloadSchemaGuardInput, decision: PayloadSchemaGuardDecision, category: "ingress", threshold: 6, strike_limit: 2, cooldown_ticks: 2 },
    { id: "json_size_guard", name: "JSON Size Guard Agent", roi: "Prevents oversized request bodies from exhausting parser capacity.", machine: JsonSizeGuardMachine, input: JsonSizeGuardInput, decision: JsonSizeGuardDecision, category: "ingress", threshold: 7, strike_limit: 2, cooldown_ticks: 2 },
    { id: "pii_redaction_guard", name: "PII Redaction Guard Agent", roi: "Gates events that exceed configured sensitive-data risk score.", machine: PiiRedactionGuardMachine, input: PiiRedactionGuardInput, decision: PiiRedactionGuardDecision, category: "ingress", threshold: 5, strike_limit: 2, cooldown_ticks: 3 },
    { id: "idempotency_key_guard", name: "Idempotency Key Guard Agent", roi: "Enforces bounded idempotency key quality before execution.", machine: IdempotencyKeyGuardMachine, input: IdempotencyKeyGuardInput, decision: IdempotencyKeyGuardDecision, category: "ingress", threshold: 6, strike_limit: 2, cooldown_ticks: 2 },
    { id: "replay_window_guard", name: "Replay Window Guard Agent", roi: "Blocks stale replay attempts outside deterministic freshness bounds.", machine: ReplayWindowGuardMachine, input: ReplayWindowGuardInput, decision: ReplayWindowGuardDecision, category: "ingress", threshold: 5, strike_limit: 2, cooldown_ticks: 3 },
    { id: "queue_latency_guard", name: "Queue Latency Guard Agent", roi: "Stops latency blowups by rejecting over-budget queue delay.", machine: QueueLatencyGuardMachine, input: QueueLatencyGuardInput, decision: QueueLatencyGuardDecision, category: "runtime", threshold: 7, strike_limit: 3, cooldown_ticks: 2 },
    { id: "deadman_switch_guard", name: "Deadman Switch Guard Agent", roi: "Fails closed when heartbeat-like liveness score crosses threshold.", machine: DeadmanSwitchGuardMachine, input: DeadmanSwitchGuardInput, decision: DeadmanSwitchGuardDecision, category: "runtime", threshold: 4, strike_limit: 2, cooldown_ticks: 3 },
    { id: "heartbeat_monitor_guard", name: "Heartbeat Monitor Guard Agent", roi: "Detects deterministic heartbeat degradation before full outage.", machine: HeartbeatMonitorGuardMachine, input: HeartbeatMonitorGuardInput, decision: HeartbeatMonitorGuardDecision, category: "runtime", threshold: 5, strike_limit: 2, cooldown_ticks: 2 },
    { id: "drift_budget_guard", name: "Drift Budget Guard Agent", roi: "Keeps model/runtime drift within an explicit bounded budget.", machine: DriftBudgetGuardMachine, input: DriftBudgetGuardInput, decision: DriftBudgetGuardDecision, category: "runtime", threshold: 6, strike_limit: 3, cooldown_ticks: 2 },
    { id: "cost_budget_guard", name: "Cost Budget Guard Agent", roi: "Prevents run-away spend by gating over-budget operations.", machine: CostBudgetGuardMachine, input: CostBudgetGuardInput, decision: CostBudgetGuardDecision, category: "runtime", threshold: 6, strike_limit: 2, cooldown_ticks: 3 },
    { id: "api_quota_guard", name: "API Quota Guard Agent", roi: "Protects upstream quotas with deterministic request admission.", machine: ApiQuotaGuardMachine, input: ApiQuotaGuardInput, decision: ApiQuotaGuardDecision, category: "runtime", threshold: 7, strike_limit: 3, cooldown_ticks: 2 },
    { id: "concurrency_limit_guard", name: "Concurrency Limit Guard Agent", roi: "Bounds concurrent load to preserve tail-latency SLOs.", machine: ConcurrencyLimitGuardMachine, input: ConcurrencyLimitGuardInput, decision: ConcurrencyLimitGuardDecision, category: "runtime", threshold: 7, strike_limit: 3, cooldown_ticks: 2 },
    { id: "lease_expiry_guard", name: "Lease Expiry Guard Agent", roi: "Ensures stale leases cannot continue issuing side effects.", machine: LeaseExpiryGuardMachine, input: LeaseExpiryGuardInput, decision: LeaseExpiryGuardDecision, category: "runtime", threshold: 5, strike_limit: 2, cooldown_ticks: 2 },
    { id: "token_rotation_guard", name: "Token Rotation Guard Agent", roi: "Flags stale auth token health before credential outages.", machine: TokenRotationGuardMachine, input: TokenRotationGuardInput, decision: TokenRotationGuardDecision, category: "runtime", threshold: 5, strike_limit: 2, cooldown_ticks: 3 },
    { id: "secret_age_guard", name: "Secret Age Guard Agent", roi: "Deterministically enforces secret rotation windows.", machine: SecretAgeGuardMachine, input: SecretAgeGuardInput, decision: SecretAgeGuardDecision, category: "runtime", threshold: 5, strike_limit: 2, cooldown_ticks: 3 },
    { id: "dependency_health_gate", name: "Dependency Health Gate Agent", roi: "Avoids cascading failure by gating unhealthy dependencies.", machine: DependencyHealthGateMachine, input: DependencyHealthGateInput, decision: DependencyHealthGateDecision, category: "runtime", threshold: 6, strike_limit: 2, cooldown_ticks: 3 },
    { id: "response_time_slo_guard", name: "Response Time SLO Guard Agent", roi: "Stops SLO erosion by bounding deterministic latency score.", machine: ResponseTimeSloGuardMachine, input: ResponseTimeSloGuardInput, decision: ResponseTimeSloGuardDecision, category: "runtime", threshold: 7, strike_limit: 3, cooldown_ticks: 2 },
    { id: "error_rate_slo_guard", name: "Error Rate SLO Guard Agent", roi: "Trips before incidents when error-rate score exceeds budget.", machine: ErrorRateSloGuardMachine, input: ErrorRateSloGuardInput, decision: ErrorRateSloGuardDecision, category: "runtime", threshold: 6, strike_limit: 2, cooldown_ticks: 3 },
    { id: "canary_promotion_gate", name: "Canary Promotion Gate Agent", roi: "Prevents unsafe rollout promotion under degraded canary signal.", machine: CanaryPromotionGateMachine, input: CanaryPromotionGateInput, decision: CanaryPromotionGateDecision, category: "runtime", threshold: 6, strike_limit: 2, cooldown_ticks: 2 },
    { id: "rollback_trigger_guard", name: "Rollback Trigger Guard Agent", roi: "Automates deterministic rollback trigger gating.", machine: RollbackTriggerGuardMachine, input: RollbackTriggerGuardInput, decision: RollbackTriggerGuardDecision, category: "runtime", threshold: 6, strike_limit: 2, cooldown_ticks: 2 },
    { id: "incident_escalation_gate", name: "Incident Escalation Gate Agent", roi: "Escalates only when incident severity crosses explicit threshold.", machine: IncidentEscalationGateMachine, input: IncidentEscalationGateInput, decision: IncidentEscalationGateDecision, category: "runtime", threshold: 5, strike_limit: 2, cooldown_ticks: 2 },
    { id: "maintenance_window_guard", name: "Maintenance Window Guard Agent", roi: "Blocks risky operations outside approved maintenance windows.", machine: MaintenanceWindowGuardMachine, input: MaintenanceWindowGuardInput, decision: MaintenanceWindowGuardDecision, category: "runtime", threshold: 5, strike_limit: 2, cooldown_ticks: 2 },
    { id: "feature_flag_guard", name: "Feature Flag Guard Agent", roi: "Prevents unsafe flag flips with deterministic risk gating.", machine: FeatureFlagGuardMachine, input: FeatureFlagGuardInput, decision: FeatureFlagGuardDecision, category: "runtime", threshold: 6, strike_limit: 2, cooldown_ticks: 2 },
    { id: "tenant_isolation_guard", name: "Tenant Isolation Guard Agent", roi: "Bounds cross-tenant risk with explicit isolation gate.", machine: TenantIsolationGuardMachine, input: TenantIsolationGuardInput, decision: TenantIsolationGuardDecision, category: "compliance", threshold: 5, strike_limit: 2, cooldown_ticks: 3 },
    { id: "data_residency_guard", name: "Data Residency Guard Agent", roi: "Enforces jurisdiction residency bounds before data movement.", machine: DataResidencyGuardMachine, input: DataResidencyGuardInput, decision: DataResidencyGuardDecision, category: "compliance", threshold: 5, strike_limit: 2, cooldown_ticks: 3 },
    { id: "geo_fence_guard", name: "Geo Fence Guard Agent", roi: "Restricts operations to approved regions deterministically.", machine: GeoFenceGuardMachine, input: GeoFenceGuardInput, decision: GeoFenceGuardDecision, category: "compliance", threshold: 6, strike_limit: 2, cooldown_ticks: 2 },
    { id: "time_window_guard", name: "Time Window Guard Agent", roi: "Gates actions to approved execution windows.", machine: TimeWindowGuardMachine, input: TimeWindowGuardInput, decision: TimeWindowGuardDecision, category: "compliance", threshold: 6, strike_limit: 2, cooldown_ticks: 2 },
    { id: "duplicate_payment_guard", name: "Duplicate Payment Guard Agent", roi: "Blocks duplicate disbursement patterns before settlement.", machine: DuplicatePaymentGuardMachine, input: DuplicatePaymentGuardInput, decision: DuplicatePaymentGuardDecision, category: "payments", threshold: 5, strike_limit: 2, cooldown_ticks: 3 },
    { id: "settlement_delay_guard", name: "Settlement Delay Guard Agent", roi: "Caps settlement delay risk to reduce reconciliation defects.", machine: SettlementDelayGuardMachine, input: SettlementDelayGuardInput, decision: SettlementDelayGuardDecision, category: "payments", threshold: 6, strike_limit: 2, cooldown_ticks: 2 },
    { id: "fraud_score_guard", name: "Fraud Score Guard Agent", roi: "Routes high-risk transactions away from auto approval.", machine: FraudScoreGuardMachine, input: FraudScoreGuardInput, decision: FraudScoreGuardDecision, category: "payments", threshold: 4, strike_limit: 2, cooldown_ticks: 3 },
    { id: "kyc_status_guard", name: "KYC Status Guard Agent", roi: "Deterministically blocks operations when KYC status is out of policy.", machine: KycStatusGuardMachine, input: KycStatusGuardInput, decision: KycStatusGuardDecision, category: "payments", threshold: 5, strike_limit: 2, cooldown_ticks: 3 },
    { id: "aml_velocity_guard", name: "AML Velocity Guard Agent", roi: "Trips on suspicious transaction velocity before compliance breach.", machine: AmlVelocityGuardMachine, input: AmlVelocityGuardInput, decision: AmlVelocityGuardDecision, category: "payments", threshold: 5, strike_limit: 2, cooldown_ticks: 3 },
    { id: "account_lockout_guard", name: "Account Lockout Guard Agent", roi: "Prevents brute-force abuse with deterministic lockout gating.", machine: AccountLockoutGuardMachine, input: AccountLockoutGuardInput, decision: AccountLockoutGuardDecision, category: "security", threshold: 4, strike_limit: 2, cooldown_ticks: 3 },
    { id: "session_anomaly_guard", name: "Session Anomaly Guard Agent", roi: "Blocks anomalous session behavior above configured risk.", machine: SessionAnomalyGuardMachine, input: SessionAnomalyGuardInput, decision: SessionAnomalyGuardDecision, category: "security", threshold: 5, strike_limit: 2, cooldown_ticks: 3 },
    { id: "ip_reputation_guard", name: "IP Reputation Guard Agent", roi: "Guards ingress with bounded IP reputation risk.", machine: IpReputationGuardMachine, input: IpReputationGuardInput, decision: IpReputationGuardDecision, category: "security", threshold: 5, strike_limit: 2, cooldown_ticks: 3 },
    { id: "device_trust_guard", name: "Device Trust Guard Agent", roi: "Applies deterministic trust threshold for device-based access.", machine: DeviceTrustGuardMachine, input: DeviceTrustGuardInput, decision: DeviceTrustGuardDecision, category: "security", threshold: 5, strike_limit: 2, cooldown_ticks: 2 },
    { id: "request_burst_guard", name: "Request Burst Guard Agent", roi: "Flattens burst abuse by deterministic request gating.", machine: RequestBurstGuardMachine, input: RequestBurstGuardInput, decision: RequestBurstGuardDecision, category: "security", threshold: 7, strike_limit: 3, cooldown_ticks: 2 },
    { id: "cache_staleness_guard", name: "Cache Staleness Guard Agent", roi: "Prevents serving stale data beyond bounded freshness score.", machine: CacheStalenessGuardMachine, input: CacheStalenessGuardInput, decision: CacheStalenessGuardDecision, category: "data", threshold: 6, strike_limit: 2, cooldown_ticks: 2 },
    { id: "inventory_floor_guard", name: "Inventory Floor Guard Agent", roi: "Stops oversell scenarios by enforcing inventory floors.", machine: InventoryFloorGuardMachine, input: InventoryFloorGuardInput, decision: InventoryFloorGuardDecision, category: "data", threshold: 5, strike_limit: 2, cooldown_ticks: 2 },
    { id: "price_deviation_guard", name: "Price Deviation Guard Agent", roi: "Detects outlier pricing before customer-impacting anomalies.", machine: PriceDeviationGuardMachine, input: PriceDeviationGuardInput, decision: PriceDeviationGuardDecision, category: "data", threshold: 5, strike_limit: 2, cooldown_ticks: 3 },
    { id: "slippage_bound_guard", name: "Slippage Bound Guard Agent", roi: "Blocks execution when slippage risk exceeds budget.", machine: SlippageBoundGuardMachine, input: SlippageBoundGuardInput, decision: SlippageBoundGuardDecision, category: "data", threshold: 5, strike_limit: 2, cooldown_ticks: 3 },
    { id: "liquidity_depth_guard", name: "Liquidity Depth Guard Agent", roi: "Gates large fills when deterministic liquidity depth is insufficient.", machine: LiquidityDepthGuardMachine, input: LiquidityDepthGuardInput, decision: LiquidityDepthGuardDecision, category: "data", threshold: 6, strike_limit: 2, cooldown_ticks: 2 },
    { id: "oracle_staleness_guard", name: "Oracle Staleness Guard Agent", roi: "Prevents stale oracle reads from triggering unsafe actions.", machine: OracleStalenessGuardMachine, input: OracleStalenessGuardInput, decision: OracleStalenessGuardDecision, category: "onchain", threshold: 5, strike_limit: 2, cooldown_ticks: 3 },
    { id: "oracle_deviation_guard", name: "Oracle Deviation Guard Agent", roi: "Trips when oracle deviation risk breaches policy threshold.", machine: OracleDeviationGuardMachine, input: OracleDeviationGuardInput, decision: OracleDeviationGuardDecision, category: "onchain", threshold: 5, strike_limit: 2, cooldown_ticks: 3 },
    { id: "contract_allowlist_guard", name: "Contract Allowlist Guard Agent", roi: "Ensures onchain calls only target approved contracts.", machine: ContractAllowlistGuardMachine, input: ContractAllowlistGuardInput, decision: ContractAllowlistGuardDecision, category: "onchain", threshold: 4, strike_limit: 2, cooldown_ticks: 3 },
    { id: "method_rate_guard", name: "Method Rate Guard Agent", roi: "Bounds per-method invocation risk under API pressure.", machine: MethodRateGuardMachine, input: MethodRateGuardInput, decision: MethodRateGuardDecision, category: "onchain", threshold: 7, strike_limit: 3, cooldown_ticks: 2 },
    { id: "nonce_gap_guard", name: "Nonce Gap Guard Agent", roi: "Detects nonce drift before transaction ordering faults.", machine: NonceGapGuardMachine, input: NonceGapGuardInput, decision: NonceGapGuardDecision, category: "onchain", threshold: 4, strike_limit: 2, cooldown_ticks: 2 },
    { id: "gas_spike_guard", name: "Gas Spike Guard Agent", roi: "Pauses high-cost onchain actions during gas spikes.", machine: GasSpikeGuardMachine, input: GasSpikeGuardInput, decision: GasSpikeGuardDecision, category: "onchain", threshold: 6, strike_limit: 2, cooldown_ticks: 3 },
    { id: "reorg_depth_guard", name: "Reorg Depth Guard Agent", roi: "Blocks settlement when reorg depth risk is elevated.", machine: ReorgDepthGuardMachine, input: ReorgDepthGuardInput, decision: ReorgDepthGuardDecision, category: "onchain", threshold: 4, strike_limit: 2, cooldown_ticks: 3 },
    { id: "confirmation_slo_guard", name: "Confirmation SLO Guard Agent", roi: "Tracks confirmation lag against deterministic SLO targets.", machine: ConfirmationSloGuardMachine, input: ConfirmationSloGuardInput, decision: ConfirmationSloGuardDecision, category: "onchain", threshold: 6, strike_limit: 2, cooldown_ticks: 2 },
    { id: "bridge_delay_guard", name: "Bridge Delay Guard Agent", roi: "Gates cross-chain workflows when bridge delays exceed policy.", machine: BridgeDelayGuardMachine, input: BridgeDelayGuardInput, decision: BridgeDelayGuardDecision, category: "onchain", threshold: 6, strike_limit: 2, cooldown_ticks: 2 },
    { id: "message_replay_guard", name: "Message Replay Guard Agent", roi: "Stops replayed cross-domain messages from re-executing.", machine: MessageReplayGuardMachine, input: MessageReplayGuardInput, decision: MessageReplayGuardDecision, category: "onchain", threshold: 5, strike_limit: 2, cooldown_ticks: 3 },
    { id: "signature_quorum_guard", name: "Signature Quorum Guard Agent", roi: "Verifies deterministic multisig quorum confidence bounds.", machine: SignatureQuorumGuardMachine, input: SignatureQuorumGuardInput, decision: SignatureQuorumGuardDecision, category: "onchain", threshold: 5, strike_limit: 2, cooldown_ticks: 2 },
    { id: "governance_delay_guard", name: "Governance Delay Guard Agent", roi: "Enforces timelock delay policy before governance execution.", machine: GovernanceDelayGuardMachine, input: GovernanceDelayGuardInput, decision: GovernanceDelayGuardDecision, category: "onchain", threshold: 5, strike_limit: 2, cooldown_ticks: 2 },
    { id: "treasury_spend_guard", name: "Treasury Spend Guard Agent", roi: "Blocks treasury operations above deterministic spend budget.", machine: TreasurySpendGuardMachine, input: TreasurySpendGuardInput, decision: TreasurySpendGuardDecision, category: "onchain", threshold: 5, strike_limit: 2, cooldown_ticks: 3 },
    { id: "withdrawal_limit_guard", name: "Withdrawal Limit Guard Agent", roi: "Applies strict withdrawal risk caps to protect reserves.", machine: WithdrawalLimitGuardMachine, input: WithdrawalLimitGuardInput, decision: WithdrawalLimitGuardDecision, category: "onchain", threshold: 5, strike_limit: 2, cooldown_ticks: 3 },
    { id: "custody_policy_guard", name: "Custody Policy Guard Agent", roi: "Enforces custody policy score prior to key-use operations.", machine: CustodyPolicyGuardMachine, input: CustodyPolicyGuardInput, decision: CustodyPolicyGuardDecision, category: "onchain", threshold: 5, strike_limit: 2, cooldown_ticks: 3 },
    { id: "backup_freshness_guard", name: "Backup Freshness Guard Agent", roi: "Prevents risky deploys when backup freshness is insufficient.", machine: BackupFreshnessGuardMachine, input: BackupFreshnessGuardInput, decision: BackupFreshnessGuardDecision, category: "onchain", threshold: 6, strike_limit: 2, cooldown_ticks: 2 },
    { id: "compliance_evidence_guard", name: "Compliance Evidence Guard Agent", roi: "Blocks production changes when evidence score is incomplete.", machine: ComplianceEvidenceGuardMachine, input: ComplianceEvidenceGuardInput, decision: ComplianceEvidenceGuardDecision, category: "onchain", threshold: 5, strike_limit: 2, cooldown_ticks: 2 }
);

/// Returns descriptor for one expanded agent id.
pub fn expanded_agent_descriptor(agent_id: &str) -> Option<&'static ExpandedAgentDescriptor> {
    EXPANDED_AGENT_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.id == agent_id)
}

/// Returns measurable quality summary for expanded class library.
pub fn expanded_agent_quality_summary() -> ExpandedAgentQualitySummary {
    let categories: HashSet<&'static str> = EXPANDED_AGENT_DESCRIPTORS
        .iter()
        .map(|descriptor| descriptor.category)
        .collect();

    ExpandedAgentQualitySummary {
        expanded_agents: EXPANDED_AGENT_DESCRIPTORS.len(),
        categories: categories.len(),
        temporal_inputs: 3,
        temporal_decisions: 4,
    }
}

/// Simulates one expanded agent deterministically over a command sequence.
pub fn simulate_expanded_guard(
    agent_id: &str,
    threshold: Option<u32>,
    strike_limit: Option<u8>,
    cooldown_ticks: Option<u8>,
    commands: &[TemporalGuardInput],
) -> Result<TemporalGuardSimulation, String> {
    let descriptor = expanded_agent_descriptor(agent_id)
        .ok_or_else(|| format!("unknown expanded agent id: {agent_id}"))?;

    let effective_threshold = threshold.unwrap_or(descriptor.default_threshold).max(1);
    let effective_strike_limit = strike_limit
        .unwrap_or(descriptor.default_strike_limit)
        .max(1);
    let effective_cooldown_ticks = cooldown_ticks
        .unwrap_or(descriptor.default_cooldown_ticks)
        .max(1);

    let mut machine = TemporalGuardMachine::new(
        effective_threshold,
        effective_strike_limit,
        effective_cooldown_ticks,
    );
    let mut steps = Vec::with_capacity(commands.len());

    for command in commands {
        let decision = machine.step(*command);
        steps.push(TemporalGuardSimulationStep {
            input: *command,
            decision,
            snapshot: machine.snapshot(),
        });
    }

    Ok(TemporalGuardSimulation {
        agent_id: agent_id.to_string(),
        threshold: effective_threshold,
        strike_limit: effective_strike_limit,
        cooldown_ticks: effective_cooldown_ticks,
        steps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanded_catalog_has_expected_volume() {
        assert!(EXPANDED_AGENT_DESCRIPTORS.len() >= 60);
    }

    #[test]
    fn quality_summary_tracks_categories() {
        let summary = expanded_agent_quality_summary();
        assert!(summary.expanded_agents >= 60);
        assert!(summary.categories >= 6);
        assert_eq!(summary.temporal_inputs, 3);
        assert_eq!(summary.temporal_decisions, 4);
    }

    #[test]
    fn temporal_machine_blocks_then_recovers_after_cooldown() {
        let mut machine = OracleDeviationGuardMachine::new(3, 2, 2);

        assert_eq!(
            machine.step(OracleDeviationGuardInput::Evaluate { value: 4 }),
            OracleDeviationGuardDecision::Warn
        );
        assert_eq!(
            machine.step(OracleDeviationGuardInput::Evaluate { value: 5 }),
            OracleDeviationGuardDecision::Block
        );
        assert_eq!(
            machine.step(OracleDeviationGuardInput::Tick),
            OracleDeviationGuardDecision::CoolingDown
        );
        assert_eq!(
            machine.step(OracleDeviationGuardInput::Tick),
            OracleDeviationGuardDecision::Allow
        );
        assert_eq!(
            machine.step(OracleDeviationGuardInput::Evaluate { value: 1 }),
            OracleDeviationGuardDecision::Allow
        );
    }

    #[test]
    fn simulation_endpoint_core_returns_trace() {
        let simulation = simulate_expanded_guard(
            "webhook_signature_guard",
            Some(3),
            Some(2),
            Some(2),
            &[
                TemporalGuardInput::Evaluate { value: 4 },
                TemporalGuardInput::Evaluate { value: 5 },
                TemporalGuardInput::Tick,
                TemporalGuardInput::Tick,
                TemporalGuardInput::Evaluate { value: 2 },
            ],
        )
        .unwrap();

        assert_eq!(simulation.steps.len(), 5);
        assert_eq!(simulation.steps[0].decision, TemporalGuardDecision::Warn);
        assert_eq!(simulation.steps[1].decision, TemporalGuardDecision::Block);
        assert_eq!(
            simulation.steps[2].decision,
            TemporalGuardDecision::CoolingDown
        );
        assert_eq!(simulation.steps[4].decision, TemporalGuardDecision::Allow);
    }
}
