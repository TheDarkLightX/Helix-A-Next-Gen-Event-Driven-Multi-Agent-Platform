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

//! Operator proposals: the action objects the LLM produces, and the parser
//! that converts raw LLM output into structured proposals.

use crate::errors::OperatorError;
use serde::{Deserialize, Serialize};

/// A single action proposed by the LLM operator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperatorProposal {
    /// The action type (e.g. "escalate_case", "log_analysis").
    #[serde(rename = "type")]
    pub action_type: String,
    /// Optional target id (case id, evidence id, claim id).
    #[serde(default)]
    pub target_id: Option<String>,
    /// The LLM's rationale for this proposal.
    #[serde(default)]
    pub rationale: String,
    /// Action-specific parameters.
    #[serde(default)]
    pub parameters: serde_json::Value,
}

/// The decision made by the guard for a proposal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperatorDecision {
    /// The original proposal.
    pub proposal: OperatorProposal,
    /// Whether the action was allowed.
    pub allowed: bool,
    /// Denial reason if not allowed.
    pub denial_reason: Option<String>,
    /// Whether human confirmation is required.
    pub requires_confirmation: bool,
}

/// The full response from one operator cycle: the LLM's raw output, parsed
/// proposals, and guard decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorProposalResponse {
    /// The LLM model used.
    pub model: String,
    /// Raw LLM response text.
    pub raw_response: String,
    /// Parsed proposals.
    pub proposals: Vec<OperatorProposal>,
    /// Guard decisions for each proposal.
    pub decisions: Vec<OperatorDecision>,
    /// Number of proposals allowed.
    pub allowed_count: usize,
    /// Number of proposals denied.
    pub denied_count: usize,
    /// Cycle timestamp.
    pub timestamp: String,
}

/// An action to execute, derived from an allowed proposal.
///
/// This is the output the operator loop hands to the execution layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OperatorAction {
    /// Log an analysis (no side effects).
    LogAnalysis {
        /// Severity level.
        severity: String,
        /// Analysis summary.
        summary: String,
        /// LLM rationale.
        rationale: String,
    },
    /// Escalate a case.
    EscalateCase {
        /// Case id.
        case_id: String,
        /// Rationale.
        rationale: String,
    },
    /// Open a new case from evidence.
    OpenCase {
        /// Evidence id.
        evidence_id: String,
        /// Case title.
        title: String,
        /// Watchlist id.
        watchlist_id: String,
        /// Rationale.
        rationale: String,
    },
    /// Review a claim.
    ReviewClaim {
        /// Claim id.
        claim_id: String,
        /// New review status: "corroborated" or "rejected".
        status: String,
        /// Rationale.
        rationale: String,
    },
    /// Broadcast to federation peers.
    FederationBroadcast {
        /// Broadcast title.
        title: String,
        /// Broadcast summary.
        summary: String,
        /// Broadcast content.
        content: String,
        /// Rationale.
        rationale: String,
    },
    /// Run a policy simulation.
    PolicySimulation {
        /// Policy commands.
        commands: serde_json::Value,
        /// Rationale.
        rationale: String,
    },
}

impl OperatorAction {
    /// The action type string for this action.
    pub fn action_type(&self) -> &'static str {
        match self {
            Self::LogAnalysis { .. } => "log_analysis",
            Self::EscalateCase { .. } => "escalate_case",
            Self::OpenCase { .. } => "open_case",
            Self::ReviewClaim { .. } => "review_claim",
            Self::FederationBroadcast { .. } => "federation_broadcast",
            Self::PolicySimulation { .. } => "policy_simulation",
        }
    }
}

/// Parses raw LLM output into a list of operator proposals.
///
/// The LLM is expected to return a JSON array of action objects. This parser
/// is fail-closed: if the output cannot be parsed, an error is returned.
/// Individual malformed entries are skipped (not fatal).
pub fn parse_operator_proposals(raw: &str) -> Result<Vec<OperatorProposal>, OperatorError> {
    let trimmed = raw.trim();

    // Strip markdown code fences if present
    let cleaned = if trimmed.starts_with("```") {
        let inner = trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        inner
    } else {
        trimmed
    };

    // Try parsing as a JSON array first
    let parsed: serde_json::Value = serde_json::from_str(cleaned)
        .map_err(|e| OperatorError::ParseError(format!("invalid JSON: {e}")))?;

    let array = match parsed {
        serde_json::Value::Array(arr) => arr,
        serde_json::Value::Object(_) => vec![parsed],
        _ => return Err(OperatorError::ParseError("expected JSON array or object".to_string())),
    };

    let mut proposals = Vec::new();
    for item in array {
        match serde_json::from_value::<OperatorProposal>(item.clone()) {
            Ok(proposal) => {
                if !proposal.action_type.trim().is_empty() {
                    proposals.push(proposal);
                }
            }
            Err(_) => {
                // Skip malformed entries — don't fail the entire parse
                tracing::warn!(entry = %item, "skipping malformed operator proposal entry");
            }
        }
    }

    Ok(proposals)
}

