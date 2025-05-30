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


//! Defines the core Agent concept and related traits.

use crate::event::Event;
use crate::types::{AgentId, CredentialId, ProfileId};
use crate::HelixError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Specifies the runtime environment for an agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "agent_runtime_type", rename_all = "lowercase")] // For PostgreSQL enum mapping
pub enum AgentRuntime {
    Native,
    Wasm,
}

impl Default for AgentRuntime {
    fn default() -> Self {
        AgentRuntime::Native
    }
}

// --- Placeholder Traits for Context Dependencies ---

/// Provides access to credentials required by agents.
#[async_trait]
pub trait CredentialProvider: Send + Sync {
    /// Retrieves a credential by its ID.
    /// TODO: Define a proper Credential type instead of String.
    async fn get_credential(&self, id: &str) -> Result<Option<String>, HelixError>;
}

/// Provides access to persistent state for agents.
#[async_trait]
pub trait StateStore: Send + Sync {
    /// Retrieves state associated with a key.
    async fn get_state(&self, key: &str) -> Result<Option<Vec<u8>>, HelixError>;

    /// Stores state associated with a key.
    async fn set_state(&self, key: &str, value: &[u8]) -> Result<(), HelixError>;

    // TODO: Add methods for deleting state, listing keys, etc.?
}

/// Represents the configuration state of an agent instance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::FromRow)] // Added sqlx::FromRow
pub struct AgentConfig {
    /// Unique ID of this agent instance.
    #[sqlx(try_from = "Uuid")] // If AgentId is a type alias for Uuid and sqlx needs explicit mapping
    pub id: AgentId,
    /// ID of the profile (tenant) this agent belongs to.
    #[sqlx(try_from = "Uuid")] // If ProfileId is a type alias for Uuid
    pub profile_id: ProfileId,
    /// Optional friendly name for the agent.
    pub name: Option<String>,
    /// Type or kind identifier for this agent (e.g., "webhook", "rss_poller").
    pub agent_kind: String, // This refers to the functional kind (e.g., "rss_poller")
    /// Specifies whether the agent runs natively or as a WASM module.
    #[sqlx(default)] // Assuming AgentRuntime::Native is the DB default if not specified
    #[serde(default)]
    pub agent_runtime: AgentRuntime,
    /// Path to the WASM module, if `agent_runtime` is `Wasm`.
    #[serde(default)]
    pub wasm_module_path: Option<String>,
    /// JSON blob containing agent-specific configuration.
    #[sqlx(json)] // To map to JSONB
    pub config_data: JsonValue,
    /// IDs of credentials required by this agent.
    #[serde(default)]
    // sqlx should handle Vec<Uuid> for UUID[] if 'uuid' feature is enabled
    pub credential_ids: Vec<CredentialId>,
    /// Indicates if the agent is currently enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// List of AgentIDs that this agent depends on.
    #[serde(default)]
    pub dependencies: Vec<AgentId>,
}

fn default_true() -> bool {
    true
}

impl AgentConfig {
    /// Creates a new AgentConfig instance
    pub fn new(
        id: AgentId,
        profile_id: ProfileId,
        name: Option<String>,
        agent_kind: String,
        config_data: JsonValue,
    ) -> Self {
        Self {
            id,
            profile_id,
            name,
            agent_kind,
            agent_runtime: AgentRuntime::Native, // Default to Native
            wasm_module_path: None,             // Default to None
            config_data,
            credential_ids: Vec::new(),
            enabled: true,
            dependencies: Vec::new(),
        }
    }

    /// Updates the agent's name
    pub fn set_name(&mut self, name: Option<String>) {
        self.name = name;
    }

    /// Updates the agent's configuration data
    pub fn set_config_data(&mut self, config_data: JsonValue) {
        self.config_data = config_data;
    }

    /// Adds a credential ID to the agent
    pub fn add_credential(&mut self, credential_id: CredentialId) {
        if !self.credential_ids.contains(&credential_id) {
            self.credential_ids.push(credential_id);
        }
    }

    /// Removes a credential ID from the agent
    pub fn remove_credential(&mut self, credential_id: &CredentialId) -> bool {
        if let Some(pos) = self.credential_ids.iter().position(|id| id == credential_id) {
            self.credential_ids.remove(pos);
            true
        } else {
            false
        }
    }

