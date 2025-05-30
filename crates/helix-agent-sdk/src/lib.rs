//! # Helix Agent SDK
//!
//! This crate provides the necessary tools, traits, and structures for developing
//! Helix agents in Rust. Agents are the core components of the Helix platform,
//! responsible for performing tasks such as sourcing data, transforming events,
//! or executing actions based on system events.
//!
//! ## Key Components
//!
//! - **Agent Traits**: [`SdkAgent`], [`SourceSdkAgent`], [`TransformSdkAgent`], [`ActionSdkAgent`]
//!   define the core behaviors and lifecycle methods for different types of agents.
//! - **`AgentContext`**: Provides agents with access to their configuration, an
//!   [`EventPublisher`] for emitting new events, a [`CredentialProvider`] for
//!   securely accessing credentials, and a [`StateStore`] for persisting and
//!   retrieving agent state.
//! - **`SdkError`**: The primary error type used throughout the SDK.
//! - **Procedural Macros**: (Defined in `helix-agent-sdk-macros`) Attributes like
//!   `#[source_agent]`, `#[action_agent]`, and `#[transform_agent]` simplify
//!   the boilerplate involved in creating agent structs.
//!
//! ## Agent Lifecycle
//!
//! SDK agents implement the [`SdkAgent`] trait, which includes the following
//! lifecycle methods managed by the Helix runtime:
//! 1. `init()`: Called once for one-time setup and resource acquisition.
//! 2. `start()`: Called after `init()` to begin the agent's main operations (e.g., polling, listening).
//! 3. `stop()`: Called to request a graceful shutdown, allowing the agent to release resources.
//!
//! Specific agent types (Source, Transform, Action) have additional methods like `run()`,
//! `transform()`, or `execute()` that define their primary operational logic.

use async_trait::async_trait;
use helix_core::{
    agent::{Agent, AgentConfig, CredentialProvider, StateStore}, // AgentConfig is Arc'd in AgentContext
    event::Event as HelixEvent,
    types::AgentId, // Explicitly used by EventPublisher
    HelixError as CoreError,
};
use serde_json::Value as JsonValue;
use std::{fmt, sync::Arc};

// --- Error Type ---

/// Errors that can occur within the Helix Agent SDK.
#[derive(Debug)]
pub enum SdkError {
    /// Error during agent initialization.
    InitializationFailed(String),
    /// Error when attempting to start an agent's operations.
    StartFailed(String),
    /// Error when attempting to stop an agent's operations.
    StopFailed(String),
    /// Error during the primary execution logic of an agent (e.g., in `run`, `transform`, `execute`).
    ExecutionFailed(String),
    /// Error related to agent configuration.
    ConfigurationError(String),
    /// Error occurred while trying to publish an event.
    EventPublishError(String),
    /// An error originating from the `helix-core` crate.
    CoreError(CoreError),
    /// A general or custom error within the SDK.
    Custom(String),
}

impl fmt::Display for SdkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SdkError::InitializationFailed(s) => write!(f, "Agent initialization failed: {}", s),
            SdkError::StartFailed(s) => write!(f, "Agent start failed: {}", s),
            SdkError::StopFailed(s) => write!(f, "Agent stop failed: {}", s),
            SdkError::ExecutionFailed(s) => write!(f, "Agent execution failed: {}", s),
            SdkError::ConfigurationError(s) => write!(f, "Agent configuration error: {}", s),
            SdkError::EventPublishError(s) => write!(f, "Failed to publish event: {}", s),
            SdkError::CoreError(e) => write!(f, "Core error: {}", e),
            SdkError::Custom(s) => write!(f, "SDK error: {}", s),
        }
    }
}

impl std::error::Error for SdkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SdkError::CoreError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<CoreError> for SdkError {
    fn from(err: CoreError) -> Self {
        SdkError::CoreError(err)
    }
}

// --- Event Publishing Abstraction ---

/// Trait for publishing events from an agent.
/// This will be implemented by the runtime and provided via `AgentContext`.
#[async_trait]
pub trait EventPublisher: Send + Sync {
    /// Publishes an event.
    ///
    /// # Arguments
    /// * `agent_id` - The ID of the agent publishing the event.
    /// * `event_payload` - The JSON payload of the event.
    /// * `event_type_override` - Optional string to override the default event type.
    async fn publish_event(
        &self,
        agent_id: &AgentId,
        event_payload: JsonValue,
        event_type_override: Option<String>,
    ) -> Result<(), SdkError>;
}

