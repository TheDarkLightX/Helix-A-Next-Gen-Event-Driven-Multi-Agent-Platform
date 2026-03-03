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
//! These classes are intentionally simple, fully deterministic threshold guards
//! used to scale Helix's agent library breadth while preserving pure-step semantics.

use serde::{Deserialize, Serialize};

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
}

macro_rules! define_threshold_guard_machine {
    ($machine:ident, $input:ident, $decision:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $decision {
            /// Input is within the configured threshold.
            Allow,
            /// Input exceeds configured threshold.
            Block,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $input {
            /// Evaluate one bounded signal value.
            Evaluate { value: u32 },
            /// Reset dynamic state to deterministic baseline.
            Reset,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub struct $machine {
            threshold: u32,
            peak_observed: u32,
        }

        impl $machine {
            /// Creates a machine with a non-zero threshold.
            pub fn new(threshold: u32) -> Self {
                Self {
                    threshold: threshold.max(1),
                    peak_observed: 0,
                }
            }

            /// Current configured threshold.
            pub fn threshold(self) -> u32 {
                self.threshold
            }

            /// Maximum value seen since last reset.
            pub fn peak_observed(self) -> u32 {
                self.peak_observed
            }

            /// Applies one deterministic step.
            pub fn step(&mut self, input: $input) -> $decision {
                match input {
                    $input::Reset => {
                        self.peak_observed = 0;
                        $decision::Allow
                    }
                    $input::Evaluate { value } => {
                        self.peak_observed = self.peak_observed.max(value);
                        if value <= self.threshold {
                            $decision::Allow
                        } else {
                            $decision::Block
                        }
                    }
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
                decision: $decision:ident
            }
        ),+ $(,)?
    ) => {
        $(define_threshold_guard_machine!($machine, $input, $decision);)+

        /// All expanded deterministic agents exposed by Helix.
        pub const EXPANDED_AGENT_DESCRIPTORS: &[ExpandedAgentDescriptor] = &[
            $(ExpandedAgentDescriptor {
                id: $id,
                name: $name,
                roi_rationale: $roi,
                machine_type: stringify!($machine),
            }),+
        ];
    };
}

define_expanded_agents!(
    { id: "webhook_signature_guard", name: "Webhook Signature Guard Agent", roi: "Rejects unsigned webhook payloads before dispatch.", machine: WebhookSignatureGuardMachine, input: WebhookSignatureGuardInput, decision: WebhookSignatureGuardDecision },
    { id: "payload_schema_guard", name: "Payload Schema Guard Agent", roi: "Blocks malformed payloads at ingress to avoid downstream faults.", machine: PayloadSchemaGuardMachine, input: PayloadSchemaGuardInput, decision: PayloadSchemaGuardDecision },
    { id: "json_size_guard", name: "JSON Size Guard Agent", roi: "Prevents oversized request bodies from exhausting parser capacity.", machine: JsonSizeGuardMachine, input: JsonSizeGuardInput, decision: JsonSizeGuardDecision },
    { id: "pii_redaction_guard", name: "PII Redaction Guard Agent", roi: "Gates events that exceed configured sensitive-data risk score.", machine: PiiRedactionGuardMachine, input: PiiRedactionGuardInput, decision: PiiRedactionGuardDecision },
    { id: "idempotency_key_guard", name: "Idempotency Key Guard Agent", roi: "Enforces bounded idempotency key quality before execution.", machine: IdempotencyKeyGuardMachine, input: IdempotencyKeyGuardInput, decision: IdempotencyKeyGuardDecision },
    { id: "replay_window_guard", name: "Replay Window Guard Agent", roi: "Blocks stale replay attempts outside deterministic freshness bounds.", machine: ReplayWindowGuardMachine, input: ReplayWindowGuardInput, decision: ReplayWindowGuardDecision },
    { id: "queue_latency_guard", name: "Queue Latency Guard Agent", roi: "Stops latency blowups by rejecting over-budget queue delay.", machine: QueueLatencyGuardMachine, input: QueueLatencyGuardInput, decision: QueueLatencyGuardDecision },
    { id: "deadman_switch_guard", name: "Deadman Switch Guard Agent", roi: "Fails closed when heartbeat-like liveness score crosses threshold.", machine: DeadmanSwitchGuardMachine, input: DeadmanSwitchGuardInput, decision: DeadmanSwitchGuardDecision },
    { id: "heartbeat_monitor_guard", name: "Heartbeat Monitor Guard Agent", roi: "Detects deterministic heartbeat degradation before full outage.", machine: HeartbeatMonitorGuardMachine, input: HeartbeatMonitorGuardInput, decision: HeartbeatMonitorGuardDecision },
    { id: "drift_budget_guard", name: "Drift Budget Guard Agent", roi: "Keeps model/runtime drift within an explicit bounded budget.", machine: DriftBudgetGuardMachine, input: DriftBudgetGuardInput, decision: DriftBudgetGuardDecision },
    { id: "cost_budget_guard", name: "Cost Budget Guard Agent", roi: "Prevents run-away spend by gating over-budget operations.", machine: CostBudgetGuardMachine, input: CostBudgetGuardInput, decision: CostBudgetGuardDecision },
    { id: "api_quota_guard", name: "API Quota Guard Agent", roi: "Protects upstream quotas with deterministic request admission.", machine: ApiQuotaGuardMachine, input: ApiQuotaGuardInput, decision: ApiQuotaGuardDecision },
    { id: "concurrency_limit_guard", name: "Concurrency Limit Guard Agent", roi: "Bounds concurrent load to preserve tail-latency SLOs.", machine: ConcurrencyLimitGuardMachine, input: ConcurrencyLimitGuardInput, decision: ConcurrencyLimitGuardDecision },
    { id: "lease_expiry_guard", name: "Lease Expiry Guard Agent", roi: "Ensures stale leases cannot continue issuing side effects.", machine: LeaseExpiryGuardMachine, input: LeaseExpiryGuardInput, decision: LeaseExpiryGuardDecision },
    { id: "token_rotation_guard", name: "Token Rotation Guard Agent", roi: "Flags stale auth token health before credential outages.", machine: TokenRotationGuardMachine, input: TokenRotationGuardInput, decision: TokenRotationGuardDecision },
    { id: "secret_age_guard", name: "Secret Age Guard Agent", roi: "Deterministically enforces secret rotation windows.", machine: SecretAgeGuardMachine, input: SecretAgeGuardInput, decision: SecretAgeGuardDecision },
    { id: "dependency_health_gate", name: "Dependency Health Gate Agent", roi: "Avoids cascading failure by gating unhealthy dependencies.", machine: DependencyHealthGateMachine, input: DependencyHealthGateInput, decision: DependencyHealthGateDecision },
    { id: "response_time_slo_guard", name: "Response Time SLO Guard Agent", roi: "Stops SLO erosion by bounding deterministic latency score.", machine: ResponseTimeSloGuardMachine, input: ResponseTimeSloGuardInput, decision: ResponseTimeSloGuardDecision },
    { id: "error_rate_slo_guard", name: "Error Rate SLO Guard Agent", roi: "Trips before incidents when error-rate score exceeds budget.", machine: ErrorRateSloGuardMachine, input: ErrorRateSloGuardInput, decision: ErrorRateSloGuardDecision },
    { id: "canary_promotion_gate", name: "Canary Promotion Gate Agent", roi: "Prevents unsafe rollout promotion under degraded canary signal.", machine: CanaryPromotionGateMachine, input: CanaryPromotionGateInput, decision: CanaryPromotionGateDecision },
    { id: "rollback_trigger_guard", name: "Rollback Trigger Guard Agent", roi: "Automates deterministic rollback trigger gating.", machine: RollbackTriggerGuardMachine, input: RollbackTriggerGuardInput, decision: RollbackTriggerGuardDecision },
    { id: "incident_escalation_gate", name: "Incident Escalation Gate Agent", roi: "Escalates only when incident severity crosses explicit threshold.", machine: IncidentEscalationGateMachine, input: IncidentEscalationGateInput, decision: IncidentEscalationGateDecision },
    { id: "maintenance_window_guard", name: "Maintenance Window Guard Agent", roi: "Blocks risky operations outside approved maintenance windows.", machine: MaintenanceWindowGuardMachine, input: MaintenanceWindowGuardInput, decision: MaintenanceWindowGuardDecision },
    { id: "feature_flag_guard", name: "Feature Flag Guard Agent", roi: "Prevents unsafe flag flips with deterministic risk gating.", machine: FeatureFlagGuardMachine, input: FeatureFlagGuardInput, decision: FeatureFlagGuardDecision },
    { id: "tenant_isolation_guard", name: "Tenant Isolation Guard Agent", roi: "Bounds cross-tenant risk with explicit isolation gate.", machine: TenantIsolationGuardMachine, input: TenantIsolationGuardInput, decision: TenantIsolationGuardDecision },
    { id: "data_residency_guard", name: "Data Residency Guard Agent", roi: "Enforces jurisdiction residency bounds before data movement.", machine: DataResidencyGuardMachine, input: DataResidencyGuardInput, decision: DataResidencyGuardDecision },
    { id: "geo_fence_guard", name: "Geo Fence Guard Agent", roi: "Restricts operations to approved regions deterministically.", machine: GeoFenceGuardMachine, input: GeoFenceGuardInput, decision: GeoFenceGuardDecision },
    { id: "time_window_guard", name: "Time Window Guard Agent", roi: "Gates actions to approved execution windows.", machine: TimeWindowGuardMachine, input: TimeWindowGuardInput, decision: TimeWindowGuardDecision },
    { id: "duplicate_payment_guard", name: "Duplicate Payment Guard Agent", roi: "Blocks duplicate disbursement patterns before settlement.", machine: DuplicatePaymentGuardMachine, input: DuplicatePaymentGuardInput, decision: DuplicatePaymentGuardDecision },
    { id: "settlement_delay_guard", name: "Settlement Delay Guard Agent", roi: "Caps settlement delay risk to reduce reconciliation defects.", machine: SettlementDelayGuardMachine, input: SettlementDelayGuardInput, decision: SettlementDelayGuardDecision },
    { id: "fraud_score_guard", name: "Fraud Score Guard Agent", roi: "Routes high-risk transactions away from auto approval.", machine: FraudScoreGuardMachine, input: FraudScoreGuardInput, decision: FraudScoreGuardDecision },
    { id: "kyc_status_guard", name: "KYC Status Guard Agent", roi: "Deterministically blocks operations when KYC status is out of policy.", machine: KycStatusGuardMachine, input: KycStatusGuardInput, decision: KycStatusGuardDecision },
    { id: "aml_velocity_guard", name: "AML Velocity Guard Agent", roi: "Trips on suspicious transaction velocity before compliance breach.", machine: AmlVelocityGuardMachine, input: AmlVelocityGuardInput, decision: AmlVelocityGuardDecision },
    { id: "account_lockout_guard", name: "Account Lockout Guard Agent", roi: "Prevents brute-force abuse with deterministic lockout gating.", machine: AccountLockoutGuardMachine, input: AccountLockoutGuardInput, decision: AccountLockoutGuardDecision },
    { id: "session_anomaly_guard", name: "Session Anomaly Guard Agent", roi: "Blocks anomalous session behavior above configured risk.", machine: SessionAnomalyGuardMachine, input: SessionAnomalyGuardInput, decision: SessionAnomalyGuardDecision },
    { id: "ip_reputation_guard", name: "IP Reputation Guard Agent", roi: "Guards ingress with bounded IP reputation risk.", machine: IpReputationGuardMachine, input: IpReputationGuardInput, decision: IpReputationGuardDecision },
    { id: "device_trust_guard", name: "Device Trust Guard Agent", roi: "Applies deterministic trust threshold for device-based access.", machine: DeviceTrustGuardMachine, input: DeviceTrustGuardInput, decision: DeviceTrustGuardDecision },
    { id: "request_burst_guard", name: "Request Burst Guard Agent", roi: "Flattens burst abuse by deterministic request gating.", machine: RequestBurstGuardMachine, input: RequestBurstGuardInput, decision: RequestBurstGuardDecision },
    { id: "cache_staleness_guard", name: "Cache Staleness Guard Agent", roi: "Prevents serving stale data beyond bounded freshness score.", machine: CacheStalenessGuardMachine, input: CacheStalenessGuardInput, decision: CacheStalenessGuardDecision },
    { id: "inventory_floor_guard", name: "Inventory Floor Guard Agent", roi: "Stops oversell scenarios by enforcing inventory floors.", machine: InventoryFloorGuardMachine, input: InventoryFloorGuardInput, decision: InventoryFloorGuardDecision },
    { id: "price_deviation_guard", name: "Price Deviation Guard Agent", roi: "Detects outlier pricing before customer-impacting anomalies.", machine: PriceDeviationGuardMachine, input: PriceDeviationGuardInput, decision: PriceDeviationGuardDecision },
    { id: "slippage_bound_guard", name: "Slippage Bound Guard Agent", roi: "Blocks execution when slippage risk exceeds budget.", machine: SlippageBoundGuardMachine, input: SlippageBoundGuardInput, decision: SlippageBoundGuardDecision },
    { id: "liquidity_depth_guard", name: "Liquidity Depth Guard Agent", roi: "Gates large fills when deterministic liquidity depth is insufficient.", machine: LiquidityDepthGuardMachine, input: LiquidityDepthGuardInput, decision: LiquidityDepthGuardDecision },
    { id: "oracle_staleness_guard", name: "Oracle Staleness Guard Agent", roi: "Prevents stale oracle reads from triggering unsafe actions.", machine: OracleStalenessGuardMachine, input: OracleStalenessGuardInput, decision: OracleStalenessGuardDecision },
    { id: "oracle_deviation_guard", name: "Oracle Deviation Guard Agent", roi: "Trips when oracle deviation risk breaches policy threshold.", machine: OracleDeviationGuardMachine, input: OracleDeviationGuardInput, decision: OracleDeviationGuardDecision },
    { id: "contract_allowlist_guard", name: "Contract Allowlist Guard Agent", roi: "Ensures onchain calls only target approved contracts.", machine: ContractAllowlistGuardMachine, input: ContractAllowlistGuardInput, decision: ContractAllowlistGuardDecision },
    { id: "method_rate_guard", name: "Method Rate Guard Agent", roi: "Bounds per-method invocation risk under API pressure.", machine: MethodRateGuardMachine, input: MethodRateGuardInput, decision: MethodRateGuardDecision },
    { id: "nonce_gap_guard", name: "Nonce Gap Guard Agent", roi: "Detects nonce drift before transaction ordering faults.", machine: NonceGapGuardMachine, input: NonceGapGuardInput, decision: NonceGapGuardDecision },
    { id: "gas_spike_guard", name: "Gas Spike Guard Agent", roi: "Pauses high-cost onchain actions during gas spikes.", machine: GasSpikeGuardMachine, input: GasSpikeGuardInput, decision: GasSpikeGuardDecision },
    { id: "reorg_depth_guard", name: "Reorg Depth Guard Agent", roi: "Blocks settlement when reorg depth risk is elevated.", machine: ReorgDepthGuardMachine, input: ReorgDepthGuardInput, decision: ReorgDepthGuardDecision },
    { id: "confirmation_slo_guard", name: "Confirmation SLO Guard Agent", roi: "Tracks confirmation lag against deterministic SLO targets.", machine: ConfirmationSloGuardMachine, input: ConfirmationSloGuardInput, decision: ConfirmationSloGuardDecision },
    { id: "bridge_delay_guard", name: "Bridge Delay Guard Agent", roi: "Gates cross-chain workflows when bridge delays exceed policy.", machine: BridgeDelayGuardMachine, input: BridgeDelayGuardInput, decision: BridgeDelayGuardDecision },
    { id: "message_replay_guard", name: "Message Replay Guard Agent", roi: "Stops replayed cross-domain messages from re-executing.", machine: MessageReplayGuardMachine, input: MessageReplayGuardInput, decision: MessageReplayGuardDecision },
    { id: "signature_quorum_guard", name: "Signature Quorum Guard Agent", roi: "Verifies deterministic multisig quorum confidence bounds.", machine: SignatureQuorumGuardMachine, input: SignatureQuorumGuardInput, decision: SignatureQuorumGuardDecision },
    { id: "governance_delay_guard", name: "Governance Delay Guard Agent", roi: "Enforces timelock delay policy before governance execution.", machine: GovernanceDelayGuardMachine, input: GovernanceDelayGuardInput, decision: GovernanceDelayGuardDecision },
    { id: "treasury_spend_guard", name: "Treasury Spend Guard Agent", roi: "Blocks treasury operations above deterministic spend budget.", machine: TreasurySpendGuardMachine, input: TreasurySpendGuardInput, decision: TreasurySpendGuardDecision },
    { id: "withdrawal_limit_guard", name: "Withdrawal Limit Guard Agent", roi: "Applies strict withdrawal risk caps to protect reserves.", machine: WithdrawalLimitGuardMachine, input: WithdrawalLimitGuardInput, decision: WithdrawalLimitGuardDecision },
    { id: "custody_policy_guard", name: "Custody Policy Guard Agent", roi: "Enforces custody policy score prior to key-use operations.", machine: CustodyPolicyGuardMachine, input: CustodyPolicyGuardInput, decision: CustodyPolicyGuardDecision },
    { id: "backup_freshness_guard", name: "Backup Freshness Guard Agent", roi: "Prevents risky deploys when backup freshness is insufficient.", machine: BackupFreshnessGuardMachine, input: BackupFreshnessGuardInput, decision: BackupFreshnessGuardDecision },
    { id: "compliance_evidence_guard", name: "Compliance Evidence Guard Agent", roi: "Blocks production changes when evidence score is incomplete.", machine: ComplianceEvidenceGuardMachine, input: ComplianceEvidenceGuardInput, decision: ComplianceEvidenceGuardDecision }
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanded_catalog_has_expected_volume() {
        assert!(EXPANDED_AGENT_DESCRIPTORS.len() >= 60);
    }

    #[test]
    fn threshold_guard_is_deterministic() {
        let mut machine = OracleDeviationGuardMachine::new(5);
        assert_eq!(
            machine.step(OracleDeviationGuardInput::Evaluate { value: 4 }),
            OracleDeviationGuardDecision::Allow
        );
        assert_eq!(
            machine.step(OracleDeviationGuardInput::Evaluate { value: 9 }),
            OracleDeviationGuardDecision::Block
        );
        assert_eq!(machine.peak_observed(), 9);
        assert_eq!(
            machine.step(OracleDeviationGuardInput::Reset),
            OracleDeviationGuardDecision::Allow
        );
        assert_eq!(machine.peak_observed(), 0);
    }
}