    /// Enables or disables the agent
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Checks if the agent has any credentials configured
    pub fn has_credentials(&self) -> bool {
        !self.credential_ids.is_empty()
    }

    /// Gets the number of configured credentials
    pub fn credential_count(&self) -> usize {
        self.credential_ids.len()
    }

    /// Validates the agent configuration
    pub fn validate(&self) -> Result<(), HelixError> {
        if self.agent_kind.trim().is_empty() {
            return Err(HelixError::ValidationError {
                context: "AgentConfig.agent_kind".to_string(),
                message: "Agent kind cannot be empty".to_string(),
            });
        }

        if self.config_data.is_null() {
            return Err(HelixError::ValidationError {
                context: "AgentConfig.config_data".to_string(),
                message: "Config data cannot be null".to_string(),
            });
        }

        if self.agent_runtime == AgentRuntime::Wasm && self.wasm_module_path.is_none() {
            return Err(HelixError::ValidationError {
                context: "AgentConfig.wasm_module_path".to_string(),
                message: "WASM module path must be provided for WASM agents".to_string(),
            });
        }

        if self.agent_runtime == AgentRuntime::Native && self.wasm_module_path.is_some() {
            return Err(HelixError::ValidationError {
                context: "AgentConfig.wasm_module_path".to_string(),
                message: "WASM module path should not be provided for native agents".to_string(),
            });
        }

        Ok(())
    }

    /// Gets the size of the configuration data in bytes (approximate)
    pub fn config_size_bytes(&self) -> usize {
        self.config_data.to_string().len()
    }
}

/// Core trait representing a unit of behavior in Helix.
/// Agents can be sources, transformers, or actions.
#[async_trait]
pub trait Agent: Send + Sync {
    /// Returns the unique ID of this agent instance.
    fn id(&self) -> AgentId;

    /// Returns the configuration of this agent.
    fn config(&self) -> &AgentConfig;

    /// Performs any necessary setup or initialization for the agent.
    async fn setup(&mut self) -> Result<(), HelixError> {
        // Default implementation does nothing
        Ok(())
    }

    /// Performs any necessary teardown or cleanup for the agent.
    async fn teardown(&mut self) -> Result<(), HelixError> {
        // Default implementation does nothing
        Ok(())
    }
}

/// Context provided to Source agents when they run.
pub struct SourceContext {
    /// The unique ID of the agent instance using this context.
    pub agent_id: AgentId,
    /// The ID of the profile this agent instance belongs to.
    pub profile_id: ProfileId,
    /// Provides access to stored credentials.
    pub credential_provider: Arc<dyn CredentialProvider>,
    /// Provides access to persistent state.
    pub state_store: Arc<dyn StateStore>,
    /// Event transmitter for sending events.
    pub event_tx: mpsc::Sender<Event>,
}

impl SourceContext {
    /// Emits an event from the source agent.
    pub async fn emit(
        &self,
        event_payload: JsonValue,
        event_type_override: Option<String>,
    ) -> Result<(), HelixError> {
        // Use override or generate a default type based on agent ID?
        let event_type =
            event_type_override.unwrap_or_else(|| format!("agent.{}.emit", self.agent_id));
        let event = Event::new(self.agent_id.to_string(), event_type, Some(event_payload)); // Pass profile_id and event_type
                                                                                           // Send the event through the channel
        self.event_tx
            .send(event)
            .await
            .map_err(|e| HelixError::MpscSendError(e.to_string()))?; // Handle potential send error
        Ok(())
    }
}

/// Trait for agents that act as event sources.
#[async_trait]
pub trait SourceAgent: Agent {
    /// Executes the source agent's logic to potentially generate events.
    async fn run(&mut self, ctx: SourceContext) -> Result<(), HelixError>;
}

/// Context provided to Transformer agents.
pub struct TransformerContext {
    /// The unique ID of the agent instance using this context.
    pub agent_id: AgentId,
    /// The ID of the profile this agent instance belongs to.
    pub profile_id: ProfileId,
    /// Provides access to stored credentials.
    pub credential_provider: Arc<dyn CredentialProvider>,
    /// Provides access to persistent state.
    pub state_store: Arc<dyn StateStore>,
}

