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

//! Peer desk registry: the set of trusted remote Helix instances.

use crate::errors::FederationError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use url::Url;

/// Minimum trust score a peer can have.
pub const MIN_PEER_TRUST: u8 = 0;
/// Maximum trust score a peer can have.
pub const MAX_PEER_TRUST: u8 = 100;
/// Minimum length for a peer name.
pub const MIN_PEER_NAME_LEN: usize = 1;
/// Maximum length for a peer name.
pub const MAX_PEER_NAME_LEN: usize = 128;
/// Maximum length for a peer id.
pub const MAX_PEER_ID_LEN: usize = 64;
/// Minimum length for an auth token.
pub const MIN_AUTH_TOKEN_LEN: usize = 8;

/// A trusted remote Helix intelligence desk.
///
/// Peers are addressed by their endpoint URL. When the local desk dispatches
/// a [`crate::FederationEvent`], it POSTs the event payload to
/// `{endpoint_url}/api/v1/federation/receive` with the configured auth token
/// in the `Authorization` header.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerDesk {
    /// Stable identifier for this peer (slug-style, unique within the registry).
    pub id: String,
    /// Human-readable name for this peer desk.
    pub name: String,
    /// Base endpoint URL (e.g. `https://helix.example.com`).
    pub endpoint_url: String,
    /// Bearer token used to authenticate outbound dispatches to this peer.
    pub auth_token: String,
    /// Trust score (0–100) that this desk assigns to intelligence received
    /// from this peer. Higher is more trusted.
    pub trust_score: u8,
    /// Whether outbound dispatch to this peer is currently enabled.
    pub enabled: bool,
    /// Optional tags for grouping or filtering peers.
    #[serde(default)]
    pub tags: Vec<String>,
    /// ISO-8601 timestamp of when this peer was registered.
    pub created_at: String,
    /// ISO-8601 timestamp of the most recent update.
    pub updated_at: String,
}

impl PeerDesk {
    /// Validates this peer's fields.
    pub fn validate(&self) -> Result<(), FederationError> {
        if self.id.trim().is_empty() || self.id.len() > MAX_PEER_ID_LEN {
            return Err(FederationError::PeerValidation(format!(
                "peer id must be 1–{MAX_PEER_ID_LEN} characters"
            )));
        }
        if self.name.trim().len() < MIN_PEER_NAME_LEN || self.name.len() > MAX_PEER_NAME_LEN {
            return Err(FederationError::PeerValidation(format!(
                "peer name must be {MIN_PEER_NAME_LEN}–{MAX_PEER_NAME_LEN} characters"
            )));
        }
        if Url::parse(&self.endpoint_url).is_err() {
            return Err(FederationError::PeerValidation(
                "endpoint_url must be a valid URL".to_string(),
            ));
        }
        if self.auth_token.trim().len() < MIN_AUTH_TOKEN_LEN {
            return Err(FederationError::PeerValidation(format!(
                "auth_token must be at least {MIN_AUTH_TOKEN_LEN} characters"
            )));
        }
        if self.trust_score > MAX_PEER_TRUST {
            return Err(FederationError::PeerValidation(format!(
                "trust_score must be {MIN_PEER_TRUST}–{MAX_PEER_TRUST}"
            )));
        }
        Ok(())
    }

    /// The full inbound URL for dispatching federation events to this peer.
    pub fn receive_url(&self) -> String {
        let base = self.endpoint_url.trim_end_matches('/');
        format!("{base}/api/v1/federation/receive")
    }
}

/// Query parameters for listing peers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PeerRegistryQuery {
    /// If set, only return peers with this enabled state.
    pub enabled: Option<bool>,
    /// If set, only return peers with this tag.
    pub tag: Option<String>,
}

/// Thread-safe registry of trusted peer desks.
#[derive(Debug, Clone)]
pub struct PeerRegistry {
    inner: Arc<RwLock<BTreeMap<String, PeerDesk>>>,
}

