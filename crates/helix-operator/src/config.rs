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

//! Desk operator configuration: user-defined rules for autonomous operation.

use crate::errors::OperatorError;
use helix_core::autopilot_guard::AutopilotMode;
use serde::{Deserialize, Serialize};

/// Minimum loop interval in seconds (safety floor).
pub const MIN_LOOP_INTERVAL_SECS: u64 = 5;
/// Maximum loop interval in seconds.
pub const MAX_LOOP_INTERVAL_SECS: u64 = 3600;
/// Maximum actions the operator can propose per loop cycle.
pub const MAX_ACTIONS_PER_CYCLE: usize = 10;
/// Maximum context items (cases/evidence) to include in the LLM prompt.
pub const MAX_CONTEXT_ITEMS: usize = 20;
/// Maximum operator rules text length.
pub const MAX_RULES_TEXT_LEN: usize = 4096;

/// The scope of actions the operator is permitted to propose.
///
/// This is a secondary gate on top of the [`AutopilotGuardMachine`]. Even if
/// the guard would allow an action, the operator will not propose actions
/// outside its configured scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorActionScope {
    /// Observe only — the operator can read the desk state and log analysis
    /// but cannot propose any actions. Useful for monitoring mode.
    ObserveOnly,
    /// Intelligence actions — ingest evidence, review claims, escalate cases,
    /// dispatch to federation peers. No policy or on-chain actions.
    Intelligence,
    /// Policy actions — includes intelligence actions plus policy simulations.
    /// No on-chain actions.
    Policy,
    /// Full authority — all action types permitted by the guard.
    /// On-chain actions still require guard confirmation if configured.
    Full,
}

impl Default for OperatorActionScope {
    fn default() -> Self {
        Self::Intelligence
    }
}

impl OperatorActionScope {
    /// Whether this scope permits the given action type string.
    pub fn permits(&self, action_type: &str) -> bool {
        match self {
            Self::ObserveOnly => false,
            Self::Intelligence => {
                matches!(
                    action_type,
                    "ingest_evidence"
                        | "review_claim"
                        | "escalate_case"
                        | "open_case"
                        | "federation_broadcast"
                        | "log_analysis"
                )
            }
            Self::Policy => matches!(
                action_type,
                "ingest_evidence"
                    | "review_claim"
                    | "escalate_case"
                    | "open_case"
                    | "federation_broadcast"
                    | "log_analysis"
                    | "policy_simulation"
            ),
            Self::Full => true,
        }
    }
}

/// The current run state of the operator loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorStatus {
    /// The operator is stopped.
    Stopped,
    /// The operator is running its observe-think-act loop.
    Running,
    /// The operator is paused (will resume when set to Running).
    Paused,
}

impl Default for OperatorStatus {
    fn default() -> Self {
        Self::Stopped
    }
}

/// User-defined configuration for the desk operator.
///
/// This is the "set of rules" the user gives the LLM. The operator loop
/// enforces these rules deterministically — the LLM never sees a config
/// it can override. Every proposed action is checked against both the
/// [`OperatorActionScope`] and the [`AutopilotGuardMachine`] before
/// execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeskOperatorConfig {
    /// Whether the operator loop is enabled.
    pub enabled: bool,
    /// The autopilot mode the operator runs under. Must be `Assist` or `Auto`.
    /// If set to `Off`, the operator will observe but never act.
    pub autopilot_mode: AutopilotMode,
    /// The scope of actions the operator is permitted to propose.
    pub action_scope: OperatorActionScope,
    /// Loop interval in seconds.
    pub loop_interval_secs: u64,
    /// Maximum actions the operator can propose per cycle.
    pub max_actions_per_cycle: usize,
    /// Maximum context items to include in the LLM prompt.
    pub max_context_items: usize,
    /// The LLM model to use (e.g. "gpt-4o-mini").
    pub model: String,
    /// User-defined rules text — natural language instructions the operator
    /// must follow. This becomes part of the system prompt.
    pub rules_text: String,
    /// Whether to dispatch operator decisions to federation peers.
    pub dispatch_to_peers: bool,
    /// Whether to log denied proposals for audit.
    pub log_denied_proposals: bool,
}

