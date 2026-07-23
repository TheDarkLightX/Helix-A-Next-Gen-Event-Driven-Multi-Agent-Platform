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

//! Sandboxing utilities for WASM execution, aligning with Wasmtime capabilities.

use crate::WasmRuntimeConfig;
use anyhow::{anyhow, Result};

/// Validate the runtime configuration before constructing an engine or linker.
///
/// Configuration fields that are not yet enforced fail closed rather than being
/// accepted as misleading documentation-only controls.
pub fn validate_runtime_config(config: &WasmRuntimeConfig) -> Result<()> {
    if config.max_memory == 0 {
        return Err(anyhow!("max_memory must be greater than zero"));
    }
    if config.max_execution_time_ms == 0 {
        return Err(anyhow!("max_execution_time_ms must be greater than zero"));
    }
    if config.max_instructions == 0 {
        return Err(anyhow!("max_instructions must be greater than zero"));
    }
    if config.resource_limits.max_stack_size == 0 {
        return Err(anyhow!("max_stack_size must be greater than zero"));
    }
    if config.resource_limits.max_tables == 0 {
        return Err(anyhow!("max_tables must be greater than zero"));
    }
    if config.resource_limits.max_table_size == 0 {
        return Err(anyhow!("max_table_size must be greater than zero"));
    }

    crate::host_functions::validate_host_function_policy(config)?;

    if config.enable_wasi {
        return Err(anyhow!(
            "WASI is not yet policy-bound; enable_wasi must remain false"
        ));
    }
    if config
        .allowed_dirs
        .as_ref()
        .is_some_and(|directories| !directories.is_empty())
    {
        return Err(anyhow!(
            "allowed_dirs are not yet enforced and must remain empty"
        ));
    }
    if config
        .allowed_env_vars
        .as_ref()
        .is_some_and(|variables| !variables.is_empty())
    {
        return Err(anyhow!(
            "allowed_env_vars are not yet enforced and must remain empty"
        ));
    }
    if config.allow_network_sockets {
        return Err(anyhow!(
            "WASI network sockets are not implemented and must remain disabled"
        ));
    }

    Ok(())
}

/// Configure a [`wasmtime::Engine`] with fuel and epoch interruption.
///
/// Fuel gives deterministic instruction bounds. Epoch interruption gives an
/// independent host wall-clock kill switch that guest code cannot opt out of.
pub fn configure_engine(config: &WasmRuntimeConfig) -> Result<wasmtime::Engine> {
    validate_runtime_config(config)?;
    let mut engine_config = wasmtime::Config::new();
    engine_config.consume_fuel(true);
    engine_config.epoch_interruption(true);
    engine_config.max_wasm_stack(config.resource_limits.max_stack_size as usize);
    engine_config.async_support(true);
    wasmtime::Engine::new(&engine_config)
}

/// Configure a [`wasmtime::Store`] with fuel, epoch, and allocation limits.
pub fn configure_store(
    store: &mut wasmtime::Store<super::host_functions::HostState>,
    config: &WasmRuntimeConfig,
) -> Result<()> {
    store.set_fuel(config.max_instructions)?;
    store.epoch_deadline_trap();
    // Set a future deadline before any guest call. The runtime resets this to
    // one tick before every invocation and increments the engine epoch at the
    // configured wall-clock deadline.
    store.set_epoch_deadline(1);
    store.limiter(|state| &mut state.store_limits);
    Ok(())
}

/// Construct a minimal WASI context.
///
/// The current runtime deliberately rejects `enable_wasi=true`; this helper is
/// retained for the future policy-bound WASI adapter and does not inherit host
/// stdio, directories, environment variables, or sockets.
pub fn configure_wasi_ctx(_config: &WasmRuntimeConfig) -> wasmtime_wasi::WasiCtx {
    wasmtime_wasi::WasiCtxBuilder::new().build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_runtime_configuration_is_enforceable() {
        validate_runtime_config(&WasmRuntimeConfig::default()).expect("valid default");
    }

    #[test]
    fn unsupported_wasi_configuration_fails_closed() {
        let mut config = WasmRuntimeConfig::default();
        config.enable_wasi = true;
        assert!(validate_runtime_config(&config).is_err());
    }

    #[test]
    fn unsupported_network_configuration_fails_closed() {
        let mut config = WasmRuntimeConfig::default();
        config.allow_network_sockets = true;
        assert!(validate_runtime_config(&config).is_err());
    }

    #[test]
    fn zero_wall_clock_budget_fails_closed() {
        let mut config = WasmRuntimeConfig::default();
        config.max_execution_time_ms = 0;
        assert!(validate_runtime_config(&config).is_err());
    }
}
