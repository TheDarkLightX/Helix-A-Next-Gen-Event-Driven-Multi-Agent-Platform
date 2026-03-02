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

//! WASM runtime implementation

use crate::{
    errors::WasmError,
    host_functions::{self, HostState},
    sandbox, WasmRuntimeConfig,
};
use helix_agent_sdk::EventPublisher;
use helix_core::types::AgentId;
use helix_core::{agent::AgentConfig, credential::CredentialProvider, state::StateStore};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
}; // Added Mutex for active_instances
use uuid::Uuid; // For InstanceId
use wasmtime::{Engine, Instance, Linker, Module, Store, StoreLimitsBuilder, Val};

/// Unique identifier for a WASM module instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstanceId(Uuid);

impl InstanceId {
    pub fn new() -> Self {
        InstanceId(Uuid::new_v4())
    }
}

impl Default for InstanceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for InstanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Represents an active, instantiated WASM module.
struct ManagedInstance {
    instance: Instance,
    store: Store<HostState>, // Store is mutable, so direct ownership or careful management needed.
    // If Store needs to be accessed mutably by multiple calls, it must be wrapped.
    // For now, assuming call_function_on_instance will take &mut self for WasmRuntime,
    // allowing mutable access to the store within ManagedInstance.
    agent_id: AgentId, // For context
}

/// WASM runtime for executing modules
pub struct WasmRuntime {
    engine: Engine,
    config: Arc<WasmRuntimeConfig>,
    active_instances: Arc<Mutex<HashMap<InstanceId, ManagedInstance>>>,
}

/// A loaded and compiled WASM module
#[derive(Clone)] // Added Clone
pub struct WasmModule {
    /// Compiled module
    module: Module,
    /// Exported functions (names) - can be extracted from the module if needed
    pub exports: Vec<String>,
}

/// Result of WASM execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Return value
    pub result: serde_json::Value,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Memory used in bytes
    pub memory_used: u64,
    /// Instructions executed
    pub instructions_executed: u64,
}