impl Default for DeskOperatorConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            autopilot_mode: AutopilotMode::Assist,
            action_scope: OperatorActionScope::Intelligence,
            loop_interval_secs: 60,
            max_actions_per_cycle: 3,
            max_context_items: 10,
            model: "gpt-4o-mini".to_string(),
            rules_text: String::new(),
            dispatch_to_peers: false,
            log_denied_proposals: true,
        }
    }
}

impl DeskOperatorConfig {
    /// Validates this configuration.
    pub fn validate(&self) -> Result<(), OperatorError> {
        if self.loop_interval_secs < MIN_LOOP_INTERVAL_SECS
            || self.loop_interval_secs > MAX_LOOP_INTERVAL_SECS
        {
            return Err(OperatorError::ConfigValidation(format!(
                "loop_interval_secs must be between {MIN_LOOP_INTERVAL_SECS} and {MAX_LOOP_INTERVAL_SECS}"
            )));
        }
        if self.max_actions_per_cycle == 0 || self.max_actions_per_cycle > MAX_ACTIONS_PER_CYCLE {
            return Err(OperatorError::ConfigValidation(format!(
                "max_actions_per_cycle must be between 1 and {MAX_ACTIONS_PER_CYCLE}"
            )));
        }
        if self.max_context_items == 0 || self.max_context_items > MAX_CONTEXT_ITEMS {
            return Err(OperatorError::ConfigValidation(format!(
                "max_context_items must be between 1 and {MAX_CONTEXT_ITEMS}"
            )));
        }
        if self.rules_text.len() > MAX_RULES_TEXT_LEN {
            return Err(OperatorError::ConfigValidation(format!(
                "rules_text must be at most {MAX_RULES_TEXT_LEN} characters"
            )));
        }
        if self.model.trim().is_empty() {
            return Err(OperatorError::ConfigValidation("model must not be empty".to_string()));
        }
        Ok(())
    }

