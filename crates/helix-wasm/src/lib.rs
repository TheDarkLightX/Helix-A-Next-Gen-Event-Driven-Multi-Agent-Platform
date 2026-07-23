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

#![warn(missing_docs)]

//! WASM runtime and plugin system for Helix.
//!
//! This crate provides:
//! - WASM module loading and execution
//! - Sandboxed plugin environment
//! - Host function bindings
//! - Resource management and limits
//! - Plugin lifecycle management

pub mod errors;
pub mod host_functions;
pub mod plugins;
pub mod runtime;
pub mod sandbox;
pub mod utils;

pub use errors::WasmError;
pub use plugins::{Plugin, PluginConfig, PluginManager};
pub use runtime::{ExecutionResult, WasmModule, WasmRuntime};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Represents a directory allowed to be accessed by a WASM module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowedDirEntry {
    /// The path on the host system.
    pub host_path: PathBuf,
    /// The path as seen by the guest WASM module.
    pub guest_path: PathBuf,
    /// Whether the guest has read-only access.
    ///
    /// Directory preopens are not yet wired into the runtime; configurations
    /// containing them currently fail closed during runtime construction.
    pub read_only: bool,
}

/// Configuration for the WASM runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmRuntimeConfig {
    /// Maximum memory in bytes.
    pub max_memory: u64,
    /// Maximum wall-clock execution time in milliseconds.
    pub max_execution_time_ms: u64,
    /// Maximum number of instructions, enforced with Wasmtime fuel.
    pub max_instructions: u64,
    /// Whether to enable WASI.
    ///
    /// WASI is currently rejected because the runtime has not yet implemented
    /// policy-bound directory, environment, and socket preopens.
    pub enable_wasi: bool,
    /// Exact host-function capabilities linked into each instance.
    pub allowed_host_functions: Vec<String>,
    /// Resource limits for the WASM instance.
    pub resource_limits: ResourceLimits,
    /// Directories accessible to the WASM module via WASI.
    pub allowed_dirs: Option<Vec<AllowedDirEntry>>,
    /// Environment variables accessible to the WASM module via WASI.
    pub allowed_env_vars: Option<HashMap<String, String>>,
    /// Whether to allow TCP/UDP socket access for WASI.
    ///
    /// Network sockets are not implemented and a true value fails closed.
    pub allow_network_sockets: bool,
}

/// Resource limits for WASM execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum stack size.
    pub max_stack_size: u32,
    /// Maximum number of globals.
    pub max_globals: u32,
    /// Maximum number of functions.
    pub max_functions: u32,
    /// Maximum number of tables.
    pub max_tables: u32,
    /// Maximum table size.
    pub max_table_size: u32,
}

/// Trait for WASM-powered agents.
#[async_trait]
pub trait WasmAgent: helix_core::agent::Agent {
    /// Load a WASM module.
    async fn load_module(&mut self, wasm_bytes: &[u8]) -> Result<(), WasmError>;

    /// Execute a function in the loaded module.
    async fn execute_function(
        &mut self,
        function_name: &str,
        args: &[serde_json::Value],
    ) -> Result<serde_json::Value, WasmError>;

    /// Get the current module's exports.
    fn get_exports(&self) -> Vec<String>;

    /// Check whether a function exists in the module.
    fn has_function(&self, function_name: &str) -> bool;
}

impl Default for WasmRuntimeConfig {
    fn default() -> Self {
        Self {
            max_memory: 64 * 1024 * 1024,
            max_execution_time_ms: 5_000,
            max_instructions: 1_000_000_000,
            enable_wasi: false,
            // Default to read-only, deterministic capabilities. State mutation,
            // event emission, credentials, host time, and randomness all require
            // an explicit per-runtime grant.
            allowed_host_functions: vec![
                host_functions::HOST_LOG_MESSAGE.to_string(),
                host_functions::HOST_GET_CONFIG_VALUE.to_string(),
                host_functions::HOST_GET_STATE.to_string(),
            ],
            resource_limits: ResourceLimits::default(),
            allowed_dirs: None,
            allowed_env_vars: None,
            allow_network_sockets: false,
        }
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_stack_size: 1024 * 1024,
            max_globals: 1_000,
            max_functions: 10_000,
            max_tables: 10,
            max_table_size: 10_000,
        }
    }
}
