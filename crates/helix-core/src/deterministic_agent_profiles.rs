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

//! Deterministic policy templates for high-ROI agent deployment.

use crate::deterministic_policy::{DeterministicPolicyConfig, PolicyCommand};
use serde::{Deserialize, Serialize};

/// Reusable policy template that bundles deterministic config and bootstrap commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeterministicAgentTemplate {
    /// Stable template identifier.
    pub id: String,
    /// Human-readable template name.
    pub name: String,
    /// Short template description.
    pub summary: String,
    /// Operator-facing recommendation for when to use the template.
    pub recommended_for: String,
    /// Agent ids required by this template.
    pub required_agents: Vec<String>,
    /// Full deterministic policy config for this template.
    pub config: DeterministicPolicyConfig,
    /// Deterministic command sequence to dry-run after applying config.
    pub bootstrap_commands: Vec<PolicyCommand>,
}

/// Returns high-ROI deterministic policy templates.
pub fn high_roi_agent_templates() -> Vec<DeterministicAgentTemplate> {
    vec![
        webhook_ingress_safety_template(),
        latency_slo_template(),
        secure_onchain_executor_template(),
        approval_controlled_ops_template(),
    ]
}

/// Finds one deterministic template by id.
pub fn find_agent_template(template_id: &str) -> Option<DeterministicAgentTemplate> {
    high_roi_agent_templates()
        .into_iter()
        .find(|template| template.id == template_id)
}

fn webhook_ingress_safety_template() -> DeterministicAgentTemplate {
    let mut config = DeterministicPolicyConfig::default();
    config.dedup_window_ticks = 5;
    config.rate_max_tokens = 25;
    config.rate_refill_per_tick = 5;
    config.breaker_failure_threshold = 4;
    config.breaker_open_duration_ticks = 2;
    config.retry_budget = 2;
    config.dlq_max_consecutive_failures = 3;

    DeterministicAgentTemplate {
        id: "webhook_ingress_safety".to_string(),
        name: "Webhook Ingress Safety".to_string(),
        summary: "Stabilizes inbound webhook/event streams with dedup, admission control, breaker, retry, and DLQ bounds.".to_string(),
        recommended_for: "Public event ingress, webhook fan-in, and high-volume trigger pipelines.".to_string(),
        required_agents: vec![
            "dedup_window".to_string(),
            "token_bucket".to_string(),
            "circuit_breaker".to_string(),
            "retry_budget".to_string(),
            "dlq_budget".to_string(),
        ],
        config,
        bootstrap_commands: vec![
            PolicyCommand::Request {
                fingerprint: 10,
                cost: 1,
            },
            PolicyCommand::Request {
                fingerprint: 10,
                cost: 1,
            },
            PolicyCommand::Failure,
            PolicyCommand::Failure,
            PolicyCommand::Request {
                fingerprint: 11,
                cost: 2,
            },
            PolicyCommand::Tick,
            PolicyCommand::Success,
            PolicyCommand::ResetRetry,
            PolicyCommand::ResetDlq,
        ],
    }
}

fn latency_slo_template() -> DeterministicAgentTemplate {
    let mut config = DeterministicPolicyConfig::default();
    config.backpressure_soft_limit = 3;
    config.backpressure_hard_limit = 5;
    config.sla_deadline_ticks = 2;
    config.retry_budget = 1;

    DeterministicAgentTemplate {
        id: "latency_slo_protection".to_string(),
        name: "Latency SLO Protection".to_string(),
        summary: "Combines queue-aware backpressure and deterministic SLA windows to fail closed when latency drifts.".to_string(),
        recommended_for: "Interactive workloads, low-latency APIs, and event consumers with strict deadlines.".to_string(),
        required_agents: vec![
            "backpressure".to_string(),
            "sla_deadline".to_string(),
            "retry_budget".to_string(),
        ],
        config,
        bootstrap_commands: vec![
            PolicyCommand::StartSlaWindow,
            PolicyCommand::EnqueueBackpressure { count: 2 },
            PolicyCommand::Request {
                fingerprint: 100,
                cost: 1,
            },
            PolicyCommand::Tick,
            PolicyCommand::EnqueueBackpressure { count: 4 },
            PolicyCommand::Tick,
            PolicyCommand::Request {
                fingerprint: 101,
                cost: 1,
            },
            PolicyCommand::CompleteSlaWindow,
            PolicyCommand::ResetSlaWindow,
        ],
    }
}

