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
    /// ESSO model path for verification.
    pub esso_model: String,
}

/// Returns the high-ROI deterministic agent catalog.
pub fn high_roi_agent_catalog() -> Vec<DeterministicAgentSpec> {
    vec![
        DeterministicAgentSpec {
            id: "dedup_window".to_string(),
            name: "Dedup Window Agent".to_string(),
            roi_rationale: "Stops duplicate event storms before downstream cost.".to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::DedupMachine".to_string(),
            esso_model: "formal/esso/roi_agents/dedup_window.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "token_bucket".to_string(),
            name: "Token Bucket Rate Limiter".to_string(),
            roi_rationale: "Enforces deterministic admission and protects service capacity."
                .to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::RateLimiterMachine"
                .to_string(),
            esso_model: "formal/esso/roi_agents/token_bucket.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "circuit_breaker".to_string(),
            name: "Circuit Breaker Agent".to_string(),
            roi_rationale: "Fast-fails unstable dependencies and controls recovery probes."
                .to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::CircuitBreakerMachine"
                .to_string(),
            esso_model: "formal/esso/roi_agents/circuit_breaker.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "retry_budget".to_string(),
            name: "Retry Budget Agent".to_string(),
            roi_rationale: "Caps retries to prevent runaway loops and queue collapse.".to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::RetryBudgetMachine"
                .to_string(),
            esso_model: "formal/esso/roi_agents/retry_budget.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "approval_gate".to_string(),
            name: "Approval Gate Agent".to_string(),
            roi_rationale: "Adds deterministic quorum control for risky operations.".to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::ApprovalGateMachine"
                .to_string(),
            esso_model: "formal/esso/roi_agents/approval_gate.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "backpressure".to_string(),
            name: "Backpressure Controller Agent".to_string(),
            roi_rationale: "Predictably throttles or sheds load under queue pressure.".to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::BackpressureMachine"
                .to_string(),
            esso_model: "formal/esso/roi_agents/backpressure.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "sla_deadline".to_string(),
            name: "SLA Deadline Agent".to_string(),
            roi_rationale: "Turns timing SLOs into deterministic state transitions.".to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::SlaDeadlineMachine"
                .to_string(),
            esso_model: "formal/esso/roi_agents/sla_deadline.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "dlq_budget".to_string(),
            name: "DLQ Budget Agent".to_string(),
            roi_rationale:
                "Routes repeated failures to DLQ before they poison throughput.".to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::DlqBudgetMachine"
                .to_string(),
            esso_model: "formal/esso/roi_agents/dlq_budget.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "nonce_manager".to_string(),
            name: "Nonce Manager Agent".to_string(),
            roi_rationale:
                "Prevents nonce collisions and tracks in-flight nonce reservations."
                    .to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::NonceManagerMachine"
                .to_string(),
            esso_model: "formal/esso/roi_agents/nonce_manager.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "fee_bidding".to_string(),
            name: "Fee Bidding Agent".to_string(),
            roi_rationale:
                "Generates deterministic EIP-1559 fee quotes with bounded bumping."
                    .to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::FeeBiddingMachine"
                .to_string(),
            esso_model: "formal/esso/roi_agents/fee_bidding.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "finality_guard".to_string(),
            name: "Finality Reorg Guard Agent".to_string(),
            roi_rationale:
                "Gates settlement on confirmation depth and fails closed on reorg signals."
                    .to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::FinalityGuardMachine"
                .to_string(),
            esso_model: "formal/esso/roi_agents/finality_guard.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "allowlist_guard".to_string(),
            name: "Allowlist Policy Guard Agent".to_string(),
            roi_rationale:
                "Blocks unauthorized chain/contract/method tuples with deterministic pause control."
                    .to_string(),
            kernel_module: "crates/helix-core/src/deterministic_agents.rs::AllowlistPolicyMachine"
                .to_string(),
            esso_model: "formal/esso/roi_agents/allowlist_guard.yaml".to_string(),
        },
        DeterministicAgentSpec {
            id: "onchain_tx_intent".to_string(),
            name: "Onchain Transaction Intent Agent".to_string(),
            roi_rationale:
                "Provides deterministic transaction lifecycle control before RPC side effects."
                    .to_string(),
            kernel_module: "crates/helix-core/src/onchain_intent.rs".to_string(),
            esso_model: "formal/esso/onchain_tx_intent.yaml".to_string(),
        },
    ]
}
