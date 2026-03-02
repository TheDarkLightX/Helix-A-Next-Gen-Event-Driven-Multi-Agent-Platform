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


//! Plugin management for WASM modules

use crate::{
    errors::WasmError,
    runtime::{ExecutionResult, InstanceId, WasmModule, WasmRuntime}, // Added InstanceId
};
use helix_core::{agent::AgentConfig, credential::CredentialProvider, state::StateStore};
use helix_agent_sdk::EventPublisher;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::PathBuf, // For ModuleSource::File
    sync::{Arc, Mutex}, // Added Mutex for active_instances in Plugin
};
use uuid::Uuid; // For PluginId
use wasmtime::Val;

/// Unique identifier for a loaded plugin configuration.
/// This is distinct from InstanceId, as one plugin (config) can have multiple instances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginId(Uuid);

impl PluginId {
    pub fn new() -> Self {
        PluginId(Uuid::new_v4())
    }
}
impl std::fmt::Display for PluginId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Plugin ID - can be generated or user-defined for uniqueness
    pub id: PluginId,
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin description
    pub description: String,
    /// WASM module source
    pub module_source: ModuleSource,
    /// Plugin permissions (example, might be more complex)
    pub permissions: Vec<String>,
    // Consider adding metadata like author, repository, etc.
}

impl PluginConfig {
    pub fn new(name: String, version: String, description: String, module_source: ModuleSource) -> Self {
        Self {
            id: PluginId::new(),
            name,
            version,
            description,
            module_source,
            permissions: Vec::new(),
        }
    }
}


/// Source of a WASM module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModuleSource {
    /// File path
    File(PathBuf), // Changed to PathBuf
    /// Raw bytes
    Bytes(Arc<Vec<u8>>), // Changed to Arc<Vec<u8>> for efficient cloning
    /// URL to download from (content will be fetched and stored as Bytes)
    Url(String),
}

/// Represents a loaded and potentially instantiated WASM plugin.
pub struct Plugin {
    pub config: Arc<PluginConfig>,
    pub module: Arc<WasmModule>, // Compiled module, shared
    // A plugin might have multiple instances. For simplicity, let's assume one primary instance for now,
    // or a way to manage multiple. The task implies managing "a" lifecycle.
    // If a plugin is just a "template" and can be instantiated multiple times with different HostState,
    // then instance_id might not belong here directly, or this struct represents a specific instance.
    // Let's assume a Plugin can have one active instance at a time for now.
    active_instance_id: Mutex<Option<InstanceId>>, // InstanceId from WasmRuntime
}

impl Plugin {
    /// Returns the ID of the currently active instance, if any.
    pub fn active_instance_id(&self) -> Option<InstanceId> {
        *self.active_instance_id.lock().unwrap()
    }
}


/// Plugin manager
pub struct PluginManager {
    runtime: Arc<WasmRuntime>,
    plugins: HashMap<PluginId, Arc<Plugin>>, // Store Arc<Plugin> for shared access
}

impl PluginManager {
    pub fn new(runtime: Arc<WasmRuntime>) -> Self {
        Self {
            runtime,
            plugins: HashMap::new(),
        }
    }

    /// Loads a plugin's configuration and compiles its WASM module.
    /// Does not automatically instantiate the plugin.
    pub async fn register_plugin(&mut self, config: PluginConfig) -> Result<PluginId, WasmError> {
        if self.plugins.values().any(|p| p.config.name == config.name && p.config.version == config.version) {
            return Err(WasmError::PluginAlreadyExists(format!("{}-{}", config.name, config.version)));
        }

        let wasm_module = match &config.module_source {
            ModuleSource::File(path) => {
                self.runtime.load_module_from_path(path).await?
            }
            ModuleSource::Bytes(bytes) => {
                self.runtime.load_module_from_bytes(bytes).await?
            }
            ModuleSource::Url(url_str) => {
                // Fetch and then load bytes
                tracing::info!("Fetching WASM module from URL: {}", url_str);
                let response = reqwest::get(url_str).await
                    .map_err(|e| WasmError::LoadingError(format!("Failed to fetch module from URL {}: {}", url_str, e)))?;
                if !response.status().is_success() {
                    return Err(WasmError::LoadingError(format!("Failed to download module from {}: HTTP {}", url_str, response.status())));
                }
                let bytes = response.bytes().await
                    .map_err(|e| WasmError::LoadingError(format!("Failed to read bytes from URL {}: {}", url_str, e)))?
                    .to_vec();
                self.runtime.load_module_from_bytes(&bytes).await?
            }
        };
        
        let plugin_id = config.id;
        let plugin_arc = Arc::new(Plugin {
            config: Arc::new(config),
            module: Arc::new(wasm_module),
            active_instance_id: Mutex::new(None),
        });

        self.plugins.insert(plugin_id, plugin_arc);
        Ok(plugin_id)
    }

