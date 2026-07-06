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

//! Integration tests for the federation crate.

use crate::{
    DispatchLog, DispatchStatus, FederationEvent, FederationEventKind, OutboundDispatcher,
    PeerDesk, PeerRegistry, PeerRegistryQuery,
};

fn make_peer(id: &str, enabled: bool) -> PeerDesk {
    PeerDesk {
        id: id.to_string(),
        name: format!("Peer {id}"),
        endpoint_url: "http://localhost:1".to_string(),
        auth_token: "test-token-12".to_string(),
        trust_score: 75,
        enabled,
        tags: vec!["test".to_string()],
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

#[tokio::test]
async fn full_federation_flow() {
    let registry = PeerRegistry::new();
    let log = DispatchLog::new(50);
    let dispatcher = OutboundDispatcher::new("desk-alpha".to_string(), registry.clone(), log.clone());

    // Register two peers, one disabled
    let peer_a = make_peer("desk-beta", true);
    let peer_b = make_peer("desk-gamma", false);
    registry.upsert(peer_a.clone()).await.unwrap();
    registry.upsert(peer_b).await.unwrap();

    // Verify registry state
    assert_eq!(registry.count().await, 2);
    assert_eq!(registry.enabled_count().await, 1);

    // Dispatch an event
    let event = FederationEvent::new(
        "desk-alpha",
        FederationEventKind::CaseEscalated,
        "Case #42 escalated to critical",
        "Executive departure detected at target company",
        "Full case details here",
        85,
    );
    let entries = dispatcher.dispatch(&event).await;

    // Only the enabled peer should receive a dispatch attempt
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].peer_id, "desk-beta");
    // localhost:1 won't be reachable
    assert_eq!(entries[0].status, DispatchStatus::Unreachable);

    // Log should have one entry
    assert_eq!(log.len().await, 1);
    let recent = log.recent(10).await;
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].event_id, event.id);

    // Remove a peer
    let removed = registry.remove("desk-beta").await.unwrap();
    assert_eq!(removed.id, "desk-beta");
    assert_eq!(registry.count().await, 1);
}

#[tokio::test]
async fn registry_query_filters() {
    let registry = PeerRegistry::from_peers(vec![
        {
            let mut p = make_peer("a", true);
            p.tags = vec!["osint".to_string()];
            p
        },
        {
            let mut p = make_peer("b", false);
            p.tags = vec!["osint".to_string()];
            p
        },
        {
            let mut p = make_peer("c", true);
            p.tags = vec!["market".to_string()];
            p
        },
    ]);

    // All peers
    let all = registry.list(&PeerRegistryQuery::default()).await;
    assert_eq!(all.len(), 3);

    // Enabled only
    let enabled = registry
        .list(&PeerRegistryQuery {
            enabled: Some(true),
            tag: None,
        })
        .await;
    assert_eq!(enabled.len(), 2);

    // Tag filter
    let osint = registry
        .list(&PeerRegistryQuery {
            enabled: None,
            tag: Some("osint".to_string()),
        })
        .await;
    assert_eq!(osint.len(), 2);

    // Combined
    let combined = registry
        .list(&PeerRegistryQuery {
            enabled: Some(true),
            tag: Some("osint".to_string()),
        })
        .await;
    assert_eq!(combined.len(), 1);
    assert_eq!(combined[0].id, "a");
}

#[tokio::test]
async fn event_kind_event_types() {
    assert_eq!(
        FederationEventKind::CaseEscalated.event_type(),
        "helix.case.escalated"
    );
    assert_eq!(
        FederationEventKind::WatchlistHit.event_type(),
        "helix.watchlist.hit"
    );
    assert_eq!(
        FederationEventKind::EvidenceIngested.event_type(),
        "helix.evidence.ingested"
    );
    assert_eq!(
        FederationEventKind::ManualBroadcast.event_type(),
        "helix.federation.broadcast"
    );
}

#[tokio::test]
async fn dispatch_log_capacity_eviction() {
    let log = DispatchLog::new(5);
    for i in 0..10 {
        let entry = crate::DispatchLogEntry {
            id: format!("e{i}"),
            event_id: format!("ev{i}"),
            peer_id: "p".to_string(),
            peer_endpoint: "http://x".to_string(),
            event_kind: "helix.test".to_string(),
            event_title: format!("T{i}"),
            status: DispatchStatus::Delivered,
            http_status: Some(200),
            error_message: None,
            latency_ms: 5,
            dispatched_at: "2026-01-01T00:00:00Z".to_string(),
        };
        log.append(entry).await;
    }
    assert_eq!(log.len().await, 5);
    let recent = log.recent(10).await;
    // Newest first: e9, e8, e7, e6, e5
    assert_eq!(recent[0].id, "e9");
    assert_eq!(recent[4].id, "e5");
}
