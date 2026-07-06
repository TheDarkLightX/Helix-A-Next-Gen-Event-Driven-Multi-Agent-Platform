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

//! Session registry: tracks all active CoPilot participants with presence.

use crate::session::{
    OperatorSession, PresenceThresholds, SessionKind, SessionStatus,
};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Thread-safe registry of CoPilot sessions.
#[derive(Debug, Clone)]
pub struct OperatorRegistry {
    inner: Arc<RwLock<HashMap<String, OperatorSession>>>,
    thresholds: PresenceThresholds,
}

impl OperatorRegistry {
    /// Creates a new empty registry with default presence thresholds.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            thresholds: PresenceThresholds::default(),
        }
    }

    /// Creates a new registry with custom presence thresholds.
    pub fn with_thresholds(thresholds: PresenceThresholds) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            thresholds,
        }
    }

    /// Adds a session to the registry.
    pub async fn add(&self, session: OperatorSession) -> String {
        let id = session.id.clone();
        self.inner.write().await.insert(id.clone(), session);
        id
    }

    /// Removes a session by id. Returns the removed session if it existed.
    pub async fn remove(&self, session_id: &str) -> Option<OperatorSession> {
        self.inner.write().await.remove(session_id)
    }

    /// Returns a session by id.
    pub async fn get(&self, session_id: &str) -> Option<OperatorSession> {
        self.inner.read().await.get(session_id).cloned()
    }

    /// Updates the heartbeat for a session. Returns false if session not found.
    pub async fn heartbeat(&self, session_id: &str) -> bool {
        let mut sessions = self.inner.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.heartbeat();
            true
        } else {
            false
        }
    }

    /// Lists all sessions, updating their computed status first.
    pub async fn list(&self) -> Vec<OperatorSession> {
        let now = Utc::now();
        let mut sessions = self.inner.write().await;
        for session in sessions.values_mut() {
            session.status = session.compute_status(
                now,
                self.thresholds.idle_secs,
                self.thresholds.disconnect_secs,
            );
        }
        sessions.values().cloned().collect()
    }

    /// Lists only active (non-disconnected) sessions.
    pub async fn list_active(&self) -> Vec<OperatorSession> {
        let all = self.list().await;
        all.into_iter()
            .filter(|s| s.status != SessionStatus::Disconnected)
            .collect()
    }

    /// Returns the count of sessions by kind.
    pub async fn counts_by_kind(&self) -> (usize, usize) {
        let sessions = self.inner.read().await;
        let humans = sessions
            .values()
            .filter(|s| s.kind == SessionKind::Human)
            .count();
        let ais = sessions
            .values()
            .filter(|s| s.kind == SessionKind::Ai)
            .count();
        (humans, ais)
    }

    /// Prunes disconnected sessions. Returns the number removed.
    pub async fn prune_stale(&self) -> usize {
        let now = Utc::now();
        let mut sessions = self.inner.write().await;
        let to_remove: Vec<String> = sessions
            .iter()
            .filter(|(_, s)| {
                s.compute_status(
                    now,
                    self.thresholds.idle_secs,
                    self.thresholds.disconnect_secs,
                ) == SessionStatus::Disconnected
            })
            .map(|(id, _)| id.clone())
            .collect();
        let count = to_remove.len();
        for id in &to_remove {
            sessions.remove(id);
        }
        count
    }

    /// Returns the total number of sessions.
    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }

    /// Returns true if the registry is empty.
    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.is_empty()
    }
}

