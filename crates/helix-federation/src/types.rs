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

//! Federation event types and status structures.

use crate::peer::PeerRegistry;
use serde::{Deserialize, Serialize};

/// The kind of intelligence event being shared with peer desks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederationEventKind {
    /// A case was escalated to a higher severity.
    CaseEscalated,
    /// A new case was opened.
    CaseOpened,
    /// A watchlist was triggered by new evidence.
    WatchlistHit,
    /// New evidence was ingested.
    EvidenceIngested,
    /// A claim was corroborated or rejected.
    ClaimReviewed,
    /// An autopilot proposal was generated.
    AutopilotProposal,
    /// A manual broadcast (operator-initiated sharing).
    ManualBroadcast,
}

impl FederationEventKind {
    /// The CloudEvents `type` string for this event kind.
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::CaseEscalated => "helix.case.escalated",
            Self::CaseOpened => "helix.case.opened",
            Self::WatchlistHit => "helix.watchlist.hit",
            Self::EvidenceIngested => "helix.evidence.ingested",
            Self::ClaimReviewed => "helix.claim.reviewed",
            Self::AutopilotProposal => "helix.autopilot.proposal",
            Self::ManualBroadcast => "helix.federation.broadcast",
        }
    }
}

/// A CloudEvents-compatible federation event payload.
///
/// This is what gets POSTed to peer desks at `/api/v1/federation/receive`.
/// The receiving desk creates evidence with provenance pointing back to
/// `source_desk_id`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FederationEvent {
    /// CloudEvents spec version.
    pub specversion: String,
    /// Unique event id.
    pub id: String,
    /// CloudEvents type (e.g. `helix.case.escalated`).
    #[serde(rename = "type")]
    pub event_type: String,
    /// The desk id of the sending desk.
    pub source_desk_id: String,
    /// The desk id of the receiving desk, or `*` for broadcast.
    pub target_desk_id: String,
    /// Human-readable title for the event.
    pub title: String,
    /// Short summary of the intelligence.
    pub summary: String,
    /// Full content / detail payload.
    pub content: String,
    /// Optional URL pointing to the source material.
    pub url: Option<String>,
    /// ISO-8601 timestamp when the event was generated.
    pub observed_at: String,
    /// The kind of event, for routing.
    pub kind: FederationEventKind,
    /// Trust score (0–100) the sending desk assigns to this intelligence.
    pub trust_score: u8,
    /// Entity labels extracted from the event.
    #[serde(default)]
    pub entity_labels: Vec<String>,
    /// Tags for categorization.
    #[serde(default)]
    pub tags: Vec<String>,
    /// The case id this event relates to, if any.
    pub case_id: Option<String>,
    /// The watchlist id that triggered, if any.
    pub watchlist_id: Option<String>,
}

impl FederationEvent {
    /// Creates a new federation event with the given fields.
    pub fn new(
        source_desk_id: &str,
        kind: FederationEventKind,
        title: &str,
        summary: &str,
        content: &str,
        trust_score: u8,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            specversion: "1.0".to_string(),
            id: format!("fed-{}", uuid::Uuid::new_v4()),
            event_type: kind.event_type().to_string(),
            source_desk_id: source_desk_id.to_string(),
            target_desk_id: "*".to_string(),
            title: title.to_string(),
            summary: summary.to_string(),
            content: content.to_string(),
            url: None,
            observed_at: now.to_rfc3339(),
            kind,
            trust_score,
            entity_labels: Vec::new(),
            tags: Vec::new(),
            case_id: None,
            watchlist_id: None,
        }
    }
}

/// Overall federation status for the local desk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationStatus {
    /// The local desk's id.
    pub desk_id: String,
    /// Total number of configured peers.
    pub peer_count: usize,
    /// Number of enabled peers.
    pub enabled_peer_count: usize,
    /// Total outbound dispatches attempted.
    pub total_dispatches: usize,
    /// Successful dispatches.
    pub successful_dispatches: usize,
    /// Failed dispatches.
    pub failed_dispatches: usize,
    /// Whether federation is enabled (has at least one enabled peer).
    pub federation_enabled: bool,
}

/// A summary overview returned by the federation status endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationOverview {
    /// The local desk's id.
    pub desk_id: String,
    /// All configured peers.
    pub peers: Vec<crate::peer::PeerDesk>,
    /// Federation status summary.
    pub status: FederationStatus,
}

/// Computes the federation overview from a registry and dispatch log.
pub async fn compute_overview(
    desk_id: &str,
    registry: &PeerRegistry,
    log: &crate::dispatcher::DispatchLog,
) -> FederationOverview {
    let peers = registry
        .list(&crate::peer::PeerRegistryQuery::default())
        .await;
    let peer_count = peers.len();
    let enabled_peer_count = peers.iter().filter(|p| p.enabled).count();
    let entries = log.recent(100).await;
    let total_dispatches = entries.len();
    let successful_dispatches = entries
        .iter()
        .filter(|e| e.status == crate::dispatcher::DispatchStatus::Delivered)
        .count();
    let failed_dispatches = entries
        .iter()
        .filter(|e| e.status == crate::dispatcher::DispatchStatus::Failed)
        .count();

    FederationOverview {
        desk_id: desk_id.to_string(),
        peers,
        status: FederationStatus {
            desk_id: desk_id.to_string(),
            peer_count,
            enabled_peer_count,
            total_dispatches,
            successful_dispatches,
            failed_dispatches,
            federation_enabled: enabled_peer_count > 0,
        },
    }
}

/// Helper to create an Arc'd default desk id from an env var or fallback.
pub fn desk_id_from_env() -> String {
    std::env::var("HELIX_DESK_ID").unwrap_or_else(|_| "local-desk".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn federation_event_new_sets_correct_type() {
        let event = FederationEvent::new(
            "desk-a",
            FederationEventKind::CaseEscalated,
            "Case escalated",
            "Summary",
            "Content",
            80,
        );
        assert_eq!(event.event_type, "helix.case.escalated");
        assert_eq!(event.source_desk_id, "desk-a");
        assert_eq!(event.target_desk_id, "*");
        assert_eq!(event.specversion, "1.0");
        assert!(!event.id.is_empty());
    }

    #[test]
    fn federation_event_serializes_roundtrip() {
        let event = FederationEvent::new(
            "desk-a",
            FederationEventKind::WatchlistHit,
            "Hit",
            "Summary",
            "Content",
            90,
        );
        let json = serde_json::to_string(&event).unwrap();
        let back: FederationEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[tokio::test]
    async fn compute_overview_reports_counts() {
        let registry = PeerRegistry::new();
        let log = crate::dispatcher::DispatchLog::new(100);
        let overview = compute_overview("desk-x", &registry, &log).await;
        assert_eq!(overview.status.desk_id, "desk-x");
        assert_eq!(overview.status.peer_count, 0);
        assert_eq!(overview.status.federation_enabled, false);
    }
}