// --- Agent Context ---

/// Provides SDK agents with access to their configuration, event publishing capabilities,
/// credential management, and state persistence.
///
/// An instance of `AgentContext` is passed to various lifecycle and execution methods
/// of an SDK agent, allowing it to interact with the broader Helix system.
pub struct AgentContext {
    /// The agent's specific configuration, derived from the recipe and system settings.
    pub agent_config: Arc<AgentConfig>,
    /// A mechanism for the agent to publish new events into the Helix event mesh.
    pub event_publisher: Arc<dyn EventPublisher>,
    /// A mechanism for the agent to securely access predefined credentials.
    pub credential_provider: Arc<dyn CredentialProvider>,
    /// A mechanism for the agent to persist and retrieve its operational state.
    pub state_store: Arc<dyn StateStore>,
}

impl AgentContext {
    /// Creates a new `AgentContext`.
    ///
    /// This constructor is typically called by the Helix runtime when an agent
    /// is being initialized.
    ///
    /// # Arguments
    /// * `agent_config` - The configuration specific to this agent instance.
    /// * `event_publisher` - An implementation of [`EventPublisher`] for event emission.
    /// * `credential_provider` - An implementation of [`CredentialProvider`] for credential access.
    /// * `state_store` - An implementation of [`StateStore`] for state management.
    pub fn new(
        agent_config: Arc<AgentConfig>,
        event_publisher: Arc<dyn EventPublisher>,
        credential_provider: Arc<dyn CredentialProvider>,
        state_store: Arc<dyn StateStore>,
    ) -> Self {
        Self {
            agent_config,
            event_publisher,
            credential_provider,
            state_store,
        }
    }

    /// Accesses the agent's configuration.
    ///
    /// The returned [`AgentConfig`] provides details such as the agent's ID,
    /// custom configuration parameters, and associated recipe ID.
    pub fn config(&self) -> &AgentConfig {
        &self.agent_config
    }

    /// Emits an event from the agent into the Helix event mesh.
    ///
    /// # Arguments
    /// * `event_payload` - The JSON payload of the event to be published.
    /// * `event_type_override` - An optional string to specify the event type.
    ///   If `None`, a default event type may be generated by the runtime or publisher
    ///   (e.g., based on the agent's ID).
    ///
    /// # Returns
    /// A `Result` indicating success or an [`SdkError::EventPublishError`] on failure.
    pub async fn emit_event(
        &self,
        event_payload: JsonValue,
        event_type_override: Option<String>,
    ) -> Result<(), SdkError> {
        self.event_publisher
            .publish_event(&self.agent_config.id, event_payload, event_type_override)
            .await
    }

    /// Accesses the [`CredentialProvider`] for retrieving credentials.
    ///
    /// Agents can use this provider to fetch credentials by their ID, as defined
    /// in the agent's configuration or system-wide settings.
    pub fn credential_provider(&self) -> &Arc<dyn CredentialProvider> {
        &self.credential_provider
    }

    /// Accesses the [`StateStore`] for persisting and retrieving agent-specific state.
    ///
    /// Agents can use this store to save and load data across invocations or
    /// different lifecycle stages.
    pub fn state_store(&self) -> &Arc<dyn StateStore> {
        &self.state_store
    }
}

// --- Core SDK Agent Traits ---

/// Base trait defining the lifecycle and core functionality for all Helix SDK agents.
///
/// This trait extends [`helix_core::agent::Agent`] and adds SDK-specific lifecycle
/// methods (`init`, `start`, `stop`). Agents developed with the SDK should implement
/// this trait, often facilitated by procedural macros like `#[source_agent]`.
///
/// Implementers are expected to manage their own internal state and perform their
/// primary logic within the methods defined by more specific traits like
/// [`SourceSdkAgent`], [`TransformSdkAgent`], or [`ActionSdkAgent`].
#[async_trait]
pub trait SdkAgent: Agent + Send + Sync {
    /// Initializes the agent with its context.
    ///
    /// This method is called once by the Helix runtime before the agent's `start`
    /// method. It's intended for one-time setup, resource acquisition (e.g.,
    /// database connections, client initializations), or validation that needs
    /// to occur before the agent begins its primary operations.
    ///
    /// # Arguments
    /// * `context` - The [`AgentContext`] providing access to configuration and system services.
    ///
    /// # Returns
    /// A `Result` indicating success or an [`SdkError::InitializationFailed`] on failure.
    async fn init(&mut self, context: &AgentContext) -> Result<(), SdkError>;

