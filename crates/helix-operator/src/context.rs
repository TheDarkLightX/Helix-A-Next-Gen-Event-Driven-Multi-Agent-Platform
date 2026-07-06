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

//! Desk context: structured snapshot of the desk state fed to the LLM.

use serde::{Deserialize, Serialize};

/// A summary of a case for the LLM context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeskContextSummary {
    /// Case id.
    pub id: String,
    /// Case title.
    pub title: String,
    /// Case status.
    pub status: String,
    /// Watchlist id.
    pub watchlist_id: String,
    /// Primary entity (if any).
    pub primary_entity: Option<String>,
    /// Number of evidence items.
    pub evidence_count: usize,
    /// Number of claims.
    pub claim_count: usize,
    /// Latest reason / briefing.
    pub latest_reason: String,
    /// Priority total (higher = more urgent).
    pub priority_total: u64,
}

/// A structured snapshot of the desk state for the LLM operator.
///
/// This is what the LLM "sees" when it thinks. It contains:
/// - Top cases by priority
/// - Recent evidence
/// - Active watchlists
/// - Desk overview stats
/// - Recent operator decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeskContextSnapshot {
    /// Desk overview stats.
    pub source_count: usize,
    /// Number of watchlists.
    pub watchlist_count: usize,
    /// Total evidence.
    pub evidence_count: usize,
    /// Total claims.
    pub claim_count: usize,
    /// Open cases.
    pub open_case_count: usize,
    /// Escalated cases.
    pub escalated_case_count: usize,
    /// Top cases by priority (limited to max_context_items).
    pub top_cases: Vec<DeskContextSummary>,
    /// Recent evidence titles (limited to max_context_items).
    pub recent_evidence: Vec<EvidenceSummary>,
    /// Active watchlist names.
    pub active_watchlists: Vec<WatchlistSummary>,
    /// Recent operator decisions (last N).
    pub recent_decisions: Vec<RecentDecisionSummary>,
}

/// Summary of an evidence item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceSummary {
    /// Evidence id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Source id.
    pub source_id: String,
    /// Trust score of the source.
    pub trust_score: u8,
    /// Observed at timestamp.
    pub observed_at: String,
    /// Entity labels.
    pub entity_labels: Vec<String>,
    /// Tags.
    pub tags: Vec<String>,
}

/// Summary of a watchlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchlistSummary {
    /// Watchlist id.
    pub id: String,
    /// Name.
    pub name: String,
    /// Severity.
    pub severity: String,
    /// Keywords.
    pub keywords: Vec<String>,
    /// Whether it's enabled.
    pub enabled: bool,
}

/// Summary of a recent operator decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentDecisionSummary {
    /// Action type proposed.
    pub action_type: String,
    /// Whether it was allowed or denied.
    pub decision: String,
    /// Denial reason (if denied).
    pub denial_reason: Option<String>,
    /// Rationale given by the LLM.
    pub rationale: String,
    /// Timestamp.
    pub timestamp: String,
}

/// Builder for [`DeskContextSnapshot`].
///
/// This trait abstracts over the data source so the operator can work with
/// any backend that can provide desk state.
pub trait DeskContext: Send + Sync {
    /// Assembles a context snapshot with at most `max_items` items per category.
    fn snapshot(&self, max_items: usize) -> DeskContextSnapshot;
}

/// Formats a [`DeskContextSnapshot`] as a JSON string for the LLM user prompt.
pub fn format_context_for_llm(snapshot: &DeskContextSnapshot) -> String {
    serde_json::to_string_pretty(snapshot).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_context_produces_valid_json() {
        let snapshot = DeskContextSnapshot {
            source_count: 5,
            watchlist_count: 3,
            evidence_count: 42,
            claim_count: 15,
            open_case_count: 4,
            escalated_case_count: 1,
            top_cases: vec![DeskContextSummary {
                id: "case-1".to_string(),
                title: "Test Case".to_string(),
                status: "open".to_string(),
                watchlist_id: "wl-1".to_string(),
                primary_entity: Some("entity-x".to_string()),
                evidence_count: 3,
                claim_count: 2,
                latest_reason: "New evidence detected".to_string(),
                priority_total: 850,
            }],
            recent_evidence: vec![],
            active_watchlists: vec![],
            recent_decisions: vec![],
        };
        let json = format_context_for_llm(&snapshot);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["source_count"], 5);
        assert_eq!(parsed["top_cases"][0]["id"], "case-1");
    }
}
