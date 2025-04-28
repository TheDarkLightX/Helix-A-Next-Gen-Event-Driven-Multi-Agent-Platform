// crates/helix-core/src/state.rs

//! Defines structures related to agent state persistence.

use crate::errors::HelixError;
use crate::types::{AgentId, ProfileId};
use async_trait::async_trait;
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

/// Trait defining the interface for agent state persistence.
///
/// Implementations will handle the actual storage mechanism (e.g., database, in-memory).
#[async_trait]
pub trait StateStore: Send + Sync {
    /// Retrieves the persisted state for a given agent within a profile.
    ///
    /// Returns `Ok(None)` if no state is found for the agent.
    async fn get_state(
        &self,
        profile_id: &ProfileId,
        agent_id: &AgentId,
    ) -> Result<Option<JsonValue>, HelixError>;

    /// Persists the state for a given agent within a profile.
    ///
    /// This typically overwrites any existing state for the agent.
    async fn set_state(
        &self,
        profile_id: &ProfileId,
        agent_id: &AgentId,
        state: JsonValue,
    ) -> Result<(), HelixError>;

    // TODO: Consider adding methods for:
    // - Deleting state?
    // - Batch operations?
    // - Versioning/optimistic locking?
}

// Potential additions:
// - Error types specific to state storage (could be added to HelixError)

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AgentId, ProfileId};
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use uuid::Uuid;

    // Mock StateStore Implementation
    struct MockStateStore {
        states: Mutex<HashMap<(ProfileId, AgentId), JsonValue>>,
    }

    impl MockStateStore {
        fn new() -> Self {
            Self {
                states: Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl StateStore for MockStateStore {
        async fn get_state(
            &self,
            profile_id: &ProfileId,
            agent_id: &AgentId,
        ) -> Result<Option<JsonValue>, HelixError> {
            let store = self.states.lock().unwrap();
            Ok(store.get(&(*profile_id, *agent_id)).cloned())
        }

        async fn set_state(
            &self,
            profile_id: &ProfileId,
            agent_id: &AgentId,
            state: JsonValue,
        ) -> Result<(), HelixError> {
            let mut store = self.states.lock().unwrap();
            store.insert((*profile_id, *agent_id), state);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_mock_state_store_logic() {
        let store = MockStateStore::new();
        let profile_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let state_data = json!({ "counter": 10, "last_run": "2024-01-01T12:00:00Z" });

        // 1. Test initial get (should be None)
        let initial_state = store.get_state(&profile_id, &agent_id).await.unwrap();
        assert!(initial_state.is_none());

        // 2. Test set
        store
            .set_state(&profile_id, &agent_id, state_data.clone())
            .await
            .unwrap();

        // 3. Test get after set
        let retrieved_state = store.get_state(&profile_id, &agent_id).await.unwrap();
        assert_eq!(retrieved_state, Some(state_data.clone()));

        // 4. Test overwrite
        let new_state_data = json!({ "counter": 11 });
        store
            .set_state(&profile_id, &agent_id, new_state_data.clone())
            .await
            .unwrap();
        let overwritten_state = store.get_state(&profile_id, &agent_id).await.unwrap();
        assert_eq!(overwritten_state, Some(new_state_data));
    }
}