    /// Instantiates a registered plugin.
    /// If an instance already exists for this plugin, it might be an error or it might be terminated first.
    /// For now, let's assume it creates a new instance, replacing the old one if any.
    pub async fn instantiate_plugin(
        &self,
        plugin_id: PluginId,
        // HostState components:
        agent_config: Arc<AgentConfig>, // This AgentConfig is for THIS instance of the plugin
        event_publisher: Arc<dyn EventPublisher + Send + Sync>,
        credential_provider: Arc<dyn CredentialProvider + Send + Sync>,
        state_store: Arc<dyn StateStore + Send + Sync>,
    ) -> Result<InstanceId, WasmError> {
        let plugin_arc = self.plugins.get(&plugin_id)
            .ok_or_else(|| WasmError::PluginNotFound(plugin_id.to_string()))?
            .clone(); // Clone Arc to work with

        // Terminate existing instance if any
        if let Some(old_instance_id) = plugin_arc.active_instance_id() {
            tracing::info!("Terminating old instance {} for plugin {}", old_instance_id, plugin_id);
            self.runtime.terminate_instance(old_instance_id).await.map_err(|e|
                WasmError::InternalError(format!("Failed to terminate old instance {}: {}", old_instance_id, e))
            )?;
            *plugin_arc.active_instance_id.lock().unwrap() = None;
        }

        let instance_id = self.runtime.instantiate_module(
            &plugin_arc.module, // Pass the compiled WasmModule
            agent_config,
            event_publisher,
            credential_provider,
            state_store,
        ).await?;

        *plugin_arc.active_instance_id.lock().unwrap() = Some(instance_id);
        Ok(instance_id)
    }


    /// Executes a function on an active instance of a plugin.
    pub async fn call_plugin_function(
        &self,
        plugin_id: PluginId,
        function_name: &str,
        args: &[Val],
    ) -> Result<ExecutionResult, WasmError> {
        let plugin = self.plugins.get(&plugin_id)
            .ok_or_else(|| WasmError::PluginNotFound(plugin_id.to_string()))?;
        
        let instance_id = plugin.active_instance_id()
            .ok_or_else(|| WasmError::PluginNotInstantiated(plugin_id.to_string()))?;

        self.runtime.call_function_on_instance(instance_id, function_name, args).await
    }

    /// Terminates the active instance of a plugin, if any.
    pub async fn terminate_plugin_instance(&self, plugin_id: PluginId) -> Result<(), WasmError> {
        let plugin = self.plugins.get(&plugin_id)
            .ok_or_else(|| WasmError::PluginNotFound(plugin_id.to_string()))?;
        
        if let Some(instance_id) = plugin.active_instance_id.lock().unwrap().take() {
            self.runtime.terminate_instance(instance_id).await
        } else {
            Ok(()) // No active instance to terminate
        }
    }
    
    /// Unregisters a plugin and terminates its active instance if any.
    /// This removes the plugin configuration and compiled module from the manager.
    pub async fn unregister_plugin(&mut self, plugin_id: PluginId) -> Result<(), WasmError> {
        if let Some(plugin_arc) = self.plugins.remove(&plugin_id) {
            if let Some(instance_id) = plugin_arc.active_instance_id.lock().unwrap().take() {
                self.runtime.terminate_instance(instance_id).await?;
            }
            Ok(())
        } else {
            Err(WasmError::PluginNotFound(plugin_id.to_string()))
        }
    }

    pub fn get_plugin_config(&self, plugin_id: PluginId) -> Option<Arc<PluginConfig>> {
        self.plugins.get(&plugin_id).map(|p| Arc::clone(&p.config))
    }

    pub fn list_plugins(&self) -> Vec<Arc<PluginConfig>> {
        self.plugins.values().map(|p| Arc::clone(&p.config)).collect()
    }
}
