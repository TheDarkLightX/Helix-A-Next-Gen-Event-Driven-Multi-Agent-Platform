// crates/helix-core/src/state/in_memory_store.rs

use crate::errors::HelixError;
use crate::state::{StateStore, StoredState};
use crate::types::{AgentId, ProfileId};
use async_trait::async_trait;
use chrono::Utc;
use serde_json; // For JsonValue and serialization
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Creates a composite string key from ProfileId and AgentId.
fn create_key(profile_id: &ProfileId, agent_id: &AgentId) -> String {
    format!("{}:{}", profile_id, agent_id)
}

/// An in-memory implementation of the `StateStore` trait.
///
/// This store uses a thread-safe `HashMap` to keep agent states in memory.
/// The keys are composite strings of `profile_id` and `agent_id`, and
/// values are `serde_json`-serialized `StoredState` objects.
#[derive(Debug, Clone)]
pub struct InMemoryStateStore {
    states: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl InMemoryStateStore {
    /// Creates a new, empty `InMemoryStateStore`.
    pub fn new() -> Self {
        Self {
            states: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryStateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StateStore for InMemoryStateStore {
    async fn get_state(
        &self,
        profile_id: &ProfileId,
        agent_id: &AgentId,
    ) -> Result<Option<serde_json::Value>, HelixError> {
        let key = create_key(profile_id, agent_id);
        let store = self.states.lock().map_err(|e| {
            HelixError::InternalError(format!("Failed to acquire lock for get_state: {}", e))
        })?;

        match store.get(&key) {
            Some(bytes) => {
                let stored_state: StoredState = serde_json::from_slice(bytes)
                    .map_err(|e| HelixError::validation_error("state.deserialization", &format!("Deserialization failed for get_state: {}", e)))?;
                Ok(Some(stored_state.data))
            }
            None => Ok(None),
        }
    }

    async fn set_state(
        &self,
        profile_id: &ProfileId,
        agent_id: &AgentId,
        state_data: serde_json::Value,
    ) -> Result<(), HelixError> {
        let key = create_key(profile_id, agent_id);
        let mut store = self.states.lock().map_err(|e| {
            HelixError::InternalError(format!("Failed to acquire lock for set_state: {}", e))
        })?;

        let current_stored_state_opt: Option<StoredState> =
            store.get(&key).and_then(|bytes| serde_json::from_slice(bytes).ok());

        let stored_state_to_save = match current_stored_state_opt {
            Some(mut existing_state) => {
                // Ensure consistency if somehow key pointed to a different agent/profile
                if &existing_state.profile_id != profile_id || &existing_state.agent_id != agent_id {
                    return Err(HelixError::InternalError(
                        "Key mismatch during state update".to_string(),
                    ));
                }
                existing_state.data = state_data;
                existing_state.updated_at = Utc::now();
                existing_state
            }
            None => StoredState::new(*profile_id, *agent_id, state_data),
        };

        let bytes = serde_json::to_vec(&stored_state_to_save)
            .map_err(|e| HelixError::validation_error("state.serialization", &format!("Serialization failed for set_state: {}", e)))?;
        store.insert(key, bytes);
        Ok(())
    }

    async fn delete_state(
        &self,
        profile_id: &ProfileId,
        agent_id: &AgentId,
    ) -> Result<bool, HelixError> {
        let key = create_key(profile_id, agent_id);
        let mut store = self.states.lock().map_err(|e| {
            HelixError::InternalError(format!("Failed to acquire lock for delete_state: {}", e))
        })?;
        Ok(store.remove(&key).is_some())
    }

    async fn list_agent_ids(&self, profile_id: &ProfileId) -> Result<Vec<AgentId>, HelixError> {
        let store = self.states.lock().map_err(|e| {
            HelixError::InternalError(format!("Failed to acquire lock for list_agent_ids: {}", e))
        })?;
        let prefix_to_match = format!("{}:", profile_id);
        let mut agent_ids = Vec::new();

        for key_str in store.keys() {
            if key_str.starts_with(&prefix_to_match) {
                let agent_id_str = &key_str[prefix_to_match.len()..];
                match Uuid::parse_str(agent_id_str) {
                    Ok(agent_uuid) => agent_ids.push(agent_uuid),
                    Err(e) => {
                        // Log or handle malformed key. This indicates an internal issue.
                        // For now, returning an error might be too disruptive if one key is bad.
                        // Consider logging and skipping.
                        eprintln!(
                            "Warning: Malformed agent_id in key '{}' during list_agent_ids: {}",
                            key_str, e
                        );
                        // Optionally, return an error:
                        // return Err(HelixError::InternalError(format!("Malformed key in store: {}", key_str)));
                    }
                }
            }
        }
        Ok(agent_ids)
    }

    async fn get_stored_state(
        &self,
        profile_id: &ProfileId,
        agent_id: &AgentId,
    ) -> Result<Option<StoredState>, HelixError> {
        let key = create_key(profile_id, agent_id);
        let store = self.states.lock().map_err(|e| {
            HelixError::InternalError(format!("Failed to acquire lock for get_stored_state: {}", e))
        })?;

        match store.get(&key) {
            Some(bytes) => {
                let stored_state: StoredState = serde_json::from_slice(bytes).map_err(|e| {
                    HelixError::validation_error("state.deserialization", &format!("Deserialization failed for get_stored_state: {}", e))
                })?;
                // Verify consistency, though create_key should ensure this.
                if &stored_state.profile_id == profile_id && &stored_state.agent_id == agent_id {
                    Ok(Some(stored_state))
                } else {
                    Err(HelixError::InternalError(
                        "Key mismatch during StoredState retrieval".to_string(),
                    ))
                }
            }
            None => Ok(None),
        }
    }

    async fn merge_state(
        &self,
        profile_id: &ProfileId,
        agent_id: &AgentId,
        data_to_merge: serde_json::Value,
    ) -> Result<(), HelixError> {
        let key = create_key(profile_id, agent_id);
        let mut store = self.states.lock().map_err(|e| {
            HelixError::InternalError(format!("Failed to acquire lock for merge_state: {}", e))
        })?;

        match store.get_mut(&key) {
            Some(bytes_val) => { // bytes_val is &mut Vec<u8>
                let mut stored_state: StoredState = serde_json::from_slice(bytes_val).map_err(|e| {
                    HelixError::validation_error("state.deserialization", &format!("Deserialization failed for merge_state (existing): {}", e))
                })?;

                if &stored_state.profile_id != profile_id || &stored_state.agent_id != agent_id {
                    return Err(HelixError::InternalError(
                        "Key mismatch during state merge".to_string(),
                    ));
                }

                stored_state.merge_data(data_to_merge)?; // This updates stored_state.updated_at

                let updated_bytes = serde_json::to_vec(&stored_state).map_err(|e| {
                    HelixError::validation_error("state.serialization", &format!("Serialization failed for merge_state (update): {}", e))
                })?;
                *bytes_val = updated_bytes;
            }
            None => {
                // Key doesn't exist, create new state
                let new_state = StoredState::new(*profile_id, *agent_id, data_to_merge);
                let bytes = serde_json::to_vec(&new_state).map_err(|e| {
                    HelixError::validation_error("state.serialization", &format!("Serialization failed for merge_state (new): {}", e))
                })?;
                store.insert(key, bytes);
            }
        }
        Ok(())
    }

    async fn clear_profile_state(&self, profile_id: &ProfileId) -> Result<u64, HelixError> {
        let mut store = self.states.lock().map_err(|e| {
            HelixError::InternalError(format!("Failed to acquire lock for clear_profile_state: {}", e))
        })?;
        let prefix_to_match = format!("{}:", profile_id);
        let mut keys_to_remove = Vec::new();

        for key_str in store.keys() {
            if key_str.starts_with(&prefix_to_match) {
                keys_to_remove.push(key_str.clone());
            }
        }

        let mut count = 0;
        for key_to_remove in keys_to_remove {
            if store.remove(&key_to_remove).is_some() {
                count += 1;
            }
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AgentId, ProfileId};
    use serde_json::json;
    use std::time::Duration;
    use tokio::time::sleep;

    fn new_profile_id() -> ProfileId {
        Uuid::new_v4()
    }

    fn new_agent_id() -> AgentId {
        Uuid::new_v4()
    }

    #[tokio::test]
    async fn test_new_store_is_empty() {
        let store = InMemoryStateStore::new();
        let profile_id = new_profile_id();
        let agent_id = new_agent_id();
        assert!(store.get_state(&profile_id, &agent_id).await.unwrap().is_none());
        assert!(store.list_agent_ids(&profile_id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_set_and_get_state() {
        let store = InMemoryStateStore::new();
        let profile_id = new_profile_id();
        let agent_id = new_agent_id();
        let state_data = json!({"key": "value"});

        store
            .set_state(&profile_id, &agent_id, state_data.clone())
            .await
            .unwrap();

        let retrieved_data = store.get_state(&profile_id, &agent_id).await.unwrap();
        assert_eq!(retrieved_data, Some(state_data));
    }

    #[tokio::test]
    async fn test_get_state_non_existent() {
        let store = InMemoryStateStore::new();
        let profile_id = new_profile_id();
        let agent_id = new_agent_id();
        assert!(store.get_state(&profile_id, &agent_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_set_state_overwrites() {
        let store = InMemoryStateStore::new();
        let profile_id = new_profile_id();
        let agent_id = new_agent_id();
        let initial_data = json!({"count": 1});
        let updated_data = json!({"count": 2});

        store
            .set_state(&profile_id, &agent_id, initial_data.clone())
            .await
            .unwrap();
        let first_stored = store.get_stored_state(&profile_id, &agent_id).await.unwrap().unwrap();
        
        sleep(Duration::from_millis(10)).await; // Ensure time passes for updated_at

        store
            .set_state(&profile_id, &agent_id, updated_data.clone())
            .await
            .unwrap();
        
        let retrieved_data = store.get_state(&profile_id, &agent_id).await.unwrap();
        assert_eq!(retrieved_data, Some(updated_data.clone()));

        let second_stored = store.get_stored_state(&profile_id, &agent_id).await.unwrap().unwrap();
        assert_eq!(second_stored.data, updated_data);
        assert_eq!(second_stored.profile_id, profile_id);
        assert_eq!(second_stored.agent_id, agent_id);
        assert_eq!(second_stored.created_at, first_stored.created_at, "created_at should be preserved on update");
        assert!(second_stored.updated_at > first_stored.updated_at, "updated_at should be newer");
    }

    #[tokio::test]
    async fn test_delete_state() {
        let store = InMemoryStateStore::new();
        let profile_id = new_profile_id();
        let agent_id = new_agent_id();
        let state_data = json!({"to_delete": true});

        store
            .set_state(&profile_id, &agent_id, state_data.clone())
            .await
            .unwrap();
        assert!(store.get_state(&profile_id, &agent_id).await.unwrap().is_some());

        let deleted = store.delete_state(&profile_id, &agent_id).await.unwrap();
        assert!(deleted);
        assert!(store.get_state(&profile_id, &agent_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_delete_state_non_existent() {
        let store = InMemoryStateStore::new();
        let profile_id = new_profile_id();
        let agent_id = new_agent_id();
        let deleted = store.delete_state(&profile_id, &agent_id).await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_list_agent_ids() {
        let store = InMemoryStateStore::new();
        let profile1 = new_profile_id();
        let profile2 = new_profile_id();
        let agent1_p1 = new_agent_id();
        let agent2_p1 = new_agent_id();
        let agent1_p2 = new_agent_id();

        store.set_state(&profile1, &agent1_p1, json!({})).await.unwrap();
        store.set_state(&profile1, &agent2_p1, json!({})).await.unwrap();
        store.set_state(&profile2, &agent1_p2, json!({})).await.unwrap();

        let p1_agents = store.list_agent_ids(&profile1).await.unwrap();
        assert_eq!(p1_agents.len(), 2);
        assert!(p1_agents.contains(&agent1_p1));
        assert!(p1_agents.contains(&agent2_p1));

        let p2_agents = store.list_agent_ids(&profile2).await.unwrap();
        assert_eq!(p2_agents.len(), 1);
        assert!(p2_agents.contains(&agent1_p2));

        let empty_profile_agents = store.list_agent_ids(&new_profile_id()).await.unwrap();
        assert!(empty_profile_agents.is_empty());
    }
    
    #[tokio::test]
    async fn test_get_stored_state() {
        let store = InMemoryStateStore::new();
        let profile_id = new_profile_id();
        let agent_id = new_agent_id();
        let state_data = json!({"meta": "data"});

        let initial_time = Utc::now();
        sleep(Duration::from_millis(10)).await;

        store.set_state(&profile_id, &agent_id, state_data.clone()).await.unwrap();
        
        sleep(Duration::from_millis(10)).await;
        let time_after_set = Utc::now();

        let stored_state_opt = store.get_stored_state(&profile_id, &agent_id).await.unwrap();
        assert!(stored_state_opt.is_some());
        let stored_state = stored_state_opt.unwrap();

        assert_eq!(stored_state.profile_id, profile_id);
        assert_eq!(stored_state.agent_id, agent_id);
        assert_eq!(stored_state.data, state_data);
        assert!(stored_state.created_at > initial_time && stored_state.created_at < time_after_set);
        assert_eq!(stored_state.created_at, stored_state.updated_at);
    }

    #[tokio::test]
    async fn test_merge_state_new() {
        let store = InMemoryStateStore::new();
        let profile_id = new_profile_id();
        let agent_id = new_agent_id();
        let initial_data = json!({"a": 1});

        store.merge_state(&profile_id, &agent_id, initial_data.clone()).await.unwrap();
        
        let state = store.get_state(&profile_id, &agent_id).await.unwrap().unwrap();
        assert_eq!(state, initial_data);

        let stored = store.get_stored_state(&profile_id, &agent_id).await.unwrap().unwrap();
        assert_eq!(stored.data, initial_data);
    }

    #[tokio::test]
    async fn test_merge_state_existing() {
        let store = InMemoryStateStore::new();
        let profile_id = new_profile_id();
        let agent_id = new_agent_id();
        let initial_data = json!({"a": 1, "b": {"x": 10}});
        let merge_data = json!({"b": {"y": 20}, "c": 3}); // "b" will be overwritten

        store.set_state(&profile_id, &agent_id, initial_data.clone()).await.unwrap();
        let first_stored = store.get_stored_state(&profile_id, &agent_id).await.unwrap().unwrap();
        
        sleep(Duration::from_millis(10)).await;

        store.merge_state(&profile_id, &agent_id, merge_data.clone()).await.unwrap();
        
        let expected_data = json!({"a": 1, "b": {"y": 20}, "c": 3});
        let state = store.get_state(&profile_id, &agent_id).await.unwrap().unwrap();
        assert_eq!(state, expected_data);

        let second_stored = store.get_stored_state(&profile_id, &agent_id).await.unwrap().unwrap();
        assert_eq!(second_stored.data, expected_data);
        assert_eq!(second_stored.created_at, first_stored.created_at);
        assert!(second_stored.updated_at > first_stored.updated_at);
    }

    #[tokio::test]
    async fn test_merge_state_non_object_fails_gracefully() {
        let store = InMemoryStateStore::new();
        let profile_id = new_profile_id();
        let agent_id = new_agent_id();
        let initial_data = json!("not an object"); // StoredState.merge_data expects object
        
        store.set_state(&profile_id, &agent_id, initial_data.clone()).await.unwrap();
        
        let merge_data = json!({"a": 1});
        let result = store.merge_state(&profile_id, &agent_id, merge_data).await;
        
        assert!(result.is_err());
        match result.err().unwrap() {
            HelixError::ValidationError { context, message } => {
                assert_eq!(context, "StoredState.merge");
                assert!(message.contains("Can only merge object states"));
            }
            _ => panic!("Expected ValidationError"),
        }
    }

    #[tokio::test]
    async fn test_clear_profile_state() {
        let store = InMemoryStateStore::new();
        let profile1 = new_profile_id();
        let profile2 = new_profile_id();
        let agent1_p1 = new_agent_id();
        let agent2_p1 = new_agent_id();
        let agent1_p2 = new_agent_id();

        store.set_state(&profile1, &agent1_p1, json!({})).await.unwrap();
        store.set_state(&profile1, &agent2_p1, json!({})).await.unwrap();
        store.set_state(&profile2, &agent1_p2, json!({})).await.unwrap();

        let cleared_count = store.clear_profile_state(&profile1).await.unwrap();
        assert_eq!(cleared_count, 2);

        assert!(store.get_state(&profile1, &agent1_p1).await.unwrap().is_none());
        assert!(store.get_state(&profile1, &agent2_p1).await.unwrap().is_none());
        assert!(store.list_agent_ids(&profile1).await.unwrap().is_empty());
        
        // Profile2 should be unaffected
        assert!(store.get_state(&profile2, &agent1_p2).await.unwrap().is_some());
        assert_eq!(store.list_agent_ids(&profile2).await.unwrap().len(), 1);

        // Clear already empty profile
        let cleared_again_count = store.clear_profile_state(&profile1).await.unwrap();
        assert_eq!(cleared_again_count, 0);

        // Clear profile2
        let cleared_p2_count = store.clear_profile_state(&profile2).await.unwrap();
        assert_eq!(cleared_p2_count, 1);
        assert!(store.list_agent_ids(&profile2).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_timestamps_on_set_and_merge() {
        let store = InMemoryStateStore::new();
        let profile_id = new_profile_id();
        let agent_id = new_agent_id();

        // Initial set
        store.set_state(&profile_id, &agent_id, json!({"v": 1})).await.unwrap();
        let s1 = store.get_stored_state(&profile_id, &agent_id).await.unwrap().unwrap();
        assert_eq!(s1.created_at, s1.updated_at);

        sleep(Duration::from_millis(20)).await;

        // Second set (update)
        store.set_state(&profile_id, &agent_id, json!({"v": 2})).await.unwrap();
        let s2 = store.get_stored_state(&profile_id, &agent_id).await.unwrap().unwrap();
        assert_eq!(s2.created_at, s1.created_at, "created_at should persist");
        assert!(s2.updated_at > s1.updated_at, "updated_at should advance on set");
        assert!(s2.updated_at > s2.created_at);
        
        sleep(Duration::from_millis(20)).await;

        // Merge
        store.merge_state(&profile_id, &agent_id, json!({"v": 3, "new": true})).await.unwrap();
        let s3 = store.get_stored_state(&profile_id, &agent_id).await.unwrap().unwrap();
        assert_eq!(s3.created_at, s2.created_at, "created_at should persist on merge");
        assert!(s3.updated_at > s2.updated_at, "updated_at should advance on merge");
        assert_eq!(s3.data, json!({"v": 3, "new": true}));
    }
}