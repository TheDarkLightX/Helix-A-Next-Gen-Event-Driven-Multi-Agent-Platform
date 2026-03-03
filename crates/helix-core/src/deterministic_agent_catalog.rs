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

//! Catalog of deterministic high-ROI agents shipped with Helix.

use crate::deterministic_agents_expanded::{
    expanded_agent_quality_summary, EXPANDED_AGENT_DESCRIPTORS,
};
use serde::{Deserialize, Serialize};

/// Metadata for one deterministic agent kernel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeterministicAgentSpec {
    /// Stable agent identifier.
    pub id: String,
    /// Human-readable agent name.
    pub name: String,
    /// High-level value proposition for operations.
    pub roi_rationale: String,
    /// Core module implementing the machine.
    pub kernel_module: String,
    /// Formal model path for verification.
    pub formal_model: String,
}

/// Catalog-level quality metrics for deterministic agent coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCatalogQuality {
    /// Total number of exposed deterministic agent classes.
    pub total_agents: usize,
    /// Number of foundational agents.
    pub foundational_agents: usize,
    /// Number of expanded temporal guard agents.
    pub expanded_agents: usize,
    /// Number of expanded categories covered.
    pub expanded_categories: usize,
    /// Number of temporal input variants used by expanded agents.
    pub temporal_inputs: usize,
    /// Number of temporal decision variants used by expanded agents.
    pub temporal_decisions: usize,
    /// External baseline for market comparison.
    pub huginn_baseline_agents: usize,
    /// True when Helix exceeds the comparison baseline.
    pub exceeds_huginn: bool,
}

const HUGINN_BASELINE_AGENTS: usize = 68;

/// Returns the high-ROI deterministic agent catalog.
pub fn high_roi_agent_catalog() -> Vec<DeterministicAgentSpec> {
    let mut catalog = vec![
        DeterministicAgentSpec {
            id: "dedup_window".to_string(),
            name: "Dedup Window Agent".to_string(),
            roi_rationale: "Stops duplicate event storms before downstream cost.".to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::DedupMachine"
                .to_string(),
            formal_model: "formal/models/roi_agents/dedup_window.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "token_bucket".to_string(),
            name: "Token Bucket Rate Limiter".to_string(),
            roi_rationale: "Enforces deterministic admission and protects service capacity."
                .to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::RateLimiterMachine"
                .to_string(),
            formal_model: "formal/models/roi_agents/token_bucket.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "circuit_breaker".to_string(),
            name: "Circuit Breaker Agent".to_string(),
            roi_rationale: "Fast-fails unstable dependencies and controls recovery probes."
                .to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::CircuitBreakerMachine"
                .to_string(),
            formal_model: "formal/models/roi_agents/circuit_breaker.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "retry_budget".to_string(),
            name: "Retry Budget Agent".to_string(),
            roi_rationale: "Caps retries to prevent runaway loops and queue collapse.".to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::RetryBudgetMachine"
                .to_string(),
            formal_model: "formal/models/roi_agents/retry_budget.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "approval_gate".to_string(),
            name: "Approval Gate Agent".to_string(),
            roi_rationale: "Adds deterministic quorum control for risky operations.".to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::ApprovalGateMachine"
                .to_string(),
            formal_model: "formal/models/roi_agents/approval_gate.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "backpressure".to_string(),
            name: "Backpressure Controller Agent".to_string(),
            roi_rationale: "Predictably throttles or sheds load under queue pressure.".to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::BackpressureMachine"
                .to_string(),
            formal_model: "formal/models/roi_agents/backpressure.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "sla_deadline".to_string(),
            name: "SLA Deadline Agent".to_string(),
            roi_rationale: "Turns timing SLOs into deterministic state transitions.".to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::SlaDeadlineMachine"
                .to_string(),
            formal_model: "formal/models/roi_agents/sla_deadline.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "dlq_budget".to_string(),
            name: "DLQ Budget Agent".to_string(),
            roi_rationale: "Routes repeated failures to DLQ before they poison throughput."
                .to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::DlqBudgetMachine"
                .to_string(),
            formal_model: "formal/models/roi_agents/dlq_budget.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "nonce_manager".to_string(),
            name: "Nonce Manager Agent".to_string(),
            roi_rationale: "Prevents nonce collisions and tracks in-flight nonce reservations."
                .to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::NonceManagerMachine"
                .to_string(),
            formal_model: "formal/models/roi_agents/nonce_manager.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "fee_bidding".to_string(),
            name: "Fee Bidding Agent".to_string(),
            roi_rationale: "Generates deterministic EIP-1559 fee quotes with bounded bumping."
                .to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::FeeBiddingMachine"
                .to_string(),
            formal_model: "formal/models/roi_agents/fee_bidding.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "finality_guard".to_string(),
            name: "Finality Reorg Guard Agent".to_string(),
            roi_rationale:
                "Gates settlement on confirmation depth and fails closed on reorg signals."
                    .to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::FinalityGuardMachine"
                .to_string(),
            formal_model: "formal/models/roi_agents/finality_guard.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "allowlist_guard".to_string(),
            name: "Allowlist Policy Guard Agent".to_string(),
            roi_rationale:
                "Blocks unauthorized chain/contract/method tuples with deterministic pause control."
                    .to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::AllowlistPolicyMachine"
                .to_string(),
            formal_model: "formal/models/roi_agents/allowlist_guard.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "symbolic_reasoning_gate".to_string(),
            name: "Symbolic Reasoning Gate Agent".to_string(),
            roi_rationale: "Uses deterministic symbolic/KRR inference to gate execution."
                .to_string(),
            kernel_module: "crates/helix-core/src/reasoning.rs::SymbolicReasoningKernel"
                .to_string(),
            formal_model: "formal/models/reasoning/symbolic_reasoning_gate.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "expert_system_gate".to_string(),
            name: "Expert System Gate Agent".to_string(),
            roi_rationale: "Applies weighted expert rules for bounded deterministic decisions."
                .to_string(),
            kernel_module: "crates/helix-core/src/reasoning.rs::ExpertSystemKernel".to_string(),
            formal_model: "formal/models/reasoning/expert_system_gate.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "neuro_risk_gate".to_string(),
            name: "Neuro Risk Gate Agent".to_string(),
            roi_rationale: "Scores deterministic ML risk model outputs for controlled admission."
                .to_string(),
            kernel_module: "crates/helix-core/src/reasoning.rs::NeuroRiskKernel".to_string(),
            formal_model: "formal/models/reasoning/neuro_risk_gate.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "neuro_symbolic_fusion_gate".to_string(),
            name: "Neuro-Symbolic Fusion Gate Agent".to_string(),
            roi_rationale:
                "Combines symbolic proof gate with neural confidence under fail-closed policy."
                    .to_string(),
            kernel_module: "crates/helix-core/src/reasoning.rs::NeuroSymbolicFusionKernel"
                .to_string(),
            formal_model: "formal/models/reasoning/neuro_symbolic_fusion_gate.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "onchain_tx_intent".to_string(),
            name: "Onchain Transaction Intent Agent".to_string(),
            roi_rationale:
                "Provides deterministic transaction lifecycle control before RPC side effects."
                    .to_string(),
            kernel_module: "crates/helix-core/src/onchain_intent.rs".to_string(),
            formal_model: "formal/models/onchain_tx_intent.yaml".to_string(),
        },
    ];
    catalog.extend(expanded_agent_catalog());
    catalog
}

