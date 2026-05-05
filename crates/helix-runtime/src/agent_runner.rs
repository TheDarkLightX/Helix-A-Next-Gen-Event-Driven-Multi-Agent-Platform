// Copyright 2026 DarkLightX
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

//! Minimal agent lifecycle runner (imperative shell).

use crate::agent_registry::AgentRegistry;
use crate::AgentStatus;
use helix_agent_sdk::{AgentContext, EventPublisher, SdkAgent, SdkError};
use helix_core::agent::{AgentConfig, AgentRuntime};
use helix_core::credential::CredentialProvider;
use helix_core::recipe::Recipe;
use helix_core::state::StateStore;
use helix_core::types::AgentId;
use helix_core::HelixError;
use helix_wasm::plugins::{ModuleSource, PluginConfig, PluginId, PluginManager};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Managed agent instance.
pub enum ManagedAgent {
    /// Native Rust agent implementing [`SdkAgent`].
    Native {
        /// Current status.
        status: AgentStatus,
        /// Agent instance.
        agent: Box<dyn SdkAgent>,
    },
    /// WASM agent tracked by plugin ID.
    Wasm {
        /// Current status.
        status: AgentStatus,
        /// Plugin ID in the [`PluginManager`].
        plugin_id: PluginId,
    },
}

impl ManagedAgent {
    fn status(&self) -> AgentStatus {
        match self {
            ManagedAgent::Native { status, .. } => *status,
            ManagedAgent::Wasm { status, .. } => *status,
        }
    }
}

/// Imperative shell that owns agent instances and drives their lifecycle.
pub struct AgentRunner {
    registry: Arc<AgentRegistry>,
    event_publisher: Arc<dyn EventPublisher>,
    credential_provider: Arc<dyn CredentialProvider>,
    state_store: Arc<dyn StateStore>,
    plugin_manager: Option<Arc<Mutex<PluginManager>>>,
    agents: HashMap<AgentId, ManagedAgent>,
}

impl AgentRunner {
    /// Creates a runner for native agents. WASM execution is disabled.
    pub fn new_native(
        registry: Arc<AgentRegistry>,
        event_publisher: Arc<dyn EventPublisher>,
        credential_provider: Arc<dyn CredentialProvider>,
        state_store: Arc<dyn StateStore>,
    ) -> Self {
        Self {
            registry,
            event_publisher,
            credential_provider,
            state_store,
            plugin_manager: None,
            agents: HashMap::new(),
        }
    }

    /// Creates a runner with WASM plugin execution support.
    pub fn new_with_wasm(
        registry: Arc<AgentRegistry>,
        event_publisher: Arc<dyn EventPublisher>,
        credential_provider: Arc<dyn CredentialProvider>,
        state_store: Arc<dyn StateStore>,
        plugin_manager: Arc<Mutex<PluginManager>>,
    ) -> Self {
        Self {
            registry,
            event_publisher,
            credential_provider,
            state_store,
            plugin_manager: Some(plugin_manager),
            agents: HashMap::new(),
        }
    }

    /// Returns the number of managed agents.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// Returns the status of a managed agent.
    pub fn agent_status(&self, agent_id: &AgentId) -> Option<AgentStatus> {
        self.agents.get(agent_id).map(|a| a.status())
    }

    /// Starts all agents in an enabled recipe using deterministic dependency order.
    pub async fn run_recipe(&mut self, recipe: &Recipe) -> Result<Vec<AgentId>, HelixError> {
        if !recipe.enabled {
            return Err(HelixError::validation_error(
                "Recipe.enabled",
                "recipe is disabled",
            ));
        }

        let ordered_agents = recipe.execution_order()?;
        if let Some(disabled_agent) = ordered_agents.iter().find(|agent| !agent.enabled) {
            return Err(HelixError::validation_error(
                format!("Recipe.graph.agents[id={}].enabled", disabled_agent.id),
                "recipe agent is disabled".to_string(),
            ));
        }

        let mut started = Vec::with_capacity(ordered_agents.len());
        for agent_config in ordered_agents {
            started.push(self.start_agent(agent_config).await?);
        }
        Ok(started)
    }

