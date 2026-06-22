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

//! The operator loop: observe-think-act cycle with deterministic guardrails.

use crate::collaboration::CollaborationBroadcaster;
use crate::config::{DeskOperatorConfig, OperatorStatus};
use crate::confirmation::{ConfirmationQueue, ConfirmationRequest};
use crate::context::{format_context_for_llm, DeskContext};
use crate::errors::OperatorError;
use crate::proposals::{
    parse_operator_proposals, proposal_to_action, OperatorAction, OperatorDecision,
    OperatorProposal, OperatorProposalResponse,
};
use helix_core::autopilot_guard::{
    AutopilotActionClass, AutopilotGuardDecision, AutopilotGuardMachine,
};
use helix_llm::providers::{LlmProvider, LlmRequest, Message, MessageRole};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A single entry in the operator activity log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperatorActivityEntry {
    /// Unique entry id.
    pub id: String,
    /// Cycle number.
    pub cycle: u64,
    /// The action type proposed.
    pub action_type: String,
    /// The LLM rationale.
    pub rationale: String,
    /// Whether the guard allowed it.
    pub allowed: bool,
    /// Denial reason if denied.
    pub denial_reason: Option<String>,
    /// Whether human confirmation is required.
    pub requires_confirmation: bool,
    /// ISO-8601 timestamp.
    pub timestamp: String,
}

/// Bounded, thread-safe activity log.
#[derive(Debug, Clone)]
pub struct OperatorActivityLog {
    inner: Arc<RwLock<VecDeque<OperatorActivityEntry>>>,
    capacity: usize,
}

impl OperatorActivityLog {
    /// Creates a new activity log with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(VecDeque::with_capacity(capacity))),
            capacity,
        }
    }

    /// Appends an entry, evicting oldest if at capacity.
    pub async fn append(&self, entry: OperatorActivityEntry) {
        let mut log = self.inner.write().await;
        if log.len() >= self.capacity {
            log.pop_front();
        }
        log.push_back(entry);
    }

    /// Returns the most recent `n` entries (newest first).
    pub async fn recent(&self, n: usize) -> Vec<OperatorActivityEntry> {
        let log = self.inner.read().await;
        log.iter().rev().take(n).cloned().collect()
    }

    /// Returns the total count.
    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }

    /// Returns true if empty.
    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.is_empty()
    }
}

/// The operator loop state.
#[derive(Debug)]
pub struct OperatorLoop {
    config: Arc<RwLock<DeskOperatorConfig>>,
    status: Arc<RwLock<OperatorStatus>>,
    guard: Arc<RwLock<AutopilotGuardMachine>>,
    activity_log: OperatorActivityLog,
    cycle_count: Arc<RwLock<u64>>,
    confirmation_queue: ConfirmationQueue,
    broadcaster: CollaborationBroadcaster,
}

