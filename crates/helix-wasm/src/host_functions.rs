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
//!
//! Host functions are capabilities. A function is linked only when its exact
//! name is present in [`WasmRuntimeConfig::allowed_host_functions`]. Unknown or
//! duplicate names fail configuration rather than silently broadening access.

use anyhow::{anyhow, Result};
use std::collections::{hash_map::DefaultHasher, BTreeSet};
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

pub const HOST_LOG_MESSAGE: &str = "helix_log_message";
pub const HOST_EMIT_EVENT: &str = "helix_emit_event";
pub const HOST_GET_CONFIG_VALUE: &str = "helix_get_config_value";
pub const HOST_GET_STATE: &str = "helix_get_state";
pub const HOST_SET_STATE: &str = "helix_set_state";
pub const HOST_GET_CREDENTIAL: &str = "helix_get_credential";
pub const HOST_GET_TIME: &str = "helix_get_time";
pub const HOST_RANDOM: &str = "helix_random";

const SUPPORTED_HOST_FUNCTIONS: [&str; 8] = [
    HOST_LOG_MESSAGE,
    HOST_EMIT_EVENT,
    HOST_GET_CONFIG_VALUE,
    HOST_GET_STATE,
    HOST_SET_STATE,
    HOST_GET_CREDENTIAL,
    HOST_GET_TIME,
    HOST_RANDOM,
];

/// State accessible by host functions.
pub struct HostState {
    pub agent_config: Arc<AgentConfig>,
    pub event_publisher: Arc<dyn EventPublisher>,
    pub credential_provider: Arc<dyn CredentialProvider>,
    pub state_store: Arc<dyn StateStore>,
    pub store_limits: StoreLimits,
}

fn read_string_from_wasm(caller: &mut Caller<'_, HostState>, ptr: i32, len: i32) -> Result<String> {
    if ptr < 0 || len < 0 {
        return Err(anyhow!("negative pointer/length"));
    }
    let start = usize::try_from(ptr).map_err(|_| anyhow!("pointer does not fit usize"))?;
    let length = usize::try_from(len).map_err(|_| anyhow!("length does not fit usize"))?;
    let end = start
        .checked_add(length)
        .ok_or_else(|| anyhow!("pointer/length overflow"))?;
    let mem = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
        .ok_or_else(|| anyhow!("failed to find host memory"))?;
    let data = mem
        .data(caller)
        .get(start..end)
        .ok_or_else(|| anyhow!("pointer/length out of bounds"))?;
    Ok(std::str::from_utf8(data)?.to_string())
}

/// Validate and canonicalize the configured host-function capability set.
pub fn validate_host_function_policy(
    runtime_config: &WasmRuntimeConfig,
) -> Result<BTreeSet<String>> {
    let supported = SUPPORTED_HOST_FUNCTIONS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut allowed = BTreeSet::new();
    for name in &runtime_config.allowed_host_functions {
        if name.trim() != name || name.is_empty() || !name.is_ascii() {
            return Err(anyhow!("host function name is not canonical: {name:?}"));
        }
        if !supported.contains(name.as_str()) {
            return Err(anyhow!("unsupported host function capability: {name}"));
        }
        if !allowed.insert(name.clone()) {
            return Err(anyhow!("duplicate host function capability: {name}"));
        }
    }
    Ok(allowed)
}

/// Link exactly the host functions authorized by the runtime configuration.
pub fn link_all_functions(
    linker: &mut Linker<HostState>,
    runtime_config: &WasmRuntimeConfig,
) -> Result<()> {
    let allowed = validate_host_function_policy(runtime_config)?;

    if allowed.contains(HOST_LOG_MESSAGE) {
        link_log_message(linker)?;
    }
    if allowed.contains(HOST_EMIT_EVENT) {
        link_emit_event(linker)?;
    }
    if allowed.contains(HOST_GET_CONFIG_VALUE) {
        link_get_config_value(linker)?;
    }
    if allowed.contains(HOST_GET_STATE) {
        link_get_state(linker)?;
    }
    if allowed.contains(HOST_SET_STATE) {
        link_set_state(linker)?;
    }
    if allowed.contains(HOST_GET_CREDENTIAL) {
        link_get_credential(linker)?;
    }
    if allowed.contains(HOST_GET_TIME) {
        link_get_time(linker)?;
    }
    if allowed.contains(HOST_RANDOM) {
        link_random(linker)?;
    }

    Ok(())
}

fn link_log_message(linker: &mut Linker<HostState>) -> Result<()> {
    linker.func_wrap(
        "env",
        HOST_LOG_MESSAGE,
        |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| -> Result<()> {
            let message = read_string_from_wasm(&mut caller, ptr, len)?;
            tracing::info!(agent_id = %caller.data().agent_config.id, "WASM: {}", message);
            Ok(())
        },
    )?;
    Ok(())
}

