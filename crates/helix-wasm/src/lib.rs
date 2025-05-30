#![warn(missing_docs)]

//! WASM runtime and plugin system for Helix.
//!
//! This crate provides:
//! - WASM module loading and execution
//! - Sandboxed plugin environment
//! - Host function bindings
//! - Resource management and limits
//! - Plugin lifecycle management

pub mod runtime;
pub mod plugins;
pub mod host_functions;
pub mod sandbox;
pub mod errors;
pub mod utils; // Declare the utils module

pub use errors::WasmError;
pub use runtime::{WasmRuntime, WasmModule, ExecutionResult};
pub use plugins::{Plugin, PluginManager, PluginConfig};

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
    /// Note: Actual enforcement depends on host OS permissions.
    pub read_only: bool,
}

/// Configuration for WASM runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmRuntimeConfig {
    /// Maximum memory in bytes
    pub max_memory: u64,
    /// Maximum execution time in milliseconds
    pub max_execution_time_ms: u64,
    /// Maximum number of instructions (used as fuel)
    pub max_instructions: u64,
    /// Whether to enable WASI
    pub enable_wasi: bool,
    /// Allowed host functions (currently informational, not strictly enforced by this config alone)
    pub allowed_host_functions: Vec<String>,
    /// Resource limits for the WASM instance
    pub resource_limits: ResourceLimits,
    /// Directories accessible to the WASM module via WASI.
    pub allowed_dirs: Option<Vec<AllowedDirEntry>>,
    /// Environment variables accessible to the WASM module via WASI.
    pub allowed_env_vars: Option<HashMap<String, String>>,
    /// Whether to allow TCP/UDP socket access for WASI.
    pub allow_network_sockets: bool,
}

/// Resource limits for WASM execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum stack size
    pub max_stack_size: u32,
    /// Maximum number of globals
    pub max_globals: u32,
    /// Maximum number of functions
    pub max_functions: u32,
    /// Maximum number of tables
    pub max_tables: u32,
    /// Maximum table size
    pub max_table_size: u32,
}

/// Trait for WASM-powered agents
#[async_trait]
pub trait WasmAgent: helix_core::agent::Agent {
    /// Load a WASM module
    async fn load_module(&mut self, wasm_bytes: &[u8]) -> Result<(), WasmError>;

    /// Execute a function in the loaded module
    async fn execute_function(
        &mut self,
        function_name: &str,
        args: &[serde_json::Value],
    ) -> Result<serde_json::Value, WasmError>;

    /// Get the current module's exports
    fn get_exports(&self) -> Vec<String>;

    /// Check if a function exists in the module
    fn has_function(&self, function_name: &str) -> bool;
}

impl Default for WasmRuntimeConfig {
    fn default() -> Self {
        Self {
            max_memory: 64 * 1024 * 1024, // 64MB
            max_execution_time_ms: 5000,   // 5 seconds
            max_instructions: 1_000_000_000, // 1 Billion instructions (fuel)
            enable_wasi: true,
            allowed_host_functions: vec![ // Examples, actual enforcement is via linker
                "helix_log_message".to_string(),
                "helix_emit_event".to_string(),
                "helix_get_config_value".to_string(),
                "helix_get_state".to_string(),
                "helix_set_state".to_string(),
                "helix_get_credential".to_string(),
            ],
            resource_limits: ResourceLimits::default(),
            allowed_dirs: None,
            allowed_env_vars: None,
            allow_network_sockets: false, // Default to no network access for security
        }
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_stack_size: 1024 * 1024, // 1MB
            max_globals: 1000,
            max_functions: 10000,
            max_tables: 10,
            max_table_size: 10000,
        }
    }
}

// Removed inline utils module, it's now in utils.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_runtime_config_default() {
        let config = WasmRuntimeConfig::default();
        assert_eq!(config.max_memory, 64 * 1024 * 1024);
        assert_eq!(config.max_execution_time_ms, 5000);
        assert!(config.enable_wasi);
    }

    #[test]
    fn test_wasm_validation() {
        // Valid WASM magic number and version
        let valid_wasm = b"\0asm\x01\x00\x00\x00";
        assert!(utils::validate_wasm(valid_wasm).is_ok());

        // Invalid magic number
        let invalid_wasm = b"invalid\x01\x00\x00\x00";
        assert!(utils::validate_wasm(invalid_wasm).is_err());

        // Too small
        let too_small = b"\0asm";
        assert!(utils::validate_wasm(too_small).is_err());
    }

    #[test]
    fn test_function_extraction() {
        let valid_wasm = b"\0asm\x01\x00\x00\x00";
        let functions = utils::extract_function_names(valid_wasm).unwrap();
        assert!(!functions.is_empty());
        assert!(functions.contains(&"main".to_string()));
    }

    #[test]
    fn test_execution_cost_estimation() {
        let valid_wasm = b"\0asm\x01\x00\x00\x00extra_data";
        let cost = utils::estimate_execution_cost(valid_wasm).unwrap();
        assert_eq!(cost, valid_wasm.len() as u64 * 10);
    }

    #[test]
    fn test_safety_check() {
        let config = WasmRuntimeConfig::default();
        let valid_wasm = b"\0asm\x01\x00\x00\x00";
        
        let is_safe = utils::is_safe_module(valid_wasm, &config).unwrap();
        assert!(is_safe);
    }
}
