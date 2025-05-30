use helix_agent_sdk::{AgentContext, SdkAgent, SdkError, EventPublisher};
use helix_agent_sdk_macros::AGENT_FACTORIES; // Import the distributed slice
use helix_core::agent::{AgentConfig, AgentStatus, AgentKind};
use helix_core::event::Event as HelixEvent;
use helix_core::types::AgentId;
use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use serde_json::Value as JsonValue; // For AgentConfig parameters

/// Type alias for an agent factory function.
///
/// The factory takes the agent's configuration (as AgentConfig, not Arc<AgentConfig> initially for simplicity in test)
/// and returns a boxed `SdkAgent` instance. The EventPublisher is handled by AgentContext.
pub type AgentFactory = Box<
   dyn Fn(AgentConfig) -> Result<Box<dyn SdkAgent>, SdkError>
       + Send
       + Sync,
>;

/// Manages the registration and creation of agent instances.
///
/// The `AgentRegistry` holds a map of agent kinds to factory functions
/// that can produce instances of those agents.
pub struct AgentRegistry {
    factories: HashMap<AgentKind, AgentFactory>, // Changed key from String to AgentKind
}

impl AgentRegistry {
    /// Creates a new `AgentRegistry` and populates it with statically registered agents.
    pub fn new() -> Self {
        let mut registry = Self {
            factories: HashMap::new(),
        };
        registry.populate_from_inventory();
        registry
    }

    /// Populates the registry from the `AGENT_FACTORIES` distributed slice.
    fn populate_from_inventory(&mut self) {
        for factory_fn in AGENT_FACTORIES {
            let (kind, factory_box) = factory_fn();
            // The factory_box is already Box<dyn Fn(...)>, which matches AgentFactory type.
            if let Err(e) = self.register_agent_factory(kind.clone(), factory_box) {
                // Consider how to handle registration errors, e.g., logging.
                // For now, we can use tracing if available, or just print.
                // tracing::error!("Failed to register agent kind '{}': {}", kind.value(), e);
                eprintln!("Failed to register agent kind '{}': {}", kind.value(), e);
            }
        }
    }

    /// Registers a new agent factory for a given agent kind.
   ///
   /// # Arguments
   /// * `kind` - The `AgentKind` for the agent type.
   /// * `factory` - A closure or function that can create an instance of the agent.
   ///
   /// # Errors
   /// Returns an error string if an agent kind is already registered.
   pub fn register_agent_factory( // Renamed from register
       &mut self,
       kind: AgentKind, // Changed from String to AgentKind
       factory: AgentFactory,
   ) -> Result<(), String> {
       if self.factories.contains_key(&kind) {
           Err(format!("Agent kind '{}' already registered.", kind.value()))
       } else {
           self.factories.insert(kind, factory);
           Ok(())
       }
   }

   /// Creates an agent instance based on its configuration.
   ///
   /// Looks up the `agent_kind` from the `agent_config` in the registry
   /// and uses the corresponding factory to create the agent.
   /// The `event_publisher` is available for constructing `AgentContext` but not directly passed to the simple factory.
   ///
   /// # Arguments
   /// * `agent_config` - The configuration for the agent to be created (Arc'd).
   /// * `_event_publisher` - The event publisher (currently not directly passed to the simplified factory).
   ///
   /// # Errors
   /// Returns an `SdkError` if no factory is found for the agent kind or if
   /// the factory itself fails.
   pub fn create_agent(
       &self,
       agent_config: Arc<AgentConfig>,
       _event_publisher: Arc<dyn EventPublisher>, // Marked as unused for now, as factory takes AgentConfig directly
   ) -> Result<Box<dyn SdkAgent>, SdkError> {
       let kind = &agent_config.agent_kind; // This is AgentKind struct
       match self.factories.get(kind) {
           // We need to dereference and clone the Arc<AgentConfig> to pass AgentConfig to the factory
           Some(factory) => factory((*agent_config).clone()), // Pass AgentConfig, not Arc<AgentConfig>
           None => Err(SdkError::ConfigurationError(format!(
               "No factory registered for agent kind '{}'",
               kind.value()
           ))),
       }
   }
}

// Default implementation for AgentRegistry
impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}
// Removed placeholder DummySourceAgent and DummyActionAgent definitions
// as they are now defined within the integration test suite (integration_tests.rs)
// or should be part of a separate example/test crate if generally needed.