    /// Starts the agent's primary operations.
    ///
    /// Called by the runtime after `init` has successfully completed. For agents
    /// that involve ongoing processes (e.g., polling an external API, listening on a
    /// message queue, managing a background task), this method should initiate those
    /// processes. For agents that are purely reactive or passive, this method might
    /// be a no-op.
    ///
    /// # Arguments
    /// * `context` - The [`AgentContext`].
    ///
    /// # Returns
    /// A `Result` indicating success or an [`SdkError::StartFailed`] on failure.
    async fn start(&mut self, context: &AgentContext) -> Result<(), SdkError>;

    /// Stops the agent's operations and performs cleanup.
    ///
    /// Called by the runtime to request a graceful shutdown of the agent.
    /// Implementations should ensure that any ongoing processes are terminated,
    /// resources are released (e.g., closing connections, flushing buffers),
    /// and the agent is left in a clean state.
    ///
    /// # Arguments
    /// * `context` - The [`AgentContext`].
    ///
    /// # Returns
    /// A `Result` indicating success or an [`SdkError::StopFailed`] on failure.
    async fn stop(&mut self, context: &AgentContext) -> Result<(), SdkError>;

    // Note: `id()`, `config()` are inherited from `helix_core::agent::Agent`.
    // The `setup()` and `teardown()` methods from `helix_core::agent::Agent` are also
    // available but are typically managed by the runtime or SDK macros. `init` and `stop`
    // are the primary lifecycle methods for SDK agent developers to implement.
}

/// Trait for SDK agents that act as event sources.
///
/// Source agents are responsible for generating initial events within the Helix system.
/// This could involve polling external systems, listening to message queues, or
/// reacting to scheduled triggers.
#[async_trait]
pub trait SourceSdkAgent: SdkAgent {
    /// Executes the source agent's primary logic to generate events.
    ///
    /// The behavior of this method depends on the nature of the source agent:
    /// - For polling-based sources, this method might be called periodically by the runtime
    ///   or an internal scheduler set up during `start()`.
    /// - For event-driven sources (e.g., listening to an external webhook via a task
    ///   spawned in `start()`), this method might not be directly called by the runtime
    ///   after `start()`, as event generation would be handled by the spawned task.
    ///
    /// Regardless of the invocation pattern, the agent should use `context.emit_event()`
    /// to publish any new events it generates.
    ///
    /// # Arguments
    /// * `context` - The [`AgentContext`].
    ///
    /// # Returns
    /// A `Result` indicating success or an [`SdkError::ExecutionFailed`] on failure.
    async fn run(&mut self, context: &AgentContext) -> Result<(), SdkError>;
}

/// Trait for SDK agents that transform incoming events into one or more outgoing events.
///
/// Transform agents act as intermediaries in an event processing pipeline, modifying,
/// filtering, enriching, or fanning out events.
#[async_trait]
pub trait TransformSdkAgent: SdkAgent {
    /// Processes an incoming event and returns a vector of zero or more transformed events.
    ///
    /// This method is called by the runtime when an event is routed to this transform agent.
    /// The implementation can perform various operations like:
    /// - Modifying the event's payload or metadata.
    /// - Filtering the event (by returning an empty vector).
    /// - Enriching the event with additional data.
    /// - Splitting one event into multiple new events.
    /// - Creating entirely new events based on the incoming one.
    ///
    /// The returned events will be published back into the Helix event mesh by the runtime.
    ///
    /// # Arguments
    /// * `context` - The [`AgentContext`].
    /// * `event` - The incoming [`HelixEvent`] to be transformed.
    ///
    /// # Returns
    /// A `Result` containing a vector of transformed [`HelixEvent`]s, or an
    /// [`SdkError::ExecutionFailed`] on failure.
    async fn transform(
        &mut self,
        context: &AgentContext,
        event: HelixEvent,
    ) -> Result<Vec<HelixEvent>, SdkError>;
}

