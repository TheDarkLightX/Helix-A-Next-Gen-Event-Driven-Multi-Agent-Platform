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

//! Integration tests for the operator crate.

use crate::{
    config::{DeskOperatorConfig, OperatorActionScope, OperatorStatus},
    context::{DeskContextSnapshot, EvidenceSummary, WatchlistSummary, DeskContextSummary, RecentDecisionSummary},
    loop_runner::OperatorLoop,
    proposals::{parse_operator_proposals, proposal_to_action, OperatorAction, OperatorProposal},
};
use helix_core::autopilot_guard::{AutopilotGuardMachine, AutopilotMode};

#[tokio::test]
async fn full_operator_cycle_flow() {
    let config = DeskOperatorConfig {
        enabled: true,
        autopilot_mode: AutopilotMode::Auto,
        action_scope: OperatorActionScope::Intelligence,
        loop_interval_secs: 10,
        max_actions_per_cycle: 5,
        max_context_items: 10,
        model: "test-model".to_string(),
        rules_text: "Prioritize cases about entity X.".to_string(),
        dispatch_to_peers: false,
        log_denied_proposals: true,
    };
    config.validate().unwrap();

    let loop_runner = OperatorLoop::from_config(config);

    // Start
    assert_eq!(loop_runner.status().await, OperatorStatus::Stopped);
    loop_runner.start().await.unwrap();
    assert_eq!(loop_runner.status().await, OperatorStatus::Running);

    // Run a cycle with mock provider
    struct StubContext;
    impl crate::context::DeskContext for StubContext {
        fn snapshot(&self, _max: usize) -> DeskContextSnapshot {
            DeskContextSnapshot {
                source_count: 3,
                watchlist_count: 2,
                evidence_count: 10,
                claim_count: 5,
                open_case_count: 2,
                escalated_case_count: 1,
                top_cases: vec![DeskContextSummary {
                    id: "case-1".to_string(),
                    title: "Entity X investigation".to_string(),
                    status: "open".to_string(),
                    watchlist_id: "wl-1".to_string(),
                    primary_entity: Some("entity-x".to_string()),
                    evidence_count: 3,
                    claim_count: 2,
                    latest_reason: "New evidence".to_string(),
                    priority_total: 900,
                }],
                recent_evidence: vec![EvidenceSummary {
                    id: "ev-1".to_string(),
                    title: "Test evidence".to_string(),
                    source_id: "src-1".to_string(),
                    trust_score: 80,
                    observed_at: "2026-01-01T00:00:00Z".to_string(),
                    entity_labels: vec!["entity-x".to_string()],
                    tags: vec!["osint".to_string()],
                }],
                active_watchlists: vec![WatchlistSummary {
                    id: "wl-1".to_string(),
                    name: "Entity X Watch".to_string(),
                    severity: "high".to_string(),
                    keywords: vec!["entity-x".to_string()],
                    enabled: true,
                }],
                recent_decisions: vec![RecentDecisionSummary {
                    action_type: "log_analysis".to_string(),
                    decision: "allowed".to_string(),
                    denial_reason: None,
                    rationale: "Initial assessment".to_string(),
                    timestamp: "2026-01-01T00:00:00Z".to_string(),
                }],
            }
        }
    }

    struct StubLlmProvider;
    #[async_trait::async_trait]
    impl helix_llm::providers::LlmProvider for StubLlmProvider {
        fn name(&self) -> &str { "mock" }
        async fn get_models(&self) -> Result<Vec<helix_llm::providers::ModelConfig>, helix_llm::errors::LlmError> { Ok(vec![]) }
        async fn complete(
            &self,
            _req: helix_llm::providers::LlmRequest,
        ) -> Result<helix_llm::providers::LlmResponse, helix_llm::errors::LlmError> {
            Ok(helix_llm::providers::LlmResponse {
                content: r#"[
                    {"type":"log_analysis","rationale":"Entity X case is high priority","parameters":{"severity":"high","summary":"Entity X requires attention"}},
                    {"type":"escalate_case","target_id":"case-1","rationale":"Critical evidence found"}
                ]"#.to_string(),
                function_call: None,
                usage: helix_llm::providers::TokenUsage {
                    prompt_tokens: 100,
                    completion_tokens: 50,
                    total_tokens: 150,
                },
                model: "test-model".to_string(),
                finish_reason: helix_llm::providers::FinishReason::Stop,
                metadata: std::collections::HashMap::new(),
            })
        }
        async fn stream_complete(
            &self,
            _req: helix_llm::providers::LlmRequest,
        ) -> Result<Box<dyn futures::Stream<Item = Result<String, helix_llm::errors::LlmError>> + Unpin + Send>, helix_llm::errors::LlmError> {
            unimplemented!()
        }
        async fn health_check(&self) -> Result<(), helix_llm::errors::LlmError> {
            Ok(())
        }
    }

    let ctx = StubContext;
    let provider = StubLlmProvider;
    let response = loop_runner.run_cycle(&ctx, &provider).await.unwrap();

    assert_eq!(response.proposals.len(), 2);
    assert_eq!(response.allowed_count, 2);
    assert_eq!(response.denied_count, 0);

    // Activity log should have 2 entries
    assert_eq!(loop_runner.activity_log().len().await, 2);

    // Extract allowed actions
    let actions = OperatorLoop::allowed_actions(&response);
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0].action_type(), "log_analysis");
    assert_eq!(actions[1].action_type(), "escalate_case");

    // Cycle count should be 1
    assert_eq!(loop_runner.cycle_count().await, 1);

    // Stop
    loop_runner.stop().await.unwrap();
    assert_eq!(loop_runner.status().await, OperatorStatus::Stopped);
}

