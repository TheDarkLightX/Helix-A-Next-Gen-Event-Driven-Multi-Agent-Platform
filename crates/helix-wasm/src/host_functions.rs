//! Host functions available to WASM modules
use std::sync::Arc;
use wasmtime::{Caller, Linker, Func, Trap, TypedFunc, Val, ValType, Global};
use std::time::SystemTime;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use helix_core::{
    agent::{AgentConfig, CredentialProvider, StateStore, Credential, CredentialData},
    event::Event as HelixEvent,
    types::{AgentId, CredentialId, ProfileId, StateData},
    HelixError,
};
use helix_agent_sdk::{EventPublisher, SdkError};
use serde_json::Value as JsonValue;
use crate::{WasmError, WasmRuntimeConfig, sandbox}; // Added WasmRuntimeConfig and sandbox
use wasmtime_wasi::WasiCtx; // Added WasiCtx


/// State accessible by host functions.
/// This struct will be stored in `wasmtime::Store` and accessed via `Caller::data_mut()`.
pub struct HostState {
    pub agent_config: Arc<AgentConfig>,
    pub event_publisher: Arc<dyn EventPublisher + Send + Sync>,
    pub credential_provider: Arc<dyn CredentialProvider + Send + Sync>,
    pub state_store: Arc<dyn StateStore + Send + Sync>,
    pub wasi_ctx: Option<WasiCtx>, // Added for WASI support
    // Potentially add a buffer for string arguments/returns if not handled by Wasmtime direct calls
}

// Helper function to read a string from WASM memory
fn read_string_from_wasm(caller: &mut Caller<'_, HostState>, ptr: i32, len: i32) -> Result<String, Trap> {
    let mem = match caller.get_export("memory") {
        Some(wasmtime::Extern::Memory(mem)) => mem,
        _ => return Err(Trap::new("failed to find host memory")),
    };
    let data = mem
        .data(&caller)
        .get(ptr as u32 as usize..(ptr as u32 + len as u32) as usize);
    match data {
        Some(data) => match std::str::from_utf8(data) {
            Ok(s) => Ok(s.to_string()),
            Err(_) => Err(Trap::new("invalid utf-8 string")),
        },
        None => Err(Trap::new("pointer/length out of bounds")),
    }
}

// Helper function to write a string to WASM memory (and return ptr/len)
// This is more complex as it requires memory allocation in WASM or a shared buffer.
// For now, host functions returning strings might be simpler if they return i32/i32 (ptr/len)
// and the WASM guest copies it. Or, use wasmtime's more advanced ABI features.
// Let's assume for now that complex return types like strings are handled by guest allocating
// and host writing into it, or by serializing to bytes.

/// Links all defined host functions, including WASI if configured, to the provided `wasmtime::Linker`.
pub fn link_all_functions(
    linker: &mut Linker<HostState>,
    runtime_config: &WasmRuntimeConfig, // Pass runtime_config to decide on WASI
) -> Result<(), anyhow::Error> {
    // Link Helix-specific host functions
    link_helix_host_functions(linker)?;

    // Conditionally link WASI
    if runtime_config.enable_wasi {
        // The closure for add_to_linker needs `&mut WasiCtx`.
        // This means HostState.wasi_ctx must be Some(WasiCtx) and mutable.
        wasmtime_wasi::add_to_linker(linker, |host_state: &mut HostState| {
            host_state.wasi_ctx.as_mut().expect("WASI enabled but WasiCtx not initialized in HostState")
        })?;
        tracing::debug!("WASI context linked.");
    }

    Ok(())
}