impl WasmRuntime {
    /// Create a new WASM runtime
    pub fn new(config: WasmRuntimeConfig) -> Result<Self, WasmError> {
        let engine = sandbox::configure_engine(&config).map_err(|e| {
            WasmError::ConfigurationError(format!("Failed to configure engine: {}", e))
        })?;
        Ok(Self {
            engine,
            config: Arc::new(config),
            active_instances: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Load and compile a WASM module from bytes.
    pub async fn load_module_from_bytes(&self, wasm_bytes: &[u8]) -> Result<WasmModule, WasmError> {
        crate::utils::validate_wasm(wasm_bytes)?;

        let module = Module::from_binary(&self.engine, wasm_bytes).map_err(|e| {
            WasmError::LoadingError(format!("Failed to compile module from bytes: {}", e))
        })?;

        let exports = module
            .exports()
            .map(|export| export.name().to_string())
            .collect::<Vec<String>>();

        Ok(WasmModule { module, exports })
    }

    /// Load and compile a WASM module from a file path.
    pub async fn load_module_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<WasmModule, WasmError> {
        let module = Module::from_file(&self.engine, path).map_err(|e| {
            WasmError::LoadingError(format!("Failed to load module from path {:?}: {}", path, e))
        })?;

        let exports = module
            .exports()
            .map(|export| export.name().to_string())
            .collect::<Vec<String>>();

        Ok(WasmModule { module, exports })
    }

    /// Instantiate a compiled WASM module.
    ///
    /// This sets up the execution environment (Store, Linker, HostState) for the module.
    pub async fn instantiate_module(
        &self,
        wasm_module: &WasmModule,
        // HostState components:
        agent_config: Arc<AgentConfig>,
        event_publisher: Arc<dyn EventPublisher + Send + Sync>,
        credential_provider: Arc<dyn CredentialProvider + Send + Sync>,
        state_store: Arc<dyn StateStore + Send + Sync>,
    ) -> Result<InstanceId, WasmError> {
        let store_limits = StoreLimitsBuilder::new()
            .memory_size(self.config.max_memory as usize)
            .instances(1)
            .tables(self.config.resource_limits.max_tables as usize)
            .table_elements(self.config.resource_limits.max_table_size)
            .build();

        let host_state = HostState {
            agent_config: Arc::clone(&agent_config),
            event_publisher,
            credential_provider,
            state_store,
            store_limits,
        };

        let mut store = Store::new(&self.engine, host_state);
        sandbox::configure_store(&mut store, &self.config).map_err(|e| {
            WasmError::ConfigurationError(format!("Failed to configure store: {}", e))
        })?;

        let mut linker = Linker::new(&self.engine);
        host_functions::link_all_functions(&mut linker, &self.config).map_err(|e| {
            WasmError::ConfigurationError(format!("Failed to link host functions: {}", e))
        })?;

        let instance = linker
            .instantiate_async(&mut store, &wasm_module.module)
            .await
            .map_err(|e| {
                WasmError::InstantiationError(format!("Failed to instantiate module: {}", e))
            })?;

        let instance_id = InstanceId::new();
        let managed_instance = ManagedInstance {
            instance,
            store, // Store is moved here
            agent_id: agent_config.id.clone(),
        };

        self.active_instances
            .lock()
            .unwrap()
            .insert(instance_id, managed_instance);
        Ok(instance_id)
    }

    /// Calls an exported function on an already instantiated WASM module.
    pub async fn call_function_on_instance(
        &self,
        instance_id: InstanceId,
        function_name: &str,
        args: &[Val],
    ) -> Result<ExecutionResult, WasmError> {
        let mut instances_guard = self.active_instances.lock().unwrap();
        let managed_instance = instances_guard
            .get_mut(&instance_id)
            .ok_or_else(|| WasmError::InstanceNotFound(instance_id.to_string()))?;

        // `store` is now `&mut managed_instance.store`
        // `instance` is `&managed_instance.instance`

        let func = managed_instance
            .instance
            .get_func(&mut managed_instance.store, function_name)
            .ok_or_else(|| {
                WasmError::FunctionNotFound(format!(
                    "Function '{}' not found in instance {}",
                    function_name, instance_id
                ))
            })?;

        let fuel_before = managed_instance.store.get_fuel().unwrap_or(0);

        let start_time = std::time::Instant::now();

        let mut results = vec![Val::I32(0); func.ty(&managed_instance.store).results().len()];
        func.call_async(&mut managed_instance.store, args, &mut results)
            .await
            .map_err(|e| {
                WasmError::ExecutionError(format!(
                    "Function call failed for instance {}: {}",
                    instance_id, e
                ))
            })?;

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        let result_json = results
            .first()
            .map(|val| match val {
                Val::I32(i) => serde_json::json!(i),
                Val::I64(i) => serde_json::json!(i),
                _ => serde_json::Value::Null,
            })
            .unwrap_or(serde_json::Value::Null);

        let fuel_after = managed_instance.store.get_fuel().unwrap_or(0);
        let fuel_consumed = fuel_before.saturating_sub(fuel_after);
        let memory_used = managed_instance
            .instance
            .get_memory(&mut managed_instance.store, "memory")
            .map(|m| m.data_size(&managed_instance.store) as u64)
            .unwrap_or(0);

        Ok(ExecutionResult {
            result: result_json,
            execution_time_ms,
            memory_used,
            instructions_executed: fuel_consumed,
        })
    }

    /// Terminates a running WASM module instance and releases its resources.
    pub async fn terminate_instance(&self, instance_id: InstanceId) -> Result<(), WasmError> {
        let mut instances_guard = self.active_instances.lock().unwrap();
        if instances_guard.remove(&instance_id).is_some() {
            Ok(())
        } else {
            Err(WasmError::InstanceNotFound(instance_id.to_string()))
        }
    }

    /// Get a list of active instance IDs.
    pub fn list_active_instances(&self) -> Vec<InstanceId> {
        self.active_instances
            .lock()
            .unwrap()
            .keys()
            .cloned()
            .collect()
    }

    /// Get agent ID for a given instance ID.
    pub fn get_agent_id_for_instance(&self, instance_id: InstanceId) -> Option<AgentId> {
        self.active_instances
            .lock()
            .unwrap()
            .get(&instance_id)
            .map(|mi| mi.agent_id.clone())
    }
}
