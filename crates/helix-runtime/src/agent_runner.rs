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


//! Manages the lifecycle and execution of Helix agents.

// use helix_core::agent::Agent; // Agent trait from helix-core
use helix_core::agent::{AgentConfig, AgentRuntime}; // Use AgentConfig and AgentRuntime from helix-core
use helix_core::types::{AgentId, ProfileId, EventId}; // Import ProfileId, EventId
use helix_core::errors::HelixError;
use helix_core::recipe::{Recipe, RecipeGraphDefinition, RecipeId}; // Added Recipe, RecipeGraphDefinition, RecipeId
use helix_core::event::Event; // Added Event
use helix_core::profile::Profile; // Added Profile for start_agent
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Arc, Mutex}; // Added Mutex
use async_trait::async_trait;
use tokio::sync::oneshot; // For signaling agent stop
use tracing::{info, error, warn};
use serde_json; // Added for deserialization

use helix_storage::postgres_state_store::PostgresStateStore; // Import PostgresStateStore
use helix_agent_sdk::{SdkAgent, AgentContext, EventPublisher, SdkError, SourceSdkAgent, ActionSdkAgent}; // SDK traits and types, Added SourceSdkAgent, ActionSdkAgent

// Imports from the local agent_registry module
use crate::agent_registry::AgentRegistry;
use crate::messaging::{self, NatsClient}; // Added NatsClient here
use crate::AgentStatus; // Import AgentStatus from lib.rs
use helix_wasm::plugins::PluginManager; // Added PluginManager

// PlaceholderEventPublisher and its implementation removed.
// InMemoryEventCollector struct and its EventPublisher impl removed (moved to messaging.rs).

/// Represents a managed agent instance, including its state and execution handle.

/// Enum to differentiate between native and WASM agent instances.
#[derive(Debug)]
enum ManagedAgentInstance {
    Native(Box<dyn SdkAgent>),
    Wasm, // For WASM agents, PluginManager handles the instance. AgentRunner stores AgentId.
}

#[derive(Debug)]
pub struct ManagedAgent {
    id: AgentId,
    instance_type: ManagedAgentInstance,
    status: AgentStatus,
    // Used to signal the agent's main loop to stop (primarily for native agents).
    // For WASM, PluginManager might have its own shutdown mechanisms.
    stop_sender: Option<oneshot::Sender<()>>,
}

impl ManagedAgent {
    /// Creates a new ManagedAgent for a native SdkAgent.
    pub fn new_native(id: AgentId, instance: Box<dyn SdkAgent>, stop_sender: Option<oneshot::Sender<()>>) -> Self {
        ManagedAgent {
            id,
            instance_type: ManagedAgentInstance::Native(instance),
            status: AgentStatus::Initializing,
            stop_sender,
        }
    }

    /// Creates a new ManagedAgent for a WASM agent.
    /// The actual WASM instance is managed by PluginManager.
    pub fn new_wasm(id: AgentId, stop_sender: Option<oneshot::Sender<()>>) -> Self {
        // For WASM, stop_sender might be less relevant if PluginManager handles lifecycle.
        // It's included for consistency but might often be None or handled differently.
        ManagedAgent {
            id,
            instance_type: ManagedAgentInstance::Wasm,
            status: AgentStatus::Initializing,
            stop_sender,
        }
    }

    pub fn status(&self) -> AgentStatus {
        self.status.clone()
    }

    pub fn set_status(&mut self, status: AgentStatus) {
        self.status = status;
    }

    pub fn agent_id(&self) -> &AgentId {
        &self.id
    }

    /// Returns a mutable reference to the native SdkAgent instance, if applicable.
    pub fn native_instance_mut(&mut self) -> Option<&mut Box<dyn SdkAgent>> {
        match &mut self.instance_type {
            ManagedAgentInstance::Native(instance) => Some(instance),
            ManagedAgentInstance::Wasm => None,
        }
    }

    pub fn is_wasm(&self) -> bool {
        matches!(self.instance_type, ManagedAgentInstance::Wasm)
    }
}


/// The central component responsible for loading, running, and managing agents.
pub struct AgentRunner {
    // Stores managed agent instances, allowing for status tracking and control.
    // Arc<Mutex<ManagedAgent>> allows for shared mutable access if AgentRunner methods
    // are called concurrently or if agent status needs to be updated from background tasks.
    agents: Arc<Mutex<HashMap<AgentId, Arc<Mutex<ManagedAgent>>>>>,
    db_pool: PgPool,
    agent_registry: Arc<AgentRegistry>,
    plugin_manager: Arc<PluginManager>, // Added PluginManager
    event_publisher: Arc<dyn EventPublisher>,
    nats_client: Option<NatsClient>, // Use imported NatsClient
}

// AgentInstance struct removed as it's more relevant for Task 1.5.3 (lifecycle management)

impl AgentRunner {
    /// Creates a new AgentRunner.
    ///
    /// # Arguments
    /// * `db_pool` - A PostgreSQL connection pool.
    /// * `agent_registry` - An Arc-wrapped `AgentRegistry` containing agent factories.
    /// * `event_publisher` - An Arc-wrapped `EventPublisher` for agents to publish events.
    /// * `nats_client` - An optional NATS client for more advanced messaging.
    pub fn new(
        db_pool: PgPool,
        agent_registry: Arc<AgentRegistry>,
        plugin_manager: Arc<PluginManager>, // Added plugin_manager
        event_publisher: Arc<dyn EventPublisher>,
        nats_client: Option<NatsClient>, // Use imported NatsClient
    ) -> Self {
        info!("Creating new AgentRunner");
        AgentRunner {
            agents: Arc::new(Mutex::new(HashMap::new())),
            db_pool,
            agent_registry,
            plugin_manager, // Store plugin_manager
            event_publisher,
            nats_client,
        }
    }

    /// Starts a new agent based on a recipe and profile.
    ///
    /// This method will:
    /// 1. Identify the agent to start from the recipe (e.g., the first agent in a simple recipe).
    /// 2. Fetch the `AgentConfig` for that agent.
    /// 3. Instantiate the agent using `AgentRegistry`.
    /// 4. Create an `AgentContext`.
    /// 5. Initialize and start the agent.
    /// 6. Store the `ManagedAgent` in the `agents` map with `AgentStatus::Running`.
    ///
    /// # Arguments
    /// * `recipe` - The recipe defining the agent(s) to run.
    /// * `agent_profile` - The profile associated with this agent execution.
    /// * `agent_id_override` - Optionally specify which agent from the recipe to start. If None, logic to pick one is needed.
    ///
    /// # Returns
    /// The `AgentId` of the started agent or an error.
    pub async fn start_agent(
        &mut self,
        recipe: &Recipe,
        _agent_profile: &Profile, // Profile might be used for context, permissions, etc.
        agent_id_to_start: &AgentId, // Explicitly specify which agent from the recipe to start
    ) -> Result<AgentId, HelixError> {
        info!(recipe_id = %recipe.id, agent_id = %agent_id_to_start, "Attempting to start agent");

        // 1. Fetch AgentConfig (assuming it's stored and accessible via PgPool)
        //    Alternatively, AgentConfig might be part of the Recipe or Profile, or constructed.
        //    For now, assume we fetch it using the agent_id_to_start.
        let store = PostgresStateStore::new(self.db_pool.clone());
        let agent_config = store.get_agent_config(agent_id_to_start).await?
            .ok_or_else(|| HelixError::AgentConfigNotFoundError(agent_id_to_start.clone()))?;
        
        if self.agents.lock().unwrap().contains_key(agent_id_to_start) {
            warn!(agent_id = %agent_id_to_start, "Agent already running or managed. Skipping start.");
            return Ok(agent_id_to_start.clone());
        }

        let agent_config_arc = Arc::new(agent_config.clone());
        let (stop_tx, _stop_rx) = oneshot::channel::<()>(); // _stop_rx for native agent's loop

        let mut managed_agent;

        match agent_config_arc.agent_runtime {
            AgentRuntime::Native => {
                info!(agent_id = %agent_id_to_start, "Starting native agent");
                let agent_instance = self.agent_registry.create_agent(
                    agent_config_arc.clone(),
                    self.event_publisher.clone(),
                )?;
                
                managed_agent = ManagedAgent::new_native(agent_id_to_start.clone(), agent_instance, Some(stop_tx));
                managed_agent.set_status(AgentStatus::Initializing);

                let context = AgentContext::new(agent_config_arc.clone(), self.event_publisher.clone());

                if let Some(native_instance) = managed_agent.native_instance_mut() {
                    if let Err(e) = native_instance.init(&context).await {
                        error!(agent_id = %agent_id_to_start, error = %e, "Failed to initialize native agent");
                        managed_agent.set_status(AgentStatus::Errored);
                        return Err(HelixError::AgentError(format!("Initialization failed for {}: {}", agent_id_to_start, e)));
                    }
                    info!(agent_id = %agent_id_to_start, "Native agent initialized successfully");

                    if let Err(e) = native_instance.start().await {
                        error!(agent_id = %agent_id_to_start, error = %e, "Failed to start native agent");
                        managed_agent.set_status(AgentStatus::Errored);
                        return Err(HelixError::AgentError(format!("Start failed for {}: {}", agent_id_to_start, e)));
                    }
                    managed_agent.set_status(AgentStatus::Running);
                    info!(agent_id = %agent_id_to_start, "Native agent started and is now running");
                } else {
                    // Should not happen if agent_runtime is Native
                    unreachable!("Native agent instance expected but not found");
                }
            }
            AgentRuntime::Wasm => {
                info!(agent_id = %agent_id_to_start, "Starting WASM agent");
                let wasm_path = agent_config_arc.wasm_module_path.as_ref().ok_or_else(|| {
                    HelixError::InvalidConfigurationError(format!(
                        "WASM module path missing for agent {}",
                        agent_id_to_start
                    ))
                })?;

                // The PluginManager needs the AgentConfig to set up HostState.
                match self.plugin_manager.load_plugin(
                    agent_id_to_start.clone(),
                    &wasm_path,
                    agent_config_arc.as_ref().clone(), // Pass the AgentConfig
                ).await {
                    Ok(_) => {
                        info!(agent_id = %agent_id_to_start, path = %wasm_path, "WASM agent plugin loaded successfully by PluginManager");
                        managed_agent = ManagedAgent::new_wasm(agent_id_to_start.clone(), Some(stop_tx));
                        managed_agent.set_status(AgentStatus::Running);
                        info!(agent_id = %agent_id_to_start, "WASM agent is now considered running");
                    }
                    Err(wasm_err) => {
                        error!(agent_id = %agent_id_to_start, path = %wasm_path, error = %wasm_err, "Failed to load WASM agent plugin");
                        return Err(HelixError::WasmError(wasm_err.to_string()));
                    }
                }
            }
        }
        
        // Store the ManagedAgent
        self.agents.lock().unwrap().insert(agent_id_to_start.clone(), Arc::new(Mutex::new(managed_agent)));
        Ok(agent_id_to_start.clone())
    }