const SHARED_EXPANDED_AGENT_FORMAL_MODEL: &str =
    "formal/models/library/threshold_guard_reference.yaml";

fn expanded_agent_catalog() -> Vec<DeterministicAgentSpec> {
    EXPANDED_AGENT_DESCRIPTORS
        .iter()
        .map(|descriptor| DeterministicAgentSpec {
            id: descriptor.id.to_string(),
            name: descriptor.name.to_string(),
            roi_rationale: descriptor.roi_rationale.to_string(),
            kernel_module: format!(
                "crates/helix-core/src/deterministic_agents_expanded.rs::{}",
                descriptor.machine_type
            ),
            formal_model: SHARED_EXPANDED_AGENT_FORMAL_MODEL.to_string(),
        })
        .collect()
}

/// Returns measurable catalog quality metrics.
pub fn agent_catalog_quality() -> AgentCatalogQuality {
    let summary = expanded_agent_quality_summary();
    let total = high_roi_agent_catalog().len();
    let foundational = total.saturating_sub(summary.expanded_agents);
    AgentCatalogQuality {
        total_agents: total,
        foundational_agents: foundational,
        expanded_agents: summary.expanded_agents,
        expanded_categories: summary.categories,
        temporal_inputs: summary.temporal_inputs,
        temporal_decisions: summary.temporal_decisions,
        huginn_baseline_agents: HUGINN_BASELINE_AGENTS,
        exceeds_huginn: total > HUGINN_BASELINE_AGENTS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_exceeds_huginn_floor() {
        assert!(
            high_roi_agent_catalog().len() > HUGINN_BASELINE_AGENTS,
            "Helix should expose more agent classes than Huginn's baseline."
        );
    }

    #[test]
    fn catalog_ids_are_unique() {
        let catalog = high_roi_agent_catalog();
        let unique: HashSet<&str> = catalog.iter().map(|agent| agent.id.as_str()).collect();
        assert_eq!(catalog.len(), unique.len());
    }

    #[test]
    fn quality_metrics_report_baseline_win() {
        let quality = agent_catalog_quality();
        assert!(quality.exceeds_huginn);
        assert!(quality.expanded_categories >= 6);
        assert_eq!(quality.temporal_inputs, 3);
        assert_eq!(quality.temporal_decisions, 4);
    }
}