    /// Starts an agent from its configuration.
    ///
    /// Fail-closed: invalid configs, unknown kinds, or missing WASM support cause errors.
    pub async fn start_agent(&mut self, config: AgentConfig) -> Result<AgentId, HelixError> {
        config.validate()?;

        if self.agents.contains_key(&config.id) {
            return Ok(config.id);
        }

        let id = config.id;
        match config.agent_runtime {
            AgentRuntime::Native => {
                let cfg = Arc::new(config);
                let mut agent = self
                    .registry
                    .create_agent(Arc::clone(&cfg))
                    .map_err(map_sdk_error)?;

                let event_publisher: Arc<dyn EventPublisher> = Arc::clone(&self.event_publisher);
                let credential_provider: Arc<dyn CredentialProvider> =
                    Arc::clone(&self.credential_provider);
                let state_store: Arc<dyn StateStore> = Arc::clone(&self.state_store);

                let ctx = AgentContext::new(cfg, event_publisher, credential_provider, state_store);

                agent.init(&ctx).await.map_err(map_sdk_error)?;
                agent.start(&ctx).await.map_err(map_sdk_error)?;

                self.agents.insert(
                    id,
                    ManagedAgent::Native {
                        status: AgentStatus::Running,
                        agent,
                    },
                );
                Ok(id)
            }
            AgentRuntime::Wasm => {
                let manager = self.plugin_manager.as_ref().ok_or_else(|| {
                    HelixError::config_error(
                        "WASM agent requested but runtime has no PluginManager",
                    )
                })?;

                let wasm_path = config.wasm_module_path.clone().ok_or_else(|| {
                    HelixError::validation_error(
                        "AgentConfig.wasm_module_path",
                        "missing WASM module path",
                    )
                })?;

                // Use a unique (kind,version) pair to avoid collisions in a long-running process.
                let plugin_name = format!("{}:{}", config.agent_kind, id);
                let plugin_version = "local".to_string();
                let plugin_desc = "helix wasm agent".to_string();
                let module_source = ModuleSource::File(PathBuf::from(wasm_path));

                let plugin_id = {
                    let mut mgr = manager.lock().await;
                    let plugin_cfg =
                        PluginConfig::new(plugin_name, plugin_version, plugin_desc, module_source);
                    mgr.register_plugin(plugin_cfg)
                        .await
                        .map_err(map_wasm_error)?
                };

                {
                    let mgr = manager.lock().await;
                    let event_publisher: Arc<dyn EventPublisher> =
                        Arc::clone(&self.event_publisher);
                    let credential_provider: Arc<dyn CredentialProvider> =
                        Arc::clone(&self.credential_provider);
                    let state_store: Arc<dyn StateStore> = Arc::clone(&self.state_store);

                    let _instance_id = mgr
                        .instantiate_plugin(
                            plugin_id,
                            Arc::new(config),
                            event_publisher,
                            credential_provider,
                            state_store,
                        )
                        .await
                        .map_err(map_wasm_error)?;
                }

                self.agents.insert(
                    id,
                    ManagedAgent::Wasm {
                        status: AgentStatus::Running,
                        plugin_id,
                    },
                );

                Ok(id)
            }
        }
    }

    /// Stops an agent and removes it from the runner.
    pub async fn stop_agent(&mut self, agent_id: &AgentId) -> Result<(), HelixError> {
        let managed = self
            .agents
            .remove(agent_id)
            .ok_or_else(|| HelixError::not_found(format!("agent {}", agent_id)))?;

        match managed {
            ManagedAgent::Native { mut agent, .. } => {
                let cfg = Arc::new(agent.config().clone());
                let event_publisher: Arc<dyn EventPublisher> = Arc::clone(&self.event_publisher);
                let credential_provider: Arc<dyn CredentialProvider> =
                    Arc::clone(&self.credential_provider);
                let state_store: Arc<dyn StateStore> = Arc::clone(&self.state_store);
                let ctx = AgentContext::new(cfg, event_publisher, credential_provider, state_store);
                agent.stop(&ctx).await.map_err(map_sdk_error)?;
                Ok(())
            }
            ManagedAgent::Wasm { plugin_id, .. } => {
                let manager = self.plugin_manager.as_ref().ok_or_else(|| {
                    HelixError::config_error(
                        "WASM agent stop requested but runtime has no PluginManager",
                    )
                })?;
                let mut mgr = manager.lock().await;
                mgr.terminate_plugin_instance(plugin_id)
                    .await
                    .map_err(map_wasm_error)?;
                mgr.unregister_plugin(plugin_id)
                    .await
                    .map_err(map_wasm_error)?;
                Ok(())
            }
        }
    }
}

fn map_sdk_error(err: SdkError) -> HelixError {
    HelixError::agent_error(format!("sdk_error: {}", err))
}

fn map_wasm_error(err: helix_wasm::WasmError) -> HelixError {
    HelixError::internal_error(format!("wasm_error: {}", err))
}

#[cfg(test)]
mod tests {
    use super::*;
    use helix_core::credential::EnvCredentialProvider;
    use helix_core::recipe::{Recipe, RecipeGraphDefinition};
    use helix_core::state::InMemoryStateStore;
    use serde_json::json;
    use uuid::Uuid;

    struct NoopAgent {
        agent_config: Arc<AgentConfig>,
    }