fn link_emit_event(linker: &mut Linker<HostState>) -> Result<()> {
    linker.func_wrap(
        "env",
        HOST_EMIT_EVENT,
        |mut caller: Caller<'_, HostState>,
         payload_ptr: i32,
         payload_len: i32,
         ty_ptr: i32,
         ty_len: i32|
         -> Result<i32> {
            let payload_str = read_string_from_wasm(&mut caller, payload_ptr, payload_len)?;
            let payload: JsonValue = serde_json::from_str(&payload_str)
                .map_err(|error| anyhow!("failed to parse event payload json: {error}"))?;

            let event_type = if ty_ptr != 0 && ty_len > 0 {
                Some(read_string_from_wasm(&mut caller, ty_ptr, ty_len)?)
            } else {
                None
            };

            let state = caller.data();
            let agent_id = state.agent_config.id;
            let publisher = Arc::clone(&state.event_publisher);

            match futures::executor::block_on(
                publisher.publish_event(&agent_id, payload, event_type),
            ) {
                Ok(_) => Ok(0),
                Err(error) => {
                    tracing::error!(agent_id = %agent_id, "helix_emit_event failed: {error}");
                    Ok(WasmError::HOST_FUNCTION_ERROR_CODE)
                }
            }
        },
    )?;
    Ok(())
}

fn link_get_config_value(linker: &mut Linker<HostState>) -> Result<()> {
    linker.func_wrap(
        "env",
        HOST_GET_CONFIG_VALUE,
        |mut caller: Caller<'_, HostState>,
         key_ptr: i32,
         key_len: i32,
         buf_ptr: i32,
         buf_len: i32|
         -> Result<i32> {
            if buf_ptr < 0 || buf_len < 0 {
                return Ok(WasmError::INVALID_ARGUMENT_CODE);
            }
            let key = read_string_from_wasm(&mut caller, key_ptr, key_len)?;
            let value = caller.data().agent_config.config_data.get(&key).cloned();
            let Some(value) = value else {
                return Ok(WasmError::VALUE_NOT_FOUND_CODE);
            };
            let value_str = serde_json::to_string(&value)
                .map_err(|error| anyhow!("serialize config value: {error}"))?;
            write_guest_bytes(&mut caller, buf_ptr, buf_len, value_str.as_bytes())
        },
    )?;
    Ok(())
}

fn link_get_state(linker: &mut Linker<HostState>) -> Result<()> {
    linker.func_wrap(
        "env",
        HOST_GET_STATE,
        |mut caller: Caller<'_, HostState>, buf_ptr: i32, buf_len: i32| -> Result<i32> {
            if buf_ptr < 0 || buf_len < 0 {
                return Ok(WasmError::INVALID_ARGUMENT_CODE);
            }
            let state_ref = caller.data();
            let profile_id: ProfileId = state_ref.agent_config.profile_id;
            let agent_id: AgentId = state_ref.agent_config.id;
            let store = Arc::clone(&state_ref.state_store);

            match futures::executor::block_on(store.get_state(&profile_id, &agent_id)) {
                Ok(Some(state_json)) => {
                    let state_str = serde_json::to_string(&state_json)
                        .map_err(|error| anyhow!("serialize state: {error}"))?;
                    write_guest_bytes(&mut caller, buf_ptr, buf_len, state_str.as_bytes())
                }
                Ok(None) => Ok(WasmError::VALUE_NOT_FOUND_CODE),
                Err(error) => {
                    tracing::error!(agent_id = %agent_id, "helix_get_state failed: {error}");
                    Ok(WasmError::HOST_FUNCTION_ERROR_CODE)
                }
            }
        },
    )?;
    Ok(())
}

fn link_set_state(linker: &mut Linker<HostState>) -> Result<()> {
    linker.func_wrap(
        "env",
        HOST_SET_STATE,
        |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| -> Result<i32> {
            let json_str = read_string_from_wasm(&mut caller, ptr, len)?;
            let state_json: JsonValue = serde_json::from_str(&json_str)
                .map_err(|error| anyhow!("deserialize state json: {error}"))?;

            let state_ref = caller.data();
            let profile_id = state_ref.agent_config.profile_id;
            let agent_id = state_ref.agent_config.id;
            let store = Arc::clone(&state_ref.state_store);

            match futures::executor::block_on(store.set_state(&profile_id, &agent_id, state_json)) {
                Ok(_) => Ok(0),
                Err(error) => {
                    tracing::error!(agent_id = %agent_id, "helix_set_state failed: {error}");
                    Ok(WasmError::HOST_FUNCTION_ERROR_CODE)
                }
            }
        },
    )?;
    Ok(())
}

