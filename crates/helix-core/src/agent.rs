//! Defines the core Agent concept and related traits.

use crate::event::Event;
use crate::types::{AgentId, CredentialId, ProfileId};
use crate::HelixError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
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

fn default_true() -> bool { true }

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
    pub async fn emit(&self, event_payload: JsonValue, kind_suffix: Option<&str>) -> Result<(), HelixError> {
        // TODO: Implement actual event emission (e.g., send to NATS)
        let kind_base = "source"; // Assuming a base kind for sources
        let _kind = match kind_suffix {
            Some(suffix) => format!("{}.{}", kind_base, suffix),
            None => kind_base.to_string(),
        };
        let event = Event::new(self.agent_id, event_payload);
       
        tracing::info!(event_id = %event.id, agent_id = %self.agent_id, "Emitting event");
        // In real implementation: self.nats_client.publish(...).await?;
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
    async fn transform(&mut self, ctx: TransformerContext, event: Event) -> Result<Vec<Event>, HelixError>;
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
    use serde_json::json;

    #[tokio::test]
    async fn test_source_context_emit() {
        let ctx = SourceContext {
            agent_id: "test-agent".to_string(),
            profile_id: "test-profile".to_string(),
            credential_provider: Arc::new(MockCredentialProvider),
            state_store: Arc::new(MockStateStore),
            event_tx: mpsc::channel(100).1,
        };
        let result = ctx.emit(json!({ "data": "test" }), None).await;
        assert!(result.is_ok());
    }
}

struct MockCredentialProvider;

#[async_trait]
impl CredentialProvider for MockCredentialProvider {
    async fn get_credential(&self, _id: &str) -> Result<Option<String>, HelixError> {
        Ok(None)
    }
}

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

struct MockSourceAgent {
    agent_id: AgentId,
    count: u32,
}

impl MockSourceAgent {
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
        let event = Event::new(self.agent_id, event_payload); 
        ctx.event_tx.send(event).await?;
        Ok(())
    }
}
