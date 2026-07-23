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

//! WASM runtime implementation.

use crate::{
    errors::WasmError,
    host_functions::{self, HostState},
    sandbox, WasmRuntimeConfig,
};
use helix_agent_sdk::EventPublisher;
use helix_core::types::AgentId;
use helix_core::{agent::AgentConfig, credential::CredentialProvider, state::StateStore};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use uuid::Uuid;
use wasmtime::{Engine, Instance, Linker, Module, Store, StoreLimitsBuilder, Val};

/// Unique identifier for a WASM module instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstanceId(Uuid);

impl InstanceId {
    /// Create a new opaque instance identity.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
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
    store: Store<HostState>,
    agent_id: AgentId,
}

/// WASM runtime for executing modules.
pub struct WasmRuntime {
    engine: Engine,
    config: Arc<WasmRuntimeConfig>,
    active_instances: Arc<Mutex<HashMap<InstanceId, ManagedInstance>>>,
}

/// A loaded and compiled WASM module.
#[derive(Clone)]
pub struct WasmModule {
    /// Compiled module.
    module: Module,
    /// Exported function and value names.
    pub exports: Vec<String>,
}

/// Result of WASM execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// First supported return value, or JSON null.
    pub result: serde_json::Value,
    /// Execution time in milliseconds.
    pub execution_time_ms: u64,
    /// Current exported linear-memory size in bytes.
    pub memory_used: u64,
    /// Instructions/fuel consumed by this invocation.
    pub instructions_executed: u64,
}

struct EpochDeadline {
    cancel: Option<mpsc::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
    fired: Arc<AtomicBool>,
}

impl EpochDeadline {
    fn start(engine: Engine, timeout: Duration) -> Self {
        let (cancel_tx, cancel_rx) = mpsc::channel();
        let fired = Arc::new(AtomicBool::new(false));
        let fired_for_thread = Arc::clone(&fired);
        let thread = thread::spawn(move || {
            if cancel_rx.recv_timeout(timeout).is_err() {
                fired_for_thread.store(true, Ordering::Release);
                engine.increment_epoch();
            }
        });
        Self {
            cancel: Some(cancel_tx),
            thread: Some(thread),
            fired,
        }
    }

    fn fired(&self) -> bool {
        self.fired.load(Ordering::Acquire)
    }
}

