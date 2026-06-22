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

//! Collaboration events: real-time broadcast of desk activity to all
//! connected CoPilot participants via Server-Sent Events.

use crate::confirmation::ConfirmationRequest;
use crate::proposals::OperatorDecision;
use crate::session::OperatorSession;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

/// An event broadcast to all connected CoPilot participants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CollaborationEvent {
    /// A new session joined the desk.
    SessionJoined {
        /// The session that joined.
        session: OperatorSession,
        /// Timestamp (ISO-8601).
        timestamp: String,
    },
    /// A session left the desk.
    SessionLeft {
        /// Session id.
        session_id: String,
        /// Display name.
        display_name: String,
        /// Timestamp.
        timestamp: String,
    },
    /// A session sent a heartbeat.
    Heartbeat {
        /// Session id.
        session_id: String,
        /// Timestamp.
        timestamp: String,
    },
    /// The operator loop started.
    LoopStarted {
        /// Timestamp.
        timestamp: String,
    },
    /// The operator loop stopped.
    LoopStopped {
        /// Timestamp.
        timestamp: String,
    },
    /// The operator loop paused.
    LoopPaused {
        /// Timestamp.
        timestamp: String,
    },
    /// A cycle completed — proposals were generated and evaluated.
    CycleCompleted {
        /// Cycle number.
        cycle: u64,
        /// Number of proposals allowed.
        allowed_count: usize,
        /// Number of proposals denied.
        denied_count: usize,
        /// Number of proposals sent to confirmation queue.
        pending_confirmation_count: usize,
        /// Timestamp.
        timestamp: String,
    },
    /// A proposal was submitted for human confirmation.
    ConfirmationRequested {
        /// The confirmation request.
        request: ConfirmationRequest,
        /// Timestamp.
        timestamp: String,
    },
    /// A confirmation request was resolved (confirmed or denied).
    ConfirmationResolved {
        /// The resolved request.
        request: ConfirmationRequest,
        /// Timestamp.
        timestamp: String,
    },
    /// An operator decision was made (allowed or denied by guard).
    OperatorDecision {
        /// Cycle number.
        cycle: u64,
        /// The decision.
        decision: OperatorDecision,
        /// Timestamp.
        timestamp: String,
    },
    /// The operator config was changed.
    ConfigChanged {
        /// Timestamp.
        timestamp: String,
    },
    /// A session was pruned (disconnected timeout).
    SessionPruned {
        /// Session id.
        session_id: String,
        /// Display name.
        display_name: String,
        /// Timestamp.
        timestamp: String,
    },
}

impl CollaborationEvent {
    fn timestamp_now() -> String {
        chrono::Utc::now().to_rfc3339()
    }

    /// Creates a SessionJoined event.
    pub fn session_joined(session: OperatorSession) -> Self {
        Self::SessionJoined {
            session,
            timestamp: Self::timestamp_now(),
        }
    }

    /// Creates a SessionLeft event.
    pub fn session_left(session_id: String, display_name: String) -> Self {
        Self::SessionLeft {
            session_id,
            display_name,
            timestamp: Self::timestamp_now(),
        }
    }

    /// Creates a Heartbeat event.
    pub fn heartbeat(session_id: String) -> Self {
        Self::Heartbeat {
            session_id,
            timestamp: Self::timestamp_now(),
        }
    }

    /// Creates a LoopStarted event.
    pub fn loop_started() -> Self {
        Self::LoopStarted {
            timestamp: Self::timestamp_now(),
        }
    }

    /// Creates a LoopStopped event.
    pub fn loop_stopped() -> Self {
        Self::LoopStopped {
            timestamp: Self::timestamp_now(),
        }
    }

    /// Creates a LoopPaused event.
    pub fn loop_paused() -> Self {
        Self::LoopPaused {
            timestamp: Self::timestamp_now(),
        }
    }

    /// Creates a CycleCompleted event.
    pub fn cycle_completed(
        cycle: u64,
        allowed_count: usize,
        denied_count: usize,
        pending_confirmation_count: usize,
    ) -> Self {
        Self::CycleCompleted {
            cycle,
            allowed_count,
            denied_count,
            pending_confirmation_count,
            timestamp: Self::timestamp_now(),
        }
    }

    /// Creates a ConfirmationRequested event.
    pub fn confirmation_requested(request: ConfirmationRequest) -> Self {
        Self::ConfirmationRequested {
            request,
            timestamp: Self::timestamp_now(),
        }
    }