/// Trait for SDK agents that perform actions based on incoming events.
///
/// Action agents are typically the terminators of an event processing pipeline. They
/// interact with external systems, trigger side effects, or perform tasks based on
/// the content of the events they receive.
#[async_trait]
pub trait ActionSdkAgent: SdkAgent {
    /// Executes an action based on an incoming event.
    ///
    /// This method is called by the runtime when an event is routed to this action agent.
    /// The implementation should perform its designated task, which might involve
    /// interacting with external APIs, databases, or other services.
    ///
    /// Action agents typically do not publish new events, but they can if necessary
    /// using `context.emit_event()`.
    ///
    /// # Arguments
    /// * `context` - The [`AgentContext`].
    /// * `event` - The incoming [`HelixEvent`] that triggers the action.
    ///
    /// # Returns
    /// A `Result` indicating success or an [`SdkError::ExecutionFailed`] on failure.
    async fn execute(&mut self, context: &AgentContext, event: HelixEvent) -> Result<(), SdkError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use helix_core::agent::{AgentConfig, Credential, CredentialProvider, StateStore};
    use helix_core::event::Event as HelixEvent;
    use helix_core::types::{AgentId, CredentialData, CredentialId, EventId, ProfileId, RecipeId, StateData};
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use tokio::runtime::Runtime; // For running async tests

    // --- Mock EventPublisher ---
    #[derive(Clone)]
    struct MockEventPublisher {
        published_events: Arc<Mutex<Vec<(AgentId, JsonValue, Option<String>)>>>,
    }

