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

//! CoPilot sessions: participants (human or AI) connected to a shared desk.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The kind of participant operating the desk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    /// A human operator.
    Human,
    /// An AI copilot (the LLM desk operator).
    Ai,
}

/// The role assigned to a session — controls what actions they can take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionRole {
    /// Can observe the desk and read all data, but cannot take actions
    /// or confirm proposals.
    Viewer,
    /// Can take intelligence actions and confirm/deny AI proposals.
    Operator,
    /// Full control — can change operator config, start/stop the loop,
    /// and manage other sessions.
    Admin,
}

impl Default for SessionRole {
    fn default() -> Self {
        Self::Operator
    }
}

impl SessionRole {
    /// Whether this role can confirm or deny pending AI proposals.
    pub fn can_confirm(&self) -> bool {
        matches!(self, Self::Operator | Self::Admin)
    }

    /// Whether this role can change operator configuration.
    pub fn can_configure(&self) -> bool {
        matches!(self, Self::Admin)
    }

    /// Whether this role can start/stop the operator loop.
    pub fn can_control_loop(&self) -> bool {
        matches!(self, Self::Admin)
    }

    /// Whether this role can manage (remove) other sessions.
    pub fn can_manage_sessions(&self) -> bool {
        matches!(self, Self::Admin)
    }
}

/// The presence status of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Actively interacting (recent heartbeat).
    Active,
    /// Connected but no recent activity (idle threshold exceeded).
    Idle,
    /// Heartbeat timeout exceeded — will be pruned.
    Disconnected,
}

impl Default for SessionStatus {
    fn default() -> Self {
        Self::Active
    }
}

/// A participant in CoPilot mode — a human or AI connected to the shared desk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorSession {
    /// Unique session identifier.
    pub id: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Whether this is a human or AI participant.
    pub kind: SessionKind,
    /// Permission level.
    pub role: SessionRole,
    /// Current presence status.
    pub status: SessionStatus,
    /// When the session was created (ISO-8601).
    pub joined_at: String,
    /// Timestamp of the last heartbeat (ISO-8601).
    pub last_heartbeat: String,
    /// Optional location/affiliation string (e.g. "Tokyo", "Desk Alpha").
    pub location: Option<String>,
}

impl OperatorSession {
    /// Creates a new session with the given parameters.
    pub fn new(
        display_name: String,
        kind: SessionKind,
        role: SessionRole,
        location: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: format!("sess-{}", uuid::Uuid::new_v4()),
            display_name,
            kind,
            role,
            status: SessionStatus::Active,
            joined_at: now.to_rfc3339(),
            last_heartbeat: now.to_rfc3339(),
            location,
        }
    }

    /// Updates the heartbeat timestamp to now.
    pub fn heartbeat(&mut self) {
        self.last_heartbeat = Utc::now().to_rfc3339();
        self.status = SessionStatus::Active;
    }

    /// Computes the current status based on heartbeat age.
    pub fn compute_status(&self, now: DateTime<Utc>, idle_secs: i64, disconnect_secs: i64) -> SessionStatus {
        let last = DateTime::parse_from_rfc3339(&self.last_heartbeat)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or(now);
        let age = (now - last).num_seconds();
        if age >= disconnect_secs {
            SessionStatus::Disconnected
        } else if age >= idle_secs {
            SessionStatus::Idle
        } else {
            SessionStatus::Active
        }
    }
}

/// Request to create a new session.
#[derive(Debug, Clone, Deserialize)]
pub struct JoinSessionRequest {
    /// Display name for this operator.
    pub display_name: String,
    /// Whether this is a human or AI joining.
    #[serde(default = "default_kind")]
    pub kind: SessionKind,
    /// Requested role.
    #[serde(default)]
    pub role: SessionRole,
    /// Optional location/affiliation.
    pub location: Option<String>,
}

fn default_kind() -> SessionKind {
    SessionKind::Human
}

/// Thresholds for presence detection.
#[derive(Debug, Clone, Copy)]
pub struct PresenceThresholds {
    /// Seconds without heartbeat before marking as idle.
    pub idle_secs: i64,
    /// Seconds without heartbeat before marking as disconnected.
    pub disconnect_secs: i64,
}

impl Default for PresenceThresholds {
    fn default() -> Self {
        Self {
            idle_secs: 30,
            disconnect_secs: 90,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_new_has_active_status() {
        let session = OperatorSession::new(
            "Alice".to_string(),
            SessionKind::Human,
            SessionRole::Operator,
            Some("Tokyo".to_string()),
        );
        assert_eq!(session.status, SessionStatus::Active);
        assert_eq!(session.kind, SessionKind::Human);
        assert_eq!(session.role, SessionRole::Operator);
        assert!(session.id.starts_with("sess-"));
    }

    #[test]
    fn heartbeat_updates_timestamp_and_status() {
        let mut session = OperatorSession::new(
            "Bob".to_string(),
            SessionKind::Human,
            SessionRole::Viewer,
            None,
        );
        let original = session.last_heartbeat.clone();
        std::thread::sleep(std::time::Duration::from_millis(10));
        session.heartbeat();
        assert_ne!(session.last_heartbeat, original);
        assert_eq!(session.status, SessionStatus::Active);
    }

    #[test]
    fn compute_status_idle_after_threshold() {
        let session = OperatorSession::new(
            "Carol".to_string(),
            SessionKind::Human,
            SessionRole::Operator,
            None,
        );
        let now = Utc::now();
        // Simulate 45 seconds since last heartbeat
        let old_time = now - chrono::Duration::seconds(45);
        let mut session = session;
        session.last_heartbeat = old_time.to_rfc3339();
        let thresholds = PresenceThresholds::default();
        assert_eq!(
            session.compute_status(now, thresholds.idle_secs, thresholds.disconnect_secs),
            SessionStatus::Idle
        );
    }

    #[test]
    fn compute_status_disconnected_after_timeout() {
        let mut session = OperatorSession::new(
            "Dave".to_string(),
            SessionKind::Ai,
            SessionRole::Operator,
            None,
        );
        let now = Utc::now();
        let old_time = now - chrono::Duration::seconds(120);
        session.last_heartbeat = old_time.to_rfc3339();
        let thresholds = PresenceThresholds::default();
        assert_eq!(
            session.compute_status(now, thresholds.idle_secs, thresholds.disconnect_secs),
            SessionStatus::Disconnected
        );
    }

    #[test]
    fn role_permissions_are_correct() {
        assert!(SessionRole::Admin.can_confirm());
        assert!(SessionRole::Operator.can_confirm());
        assert!(!SessionRole::Viewer.can_confirm());

        assert!(SessionRole::Admin.can_configure());
        assert!(!SessionRole::Operator.can_configure());
        assert!(!SessionRole::Viewer.can_configure());

        assert!(SessionRole::Admin.can_control_loop());
        assert!(!SessionRole::Operator.can_control_loop());

        assert!(SessionRole::Admin.can_manage_sessions());
        assert!(!SessionRole::Operator.can_manage_sessions());
    }

    #[test]
    fn join_request_deserializes_with_defaults() {
        let json = r#"{"display_name": "Eve"}"#;
        let req: JoinSessionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.display_name, "Eve");
        assert_eq!(req.kind, SessionKind::Human);
        assert_eq!(req.role, SessionRole::Operator);
    }

    #[test]
    fn join_request_with_ai_kind() {
        let json = r#"{"display_name": "Copilot", "kind": "ai", "role": "operator"}"#;
        let req: JoinSessionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.kind, SessionKind::Ai);
    }
}
