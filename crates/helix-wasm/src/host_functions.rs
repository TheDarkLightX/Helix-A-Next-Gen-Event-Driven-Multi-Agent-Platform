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

//! Host functions available to WASM modules.

use anyhow::{anyhow, Result};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::SystemTime;

use helix_agent_sdk::EventPublisher;
use helix_core::agent::AgentConfig;
use helix_core::credential::CredentialProvider;
use helix_core::state::StateStore;
use helix_core::types::{AgentId, CredentialId, ProfileId};
use serde_json::Value as JsonValue;
use wasmtime::{Caller, Linker, StoreLimits};

use crate::{WasmError, WasmRuntimeConfig};

/// State accessible by host functions.
pub struct HostState {
    pub agent_config: Arc<AgentConfig>,
    pub event_publisher: Arc<dyn EventPublisher>,
    pub credential_provider: Arc<dyn CredentialProvider>,
    pub state_store: Arc<dyn StateStore>,
    pub store_limits: StoreLimits,
}

fn read_string_from_wasm(caller: &mut Caller<'_, HostState>, ptr: i32, len: i32) -> Result<String> {
    let mem = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| anyhow!("failed to find host memory"))?;
    let data = mem
        .data(caller)
        .get(ptr as u32 as usize..(ptr as u32 + len as u32) as usize)
        .ok_or_else(|| anyhow!("pointer/length out of bounds"))?;
    Ok(std::str::from_utf8(data)?.to_string())
}

pub fn link_all_functions(
    linker: &mut Linker<HostState>,
    runtime_config: &WasmRuntimeConfig,
) -> Result<()> {
    let _ = runtime_config; // currently unused
    link_helix_host_functions(linker)?;
    Ok(())
}