impl Default for OperatorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionRole;

    #[tokio::test]
    async fn add_and_get_session() {
        let registry = OperatorRegistry::new();
        let session = OperatorSession::new(
            "Alice".to_string(),
            SessionKind::Human,
            SessionRole::Operator,
            None,
        );
        let id = registry.add(session).await;
        let retrieved = registry.get(&id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().display_name, "Alice");
    }

    #[tokio::test]
    async fn remove_session() {
        let registry = OperatorRegistry::new();
        let session = OperatorSession::new(
            "Bob".to_string(),
            SessionKind::Human,
            SessionRole::Viewer,
            None,
        );
        let id = registry.add(session).await;
        assert_eq!(registry.len().await, 1);
        let removed = registry.remove(&id).await;
        assert!(removed.is_some());
        assert_eq!(registry.len().await, 0);
    }

    #[tokio::test]
    async fn heartbeat_updates_session() {
        let registry = OperatorRegistry::new();
        let session = OperatorSession::new(
            "Carol".to_string(),
            SessionKind::Human,
            SessionRole::Operator,
            None,
        );
        let id = registry.add(session).await;
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(registry.heartbeat(&id).await);
        let updated = registry.get(&id).await.unwrap();
        assert_eq!(updated.status, SessionStatus::Active);
    }

    #[tokio::test]
    async fn heartbeat_returns_false_for_missing() {
        let registry = OperatorRegistry::new();
        assert!(!registry.heartbeat("nonexistent").await);
    }

    #[tokio::test]
    async fn counts_by_kind() {
        let registry = OperatorRegistry::new();
        registry
            .add(OperatorSession::new(
                "Alice".to_string(),
                SessionKind::Human,
                SessionRole::Operator,
                None,
            ))
            .await;
        registry
            .add(OperatorSession::new(
                "Bob".to_string(),
                SessionKind::Human,
                SessionRole::Viewer,
                None,
            ))
            .await;
        registry
            .add(OperatorSession::new(
                "Copilot".to_string(),
                SessionKind::Ai,
                SessionRole::Operator,
                None,
            ))
            .await;
        let (humans, ais) = registry.counts_by_kind().await;
        assert_eq!(humans, 2);
        assert_eq!(ais, 1);
    }

    #[tokio::test]
    async fn prune_removes_disconnected() {
        let thresholds = PresenceThresholds {
            idle_secs: 1,
            disconnect_secs: 2,
        };
        let registry = OperatorRegistry::with_thresholds(thresholds);
        let mut stale = OperatorSession::new(
            "Dave".to_string(),
            SessionKind::Human,
            SessionRole::Operator,
            None,
        );
        stale.last_heartbeat = (Utc::now() - chrono::Duration::seconds(10)).to_rfc3339();
        registry.add(stale).await;

        let fresh = OperatorSession::new(
            "Eve".to_string(),
            SessionKind::Human,
            SessionRole::Operator,
            None,
        );
        registry.add(fresh).await;

        assert_eq!(registry.len().await, 2);
        let pruned = registry.prune_stale().await;
        assert_eq!(pruned, 1);
        assert_eq!(registry.len().await, 1);
    }

    #[tokio::test]
    async fn list_returns_all_sessions() {
        let registry = OperatorRegistry::new();
        for i in 0..3 {
            registry
                .add(OperatorSession::new(
                    format!("User{i}"),
                    SessionKind::Human,
                    SessionRole::Operator,
                    None,
                ))
                .await;
        }
        let sessions = registry.list().await;
        assert_eq!(sessions.len(), 3);
    }

    #[tokio::test]
    async fn list_active_excludes_disconnected() {
        let thresholds = PresenceThresholds {
            idle_secs: 1,
            disconnect_secs: 2,
        };
        let registry = OperatorRegistry::with_thresholds(thresholds);
        let mut stale = OperatorSession::new(
            "Stale".to_string(),
            SessionKind::Human,
            SessionRole::Operator,
            None,
        );
        stale.last_heartbeat = (Utc::now() - chrono::Duration::seconds(10)).to_rfc3339();
        registry.add(stale).await;

        registry
            .add(OperatorSession::new(
                "Fresh".to_string(),
                SessionKind::Human,
                SessionRole::Operator,
                None,
            ))
            .await;

        let active = registry.list_active().await;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].display_name, "Fresh");
    }
}