    impl MockEventPublisher {
        fn new() -> Self {
            Self {
                published_events: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_published_events(&self) -> Vec<(AgentId, JsonValue, Option<String>)> {
            self.published_events.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl EventPublisher for MockEventPublisher {
        async fn publish_event(
            &self,
            agent_id: &AgentId,
            event_payload: JsonValue,
            event_type_override: Option<String>,
        ) -> Result<(), SdkError> {
            self.published_events
                .lock()
                .unwrap()
                .push((agent_id.clone(), event_payload, event_type_override));
            Ok(())
        }
    }

    // --- Mock CredentialProvider ---
    #[derive(Clone)]
    struct MockCredentialProvider {
        credentials: Arc<Mutex<HashMap<CredentialId, Credential>>>,
    }

    impl MockCredentialProvider {
        fn new() -> Self {
            Self {
                credentials: Arc::new(Mutex::new(HashMap::new())),
            }
        }
        #[allow(dead_code)] // May be used in future tests
        fn add_credential(&self, cred: Credential) {
            self.credentials.lock().unwrap().insert(cred.id.clone(), cred);
        }
    }

    #[async_trait]
    impl CredentialProvider for MockCredentialProvider {
        async fn get_credential(&self, credential_id: &CredentialId) -> Result<Option<Credential>, CoreError> {
            Ok(self.credentials.lock().unwrap().get(credential_id).cloned())
        }
    }

    // --- Mock StateStore ---
    #[derive(Clone)]
    struct MockStateStore {
        states: Arc<Mutex<HashMap<String, StateData>>>,
    }

    impl MockStateStore {
        fn new() -> Self {
            Self {
                states: Arc::new(Mutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl StateStore for MockStateStore {
        async fn get_state(&self, profile_id: &ProfileId, agent_id: &AgentId) -> Result<Option<StateData>, CoreError> {
            let key = format!("{}:{}", profile_id, agent_id);
            Ok(self.states.lock().unwrap().get(&key).cloned())
        }

        async fn set_state(&self, profile_id: &ProfileId, agent_id: &AgentId, value: StateData) -> Result<(), CoreError> {
            let key = format!("{}:{}", profile_id, agent_id);
            self.states.lock().unwrap().insert(key, value);
            Ok(())
        }

        async fn delete_state(&self, profile_id: &ProfileId, agent_id: &AgentId) -> Result<(), CoreError> {
            let key = format!("{}:{}", profile_id, agent_id);
            self.states.lock().unwrap().remove(&key);
            Ok(())
        }
    }


    // --- Helper to create AgentConfig ---
    fn create_test_agent_config(agent_id_str: &str) -> Arc<AgentConfig> {
        Arc::new(AgentConfig {
            id: AgentId::new(agent_id_str),
            name: format!("Test Agent {}", agent_id_str),
            class_name: "TestAgentClass".to_string(),
            config: json!({ "key": "value" }),
            recipe_id: RecipeId::new("test-recipe"),
            credentials: None, // Assuming no credentials needed for these tests
        })
    }

    // --- AgentContext Tests ---
    #[test]
    fn agent_context_creation_and_config_access() {
        let agent_config = create_test_agent_config("agent_context_tester");
        let mock_publisher = Arc::new(MockEventPublisher::new()) as Arc<dyn EventPublisher>;
        let mock_cred_provider = Arc::new(MockCredentialProvider::new()) as Arc<dyn CredentialProvider>;
        let mock_state_store = Arc::new(MockStateStore::new()) as Arc<dyn StateStore>;

        let context = AgentContext::new(
            Arc::clone(&agent_config),
            mock_publisher,
            mock_cred_provider,
            mock_state_store,
        );

        assert_eq!(context.config().id, agent_config.id);
        assert_eq!(context.config().name, agent_config.name);
        assert_eq!(context.config().config, agent_config.config);
        assert_eq!(context.event_publisher.type_id(), mock_publisher.type_id()); // Check by type_id as direct comparison of Arc<dyn Trait> is tricky
        assert_eq!(context.credential_provider.type_id(), mock_cred_provider.type_id());
        assert_eq!(context.state_store.type_id(), mock_state_store.type_id());
    }

    #[tokio::test]
    async fn agent_context_emit_event() {
        let agent_id_str = "event_emitter_agent";
        let agent_config = create_test_agent_config(agent_id_str);
        let mock_publisher = Arc::new(MockEventPublisher::new());
        let mock_cred_provider = Arc::new(MockCredentialProvider::new());
        let mock_state_store = Arc::new(MockStateStore::new());

        let context = AgentContext::new(
            Arc::clone(&agent_config),
            Arc::clone(&mock_publisher) as Arc<dyn EventPublisher>,
            mock_cred_provider as Arc<dyn CredentialProvider>,
            mock_state_store as Arc<dyn StateStore>,
        );

        let payload = json!({ "data": "test_event" });
        let event_type = Some("custom.test.event".to_string());

        let result = context.emit_event(payload.clone(), event_type.clone()).await;
        assert!(result.is_ok());

        let published = mock_publisher.get_published_events();
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].0, agent_config.id);
        assert_eq!(published[0].1, payload);
        assert_eq!(published[0].2, event_type);
    }

    // --- Mock Agent Implementations for Trait Testing ---

    struct MockSdkAgentImpl {
        config: Arc<AgentConfig>,
        init_called: bool,
        start_called: bool,
        stop_called: bool,
    }

    impl MockSdkAgentImpl {
        fn new(config: Arc<AgentConfig>) -> Self {
            Self {
                config,
                init_called: false,
                start_called: false,
                stop_called: false,
            }
        }
    }

    #[async_trait]
    impl helix_core::agent::Agent for MockSdkAgentImpl {
        fn id(&self) -> AgentId { self.config.id.clone() }
        fn config(&self) -> &AgentConfig { &self.config }
        async fn setup(&mut self) -> Result<(), CoreError> { Ok(()) }
        async fn teardown(&mut self) -> Result<(), CoreError> { Ok(()) }
    }

    #[async_trait]
    impl SdkAgent for MockSdkAgentImpl {
        async fn init(&mut self, _context: &AgentContext) -> Result<(), SdkError> {
            self.init_called = true;
            Ok(())
        }
        async fn start(&mut self, _context: &AgentContext) -> Result<(), SdkError> {
            self.start_called = true;
            Ok(())
        }
        async fn stop(&mut self, _context: &AgentContext) -> Result<(), SdkError> {
            self.stop_called = true;
            Ok(())
        }
    }

    struct MockSourceAgentImpl {
        sdk_agent: MockSdkAgentImpl, // Composition for SdkAgent behavior
        run_called: bool,
    }

    impl MockSourceAgentImpl {
        fn new(config: Arc<AgentConfig>) -> Self {
            Self {
                sdk_agent: MockSdkAgentImpl::new(config),
                run_called: false,
            }
        }
    }
    
    // Forward Agent and SdkAgent to the inner MockSdkAgentImpl
    #[async_trait]
    impl helix_core::agent::Agent for MockSourceAgentImpl {
        fn id(&self) -> AgentId { self.sdk_agent.id() }
        fn config(&self) -> &AgentConfig { self.sdk_agent.config() }
        async fn setup(&mut self) -> Result<(), CoreError> { self.sdk_agent.setup().await }
        async fn teardown(&mut self) -> Result<(), CoreError> { self.sdk_agent.teardown().await }
    }

    #[async_trait]
    impl SdkAgent for MockSourceAgentImpl {
        async fn init(&mut self, context: &AgentContext) -> Result<(), SdkError> {
            self.sdk_agent.init(context).await
        }
        async fn start(&mut self, context: &AgentContext) -> Result<(), SdkError> {
            self.sdk_agent.start(context).await
        }
        async fn stop(&mut self, context: &AgentContext) -> Result<(), SdkError> {
            self.sdk_agent.stop(context).await
        }
    }

    #[async_trait]
    impl SourceSdkAgent for MockSourceAgentImpl {
        async fn run(&mut self, _context: &AgentContext) -> Result<(), SdkError> {
            self.run_called = true;
            Ok(())
        }
    }

    struct MockActionAgentImpl {
        sdk_agent: MockSdkAgentImpl,
        execute_called_with: Option<HelixEvent>,
    }

    impl MockActionAgentImpl {
        fn new(config: Arc<AgentConfig>) -> Self {
            Self {
                sdk_agent: MockSdkAgentImpl::new(config),
                execute_called_with: None,
            }
        }
    }

    #[async_trait]
    impl helix_core::agent::Agent for MockActionAgentImpl {
        fn id(&self) -> AgentId { self.sdk_agent.id() }
        fn config(&self) -> &AgentConfig { self.sdk_agent.config() }
        async fn setup(&mut self) -> Result<(), CoreError> { self.sdk_agent.setup().await }
        async fn teardown(&mut self) -> Result<(), CoreError> { self.sdk_agent.teardown().await }
    }

    #[async_trait]
    impl SdkAgent for MockActionAgentImpl {
        async fn init(&mut self, context: &AgentContext) -> Result<(), SdkError> {
            self.sdk_agent.init(context).await
        }
        async fn start(&mut self, context: &AgentContext) -> Result<(), SdkError> {
            self.sdk_agent.start(context).await
        }
        async fn stop(&mut self, context: &AgentContext) -> Result<(), SdkError> {
            self.sdk_agent.stop(context).await
        }
    }
    
    #[async_trait]
    impl ActionSdkAgent for MockActionAgentImpl {
        async fn execute(&mut self, _context: &AgentContext, event: HelixEvent) -> Result<(), SdkError> {
            self.execute_called_with = Some(event);
            Ok(())
        }
    }

    // --- Trait Tests ---
    #[tokio::test]
    async fn sdk_agent_trait_methods() {
        let agent_config = create_test_agent_config("sdk_agent_tester");
        let mock_publisher = Arc::new(MockEventPublisher::new());
        let context = AgentContext::new(Arc::clone(&agent_config), mock_publisher);
        let mut agent = MockSdkAgentImpl::new(agent_config);

        assert!(!agent.init_called);
        agent.init(&context).await.unwrap();
        assert!(agent.init_called);

        assert!(!agent.start_called);
        agent.start(&context).await.unwrap();
        assert!(agent.start_called);

        assert!(!agent.stop_called);
        agent.stop(&context).await.unwrap();
        assert!(agent.stop_called);
    }

    #[tokio::test]
    async fn source_sdk_agent_trait_methods() {
        let agent_config = create_test_agent_config("source_agent_tester");
        let mock_publisher = Arc::new(MockEventPublisher::new());
        let context = AgentContext::new(Arc::clone(&agent_config), mock_publisher);
        let mut agent = MockSourceAgentImpl::new(agent_config);

        // Test SdkAgent methods inherited through composition
        agent.init(&context).await.unwrap();
        assert!(agent.sdk_agent.init_called);
        agent.start(&context).await.unwrap();
        assert!(agent.sdk_agent.start_called);

        // Test SourceSdkAgent specific method
        assert!(!agent.run_called);
        agent.run(&context).await.unwrap();
        assert!(agent.run_called);
        
        agent.stop(&context).await.unwrap();
        assert!(agent.sdk_agent.stop_called);
    }

    #[tokio::test]
    async fn action_sdk_agent_trait_methods() {
        let agent_config = create_test_agent_config("action_agent_tester");
        let mock_publisher = Arc::new(MockEventPublisher::new());
        let context = AgentContext::new(Arc::clone(&agent_config), mock_publisher);
        let mut agent = MockActionAgentImpl::new(Arc::clone(&agent_config));

        agent.init(&context).await.unwrap();
        assert!(agent.sdk_agent.init_called);

        let test_event = HelixEvent {
            id: EventId::new_v4(),
            source_agent_id: agent_config.id.clone(),
            recipe_id: agent_config.recipe_id.clone(),
            event_type: "test.event".to_string(),
            data: json!({"key": "value"}),
            metadata: None,
            timestamp: chrono::Utc::now(),
        };

        assert!(agent.execute_called_with.is_none());
        agent.execute(&context, test_event.clone()).await.unwrap();
        assert!(agent.execute_called_with.is_some());
        assert_eq!(agent.execute_called_with.unwrap().id, test_event.id);

        agent.stop(&context).await.unwrap();
        assert!(agent.sdk_agent.stop_called);
    }
}