/// Trait for agents that transform incoming events.
#[async_trait]
pub trait TransformerAgent: Agent {
    /// Processes an incoming event and returns zero or more transformed events.
    async fn transform(
        &mut self,
        ctx: TransformerContext,
        event: Event,
    ) -> Result<Vec<Event>, HelixError>;
}

/// Context provided to Action agents.
pub struct ActionContext {
    /// The unique ID of the agent instance using this context.
    pub agent_id: AgentId,
    /// The ID of the profile this agent instance belongs to.
    pub profile_id: ProfileId,
    /// Provides access to stored credentials.
    pub credential_provider: Arc<dyn CredentialProvider>,
    /// Provides access to persistent state.
    pub state_store: Arc<dyn StateStore>,
}

/// Trait for agents that perform actions based on incoming events.
#[async_trait]
pub trait ActionAgent: Agent {
    /// Executes an action based on an incoming event.
    async fn execute(&mut self, ctx: ActionContext, event: Event) -> Result<(), HelixError>;
}

#[cfg(test)]
mod tests {
    // Add tests for agent structures and traits
    use super::*;
    use crate::event::Event;
    use crate::types::AgentId;
    use uuid::Uuid;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    #[test]
    fn test_agent_config_creation() {
        let id = Uuid::new_v4();
        let profile_id = Uuid::new_v4();
        let name = Some("Test Agent".to_string());
        let agent_kind = "webhook".to_string();
        let config_data = json!({"url": "https://example.com"});

        let config = AgentConfig::new(id, profile_id, name.clone(), agent_kind.clone(), config_data.clone());

        assert_eq!(config.id, id);
        assert_eq!(config.profile_id, profile_id);
        assert_eq!(config.name, name);
        assert_eq!(config.agent_kind, agent_kind);
        assert_eq!(config.config_data, config_data);
        assert!(config.credential_ids.is_empty());
        assert!(config.enabled);
        assert!(config.dependencies.is_empty());
        assert_eq!(config.agent_runtime, AgentRuntime::Native);
        assert_eq!(config.wasm_module_path, None);
    }

    #[test]
    fn test_agent_config_set_name() {
        let mut config = AgentConfig::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            None,
            "test".to_string(),
            json!({}),
        );

        config.set_name(Some("New Name".to_string()));
        assert_eq!(config.name, Some("New Name".to_string()));

