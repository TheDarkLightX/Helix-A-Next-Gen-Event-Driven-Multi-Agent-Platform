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

use crate::deterministic_agents_expanded::EXPANDED_AGENT_DESCRIPTORS;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_exceeds_huginn_floor() {
        assert!(
            high_roi_agent_catalog().len() > 68,
            "Helix should expose more agent classes than Huginn's 68-class baseline."
        );
    }

    #[test]
    fn catalog_ids_are_unique() {
        let catalog = high_roi_agent_catalog();
        let unique: HashSet<&str> = catalog.iter().map(|agent| agent.id.as_str()).collect();
        assert_eq!(catalog.len(), unique.len());
    }
}