fn link_get_credential(linker: &mut Linker<HostState>) -> Result<()> {
    linker.func_wrap(
        "env",
        HOST_GET_CREDENTIAL,
        |mut caller: Caller<'_, HostState>,
         id_ptr: i32,
         id_len: i32,
         buf_ptr: i32,
         buf_len: i32|
         -> Result<i32> {
            if buf_ptr < 0 || buf_len < 0 {
                return Ok(WasmError::INVALID_ARGUMENT_CODE);
            }
            let id_str = read_string_from_wasm(&mut caller, id_ptr, id_len)?;
            let credential_id = CredentialId::parse_str(&id_str)
                .map_err(|error| anyhow!("invalid credential id: {error}"))?;

            // Possession of the host function is not blanket credential authority.
            // The credential must also be explicitly assigned to this agent.
            if !caller
                .data()
                .agent_config
                .credential_ids
                .contains(&credential_id)
            {
                tracing::warn!(
                    agent_id = %caller.data().agent_config.id,
                    credential_id = %credential_id,
                    "WASM credential access denied"
                );
                return Ok(WasmError::VALUE_NOT_FOUND_CODE);
            }

            let provider = Arc::clone(&caller.data().credential_provider);
            match futures::executor::block_on(provider.get_credential(&credential_id)) {
                Ok(Some(credential)) => {
                    let credential_str = serde_json::to_string(&credential)
                        .map_err(|error| anyhow!("serialize credential: {error}"))?;
                    write_guest_bytes(&mut caller, buf_ptr, buf_len, credential_str.as_bytes())
                }
                Ok(None) => Ok(WasmError::VALUE_NOT_FOUND_CODE),
                Err(error) => {
                    tracing::error!(
                        agent_id = %caller.data().agent_config.id,
                        credential_id = %credential_id,
                        "helix_get_credential failed: {error}"
                    );
                    Ok(WasmError::HOST_FUNCTION_ERROR_CODE)
                }
            }
        },
    )?;
    Ok(())
}

fn link_get_time(linker: &mut Linker<HostState>) -> Result<()> {
    linker.func_wrap("env", HOST_GET_TIME, || -> Result<u64> {
        let millis = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_millis();
        u64::try_from(millis).map_err(|_| anyhow!("host time exceeds u64"))
    })?;
    Ok(())
}

fn link_random(linker: &mut Linker<HostState>) -> Result<()> {
    linker.func_wrap("env", HOST_RANDOM, || -> Result<u32> {
        // This legacy helper is not cryptographic and is intentionally unavailable
        // unless explicitly allowlisted. Security-sensitive callers need a separate
        // capability with a documented entropy contract.
        let mut hasher = DefaultHasher::new();
        SystemTime::now().hash(&mut hasher);
        Ok(hasher.finish() as u32)
    })?;
    Ok(())
}

fn write_guest_bytes(
    caller: &mut Caller<'_, HostState>,
    buf_ptr: i32,
    buf_len: i32,
    bytes: &[u8],
) -> Result<i32> {
    let start = usize::try_from(buf_ptr).map_err(|_| anyhow!("negative buffer pointer"))?;
    let capacity = usize::try_from(buf_len).map_err(|_| anyhow!("negative buffer length"))?;
    if bytes.len() > capacity {
        return Ok(WasmError::BUFFER_TOO_SMALL_CODE);
    }
    let memory = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
        .ok_or_else(|| anyhow!("failed to find host memory"))?;
    memory
        .write(caller, start, bytes)
        .map_err(|error| anyhow!(error))?;
    i32::try_from(bytes.len()).map_err(|_| anyhow!("host response exceeds i32"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_host_capabilities_are_read_only() {
        let config = WasmRuntimeConfig::default();
        let allowed = validate_host_function_policy(&config).expect("valid default policy");
        assert!(allowed.contains(HOST_LOG_MESSAGE));
        assert!(allowed.contains(HOST_GET_CONFIG_VALUE));
        assert!(allowed.contains(HOST_GET_STATE));
        assert!(!allowed.contains(HOST_EMIT_EVENT));
        assert!(!allowed.contains(HOST_SET_STATE));
        assert!(!allowed.contains(HOST_GET_CREDENTIAL));
        assert!(!allowed.contains(HOST_GET_TIME));
        assert!(!allowed.contains(HOST_RANDOM));
    }

    #[test]
    fn unknown_host_capability_fails_closed() {
        let mut config = WasmRuntimeConfig::default();
        config
            .allowed_host_functions
            .push("host_everything".to_string());
        assert!(validate_host_function_policy(&config).is_err());
    }

    #[test]
    fn duplicate_host_capability_fails_closed() {
        let mut config = WasmRuntimeConfig::default();
        config
            .allowed_host_functions
            .push(HOST_LOG_MESSAGE.to_string());
        assert!(validate_host_function_policy(&config).is_err());
    }
}