        config.set_name(None);
        assert_eq!(config.name, None);
    }

    #[test]
    fn test_agent_config_set_config_data() {
        let mut config = AgentConfig::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            None,
            "test".to_string(),
            json!({}),
        );

        let new_config = json!({"key": "value"});
        config.set_config_data(new_config.clone());
        assert_eq!(config.config_data, new_config);
    }

    #[test]
    fn test_agent_config_credentials() {
        let mut config = AgentConfig::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            None,
            "test".to_string(),
            json!({}),
        );

        assert!(!config.has_credentials());
        assert_eq!(config.credential_count(), 0);

        let cred1 = Uuid::new_v4();
        let cred2 = Uuid::new_v4();

        config.add_credential(cred1);
        assert!(config.has_credentials());
        assert_eq!(config.credential_count(), 1);
        assert!(config.credential_ids.contains(&cred1));

        config.add_credential(cred2);
        assert_eq!(config.credential_count(), 2);

        // Adding same credential should not duplicate
        config.add_credential(cred1);
        assert_eq!(config.credential_count(), 2);

        // Remove credential
        assert!(config.remove_credential(&cred1));
        assert_eq!(config.credential_count(), 1);
        assert!(!config.credential_ids.contains(&cred1));

        // Remove non-existent credential
        assert!(!config.remove_credential(&cred1));
        assert_eq!(config.credential_count(), 1);
    }

    #[test]
    fn test_agent_config_enabled() {
        let mut config = AgentConfig::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            None,
            "test".to_string(),
            json!({}),
        );

        assert!(config.enabled);

        config.set_enabled(false);
        assert!(!config.enabled);

        config.set_enabled(true);
        assert!(config.enabled);
    }

    #[test]
    fn test_agent_config_validation() {
        let mut config = AgentConfig::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            None,
            "test".to_string(),
            json!({"valid": true}),
        );

        assert!(config.validate().is_ok());

        // Test empty agent kind
        config.agent_kind = "".to_string();
        let result = config.validate();
        assert!(result.is_err());
        if let Err(HelixError::ValidationError { context, message }) = result {
            assert_eq!(context, "AgentConfig.agent_kind");
            assert!(message.contains("cannot be empty"));
        }

        // Test whitespace-only agent kind
        config.agent_kind = "   ".to_string();
        let result = config.validate();
        assert!(result.is_err());

        // Test null config data
        config.agent_kind = "test".to_string(); // Reset agent_kind
        config.config_data = json!(null);
        let result = config.validate();
        assert!(result.is_err());
        if let Err(HelixError::ValidationError { context, message }) = result {
            assert_eq!(context, "AgentConfig.config_data");
            assert!(message.contains("cannot be null"));
        }
        config.config_data = json!({}); // Reset config_data

        // Test WASM agent without path
        config.agent_runtime = AgentRuntime::Wasm;
        config.wasm_module_path = None;
        let result = config.validate();
        assert!(result.is_err());
        if let Err(HelixError::ValidationError { context, message }) = result {
            assert_eq!(context, "AgentConfig.wasm_module_path");
            assert!(message.contains("must be provided for WASM agents"));
        }

        // Test Native agent with path
        config.agent_runtime = AgentRuntime::Native;
        config.wasm_module_path = Some("some/path.wasm".to_string());
        let result = config.validate();
        assert!(result.is_err());
        if let Err(HelixError::ValidationError { context, message }) = result {
            assert_eq!(context, "AgentConfig.wasm_module_path");
            assert!(message.contains("should not be provided for native agents"));
        }

        // Test valid WASM agent
        config.agent_runtime = AgentRuntime::Wasm;
        config.wasm_module_path = Some("some/path.wasm".to_string());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_agent_config_size_bytes() {
        let config = AgentConfig::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            None,
            "test".to_string(),
            json!({"key": "value"}),
        );

        let size = config.config_size_bytes();
        assert!(size > 0);
        assert!(size < 100); // Should be small for this simple config
    }

    #[test]
    fn test_agent_config_serialization() {
        let config = AgentConfig::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some("Test Agent".to_string()),
            "webhook".to_string(),
            json!({"url": "https://example.com"}),
        );
        let mut config_wasm = config.clone();
        config_wasm.agent_runtime = AgentRuntime::Wasm;
        config_wasm.wasm_module_path = Some("path/to/mod.wasm".to_string());


        let serialized = serde_json::to_string(&config).expect("Failed to serialize native");
        let deserialized: AgentConfig = serde_json::from_str(&serialized).expect("Failed to deserialize native");

        assert_eq!(config.id, deserialized.id);
        assert_eq!(config.agent_runtime, AgentRuntime::Native); // Default
        assert_eq!(deserialized.agent_runtime, AgentRuntime::Native);
        assert_eq!(deserialized.wasm_module_path, None);

        let serialized_wasm = serde_json::to_string(&config_wasm).expect("Failed to serialize wasm");
        let deserialized_wasm: AgentConfig = serde_json::from_str(&serialized_wasm).expect("Failed to deserialize wasm");

        assert_eq!(config_wasm.id, deserialized_wasm.id);
        assert_eq!(config.profile_id, deserialized.profile_id);
        assert_eq!(config.name, deserialized.name);
        assert_eq!(config.agent_kind, deserialized.agent_kind);
        assert_eq!(config.config_data, deserialized.config_data);
        assert_eq!(config.enabled, deserialized.enabled);
        assert_eq!(config.dependencies, deserialized.dependencies);

        assert_eq!(deserialized_wasm.agent_runtime, AgentRuntime::Wasm);
        assert_eq!(deserialized_wasm.wasm_module_path, Some("path/to/mod.wasm".to_string()));
    }

    #[test]
    fn test_agent_config_equality() {
        let id = Uuid::new_v4();
        let profile_id = Uuid::new_v4();
        let config_data = json!({"test": true});

        let config1 = AgentConfig::new(
            id,
            profile_id,
            Some("Test".to_string()),
            "webhook".to_string(),
            config_data.clone(),
        );

        let config2 = AgentConfig::new(
            id,
            profile_id,
            Some("Test".to_string()),
            "webhook".to_string(),
            config_data.clone(),
        );

        let config2_with_deps = AgentConfig {
            dependencies: vec![Uuid::new_v4()],
            ..config2.clone() // Ensure all other fields are the same
        };

        assert_eq!(config1, config2); // config1 and config2 should be equal
        assert_ne!(config1, config2_with_deps); // config1 and config2_with_deps should not be equal

        let config3 = AgentConfig::new(
            Uuid::new_v4(),
            profile_id,
            Some("Test".to_string()),
            "webhook".to_string(),
            json!({"test": true}),
        );

        assert_ne!(config1, config3);
    }

    #[test]
    fn test_agent_config_debug_format() {
        let config = AgentConfig::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some("Debug Test".to_string()),
            "test_agent".to_string(),
            json!({"debug": true}),
        );

        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("AgentConfig"));
        assert!(debug_str.contains("Debug Test"));
        assert!(debug_str.contains("test_agent"));
    }

    #[test]
    fn test_agent_config_clone() {
        let original = AgentConfig::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some("Clone Test".to_string()),
            "cloneable".to_string(),
            json!({"clone": true}),
        );

        let cloned = original.clone();
        assert_eq!(original, cloned);
        assert_eq!(original.id, cloned.id);
        assert_eq!(original.name, cloned.name);
        assert_eq!(original.dependencies, cloned.dependencies);
        assert_eq!(original.agent_runtime, cloned.agent_runtime);
        assert_eq!(original.wasm_module_path, cloned.wasm_module_path);
    }

    #[tokio::test]
    async fn test_source_context_emit() {
        let (tx, _rx) = mpsc::channel::<Event>(10);
        let agent_id = Uuid::new_v4(); // Use Uuid
        let profile_id = Uuid::new_v4(); // Use Uuid

        // Create mock providers (adjust as necessary if they have state/methods)
        let mock_cred_provider = Arc::new(MockCredentialProvider);
        let mock_state_store = Arc::new(MockStateStore);

        let ctx = SourceContext {
            agent_id,   // Use Uuid variable
            profile_id, // Use Uuid variable
            // These need actual mock implementations conforming to the traits
            credential_provider: mock_cred_provider,
            state_store: mock_state_store,
            event_tx: tx, // Use sender tx (channel.0)
        };

        let payload = json!({ "message": "hello" });
        let result = ctx.emit(payload, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_action_agent_execute() {
        let (tx, _rx) = mpsc::channel(10);
        let agent_id = Uuid::new_v4();
        let profile_id = Uuid::new_v4(); // Keep profile_id for context if needed
        let mut agent = MockActionAgent::new(agent_id, tx);

        let event_payload = json!({ "action": "test" });
        // Create the event with the correct signature
        let event = Event::new(
            agent_id.to_string(), // Use agent_id as source for this test
            "test.trigger".to_string(),
            Some(event_payload),
        );

        let ctx = ActionContext {
            agent_id,
            profile_id,
            // These need actual mock implementations conforming to the traits
            credential_provider: Arc::new(MockCredentialProvider),
            state_store: Arc::new(MockStateStore),
        };

        let result = agent.execute(ctx, event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_source_context_emit_with_override() {
        let (tx, mut rx) = mpsc::channel::<Event>(10);
        let agent_id = Uuid::new_v4();
        let profile_id = Uuid::new_v4();

        let ctx = SourceContext {
            agent_id,
            profile_id,
            credential_provider: Arc::new(MockCredentialProvider),
            state_store: Arc::new(MockStateStore),
            event_tx: tx,
        };

        let payload = json!({ "custom": "data" });
        let custom_type = "custom.event.type".to_string();
        let result = ctx.emit(payload.clone(), Some(custom_type.clone())).await;
        assert!(result.is_ok());

        // Verify the event was sent with custom type
        let event = rx.recv().await.unwrap();
        assert_eq!(event.r#type, custom_type);
        assert_eq!(event.data, Some(payload));
    }

    #[tokio::test]
    async fn test_source_context_emit_default_type() {
        let (tx, mut rx) = mpsc::channel::<Event>(10);
        let agent_id = Uuid::new_v4();
        let profile_id = Uuid::new_v4();

        let ctx = SourceContext {
            agent_id,
            profile_id,
            credential_provider: Arc::new(MockCredentialProvider),
            state_store: Arc::new(MockStateStore),
            event_tx: tx,
        };

        let payload = json!({ "default": "type" });
        let result = ctx.emit(payload, None).await;
        assert!(result.is_ok());

        // Verify the event was sent with default type
        let event = rx.recv().await.unwrap();
        let expected_type = format!("agent.{}.emit", agent_id);
        assert_eq!(event.r#type, expected_type);
    }

    #[tokio::test]
    async fn test_mock_credential_provider() {
        let provider = MockCredentialProvider;
        let result = provider.get_credential("test_id").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[tokio::test]
    async fn test_mock_state_store() {
        let store = MockStateStore;

        // Test get non-existent state
        let result = store.get_state("test_key").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);

        // Test set state
        let result = store.set_state("test_key", b"test_value").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_source_agent() {
        let agent_id = Uuid::new_v4();
        let mut agent = MockSourceAgent::new(agent_id);

        assert_eq!(agent.id(), agent_id);
        assert_eq!(agent.count, 0);

        let (tx, mut rx) = mpsc::channel::<Event>(10);
        let ctx = SourceContext {
            agent_id,
            profile_id: Uuid::new_v4(),
            credential_provider: Arc::new(MockCredentialProvider),
            state_store: Arc::new(MockStateStore),
            event_tx: tx,
        };

        let result = agent.run(ctx).await;
        assert!(result.is_ok());
        assert_eq!(agent.count, 1);

        // Verify event was emitted
        let event = rx.recv().await.unwrap();
        assert_eq!(event.r#type, "source.tick");
        let data = event.data.unwrap();
        assert_eq!(data["count"], 1);
        assert_eq!(data["message"], "Tick");
    }

    #[tokio::test]
    async fn test_mock_action_agent() {
        let agent_id = Uuid::new_v4();
        let (tx, _rx) = mpsc::channel::<Event>(10);
        let mut agent = MockActionAgent::new(agent_id, tx);

        assert_eq!(agent.id(), agent_id);

        let ctx = ActionContext {
            agent_id,
            profile_id: Uuid::new_v4(),
            credential_provider: Arc::new(MockCredentialProvider),
            state_store: Arc::new(MockStateStore),
        };

        let event = Event::new(
            "test_source".to_string(),
            "test.event".to_string(),
            Some(json!({"action": "test"})),
        );

        let result = agent.execute(ctx, event).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_agent_config_unicode_support() {
        let config = AgentConfig::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some("测试代理 🤖".to_string()),
            "unicode_agent".to_string(),
            json!({"message": "你好世界", "emoji": "🚀"}),
        );

        assert_eq!(config.name, Some("测试代理 🤖".to_string()));
        assert_eq!(config.config_data["message"], "你好世界");
        assert_eq!(config.config_data["emoji"], "🚀");
    }

    #[test]
    fn test_agent_config_large_config_data() {
        let large_config = json!({
            "large_array": (0..1000).collect::<Vec<i32>>(),
            "nested": {
                "deep": {
                    "value": "test"
                }
            }
        });

        let config = AgentConfig::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some("Large Config Agent".to_string()),
            "large_config".to_string(),
            large_config.clone(),
        );

        assert_eq!(config.config_data, large_config);
        assert!(config.config_size_bytes() > 1000);
    }

    #[test]
    fn test_agent_config_edge_cases() {
        // Test with empty name
        let config = AgentConfig::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some("".to_string()),
            "empty_name".to_string(),
            json!({}),
        );
        assert_eq!(config.name, Some("".to_string()));

        // Test with complex JSON
        let complex_json = json!({
            "array": [1, "two", {"three": 3}, [4, 5]],
            "boolean": true,
            "null_value": null,
            "number": std::f64::consts::PI,
            "string": "test"
        });

        let config = AgentConfig::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            None,
            "complex".to_string(),
            complex_json.clone(),
        );
        assert_eq!(config.config_data, complex_json);
    }

    #[test]
    fn test_agent_config_boundary_values() {
        // Test with minimum UUID values
        let min_uuid = Uuid::from_bytes([0; 16]);
        let max_uuid = Uuid::from_bytes([255; 16]);

        let config = AgentConfig::new(
            min_uuid,
            max_uuid,
            Some("Boundary Test".to_string()),
            "boundary".to_string(),
            json!({"boundary": "test"}),
        );

        assert_eq!(config.id, min_uuid);
        assert_eq!(config.profile_id, max_uuid);
    }

    #[test]
    fn test_agent_config_json_edge_cases() {
        let config = AgentConfig::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some(r#"Name with "quotes" and \backslashes"#.to_string()),
            "json_test".to_string(),
            json!({"key": "value with \"quotes\"", "path": "C:\\Windows\\System32"}),
        );

        let serialized = serde_json::to_string(&config).expect("Failed to serialize");
        let deserialized: AgentConfig = serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(config, deserialized);
        assert!(config.name.as_ref().unwrap().contains("quotes"));
        assert!(config.config_data["path"].as_str().unwrap().contains("\\"));
    }

    #[tokio::test]
    async fn test_source_context_channel_closed() {
        let (tx, rx) = mpsc::channel::<Event>(1);
        drop(rx); // Close the receiver

        let ctx = SourceContext {
            agent_id: Uuid::new_v4(),
            profile_id: Uuid::new_v4(),
            credential_provider: Arc::new(MockCredentialProvider),
            state_store: Arc::new(MockStateStore),
            event_tx: tx,
        };

        let payload = json!({ "test": "data" });
        let result = ctx.emit(payload, None).await;

        // Should return an error when channel is closed
        assert!(result.is_err());
        if let Err(HelixError::MpscSendError(_)) = result {
            // Expected error type
        } else {
            panic!("Expected MpscSendError");
        }
    }

    // --- Mock Implementations for Testing ---
    #[derive(Debug)]
    #[allow(dead_code)] // Allowed for test mock
    struct MockCredentialProvider;

    #[async_trait]
    impl CredentialProvider for MockCredentialProvider {
        async fn get_credential(&self, _id: &str) -> Result<Option<String>, HelixError> {
            Ok(None)
        }
    }

    #[derive(Debug)]
    #[allow(dead_code)] // Allowed for test mock
    struct MockStateStore;

    #[async_trait]
    impl StateStore for MockStateStore {
        async fn get_state(&self, _key: &str) -> Result<Option<Vec<u8>>, HelixError> {
            Ok(None)
        }

        async fn set_state(&self, _key: &str, _value: &[u8]) -> Result<(), HelixError> {
            Ok(())
        }
    }

    #[derive(Debug)]
    #[allow(dead_code)] // Allowed for test mock
    struct MockSourceAgent {
        agent_id: AgentId,
        count: u32,
    }

    #[allow(dead_code)] // Allowed for test mock helper
    impl MockSourceAgent {
        #[allow(dead_code)] // Allowed for test mock helper
        fn new(agent_id: AgentId) -> Self {
            Self { agent_id, count: 0 }
        }
    }

    #[async_trait]
    impl Agent for MockSourceAgent {
        fn id(&self) -> AgentId {
            self.agent_id
        }

        fn config(&self) -> &AgentConfig {
            unimplemented!()
        }
    }

    #[async_trait]
    impl SourceAgent for MockSourceAgent {
        async fn run(&mut self, ctx: SourceContext) -> Result<(), HelixError> {
            self.count += 1;
            let event_payload = json!({ "count": self.count, "message": "Tick" });
            let event = Event::new(self.agent_id.to_string(), "source.tick".to_string(), Some(event_payload)); // Pass profile_id and event_type
            ctx.event_tx.send(event).await?;
            Ok(())
        }
    }

    #[derive(Debug)]
    #[allow(dead_code)] // Allowed for test mock
    struct MockActionAgent {
        agent_id: AgentId,
        event_tx: mpsc::Sender<Event>,
    }

    #[allow(dead_code)] // Allowed for test mock helper
    impl MockActionAgent {
        #[allow(dead_code)] // Allowed for test mock helper
        fn new(agent_id: AgentId, event_tx: mpsc::Sender<Event>) -> Self {
            Self { agent_id, event_tx }
        }
    }

    #[async_trait]
    impl Agent for MockActionAgent {
        fn id(&self) -> AgentId {
            self.agent_id
        }

        fn config(&self) -> &AgentConfig {
            unimplemented!()
        }
    }

    #[async_trait]
    impl ActionAgent for MockActionAgent {
        async fn execute(&mut self, _ctx: ActionContext, _event: Event) -> Result<(), HelixError> {
            Ok(())
        }
    }
}