#[tokio::test]
async fn observe_only_scope_denies_all() {
    let config = DeskOperatorConfig {
        enabled: true,
        autopilot_mode: AutopilotMode::Auto,
        action_scope: OperatorActionScope::ObserveOnly,
        ..Default::default()
    };
    let loop_runner = OperatorLoop::from_config(config);
    loop_runner.start().await.unwrap();

    struct StubContext;
    impl crate::context::DeskContext for StubContext {
        fn snapshot(&self, _max: usize) -> DeskContextSnapshot {
            DeskContextSnapshot {
                source_count: 0, watchlist_count: 0, evidence_count: 0,
                claim_count: 0, open_case_count: 0, escalated_case_count: 0,
                top_cases: vec![], recent_evidence: vec![], active_watchlists: vec![], recent_decisions: vec![],
            }
        }
    }

    struct StubLlmProvider;
    #[async_trait::async_trait]
    impl helix_llm::providers::LlmProvider for StubLlmProvider {
        fn name(&self) -> &str { "mock" }
        async fn get_models(&self) -> Result<Vec<helix_llm::providers::ModelConfig>, helix_llm::errors::LlmError> { Ok(vec![]) }
        async fn complete(&self, _req: helix_llm::providers::LlmRequest) -> Result<helix_llm::providers::LlmResponse, helix_llm::errors::LlmError> {
            Ok(helix_llm::providers::LlmResponse {
                content: r#"[{"type":"log_analysis","rationale":"test","parameters":{"severity":"low","summary":"ok"}}]"#.to_string(),
                function_call: None,
                usage: helix_llm::providers::TokenUsage { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 },
                model: "test".to_string(),
                finish_reason: helix_llm::providers::FinishReason::Stop,
                metadata: std::collections::HashMap::new(),
            })
        }
        async fn stream_complete(&self, _req: helix_llm::providers::LlmRequest) -> Result<Box<dyn futures::Stream<Item = Result<String, helix_llm::errors::LlmError>> + Unpin + Send>, helix_llm::errors::LlmError> { unimplemented!() }
        async fn health_check(&self) -> Result<(), helix_llm::errors::LlmError> { Ok(()) }
    }

    let result = loop_runner.run_cycle(&StubContext, &StubLlmProvider).await.unwrap();
    assert_eq!(result.denied_count, 1);
    assert_eq!(result.allowed_count, 0);
    assert_eq!(result.decisions[0].denial_reason.as_deref(), Some("out_of_scope"));
}

