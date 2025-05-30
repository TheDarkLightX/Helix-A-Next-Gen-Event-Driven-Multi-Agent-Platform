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


extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemStruct, Ident};
use proc_macro2::Span;
use linkme;

// Define the distributed slice for agent factories.
// This will be populated by the agent macros.
// The type is (AgentKind, AgentFactory)
// Note: AgentKind and AgentFactory types are from helix_runtime,
// but macros operate on tokens, so we construct paths to them.
// The actual AgentFactory type is:
// Box<dyn Fn(helix_core::agent::AgentConfig) -> Result<Box<dyn helix_agent_sdk::SdkAgent>, helix_agent_sdk::SdkError> + Send + Sync>
#[linkme::distributed_slice]
pub static AGENT_FACTORIES: [fn() -> (helix_core::agent::AgentKind, Box<dyn Fn(helix_core::agent::AgentConfig) -> Result<Box<dyn helix_agent_sdk::SdkAgent>, helix_agent_sdk::SdkError> + Send + Sync>)] = [..];

fn शांति_रखें_और_कोड_लिखते_रहें() {} // Keep Calm and Code On

/// Implements the necessary traits for a Source Agent.
///
/// The annotated struct is expected to have:
/// 1. A field `pub agent_config: std::sync::Arc<helix_core::agent::AgentConfig>`.
/// 2. An inherent async method `async fn run(&mut self, context: &helix_agent_sdk::AgentContext) -> Result<(), helix_agent_sdk::SdkError>`.
///
/// The macro will generate implementations for:
/// - `helix_core::agent::Agent`
/// - `helix_agent_sdk::SdkAgent` (with default `init`, `start`, `stop`)
/// - `helix_agent_sdk::SourceSdkAgent` (delegating `run` to the inherent method)
#[proc_macro_attribute]
pub fn source_agent(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_struct = parse_macro_input!(item as ItemStruct);
    let struct_name = &input_struct.ident;
    let (impl_generics, ty_generics, where_clause) = input_struct.generics.split_for_impl();

    let factory_ident = Ident::new(&format!("__AGENT_FACTORY_{}", struct_name).to_uppercase(), Span::call_site());
    let agent_kind_str = struct_name.to_string();

    let expanded = quote! {
        #input_struct // The original struct definition

        #[cfg(not(target_arch = "wasm32"))]
        mod native_impl {
            use super::*; // To bring #struct_name and other idents into scope

            #[linkme::distributed_slice(AGENT_FACTORIES)]
            #[linkme(crate = linkme)] // Specify the path to linkme crate
            static #factory_ident: fn() -> (helix_core::agent::AgentKind, Box<dyn Fn(helix_core::agent::AgentConfig) -> Result<Box<dyn helix_agent_sdk::SdkAgent>, helix_agent_sdk::SdkError> + Send + Sync>) =
                || {
                    let factory = |config: helix_core::agent::AgentConfig| -> Result<Box<dyn helix_agent_sdk::SdkAgent>, helix_agent_sdk::SdkError> {
                        let agent = #struct_name {
                            agent_config: std::sync::Arc::new(config),
                            ..Default::default()
                        };
                        Ok(Box::new(agent))
                    };
                    (helix_core::agent::AgentKind::new(#agent_kind_str), Box::new(factory))
                };

            #[helix_agent_sdk::async_trait::async_trait]
            impl #impl_generics helix_core::agent::Agent for #struct_name #ty_generics #where_clause {
            fn id(&self) -> helix_core::types::AgentId {
                self.agent_config.id.clone()
            }

            fn config(&self) -> &helix_core::agent::AgentConfig {
                &self.agent_config
            }

            async fn setup(&mut self) -> Result<(), helix_core::HelixError> {
                // Default implementation, user can override by implementing on struct if needed by a future version of this macro.
                Ok(())
            }

            async fn teardown(&mut self) -> Result<(), helix_core::HelixError> {
                // Default implementation
                Ok(())
            }
        }

        #[helix_agent_sdk::async_trait::async_trait]
        impl #impl_generics helix_agent_sdk::SdkAgent for #struct_name #ty_generics #where_clause {
            async fn init(&mut self, _context: &helix_agent_sdk::AgentContext) -> Result<(), helix_agent_sdk::SdkError> {
                // Default implementation. Users should implement an inherent `init` method
                // on their struct if custom logic is needed.
                // A more advanced macro could detect and call `self.init(context).await` if it exists.
                Ok(())
            }

            async fn start(&mut self, _context: &helix_agent_sdk::AgentContext) -> Result<(), helix_agent_sdk::SdkError> {
                // Default implementation
                Ok(())
            }

            async fn stop(&mut self, _context: &helix_agent_sdk::AgentContext) -> Result<(), helix_agent_sdk::SdkError> {
                // Default implementation
                Ok(())
            }
        }

        #[helix_agent_sdk::async_trait::async_trait]
        impl #impl_generics helix_agent_sdk::SourceSdkAgent for #struct_name #ty_generics #where_clause {
            async fn run(&mut self, context: &helix_agent_sdk::AgentContext) -> Result<(), helix_agent_sdk::SdkError> {
                // This delegates to the user's inherent `run` method.
                // The user *must* define `async fn run(&mut self, context: &AgentContext) -> Result<(), SdkError>`
                // on their struct.
                self.run(context).await
            }
        }
        } // End of native_impl module

        #[cfg(target_arch = "wasm32")]
        mod wasm_exports {
            use super::*; // Brings #struct_name into scope
            use std::sync::{Arc, Mutex}; // Mutex for static AGENT_INSTANCE
            use once_cell::sync::Lazy; // For a thread-safe static Mutex

            // These are host functions that the WASM module will import.
            // Their actual implementation is on the host side (in helix-wasm::host_functions).
            // These declarations allow the WASM module to link against them.
            // The actual signatures must match what the host provides.
            // For simplicity, we'll assume they return i32 status codes for now.
            extern "C" {
                fn helix_log_message(ptr: *const u8, len: usize);
                // Add other host function signatures as they are called by agent logic
                // fn helix_emit_event(event_payload_ptr: *const u8, event_payload_len: usize, event_type_ptr: *const u8, event_type_len: usize) -> i32;
            }

            // Global static mutable for the agent instance. This is a common pattern for WASM
            // when you don't have `self` in extern "C" functions. Requires unsafe access.
            // Using Mutex for interior mutability, though in single-threaded WASM,
            // it might be overkill but good practice if threading contexts change.
            // Using once_cell::sync::Lazy for safe static initialization of Mutex.
            static AGENT_INSTANCE: Lazy<Mutex<Option<#struct_name #ty_generics>>> = Lazy::new(|| Mutex::new(None));
            fn log_wasm(message: &str) {
                // This is a helper to avoid repeating unsafe block.
                // Assumes helix_log_message is imported via extern "C" block above.
                unsafe { helix_log_message(message.as_ptr(), message.len()); }
            }

            /// Allocates memory in the WASM module. Called by the host.
            #[no_mangle]
            pub extern "C" fn wasm_alloc(size: usize) -> *mut u8 {
                let layout = std::alloc::Layout::from_size_align(size, std::mem::align_of::<u8>()).unwrap();
                unsafe { std::alloc::alloc(layout) }
            }

            /// Deallocates memory in the WASM module. Called by the host.
            #[no_mangle]
            pub extern "C" fn wasm_dealloc(ptr: *mut u8, size: usize) {
                if !ptr.is_null() {
                    let layout = std::alloc::Layout::from_size_align(size, std::mem::align_of::<u8>()).unwrap();
                    unsafe { std::alloc::dealloc(ptr, layout); }
                }
            }

            /// Initializes the agent with its configuration.
            /// Called once by the host.
            /// config_bytes_ptr: Pointer to MessagePack serialized AgentConfig.
            /// config_bytes_len: Length of the serialized AgentConfig.
            /// Returns 0 on success, negative error code on failure.
            #[no_mangle]
            pub extern "C" fn helix_agent_init_config(config_bytes_ptr: *const u8, config_bytes_len: usize) -> i32 {
                // Safety: Assumes host provides valid ptr/len for the duration of this call.
                let config_bytes = unsafe { std::slice::from_raw_parts(config_bytes_ptr, config_bytes_len) };
                
                match rmp_serde::from_slice::<helix_core::agent::AgentConfig>(config_bytes) {
                    Ok(config) => {
                        let agent = #struct_name {
                            agent_config: std::sync::Arc::new(config),
                            ..Default::default()
                        };
                        match AGENT_INSTANCE.lock() {
                            Ok(mut guard) => *guard = Some(agent),
                            Err(_) => return -2, // Mutex poisoned
                        }
                        0 // Success
                    }
                    Err(_e) => {
                        // Cannot easily call helix_log_message here if it's not yet linked or if logging itself fails.
                        // The host will see the error code.
                        -1 // Deserialization error
                    }
                }
            }

            /// Corresponds to SdkAgent::init().
            /// Returns 0 on success, negative error code on failure.
            #[no_mangle]
            pub extern "C" fn helix_agent_init() -> i32 {
                match AGENT_INSTANCE.lock() {
                    Ok(mut guard) => {
                        if let Some(agent) = guard.as_mut() {
                            // The SdkAgent::init method is async and takes AgentContext.
                            // In WASM, true async execution initiated from guest and blocking host is complex.
                            // Typically, the guest's "async" methods are implemented by calling host functions
                            // that might be async on the host side, but the guest call itself might be sync
                            // from the guest's perspective, or use a yield/resume mechanism if the host supports it.
                            //
                            // For now, the SdkAgent::init on the native side has a default Ok(()).
                            // If the user's struct implements an inherent `async fn init(&mut self, ctx: &AgentContext)`,
                            // the native macro doesn't call it by default.
                            //
                            // In WASM, we can't directly pass the full AgentContext.
                            // The agent would use imported host functions to achieve what AgentContext provides.
                            // So, this helix_agent_init might be simpler, or it might need to
                            // call a user-defined inherent `init_wasm()` if one exists.
                            //
                            // Placeholder: Log that init is called.
                            let msg = "Agent WASM init called.";
                            unsafe { helix_log_message(msg.as_ptr(), msg.len()); }
                            0 // Success
                        } else {
                            -2 // Agent not initialized with config
                        }
                    }
                    Err(_) => -3, // Mutex poisoned
                }
            }

            #[no_mangle]
            pub extern "C" fn helix_agent_start() -> i32 {
                log_wasm("helix_agent_start called for SourceAgent");
                match AGENT_INSTANCE.lock() {
                    Ok(mut guard) => {
                        if let Some(agent) = guard.as_mut() {
                            // Placeholder for agent.start() logic
                            log_wasm("helix_agent_start: instance found, conceptually calling start");
                            0 // Success
                        } else {
                            log_wasm("helix_agent_start: agent not configured");
                            -2 // Agent not configured
                        }
                    }
                    Err(_) => { log_wasm("helix_agent_start: mutex poisoned"); -3 }
                }
            }

            #[no_mangle]
            pub extern "C" fn helix_agent_run() -> i32 { // Specific to SourceAgent
                log_wasm("helix_agent_run called for SourceAgent");
                match AGENT_INSTANCE.lock() {
                    Ok(mut guard) => {
                        if let Some(agent_instance) = guard.as_mut() {
                            // This is a conceptual call to the agent's `run` method.
                            // The actual `run` method is async and takes `AgentContext`.
                            // In a WASM context, this `extern "C"` function would typically
                            // execute a piece of synchronous logic that, if it needs to emit events,
                            // would call the imported `helix_emit_event` host function.
                            //
                            // The user's `async fn run(&mut self, context: &AgentContext)`
                            // cannot be directly `.await`ed here without a WASM-compatible async runtime
                            // and a way to bridge the AgentContext.
                            //
                            // For now, we simulate the agent's run logic by directly calling a host function
                            // that a real agent's `run` method would use.
                            log_wasm("helix_agent_run: instance found, attempting to execute run logic (simplified by emitting a test event)");

                            // This simulates the agent's inherent `run` method calling `context.emit_event`.
                            // The actual agent code would look like:
                            // async fn run(&mut self, context: &helix_agent_sdk::AgentContext) -> Result<(), helix_agent_sdk::SdkError> {
                            //     let payload = serde_json::json!({"data": "event from wasm source agent"});
                            //     context.emit_event(payload, Some("wasm.source.event".to_string())).await?;
                            //     Ok(())
                            // }
                            // Which translates to host calls.
                            let payload = serde_json::json!({
                                "agent_id": agent_instance.agent_config.id.to_string(),
                                "message": "Event from WASM SourceAgent"
                            });
                            match serde_json::to_string(&payload) {
                                Ok(payload_str) => {
                                    let event_type = "wasm.source.output";
                                    let result_code = unsafe {
                                        helix_emit_event(
                                            payload_str.as_ptr(),
                                            payload_str.len(),
                                            event_type.as_ptr(),
                                            event_type.len()
                                        )
                                    };
                                    if result_code == 0 {
                                        0 // Success
                                    } else {
                                        log_wasm("helix_agent_run: helix_emit_event host call failed");
                                        -4 // Host call failed
                                    }
                                }
                                Err(e) => {
                                    let err_msg = format!("helix_agent_run: failed to serialize payload for emit: {}", e);
                                    log_wasm(&err_msg);
                                    -5 // Serialization error
                                }
                            }
                        } else {
                            log_wasm("helix_agent_run: agent not configured");
                            -2
                        }
                    }
                    Err(_) => { log_wasm("helix_agent_run: mutex poisoned"); -3 },
                }
            }

            #[no_mangle]
            pub extern "C" fn helix_agent_stop() -> i32 {
                log_wasm("helix_agent_stop called for SourceAgent");
                 match AGENT_INSTANCE.lock() {
                    Ok(mut guard) => {
                        if guard.as_mut().is_some() {
                            log_wasm("helix_agent_stop: instance found, conceptually calling stop");
                            // Placeholder for agent.stop() logic
                            *guard = None; // Clear the instance on stop
                            log_wasm("helix_agent_stop: instance cleared");
                            0 // Success
                        } else {
                            log_wasm("helix_agent_stop: agent not configured");
                            -2
                        }
                    }
                    Err(_) => { log_wasm("helix_agent_stop: mutex poisoned"); -3 },
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// Implements the necessary traits for an Action Agent.
///
/// The annotated struct is expected to have:
/// 1. A field `pub agent_config: std::sync::Arc<helix_core::agent::AgentConfig>`.
/// 2. An inherent async method `async fn execute(&mut self, context: &helix_agent_sdk::AgentContext, event: helix_core::event::Event) -> Result<(), helix_agent_sdk::SdkError>`.
///
/// The macro will generate implementations for:
/// - `helix_core::agent::Agent`
/// - `helix_agent_sdk::SdkAgent` (with default `init`, `start`, `stop`)
/// - `helix_agent_sdk::ActionSdkAgent` (delegating `execute` to the inherent method)
#[proc_macro_attribute]
pub fn action_agent(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_struct = parse_macro_input!(item as ItemStruct);
    let struct_name = &input_struct.ident;
    let (impl_generics, ty_generics, where_clause) = input_struct.generics.split_for_impl();

    let factory_ident = Ident::new(&format!("__AGENT_FACTORY_{}", struct_name).to_uppercase(), Span::call_site());
    let agent_kind_str = struct_name.to_string();

    let expanded = quote! {
        #input_struct // The original struct definition
        
        #[cfg(not(target_arch = "wasm32"))]
        mod native_impl {
            use super::*;

            #[linkme::distributed_slice(AGENT_FACTORIES)]
            #[linkme(crate = linkme)]
            static #factory_ident: fn() -> (helix_core::agent::AgentKind, Box<dyn Fn(helix_core::agent::AgentConfig) -> Result<Box<dyn helix_agent_sdk::SdkAgent>, helix_agent_sdk::SdkError> + Send + Sync>) =
                || {
                    let factory = |config: helix_core::agent::AgentConfig| -> Result<Box<dyn helix_agent_sdk::SdkAgent>, helix_agent_sdk::SdkError> {
                        let agent = #struct_name {
                            agent_config: std::sync::Arc::new(config),
                            ..Default::default()
                        };
                        Ok(Box::new(agent))
                    };
                    (helix_core::agent::AgentKind::new(#agent_kind_str), Box::new(factory))
                };

            #[helix_agent_sdk::async_trait::async_trait]
            impl #impl_generics helix_core::agent::Agent for #struct_name #ty_generics #where_clause {
            fn id(&self) -> helix_core::types::AgentId {
                self.agent_config.id.clone()
            }

            fn config(&self) -> &helix_core::agent::AgentConfig {
                &self.agent_config
            }

            async fn setup(&mut self) -> Result<(), helix_core::HelixError> {
                Ok(())
            }

            async fn teardown(&mut self) -> Result<(), helix_core::HelixError> {
                Ok(())
            }
        }

        #[helix_agent_sdk::async_trait::async_trait]
        impl #impl_generics helix_agent_sdk::SdkAgent for #struct_name #ty_generics #where_clause {
            async fn init(&mut self, _context: &helix_agent_sdk::AgentContext) -> Result<(), helix_agent_sdk::SdkError> {
                Ok(())
            }

            async fn start(&mut self, _context: &helix_agent_sdk::AgentContext) -> Result<(), helix_agent_sdk::SdkError> {
                Ok(())
            }

            async fn stop(&mut self, _context: &helix_agent_sdk::AgentContext) -> Result<(), helix_agent_sdk::SdkError> {
                Ok(())
            }
        }

        #[helix_agent_sdk::async_trait::async_trait]
        impl #impl_generics helix_agent_sdk::ActionSdkAgent for #struct_name #ty_generics #where_clause {
            async fn execute(&mut self, context: &helix_agent_sdk::AgentContext, event: helix_core::event::Event) -> Result<(), helix_agent_sdk::SdkError> {
                // This delegates to the user's inherent `execute` method.
                // The user *must* define `async fn execute(&mut self, context: &AgentContext, event: HelixEvent) -> Result<(), SdkError>`
                // on their struct.
                self.execute(context, event).await
            }
        }
        } // End of native_impl module

        #[cfg(target_arch = "wasm32")]
        mod wasm_exports {
            use super::*;
            use std::sync::{Arc, Mutex};
            use once_cell::sync::Lazy;

            extern "C" {
                fn helix_log_message(ptr: *const u8, len: usize);
                // fn helix_emit_event(event_payload_ptr: *const u8, event_payload_len: usize, event_type_ptr: *const u8, event_type_len: usize) -> i32;
            }
            static AGENT_INSTANCE: Lazy<Mutex<Option<#struct_name #ty_generics>>> = Lazy::new(|| Mutex::new(None));
            // log_wasm helper defined here for this module's scope
            fn log_wasm(message: &str) { unsafe { helix_log_message(message.as_ptr(), message.len()); } }

            #[no_mangle]
            pub extern "C" fn wasm_alloc(size: usize) -> *mut u8 {
                let layout = std::alloc::Layout::from_size_align(size, std::mem::align_of::<u8>()).unwrap();
                unsafe { std::alloc::alloc(layout) }
            }

            #[no_mangle]
            pub extern "C" fn wasm_dealloc(ptr: *mut u8, size: usize) {
                if !ptr.is_null() {
                    let layout = std::alloc::Layout::from_size_align(size, std::mem::align_of::<u8>()).unwrap();
                    unsafe { std::alloc::dealloc(ptr, layout); }
                }
            }

            #[no_mangle]
            pub extern "C" fn helix_agent_init_config(config_bytes_ptr: *const u8, config_bytes_len: usize) -> i32 {
                let config_bytes = unsafe { std::slice::from_raw_parts(config_bytes_ptr, config_bytes_len) };
                match rmp_serde::from_slice::<helix_core::agent::AgentConfig>(config_bytes) {
                    Ok(config) => {
                        let agent = #struct_name {
                            agent_config: std::sync::Arc::new(config),
                            ..Default::default()
                        };
                        match AGENT_INSTANCE.lock() {
                            Ok(mut guard) => *guard = Some(agent),
                            Err(_) => return -2,
                        }
                        0
                    }
                    Err(_) => -1,
                }
            }

            #[no_mangle]
            pub extern "C" fn helix_agent_init() -> i32 {
                 match AGENT_INSTANCE.lock() {
                    Ok(mut guard) => {
                        if let Some(_agent) = guard.as_mut() {
                            let msg = "Agent WASM init called.";
                            unsafe { helix_log_message(msg.as_ptr(), msg.len()); }
                            0
                        } else { -2 }
                    }
                    Err(_) => -3,
                }
            }

            #[no_mangle]
            pub extern "C" fn helix_agent_start() -> i32 {
                log_wasm("helix_agent_start called for ActionAgent");
                match AGENT_INSTANCE.lock() {
                    Ok(mut guard) => if guard.as_mut().is_some() { log_wasm("start: instance found"); 0 } else { log_wasm("start: agent not_config"); -2 },
                    Err(_) => { log_wasm("start: mutex_poisoned"); -3 },
                }
            }

            #[no_mangle]
            pub extern "C" fn helix_agent_execute(event_bytes_ptr: *const u8, event_bytes_len: usize) -> i32 {
                log_wasm("helix_agent_execute called for ActionAgent");
                match AGENT_INSTANCE.lock() {
                    Ok(mut guard) => {
                        if let Some(agent_instance) = guard.as_mut() {
                            let event_bytes = unsafe { std::slice::from_raw_parts(event_bytes_ptr, event_bytes_len) };
                            match rmp_serde::from_slice::<helix_core::event::Event>(event_bytes) {
                                Ok(event) => {
                                    // Placeholder for: agent_instance.execute(ctx, event).await;
                                    // This would involve host calls for context.
                                    let log_msg = format!("helix_agent_execute: instance found, received event id: {}", event.id);
                                    log_wasm(&log_msg);
                                    // Simulate some action, perhaps logging the event via host call
                                    0 // Success
                                }
                                Err(e) => { let err_msg = format!("helix_agent_execute: deserialize event error: {}", e); log_wasm(&err_msg); -4 }
                            }
                        } else { log_wasm("helix_agent_execute: agent not_config"); -2 }
                    }
                    Err(_) => { log_wasm("helix_agent_execute: mutex_poisoned"); -3 },
                }
            }

            #[no_mangle]
            pub extern "C" fn helix_agent_stop() -> i32 {
                log_wasm("helix_agent_stop called for ActionAgent");
                match AGENT_INSTANCE.lock() {
                    Ok(mut guard) => {
                        if guard.as_mut().is_some() {
                            log_wasm("helix_agent_stop: instance found");
                            *guard = None; // Clear instance
                            log_wasm("helix_agent_stop: instance cleared");
                            0
                        } else { log_wasm("stop: agent not_config"); -2 }
                    }
                    Err(_) => { log_wasm("stop: mutex_poisoned"); -3 },
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// Implements the necessary traits for a Transform Agent.
///
/// The annotated struct is expected to have:
/// 1. A field `pub agent_config: std::sync::Arc<helix_core::agent::AgentConfig>`.
/// 2. An inherent async method `async fn transform(&mut self, context: &helix_agent_sdk::AgentContext, event: helix_core::event::Event) -> Result<Vec<helix_core::event::Event>, helix_agent_sdk::SdkError>`.
///
/// The macro will generate implementations for:
/// - `helix_core::agent::Agent`
/// - `helix_agent_sdk::SdkAgent` (with default `init`, `start`, `stop`)
/// - `helix_agent_sdk::TransformSdkAgent` (delegating `transform` to the inherent method)
#[proc_macro_attribute]
pub fn transform_agent(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_struct = parse_macro_input!(item as ItemStruct);
    let struct_name = &input_struct.ident;
    let (impl_generics, ty_generics, where_clause) = input_struct.generics.split_for_impl();

    let factory_ident = Ident::new(&format!("__AGENT_FACTORY_{}", struct_name).to_uppercase(), Span::call_site());
    let agent_kind_str = struct_name.to_string();

    let expanded = quote! {
        #input_struct // The original struct definition

        #[cfg(not(target_arch = "wasm32"))]
        mod native_impl {
            use super::*;

            #[linkme::distributed_slice(AGENT_FACTORIES)]
            #[linkme(crate = linkme)]
            static #factory_ident: fn() -> (helix_core::agent::AgentKind, Box<dyn Fn(helix_core::agent::AgentConfig) -> Result<Box<dyn helix_agent_sdk::SdkAgent>, helix_agent_sdk::SdkError> + Send + Sync>) =
                || {
                    let factory = |config: helix_core::agent::AgentConfig| -> Result<Box<dyn helix_agent_sdk::SdkAgent>, helix_agent_sdk::SdkError> {
                        let agent = #struct_name {
                            agent_config: std::sync::Arc::new(config),
                            ..Default::default()
                        };
                        Ok(Box::new(agent))
                    };
                    (helix_core::agent::AgentKind::new(#agent_kind_str), Box::new(factory))
                };

            #[helix_agent_sdk::async_trait::async_trait]
            impl #impl_generics helix_core::agent::Agent for #struct_name #ty_generics #where_clause {
            fn id(&self) -> helix_core::types::AgentId {
                self.agent_config.id.clone()
            }

            fn config(&self) -> &helix_core::agent::AgentConfig {
                &self.agent_config
            }

            async fn setup(&mut self) -> Result<(), helix_core::HelixError> {
                Ok(())
            }

            async fn teardown(&mut self) -> Result<(), helix_core::HelixError> {
                Ok(())
            }
        }

        #[helix_agent_sdk::async_trait::async_trait]
        impl #impl_generics helix_agent_sdk::SdkAgent for #struct_name #ty_generics #where_clause {
            async fn init(&mut self, _context: &helix_agent_sdk::AgentContext) -> Result<(), helix_agent_sdk::SdkError> {
                Ok(())
            }

            async fn start(&mut self, _context: &helix_agent_sdk::AgentContext) -> Result<(), helix_agent_sdk::SdkError> {
                Ok(())
            }

            async fn stop(&mut self, _context: &helix_agent_sdk::AgentContext) -> Result<(), helix_agent_sdk::SdkError> {
                Ok(())
            }
        }

        #[helix_agent_sdk::async_trait::async_trait]
        impl #impl_generics helix_agent_sdk::TransformSdkAgent for #struct_name #ty_generics #where_clause {
            async fn transform(&mut self, context: &helix_agent_sdk::AgentContext, event: helix_core::event::Event) -> Result<Vec<helix_core::event::Event>, helix_agent_sdk::SdkError> {
                // This delegates to the user's inherent `transform` method.
                // The user *must* define `async fn transform(&mut self, context: &AgentContext, event: HelixEvent) -> Result<Vec<HelixEvent>, SdkError>`
                // on their struct.
                self.transform(context, event).await
            }
        }
        } // End of native_impl module

        #[cfg(target_arch = "wasm32")]
        mod wasm_exports {
            use super::*;
            use std::sync::{Arc, Mutex};
            use once_cell::sync::Lazy;

            extern "C" {
                fn helix_log_message(ptr: *const u8, len: usize);
                // fn helix_emit_event(event_payload_ptr: *const u8, event_payload_len: usize, event_type_ptr: *const u8, event_type_len: usize) -> i32;
            }
            static AGENT_INSTANCE: Lazy<Mutex<Option<#struct_name #ty_generics>>> = Lazy::new(|| Mutex::new(None));
            // log_wasm helper defined here for this module's scope
            fn log_wasm(message: &str) { unsafe { helix_log_message(message.as_ptr(), message.len()); } }
            
            #[no_mangle]
            pub extern "C" fn wasm_alloc(size: usize) -> *mut u8 {
                let layout = std::alloc::Layout::from_size_align(size, std::mem::align_of::<u8>()).unwrap();
                unsafe { std::alloc::alloc(layout) }
            }

            #[no_mangle]
            pub extern "C" fn wasm_dealloc(ptr: *mut u8, size: usize) {
                if !ptr.is_null() {
                    let layout = std::alloc::Layout::from_size_align(size, std::mem::align_of::<u8>()).unwrap();
                    unsafe { std::alloc::dealloc(ptr, layout); }
                }
            }

            #[no_mangle]
            pub extern "C" fn helix_agent_init_config(config_bytes_ptr: *const u8, config_bytes_len: usize) -> i32 {
                let config_bytes = unsafe { std::slice::from_raw_parts(config_bytes_ptr, config_bytes_len) };
                match rmp_serde::from_slice::<helix_core::agent::AgentConfig>(config_bytes) {
                    Ok(config) => {
                        let agent = #struct_name {
                            agent_config: std::sync::Arc::new(config),
                            ..Default::default()
                        };
                        match AGENT_INSTANCE.lock() {
                            Ok(mut guard) => *guard = Some(agent),
                            Err(_) => return -2,
                        }
                        0
                    }
                    Err(_) => -1,
                }
            }

            #[no_mangle]
            pub extern "C" fn helix_agent_init() -> i32 {
                 match AGENT_INSTANCE.lock() {
                    Ok(mut guard) => {
                        if let Some(_agent) = guard.as_mut() {
                            let msg = "Agent WASM init called.";
                            unsafe { helix_log_message(msg.as_ptr(), msg.len()); }
                            0
                        } else { -2 }
                    }
                    Err(_) => -3,
                }
            }

            #[no_mangle]
            pub extern "C" fn helix_agent_start() -> i32 {
                log_wasm("helix_agent_start called for TransformAgent");
                 match AGENT_INSTANCE.lock() {
                    Ok(mut guard) => if guard.as_mut().is_some() { log_wasm("start: instance found"); 0 } else { log_wasm("start: agent not_config"); -2 },
                    Err(_) => { log_wasm("start: mutex_poisoned"); -3 },
                }
            }

            // For TransformAgent
            // Guest allocates result buffer using its own allocator, host reads from it after this call.
            // This function returns a packed (ptr << 32) | len for the result in WASM memory.
            // Or, more simply, guest calls a host function to allocate return buffer, then writes to it.
            // Or, host provides buffer, guest writes, returns len. (Current approach for helix_get_config_value etc.)
            // Let's use: guest writes to provided buffer.
            #[no_mangle]
            pub extern "C" fn helix_agent_transform(
                event_bytes_ptr: *const u8, event_bytes_len: usize,
                result_buf_ptr: *mut u8, result_buf_len: usize
            ) -> i32 { // Returns length of serialized Vec<HelixEvent> written, or error code
                log_wasm("helix_agent_transform called");
                match AGENT_INSTANCE.lock() {
                    Ok(mut guard) => {
                        if let Some(agent_instance) = guard.as_mut() {
                            let event_bytes = unsafe { std::slice::from_raw_parts(event_bytes_ptr, event_bytes_len) };
                            match rmp_serde::from_slice::<helix_core::event::Event>(event_bytes) {
                                Ok(event) => {
                                    // Placeholder for: let transformed_events = agent_instance.transform(ctx, event).await;
                                    // This is highly simplified. Real transform is async and needs context via host calls.
                                    let log_msg = format!("helix_agent_transform: instance found, received event id: {}", event.id);
                                    log_wasm(&log_msg);
                                    
                                    let dummy_transformed_event = helix_core::event::Event {
                                        id: helix_core::types::EventId::new_v4(),
                                        source_agent_id: agent_instance.agent_config.id.clone(),
                                        recipe_id: agent_instance.agent_config.recipe_id.clone(),
                                        event_type: "wasm.transformed.event".to_string(),
                                        data: Some(serde_json::json!({
                                            "transformed_by_wasm": true,
                                            "original_event_id": event.id.to_string()
                                        })),
                                        metadata: None,
                                        timestamp: chrono::Utc::now(), // Requires chrono in wasm, or get time from host
                                    };
                                    let result_vec = vec![dummy_transformed_event];

                                    match rmp_serde::to_vec_named(&result_vec) {
                                        Ok(serialized_result) => {
                                            if serialized_result.len() <= result_buf_len {
                                                unsafe {
                                                    std::ptr::copy_nonoverlapping(serialized_result.as_ptr(), result_buf_ptr, serialized_result.len());
                                                }
                                                log_wasm("helix_agent_transform: success, result written to buffer");
                                                serialized_result.len() as i32
                                            } else {
                                                let err_msg = format!("helix_agent_transform: result buffer too small. Required: {}, Available: {}", serialized_result.len(), result_buf_len);
                                                log_wasm(&err_msg);
                                                -5 // Buffer too small error code (consistent with host_functions.rs idea)
                                            }
                                        }
                                        Err(e) => { let err_msg = format!("helix_agent_transform: serialize result error: {}", e); log_wasm(&err_msg); -6 } // Serialization error
                                    }
                                }
                                Err(e) => { let err_msg = format!("helix_agent_transform: deserialize event error: {}", e); log_wasm(&err_msg); -4 } // Deserialization error
                            }
                        } else { log_wasm("helix_agent_transform: agent not_config"); -2 }
                    }
                    Err(_) => { log_wasm("helix_agent_transform: mutex_poisoned"); -3 },
                }
            }

            #[no_mangle]
            pub extern "C" fn helix_agent_stop() -> i32 {
                log_wasm("helix_agent_stop called for TransformAgent");
                match AGENT_INSTANCE.lock() {
                    Ok(mut guard) => {
                        if guard.as_mut().is_some() {
                            log_wasm("helix_agent_stop: instance found");
                            *guard = None; // Clear instance
                            log_wasm("helix_agent_stop: instance cleared");
                            0
                        } else { log_wasm("stop: agent not_config"); -2 }
                    }
                    Err(_) => { log_wasm("stop: mutex_poisoned"); -3 },
                }
            }
        }
    };

    TokenStream::from(expanded)
}

// TODO: Agent registration boilerplate (Task 1.2.1) is TBD and depends on runtime specifics.
//       The macros might need to generate a static registration function or similar.
#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;
    use syn;

    // Helper to check if a specific trait is implemented in the generated code.
    // This is a simplified check; more robust parsing might be needed for complex cases.
    fn check_trait_impl(tokens: &TokenStream, trait_path_str: &str) -> bool {
        let code_str = tokens.to_string();
        // A bit simplistic: just checks if the string "impl SomeTrait for" exists.
        // A more robust way would be to parse the TokenStream back into syn::File
        // and inspect the items, but that's more involved for this example.
        code_str.contains(&format!("impl {}", trait_path_str))
    }

    #[test]
    fn test_source_agent_macro_generates_traits() {
        let item_struct = quote! {
            pub struct MyTestSourceAgent {
                pub agent_config: std::sync::Arc<helix_core::agent::AgentConfig>,
            }

            impl MyTestSourceAgent {
                // Required inherent method
                pub async fn run(&mut self, _context: &helix_agent_sdk::AgentContext) -> Result<(), helix_agent_sdk::SdkError> {
                    Ok(())
                }
            }
        };
        let tokens = source_agent(TokenStream::new(), item_struct.into());
        
        assert!(check_trait_impl(&tokens, "helix_core :: agent :: Agent for MyTestSourceAgent"));
        assert!(check_trait_impl(&tokens, "helix_agent_sdk :: SdkAgent for MyTestSourceAgent"));
        assert!(check_trait_impl(&tokens, "helix_agent_sdk :: SourceSdkAgent for MyTestSourceAgent"));
    }

    #[test]
    fn test_action_agent_macro_generates_traits() {
        let item_struct = quote! {
            pub struct MyTestActionAgent {
                pub agent_config: std::sync::Arc<helix_core::agent::AgentConfig>,
            }

            impl MyTestActionAgent {
                // Required inherent method
                pub async fn execute(&mut self, _context: &helix_agent_sdk::AgentContext, _event: helix_core::event::Event) -> Result<(), helix_agent_sdk::SdkError> {
                    Ok(())
                }
            }
        };
        let tokens = action_agent(TokenStream::new(), item_struct.into());

        assert!(check_trait_impl(&tokens, "helix_core :: agent :: Agent for MyTestActionAgent"));
        assert!(check_trait_impl(&tokens, "helix_agent_sdk :: SdkAgent for MyTestActionAgent"));
        assert!(check_trait_impl(&tokens, "helix_agent_sdk :: ActionSdkAgent for MyTestActionAgent"));
    }

    #[test]
    fn test_transform_agent_macro_generates_traits() {
        let item_struct = quote! {
            pub struct MyTestTransformAgent {
                pub agent_config: std::sync::Arc<helix_core::agent::AgentConfig>,
            }

            impl MyTestTransformAgent {
                // Required inherent method
                pub async fn transform(&mut self, _context: &helix_agent_sdk::AgentContext, _event: helix_core::event::Event) -> Result<Vec<helix_core::event::Event>, helix_agent_sdk::SdkError> {
                    Ok(Vec::new())
                }
            }
        };
        let tokens = transform_agent(TokenStream::new(), item_struct.into());

        assert!(check_trait_impl(&tokens, "helix_core :: agent :: Agent for MyTestTransformAgent"));
        assert!(check_trait_impl(&tokens, "helix_agent_sdk :: SdkAgent for MyTestTransformAgent"));
        assert!(check_trait_impl(&tokens, "helix_agent_sdk :: TransformSdkAgent for MyTestTransformAgent"));
    }
}