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

//! Confirmation queue: pending AI proposals awaiting human review in
//! CoPilot assist mode.

use crate::proposals::OperatorProposal;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

/// The status of a confirmation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationStatus {
    /// Waiting for a human operator to review.
    Pending,
    /// A human operator confirmed the proposal.
    Confirmed,
    /// A human operator denied the proposal.
    Denied,
    /// The request expired before anyone reviewed it.
    Expired,
}

/// A pending AI proposal awaiting human confirmation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmationRequest {
    /// Unique request id.
    pub id: String,
    /// The cycle number this proposal came from.
    pub cycle: u64,
    /// The original proposal from the LLM.
    pub proposal: OperatorProposal,
    /// The LLM's rationale (copied from proposal for convenience).
    pub rationale: String,
    /// When the request was created (ISO-8601).
    pub requested_at: String,
    /// Current status.
    pub status: ConfirmationStatus,
    /// Session id of the human who confirmed or denied (if resolved).
    pub resolved_by: Option<String>,
    /// Display name of the resolver (if resolved).
    pub resolved_by_name: Option<String>,
    /// When the request was resolved (if resolved).
    pub resolved_at: Option<String>,
    /// Denial reason if denied.
    pub denial_reason: Option<String>,
}

impl ConfirmationRequest {
    /// Creates a new pending confirmation request from a proposal.
    pub fn new(cycle: u64, proposal: OperatorProposal) -> Self {
        let rationale = proposal.rationale.clone();
        Self {
            id: format!("conf-{}", uuid::Uuid::new_v4()),
            cycle,
            proposal,
            rationale,
            requested_at: Utc::now().to_rfc3339(),
            status: ConfirmationStatus::Pending,
            resolved_by: None,
            resolved_by_name: None,
            resolved_at: None,
            denial_reason: None,
        }
    }

    /// Whether this request is still pending review.
    pub fn is_pending(&self) -> bool {
        self.status == ConfirmationStatus::Pending
    }
}

/// Thread-safe bounded queue of pending confirmation requests.
#[derive(Debug, Clone)]
pub struct ConfirmationQueue {
    inner: Arc<RwLock<VecDeque<ConfirmationRequest>>>,
    capacity: usize,
}