    /// Builds the system prompt for the LLM operator.
    pub fn system_prompt(&self) -> String {
        let scope_desc = match self.action_scope {
            OperatorActionScope::ObserveOnly => "You may only observe and log analysis. You cannot propose actions.",
            OperatorActionScope::Intelligence => "You may propose: ingest_evidence, review_claim, escalate_case, open_case, federation_broadcast, log_analysis.",
            OperatorActionScope::Policy => "You may propose intelligence actions plus policy_simulation.",
            OperatorActionScope::Full => "You may propose any action type. On-chain actions still require guard confirmation.",
        };

        let mode_desc = match self.autopilot_mode {
            AutopilotMode::Off => "Autopilot is OFF. You can observe but all actions will be denied.",
            AutopilotMode::Assist => "Autopilot is in ASSIST mode. Your proposals will be presented for human confirmation.",
            AutopilotMode::Auto => "Autopilot is in AUTO mode. Your proposals will be executed automatically within guardrails.",
        };

        format!(
            concat!(
                "You are a Helix Intelligence Desk Operator.\n",
                "You operate an intelligence desk autonomously, observing the desk state\n",
                "and proposing actions based on the rules below.\n",
                "\n",
                "## Operating Mode\n",
                "{mode_desc}\n",
                "\n",
                "## Action Scope\n",
                "{scope_desc}\n",
                "\n",
                "## Constraints\n",
                "- You may propose at most {max_actions} actions per cycle.\n",
                "- Every proposal is checked by a deterministic guard before execution.\n",
                "- The guard can deny any proposal. Denied proposals are logged.\n",
                "- You never bypass the guard. You propose; the guard decides.\n",
                "\n",
                "## Output Format\n",
                "Return ONLY a JSON array of action objects. No prose. No markdown.\n",
                "Each action object has:\n",
                "  {{\"type\": \"<action_type>\", \"target_id\": \"<optional id>\", \"rationale\": \"<why>\", \"parameters\": {{...}}}}\n",
                "\n",
                "Action types:\n",
                "- log_analysis: {{\"type\": \"log_analysis\", \"rationale\": \"<your analysis>\", \"parameters\": {{\"severity\": \"low|medium|high|critical\", \"summary\": \"<summary>\"}}}}\n",
                "- escalate_case: {{\"type\": \"escalate_case\", \"target_id\": \"<case_id>\", \"rationale\": \"<why>\"}}\n",
                "- open_case: {{\"type\": \"open_case\", \"target_id\": \"<evidence_id>\", \"rationale\": \"<why>\", \"parameters\": {{\"title\": \"<title>\", \"watchlist_id\": \"<id>\"}}}}\n",
                "- review_claim: {{\"type\": \"review_claim\", \"target_id\": \"<claim_id>\", \"rationale\": \"<why>\", \"parameters\": {{\"status\": \"corroborated|rejected\"}}}}\n",
                "- federation_broadcast: {{\"type\": \"federation_broadcast\", \"rationale\": \"<why>\", \"parameters\": {{\"title\": \"<title>\", \"summary\": \"<summary>\", \"content\": \"<content>\"}}}}\n",
                "- policy_simulation: {{\"type\": \"policy_simulation\", \"rationale\": \"<why>\", \"parameters\": {{\"commands\": [<policy_command>]}}}}\n",
                "\n",
                "## User-Defined Rules\n",
                "{rules}\n"
            ),
            mode_desc = mode_desc,
            scope_desc = scope_desc,
            max_actions = self.max_actions_per_cycle,
            rules = if self.rules_text.trim().is_empty() {
                "(none — use your best judgment)"
            } else {
                &self.rules_text
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_validates() {
        assert!(DeskOperatorConfig::default().validate().is_ok());
    }

    #[test]
    fn config_rejects_too_fast_loop() {
        let mut config = DeskOperatorConfig::default();
        config.loop_interval_secs = 1;
        assert!(config.validate().is_err());
    }

    #[test]
    fn config_rejects_too_many_actions() {
        let mut config = DeskOperatorConfig::default();
        config.max_actions_per_cycle = 100;
        assert!(config.validate().is_err());
    }

    #[test]
    fn config_rejects_empty_model() {
        let mut config = DeskOperatorConfig::default();
        config.model = "  ".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn system_prompt_includes_rules() {
        let mut config = DeskOperatorConfig::default();
        config.rules_text = "Always prioritize cases about entity X.".to_string();
        let prompt = config.system_prompt();
        assert!(prompt.contains("Always prioritize cases about entity X."));
        assert!(prompt.contains("Helix Intelligence Desk Operator"));
    }

    #[test]
    fn system_prompt_handles_empty_rules() {
        let config = DeskOperatorConfig::default();
        let prompt = config.system_prompt();
        assert!(prompt.contains("(none — use your best judgment)"));
    }

    #[test]
    fn action_scope_observe_only_denies_all() {
        let scope = OperatorActionScope::ObserveOnly;
        assert!(!scope.permits("ingest_evidence"));
        assert!(!scope.permits("escalate_case"));
        assert!(!scope.permits("policy_simulation"));
    }

    #[test]
    fn action_scope_intelligence_allows_intel_actions() {
        let scope = OperatorActionScope::Intelligence;
        assert!(scope.permits("escalate_case"));
        assert!(scope.permits("review_claim"));
        assert!(scope.permits("federation_broadcast"));
        assert!(!scope.permits("policy_simulation"));
    }

    #[test]
    fn action_scope_full_allows_everything() {
        let scope = OperatorActionScope::Full;
        assert!(scope.permits("policy_simulation"));
        assert!(scope.permits("escalate_case"));
    }
}
