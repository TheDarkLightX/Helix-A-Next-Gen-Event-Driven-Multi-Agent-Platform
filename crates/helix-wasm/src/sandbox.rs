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

use crate::WasmRuntimeConfig; // Assuming WasmRuntimeConfig is in crate::lib
use anyhow::Result; // For wasmtime configuration results
use std::collections::HashMap; // For allowed_env_vars

/// Configures a `wasmtime::Engine` with resource limits based on `WasmRuntimeConfig`.
///
/// This function sets up fuel consumption for instruction counting and can be
/// extended to configure other engine-level settings like memory allocation strategies.
pub fn configure_engine(config: &WasmRuntimeConfig) -> Result<wasmtime::Engine> {
    let mut engine_config = wasmtime::Config::new();
    engine_config.consume_fuel(true); // Enable fuel for instruction counting/limiting execution time
    engine_config.max_wasm_stack(config.resource_limits.max_stack_size as usize); // If needed directly on engine
    
    // TODO: Explore if wasmtime::PoolingAllocationStrategy or other memory strategies
    // are beneficial and how they interact with instance-level memory limits.
    // For now, instance-level memory limits are set on the Store/Linker.

    wasmtime::Engine::new(&engine_config)
}

/// Configures a `wasmtime::Store` with resource limits from `WasmRuntimeConfig`.
///
/// Sets fuel available for an instance and can add memory limits.
pub fn configure_store(store: &mut wasmtime::Store<super::host_functions::HostState>, config: &WasmRuntimeConfig) -> Result<()> {
    store.set_fuel(config.max_instructions)?; // max_instructions used as fuel
    
    // InstanceLimiter can limit memory, tables, instances for a Store.
    let mut limiter_builder = wasmtime::StoreLimitsBuilder::new();
    limiter_builder.memory_size(config.max_memory as usize);
    limiter_builder.tables(config.resource_limits.max_tables as usize); // Max number of tables
    limiter_builder.table_elements(config.resource_limits.max_table_size); // Max elements per table
    limiter_builder.instances(1); // Typically one instance per store for agent execution
    // Note: max_globals and max_functions are usually validated at module compilation or linking time,
    // not directly limited by StoreLimitsBuilder in this manner.
    store.limiter(move |_| limiter_builder.build()); // Using the closure form for Store::limiter

    // Note: `Store::limiter` is one way. Another is `Instance::set_memory_size_limit` if available
    // or through Linker memory type definitions. Wasmtime's exact API for memory limits
    // on instances vs. store might evolve. For now, fuel is the primary execution limit.
    // Max memory is often defined when defining the memory for the linker.
    Ok(())
}


/// Creates and configures a `wasmtime_wasi::WasiCtx` based on `WasmRuntimeConfig`.
///
/// This sets up the WASI environment, including pre-opened directories,
/// environment variables, and network access restrictions.
pub fn configure_wasi_ctx(config: &WasmRuntimeConfig) -> wasmtime_wasi::WasiCtx {
    let mut builder = wasmtime_wasi::WasiCtxBuilder::new();
    builder.inherit_stdio(); // Or configure specific stdin/stdout/stderr

    if config.enable_wasi {
        // Configure Environment Variables
        // Assumes WasmRuntimeConfig has: pub allowed_env_vars: Option<HashMap<String, String>>
        if let Some(env_vars_map) = &config.allowed_env_vars {
            let envs: Vec<(String, String)> = env_vars_map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            if !envs.is_empty() {
                if let Err(e) = builder.envs(&envs) {
                    tracing::error!("Failed to set WASI environment variables: {}", e);
                } else {
                    tracing::debug!("Set WASI environment variables: {:?}", envs.iter().map(|(k,_)| k).collect::<Vec<&String>>());
                }
            }
        }

        // Configure Pre-opened Directories
        // Assumes WasmRuntimeConfig has: pub allowed_dirs: Option<Vec<crate::AllowedDirEntry>>
        // and crate::AllowedDirEntry { host_path: PathBuf, guest_path: PathBuf, read_only: bool }
        if let Some(allowed_dirs_vec) = &config.allowed_dirs {
            for dir_entry in allowed_dirs_vec {
                match std::fs::metadata(&dir_entry.host_path) {
                    Ok(metadata) => {
                        if metadata.is_dir() {
                            match wasmtime_wasi::Dir::open_ambient_dir(&dir_entry.host_path, wasmtime_wasi::ambient_authority()) {
                                Ok(dir) => {
                                    // Note: WasiCtxBuilder::preopened_dir does not have a direct read-only flag.
                                    // Read-only enforcement relies on the host OS permissions for the opened directory.
                                    // We can log a warning if read_only is true but cannot enforce it here beyond host capabilities.
                                    if dir_entry.read_only {
                                        tracing::warn!(
                                            "Attempting to preopen host path {:?} as read-only for guest path {:?}. Actual enforcement depends on host OS permissions for the user running Helix.",
                                            dir_entry.host_path, dir_entry.guest_path
                                        );
                                    }
                                    let guest_path_str = dir_entry.guest_path.to_string_lossy().into_owned();
                                    if let Err(e) = builder.preopened_dir(dir, guest_path_str) {
                                        tracing::error!("Failed to preopen guest path for host path {:?}: {}", dir_entry.host_path, e);
                                    } else {
                                        tracing::debug!("Successfully preopened host path {:?} as guest path {:?}", dir_entry.host_path, dir_entry.guest_path);
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Failed to open host directory {:?} for WASI preopen: {}", dir_entry.host_path, e);
                                }
                            }
                        } else {
                            tracing::error!("Host path {:?} for WASI preopen is not a directory.", dir_entry.host_path);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to get metadata for host path {:?} for WASI preopen: {}", dir_entry.host_path, e);
                    }
                }
            }
        }

        // Configure Network Access
        // Assumes WasmRuntimeConfig has: pub allow_network_sockets: bool (replacing old allow_network)
        if config.allow_network_sockets {
            if let Err(e) = builder.allow_tcp(true) { tracing::warn!("Failed to allow TCP for WASI: {}", e); }
            if let Err(e) = builder.allow_udp(true) { tracing::warn!("Failed to allow UDP for WASI: {}", e); }
            tracing::debug!("Allowed TCP/UDP sockets for WASI.");
        } else {
            if let Err(e) = builder.allow_tcp(false) { tracing::warn!("Failed to disallow TCP for WASI: {}", e); }
            if let Err(e) = builder.allow_udp(false) { tracing::warn!("Failed to disallow UDP for WASI: {}", e); }
            tracing::debug!("Disallowed TCP/UDP sockets for WASI.");
        }

    } else {
        // If WASI is not enabled, don't provide any WASI imports.
        // Also, explicitly disallow network capabilities that might be on by default.
        if let Err(e) = builder.allow_tcp(false) { tracing::warn!("WASI disabled; failed to explicitly disallow TCP: {}", e); }
        if let Err(e) = builder.allow_udp(false) { tracing::warn!("WASI disabled; failed to explicitly disallow UDP: {}", e); }
        tracing::debug!("WASI disabled, explicitly disallowed TCP/UDP sockets.");
    }
    builder.build()
}

// The old SandboxConfig and Sandbox structs are removed as their responsibilities
// are now handled by WasmRuntimeConfig and wasmtime's built-in mechanisms.