impl ConfirmationQueue {
    /// Creates a new queue with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(VecDeque::with_capacity(capacity))),
            capacity,
        }
    }

    /// Enqueues a pending confirmation request. If at capacity, the oldest
    /// pending request is expired to make room.
    pub async fn enqueue(&self, request: ConfirmationRequest) -> String {
        let id = request.id.clone();
        let mut queue = self.inner.write().await;
        if queue.len() >= self.capacity {
            // Expire the oldest pending request
            if let Some(front) = queue.front_mut() {
                if front.is_pending() {
                    front.status = ConfirmationStatus::Expired;
                    front.resolved_at = Some(Utc::now().to_rfc3339());
                }
            }
            // Remove expired entries from the front
            while queue.front().map(|r| r.status == ConfirmationStatus::Expired).unwrap_or(false) {
                queue.pop_front();
            }
        }
        queue.push_back(request);
        id
    }

    /// Returns all requests (pending and resolved), newest first.
    pub async fn all(&self) -> Vec<ConfirmationRequest> {
        let queue = self.inner.read().await;
        queue.iter().rev().cloned().collect()
    }

    /// Returns only pending requests, oldest first.
    pub async fn pending(&self) -> Vec<ConfirmationRequest> {
        let queue = self.inner.read().await;
        queue
            .iter()
            .filter(|r| r.is_pending())
            .cloned()
            .collect()
    }

    /// Returns a request by id.
    pub async fn get(&self, id: &str) -> Option<ConfirmationRequest> {
        let queue = self.inner.read().await;
        queue.iter().find(|r| r.id == id).cloned()
    }

    /// Confirms a pending request. Returns the updated request, or None if
    /// not found or not pending.
    pub async fn confirm(
        &self,
        id: &str,
        resolver_session_id: &str,
        resolver_name: &str,
    ) -> Option<ConfirmationRequest> {
        let mut queue = self.inner.write().await;
        let request = queue.iter_mut().find(|r| r.id == id)?;
        if !request.is_pending() {
            return None;
        }
        request.status = ConfirmationStatus::Confirmed;
        request.resolved_by = Some(resolver_session_id.to_string());
        request.resolved_by_name = Some(resolver_name.to_string());
        request.resolved_at = Some(Utc::now().to_rfc3339());
        Some(request.clone())
    }

    /// Denies a pending request. Returns the updated request, or None if
    /// not found or not pending.
    pub async fn deny(
        &self,
        id: &str,
        resolver_session_id: &str,
        resolver_name: &str,
        reason: Option<String>,
    ) -> Option<ConfirmationRequest> {
        let mut queue = self.inner.write().await;
        let request = queue.iter_mut().find(|r| r.id == id)?;
        if !request.is_pending() {
            return None;
        }
        request.status = ConfirmationStatus::Denied;
        request.resolved_by = Some(resolver_session_id.to_string());
        request.resolved_by_name = Some(resolver_name.to_string());
        request.resolved_at = Some(Utc::now().to_rfc3339());
        request.denial_reason = reason;
        Some(request.clone())
    }

    /// Expires requests older than the given timeout (in seconds).
    /// Returns the number of requests expired.
    pub async fn expire_stale(&self, timeout_secs: i64) -> usize {
        let now = Utc::now();
        let mut count = 0;
        let mut queue = self.inner.write().await;
        for request in queue.iter_mut() {
            if !request.is_pending() {
                continue;
            }
            if let Ok(requested) = chrono::DateTime::parse_from_rfc3339(&request.requested_at) {
                let age = (now - requested.with_timezone(&Utc)).num_seconds();
                if age >= timeout_secs {
                    request.status = ConfirmationStatus::Expired;
                    request.resolved_at = Some(now.to_rfc3339());
                    count += 1;
                }
            }
        }
        count
    }

    /// Returns the number of pending requests.
    pub async fn pending_count(&self) -> usize {
        self.inner
            .read()
            .await
            .iter()
            .filter(|r| r.is_pending())
            .count()
    }

    /// Returns the total number of requests (pending + resolved).
    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }

    /// Clears resolved (confirmed/denied/expired) entries.
    pub async fn clear_resolved(&self) -> usize {
        let mut queue = self.inner.write().await;
        let before = queue.len();
        queue.retain(|r| r.is_pending());
        before - queue.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_proposal(action_type: &str, rationale: &str) -> OperatorProposal {
        OperatorProposal {
            action_type: action_type.to_string(),
            target_id: Some("case-1".to_string()),
            rationale: rationale.to_string(),
            parameters: serde_json::Value::Null,
        }
    }

    #[tokio::test]
    async fn enqueue_and_get_pending() {
        let queue = ConfirmationQueue::new(10);
        let req = ConfirmationRequest::new(1, make_proposal("escalate_case", "test"));
        let id = queue.enqueue(req).await;
        let pending = queue.pending().await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, id);
        assert_eq!(pending[0].status, ConfirmationStatus::Pending);
    }

    #[tokio::test]
    async fn confirm_resolves_request() {
        let queue = ConfirmationQueue::new(10);
        let req = ConfirmationRequest::new(1, make_proposal("escalate_case", "test"));
        let id = queue.enqueue(req).await;
        let resolved = queue.confirm(&id, "sess-1", "Alice").await;
        assert!(resolved.is_some());
        let resolved = resolved.unwrap();
        assert_eq!(resolved.status, ConfirmationStatus::Confirmed);
        assert_eq!(resolved.resolved_by_name, Some("Alice".to_string()));
        assert!(queue.pending().await.is_empty());
    }

    #[tokio::test]
    async fn deny_resolves_request() {
        let queue = ConfirmationQueue::new(10);
        let req = ConfirmationRequest::new(1, make_proposal("escalate_case", "test"));
        let id = queue.enqueue(req).await;
        let resolved = queue
            .deny(&id, "sess-1", "Bob", Some("not enough evidence".to_string()))
            .await;
        assert!(resolved.is_some());
        let resolved = resolved.unwrap();
        assert_eq!(resolved.status, ConfirmationStatus::Denied);
        assert_eq!(resolved.denial_reason, Some("not enough evidence".to_string()));
    }

    #[tokio::test]
    async fn confirm_nonexistent_returns_none() {
        let queue = ConfirmationQueue::new(10);
        assert!(queue.confirm("fake-id", "sess-1", "Alice").await.is_none());
    }

    #[tokio::test]
    async fn confirm_already_resolved_returns_none() {
        let queue = ConfirmationQueue::new(10);
        let req = ConfirmationRequest::new(1, make_proposal("escalate_case", "test"));
        let id = queue.enqueue(req).await;
        queue.confirm(&id, "sess-1", "Alice").await;
        // Second confirm should fail
        assert!(queue.confirm(&id, "sess-2", "Bob").await.is_none());
    }

    #[tokio::test]
    async fn capacity_evicts_oldest_pending() {
        let queue = ConfirmationQueue::new(2);
        let id1 = queue
            .enqueue(ConfirmationRequest::new(1, make_proposal("log_analysis", "first")))
            .await;
        queue
            .enqueue(ConfirmationRequest::new(2, make_proposal("log_analysis", "second")))
            .await;
        // Third enqueue should expire the first
        queue
            .enqueue(ConfirmationRequest::new(3, make_proposal("log_analysis", "third")))
            .await;
        let first = queue.get(&id1).await;
        // The first should have been expired and removed
        assert!(first.is_none() || first.unwrap().status == ConfirmationStatus::Expired);
    }

    #[tokio::test]
    async fn expire_stale_expires_old_pending() {
        let queue = ConfirmationQueue::new(10);
        let mut req = ConfirmationRequest::new(1, make_proposal("escalate_case", "test"));
        req.requested_at = (Utc::now() - chrono::Duration::seconds(300)).to_rfc3339();
        queue.enqueue(req).await;
        let expired = queue.expire_stale(60).await;
        assert_eq!(expired, 1);
        assert!(queue.pending().await.is_empty());
    }

    #[tokio::test]
    async fn clear_resolved_removes_non_pending() {
        let queue = ConfirmationQueue::new(10);
        let id1 = queue
            .enqueue(ConfirmationRequest::new(1, make_proposal("log_analysis", "first")))
            .await;
        let id2 = queue
            .enqueue(ConfirmationRequest::new(2, make_proposal("log_analysis", "second")))
            .await;
        queue.confirm(&id1, "sess-1", "Alice").await;
        assert_eq!(queue.len().await, 2);
        let cleared = queue.clear_resolved().await;
        assert_eq!(cleared, 1);
        assert_eq!(queue.len().await, 1);
        // Remaining should be the pending one
        let remaining = queue.get(&id2).await.unwrap();
        assert!(remaining.is_pending());
    }

    #[tokio::test]
    async fn pending_count() {
        let queue = ConfirmationQueue::new(10);
        queue
            .enqueue(ConfirmationRequest::new(1, make_proposal("log_analysis", "a")))
            .await;
        queue
            .enqueue(ConfirmationRequest::new(2, make_proposal("log_analysis", "b")))
            .await;
        assert_eq!(queue.pending_count().await, 2);
    }
}
