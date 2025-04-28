// crates/helix-core/src/state.rs

//! Defines structures related to agent state persistence.

use crate::types::{AgentId, ProfileId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Represents the persisted state of an agent retrieved from a StateStore.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredState {
    /// The ID of the profile (tenant) this state belongs to.
    pub profile_id: ProfileId,
    /// The ID of the agent this state belongs to.
    pub agent_id: AgentId,
    /// The actual persisted state data.
    /// The interpretation of this value depends on the specific agent.
    pub data: JsonValue,
    /// Timestamp when the state was initially created or last overwritten.
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
    /// Timestamp when the state was last updated.
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
    // pub version: Option<u64>,
}

impl StoredState {
    /// Creates a new StoredState instance.
    pub fn new(profile_id: ProfileId, agent_id: AgentId, data: JsonValue) -> Self {
        let now = Utc::now();
        Self {
            profile_id,
            agent_id,
            data,
            created_at: now,
            updated_at: now,
            // version: None, // Initialize version if using
        }
    }
}

// Potential additions:
// - Error types specific to state storage (could be added to HelixError)
// - Traits for state serialization/deserialization if not using JsonValue directly.