impl Drop for EpochDeadline {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl WasmRuntime {
    /// Create a new policy-bound WASM runtime.
    pub fn new(config: WasmRuntimeConfig) -> Result<Self, WasmError> {
        sandbox::validate_runtime_config(&config).map_err(|error| {
            WasmError::ConfigurationError(format!("invalid runtime policy: {error}"))
        })?;
        let engine = sandbox::configure_engine(&config).map_err(|error| {
            WasmError::ConfigurationError(format!("failed to configure engine: {error}"))
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

        let module = Module::from_binary(&self.engine, wasm_bytes).map_err(|error| {
            WasmError::LoadingError(format!("failed to compile module from bytes: {error}"))
        })?;

        let exports = module
            .exports()
            .map(|export| export.name().to_string())
            .collect::<Vec<_>>();

        Ok(WasmModule { module, exports })
    }

    /// Load, validate, and compile a WASM module from a file path.
    pub async fn load_module_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<WasmModule, WasmError> {
        let bytes = tokio::fs::read(path).await.map_err(|error| {
            WasmError::LoadingError(format!("failed to read module from {path:?}: {error}"))
        })?;
        self.load_module_from_bytes(&bytes).await
    }

    /// Instantiate a compiled WASM module with exactly the configured host capabilities.
    pub async fn instantiate_module(
        &self,
        wasm_module: &WasmModule,
        agent_config: Arc<AgentConfig>,
        event_publisher: Arc<dyn EventPublisher>,
        credential_provider: Arc<dyn CredentialProvider>,
        state_store: Arc<dyn StateStore>,
    ) -> Result<InstanceId, WasmError> {
        let max_memory = usize::try_from(self.config.max_memory).map_err(|_| {
            WasmError::ConfigurationError("max_memory exceeds platform limits".to_string())
        })?;
        let max_tables = usize::try_from(self.config.resource_limits.max_tables).map_err(|_| {
            WasmError::ConfigurationError("max_tables exceeds platform limits".to_string())
        })?;
        let max_table_size =
            usize::try_from(self.config.resource_limits.max_table_size).map_err(|_| {
                WasmError::ConfigurationError(
                    "max_table_size exceeds platform limits".to_string(),
                )
            })?;
        let store_limits = StoreLimitsBuilder::new()
            .memory_size(max_memory)
            .instances(1)
            .tables(max_tables)
            .table_elements(max_table_size)
            .build();

        let host_state = HostState {
            agent_config: Arc::clone(&agent_config),
            event_publisher,
            credential_provider,
            state_store,
            store_limits,
        };

        let mut store = Store::new(&self.engine, host_state);
        sandbox::configure_store(&mut store, &self.config).map_err(|error| {
            WasmError::ConfigurationError(format!("failed to configure store: {error}"))
        })?;

        let mut linker = Linker::new(&self.engine);
        host_functions::link_all_functions(&mut linker, &self.config).map_err(|error| {
            WasmError::ConfigurationError(format!("failed to link host capabilities: {error}"))
        })?;

        let instance = linker
            .instantiate_async(&mut store, &wasm_module.module)
            .await
            .map_err(|error| {
                WasmError::InstantiationError(format!("failed to instantiate module: {error}"))
            })?;

        let instance_id = InstanceId::new();
        let managed_instance = ManagedInstance {
            instance,
            store,
            agent_id: agent_config.id,
        };
        self.active_instances
            .lock()
            .map_err(|_| WasmError::InternalError("instance registry lock poisoned".to_string()))?
            .insert(instance_id, managed_instance);
        Ok(instance_id)
    }

    /// Call an exported function under per-call fuel and wall-clock bounds.
    ///
    /// A timed-out instance is removed from the registry because a runtime that
    /// required forced interruption is not reused implicitly.
    pub async fn call_function_on_instance(
        &self,
        instance_id: InstanceId,
        function_name: &str,
        args: &[Val],
    ) -> Result<ExecutionResult, WasmError> {
        let mut instances = self
            .active_instances
            .lock()
            .map_err(|_| WasmError::InternalError("instance registry lock poisoned".to_string()))?;

        let timeout = Duration::from_millis(self.config.max_execution_time_ms);
        let deadline = EpochDeadline::start(self.engine.clone(), timeout);
        let start = std::time::Instant::now();

        let call_outcome = {
            let managed_instance = instances
                .get_mut(&instance_id)
                .ok_or_else(|| WasmError::InstanceNotFound(instance_id.to_string()))?;

            managed_instance
                .store
                .set_fuel(self.config.max_instructions)
                .map_err(|error| {
                    WasmError::ConfigurationError(format!("failed to reset fuel: {error}"))
                })?;
            managed_instance.store.set_epoch_deadline(1);
            managed_instance.store.epoch_deadline_trap();

            let function = managed_instance
                .instance
                .get_func(&mut managed_instance.store, function_name)
                .ok_or_else(|| {
                    WasmError::FunctionNotFound(format!(
                        "function {function_name:?} not found in instance {instance_id}"
                    ))
                })?;

            let fuel_before = managed_instance.store.get_fuel().unwrap_or(0);
            let result_count = function.ty(&managed_instance.store).results().len();
            let mut results = vec![Val::I32(0); result_count];
            let call_result = function
                .call_async(&mut managed_instance.store, args, &mut results)
                .await;
            let fuel_after = managed_instance.store.get_fuel().unwrap_or(0);
            let memory_used = managed_instance
                .instance
                .get_memory(&mut managed_instance.store, "memory")
                .map(|memory| memory.data_size(&managed_instance.store) as u64)
                .unwrap_or(0);

            (call_result, results, fuel_before, fuel_after, memory_used)
        };

        let timed_out = deadline.fired();
        drop(deadline);
        let execution_time_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);

        if timed_out {
            instances.remove(&instance_id);
            return Err(WasmError::ExecutionTimeout(
                self.config.max_execution_time_ms,
            ));
        }

        let (call_result, results, fuel_before, fuel_after, memory_used) = call_outcome;
        call_result.map_err(|error| {
            WasmError::ExecutionError(format!(
                "function call failed for instance {instance_id}: {error}"
            ))
        })?;

        let result = results
            .first()
            .map(|value| match value {
                Val::I32(value) => serde_json::json!(value),
                Val::I64(value) => serde_json::json!(value),
                Val::F32(value) => serde_json::json!(f32::from_bits(*value)),
                Val::F64(value) => serde_json::json!(f64::from_bits(*value)),
                _ => serde_json::Value::Null,
            })
            .unwrap_or(serde_json::Value::Null);

        Ok(ExecutionResult {
            result,
            execution_time_ms,
            memory_used,
            instructions_executed: fuel_before.saturating_sub(fuel_after),
        })
    }

    /// Terminate a running WASM module instance and release its resources.
    pub async fn terminate_instance(&self, instance_id: InstanceId) -> Result<(), WasmError> {
        let removed = self
            .active_instances
            .lock()
            .map_err(|_| WasmError::InternalError("instance registry lock poisoned".to_string()))?
            .remove(&instance_id)
            .is_some();
        if removed {
            Ok(())
        } else {
            Err(WasmError::InstanceNotFound(instance_id.to_string()))
        }
    }

    /// Get a stable list of active instance IDs.
    #[must_use]
    pub fn list_active_instances(&self) -> Vec<InstanceId> {
        let Ok(instances) = self.active_instances.lock() else {
            return Vec::new();
        };
        let mut ids = instances.keys().copied().collect::<Vec<_>>();
        ids.sort_by_key(ToString::to_string);
        ids
    }

    /// Get the agent ID associated with an instance.
    #[must_use]
    pub fn get_agent_id_for_instance(&self, instance_id: InstanceId) -> Option<AgentId> {
        self.active_instances
            .lock()
            .ok()?
            .get(&instance_id)
            .map(|instance| instance.agent_id)
    }
}
