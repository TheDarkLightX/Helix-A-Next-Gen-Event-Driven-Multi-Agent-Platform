// Copyright 2024 Helix Platform
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


// crates/helix-core/src/state.rs

//! Defines structures related to agent state persistence.

use crate::errors::HelixError;
use crate::types::{AgentId, ProfileId};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

pub mod in_memory_store; // Add this line
pub use in_memory_store::InMemoryStateStore; // Add this line

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

    /// Updates the state data and timestamp
    pub fn update_data(&mut self, data: JsonValue) {
        self.data = data;
        self.updated_at = Utc::now();
    }

    /// Gets the age of the state in seconds
    pub fn age_seconds(&self) -> i64 {
        (Utc::now() - self.updated_at).num_seconds()
    }

    /// Checks if the state is older than the specified duration in seconds
    pub fn is_older_than(&self, seconds: i64) -> bool {
        self.age_seconds() > seconds
    }

    /// Gets the size of the state data in bytes (approximate)
    pub fn size_bytes(&self) -> usize {
        self.data.to_string().len()
    }

    /// Checks if the state data is empty
    pub fn is_empty(&self) -> bool {
        match &self.data {
            JsonValue::Null => true,
            JsonValue::Object(map) => map.is_empty(),
            JsonValue::Array(arr) => arr.is_empty(),
            JsonValue::String(s) => s.is_empty(),
            _ => false,
        }
    }

    /// Validates the state structure
    pub fn validate(&self) -> Result<(), HelixError> {
        // Check that the data is valid JSON (it should be by construction)
        if self.data.is_null() {
            return Err(HelixError::ValidationError {
                context: "StoredState.data".to_string(),
                message: "State data cannot be null".to_string(),
            });
        }

        // Check timestamps are valid
        if self.updated_at < self.created_at {
            return Err(HelixError::ValidationError {
                context: "StoredState.timestamps".to_string(),
                message: "Updated timestamp cannot be before created timestamp".to_string(),
            });
        }

        Ok(())
    }

    /// Merges another state's data into this state (shallow merge for objects)
    pub fn merge_data(&mut self, other_data: JsonValue) -> Result<(), HelixError> {
        match (&mut self.data, other_data) {
            (JsonValue::Object(ref mut map1), JsonValue::Object(map2)) => {
                for (key, value) in map2 {
                    map1.insert(key, value);
                }
                self.updated_at = Utc::now();
                Ok(())
            }
            _ => Err(HelixError::ValidationError {
                context: "StoredState.merge".to_string(),
                message: "Can only merge object states".to_string(),
            }),
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

    /// Deletes the state for a given agent within a profile.
    ///
    /// Returns `Ok(true)` if state was deleted, `Ok(false)` if no state existed.
    async fn delete_state(
        &self,
        profile_id: &ProfileId,
        agent_id: &AgentId,
    ) -> Result<bool, HelixError>;

    /// Lists all agent IDs that have state within a profile.
    async fn list_agent_ids(&self, profile_id: &ProfileId) -> Result<Vec<AgentId>, HelixError>;

    /// Gets the full StoredState object (with metadata) for an agent.
    async fn get_stored_state(
        &self,
        profile_id: &ProfileId,
        agent_id: &AgentId,
    ) -> Result<Option<StoredState>, HelixError>;

    /// Updates existing state by merging with new data.
    ///
    /// If no state exists, creates new state with the provided data.
    async fn merge_state(
        &self,
        profile_id: &ProfileId,
        agent_id: &AgentId,
        data: JsonValue,
    ) -> Result<(), HelixError>;

    /// Clears all state for a profile.
    async fn clear_profile_state(&self, profile_id: &ProfileId) -> Result<u64, HelixError>;

    // TODO: Consider adding methods for:
    // - Batch operations?
    // - Versioning/optimistic locking?
    // - State history/audit trail?
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
        states: Mutex<HashMap<(ProfileId, AgentId), StoredState>>,
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
            Ok(store.get(&(*profile_id, *agent_id)).map(|s| s.data.clone()))
        }

        async fn set_state(
            &self,
            profile_id: &ProfileId,
            agent_id: &AgentId,
            state: JsonValue,
        ) -> Result<(), HelixError> {
            let mut store = self.states.lock().unwrap();
            let stored_state = StoredState::new(*profile_id, *agent_id, state);
            store.insert((*profile_id, *agent_id), stored_state);
            Ok(())
        }

        async fn delete_state(
            &self,
            profile_id: &ProfileId,
            agent_id: &AgentId,
        ) -> Result<bool, HelixError> {
            let mut store = self.states.lock().unwrap();
            Ok(store.remove(&(*profile_id, *agent_id)).is_some())
        }

        async fn list_agent_ids(&self, profile_id: &ProfileId) -> Result<Vec<AgentId>, HelixError> {
            let store = self.states.lock().unwrap();
            let agent_ids: Vec<AgentId> = store
                .keys()
                .filter(|(pid, _)| pid == profile_id)
                .map(|(_, aid)| *aid)
                .collect();
            Ok(agent_ids)
        }

        async fn get_stored_state(
            &self,
            profile_id: &ProfileId,
            agent_id: &AgentId,
        ) -> Result<Option<StoredState>, HelixError> {
            let store = self.states.lock().unwrap();
            Ok(store.get(&(*profile_id, *agent_id)).cloned())
        }

        async fn merge_state(
            &self,
            profile_id: &ProfileId,
            agent_id: &AgentId,
            data: JsonValue,
        ) -> Result<(), HelixError> {
            let mut store = self.states.lock().unwrap();
            match store.get_mut(&(*profile_id, *agent_id)) {
                Some(stored_state) => {
                    stored_state.merge_data(data)?;
                }
                None => {
                    let stored_state = StoredState::new(*profile_id, *agent_id, data);
                    store.insert((*profile_id, *agent_id), stored_state);
                }
            }
            Ok(())
        }

        async fn clear_profile_state(&self, profile_id: &ProfileId) -> Result<u64, HelixError> {
            let mut store = self.states.lock().unwrap();
            let keys_to_remove: Vec<_> = store
                .keys()
                .filter(|(pid, _)| pid == profile_id)
                .cloned()
                .collect();

            let count = keys_to_remove.len() as u64;
            for key in keys_to_remove {
                store.remove(&key);
            }
            Ok(count)
        }
    }

    #[test]
    fn test_stored_state_creation() {
        let profile_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let data = json!({"key": "value"});

        let state = StoredState::new(profile_id, agent_id, data.clone());

        assert_eq!(state.profile_id, profile_id);
        assert_eq!(state.agent_id, agent_id);
        assert_eq!(state.data, data);
        assert!(state.created_at <= state.updated_at);
    }

    #[test]
    fn test_stored_state_update_data() {
        let mut state = StoredState::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            json!({"counter": 1}),
        );

        let original_updated = state.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(1));

        let new_data = json!({"counter": 2});
        state.update_data(new_data.clone());

        assert_eq!(state.data, new_data);
        assert!(state.updated_at > original_updated);
    }

    #[test]
    fn test_stored_state_age_and_older_than() {
        let mut state = StoredState::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            json!({}),
        );

        // Manually set updated_at to 10 seconds ago
        state.updated_at = Utc::now() - chrono::Duration::seconds(10);

        assert!(state.age_seconds() >= 9); // Allow for some timing variance
        assert!(state.is_older_than(5));
        assert!(!state.is_older_than(15));
    }

    #[test]
    fn test_stored_state_size_bytes() {
        let state = StoredState::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            json!({"key": "value"}),
        );

        assert!(state.size_bytes() > 0);
        assert!(state.size_bytes() < 100); // Should be small for this simple object
    }

    #[test]
    fn test_stored_state_is_empty() {
        let empty_states = vec![
            json!(null),
            json!({}),
            json!([]),
            json!(""),
        ];

        for data in empty_states {
            let state = StoredState::new(Uuid::new_v4(), Uuid::new_v4(), data);
            assert!(state.is_empty(), "Expected empty state for data: {:?}", state.data);
        }

        let non_empty_states = vec![
            json!({"key": "value"}),
            json!([1, 2, 3]),
            json!("hello"),
            json!(42),
            json!(true),
        ];

        for data in non_empty_states {
            let state = StoredState::new(Uuid::new_v4(), Uuid::new_v4(), data);
            assert!(!state.is_empty(), "Expected non-empty state for data: {:?}", state.data);
        }
    }

    #[test]
    fn test_stored_state_validate() {
        let valid_state = StoredState::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            json!({"valid": true}),
        );
        assert!(valid_state.validate().is_ok());

        let null_state = StoredState::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            json!(null),
        );
        let result = null_state.validate();
        assert!(result.is_err());
        if let Err(HelixError::ValidationError { context, message }) = result {
            assert_eq!(context, "StoredState.data");
            assert!(message.contains("cannot be null"));
        }

        let mut invalid_timestamp_state = StoredState::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            json!({}),
        );
        invalid_timestamp_state.updated_at = invalid_timestamp_state.created_at - chrono::Duration::seconds(1);

        let result = invalid_timestamp_state.validate();
        assert!(result.is_err());
        if let Err(HelixError::ValidationError { context, message }) = result {
            assert_eq!(context, "StoredState.timestamps");
            assert!(message.contains("Updated timestamp cannot be before created timestamp"));
        }
    }

    #[test]
    fn test_stored_state_merge_data() {
        let mut state = StoredState::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            json!({"a": 1, "b": 2}),
        );

        let merge_data = json!({"b": 3, "c": 4});
        let result = state.merge_data(merge_data);
        assert!(result.is_ok());

        let expected = json!({"a": 1, "b": 3, "c": 4});
        assert_eq!(state.data, expected);

        // Test merging non-object data should fail
        let mut string_state = StoredState::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            json!("string"),
        );

        let result = string_state.merge_data(json!({"key": "value"}));
        assert!(result.is_err());
        if let Err(HelixError::ValidationError { context, message }) = result {
            assert_eq!(context, "StoredState.merge");
            assert!(message.contains("Can only merge object states"));
        }
    }

    #[test]
    fn test_stored_state_serialization() {
        let state = StoredState::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            json!({"test": "data"}),
        );

        let serialized = serde_json::to_string(&state).expect("Failed to serialize");
        let deserialized: StoredState = serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(state.profile_id, deserialized.profile_id);
        assert_eq!(state.agent_id, deserialized.agent_id);
        assert_eq!(state.data, deserialized.data);
    }

    #[test]
    fn test_stored_state_equality() {
        let profile_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let data = json!({"test": true});

        let state1 = StoredState::new(profile_id, agent_id, data.clone());
        let state2 = StoredState::new(profile_id, agent_id, data.clone());

        // States with same data should be equal (ignoring timestamps)
        assert_eq!(state1.profile_id, state2.profile_id);
        assert_eq!(state1.agent_id, state2.agent_id);
        assert_eq!(state1.data, state2.data);

        let state3 = StoredState::new(Uuid::new_v4(), agent_id, data);
        assert_ne!(state1.profile_id, state3.profile_id);
    }

    #[tokio::test]
    async fn test_mock_state_store_basic_operations() {
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

    #[tokio::test]
    async fn test_mock_state_store_delete() {
        let store = MockStateStore::new();
        let profile_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let state_data = json!({"test": "data"});

        // Test delete non-existent state
        let deleted = store.delete_state(&profile_id, &agent_id).await.unwrap();
        assert!(!deleted);

        // Set state and then delete
        store.set_state(&profile_id, &agent_id, state_data).await.unwrap();
        let deleted = store.delete_state(&profile_id, &agent_id).await.unwrap();
        assert!(deleted);

        // Verify state is gone
        let state = store.get_state(&profile_id, &agent_id).await.unwrap();
        assert!(state.is_none());

        // Test delete again (should return false)
        let deleted = store.delete_state(&profile_id, &agent_id).await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_mock_state_store_list_agent_ids() {
        let store = MockStateStore::new();
        let profile1 = Uuid::new_v4();
        let profile2 = Uuid::new_v4();
        let agent1 = Uuid::new_v4();
        let agent2 = Uuid::new_v4();
        let agent3 = Uuid::new_v4();

        // Initially empty
        let agents = store.list_agent_ids(&profile1).await.unwrap();
        assert!(agents.is_empty());

        // Add states for different profiles
        store.set_state(&profile1, &agent1, json!({"p1a1": true})).await.unwrap();
        store.set_state(&profile1, &agent2, json!({"p1a2": true})).await.unwrap();
        store.set_state(&profile2, &agent3, json!({"p2a3": true})).await.unwrap();

        // Check profile1 agents
        let mut agents1 = store.list_agent_ids(&profile1).await.unwrap();
        agents1.sort();
        let mut expected1 = vec![agent1, agent2];
        expected1.sort();
        assert_eq!(agents1, expected1);

        // Check profile2 agents
        let agents2 = store.list_agent_ids(&profile2).await.unwrap();
        assert_eq!(agents2, vec![agent3]);

        // Check empty profile
        let empty_profile = Uuid::new_v4();
        let agents_empty = store.list_agent_ids(&empty_profile).await.unwrap();
        assert!(agents_empty.is_empty());
    }

    #[tokio::test]
    async fn test_mock_state_store_get_stored_state() {
        let store = MockStateStore::new();
        let profile_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let state_data = json!({"metadata": "test"});

        // Test get non-existent stored state
        let stored_state = store.get_stored_state(&profile_id, &agent_id).await.unwrap();
        assert!(stored_state.is_none());

        // Set state and get stored state
        store.set_state(&profile_id, &agent_id, state_data.clone()).await.unwrap();
        let stored_state = store.get_stored_state(&profile_id, &agent_id).await.unwrap();

        assert!(stored_state.is_some());
        let state = stored_state.unwrap();
        assert_eq!(state.profile_id, profile_id);
        assert_eq!(state.agent_id, agent_id);
        assert_eq!(state.data, state_data);
        assert!(state.created_at <= state.updated_at);
    }

    #[tokio::test]
    async fn test_mock_state_store_merge_state() {
        let store = MockStateStore::new();
        let profile_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();

        // Test merge into non-existent state (should create new)
        let initial_data = json!({"a": 1, "b": 2});
        store.merge_state(&profile_id, &agent_id, initial_data.clone()).await.unwrap();

        let state = store.get_state(&profile_id, &agent_id).await.unwrap();
        assert_eq!(state, Some(initial_data));

        // Test merge into existing state
        let merge_data = json!({"b": 3, "c": 4});
        store.merge_state(&profile_id, &agent_id, merge_data).await.unwrap();

        let merged_state = store.get_state(&profile_id, &agent_id).await.unwrap();
        let expected = json!({"a": 1, "b": 3, "c": 4});
        assert_eq!(merged_state, Some(expected));
    }

    #[tokio::test]
    async fn test_mock_state_store_clear_profile_state() {
        let store = MockStateStore::new();
        let profile1 = Uuid::new_v4();
        let profile2 = Uuid::new_v4();
        let agent1 = Uuid::new_v4();
        let agent2 = Uuid::new_v4();
        let agent3 = Uuid::new_v4();

        // Test clear empty profile
        let cleared = store.clear_profile_state(&profile1).await.unwrap();
        assert_eq!(cleared, 0);

        // Add states for multiple profiles
        store.set_state(&profile1, &agent1, json!({"p1a1": true})).await.unwrap();
        store.set_state(&profile1, &agent2, json!({"p1a2": true})).await.unwrap();
        store.set_state(&profile2, &agent3, json!({"p2a3": true})).await.unwrap();

        // Clear profile1
        let cleared = store.clear_profile_state(&profile1).await.unwrap();
        assert_eq!(cleared, 2);

        // Verify profile1 states are gone
        let agents1 = store.list_agent_ids(&profile1).await.unwrap();
        assert!(agents1.is_empty());

        // Verify profile2 states remain
        let agents2 = store.list_agent_ids(&profile2).await.unwrap();
        assert_eq!(agents2, vec![agent3]);

        // Clear profile1 again (should return 0)
        let cleared = store.clear_profile_state(&profile1).await.unwrap();
        assert_eq!(cleared, 0);
    }

    #[tokio::test]
    async fn test_mock_state_store_multiple_profiles() {
        let store = MockStateStore::new();
        let profile1 = Uuid::new_v4();
        let profile2 = Uuid::new_v4();
        let agent_id = Uuid::new_v4(); // Same agent ID in different profiles

        let data1 = json!({"profile": 1});
        let data2 = json!({"profile": 2});

        // Set same agent ID in different profiles
        store.set_state(&profile1, &agent_id, data1.clone()).await.unwrap();
        store.set_state(&profile2, &agent_id, data2.clone()).await.unwrap();

        // Verify isolation
        let state1 = store.get_state(&profile1, &agent_id).await.unwrap();
        let state2 = store.get_state(&profile2, &agent_id).await.unwrap();

        assert_eq!(state1, Some(data1));
        assert_eq!(state2, Some(data2));
    }

    #[tokio::test]
    async fn test_mock_state_store_large_data() {
        let store = MockStateStore::new();
        let profile_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();

        // Create large state data
        let large_array: Vec<i32> = (0..1000).collect();
        let large_data = json!({"large_array": large_array, "metadata": "test"});

        store.set_state(&profile_id, &agent_id, large_data.clone()).await.unwrap();
        let retrieved = store.get_state(&profile_id, &agent_id).await.unwrap();

        assert_eq!(retrieved, Some(large_data));
    }

    #[tokio::test]
    async fn test_mock_state_store_unicode_data() {
        let store = MockStateStore::new();
        let profile_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();

        let unicode_data = json!({
            "chinese": "你好世界",
            "emoji": "🚀🌟💫",
            "arabic": "مرحبا بالعالم",
            "mixed": "Hello 世界 🌍"
        });

        store.set_state(&profile_id, &agent_id, unicode_data.clone()).await.unwrap();
        let retrieved = store.get_state(&profile_id, &agent_id).await.unwrap();

        assert_eq!(retrieved, Some(unicode_data));
    }

    #[tokio::test]
    async fn test_mock_state_store_complex_json() {
        let store = MockStateStore::new();
        let profile_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();

        let complex_data = json!({
            "nested": {
                "deep": {
                    "value": 42
                }
            },
            "array": [1, "two", {"three": 3}, [4, 5]],
            "boolean": true,
            "null_value": null,
            "number": std::f64::consts::PI
        });

        store.set_state(&profile_id, &agent_id, complex_data.clone()).await.unwrap();
        let retrieved = store.get_state(&profile_id, &agent_id).await.unwrap();

        assert_eq!(retrieved, Some(complex_data));
    }

    #[test]
    fn test_stored_state_debug_format() {
        let state = StoredState::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            json!({"debug": "test"}),
        );

        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("StoredState"));
        assert!(debug_str.contains("debug"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_stored_state_clone() {
        let original = StoredState::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            json!({"clone": "test"}),
        );

        let cloned = original.clone();
        assert_eq!(original.profile_id, cloned.profile_id);
        assert_eq!(original.agent_id, cloned.agent_id);
        assert_eq!(original.data, cloned.data);
        assert_eq!(original.created_at, cloned.created_at);
        assert_eq!(original.updated_at, cloned.updated_at);
    }

    #[test]
    fn test_stored_state_with_complex_data() {
        let profile_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let complex_data = json!({
            "nested": {
                "array": [1, 2, 3, {"inner": "value"}],
                "boolean": true,
                "null_value": null
            },
            "unicode": "测试数据 🚀",
            "large_number": 9223372036854775807i64,
            "float": std::f64::consts::PI
        });

        let state = StoredState::new(profile_id, agent_id, complex_data.clone());
        assert_eq!(state.data, complex_data);

        // Test serialization with complex data
        let serialized = serde_json::to_string(&state).unwrap();
        let deserialized: StoredState = serde_json::from_str(&serialized).unwrap();
        assert_eq!(state.data, deserialized.data);
    }

    #[test]
    fn test_stored_state_age_calculation() {
        let profile_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let data = json!({"test": "data"});

        let mut state = StoredState::new(profile_id, agent_id, data);

        // Age should be very small initially
        let initial_age = state.age_seconds();
        assert!(initial_age >= 0);
        assert!(initial_age < 2); // Should be less than 2 seconds

        // Manually set an older timestamp
        state.updated_at = Utc::now() - chrono::Duration::seconds(100);
        let older_age = state.age_seconds();
        assert!(older_age >= 100);
        assert!(older_age < 102); // Allow for small timing differences
    }

    #[test]
    fn test_stored_state_is_older_than() {
        let profile_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let data = json!({"test": "data"});

        let mut state = StoredState::new(profile_id, agent_id, data);

        // Fresh state should not be older than 1 second
        assert!(!state.is_older_than(1));

        // Set timestamp to 10 seconds ago
        state.updated_at = Utc::now() - chrono::Duration::seconds(10);

        assert!(state.is_older_than(5));  // Should be older than 5 seconds
        assert!(!state.is_older_than(15)); // Should not be older than 15 seconds
    }

    #[test]
    fn test_stored_state_update_data_comprehensive() {
        let profile_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let initial_data = json!({"initial": "value"});

        let mut state = StoredState::new(profile_id, agent_id, initial_data);
        let original_updated_at = state.updated_at;

        // Small delay to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(1));

        let new_data = json!({"updated": "value"});
        state.update_data(new_data.clone());

        assert_eq!(state.data, new_data);
        assert!(state.updated_at > original_updated_at);
    }

    #[test]
    fn test_stored_state_merge_data_comprehensive() {
        let profile_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let initial_data = json!({
            "existing_key": "existing_value",
            "number": 42,
            "config": {
                "timeout": 30,
                "retries": 3
            }
        });

        let mut state = StoredState::new(profile_id, agent_id, initial_data);

        let merge_data = json!({
            "new_key": "new_value",
            "number": 100,
            "config": {
                "timeout": 60,
                "max_connections": 10
            }
        });

        state.merge_data(merge_data).unwrap();

        // Should have merged properly (shallow merge - config object is replaced)
        assert_eq!(state.data["existing_key"], "existing_value");
        assert_eq!(state.data["new_key"], "new_value");
        assert_eq!(state.data["number"], 100);
        assert_eq!(state.data["config"]["timeout"], 60);
        assert_eq!(state.data["config"]["max_connections"], 10);
        // Note: retries is NOT preserved because merge_data does shallow merge
        assert!(state.data["config"].get("retries").is_none());
    }

    #[test]
    fn test_stored_state_boundary_values() {
        // Test with minimum UUID values
        let min_uuid = Uuid::from_bytes([0; 16]);
        let max_uuid = Uuid::from_bytes([255; 16]);

        let state = StoredState::new(min_uuid, max_uuid, json!({"boundary": "test"}));
        assert_eq!(state.profile_id, min_uuid);
        assert_eq!(state.agent_id, max_uuid);
    }

    #[test]
    fn test_stored_state_timestamp_edge_cases() {
        let profile_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();
        let data = json!({"test": "data"});

        let mut state = StoredState::new(profile_id, agent_id, data);

        // Test with very old timestamp
        state.updated_at = DateTime::parse_from_rfc3339("1970-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let age = state.age_seconds();
        assert!(age > 1_000_000_000); // Should be very old
        assert!(state.is_older_than(1_000_000_000));

        // Test with future timestamp (edge case)
        state.updated_at = Utc::now() + chrono::Duration::seconds(100);
        let future_age = state.age_seconds();
        assert!(future_age < 0); // Should be negative
    }

    #[tokio::test]
    async fn test_mock_state_store_concurrent_access() {
        use std::sync::Arc;
        use tokio::task;

        let store = Arc::new(MockStateStore::new());
        let profile_id = Uuid::new_v4();
        let mut handles = vec![];

        // Spawn multiple tasks that access the store concurrently
        for i in 0..10 {
            let store_clone = Arc::clone(&store);
            let agent_id = Uuid::new_v4();
            let handle = task::spawn(async move {
                let data = json!({"task": i, "data": "test"});
                store_clone.set_state(&profile_id, &agent_id, data.clone()).await.unwrap();

                let retrieved = store_clone.get_state(&profile_id, &agent_id).await.unwrap();
                assert_eq!(retrieved, Some(data));
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify final state
        let states = store.states.lock().unwrap();
        assert_eq!(states.len(), 10);
    }

    #[tokio::test]
    async fn test_mock_state_store_error_scenarios() {
        let store = MockStateStore::new();
        let profile_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();

        // Test with invalid JSON that can't be merged
        let initial_data = json!("not_an_object");
        store.set_state(&profile_id, &agent_id, initial_data).await.unwrap();

        let merge_data = json!({"key": "value"});
        let result = store.merge_state(&profile_id, &agent_id, merge_data).await;

        // Should handle merge errors gracefully
        assert!(result.is_err());
    }

    #[test]
    fn test_stored_state_memory_efficiency() {
        let state = StoredState::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            json!({"test": "data"}),
        );

        let size = std::mem::size_of_val(&state);
        // StoredState should be reasonably sized
        assert!(size < 500, "StoredState is too large: {} bytes", size);
    }
}