    /// Creates a ConfirmationResolved event.
    pub fn confirmation_resolved(request: ConfirmationRequest) -> Self {
        Self::ConfirmationResolved {
            request,
            timestamp: Self::timestamp_now(),
        }
    }

    /// Creates an OperatorDecision event.
    pub fn operator_decision(cycle: u64, decision: OperatorDecision) -> Self {
        Self::OperatorDecision {
            cycle,
            decision,
            timestamp: Self::timestamp_now(),
        }
    }

    /// Creates a ConfigChanged event.
    pub fn config_changed() -> Self {
        Self::ConfigChanged {
            timestamp: Self::timestamp_now(),
        }
    }

    /// Creates a SessionPruned event.
    pub fn session_pruned(session_id: String, display_name: String) -> Self {
        Self::SessionPruned {
            session_id,
            display_name,
            timestamp: Self::timestamp_now(),
        }
    }
}

/// Thread-safe broadcaster for collaboration events.
///
/// Uses a tokio broadcast channel — each SSE client subscribes by calling
/// `subscribe()`, which returns a receiver. Events are serialized as JSON
/// for transmission over SSE.
#[derive(Debug, Clone)]
pub struct CollaborationBroadcaster {
    sender: broadcast::Sender<CollaborationEvent>,
}

impl CollaborationBroadcaster {
    /// Creates a new broadcaster with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Subscribes to the event stream. Each subscriber gets its own receiver.
    pub fn subscribe(&self) -> broadcast::Receiver<CollaborationEvent> {
        self.sender.subscribe()
    }

    /// Broadcasts an event to all subscribers.
    pub fn broadcast(&self, event: CollaborationEvent) {
        // Ignore send errors — no subscribers means nobody is listening,
        // which is fine.
        let _ = self.sender.send(event);
    }

    /// Returns the number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for CollaborationBroadcaster {
    fn default() -> Self {
        Self::new(256)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proposals::OperatorProposal;
    use crate::session::{SessionKind, SessionRole};

    #[tokio::test]
    async fn broadcast_delivers_to_subscribers() {
        let broadcaster = CollaborationBroadcaster::new(16);
        let mut rx1 = broadcaster.subscribe();
        let mut rx2 = broadcaster.subscribe();

        broadcaster.broadcast(CollaborationEvent::loop_started());

        let event1 = rx1.recv().await.unwrap();
        let event2 = rx2.recv().await.unwrap();

        assert!(matches!(event1, CollaborationEvent::LoopStarted { .. }));
        assert!(matches!(event2, CollaborationEvent::LoopStarted { .. }));
    }

    #[tokio::test]
    async fn broadcast_with_no_subscribers_is_ok() {
        let broadcaster = CollaborationBroadcaster::new(16);
        // No subscribers — should not panic
        broadcaster.broadcast(CollaborationEvent::loop_stopped());
    }

    #[tokio::test]
    async fn subscriber_count_tracks_receivers() {
        let broadcaster = CollaborationBroadcaster::new(16);
        assert_eq!(broadcaster.subscriber_count(), 0);
        let _rx1 = broadcaster.subscribe();
        assert_eq!(broadcaster.subscriber_count(), 1);
        let _rx2 = broadcaster.subscribe();
        assert_eq!(broadcaster.subscriber_count(), 2);
    }

    #[tokio::test]
    async fn session_joined_event_serializes() {
        let session = OperatorSession::new(
            "Alice".to_string(),
            SessionKind::Human,
            SessionRole::Operator,
            None,
        );
        let event = CollaborationEvent::session_joined(session);
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("session_joined"));
        assert!(json.contains("Alice"));
    }

    #[tokio::test]
    async fn confirmation_requested_event_serializes() {
        let proposal = OperatorProposal {
            action_type: "escalate_case".to_string(),
            target_id: Some("case-1".to_string()),
            rationale: "test".to_string(),
            parameters: serde_json::Value::Null,
        };
        let request = ConfirmationRequest::new(1, proposal);
        let event = CollaborationEvent::confirmation_requested(request);
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("confirmation_requested"));
        assert!(json.contains("escalate_case"));
    }

    #[tokio::test]
    async fn late_subscriber_misses_earlier_events() {
        let broadcaster = CollaborationBroadcaster::new(16);
        broadcaster.broadcast(CollaborationEvent::loop_started());
        // Subscribe after the broadcast
        let mut rx = broadcaster.subscribe();
        broadcaster.broadcast(CollaborationEvent::loop_stopped());
        let event = rx.recv().await.unwrap();
        // Should only get the second event
        assert!(matches!(event, CollaborationEvent::LoopStopped { .. }));
    }
}
