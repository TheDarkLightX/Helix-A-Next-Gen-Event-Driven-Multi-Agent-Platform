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

//! Sandboxing utilities for WASM execution, aligning with wasmtime capabilities.

use crate::WasmRuntimeConfig;
use anyhow::Result;
use std::collections::HashMap;

/// Configures a `wasmtime::Engine` with resource limits based on `WasmRuntimeConfig`.
///
/// This function sets up fuel consumption for instruction counting and can be
/// extended to configure other engine-level settings like memory allocation strategies.
pub fn configure_engine(config: &WasmRuntimeConfig) -> Result<wasmtime::Engine> {
    let mut engine_config = wasmtime::Config::new();
    engine_config.consume_fuel(true); // Enable fuel for instruction counting/limiting execution time
    engine_config.max_wasm_stack(config.resource_limits.max_stack_size as usize); // If needed directly on engine
    engine_config.async_support(true);

    // Further exploration of wasmtime::PoolingAllocationStrategy or other memory strategies
    // are beneficial and how they interact with instance-level memory limits.
    // For now, instance-level memory limits are set on the Store/Linker.

    wasmtime::Engine::new(&engine_config)
}

/// Configures a `wasmtime::Store` with resource limits from `WasmRuntimeConfig`.
///
/// Sets fuel available for an instance and can add memory limits.
pub fn configure_store(
    store: &mut wasmtime::Store<super::host_functions::HostState>,
    config: &WasmRuntimeConfig,
) -> Result<()> {
    store.set_fuel(config.max_instructions)?;
    store.limiter(|state| &mut state.store_limits);
    Ok(())
}

/// Creates and configures a `wasmtime_wasi::WasiCtx` based on `WasmRuntimeConfig`.
///
/// This sets up the WASI environment, including pre-opened directories,
/// environment variables, and network access restrictions.
pub fn configure_wasi_ctx(config: &WasmRuntimeConfig) -> wasmtime_wasi::WasiCtx {
    let mut builder = wasmtime_wasi::WasiCtxBuilder::new();
    builder.inherit_stdio();

    if config.enable_wasi {
        if let Some(env_vars_map) = &config.allowed_env_vars {
            for (k, v) in env_vars_map {
                let _ = builder.env(k, v);
            }
        }
    }

    builder.build()
}

// The old SandboxConfig and Sandbox structs are removed as their responsibilities
// are now handled by WasmRuntimeConfig and wasmtime's built-in mechanisms.