/// Links only Helix-specific host functions (excluding WASI).
fn link_helix_host_functions(linker: &mut Linker<HostState>) -> Result<(), anyhow::Error> {
    // Logging
    linker.func_wrap("env", "helix_log_message", |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| {
        let message = read_string_from_wasm(&mut caller, ptr, len)?;
        tracing::info!(agent_id = %caller.data().agent_config.id, "WASM: {}", message);
        Ok(())
    })?;

    // Event Publishing
    // Event Publishing
    // Guest provides event_payload as a JSON string.
    // Guest may provide event_type_override as a string.
    linker.func_wrap("env", "helix_emit_event",
        |mut caller: Caller<'_, HostState>, event_payload_ptr: i32, event_payload_len: i32, event_type_ptr: i32, event_type_len: i32|
        -> Result<i32, Trap> {
        // Read and deserialize the event payload JSON string from WASM memory
        let payload_str = read_string_from_wasm(&mut caller, event_payload_ptr, event_payload_len)?;
        let event_payload: JsonValue = serde_json::from_str(&payload_str)
            .map_err(|e| Trap::new(format!("Failed to deserialize event payload JSON: {}", e)))?;
        
        // Read optional event type override string from WASM memory
        let event_type_override = if event_type_ptr != 0 && event_type_len > 0 {
            Some(read_string_from_wasm(&mut caller, event_type_ptr, event_type_len)?)
        } else {
            None
        };

        let host_state = caller.data();
        let agent_id = host_state.agent_config.id.clone();
        let publisher = Arc::clone(&host_state.event_publisher);
        
        // Publish the event
        let result = futures::executor::block_on(
            publisher.publish_event(&agent_id, event_payload, event_type_override)
        );

        match result {
            Ok(_) => Ok(0), // Success
            Err(e) => {
                let err_msg = format!("helix_emit_event failed: {}", e);
                tracing::error!("WASM: {}", err_msg);
                Ok(WasmError::HOST_FUNCTION_ERROR_CODE) // Return an error code to WASM
            }
        }
    })?;

    // Get Config Value
    // Guest provides key as a string.
    // Host returns the config value serialized as a JSON string.
    linker.func_wrap("env", "helix_get_config_value",
        |mut caller: Caller<'_, HostState>, key_ptr: i32, key_len: i32, result_buf_ptr: i32, result_buf_len: i32|
        -> Result<i32, Trap> {
        let key = read_string_from_wasm(&mut caller, key_ptr, key_len)?;
        let config_json = &caller.data().agent_config.config;

        match config_json.get(&key) {
            Some(value) => {
                // Serialize the config value to a JSON string
                let value_str = serde_json::to_string(value)
                    .map_err(|e| Trap::new(format!("Failed to serialize config value to JSON string: {}", e)))?;
                let value_bytes = value_str.as_bytes();

                // Check if the result buffer is large enough
                if value_bytes.len() > result_buf_len as usize {
                    tracing::warn!("WASM helix_get_config_value: result buffer too small for key '{}'. Required: {}, Available: {}", key, value_bytes.len(), result_buf_len);
                    return Ok(WasmError::BUFFER_TOO_SMALL_CODE); // Return buffer too small error code
                }

                // Write the JSON string to the WASM memory buffer
                let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
                mem.write(&mut caller, result_buf_ptr as usize, value_bytes)?;
                Ok(value_bytes.len() as i32) // Return the number of bytes written
            }
            None => {
                tracing::debug!("WASM helix_get_config_value: key '{}' not found.", key);
                Ok(WasmError::VALUE_NOT_FOUND_CODE) // Return value not found error code
            }
        }
    })?;

    // Get State
    // Host returns the agent's state (Vec<u8>) serialized as a JSON string (JSON array of numbers).
    linker.func_wrap("env", "helix_get_state",
        |mut caller: Caller<'_, HostState>, result_buf_ptr: i32, result_buf_len: i32|
        -> Result<i32, Trap> {
        let host_state = caller.data();
        let profile_id = host_state.agent_config.profile_id.clone();
        let agent_id = host_state.agent_config.id.clone();
        let state_store = Arc::clone(&host_state.state_store);

        let result = futures::executor::block_on(
            state_store.get_state(&profile_id, &agent_id)
        );

        match result {
            Ok(Some(state_data)) => {
                // Serialize StateData (Vec<u8>) to a JSON string (e.g., "[1,2,3]")
                let state_json_str = serde_json::to_string(state_data.as_ref())
                    .map_err(|e| Trap::new(format!("Failed to serialize state data to JSON string: {}", e)))?;
                let state_json_bytes = state_json_str.as_bytes();

                if state_json_bytes.len() > result_buf_len as usize {
                    tracing::warn!("WASM helix_get_state: result buffer too small. Required: {}, Available: {}", state_json_bytes.len(), result_buf_len);
                    return Ok(WasmError::BUFFER_TOO_SMALL_CODE);
                }
                let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
                mem.write(&mut caller, result_buf_ptr as usize, state_json_bytes)?;
                Ok(state_json_bytes.len() as i32)
            }
            Ok(None) => {
                tracing::debug!("WASM helix_get_state: no state found for agent {}", caller.data().agent_config.id);
                // Return empty JSON array "[]" to indicate no state, and 0 bytes written, or specific code?
                // For consistency, let's return VALUE_NOT_FOUND_CODE. Guest can check this.
                Ok(WasmError::VALUE_NOT_FOUND_CODE)
            }
            Err(e) => {
                let err_msg = format!("helix_get_state failed for agent {}: {}", caller.data().agent_config.id, e);
                tracing::error!("WASM: {}", err_msg);
                Ok(WasmError::HOST_FUNCTION_ERROR_CODE)
            }
        }
    })?;

    // Set State
    // Guest provides the state as a JSON string (representing a Vec<u8>, e.g., "[1,2,3]").
    // Host deserializes this JSON string to Vec<u8> and stores it.
    linker.func_wrap("env", "helix_set_state",
        |mut caller: Caller<'_, HostState>, state_json_ptr: i32, state_json_len: i32|
        -> Result<i32, Trap> {
        // Read the JSON string from WASM memory
        let state_json_str = read_string_from_wasm(&mut caller, state_json_ptr, state_json_len)?;
        
        // Deserialize the JSON string to Vec<u8>
        let state_bytes: Vec<u8> = serde_json::from_str(&state_json_str)
            .map_err(|e| Trap::new(format!("Failed to deserialize state JSON to bytes: {}. Expected JSON array of numbers. Received: '{}'", e, state_json_str)))?;
        
        let host_state = caller.data();
        let profile_id = host_state.agent_config.profile_id.clone();
        let agent_id = host_state.agent_config.id.clone();
        let state_store = Arc::clone(&host_state.state_store);

        let result = futures::executor::block_on(
            state_store.set_state(&profile_id, &agent_id, StateData::new(state_bytes))
        );
        match result {
            Ok(_) => Ok(0), // Success
            Err(e) => {
                let err_msg = format!("helix_set_state failed for agent {}: {}", caller.data().agent_config.id, e);
                tracing::error!("WASM: {}", err_msg);
                Ok(WasmError::HOST_FUNCTION_ERROR_CODE)
            }
        }
    })?;
    
    // Get Credential
    // Guest provides cred_id as a string.
    // Host returns the credential data serialized as a JSON string.
    linker.func_wrap("env", "helix_get_credential",
        |mut caller: Caller<'_, HostState>, cred_id_ptr: i32, cred_id_len: i32, result_buf_ptr: i32, result_buf_len: i32|
        -> Result<i32, Trap> {
        let cred_id_str = read_string_from_wasm(&mut caller, cred_id_ptr, cred_id_len)?;
        let cred_id = CredentialId::new(&cred_id_str);

        let host_state = caller.data();
        let credential_provider = Arc::clone(&host_state.credential_provider);

        let result = futures::executor::block_on(
            credential_provider.get_credential(&cred_id)
        );

        match result {
            Ok(Some(credential)) => {
                // Serialize credential data to JSON.
                // If CredentialData is Bytes, it will be serialized as a JSON array of numbers.
                // If CredentialData is Json, it will be serialized as is.
                let data_to_serialize = match credential.data {
                    CredentialData::Json(json_val) => json_val,
                    CredentialData::Bytes(bytes) => {
                        // Serialize Vec<u8> as a JSON array of numbers
                        serde_json::to_value(bytes)
                            .map_err(|e| Trap::new(format!("Failed to convert credential bytes to JSON value: {}", e)))?
                    }
                };
                let data_str = serde_json::to_string(&data_to_serialize)
                    .map_err(|e| Trap::new(format!("Failed to serialize credential data to JSON string: {}", e)))?;
                
                let data_bytes = data_str.as_bytes();

                if data_bytes.len() > result_buf_len as usize {
                     tracing::warn!("WASM helix_get_credential: result buffer too small for cred_id '{}'. Required: {}, Available: {}", cred_id_str, data_bytes.len(), result_buf_len);
                    return Ok(WasmError::BUFFER_TOO_SMALL_CODE);
                }
                let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
                mem.write(&mut caller, result_buf_ptr as usize, data_bytes)?;
                Ok(data_bytes.len() as i32)
            }
            Ok(None) => {
                tracing::debug!("WASM helix_get_credential: credential_id '{}' not found.", cred_id_str);
                Ok(WasmError::VALUE_NOT_FOUND_CODE)
            }
            Err(e) => {
                let err_msg = format!("helix_get_credential failed for cred_id {}: {}", cred_id_str, e);
                tracing::error!("WASM: {}", err_msg);
                Ok(WasmError::HOST_FUNCTION_ERROR_CODE)
            }
        }
    })?;

    // Placeholder for other functions like get_time, random if still needed
    linker.func_wrap("env", "helix_get_time", || -> Result<u64, Trap> {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|e| Trap::new(format!("Failed to get system time: {}", e)))
            .map(|d| d.as_millis() as u64)
    })?;

    linker.func_wrap("env", "helix_random", || -> Result<u32, Trap> {
        let mut hasher = DefaultHasher::new();
        SystemTime::now().hash(&mut hasher);
        Ok(hasher.finish() as u32)
    })?;

    Ok(())
}

// Old HostFunctions struct and impl can be removed or refactored.
// The new approach is to use free functions linked via the Linker.
/*
pub struct HostFunctions;

impl HostFunctions {
    /// Log a message from WASM
    pub fn log(message: &str) {
        tracing::info!("WASM log: {}", message);
    }

    /// Get current timestamp
    pub fn get_time() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    /// Generate random number
    pub fn random() -> u32 {
        // Simple random number generation
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        std::time::SystemTime::now().hash(&mut hasher);
        hasher.finish() as u32
    }
}
*/