#[tokio::test]
async fn assist_mode_requires_confirmation() {
    let config = DeskOperatorConfig {
        enabled: true,
        autopilot_mode: AutopilotMode::Assist,
        ..Default::default()
    };
    let loop_runner = OperatorLoop::from_config(config);
    loop_runner.start().await.unwrap();

    struct StubContext;
    impl crate::context::DeskContext for StubContext {
        fn snapshot(&self, _max: usize) -> DeskContextSnapshot {
            DeskContextSnapshot {
                source_count: 0, watchlist_count: 0, evidence_count: 0,
                claim_count: 0, open_case_count: 0, escalated_case_count: 0,
                top_cases: vec![], recent_evidence: vec![], active_watchlists: vec![], recent_decisions: vec![],
            }
        }
    }

    struct StubLlmProvider;
    #[async_trait::async_trait]
    impl helix_llm::providers::LlmProvider for StubLlmProvider {
        fn name(&self) -> &str { "mock" }
        async fn get_models(&self) -> Result<Vec<helix_llm::providers::ModelConfig>, helix_llm::errors::LlmError> { Ok(vec![]) }
        async fn complete(&self, _req: helix_llm::providers::LlmRequest) -> Result<helix_llm::providers::LlmResponse, helix_llm::errors::LlmError> {
            Ok(helix_llm::providers::LlmResponse {
                content: r#"[{"type":"log_analysis","rationale":"test","parameters":{"severity":"low","summary":"ok"}}]"#.to_string(),
                function_call: None,
                usage: helix_llm::providers::TokenUsage { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 },
                model: "test".to_string(),
                finish_reason: helix_llm::providers::FinishReason::Stop,
                metadata: std::collections::HashMap::new(),
            })
        }
        async fn stream_complete(&self, _req: helix_llm::providers::LlmRequest) -> Result<Box<dyn futures::Stream<Item = Result<String, helix_llm::errors::LlmError>> + Unpin + Send>, helix_llm::errors::LlmError> { unimplemented!() }
        async fn health_check(&self) -> Result<(), helix_llm::errors::LlmError> { Ok(()) }
    }

    let result = loop_runner.run_cycle(&StubContext, &StubLlmProvider).await.unwrap();
    // In assist mode without human confirmation, proposals are allowed with
    // requires_confirmation=true and enqueued to the confirmation queue.
    // They are not "denied" — they're pending human review.
    assert_eq!(result.denied_count, 0);
    assert_eq!(result.allowed_count, 0);
    assert_eq!(result.proposals.len(), 1);
    // The proposal should be pending confirmation
    assert!(result.decisions.iter().all(|d| d.requires_confirmation));
}

#[test]
fn parse_proposals_with_fenced_json() {
    let raw = "```json\n[{\"type\":\"log_analysis\",\"rationale\":\"test\"}]\n```";
    let proposals = parse_operator_proposals(raw).unwrap();
    assert_eq!(proposals.len(), 1);
}

#[test]
fn parse_proposals_empty_array() {
    let proposals = parse_operator_proposals("[]").unwrap();
    assert!(proposals.is_empty());
}

#[test]
fn proposal_to_action_review_claim() {
    let proposal = OperatorProposal {
        action_type: "review_claim".to_string(),
        target_id: Some("claim-1".to_string()),
        rationale: "Evidence corroborates".to_string(),
        parameters: serde_json::json!({"status": "corroborated"}),
    };
    let action = proposal_to_action(&proposal).unwrap();
    match action {
        OperatorAction::ReviewClaim { claim_id, status, .. } => {
            assert_eq!(claim_id, "claim-1");
            assert_eq!(status, "corroborated");
        }
        _ => panic!("wrong action"),
    }
}

#[test]
fn proposal_to_action_open_case() {
    let proposal = OperatorProposal {
        action_type: "open_case".to_string(),
        target_id: Some("ev-1".to_string()),
        rationale: "New investigation".to_string(),
        parameters: serde_json::json!({"title": "New Case", "watchlist_id": "wl-1"}),
    };
    let action = proposal_to_action(&proposal).unwrap();
    match action {
        OperatorAction::OpenCase { evidence_id, title, watchlist_id, .. } => {
            assert_eq!(evidence_id, "ev-1");
            assert_eq!(title, "New Case");
            assert_eq!(watchlist_id, "wl-1");
        }
        _ => panic!("wrong action"),
    }
}

#[tokio::test]
async fn pause_and_resume() {
    let loop_runner = OperatorLoop::new(
        DeskOperatorConfig::default(),
        AutopilotGuardMachine::default(),
    );
    loop_runner.start().await.unwrap();
    loop_runner.pause().await.unwrap();
    assert_eq!(loop_runner.status().await, OperatorStatus::Paused);
    loop_runner.start().await.unwrap();
    assert_eq!(loop_runner.status().await, OperatorStatus::Running);
}
