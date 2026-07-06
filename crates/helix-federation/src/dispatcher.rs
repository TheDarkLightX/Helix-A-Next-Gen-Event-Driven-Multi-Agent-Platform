// Copyright 2026 DarkLightX
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Outbound dispatcher: sends federation events to peer desks via HTTP POST
//! and records delivery outcomes in a dispatch log.

use crate::peer::{PeerDesk, PeerRegistry};
use crate::types::FederationEvent;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// The delivery status of an outbound dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchStatus {
    /// The event was successfully delivered to the peer.
    Delivered,
    /// The peer returned an error status code.
    Failed,
    /// The peer was unreachable (network error, timeout, etc.).
    Unreachable,
}

/// A single entry in the dispatch log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DispatchLogEntry {
    /// Unique entry id.
    pub id: String,
    /// The federation event id that was dispatched.
    pub event_id: String,
    /// The peer desk id that was the target.
    pub peer_id: String,
    /// The peer endpoint URL at dispatch time.
    pub peer_endpoint: String,
    /// The event kind.
    pub event_kind: String,
    /// The event title.
    pub event_title: String,
    /// Delivery status.
    pub status: DispatchStatus,
    /// HTTP status code if the peer responded, else null.
    pub http_status: Option<u16>,
    /// Error message if the dispatch failed.
    pub error_message: Option<String>,
    /// Round-trip latency in milliseconds.
    pub latency_ms: u64,
    /// ISO-8601 timestamp of the dispatch attempt.
    pub dispatched_at: String,
}

/// A bounded, thread-safe log of recent dispatch attempts.
#[derive(Debug, Clone)]
pub struct DispatchLog {
    inner: Arc<RwLock<VecDeque<DispatchLogEntry>>>,
    capacity: usize,
}

impl DispatchLog {
    /// Creates a new dispatch log with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(VecDeque::with_capacity(capacity))),
            capacity,
        }
    }

    /// Appends an entry, evicting the oldest if at capacity.
    pub async fn append(&self, entry: DispatchLogEntry) {
        let mut log = self.inner.write().await;
        if log.len() >= self.capacity {
            log.pop_front();
        }
        log.push_back(entry);
    }

    /// Returns the most recent `n` entries (newest first).
    pub async fn recent(&self, n: usize) -> Vec<DispatchLogEntry> {
        let log = self.inner.read().await;
        log.iter()
            .rev()
            .take(n)
            .cloned()
            .collect()
    }

    /// Returns the total number of entries.
    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }

    /// Returns true if the log is empty.
    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.is_empty()
    }
}

/// The outbound dispatcher sends federation events to enabled peers.
pub struct OutboundDispatcher {
    registry: PeerRegistry,
    log: DispatchLog,
    http_client: reqwest::Client,
    desk_id: String,
}

impl OutboundDispatcher {
    /// Creates a new dispatcher bound to a peer registry and dispatch log.
    pub fn new(
        desk_id: String,
        registry: PeerRegistry,
        log: DispatchLog,
    ) -> Self {
        Self {
            registry,
            log,
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("failed to build federation HTTP client"),
            desk_id,
        }
    }

    /// Dispatches a single event to all enabled peers.
    ///
    /// This is fail-closed: each peer dispatch is independent. A failure for
    /// one peer does not prevent dispatch to others. All outcomes are logged.
    pub async fn dispatch(&self, event: &FederationEvent) -> Vec<DispatchLogEntry> {
        let peers = self.registry.enabled_peers().await;
        let mut entries = Vec::with_capacity(peers.len());

        for peer in peers {
            let entry = self.dispatch_to_peer(event, &peer).await;
            self.log.append(entry.clone()).await;
            entries.push(entry);
        }

        entries
    }