fn link_helix_host_functions(linker: &mut Linker<HostState>) -> Result<()> {
    linker.func_wrap(
        "env",
        "helix_log_message",
        |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| -> Result<()> {
            let message = read_string_from_wasm(&mut caller, ptr, len)?;
            tracing::info!(agent_id = %caller.data().agent_config.id, "WASM: {}", message);
            Ok(())
        },
    )?;

    linker.func_wrap(
        "env",
        "helix_emit_event",
        |mut caller: Caller<'_, HostState>,
         payload_ptr: i32,
         payload_len: i32,
         ty_ptr: i32,
         ty_len: i32|
         -> Result<i32> {
            let payload_str = read_string_from_wasm(&mut caller, payload_ptr, payload_len)?;
            let payload: JsonValue = serde_json::from_str(&payload_str)
                .map_err(|e| anyhow!("failed to parse event payload json: {}", e))?;

            let event_type = if ty_ptr != 0 && ty_len > 0 {
                Some(read_string_from_wasm(&mut caller, ty_ptr, ty_len)?)
            } else {
                None
            };

            let state = caller.data();
            let agent_id = state.agent_config.id;
            let publisher = Arc::clone(&state.event_publisher);

            let res = futures::executor::block_on(
                publisher.publish_event(&agent_id, payload, event_type),
            );
            match res {
                Ok(_) => Ok(0),
                Err(e) => {
                    tracing::error!("helix_emit_event failed: {}", e);
                    Ok(WasmError::HOST_FUNCTION_ERROR_CODE)
                }
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "helix_get_config_value",
        |mut caller: Caller<'_, HostState>,
         key_ptr: i32,
         key_len: i32,
         buf_ptr: i32,
         buf_len: i32|
         -> Result<i32> {
            let key = read_string_from_wasm(&mut caller, key_ptr, key_len)?;
            let config = &caller.data().agent_config.config_data;

            match config.get(&key) {
                Some(value) => {
                    let value_str = serde_json::to_string(value)
                        .map_err(|e| anyhow!("serialize config value: {}", e))?;
                    let bytes = value_str.as_bytes();
                    if bytes.len() > buf_len as usize {
                        return Ok(WasmError::BUFFER_TOO_SMALL_CODE);
                    }
                    let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
                    mem.write(&mut caller, buf_ptr as usize, bytes)
                        .map_err(|e| anyhow!(e))?;
                    Ok(bytes.len() as i32)
                }
                None => Ok(WasmError::VALUE_NOT_FOUND_CODE),
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "helix_get_state",
        |mut caller: Caller<'_, HostState>, buf_ptr: i32, buf_len: i32| -> Result<i32> {
            let state_ref = caller.data();
            let profile_id: ProfileId = state_ref.agent_config.profile_id;
            let agent_id: AgentId = state_ref.agent_config.id;
            let store = Arc::clone(&state_ref.state_store);

            let res = futures::executor::block_on(store.get_state(&profile_id, &agent_id));
            match res {
                Ok(Some(state_json)) => {
                    let state_str = serde_json::to_string(&state_json)
                        .map_err(|e| anyhow!("serialize state: {}", e))?;
                    let bytes = state_str.as_bytes();
                    if bytes.len() > buf_len as usize {
                        return Ok(WasmError::BUFFER_TOO_SMALL_CODE);
                    }
                    let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
                    mem.write(&mut caller, buf_ptr as usize, bytes)
                        .map_err(|e| anyhow!(e))?;
                    Ok(bytes.len() as i32)
                }
                Ok(None) => Ok(WasmError::VALUE_NOT_FOUND_CODE),
                Err(e) => {
                    tracing::error!("helix_get_state failed: {}", e);
                    Ok(WasmError::HOST_FUNCTION_ERROR_CODE)
                }
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "helix_set_state",
        |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| -> Result<i32> {
            let json_str = read_string_from_wasm(&mut caller, ptr, len)?;
            let state_json: JsonValue = serde_json::from_str(&json_str)
                .map_err(|e| anyhow!("deserialize state json: {}", e))?;

            let state_ref = caller.data();
            let profile_id = state_ref.agent_config.profile_id;
            let agent_id = state_ref.agent_config.id;
            let store = Arc::clone(&state_ref.state_store);

            let res =
                futures::executor::block_on(store.set_state(&profile_id, &agent_id, state_json));
            match res {
                Ok(_) => Ok(0),
                Err(e) => {
                    tracing::error!("helix_set_state failed: {}", e);
                    Ok(WasmError::HOST_FUNCTION_ERROR_CODE)
                }
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "helix_get_credential",
        |mut caller: Caller<'_, HostState>,
         id_ptr: i32,
         id_len: i32,
         buf_ptr: i32,
         buf_len: i32|
         -> Result<i32> {
            let id_str = read_string_from_wasm(&mut caller, id_ptr, id_len)?;
            let cred_id = CredentialId::parse_str(&id_str)
                .map_err(|e| anyhow!("invalid credential id: {}", e))?;

            let provider = Arc::clone(&caller.data().credential_provider);
            let res = futures::executor::block_on(provider.get_credential(&cred_id));
            match res {
                Ok(Some(cred)) => {
                    let cred_str = serde_json::to_string(&cred)
                        .map_err(|e| anyhow!("serialize credential: {}", e))?;
                    let bytes = cred_str.as_bytes();
                    if bytes.len() > buf_len as usize {
                        return Ok(WasmError::BUFFER_TOO_SMALL_CODE);
                    }
                    let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
                    mem.write(&mut caller, buf_ptr as usize, bytes)
                        .map_err(|e| anyhow!(e))?;
                    Ok(bytes.len() as i32)
                }
                Ok(None) => Ok(WasmError::VALUE_NOT_FOUND_CODE),
                Err(e) => {
                    tracing::error!("helix_get_credential failed: {}", e);
                    Ok(WasmError::HOST_FUNCTION_ERROR_CODE)
                }
            }
        },
    )?;

    linker.func_wrap("env", "helix_get_time", || -> Result<u64> {
        Ok(SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_millis() as u64)
    })?;

    linker.func_wrap("env", "helix_random", || -> Result<u32> {
        let mut hasher = DefaultHasher::new();
        SystemTime::now().hash(&mut hasher);
        Ok(hasher.finish() as u32)
    })?;

    Ok(())
}