    /// Stops a running agent.
    ///
    /// # Arguments
    /// * `agent_id` - The ID of the agent to stop.
    ///
    /// # Returns
    /// Ok(()) if stopping was initiated, or an error if the agent is not found or stop fails.
    pub async fn stop_agent(&mut self, agent_id: &AgentId) -> Result<(), HelixError> {
        info!(%agent_id, "Attempting to stop agent");
        let mut agents_map = self.agents.lock().unwrap();

        if let Some(managed_agent_arc) = agents_map.get(agent_id) {
            let mut managed_agent = managed_agent_arc.lock().unwrap();
            
            if managed_agent.status() == AgentStatus::Stopped || managed_agent.status() == AgentStatus::Completed {
                info!(%agent_id, status = ?managed_agent.status(), "Agent already stopped or completed.");
                return Ok(());
            }

            // Signal the agent to stop if it has a stop_sender
            if let Some(stop_sender) = managed_agent.stop_sender.take() {
                let _ = stop_sender.send(()); // Result ignored as receiver might have dropped
            }

            // Call the agent's own stop method if native
            if let Some(native_instance) = managed_agent.native_instance_mut() {
                match native_instance.stop().await {
                    Ok(_) => {
                        info!(%agent_id, "Native agent stop method completed successfully.");
                        managed_agent.set_status(AgentStatus::Stopped);
                    }
                    Err(e) => {
                        error!(%agent_id, error = %e, "Error calling native agent's stop method.");
                        managed_agent.set_status(AgentStatus::Errored);
                        return Err(HelixError::AgentError(format!("Failed to stop agent {}: {}", agent_id, e)));
                    }
                }
            } else if managed_agent.is_wasm() {
                info!(%agent_id, "Stopping WASM agent via PluginManager.");
                match self.plugin_manager.unload_plugin(agent_id).await {
                    Ok(_) => {
                        info!(%agent_id, "WASM agent unloaded successfully by PluginManager.");
                        managed_agent.set_status(AgentStatus::Stopped);
                    }
                    Err(e) => {
                        error!(%agent_id, error = %e, "Error unloading WASM agent plugin.");
                        managed_agent.set_status(AgentStatus::Errored);
                        return Err(HelixError::WasmError(format!("Failed to unload WASM agent {}: {}", agent_id, e)));
                    }
                }
            }
            Ok(())
        } else {
            warn!(%agent_id, "Agent not found for stopping.");
            Err(HelixError::AgentNotFoundError(agent_id.clone()))
        }
    }

    /// Gets the status of a managed agent.
    ///
    /// # Arguments
    /// * `agent_id` - The ID of the agent to query.
    ///
    /// # Returns
    /// `Some(AgentStatus)` if the agent is found, `None` otherwise.
    pub fn get_agent_status(&self, agent_id: &AgentId) -> Option<AgentStatus> {
        self.agents.lock().unwrap()
            .get(agent_id)
            .map(|managed_agent_arc| managed_agent_arc.lock().unwrap().status())
    }


    /// Loads agent configurations from the database for a specific profile.
    pub async fn load_configs_from_db(&self, profile_id: &ProfileId) -> Result<Vec<AgentConfig>, HelixError> {
        info!(profile_id = %profile_id, "Loading agent configurations from database for profile");
        
        let store = PostgresStateStore::new(self.db_pool.clone());

        match store.list_agent_configs_by_profile(profile_id).await {
            Ok(configs) => {
                info!(profile_id = %profile_id, count = configs.len(), "Successfully loaded agent configurations");
                Ok(configs)
            }
            Err(e) => {
                error!(profile_id = %profile_id, error = %e, "Failed to fetch agent configurations from DB");
                // Ensure HelixError::DatabaseError or a similar variant exists and is appropriate
                // For now, using InternalError as per PostgresStateStore's error mapping
                Err(HelixError::InternalError(format!(
                    "Failed to load agent configurations for profile {}: {}",
                    profile_id, e
                )))
            }
        }
    }

