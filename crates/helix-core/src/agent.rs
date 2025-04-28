//! Defines the core Agent concept and related traits.

use crate::event::Event;
use crate::types::{AgentId, CredentialId, ProfileId};
use crate::HelixError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tokio::sync::mpsc;

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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentConfig {
    /// Unique ID of this agent instance.
    pub id: AgentId,
    /// ID of the profile (tenant) this agent belongs to.
    pub profile_id: ProfileId,
    /// Optional friendly name for the agent.
    pub name: Option<String>,
    /// Type or kind identifier for this agent (e.g., "webhook", "rss_poller").
    pub agent_kind: String,
    /// JSON blob containing agent-specific configuration.
    pub config_data: JsonValue,
    /// IDs of credentials required by this agent.
    #[serde(default)]
    pub credential_ids: Vec<CredentialId>,
    /// Indicates if the agent is currently enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
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
        let event = Event::new(self.agent_id, self.profile_id, event_type, event_payload); // Pass profile_id and event_type
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
    use crate::types::{AgentId, ProfileId};
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::mpsc;

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
        let agent_id = Uuid::new_v4();
        let profile_id = Uuid::new_v4();
        let mut agent = MockActionAgent;
        let event_payload = json!({ "data": "process me" });
        // Create the event with the new required fields
        let event = Event::new(
            Uuid::new_v4(),
            profile_id,
            "test.trigger".to_string(),
            event_payload,
        ); // Added profile_id and event_type

        let ctx = ActionContext {
            agent_id,   // Use Uuid variable
            profile_id, // Use Uuid variable
            // These need actual mock implementations conforming to the traits
            credential_provider: Arc::new(MockCredentialProvider),
            state_store: Arc::new(MockStateStore),
        };

        let result = agent.execute(ctx, event).await;
        assert!(result.is_ok());
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
            let event = Event::new(
                self.agent_id,
                ctx.profile_id,
                "source.tick".to_string(),
                event_payload,
            ); // Pass profile_id and event_type
            ctx.event_tx.send(event).await?;
            Ok(())
        }
    }

    #[derive(Debug)]
    #[allow(dead_code)] // Allowed for test mock
    struct MockActionAgent;

    #[async_trait]
    impl Agent for MockActionAgent {
        fn id(&self) -> AgentId {
            Uuid::new_v4()
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