impl OperatorLoop {
    /// Creates a new operator loop with the given config and guard.
    pub fn new(
        config: DeskOperatorConfig,
        guard: AutopilotGuardMachine,
    ) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            status: Arc::new(RwLock::new(OperatorStatus::Stopped)),
            guard: Arc::new(RwLock::new(guard)),
            activity_log: OperatorActivityLog::new(200),
            cycle_count: Arc::new(RwLock::new(0)),
            confirmation_queue: ConfirmationQueue::new(50),
            broadcaster: CollaborationBroadcaster::default(),
        }
    }

    /// Creates a new operator loop, deriving the guard from the config.
    ///
    /// The guard's autopilot mode and on-chain flags are synchronized with the
    /// config. Use [`OperatorLoop::new`] if you need to supply a custom guard
    /// (e.g. one restored from a snapshot).
    pub fn from_config(config: DeskOperatorConfig) -> Self {
        let allow_onchain = matches!(
            config.action_scope,
            crate::config::OperatorActionScope::Full
        );
        let guard_config = helix_core::autopilot_guard::AutopilotGuardConfig {
            mode: config.autopilot_mode,
            allow_onchain,
            require_onchain_confirmation: true,
            require_onchain_dry_run: true,
            max_policy_commands: 128,
        };
        Self::new(config, AutopilotGuardMachine::new(guard_config))
    }

    /// Returns the current config.
    pub async fn config(&self) -> DeskOperatorConfig {
        self.config.read().await.clone()
    }

    /// Updates the config.
    pub async fn set_config(&self, config: DeskOperatorConfig) -> Result<(), OperatorError> {
        config.validate()?;
        // Sync the guard mode
        let mut guard = self.guard.write().await;
        guard.step(helix_core::autopilot_guard::AutopilotGuardInput::SetConfig {
            config: helix_core::autopilot_guard::AutopilotGuardConfig {
                mode: config.autopilot_mode,
                allow_onchain: matches!(config.action_scope, crate::config::OperatorActionScope::Full),
                require_onchain_confirmation: true,
                require_onchain_dry_run: true,
                max_policy_commands: 128,
            },
        });
        *self.config.write().await = config;
        drop(guard);
        self.broadcaster
            .broadcast(crate::collaboration::CollaborationEvent::config_changed());
        Ok(())
    }

    /// Returns the current status.
    pub async fn status(&self) -> OperatorStatus {
        *self.status.read().await
    }

    /// Starts the operator loop.
    pub async fn start(&self) -> Result<(), OperatorError> {
        let mut status = self.status.write().await;
        if *status == OperatorStatus::Running {
            return Err(OperatorError::AlreadyRunning);
        }
        *status = OperatorStatus::Running;
        drop(status);
        self.broadcaster
            .broadcast(crate::collaboration::CollaborationEvent::loop_started());
        Ok(())
    }

    /// Stops the operator loop.
    pub async fn stop(&self) -> Result<(), OperatorError> {
        let mut status = self.status.write().await;
        *status = OperatorStatus::Stopped;
        drop(status);
        self.broadcaster
            .broadcast(crate::collaboration::CollaborationEvent::loop_stopped());
        Ok(())
    }

    /// Pauses the operator loop.
    pub async fn pause(&self) -> Result<(), OperatorError> {
        let mut status = self.status.write().await;
        *status = OperatorStatus::Paused;
        drop(status);
        self.broadcaster
            .broadcast(crate::collaboration::CollaborationEvent::loop_paused());
        Ok(())
    }

    /// Returns the current cycle count.
    pub async fn cycle_count(&self) -> u64 {
        *self.cycle_count.read().await
    }

    /// Returns a reference to the activity log.
    pub fn activity_log(&self) -> &OperatorActivityLog {
        &self.activity_log
    }

    /// Returns a reference to the confirmation queue.
    pub fn confirmation_queue(&self) -> &ConfirmationQueue {
        &self.confirmation_queue
    }

    /// Returns a reference to the collaboration broadcaster.
    pub fn broadcaster(&self) -> &CollaborationBroadcaster {
        &self.broadcaster
    }

    /// Runs one observe-think-act cycle.
    ///
    /// This is the core of the operator. It:
    /// 1. Reads the desk context
    /// 2. Sends it to the LLM with the system prompt
    /// 3. Parses the LLM response into proposals
    /// 4. Evaluates each proposal through the guard
    /// 5. Returns the proposals and decisions
    ///
    /// The caller is responsible for executing allowed actions.
    pub async fn run_cycle(
        &self,
        context: &dyn DeskContext,
        provider: &dyn LlmProvider,
    ) -> Result<OperatorProposalResponse, OperatorError> {
        let config = self.config.read().await.clone();

        // Check status
        {
            let status = self.status.read().await;
            if *status != OperatorStatus::Running {
                return Err(OperatorError::NotRunning);
            }
        }

        // Increment cycle
        let cycle = {
            let mut count = self.cycle_count.write().await;
            *count += 1;
            *count
        };

        // 1. Observe: build desk context
        let snapshot = context.snapshot(config.max_context_items);
        let user_prompt = format_context_for_llm(&snapshot);

        // 2. Think: call LLM
        let system_prompt = config.system_prompt();
        let mut parameters = std::collections::HashMap::new();
        parameters.insert("model".to_string(), serde_json::Value::String(config.model.clone()));

        let llm_request = LlmRequest {
            system_prompt: Some(system_prompt),
            messages: vec![Message {
                role: MessageRole::User,
                content: user_prompt,
                function_call: None,
            }],
            max_tokens: Some(1024),
            temperature: Some(0.0),
            top_p: Some(1.0),
            functions: None,
            parameters,
        };

        let llm_response = provider
            .complete(llm_request)
            .await
            .map_err(|e| OperatorError::LlmError(e.to_string()))?;

        // 3. Parse proposals
        let proposals = parse_operator_proposals(&llm_response.content)?;

        // Limit to max_actions_per_cycle
        let proposals: Vec<OperatorProposal> = proposals
            .into_iter()
            .take(config.max_actions_per_cycle)
            .collect();

        // 4. Evaluate each proposal through scope + guard
        let mut decisions = Vec::new();
        let mut allowed_count = 0;
        let mut denied_count = 0;
        let mut pending_confirmation_count = 0;

        for proposal in &proposals {
            // Scope check (first gate)
            if !config.action_scope.permits(&proposal.action_type) {
                let decision = OperatorDecision {
                    proposal: proposal.clone(),
                    allowed: false,
                    denial_reason: Some("out_of_scope".to_string()),
                    requires_confirmation: false,
                };
                if config.log_denied_proposals {
                    self.log_activity(cycle, &decision).await;
                }
                self.broadcaster.broadcast(
                    crate::collaboration::CollaborationEvent::operator_decision(cycle, decision.clone()),
                );
                decisions.push(decision);
                denied_count += 1;
                continue;
            }

            // Guard check (second gate — deterministic)
            let action_class = classify_proposal(proposal);
            let guard_decision = {
                let mut guard = self.guard.write().await;
                guard.step(helix_core::autopilot_guard::AutopilotGuardInput::Evaluate {
                    action: action_class,
                    confirmed_by_human: false, // LLM proposals are never "human confirmed"
                })
            };

            let decision = match guard_decision {
                AutopilotGuardDecision::Allow {
                    requires_confirmation,
                } => {
                    if requires_confirmation {
                        // CoPilot assist mode: enqueue for human review
                        let conf_req = ConfirmationRequest::new(cycle, proposal.clone());
                        self.confirmation_queue.enqueue(conf_req.clone()).await;
                        pending_confirmation_count += 1;
                        self.broadcaster.broadcast(
                            crate::collaboration::CollaborationEvent::confirmation_requested(conf_req),
                        );
                        // Not allowed yet — pending human confirmation
                        OperatorDecision {
                            proposal: proposal.clone(),
                            allowed: false,
                            denial_reason: Some("pending_confirmation".to_string()),
                            requires_confirmation: true,
                        }
                    } else {
                        allowed_count += 1;
                        OperatorDecision {
                            proposal: proposal.clone(),
                            allowed: true,
                            denial_reason: None,
                            requires_confirmation: false,
                        }
                    }
                }
                AutopilotGuardDecision::Deny { reason } => {
                    denied_count += 1;
                    OperatorDecision {
                        proposal: proposal.clone(),
                        allowed: false,
                        denial_reason: Some(reason),
                        requires_confirmation: false,
                    }
                }
                AutopilotGuardDecision::ConfigUpdated => {
                    denied_count += 1;
                    OperatorDecision {
                        proposal: proposal.clone(),
                        allowed: false,
                        denial_reason: Some("config_updated".to_string()),
                        requires_confirmation: false,
                    }
                }
            };

            // Log all activities (both allowed and denied)
            self.log_activity(cycle, &decision).await;

            // Broadcast the decision to all CoPilot participants
            self.broadcaster.broadcast(
                crate::collaboration::CollaborationEvent::operator_decision(cycle, decision.clone()),
            );

            decisions.push(decision);
        }

        // Broadcast cycle completion
        self.broadcaster.broadcast(
            crate::collaboration::CollaborationEvent::cycle_completed(
                cycle,
                allowed_count,
                denied_count,
                pending_confirmation_count,
            ),
        );

        Ok(OperatorProposalResponse {
            model: llm_response.model,
            raw_response: llm_response.content,
            proposals,
            decisions,
            allowed_count,
            denied_count,
            timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Extracts allowed actions from a proposal response.
    pub fn allowed_actions(response: &OperatorProposalResponse) -> Vec<OperatorAction> {
        response
            .decisions
            .iter()
            .filter(|d| d.allowed)
            .filter_map(|d| proposal_to_action(&d.proposal))
            .collect()
    }

    async fn log_activity(&self, cycle: u64, decision: &OperatorDecision) {
        let entry = OperatorActivityEntry {
            id: format!("op-{}", uuid::Uuid::new_v4()),
            cycle,
            action_type: decision.proposal.action_type.clone(),
            rationale: decision.proposal.rationale.clone(),
            allowed: decision.allowed,
            denial_reason: decision.denial_reason.clone(),
            requires_confirmation: decision.requires_confirmation,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        self.activity_log.append(entry).await;
    }
}

/// Classifies an operator proposal into an autopilot guard action class.
fn classify_proposal(proposal: &OperatorProposal) -> AutopilotActionClass {
    match proposal.action_type.as_str() {
        "policy_simulation" => {
            let command_count = proposal
                .parameters
                .get("commands")
                .and_then(|v| v.as_array())
                .map(|arr| arr.len().min(usize::from(u16::MAX)) as u16)
                .unwrap_or(0);
            AutopilotActionClass::PolicySimulation { command_count }
        }
        _ => {
            // All non-policy actions are treated as policy simulations with
            // 1 command for guard purposes — they're intelligence actions
            // that don't touch on-chain, so the guard's on-chain checks
            // don't apply. The scope check already filtered them.
            AutopilotActionClass::PolicySimulation { command_count: 1 }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::DeskContextSnapshot;
    use helix_core::autopilot_guard::AutopilotMode;

    struct StubContext;
    impl DeskContext for StubContext {
        fn snapshot(&self, _max_items: usize) -> DeskContextSnapshot {
            DeskContextSnapshot {
                source_count: 1,
                watchlist_count: 1,
                evidence_count: 1,
                claim_count: 0,
                open_case_count: 1,
                escalated_case_count: 0,
                top_cases: vec![],
                recent_evidence: vec![],
                active_watchlists: vec![],
                recent_decisions: vec![],
            }
        }
    }

    struct StubLlmProvider;
    #[async_trait::async_trait]
    impl LlmProvider for StubLlmProvider {
        fn name(&self) -> &str { "mock" }
        async fn get_models(&self) -> Result<Vec<helix_llm::providers::ModelConfig>, helix_llm::errors::LlmError> {
            Ok(vec![])
        }
        async fn complete(
            &self,
            _request: LlmRequest,
        ) -> Result<helix_llm::providers::LlmResponse, helix_llm::errors::LlmError> {
            Ok(helix_llm::providers::LlmResponse {
                content: r#"[{"type":"log_analysis","rationale":"test","parameters":{"severity":"low","summary":"ok"}}]"#
                    .to_string(),
                function_call: None,
                usage: helix_llm::providers::TokenUsage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                },
                model: "mock".to_string(),
                finish_reason: helix_llm::providers::FinishReason::Stop,
                metadata: std::collections::HashMap::new(),
            })
        }
        async fn stream_complete(
            &self,
            _request: LlmRequest,
        ) -> Result<Box<dyn futures::Stream<Item = Result<String, helix_llm::errors::LlmError>> + Unpin + Send>, helix_llm::errors::LlmError> {
            unimplemented!()
        }
        async fn health_check(&self) -> Result<(), helix_llm::errors::LlmError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn loop_starts_and_stops() {
        let loop_runner = OperatorLoop::new(
            DeskOperatorConfig::default(),
            AutopilotGuardMachine::default(),
        );
        assert_eq!(loop_runner.status().await, OperatorStatus::Stopped);
        loop_runner.start().await.unwrap();
        assert_eq!(loop_runner.status().await, OperatorStatus::Running);
        loop_runner.stop().await.unwrap();
        assert_eq!(loop_runner.status().await, OperatorStatus::Stopped);
    }

    #[tokio::test]
    async fn loop_double_start_fails() {
        let loop_runner = OperatorLoop::new(
            DeskOperatorConfig::default(),
            AutopilotGuardMachine::default(),
        );
        loop_runner.start().await.unwrap();
        assert!(loop_runner.start().await.is_err());
    }

    #[tokio::test]
    async fn run_cycle_when_stopped_fails() {
        let loop_runner = OperatorLoop::new(
            DeskOperatorConfig::default(),
            AutopilotGuardMachine::default(),
        );
        let ctx = StubContext;
        let provider = StubLlmProvider;
        let result = loop_runner.run_cycle(&ctx, &provider).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn run_cycle_in_off_mode_denies_all() {
        let config = DeskOperatorConfig {
            enabled: true,
            autopilot_mode: AutopilotMode::Off,
            ..Default::default()
        };
        let loop_runner = OperatorLoop::from_config(config);
        loop_runner.start().await.unwrap();
        let ctx = StubContext;
        let provider = StubLlmProvider;
        let result = loop_runner.run_cycle(&ctx, &provider).await.unwrap();
        assert_eq!(result.denied_count, 1);
        assert_eq!(result.allowed_count, 0);
    }

    #[tokio::test]
    async fn run_cycle_in_auto_mode_allows() {
        let config = DeskOperatorConfig {
            enabled: true,
            autopilot_mode: AutopilotMode::Auto,
            action_scope: crate::config::OperatorActionScope::Intelligence,
            ..Default::default()
        };
        let loop_runner = OperatorLoop::from_config(config);
        loop_runner.start().await.unwrap();
        let ctx = StubContext;
        let provider = StubLlmProvider;
        let result = loop_runner.run_cycle(&ctx, &provider).await.unwrap();
        assert_eq!(result.proposals.len(), 1);
        assert_eq!(result.allowed_count, 1);
        assert_eq!(result.denied_count, 0);
    }

    #[tokio::test]
    async fn run_cycle_logs_activity() {
        let config = DeskOperatorConfig {
            enabled: true,
            autopilot_mode: AutopilotMode::Auto,
            ..Default::default()
        };
        let loop_runner = OperatorLoop::from_config(config);
        loop_runner.start().await.unwrap();
        let ctx = StubContext;
        let provider = StubLlmProvider;
        let _ = loop_runner.run_cycle(&ctx, &provider).await.unwrap();
        assert_eq!(loop_runner.activity_log().len().await, 1);
        let entries = loop_runner.activity_log().recent(10).await;
        assert_eq!(entries[0].action_type, "log_analysis");
        assert!(entries[0].allowed);
    }

    #[tokio::test]
    async fn run_cycle_increments_cycle_count() {
        let config = DeskOperatorConfig {
            enabled: true,
            autopilot_mode: AutopilotMode::Auto,
            ..Default::default()
        };
        let loop_runner = OperatorLoop::from_config(config);
        loop_runner.start().await.unwrap();
        let ctx = StubContext;
        let provider = StubLlmProvider;
        let _ = loop_runner.run_cycle(&ctx, &provider).await.unwrap();
        let _ = loop_runner.run_cycle(&ctx, &provider).await.unwrap();
        assert_eq!(loop_runner.cycle_count().await, 2);
    }

    #[tokio::test]
    async fn set_config_updates_guard() {
        let loop_runner = OperatorLoop::new(
            DeskOperatorConfig::default(),
            AutopilotGuardMachine::default(),
        );
        let new_config = DeskOperatorConfig {
            autopilot_mode: AutopilotMode::Auto,
            ..Default::default()
        };
        loop_runner.set_config(new_config).await.unwrap();
        let guard = loop_runner.guard.read().await;
        assert_eq!(guard.config().mode, AutopilotMode::Auto);
    }

    #[tokio::test]
    async fn activity_log_capacity_evicts() {
        let log = OperatorActivityLog::new(3);
        for i in 0..5 {
            log.append(OperatorActivityEntry {
                id: format!("e{i}"),
                cycle: i as u64,
                action_type: "log_analysis".to_string(),
                rationale: "test".to_string(),
                allowed: true,
                denial_reason: None,
                requires_confirmation: false,
                timestamp: "2026-01-01T00:00:00Z".to_string(),
            })
            .await;
        }
        assert_eq!(log.len().await, 3);
        let recent = log.recent(10).await;
        assert_eq!(recent[0].id, "e4");
    }

    #[tokio::test]
    async fn allowed_actions_extracts_allowed() {
        let response = OperatorProposalResponse {
            model: "test".to_string(),
            raw_response: "[]".to_string(),
            proposals: vec![
                OperatorProposal {
                    action_type: "log_analysis".to_string(),
                    target_id: None,
                    rationale: "test".to_string(),
                    parameters: serde_json::json!({"severity": "low", "summary": "ok"}),
                },
            ],
            decisions: vec![OperatorDecision {
                proposal: OperatorProposal {
                    action_type: "log_analysis".to_string(),
                    target_id: None,
                    rationale: "test".to_string(),
                    parameters: serde_json::json!({"severity": "low", "summary": "ok"}),
                },
                allowed: true,
                denial_reason: None,
                requires_confirmation: false,
            }],
            allowed_count: 1,
            denied_count: 0,
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        };
        let actions = OperatorLoop::allowed_actions(&response);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            OperatorAction::LogAnalysis { severity, .. } => {
                assert_eq!(severity, "low");
            }
            _ => panic!("wrong action"),
        }
    }
}