    /// Instantiates agents based on the loaded configurations and stores them.
    /// This is part of Task 1.5.2.
    pub async fn instantiate_agents_from_configs(
        &mut self,
        configs: Vec<AgentConfig>,
    ) -> Result<(), HelixError> {
        info!("Instantiating agents from {} configurations", configs.len());
        let mut successful_instantiations = 0;

        for config in configs {
            let agent_id = config.id.clone();
            let agent_config_arc = Arc::new(config);
            let log_agent_id = agent_config_arc.id.clone();
            let log_agent_kind = agent_config_arc.agent_kind.clone(); // Functional kind
            let agent_runtime = agent_config_arc.agent_runtime.clone();

            let (stop_tx, _stop_rx) = oneshot::channel::<()>();
            let mut managed_agent_option: Option<ManagedAgent> = None;

            match agent_runtime {
                AgentRuntime::Native => {
                    info!(agent_id = %log_agent_id, kind = %log_agent_kind, "Instantiating native agent from config");
                    match self.agent_registry.create_agent(
                        agent_config_arc.clone(),
                        self.event_publisher.clone(),
                    ) {
                        Ok(agent_instance_box) => {
                            let mut managed_agent = ManagedAgent::new_native(log_agent_id.clone(), agent_instance_box, Some(stop_tx));
                            let context = AgentContext::new(agent_config_arc.clone(), self.event_publisher.clone());
                            if let Some(native_instance) = managed_agent.native_instance_mut() {
                                match native_instance.init(&context).await {
                                    Ok(_) => {
                                        managed_agent.set_status(AgentStatus::Initializing);
                                        match native_instance.start().await {
                                            Ok(_) => {
                                                managed_agent.set_status(AgentStatus::Running);
                                                managed_agent_option = Some(managed_agent);
                                                successful_instantiations += 1;
                                                info!(agent_id = %log_agent_id, "Native agent started successfully from config.");
                                            }
                                            Err(e) => {
                                                error!(agent_id = %log_agent_id, error = %e, "Failed to start native agent from config.");
                                                managed_agent.set_status(AgentStatus::Errored);
                                                // Optionally store errored agent: self.agents.lock().unwrap().insert(agent_id.clone(), Arc::new(Mutex::new(managed_agent)));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!(agent_id = %log_agent_id, error = %e, "Failed to initialize native agent from config.");
                                        managed_agent.set_status(AgentStatus::Errored);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!(agent_id = %log_agent_id, kind = %log_agent_kind, error = %e, "Failed to create native agent instance from registry");
                        }
                    }
                }
                AgentRuntime::Wasm => {
                    info!(agent_id = %log_agent_id, kind = %log_agent_kind, "Instantiating WASM agent from config");
                    if let Some(wasm_path) = agent_config_arc.wasm_module_path.as_ref() {
                        match self.plugin_manager.load_plugin(
                            log_agent_id.clone(),
                            wasm_path,
                            agent_config_arc.as_ref().clone(),
                        ).await {
                            Ok(_) => {
                                info!(agent_id = %log_agent_id, path = %wasm_path, "WASM agent plugin loaded successfully from config.");
                                let mut managed_agent = ManagedAgent::new_wasm(log_agent_id.clone(), Some(stop_tx));
                                managed_agent.set_status(AgentStatus::Running); // Assume running after load
                                managed_agent_option = Some(managed_agent);
                                successful_instantiations += 1;
                            }
                            Err(wasm_err) => {
                                error!(agent_id = %log_agent_id, path = %wasm_path, error = %wasm_err, "Failed to load WASM agent plugin from config.");
                                // Optionally create a ManagedAgent with Errored status
                                // let mut managed_agent = ManagedAgent::new_wasm(log_agent_id.clone(), Some(stop_tx));
                                // managed_agent.set_status(AgentStatus::Errored);
                                // self.agents.lock().unwrap().insert(agent_id.clone(), Arc::new(Mutex::new(managed_agent)));
                            }
                        }
                    } else {
                        error!(agent_id = %log_agent_id, "WASM module path missing in config for WASM agent.");
                    }
                }
            }

            if let Some(ma) = managed_agent_option {
                self.agents.lock().unwrap().insert(agent_id.clone(), Arc::new(Mutex::new(ma)));
            }
        }
        info!("Successfully instantiated {} agents.", successful_instantiations);
        if successful_instantiations < configs.len() {
            warn!("{} agents failed to instantiate.", configs.len() - successful_instantiations);
            // Potentially return an error or a summary of failures if needed.
            // For now, completing successfully if at least some agents are up.
        }
        Ok(())
    }

    /// Stops all active agents, typically called during shutdown.
    pub async fn stop_all_agents(&mut self) {
        let mut agents_map_guard = self.agents.lock().unwrap();
        info!("Stopping all active agents. Total agents at start of stop sequence: {}", agents_map_guard.len());
        let mut successfully_stopped_count = 0;
        
        let agent_ids_to_stop: Vec<AgentId> = agents_map_guard.keys().cloned().collect();
        let total_agents_to_stop = agent_ids_to_stop.len();

        // Drop the guard before await points if stop_agent itself tries to lock `self.agents`
        // However, stop_agent as implemented takes &mut self, so this loop needs to be careful.
        // A better pattern might be to call a helper that doesn't take &mut self.
        // For now, let's assume stop_agent can be called like this.
        // Or, collect ManagedAgent Arcs and operate on them outside the main lock.
        
        // To avoid deadlock if stop_agent re-locks, collect Arcs first.
        let managed_agent_arcs: Vec<Arc<Mutex<ManagedAgent>>> = agents_map_guard.values().cloned().collect();
        drop(agents_map_guard); // Release the lock on the HashMap

        for managed_agent_arc in managed_agent_arcs {
            let agent_id_clone;
            { // Scope for locking individual agent
                let mut managed_agent = managed_agent_arc.lock().unwrap();
                agent_id_clone = managed_agent.agent_id().clone(); // Clone for logging outside lock

                if managed_agent.status() == AgentStatus::Stopped || managed_agent.status() == AgentStatus::Completed {
                    info!(agent_id = %agent_id_clone, status = ?managed_agent.status(), "Agent already stopped or completed.");
                    successfully_stopped_count += 1; // Count it as "successfully stopped" in this context
                    continue;
                }

                info!(agent_id = %agent_id_clone, "Attempting to stop agent...");
                 // Signal the agent to stop if it has a stop_sender
                if let Some(stop_sender) = managed_agent.stop_sender.take() {
                    let _ = stop_sender.send(()); // Signal native agent's loop
                }

                if let Some(native_instance) = managed_agent.native_instance_mut() {
                    match native_instance.stop().await {
                        Ok(_) => {
                            info!(agent_id = %agent_id_clone, "Native agent stopped successfully.");
                            managed_agent.set_status(AgentStatus::Stopped);
                            successfully_stopped_count += 1;
                        }
                        Err(e) => {
                            error!(agent_id = %agent_id_clone, error = %e, "Error stopping native agent.");
                            managed_agent.set_status(AgentStatus::Errored);
                        }
                    }
                } else if managed_agent.is_wasm() {
                    info!(agent_id = %agent_id_clone, "Stopping WASM agent in stop_all_agents via PluginManager.");
                    match self.plugin_manager.unload_plugin(&agent_id_clone).await {
                        Ok(_) => {
                            info!(agent_id = %agent_id_clone, "WASM agent unloaded successfully by PluginManager during stop_all.");
                            managed_agent.set_status(AgentStatus::Stopped);
                            successfully_stopped_count += 1;
                        }
                        Err(e) => {
                            error!(agent_id = %agent_id_clone, error = %e, "Error unloading WASM agent plugin during stop_all.");
                            managed_agent.set_status(AgentStatus::Errored);
                        }
                    }
                }
            } // Individual agent lock released
        }

        if total_agents_to_stop > 0 {
            if successfully_stopped_count < total_agents_to_stop {
                warn!(
                    "Successfully stopped {} out of {} agents. {} agents may not have stopped cleanly or were already stopped.",
                    successfully_stopped_count,
                    total_agents_to_stop,
                    total_agents_to_stop - successfully_stopped_count
                );
            } else {
                info!("All {} agents processed for stopping successfully.", successfully_stopped_count);
            }
        } else {
            info!("No active agents were present to stop.");
        }

        // Clear the agents map as this is part of a shutdown sequence for the AgentRunner.
        // Or, keep them if their final status is important for querying later.
        // For now, let's clear, assuming `stop_all_agents` is for full shutdown.
        self.agents.lock().unwrap().clear();
        info!("Managed agent collection cleared after stop sequence.");
    }

    // TODO (Task 1.5.4): Implement logic for executing simple recipes.
    /// Executes a recipe identified by `recipe_id`.
    ///
    /// This method fetches the recipe, deserializes its graph, and orchestrates
    /// the execution of agents in a linear sequence (source -> action).
    ///
    /// # Arguments
    /// * `recipe_id` - The ID of the recipe to execute.
    /// * `_initial_event` - An optional initial event to trigger the recipe (currently unused).
    ///
    /// # Errors
    /// Returns `HelixError` if the recipe cannot be found, is disabled,
    /// or if any agent execution fails.
    ///
    /// **Assumptions for this implementation:**
    /// - `run_recipe` takes `&mut self` for mutable access to agents.
    /// - `SdkAgent` trait (or `Box<dyn SdkAgent>`) provides `as_source_mut()` and `as_action_mut()` methods
    ///   for safe downcasting. These are placeholder calls for required SDK functionality.
    /// - `PostgresStateStore` has `get_agent_config(agent_id: &AgentId)` method.
    /// - `HelixError` has `RecipeDisabledError(RecipeId)` and `RecipeNotFoundError(RecipeId)`.
    pub async fn run_recipe(
        &mut self, // Changed to &mut self
        recipe_id: &RecipeId,
        _initial_event: Option<Event>, // initial_event is not used in this simplified version
    ) -> Result<(), HelixError> {
        info!(%recipe_id, "Attempting to run recipe");

        let store = PostgresStateStore::new(self.db_pool.clone());
        let recipe = match store.get_recipe(recipe_id).await {
            Ok(Some(r)) => {
                if !r.enabled {
                    error!(%recipe_id, "Recipe is disabled");
                    // Assumes HelixError::RecipeDisabledError exists
                    return Err(HelixError::InternalError(format!("Recipe {} is disabled", recipe_id)));
                }
                info!(%recipe_id, "Recipe found and enabled");
                r
            }
            Ok(None) => {
                error!(%recipe_id, "Recipe not found");
                // Assumes HelixError::RecipeNotFoundError exists
                return Err(HelixError::InternalError(format!("Recipe {} not found", recipe_id)));
            }
            Err(e) => {
                error!(%recipe_id, error = %e, "Failed to fetch recipe from DB");
                return Err(HelixError::DatabaseError(format!(
                    "Failed to fetch recipe {}: {}",
                    recipe_id, e
                )));
            }
        };

        let graph_definition: RecipeGraphDefinition = match serde_json::from_value(recipe.graph.0) {
            Ok(gd) => gd,
            Err(e) => {
                error!(%recipe_id, error = %e, "Failed to deserialize recipe graph definition");
                return Err(HelixError::RecipeGraphError(format!(
                    "Failed to deserialize graph for recipe {}: {}",
                    recipe_id, e
                )));
            }
        };

        info!(%recipe_id, "Recipe graph definition deserialized. Agents in graph: {}", graph_definition.agents.len());

        // Simplified identification: first agent is source, second is action.
        let source_agent_node = graph_definition.agents.get(0).ok_or_else(|| {
            error!(%recipe_id, "Recipe graph does not contain a source agent (expected at index 0)");
            HelixError::RecipeGraphError("Recipe graph must contain at least a source agent".to_string())
        })?;
        let action_agent_node = graph_definition.agents.get(1).ok_or_else(|| {
            error!(%recipe_id, "Recipe graph does not contain an action agent (expected at index 1)");
            HelixError::RecipeGraphError("Recipe graph must contain at least an action agent for this model".to_string())
        })?;

        let source_agent_id = &source_agent_node.agent_id;
        let action_agent_id = &action_agent_node.agent_id;

        info!(%recipe_id, %source_agent_id, %action_agent_id, "Identified source and action agents for linear execution");

        // Fetch AgentConfigs for context creation (suboptimal: ideally cached or part of agent instance)
        let source_agent_config = store.get_agent_config(source_agent_id).await?
            .ok_or_else(|| HelixError::AgentConfigNotFoundError(source_agent_id.clone()))?;
        let action_agent_config = store.get_agent_config(action_agent_id).await?
            .ok_or_else(|| HelixError::AgentConfigNotFoundError(action_agent_id.clone()))?;

        // Use InMemoryEventCollector from the messaging module
        let collecting_event_publisher = Arc::new(InMemoryEventCollector::new());
        let source_context_for_run = AgentContext::new(
            Arc::new(source_agent_config),
            collecting_event_publisher.clone(),
        );

        // Orchestration
        // This part needs to acquire locks on individual ManagedAgent instances.
        let events_from_source: Vec<Event> = {
            let agents_guard = self.agents.lock().unwrap();
            let source_managed_agent_arc = agents_guard.get(source_agent_id).ok_or_else(|| {
                error!(%recipe_id, %source_agent_id, "Source agent not found in managed agents map");
                HelixError::AgentNotFoundError(source_agent_id.clone())
            })?.clone(); // Clone Arc to release agents_guard lock
            drop(agents_guard);

            let mut source_managed_agent = source_managed_agent_arc.lock().unwrap();
            if source_managed_agent.status() != AgentStatus::Running {
                 error!(%recipe_id, %source_agent_id, status = ?source_managed_agent.status(), "Source agent is not in Running state for recipe execution.");
                 return Err(HelixError::AgentError(format!("Source agent {} not running", source_agent_id)));
            }

            // Determine agent type and execute accordingly
            if source_managed_agent.is_wasm() {
                info!(%recipe_id, %source_agent_id, "Running WASM source agent for recipe");
                // For WASM, the context is the AgentConfig.
                // The `source_agent_config` is already fetched and is the correct one.
                let serialized_config = serde_json::to_value(&source_agent_config).map_err(|e|
                    HelixError::SerializationError(format!("Failed to serialize AgentConfig for WASM source {}: {}", source_agent_id, e))
                )?;

                match self.plugin_manager.call_plugin_function(
                    source_agent_id,
                    "_helix_run_source", // Agreed upon function name
                    serialized_config
                ).await {
                    Ok(result_json) => {
                        serde_json::from_value(result_json).map_err(|e| {
                            error!(%recipe_id, %source_agent_id, error = %e, "Failed to deserialize events from WASM source agent");
                            source_managed_agent.set_status(AgentStatus::Errored);
                            HelixError::DeserializationError(format!("Failed to deserialize events from WASM source {}: {}", source_agent_id, e))
                        })?
                    }
                    Err(wasm_err) => {
                        error!(%recipe_id, %source_agent_id, error = %wasm_err, "WASM source agent execution failed");
                        source_managed_agent.set_status(AgentStatus::Errored);
                        Err(HelixError::WasmError(format!("WASM source agent {} failed: {}", source_agent_id, wasm_err)))?
                    }
                }
            } else if let Some(source_agent_ref) = source_managed_agent.native_instance_mut().and_then(|i| i.as_source_mut()) {
                info!(%recipe_id, %source_agent_id, "Running native source agent for recipe");
                source_agent_ref.run(&source_context_for_run).await.map_err(|sdk_err| {
                    error!(%recipe_id, %source_agent_id, error = %sdk_err, "Native source agent execution failed in recipe");
                    source_managed_agent.set_status(AgentStatus::Errored);
                    HelixError::AgentError(format!("Native source agent {} failed during recipe: {}", source_agent_id, sdk_err))
                })?;
                collecting_event_publisher.get_events().await // Native agents use the collector
            } else {
                 error!(%recipe_id, %source_agent_id, "Source agent is neither a valid native source nor a WASM agent.");
                 return Err(HelixError::AgentError(format!("Agent {} is not a valid source type", source_agent_id)));
            }
        };

        if events_from_source.is_empty() {
            warn!(%recipe_id, %source_agent_id, "Source agent did not produce any events for recipe.");
        } else {
            info!(%recipe_id, count = events_from_source.len(), "Source agent produced events for recipe. Passing to action agent.");
        }

        let agents_guard = self.agents.lock().unwrap();
        let action_managed_agent_arc = agents_guard.get(action_agent_id).ok_or_else(|| {
            error!(%recipe_id, %action_agent_id, "Action agent not found in managed agents map");
            HelixError::AgentNotFoundError(action_agent_id.clone())
        })?.clone();
        drop(agents_guard);

        let mut action_managed_agent = action_managed_agent_arc.lock().unwrap();
        if action_managed_agent.status() != AgentStatus::Running {
            error!(%recipe_id, %action_agent_id, status = ?action_managed_agent.status(), "Action agent is not in Running state for recipe execution.");
            return Err(HelixError::AgentError(format!("Action agent {} not running", action_agent_id)));
        }

        for event in events_from_source {
            info!(%recipe_id, %action_agent_id, event_id = %event.id, "Processing event with action agent");
            let action_context_for_run = AgentContext::new(
                Arc::new(action_agent_config.clone()),
                self.event_publisher.clone(),
            );

            if action_managed_agent.is_wasm() {
                info!(%recipe_id, %action_agent_id, event_id = %event.id, "Executing event with WASM action agent");
                // For WASM, payload includes AgentConfig and the Event.
                // `action_agent_config` is already fetched.
                let params = serde_json::json!({
                    "config": action_agent_config, // The full AgentConfig
                    "event": event
                });
                
                match self.plugin_manager.call_plugin_function(
                    action_agent_id,
                    "_helix_execute_event", // Agreed upon function name
                    params
                ).await {
                    Ok(_result_json) => { // Assuming _helix_execute_event might return null or a status
                        // info!(%recipe_id, %action_agent_id, "WASM action agent executed event successfully.");
                    }
                    Err(wasm_err) => {
                        error!(%recipe_id, %action_agent_id, error = %wasm_err, "WASM action agent execution failed for an event");
                        action_managed_agent.set_status(AgentStatus::Errored);
                        return Err(HelixError::WasmError(format!("WASM action agent {} failed: {}", action_agent_id, wasm_err)));
                    }
                }
            } else if let Some(action_agent_ref) = action_managed_agent.native_instance_mut().and_then(|i| i.as_action_mut()) {
                info!(%recipe_id, %action_agent_id, event_id = %event.id, "Executing event with native action agent");
                action_agent_ref.execute_event(&action_context_for_run, event).await.map_err(|sdk_err| {
                    error!(%recipe_id, %action_agent_id, error = %sdk_err, "Native action agent execution failed for an event in recipe");
                    action_managed_agent.set_status(AgentStatus::Errored);
                    HelixError::AgentError(format!("Native action agent {} failed during recipe: {}", action_agent_id, sdk_err))
                })?;
            } else {
                error!(%recipe_id, %action_agent_id, "Action agent is neither a valid native action nor a WASM agent.");
                return Err(HelixError::AgentError(format!("Agent {} is not a valid action type", action_agent_id)));
            }
        }
        info!(%recipe_id, "Recipe execution processed by agents successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use helix_core::{
        agent::AgentConfig,
        errors::HelixError,
        types::{AgentId, ProfileId},
    };
    use mockall::predicate::*;
    use serde_json::json;
    use sqlx::postgres::PgPoolOptions;
    use std::sync::Arc;
    use uuid::Uuid;

    // IMPORTANT ASSUMPTION FOR THESE TESTS:
    // These tests assume that the `PostgresStateStore` struct in the `helix-storage` crate
    // and its `new` associated function are annotated with `#[cfg_attr(test, mockall::automock)]`.
    // This would generate `helix_storage::postgres_state_store::MockPostgresStateStore`
    // and allow mocking its `new` method and instance methods as shown below.
    // Without such changes in `helix-storage`, these unit tests for `load_configs_from_db`
    // cannot mock `PostgresStateStore` as it's instantiated internally.

    // Due to the above, we'd need to import the mock like this:
    // use helix_storage::postgres_state_store::MockPostgresStateStore;
    // For the purpose of this example, we will define a local mock struct
    // that mirrors what `automock` would generate for `PostgresStateStore`.
    // In a real scenario, `helix-storage` would provide the actual mock.

    mod mock_db_mod {
        // This inner module is to scope the mock definition if we were defining it locally.
        // In reality, this mock would come from `helix_storage`.
        use super::*; // Import types from outer scope for the mock definition
        use helix_storage::postgres_state_store::PostgresStateStore as RealPostgresStateStore;
        use sqlx::PgPool; // For the `new` method signature

        mockall::mock! {
            pub PostgresStateStore { // Name matches the struct we are mocking
                // Mock the `new` associated function
                // The real signature: pub fn new(pool: PgPool) -> Self
                // `mockall` handles `new` specially for mocking associated functions.
                // We will use `MockPostgresStateStore::new_context()` for this.

                // Mock the instance method
                // pub async fn list_agent_configs_by_profile(&self, profile_id: &ProfileId) -> Result<Vec<AgentConfig>, HelixError>
                pub async fn list_agent_configs_by_profile(&self, profile_id: &ProfileId) -> Result<Vec<AgentConfig>, HelixError>;

                // Add other methods of PostgresStateStore if they were to be mocked for other tests.
                // For example:
                // pub async fn get_agent_config(&self, agent_id: &AgentId) -> Result<Option<AgentConfig>, HelixError>;
                // pub async fn get_recipe(&self, recipe_id: &RecipeId) -> Result<Option<Recipe>, HelixError>;
            }
        }

        // This is how we'd mock the `new` function if `PostgresStateStore` itself was local or `automock` was used.
        // For an external crate, `automock` on the original struct is the way.
        // The `new_context()` is generated by `#[automock]` on the struct or `mock!` for the struct.
        // So, we'd use `MockPostgresStateStore::new_context()` if `helix_storage::postgres_state_store::PostgresStateStore` was `automock`ed.
    }
    // We will use `mock_db_mod::MockPostgresStateStore` as the type for our mock instance.
    // And `mock_db_mod::MockPostgresStateStore::new_context()` to mock the `new` call.


    #[tokio::test]
    async fn test_load_configs_from_db_success() {
        let profile_id = ProfileId::new_v4();
        let expected_configs = vec![AgentConfig {
            id: AgentId::new_v4(),
            profile_id,
            name: Some("Test Agent".to_string()),
            agent_kind: "dummy_kind".to_string(),
            config_data: json!({"key": "value"}),
            credential_ids: vec![],
            enabled: true,
        }];
        let expected_configs_clone = expected_configs.clone();

        // 1. Create a mock instance of PostgresStateStore
        let mut mock_store_instance = mock_db_mod::MockPostgresStateStore::default();
        mock_store_instance
            .expect_list_agent_configs_by_profile()
            .with(eq(profile_id))
            .times(1)
            .returning(move |_| Ok(expected_configs_clone.clone()));

        // 2. Get the context for mocking the `PostgresStateStore::new` associated function
        let new_context = mock_db_mod::MockPostgresStateStore::new_context();
        new_context
            .expect()
            .times(1)
            // The argument to `new` is PgPool. We can ignore it in the mock setup if not used.
            .returning(move |_pool| mock_store_instance); // `new` returns our mock_store_instance

        // 3. Create AgentRunner
        // `PgPool::connect_lazy` creates a pool that doesn't connect immediately.
        // This is suitable for unit tests where the actual DB connection is not needed.
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://mockuser:mockpass@localhost/mockdb")
            .expect("Failed to create lazy PgPool");
        let agent_runner = AgentRunner::new(pool);

        // 4. Call the function under test
        let result = agent_runner.load_configs_from_db(&profile_id).await;

        // 5. Assertions
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_configs);

        // `new_context` expectation is consumed, drops here.
    }

    #[tokio::test]
    async fn test_load_configs_from_db_error() {
        let profile_id = ProfileId::new_v4();
        let db_error = HelixError::InternalError("Simulated DB Error".to_string());

        // 1. Create a mock instance of PostgresStateStore
        let mut mock_store_instance = mock_db_mod::MockPostgresStateStore::default();
        mock_store_instance
            .expect_list_agent_configs_by_profile()
            .with(eq(profile_id))
            .times(1)
            .returning(move |_| Err(HelixError::InternalError("Simulated DB Error".to_string()))); // Return the error

        // 2. Get the context for mocking the `PostgresStateStore::new` associated function
        let new_context = mock_db_mod::MockPostgresStateStore::new_context();
        new_context
            .expect()
            .times(1)
            .returning(move |_pool| mock_store_instance);

        // 3. Create AgentRunner
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://mockuser:mockpass@localhost/mockdb")
            .expect("Failed to create lazy PgPool");
        let agent_runner = AgentRunner::new(pool);

        // 4. Call the function under test
        let result = agent_runner.load_configs_from_db(&profile_id).await;

        // 5. Assertions
        assert!(result.is_err());
        match result.err().unwrap() {
            HelixError::InternalError(msg) => {
                // The error message includes the profile_id and the original error.
                // Example: "Failed to load agent configurations for profile {}: {}"
                assert!(msg.contains(&profile_id.to_string()));
                assert!(msg.contains("Simulated DB Error"));
            }
            _ => panic!("Expected HelixError::InternalError"),
        }
        // `new_context` expectation is consumed, drops here.
    }
}
// --- Mocks for Agent Instantiation Tests ---
    use helix_agent_sdk::{AgentContext, SdkAgent, SdkError, SourceSdkAgent, ActionSdkAgent};
    use helix_core::event::Event as CoreEvent; // Alias to avoid conflict with mockall Event

    mockall::mock! {
        pub MockSdkAgent {
            // SdkAgent methods
            fn id<'a>(&'a self) -> &'a AgentId;
            fn kind<'a>(&'a self) -> &'a str;
            async fn init<'a>(&'a mut self, context: &'a AgentContext) -> Result<(), SdkError>;
            async fn start<'a>(&'a mut self) -> Result<(), SdkError>;
            async fn stop<'a>(&'a mut self) -> Result<(), SdkError>;
            fn as_source<'s>(&'s self) -> Option<&'s (dyn SourceSdkAgent + 'static)>; // Note: 's lifetime
            fn as_source_mut<'s>(&'s mut self) -> Option<&'s mut (dyn SourceSdkAgent + 'static)>; // Note: 's lifetime
            fn as_action<'s>(&'s self) -> Option<&'s (dyn ActionSdkAgent + 'static)>; // Note: 's lifetime
            fn as_action_mut<'s>(&'s mut self) -> Option<&'s mut (dyn ActionSdkAgent + 'static)>; // Note: 's lifetime

            // SourceSdkAgent methods (if MockSdkAgent also impls SourceSdkAgent)
            async fn run<'a>(&'a mut self, context: &'a AgentContext) -> Result<(), SdkError>;

            // ActionSdkAgent methods (if MockSdkAgent also impls ActionSdkAgent)
            async fn execute_event<'a>(&'a mut self, context: &'a AgentContext, event: CoreEvent) -> Result<(), SdkError>;
        }
    }

    #[async_trait]
    impl SdkAgent for MockSdkAgent {
        fn id(&self) -> &AgentId {
            // This will call the mocked `id` method from `MockSdkAgent`
            self.id()
        }
        fn kind(&self) -> &str {
            self.kind()
        }
        async fn init(&mut self, context: &AgentContext) -> Result<(), SdkError> {
            self.init(context).await
        }
        async fn start(&mut self) -> Result<(), SdkError> {
            self.start().await
        }
        async fn stop(&mut self) -> Result<(), SdkError> {
            self.stop().await
        }

        // Updated for downcasting: these now rely on MockSdkAgent also implementing
        // SourceSdkAgent and ActionSdkAgent. The mockall macro handles the 'static bound.
        fn as_source(&self) -> Option<&dyn SourceSdkAgent> {
            // The mock! macro generates a method `as_source()` on the mock object.
            // We call that, and it allows us to set expectations on `as_source` itself if needed.
            // However, for the downcast to work, `MockSdkAgent` must implement `SourceSdkAgent`.
            // The cast `self as &dyn SourceSdkAgent` is what we're aiming for.
            // Mockall's generated `as_source()` method should facilitate this.
            // If direct mocking of `as_source` is not needed, one could do:
            // Some(self as &dyn SourceSdkAgent)
            // But it's generally better to use the mock's own method if available.
            // For this to compile, MockSdkAgent must implement SourceSdkAgent.
             self.as_source()
        }
        fn as_source_mut(&mut self) -> Option<&mut dyn SourceSdkAgent> {
            // Similar to as_source, but mutable.
            // Some(self as &mut dyn SourceSdkAgent)
             self.as_source_mut()
        }
        fn as_action(&self) -> Option<&dyn ActionSdkAgent> {
            // Some(self as &dyn ActionSdkAgent)
             self.as_action()
        }
        fn as_action_mut(&mut self) -> Option<&mut dyn ActionSdkAgent> {
            // Some(self as &mut dyn ActionSdkAgent)
             self.as_action_mut()
        }
    }

    #[async_trait]
    impl SourceSdkAgent for MockSdkAgent {
       async fn run(&mut self, context: &AgentContext) -> Result<(), SdkError> {
           // This calls the mocked `run` method from `MockSdkAgent`
           self.run(context).await
       }
    }

    #[async_trait]
    impl ActionSdkAgent for MockSdkAgent {
       async fn execute_event(&mut self, context: &AgentContext, event: CoreEvent) -> Result<(), SdkError> {
           // This calls the mocked `execute_event` method from `MockSdkAgent`
           self.execute_event(context, event).await
       }
    }


    #[tokio::test]
    async fn test_instantiate_agents_success() {
        let agent_id = AgentId::new_v4();
        let profile_id = ProfileId::new_v4();
        let agent_kind_str = "mock_agent";

        let config = AgentConfig {
            id: agent_id,
            profile_id,
            name: Some("Mock Agent Instance".to_string()),
            agent_kind: agent_kind_str.to_string(),
            config_data: json!({}),
            credential_ids: vec![],
            enabled: true,
        };
        let configs = vec![config.clone()];

        let mut mock_agent_instance = MockSdkAgent::new();
        mock_agent_instance.expect_id().return_const(agent_id); // Or .returning(move || &agent_id_clone) if AgentId is not Copy
        mock_agent_instance.expect_kind().return_const(agent_kind_str);
        mock_agent_instance.expect_init().times(1).returning(|_ctx| Ok(()));
        mock_agent_instance.expect_start().times(1).returning(|| Ok(()));
        // No stop() expected during instantiation

        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://mockuser:mockpass@localhost/mockdb")
            .expect("Failed to create lazy PgPool");
        
        let mut agent_runner = AgentRunner::new(pool);
        
        // Register the mock agent factory
        // The factory closure captures `mock_agent_instance` by value.
        // This is tricky because the factory might be called multiple times if we had multiple configs.
        // For a single agent, this is okay. For multiple, the factory would need to create new mocks.
        // For this test, we only have one config, so one call to factory.
        agent_runner.agent_registry.register(
            agent_kind_str.to_string(),
            Box::new(move |_cfg_arc, _event_pub_arc| {
                // This is where the pre-configured mock_agent_instance is moved.
                // If testing multiple agents of the same kind, this factory would need to be smarter
                // or we'd need separate test cases / more complex mock management.
                // For this specific test with one agent, it's fine.
                Ok(Box::new(mock_agent_instance))
            })
        ).unwrap();

        let result = agent_runner.instantiate_agents_from_configs(configs).await;

        assert!(result.is_ok());
        let agents_map = agent_runner.agents.lock().unwrap();
        assert!(agents_map.contains_key(&agent_id));
        assert_eq!(agents_map.len(), 1);
        assert_eq!(agents_map.get(&agent_id).unwrap().lock().unwrap().status(), AgentStatus::Running);

        // Mock expectations are automatically verified when `mock_agent_instance` goes out of scope.
    }

    #[tokio::test]
    async fn test_instantiate_agents_init_fails() {
        let agent_id = AgentId::new_v4();
        let profile_id = ProfileId::new_v4();
        let agent_kind_str = "failing_init_agent";

        let config = AgentConfig {
            id: agent_id,
            profile_id,
            name: Some("Failing Init Agent".to_string()),
            agent_kind: agent_kind_str.to_string(),
            config_data: json!({}),
            credential_ids: vec![],
            enabled: true,
        };
        let configs = vec![config.clone()];

        let mut mock_agent_instance = MockSdkAgent::new();
        mock_agent_instance.expect_id().return_const(agent_id);
        mock_agent_instance.expect_kind().return_const(agent_kind_str);
        mock_agent_instance.expect_init()
            .times(1)
            .returning(|_ctx| Err(SdkError::InitializationError("init failed".to_string())));
        mock_agent_instance.expect_start().times(0); // Start should not be called

        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://mockuser:mockpass@localhost/mockdb")
            .expect("Failed to create lazy PgPool");
        let mut agent_runner = AgentRunner::new(pool);
        
        agent_runner.agent_registry.register(
            agent_kind_str.to_string(),
            Box::new(move |_cfg, _pub| Ok(Box::new(mock_agent_instance)))
        ).unwrap();

        let result = agent_runner.instantiate_agents_from_configs(configs).await;

        assert!(result.is_ok()); // The method itself doesn't error out, just logs and skips.
        let agents_map = agent_runner.agents.lock().unwrap();
        // Agent might be added with Errored status, or not added, depending on chosen strategy.
        // Current code does not add it if init/start fails.
        assert!(agents_map.is_empty());
    }

    #[tokio::test]
    async fn test_instantiate_agents_start_fails() {
        let agent_id = AgentId::new_v4();
        let profile_id = ProfileId::new_v4();
        let agent_kind_str = "failing_start_agent";

        let config = AgentConfig {
            id: agent_id,
            profile_id,
            name: Some("Failing Start Agent".to_string()),
            agent_kind: agent_kind_str.to_string(),
            config_data: json!({}),
            credential_ids: vec![],
            enabled: true,
        };
        let configs = vec![config.clone()];

        let mut mock_agent_instance = MockSdkAgent::new();
        mock_agent_instance.expect_id().return_const(agent_id);
        mock_agent_instance.expect_kind().return_const(agent_kind_str);
        mock_agent_instance.expect_init().times(1).returning(|_ctx| Ok(()));
        mock_agent_instance.expect_start()
            .times(1)
            .returning(|| Err(SdkError::RuntimeError("start failed".to_string())));

        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://mockuser:mockpass@localhost/mockdb")
            .expect("Failed to create lazy PgPool");
        let mut agent_runner = AgentRunner::new(pool);

        agent_runner.agent_registry.register(
            agent_kind_str.to_string(),
            Box::new(move |_cfg, _pub| Ok(Box::new(mock_agent_instance)))
        ).unwrap();

        let result = agent_runner.instantiate_agents_from_configs(configs).await;

        assert!(result.is_ok()); // Method completes, logs error.
        let agents_map = agent_runner.agents.lock().unwrap();
        // Agent might be added with Errored status, or not added.
        // Current code does not add it if init/start fails.
        assert!(agents_map.is_empty());
    }

    #[tokio::test]
    async fn test_instantiate_agents_unknown_kind() {
        let agent_id = AgentId::new_v4();
        let profile_id = ProfileId::new_v4();

        let config = AgentConfig {
            id: agent_id,
            profile_id,
            name: Some("Unknown Kind Agent".to_string()),
            agent_kind: "unknown_kind_agent".to_string(), // This kind is not registered
            config_data: json!({}),
            credential_ids: vec![],
            enabled: true,
        };
        let configs = vec![config.clone()];

        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://mockuser:mockpass@localhost/mockdb")
            .expect("Failed to create lazy PgPool");
        let mut agent_runner = AgentRunner::new(pool);
        // No agent registered for "unknown_kind_agent"

        let result = agent_runner.instantiate_agents_from_configs(configs).await;

        assert!(result.is_ok()); // Method completes, logs error.
        assert!(agent_runner.agents.lock().unwrap().is_empty()); // Agent should not be created or added
    }

    // --- Tests for stop_all_agents ---
    // Helper to create a simple AgentRunner for these tests
    fn create_agent_runner_for_stop_tests() -> AgentRunner { // Keep this if other tests specifically rely on its minimal setup
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://mockuser:mockpass@localhost/mockdb")
            .expect("Failed to create lazy PgPool");
        let agent_registry = Arc::new(AgentRegistry::new()); // Basic registry
        
        // Mock EventPublisher
        mockall::mock! {
            MockEventPublisher {}
            #[async_trait]
            impl EventPublisher for MockEventPublisher {
                async fn publish_event(&self, _agent_id: &AgentId, _data: serde_json::Value, _event_type_override: Option<String>) -> Result<EventId, SdkError>;
                async fn publish_raw_event(&self, _event: &Event) -> Result<(), SdkError>;
            }
        }
        let event_publisher = Arc::new(MockEventPublisher::new());

        AgentRunner::new(pool, agent_registry, event_publisher, None)
    }
    
    // Helper to create a mock ManagedAgent
    fn create_mock_managed_agent(agent_id: AgentId, initial_status: AgentStatus, expect_stop_result: Result<(), SdkError>) -> Arc<Mutex<ManagedAgent>> {
        let mut mock_sdk_agent = MockSdkAgent::new();
        mock_sdk_agent.expect_id().return_const(agent_id.clone());
        mock_sdk_agent.expect_kind().return_const("mock_kind");
        mock_sdk_agent.expect_stop().times(1).returning(move || expect_stop_result.clone());
        // No init/start expectations needed for stop tests if we manually set status

        let (stop_tx, _stop_rx) = oneshot::channel();
        let mut managed_agent = ManagedAgent::new(agent_id, Box::new(mock_sdk_agent), Some(stop_tx));
        managed_agent.set_status(initial_status);
        Arc::new(Mutex::new(managed_agent))
    }