/// Converts an allowed proposal into an [`OperatorAction`].
///
/// Returns `None` for action types that don't map to a concrete action
/// (e.g. unknown types).
pub fn proposal_to_action(proposal: &OperatorProposal) -> Option<OperatorAction> {
    match proposal.action_type.as_str() {
        "log_analysis" => {
            let severity = proposal.parameters
                .get("severity")
                .and_then(|v| v.as_str())
                .unwrap_or("low")
                .to_string();
            let summary = proposal.parameters
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Some(OperatorAction::LogAnalysis {
                severity,
                summary,
                rationale: proposal.rationale.clone(),
            })
        }
        "escalate_case" => {
            let case_id = proposal.target_id.clone().unwrap_or_default();
            if case_id.is_empty() {
                return None;
            }
            Some(OperatorAction::EscalateCase {
                case_id,
                rationale: proposal.rationale.clone(),
            })
        }
        "open_case" => {
            let evidence_id = proposal.target_id.clone().unwrap_or_default();
            let title = proposal.parameters
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Operator-opened case")
                .to_string();
            let watchlist_id = proposal.parameters
                .get("watchlist_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if evidence_id.is_empty() || watchlist_id.is_empty() {
                return None;
            }
            Some(OperatorAction::OpenCase {
                evidence_id,
                title,
                watchlist_id,
                rationale: proposal.rationale.clone(),
            })
        }
        "review_claim" => {
            let claim_id = proposal.target_id.clone().unwrap_or_default();
            let status = proposal.parameters
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("corroborated")
                .to_string();
            if claim_id.is_empty() {
                return None;
            }
            Some(OperatorAction::ReviewClaim {
                claim_id,
                status,
                rationale: proposal.rationale.clone(),
            })
        }
        "federation_broadcast" => {
            let title = proposal.parameters
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Operator broadcast")
                .to_string();
            let summary = proposal.parameters
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content = proposal.parameters
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Some(OperatorAction::FederationBroadcast {
                title,
                summary,
                content,
                rationale: proposal.rationale.clone(),
            })
        }
        "policy_simulation" => {
            let commands = proposal.parameters
                .get("commands")
                .cloned()
                .unwrap_or(serde_json::Value::Array(vec![]));
            Some(OperatorAction::PolicySimulation {
                commands,
                rationale: proposal.rationale.clone(),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_array() {
        let raw = r#"[
            {"type": "escalate_case", "target_id": "case-1", "rationale": "High priority"},
            {"type": "log_analysis", "rationale": "Monitoring", "parameters": {"severity": "high", "summary": "test"}}
        ]"#;
        let proposals = parse_operator_proposals(raw).unwrap();
        assert_eq!(proposals.len(), 2);
        assert_eq!(proposals[0].action_type, "escalate_case");
        assert_eq!(proposals[1].action_type, "log_analysis");
    }

    #[test]
    fn parse_strips_markdown_fences() {
        let raw = r#"```json
        [{"type": "log_analysis", "rationale": "test"}]
        ```"#;
        let proposals = parse_operator_proposals(raw).unwrap();
        assert_eq!(proposals.len(), 1);
    }

    #[test]
    fn parse_skips_malformed_entries() {
        let raw = r#"[
            {"type": "log_analysis", "rationale": "good"},
            {"bad": "entry"},
            {"type": "escalate_case", "target_id": "case-1", "rationale": "also good"}
        ]"#;
        let proposals = parse_operator_proposals(raw).unwrap();
        assert_eq!(proposals.len(), 2);
    }

    #[test]
    fn parse_rejects_non_json() {
        let raw = "this is not json";
        assert!(parse_operator_proposals(raw).is_err());
    }

    #[test]
    fn parse_single_object_wraps_into_array() {
        let raw = r#"{"type": "log_analysis", "rationale": "single"}"#;
        let proposals = parse_operator_proposals(raw).unwrap();
        assert_eq!(proposals.len(), 1);
    }

    #[test]
    fn proposal_to_action_escalate_case() {
        let proposal = OperatorProposal {
            action_type: "escalate_case".to_string(),
            target_id: Some("case-42".to_string()),
            rationale: "Critical finding".to_string(),
            parameters: serde_json::Value::Null,
        };
        let action = proposal_to_action(&proposal).unwrap();
        match action {
            OperatorAction::EscalateCase { case_id, rationale } => {
                assert_eq!(case_id, "case-42");
                assert_eq!(rationale, "Critical finding");
            }
            _ => panic!("wrong action type"),
        }
    }

    #[test]
    fn proposal_to_action_log_analysis() {
        let proposal = OperatorProposal {
            action_type: "log_analysis".to_string(),
            target_id: None,
            rationale: "Observing".to_string(),
            parameters: serde_json::json!({"severity": "high", "summary": "test summary"}),
        };
        let action = proposal_to_action(&proposal).unwrap();
        match action {
            OperatorAction::LogAnalysis { severity, summary, .. } => {
                assert_eq!(severity, "high");
                assert_eq!(summary, "test summary");
            }
            _ => panic!("wrong action type"),
        }
    }

    #[test]
    fn proposal_to_action_escalate_without_target_returns_none() {
        let proposal = OperatorProposal {
            action_type: "escalate_case".to_string(),
            target_id: None,
            rationale: "test".to_string(),
            parameters: serde_json::Value::Null,
        };
        assert!(proposal_to_action(&proposal).is_none());
    }

    #[test]
    fn proposal_to_action_unknown_type_returns_none() {
        let proposal = OperatorProposal {
            action_type: "unknown_action".to_string(),
            target_id: None,
            rationale: "test".to_string(),
            parameters: serde_json::Value::Null,
        };
        assert!(proposal_to_action(&proposal).is_none());
    }

    #[test]
    fn proposal_to_action_federation_broadcast() {
        let proposal = OperatorProposal {
            action_type: "federation_broadcast".to_string(),
            target_id: None,
            rationale: "Share with peers".to_string(),
            parameters: serde_json::json!({
                "title": "Alert",
                "summary": "Brief",
                "content": "Details"
            }),
        };
        let action = proposal_to_action(&proposal).unwrap();
        match action {
            OperatorAction::FederationBroadcast { title, summary, content, .. } => {
                assert_eq!(title, "Alert");
                assert_eq!(summary, "Brief");
                assert_eq!(content, "Details");
            }
            _ => panic!("wrong action type"),
        }
    }
}
