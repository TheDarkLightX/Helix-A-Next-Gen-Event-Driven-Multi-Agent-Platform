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


//! Error types for WASM operations

use thiserror::Error;

/// Errors that can occur during WASM operations
#[derive(Error, Debug)]
pub enum WasmError {
    /// Invalid WASM module
    #[error("Invalid WASM module: {0}")]
    InvalidModule(String),

    /// Module loading failed
    #[error("Module loading failed: {0}")]
    LoadingError(String),

    /// Instance not found in runtime
    #[error("WASM Instance not found: {0}")]
    InstanceNotFound(String),

    /// Function execution failed
    #[error("Function execution failed: {0}")]
    ExecutionError(String),

    /// Function not found
    #[error("Function not found: {0}")]
    FunctionNotFound(String),

    /// Invalid function arguments
    #[error("Invalid function arguments: {0}")]
    InvalidArguments(String),

    /// Resource limit exceeded
    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    /// Execution timeout
    #[error("Execution timeout: {0}ms")]
    ExecutionTimeout(u64),

    /// Memory access violation
    #[error("Memory access violation: {0}")]
    MemoryViolation(String),

    /// Host function error
    #[error("Host function error: {0}")]
    HostFunctionError(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// WASI error
    #[error("WASI error: {0}")]
    WasiError(String),

    /// Sandbox violation
    #[error("Sandbox violation: {0}")]
    SandboxViolation(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// Generic internal error
    #[error("Internal WASM error: {0}")]
    InternalError(String),

    /// Error during WASM module instantiation
    #[error("WASM Instantiation error: {0}")]
    InstantiationError(String),

    /// Plugin not found
    #[error("Plugin not found: {0}")]
    PluginNotFound(String),

    /// Plugin already exists
    #[error("Plugin already exists: {0}")]
    PluginAlreadyExists(String),

    /// Plugin is not instantiated
    #[error("Plugin not instantiated: {0}")]
    PluginNotInstantiated(String),
    
    /// Trap during WASM execution
    #[error("WASM Trap: {0}")]
    Trap(#[from] wasmtime::Trap),
}

impl WasmError {
    // Standardized error codes for host functions returning i32 to WASM
    // to indicate specific error conditions.
    // Guest code can check for these negative values.
    // A successful operation returning a length or count should be >= 0.

    /// Generic error in host function execution.
    pub const HOST_FUNCTION_ERROR_CODE: i32 = -1;
    /// Requested value or resource not found.
    pub const VALUE_NOT_FOUND_CODE: i32 = -2;
    /// Provided buffer by guest was too small for the result.
    pub const BUFFER_TOO_SMALL_CODE: i32 = -3;
    /// Invalid argument provided by the guest to a host function.
    pub const INVALID_ARGUMENT_CODE: i32 = -4;
    /// Operation resulted in an I/O error on the host.
    pub const IO_ERROR_CODE: i32 = -5;
    /// Serialization or deserialization error during host-guest data exchange.
    pub const SERIALIZATION_ERROR_CODE: i32 = -6;
    // Add more as needed
}

impl From<WasmError> for helix_core::HelixError {
    fn from(err: WasmError) -> Self {
        helix_core::HelixError::InternalError(format!("WASM error: {}", err))
    }
}