#[tokio::test]
    async fn test_stop_all_agents_success() {
        let agent_id1 = AgentId::new_v4();
        let agent_kind1 = "stoppable_agent_1";
        let mut mock_agent1 = MockSdkAgent::new();
        mock_agent1.expect_stop().times(1).returning(|| Ok(()));

        let agent_id2 = AgentId::new_v4();
        let agent_kind2 = "stoppable_agent_2";
        let mut mock_agent2 = MockSdkAgent::new();
        mock_agent2.expect_stop().times(1).returning(|| Ok(()));
        
        let mut agent_runner = create_agent_runner_for_stop_tests();
        
        let managed_agent1 = create_mock_managed_agent(agent_id1.clone(), AgentStatus::Running, Ok(()));
        let managed_agent2 = create_mock_managed_agent(agent_id2.clone(), AgentStatus::Running, Ok(()));

        {
            let mut agents_map = agent_runner.agents.lock().unwrap();
            agents_map.insert(agent_id1.clone(), managed_agent1.clone());
            agents_map.insert(agent_id2.clone(), managed_agent2.clone());
            assert_eq!(agents_map.len(), 2);
        }
        
        agent_runner.stop_all_agents().await;

        assert!(agent_runner.agents.lock().unwrap().is_empty()); // Agents map should be cleared
        assert_eq!(managed_agent1.lock().unwrap().status(), AgentStatus::Stopped);
        assert_eq!(managed_agent2.lock().unwrap().status(), AgentStatus::Stopped);
    }

    #[tokio::test]
    async fn test_stop_all_agents_one_fails() {
        let agent_id1 = AgentId::new_v4();
        let mut mock_agent1 = MockSdkAgent::new();
        mock_agent1.expect_stop().times(1).returning(|| Ok(())); // Succeeds

        let agent_id2 = AgentId::new_v4();
        let mut mock_agent2 = MockSdkAgent::new();
        mock_agent2.expect_stop()
            .times(1)
            .returning(|| Err(SdkError::RuntimeError("stop failed".to_string()))); // Fails
        
        let mut agent_runner = create_agent_runner_for_stop_tests();

        let managed_agent1 = create_mock_managed_agent(agent_id1.clone(), AgentStatus::Running, Ok(()));
        let managed_agent2 = create_mock_managed_agent(agent_id2.clone(), AgentStatus::Running, Err(SdkError::RuntimeError("stop failed".to_string())));
        
        {
            let mut agents_map = agent_runner.agents.lock().unwrap();
            agents_map.insert(agent_id1.clone(), managed_agent1.clone());
            agents_map.insert(agent_id2.clone(), managed_agent2.clone());
            assert_eq!(agents_map.len(), 2);
        }

        agent_runner.stop_all_agents().await;

        assert!(agent_runner.agents.lock().unwrap().is_empty());
        assert_eq!(managed_agent1.lock().unwrap().status(), AgentStatus::Stopped);
        assert_eq!(managed_agent2.lock().unwrap().status(), AgentStatus::Errored);
    }

    #[tokio::test]
    async fn test_stop_all_agents_no_agents() {
        let mut agent_runner = create_agent_runner_for_stop_tests();
    // Helper function to create an AgentRunner with mocked dependencies for lifecycle tests
    fn create_agent_runner_for_lifecycle_tests(
        db_mock_builder: Option<Box<dyn FnOnce() -> mock_db_mod::MockPostgresStateStore + Send + 'static>>,
        agent_registry: Option<Arc<AgentRegistry>> // Allow passing a pre-configured registry
    ) -> AgentRunner {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://mockuser:mockpass@localhost/mockdb")
            .expect("Failed to create lazy PgPool");

        let final_agent_registry = agent_registry.unwrap_or_else(|| Arc::new(AgentRegistry::new()));
        
        mockall::mock! {
            MockEventPublisher {}
            #[async_trait]
            impl EventPublisher for MockEventPublisher {
                async fn publish_event(&self, _agent_id: &AgentId, _data: serde_json::Value, _event_type_override: Option<String>) -> Result<EventId, SdkError>;
                async fn publish_raw_event(&self, _event: &Event) -> Result<(), SdkError>;
            }
        }
        let event_publisher = Arc::new(MockEventPublisher::new());

        if let Some(builder) = db_mock_builder {
            let new_pg_store_context = mock_db_mod::MockPostgresStateStore::new_context();
            new_pg_store_context.expect()
                .times(1) // Expect `new` to be called once by start_agent
                .returning(move |_pool| builder());
        }
        // If no db_mock_builder, PostgresStateStore::new will be called with the real pool,
        // which is fine if the test doesn't interact with the DB through AgentRunner directly.

        AgentRunner::new(pool, final_agent_registry, event_publisher, None)
    }

    #[tokio::test]
    async fn test_start_agent_success() {
        let agent_id = AgentId::new_v4();
        let profile_id = ProfileId::new_v4();
        let recipe_id = RecipeId::new_v4();
        let agent_kind_str = "lifecycle_test_agent";

        let agent_config = AgentConfig {
            id: agent_id.clone(),
            profile_id,
            name: Some("Lifecycle Test Agent".to_string()),
            agent_kind: agent_kind_str.to_string(),
            config_data: json!({}),
            credential_ids: vec![],
            enabled: true,
        };
        let recipe = Recipe { // Dummy recipe
            id: recipe_id, profile_id, name: "Test Recipe".to_string(), description: None,
            graph_definition: helix_core::recipe::JsonGraph(json!({"agents": [{"agent_id": agent_id.to_string()}]})),
            enabled: true, created_at: chrono::Utc::now(), updated_at: chrono::Utc::now(),
        };
        let profile = Profile { id: profile_id, name: "Test Profile".to_string(), description: None, settings: json!({}) };

        let mut mock_sdk_agent = MockSdkAgent::new();
        mock_sdk_agent.expect_init().times(1).returning(|_| Ok(()));
        mock_sdk_agent.expect_start().times(1).returning(|| Ok(()));
        mock_sdk_agent.expect_id().return_const(agent_id.clone()); // For ManagedAgent::new

        let agent_registry = Arc::new(AgentRegistry::new());
        let factory_agent_id = agent_id.clone();
        agent_registry.register(
            agent_kind_str.to_string(),
            Box::new(move |_cfg_arc, _event_pub_arc| {
                let mut sdk_agent = MockSdkAgent::new();
                sdk_agent.expect_init().times(1).returning(|_| Ok(()));
                sdk_agent.expect_start().times(1).returning(|| Ok(()));
                sdk_agent.expect_id().return_const(factory_agent_id.clone());
                Ok(Box::new(sdk_agent))
            })
        ).unwrap();
        
        let agent_config_clone = agent_config.clone();
        let db_mock_builder = Box::new(move || {
            let mut mock_store = mock_db_mod::MockPostgresStateStore::default();
            mock_store.expect_get_agent_config()
                .with(eq(agent_id.clone()))
                .times(1)
                .returning(move |_| Ok(Some(agent_config_clone.clone())));
            mock_store
        });

        let mut agent_runner = create_agent_runner_for_lifecycle_tests(Some(db_mock_builder), Some(agent_registry.clone()));
        
        let result = agent_runner.start_agent(&recipe, &profile, &agent_id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), agent_id);

        let status = agent_runner.get_agent_status(&agent_id);
        assert_eq!(status, Some(AgentStatus::Running));

        let agents_map = agent_runner.agents.lock().unwrap();
        assert!(agents_map.contains_key(&agent_id));
        let managed_agent = agents_map.get(&agent_id).unwrap().lock().unwrap();
        assert_eq!(managed_agent.status(), AgentStatus::Running);
    }

    #[tokio::test]
    async fn test_start_agent_config_not_found() {
        let agent_id = AgentId::new_v4();
        let profile_id = ProfileId::new_v4();
        let recipe_id = RecipeId::new_v4();
         let recipe = Recipe {
            id: recipe_id, profile_id, name: "Test Recipe".to_string(), description: None,
            graph_definition: helix_core::recipe::JsonGraph(json!({"agents": [{"agent_id": agent_id.to_string()}]})),
            enabled: true, created_at: chrono::Utc::now(), updated_at: chrono::Utc::now(),
        };
        let profile = Profile { id: profile_id, name: "Test Profile".to_string(), description: None, settings: json!({}) };

        let db_mock_builder = Box::new(move || {
            let mut mock_store = mock_db_mod::MockPostgresStateStore::default();
            mock_store.expect_get_agent_config()
                .with(eq(agent_id.clone()))
                .times(1)
                .returning(|_| Ok(None)); // Config not found
            mock_store
        });
        let mut agent_runner = create_agent_runner_for_lifecycle_tests(Some(db_mock_builder), None);

        let result = agent_runner.start_agent(&recipe, &profile, &agent_id).await;
        assert!(result.is_err());
        match result.err().unwrap() {
            HelixError::AgentConfigNotFoundError(id) => assert_eq!(id, agent_id),
            _ => panic!("Expected AgentConfigNotFoundError"),
        }
        assert!(agent_runner.agents.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_start_agent_init_fails() {
        let agent_id = AgentId::new_v4();
        let profile_id = ProfileId::new_v4();
        let recipe_id = RecipeId::new_v4();
        let agent_kind_str = "init_fail_agent";

        let agent_config = AgentConfig {
            id: agent_id.clone(), profile_id, name: Some("Init Fail".to_string()), agent_kind: agent_kind_str.to_string(),
            config_data: json!({}), credential_ids: vec![], enabled: true,
        };
        let recipe = Recipe {
            id: recipe_id, profile_id, name: "Test Recipe".to_string(), description: None,
            graph_definition: helix_core::recipe::JsonGraph(json!({"agents": [{"agent_id": agent_id.to_string()}]})),
            enabled: true, created_at: chrono::Utc::now(), updated_at: chrono::Utc::now(),
        };
        let profile = Profile { id: profile_id, name: "Test Profile".to_string(), description: None, settings: json!({}) };

        let agent_registry = Arc::new(AgentRegistry::new());
        let factory_agent_id = agent_id.clone();
        agent_registry.register(
            agent_kind_str.to_string(),
            Box::new(move |_cfg_arc, _event_pub_arc| {
                let mut sdk_agent = MockSdkAgent::new();
                sdk_agent.expect_init().times(1).returning(|_| Err(SdkError::InitializationError("init failed".into())));
                sdk_agent.expect_start().times(0); // Should not be called
                sdk_agent.expect_id().return_const(factory_agent_id.clone());
                Ok(Box::new(sdk_agent))
            })
        ).unwrap();

        let agent_config_clone = agent_config.clone();
        let db_mock_builder = Box::new(move || {
            let mut mock_store = mock_db_mod::MockPostgresStateStore::default();
            mock_store.expect_get_agent_config().returning(move |_| Ok(Some(agent_config_clone.clone())));
            mock_store
        });
        let mut agent_runner = create_agent_runner_for_lifecycle_tests(Some(db_mock_builder), Some(agent_registry.clone()));

        let result = agent_runner.start_agent(&recipe, &profile, &agent_id).await;
        assert!(result.is_err());
        match result.err().unwrap() {
            HelixError::AgentError(msg) => assert!(msg.contains("Initialization failed")),
            _ => panic!("Expected AgentError due to init failure"),
        }
        // AgentRunner currently doesn't store errored agents from start_agent, so map should be empty
        assert!(agent_runner.agents.lock().unwrap().is_empty());
        assert_eq!(agent_runner.get_agent_status(&agent_id), None);
    }


    #[tokio::test]
    async fn test_stop_agent_success() {
        let agent_id = AgentId::new_v4();
        let profile_id = ProfileId::new_v4();
        let recipe_id = RecipeId::new_v4();
        let agent_kind_str = "stoppable_agent";

        let agent_config = AgentConfig {
            id: agent_id.clone(), profile_id, name: Some("Stoppable".to_string()), agent_kind: agent_kind_str.to_string(),
            config_data: json!({}), credential_ids: vec![], enabled: true,
        };
         let recipe = Recipe {
            id: recipe_id, profile_id, name: "Test Recipe".to_string(), description: None,
            graph_definition: helix_core::recipe::JsonGraph(json!({"agents": [{"agent_id": agent_id.to_string()}]})),
            enabled: true, created_at: chrono::Utc::now(), updated_at: chrono::Utc::now(),
        };
        let profile = Profile { id: profile_id, name: "Test Profile".to_string(), description: None, settings: json!({}) };

        let agent_registry = Arc::new(AgentRegistry::new());
        let factory_agent_id = agent_id.clone();
        agent_registry.register(
            agent_kind_str.to_string(),
            Box::new(move |_cfg_arc, _event_pub_arc| {
                let mut sdk_agent = MockSdkAgent::new();
                sdk_agent.expect_init().times(1).returning(|_| Ok(()));
                sdk_agent.expect_start().times(1).returning(|| Ok(()));
                sdk_agent.expect_stop().times(1).returning(|| Ok(())); // Expect stop to be called
                sdk_agent.expect_id().return_const(factory_agent_id.clone());
                Ok(Box::new(sdk_agent))
            })
        ).unwrap();
        
        let agent_config_clone = agent_config.clone();
        let db_mock_builder = Box::new(move || {
            let mut mock_store = mock_db_mod::MockPostgresStateStore::default();
            mock_store.expect_get_agent_config().returning(move |_| Ok(Some(agent_config_clone.clone())));
            mock_store
        });
        let mut agent_runner = create_agent_runner_for_lifecycle_tests(Some(db_mock_builder), Some(agent_registry.clone()));

        // Start the agent
        agent_runner.start_agent(&recipe, &profile, &agent_id).await.unwrap();
        assert_eq!(agent_runner.get_agent_status(&agent_id), Some(AgentStatus::Running));

        // Stop the agent
        let stop_result = agent_runner.stop_agent(&agent_id).await;
        assert!(stop_result.is_ok());
        assert_eq!(agent_runner.get_agent_status(&agent_id), Some(AgentStatus::Stopped));
        
        // Verify it's still in the map but with status Stopped
        let agents_map = agent_runner.agents.lock().unwrap();
        assert!(agents_map.contains_key(&agent_id));
        assert_eq!(agents_map.get(&agent_id).unwrap().lock().unwrap().status(), AgentStatus::Stopped);
    }

    #[tokio::test]
    async fn test_stop_agent_not_found() {
        let agent_id = AgentId::new_v4();
        let mut agent_runner = create_agent_runner_for_lifecycle_tests(None, None);

        let result = agent_runner.stop_agent(&agent_id).await;
        assert!(result.is_err());
        match result.err().unwrap() {
            HelixError::AgentNotFoundError(id) => assert_eq!(id, agent_id),
            _ => panic!("Expected AgentNotFoundError"),
        }
    }
    
    #[tokio::test]
    async fn test_get_agent_status_various_states() {
        let agent_id_running = AgentId::new_v4();
        let agent_id_stopped = AgentId::new_v4();
        let agent_id_errored = AgentId::new_v4();
        let agent_id_unknown = AgentId::new_v4();

        let mut agent_runner = create_agent_runner_for_lifecycle_tests(None, None);

        // Manually insert agents with different statuses for this test
        let (stop_tx_running, _) = oneshot::channel();
        let running_agent_instance = MockSdkAgent::new();
        let managed_running = ManagedAgent::new(agent_id_running.clone(), Box::new(running_agent_instance), Some(stop_tx_running));
        managed_running.status = AgentStatus::Running; // Manually set status

        let (stop_tx_stopped, _) = oneshot::channel();
        let stopped_agent_instance = MockSdkAgent::new();
        let mut managed_stopped = ManagedAgent::new(agent_id_stopped.clone(), Box::new(stopped_agent_instance), Some(stop_tx_stopped));
        managed_stopped.set_status(AgentStatus::Stopped);

        let (stop_tx_errored, _) = oneshot::channel();
        let errored_agent_instance = MockSdkAgent::new();
        let mut managed_errored = ManagedAgent::new(agent_id_errored.clone(), Box::new(errored_agent_instance), Some(stop_tx_errored));
        managed_errored.set_status(AgentStatus::Errored);

        {
            let mut agents_map = agent_runner.agents.lock().unwrap();
            agents_map.insert(agent_id_running.clone(), Arc::new(Mutex::new(managed_running)));
            agents_map.insert(agent_id_stopped.clone(), Arc::new(Mutex::new(managed_stopped)));
            agents_map.insert(agent_id_errored.clone(), Arc::new(Mutex::new(managed_errored)));
        }

        assert_eq!(agent_runner.get_agent_status(&agent_id_running), Some(AgentStatus::Running));
        assert_eq!(agent_runner.get_agent_status(&agent_id_stopped), Some(AgentStatus::Stopped));
        assert_eq!(agent_runner.get_agent_status(&agent_id_errored), Some(AgentStatus::Errored));
        assert_eq!(agent_runner.get_agent_status(&agent_id_unknown), None);
    }


    #[tokio::test]
    async fn test_run_recipe_success() { // Existing test, may need minor adjustments if ManagedAgent impacts it
        let profile_id = ProfileId::new_v4();
        let recipe_id = RecipeId::new_v4();
        let source_agent_id = AgentId::new_v4();
        let action_agent_id = AgentId::new_v4();

        // 1. Mock AgentConfigs
        let source_agent_config = AgentConfig {
            id: source_agent_id,
            profile_id,
            name: Some("Mock Source Agent".to_string()),
            agent_kind: "mock_source_kind".to_string(),
            config_data: json!({}),
            credential_ids: vec![],
            enabled: true,
        };
        let source_agent_config_arc = Arc::new(source_agent_config.clone());

        let action_agent_config = AgentConfig {
            id: action_agent_id,
            profile_id,
            name: Some("Mock Action Agent".to_string()),
            agent_kind: "mock_action_kind".to_string(),
            config_data: json!({}),
            credential_ids: vec![],
            enabled: true,
        };
        let action_agent_config_arc = Arc::new(action_agent_config.clone());

        // 2. Mock Recipe
        let recipe_graph_def = RecipeGraphDefinition {
            agents: vec![
                RecipeNode { agent_id: source_agent_id, depends_on: vec![] },
                RecipeNode { agent_id: action_agent_id, depends_on: vec![source_agent_id.to_string()] }, // Simplified dependency
            ],
            // other fields if necessary
        };
        let mock_recipe = Recipe {
            id: recipe_id,
            profile_id,
            name: "Test Recipe".to_string(),
            description: None,
            graph_definition: helix_core::recipe::JsonGraph(serde_json::to_value(recipe_graph_def).unwrap()),
            enabled: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        // 3. Mock PostgresStateStore instance
        let mut mock_pg_store_instance = mock_db_mod::MockPostgresStateStore::default();
        let recipe_clone = mock_recipe.clone();
        mock_pg_store_instance.expect_get_recipe()
            .with(eq(recipe_id))
            .times(1)
            .returning(move |_| Ok(Some(recipe_clone.clone())));
        
        let source_config_clone_for_db = source_agent_config.clone();
        mock_pg_store_instance.expect_get_agent_config()
            .with(eq(source_agent_id))
            .times(1) // Called once for source
            .returning(move |_| Ok(Some(source_config_clone_for_db.clone())));

        let action_config_clone_for_db = action_agent_config.clone();
        mock_pg_store_instance.expect_get_agent_config()
            .with(eq(action_agent_id))
            .times(1) // Called once for action
            .returning(move |_| Ok(Some(action_config_clone_for_db.clone())));
        
        // Mock the `PostgresStateStore::new` call
        let new_pg_store_context = mock_db_mod::MockPostgresStateStore::new_context();
        new_pg_store_context.expect()
            .times(1) // `new` is called once at the start of `run_recipe`
            .returning(move |_pool| mock_pg_store_instance);


        // 4. Create AgentRunner with mocked DB
        let mut agent_runner = create_agent_runner_for_recipe_tests(move || mock_pg_store_instance);

        // 5. Create and configure Mock Source Agent (as ManagedAgent)
        let mut mock_source_agent = MockSdkAgent::new();
        mock_source_agent.expect_id().return_const(source_agent_id);
        mock_source_agent.expect_kind().return_const("mock_source_kind");
        mock_source_agent.expect_as_source_mut().returning(|s| Some(s)); // Enable downcast
        
        let produced_event_data = json!({"source_data": "event_payload"});
        let produced_event_data_clone = produced_event_data.clone();
        let source_agent_id_clone = source_agent_id.clone();

        mock_source_agent.expect_run()
            .times(1)
            .returning(move |ctx| {
                let event_payload = produced_event_data_clone.clone();
                let agent_id_for_event = source_agent_id_clone.clone(); // Use the agent's own ID
                
                // The source agent uses the event publisher from its context to emit an event.
                // This context is created by `run_recipe` with an `InMemoryEventCollector`.
                let fut = async move {
                    ctx.event_publisher.publish_event(
                        &agent_id_for_event, // AgentId of the publisher
                        event_payload,      // Actual data
                        Some("mock_source_event_type".to_string()) // Optional event type override
                    ).await?;
                    Ok(())
                };
                Box::pin(fut) // Pin the future
            });
        
        // 6. Create and configure Mock Action Agent
        let mut mock_action_agent = MockSdkAgent::new();
        mock_action_agent.expect_id().return_const(action_agent_id);
        mock_action_agent.expect_kind().return_const("mock_action_kind");
        mock_action_agent.expect_as_action_mut().returning(|s| Some(s)); // Enable downcast

        let expected_event_data_for_action = produced_event_data.clone();
        mock_action_agent.expect_execute_event()
            .times(1)
            .withf(move |_ctx, event| { // Use withf for custom matching logic on the event
                event.agent_id == source_agent_id && // Event originated from source
                event.data == expected_event_data_for_action &&
                event.event_type == "mock_source_event_type"
            })
            .returning(|_ctx, _event| Box::pin(async { Ok(()) }));


        // 7. Manually insert mock ManagedAgents into AgentRunner
        let (s_stop_tx, _) = oneshot::channel();
        let mut managed_source = ManagedAgent::new(source_agent_id.clone(), Box::new(mock_source_agent), Some(s_stop_tx));
        managed_source.set_status(AgentStatus::Running); // Assume running for recipe test

        let (a_stop_tx, _) = oneshot::channel();
        let mut managed_action = ManagedAgent::new(action_agent_id.clone(), Box::new(mock_action_agent), Some(a_stop_tx));
        managed_action.set_status(AgentStatus::Running); // Assume running

        {
            let mut agents_map = agent_runner.agents.lock().unwrap();
            agents_map.insert(source_agent_id, Arc::new(Mutex::new(managed_source)));
            agents_map.insert(action_agent_id, Arc::new(Mutex::new(managed_action)));
        }
        
        // 8. Call run_recipe
        let result = agent_runner.run_recipe(&recipe_id, None).await;

        // 9. Assertions
        assert!(result.is_ok(), "run_recipe failed: {:?}", result.err());
        
        // Mock expectations are verified when the mocks go out of scope.
        // new_pg_store_context expectation is also verified.
    }

    // TODO: Add more tests for run_recipe:
    // - Recipe not found
    // - Recipe disabled
    // - Recipe graph deserialization error
    // - Source agent not found in runner.agents
    // - Action agent not found in runner.agents
    // - Source agent fails to downcast (returns None from as_source_mut)
    // - Action agent fails to downcast
    // - Source agent's run method returns an error
    // - Action agent's execute_event method returns an error
    // - Source agent produces no events (should still be Ok, but action agent not called)
        // The rest of the test_run_recipe_success would need to be adapted if agent status changes
        // are expected during recipe execution (e.g. to Completed or Errored).
        // The current `run_recipe` updates status on error.
    }