    /// Dispatches an event to a single peer.
    async fn dispatch_to_peer(
        &self,
        event: &FederationEvent,
        peer: &PeerDesk,
    ) -> DispatchLogEntry {
        let start = Instant::now();
        let url = peer.receive_url();

        let result = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", peer.auth_token))
            .header("Content-Type", "application/json")
            .header("X-Helix-Desk-Id", &self.desk_id)
            .json(event)
            .send()
            .await;

        let latency_ms = start.elapsed().as_millis() as u64;
        let now = chrono::Utc::now().to_rfc3339();

        match result {
            Ok(response) => {
                let status_code = response.status().as_u16();
                if response.status().is_success() {
                    DispatchLogEntry {
                        id: format!("log-{}", uuid::Uuid::new_v4()),
                        event_id: event.id.clone(),
                        peer_id: peer.id.clone(),
                        peer_endpoint: peer.endpoint_url.clone(),
                        event_kind: event.event_type.clone(),
                        event_title: event.title.clone(),
                        status: DispatchStatus::Delivered,
                        http_status: Some(status_code),
                        error_message: None,
                        latency_ms,
                        dispatched_at: now,
                    }
                } else {
                    let body = response.text().await.unwrap_or_default();
                    DispatchLogEntry {
                        id: format!("log-{}", uuid::Uuid::new_v4()),
                        event_id: event.id.clone(),
                        peer_id: peer.id.clone(),
                        peer_endpoint: peer.endpoint_url.clone(),
                        event_kind: event.event_type.clone(),
                        event_title: event.title.clone(),
                        status: DispatchStatus::Failed,
                        http_status: Some(status_code),
                        error_message: Some(format!("HTTP {status_code}: {body}").chars().take(512).collect()),
                        latency_ms,
                        dispatched_at: now,
                    }
                }
            }
            Err(err) => DispatchLogEntry {
                id: format!("log-{}", uuid::Uuid::new_v4()),
                event_id: event.id.clone(),
                peer_id: peer.id.clone(),
                peer_endpoint: peer.endpoint_url.clone(),
                event_kind: event.event_type.clone(),
                event_title: event.title.clone(),
                status: DispatchStatus::Unreachable,
                http_status: None,
                error_message: Some(err.to_string()),
                latency_ms,
                dispatched_at: now,
            },
        }
    }

    /// Returns a reference to the dispatch log.
    pub fn log(&self) -> &DispatchLog {
        &self.log
    }

    /// Returns a reference to the peer registry.
    pub fn registry(&self) -> &PeerRegistry {
        &self.registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer::PeerDesk;

    fn make_peer(id: &str, enabled: bool) -> PeerDesk {
        PeerDesk {
            id: id.to_string(),
            name: format!("Peer {id}"),
            endpoint_url: "http://localhost:9999".to_string(),
            auth_token: "test-token-123".to_string(),
            trust_score: 80,
            enabled,
            tags: vec![],
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn dispatch_log_append_and_recent() {
        let log = DispatchLog::new(3);
        for i in 0..5 {
            let entry = DispatchLogEntry {
                id: format!("log-{i}"),
                event_id: format!("evt-{i}"),
                peer_id: "peer-a".to_string(),
                peer_endpoint: "http://localhost".to_string(),
                event_kind: "helix.case.escalated".to_string(),
                event_title: format!("Event {i}"),
                status: DispatchStatus::Delivered,
                http_status: Some(200),
                error_message: None,
                latency_ms: 10,
                dispatched_at: "2026-01-01T00:00:00Z".to_string(),
            };
            log.append(entry).await;
        }
        // Capacity is 3, so only 3 remain
        assert_eq!(log.len().await, 3);
        let recent = log.recent(10).await;
        // Newest first
        assert_eq!(recent[0].id, "log-4");
        assert_eq!(recent[2].id, "log-2");
    }

    #[tokio::test]
    async fn dispatch_log_empty() {
        let log = DispatchLog::new(10);
        assert!(log.is_empty().await);
        assert_eq!(log.len().await, 0);
        assert!(log.recent(5).await.is_empty());
    }

    #[tokio::test]
    async fn dispatcher_skips_disabled_peers() {
        let registry = PeerRegistry::from_peers(vec![
            make_peer("peer-a", false),
            make_peer("peer-b", false),
        ]);
        let log = DispatchLog::new(100);
        let dispatcher = OutboundDispatcher::new("desk-x".to_string(), registry, log);
        let event = FederationEvent::new(
            "desk-x",
            crate::types::FederationEventKind::CaseEscalated,
            "Test",
            "Summary",
            "Content",
            80,
        );
        let entries = dispatcher.dispatch(&event).await;
        assert!(entries.is_empty());
        assert!(dispatcher.log().is_empty().await);
    }

    #[tokio::test]
    async fn dispatcher_logs_unreachable_peer() {
        // Point at a port that's not listening
        let registry = PeerRegistry::from_peers(vec![make_peer("peer-a", true)]);
        let log = DispatchLog::new(100);
        let dispatcher = OutboundDispatcher::new("desk-x".to_string(), registry, log);
        let event = FederationEvent::new(
            "desk-x",
            crate::types::FederationEventKind::CaseEscalated,
            "Test",
            "Summary",
            "Content",
            80,
        );
        let entries = dispatcher.dispatch(&event).await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].status, DispatchStatus::Unreachable);
        assert!(entries[0].error_message.is_some());
        assert!(entries[0].http_status.is_none());
    }
}