    #[async_trait::async_trait]
    impl helix_core::agent::Agent for NoopAgent {
        fn id(&self) -> AgentId {
            self.agent_config.id
        }

        fn config(&self) -> &AgentConfig {
            &self.agent_config
        }
    }

    #[async_trait::async_trait]
    impl SdkAgent for NoopAgent {
        async fn init(&mut self, _context: &AgentContext) -> Result<(), SdkError> {
            Ok(())
        }

        async fn start(&mut self, _context: &AgentContext) -> Result<(), SdkError> {
            Ok(())
        }

        async fn stop(&mut self, _context: &AgentContext) -> Result<(), SdkError> {
            Ok(())
        }
    }

    fn registry_with_noop() -> Arc<AgentRegistry> {
        let mut reg = AgentRegistry::new().unwrap_or_else(|_| AgentRegistry::default());
        reg.register(
            "noop",
            Box::new(|config: AgentConfig| {
                Ok(Box::new(NoopAgent {
                    agent_config: Arc::new(config),
                }))
            }),
        )
        .unwrap();
        Arc::new(reg)
    }

    #[tokio::test]
    async fn starts_and_stops_native_agent() {
        let registry = registry_with_noop();
        let publisher: Arc<dyn EventPublisher> =
            Arc::new(crate::messaging::InMemoryEventCollector::new());
        let creds: Arc<dyn CredentialProvider> = Arc::new(EnvCredentialProvider::default());
        let state: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::new());

        let mut runner = AgentRunner::new_native(registry, publisher, creds, state);
        let id = Uuid::new_v4();
        let profile_id = Uuid::new_v4();
        let cfg = AgentConfig::new(id, profile_id, None, "noop".to_string(), json!({}));

        let started = runner.start_agent(cfg).await.unwrap();
        assert_eq!(runner.agent_status(&started), Some(AgentStatus::Running));

        runner.stop_agent(&started).await.unwrap();
        assert_eq!(runner.agent_status(&started), None);
    }

    #[tokio::test]
    async fn run_recipe_starts_agents_in_dependency_order() {
        let registry = registry_with_noop();
        let publisher: Arc<dyn EventPublisher> =
            Arc::new(crate::messaging::InMemoryEventCollector::new());
        let creds: Arc<dyn CredentialProvider> = Arc::new(EnvCredentialProvider::default());
        let state: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::new());
        let mut runner = AgentRunner::new_native(registry, publisher, creds, state);

        let profile_id = Uuid::new_v4();
        let first_id = Uuid::parse_str("20000000-0000-0000-0000-000000000001").unwrap();
        let second_id = Uuid::parse_str("20000000-0000-0000-0000-000000000002").unwrap();

        let first = AgentConfig::new(
            first_id,
            profile_id,
            Some("First".to_string()),
            "noop".to_string(),
            json!({}),
        );
        let mut second = AgentConfig::new(
            second_id,
            profile_id,
            Some("Second".to_string()),
            "noop".to_string(),
            json!({}),
        );
        second.dependencies = vec![first_id];

        let recipe = Recipe::new(
            Uuid::new_v4(),
            profile_id,
            "Runtime Order".to_string(),
            None,
            RecipeGraphDefinition {
                agents: vec![second, first],
            },
        );

        let started = runner.run_recipe(&recipe).await.unwrap();

        assert_eq!(started, vec![first_id, second_id]);
        assert_eq!(runner.agent_count(), 2);
        assert_eq!(runner.agent_status(&first_id), Some(AgentStatus::Running));
        assert_eq!(runner.agent_status(&second_id), Some(AgentStatus::Running));
    }

    #[tokio::test]
    async fn run_recipe_rejects_disabled_agent() {
        let registry = registry_with_noop();
        let publisher: Arc<dyn EventPublisher> =
            Arc::new(crate::messaging::InMemoryEventCollector::new());
        let creds: Arc<dyn CredentialProvider> = Arc::new(EnvCredentialProvider::default());
        let state: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::new());
        let mut runner = AgentRunner::new_native(registry, publisher, creds, state);

        let profile_id = Uuid::new_v4();
        let agent_id = Uuid::parse_str("20000000-0000-0000-0000-000000000003").unwrap();
        let mut agent = AgentConfig::new(
            agent_id,
            profile_id,
            Some("Disabled".to_string()),
            "noop".to_string(),
            json!({}),
        );
        agent.enabled = false;

        let recipe = Recipe::new(
            Uuid::new_v4(),
            profile_id,
            "Disabled Agent".to_string(),
            None,
            RecipeGraphDefinition {
                agents: vec![agent],
            },
        );

        let result = runner.run_recipe(&recipe).await;

        assert!(result.is_err());
        assert_eq!(runner.agent_count(), 0);
    }
}