fn secure_onchain_executor_template() -> DeterministicAgentTemplate {
    let mut config = DeterministicPolicyConfig::default();
    config.nonce_start = 0;
    config.nonce_max_in_flight = 32;
    config.fee_base_fee = 120;
    config.fee_priority_fee = 3;
    config.fee_bump_bps = 300;
    config.fee_max_fee_cap = 20_000;
    config.finality_required_depth = 12;
    config.allowlist_chain_id = 1;
    config.allowlist_contract_tag = 55;
    config.allowlist_method_tag = 0xdeadbeef;

    DeterministicAgentTemplate {
        id: "secure_onchain_executor".to_string(),
        name: "Secure Onchain Executor".to_string(),
        summary: "Hardens transaction execution with nonce control, bounded fees, finality guard, and allowlist enforcement.".to_string(),
        recommended_for: "LLM-assisted onchain actions, treasury operations, and production transaction relays.".to_string(),
        required_agents: vec![
            "nonce_manager".to_string(),
            "fee_bidding".to_string(),
            "finality_guard".to_string(),
            "allowlist_guard".to_string(),
            "onchain_tx_intent".to_string(),
        ],
        config,
        bootstrap_commands: vec![
            PolicyCommand::AllowlistEvaluate {
                chain_id: 1,
                contract_tag: 55,
                method_tag: 0xdeadbeef,
            },
            PolicyCommand::NonceReserve,
            PolicyCommand::FeeQuote { urgent: false },
            PolicyCommand::FeeRejected,
            PolicyCommand::FeeQuote { urgent: true },
            PolicyCommand::FinalityObserveDepth { depth: 3 },
            PolicyCommand::FinalityObserveDepth { depth: 12 },
        ],
    }
}

fn approval_controlled_ops_template() -> DeterministicAgentTemplate {
    let mut config = DeterministicPolicyConfig::default();
    config.approval_quorum = 2;
    config.approval_reviewers = 3;
    config.allowlist_chain_id = 1;
    config.allowlist_contract_tag = 55;
    config.allowlist_method_tag = 0xdeadbeef;

    DeterministicAgentTemplate {
        id: "approval_controlled_ops".to_string(),
        name: "Approval Controlled Operations".to_string(),
        summary: "Adds deterministic quorum gating before privileged operations and enforces allowlist boundaries.".to_string(),
        recommended_for: "Production change management, compliance-sensitive flows, and human-in-the-loop runbooks.".to_string(),
        required_agents: vec!["approval_gate".to_string(), "allowlist_guard".to_string()],
        config,
        bootstrap_commands: vec![
            PolicyCommand::ResetApprovals,
            PolicyCommand::Approve,
            PolicyCommand::Approve,
            PolicyCommand::AllowlistEvaluate {
                chain_id: 1,
                contract_tag: 55,
                method_tag: 0xdeadbeef,
            },
            PolicyCommand::AllowlistEvaluate {
                chain_id: 1,
                contract_tag: 77,
                method_tag: 0xdeadbeef,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deterministic_agent_catalog::high_roi_agent_catalog;
    use std::collections::HashSet;

    #[test]
    fn template_ids_are_unique() {
        let templates = high_roi_agent_templates();
        let ids: HashSet<&str> = templates
            .iter()
            .map(|template| template.id.as_str())
            .collect();
        assert_eq!(ids.len(), templates.len());
    }

    #[test]
    fn required_agents_exist_in_catalog() {
        let known_agents: HashSet<String> = high_roi_agent_catalog()
            .into_iter()
            .map(|agent| agent.id)
            .collect();

        for template in high_roi_agent_templates() {
            for agent_id in template.required_agents {
                assert!(
                    known_agents.contains(&agent_id),
                    "unknown required agent id: {agent_id}"
                );
            }
        }
    }

    #[test]
    fn find_template_returns_expected_template() {
        let template =
            find_agent_template("secure_onchain_executor").expect("template should exist");
        assert_eq!(template.name, "Secure Onchain Executor");
        assert!(!template.bootstrap_commands.is_empty());
    }
}