impl Default for PeerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PeerRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// Creates a registry seeded with the given peers.
    pub fn from_peers(peers: Vec<PeerDesk>) -> Self {
        let map = peers.into_iter().map(|p| (p.id.clone(), p)).collect();
        Self {
            inner: Arc::new(RwLock::new(map)),
        }
    }

    /// Lists peers, optionally filtered.
    pub async fn list(&self, query: &PeerRegistryQuery) -> Vec<PeerDesk> {
        let peers = self.inner.read().await;
        peers
            .values()
            .filter(|peer| match query.enabled {
                Some(want) => peer.enabled == want,
                None => true,
            })
            .filter(|peer| match &query.tag {
                Some(tag) => peer.tags.contains(tag),
                None => true,
            })
            .cloned()
            .collect()
    }

    /// Returns a single peer by id.
    pub async fn get(&self, id: &str) -> Option<PeerDesk> {
        let peers = self.inner.read().await;
        peers.get(id).cloned()
    }

    /// Inserts or replaces a peer. Validates first.
    pub async fn upsert(&self, peer: PeerDesk) -> Result<PeerDesk, FederationError> {
        peer.validate()?;
        let mut peers = self.inner.write().await;
        peers.insert(peer.id.clone(), peer.clone());
        Ok(peer)
    }

    /// Removes a peer by id.
    pub async fn remove(&self, id: &str) -> Result<PeerDesk, FederationError> {
        let mut peers = self.inner.write().await;
        peers
            .remove(id)
            .ok_or_else(|| FederationError::PeerNotFound(id.to_string()))
    }

    /// Returns only enabled peers (used by the dispatcher).
    pub async fn enabled_peers(&self) -> Vec<PeerDesk> {
        let peers = self.inner.read().await;
        peers.values().filter(|p| p.enabled).cloned().collect()
    }

    /// Returns the count of all peers.
    pub async fn count(&self) -> usize {
        self.inner.read().await.len()
    }

    /// Returns the count of enabled peers.
    pub async fn enabled_count(&self) -> usize {
        self.inner
            .read()
            .await
            .values()
            .filter(|p| p.enabled)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_peer() -> PeerDesk {
        PeerDesk {
            id: "desk-alpha".to_string(),
            name: "Alpha Desk".to_string(),
            endpoint_url: "https://alpha.helix.io".to_string(),
            auth_token: "secret-token-123".to_string(),
            trust_score: 80,
            enabled: true,
            tags: vec!["osint".to_string()],
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn peer_validation_accepts_valid_peer() {
        assert!(valid_peer().validate().is_ok());
    }

    #[test]
    fn peer_validation_rejects_empty_id() {
        let mut peer = valid_peer();
        peer.id = "".to_string();
        assert!(peer.validate().is_err());
    }

    #[test]
    fn peer_validation_rejects_bad_url() {
        let mut peer = valid_peer();
        peer.endpoint_url = "not a url".to_string();
        assert!(peer.validate().is_err());
    }

    #[test]
    fn peer_validation_rejects_short_token() {
        let mut peer = valid_peer();
        peer.auth_token = "short".to_string();
        assert!(peer.validate().is_err());
    }

    #[test]
    fn peer_validation_rejects_trust_over_100() {
        let mut peer = valid_peer();
        peer.trust_score = 101;
        assert!(peer.validate().is_err());
    }

    #[test]
    fn receive_url_strips_trailing_slash() {
        let mut peer = valid_peer();
        peer.endpoint_url = "https://alpha.helix.io/".to_string();
        assert_eq!(
            peer.receive_url(),
            "https://alpha.helix.io/api/v1/federation/receive"
        );
    }

    #[tokio::test]
    async fn registry_upsert_and_get() {
        let registry = PeerRegistry::new();
        let peer = valid_peer();
        registry.upsert(peer.clone()).await.unwrap();
        let got = registry.get("desk-alpha").await.unwrap();
        assert_eq!(got, peer);
    }

    #[tokio::test]
    async fn registry_remove_returns_peer() {
        let registry = PeerRegistry::from_peers(vec![valid_peer()]);
        let removed = registry.remove("desk-alpha").await.unwrap();
        assert_eq!(removed.id, "desk-alpha");
        assert!(registry.get("desk-alpha").await.is_none());
    }

    #[tokio::test]
    async fn registry_remove_missing_returns_error() {
        let registry = PeerRegistry::new();
        let result = registry.remove("nope").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn registry_enabled_peers_filters_disabled() {
        let mut peer = valid_peer();
        peer.enabled = false;
        let registry = PeerRegistry::from_peers(vec![peer]);
        assert_eq!(registry.enabled_peers().await.len(), 0);
    }

    #[tokio::test]
    async fn registry_list_filters_by_tag() {
        let mut peer_a = valid_peer();
        peer_a.id = "desk-a".to_string();
        peer_a.tags = vec!["osint".to_string()];
        let mut peer_b = valid_peer();
        peer_b.id = "desk-b".to_string();
        peer_b.tags = vec!["market".to_string()];
        let registry = PeerRegistry::from_peers(vec![peer_a, peer_b]);
        let query = PeerRegistryQuery {
            enabled: None,
            tag: Some("osint".to_string()),
        };
        let result = registry.list(&query).await;
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "desk-a");
    }

    #[tokio::test]
    async fn registry_counts() {
        let mut peer_a = valid_peer();
        peer_a.id = "desk-a".to_string();
        let mut peer_b = valid_peer();
        peer_b.id = "desk-b".to_string();
        peer_b.enabled = false;
        let registry = PeerRegistry::from_peers(vec![peer_a, peer_b]);
        assert_eq!(registry.count().await, 2);
        assert_eq!(registry.enabled_count().await, 1);
    }
}
