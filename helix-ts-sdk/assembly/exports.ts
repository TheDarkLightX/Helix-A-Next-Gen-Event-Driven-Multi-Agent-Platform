/**
 * This file serves as the entry point for compiling a TypeScript agent to WASM.
 * It handles initializing the agent, setting up SDK utilities (like memory),
 * and exporting the C-compatible functions that the Helix WASM runtime will call.
 *
 * Agent developers will typically:
 * 1. Import their specific agent class (e.g., `MySourceAgent`).
 * 2. Import necessary SDK components if not already handled by their agent's base class.
 * 3. Update the `AGENT_CLASS_TO_INSTANTIATE` constant to their agent class.
 * 4. Ensure their agent class constructor can accept an `AgentConfig`.
 * 5. Optionally, if their agent needs a larger shared buffer, adjust `SHARED_BUFFER_SIZE`.
 */

import {
  AbstractHelixAgent,
  SourceAgent,
  ActionAgent,
  TransformAgent,
} from "./core/agent.ts";
import { AgentConfig, Event } from "./core/types.ts";
import {
  initWasmMemory,
  _helix_sdk_init_shared_buffer,
  readStringFromWasmMemory,
  writeStringToSharedBuffer,
  getSharedBufferPtr,
  getSharedBufferSize,
} from "./utils/memory.ts";
import { logMessage } from "./host/log.ts"; // For internal logging if needed

// --- Configuration by Agent Developer ---
import { SimpleSourceAgent } from "../examples/simple_source_agent.ts";
const AGENT_CLASS_TO_INSTANTIATE = SimpleSourceAgent;

// Define the desired size for the shared buffer for host communication.
// Agent developers can adjust this based on the expected size of data
// they will exchange with host functions (e.g., config values, event payloads).
const SHARED_BUFFER_SIZE = 4096; // 4KB, adjust as needed

// --- SDK Internal State ---
let agentInstance: AbstractHelixAgent | null = null;
let wasmMemory: WebAssembly.Memory | null = null;

// A global static buffer within the WASM module's linear memory.
// Its pointer will be passed to `_helix_sdk_init_shared_buffer`.
// This is one way to provide a memory region for the shared buffer.
// The size must match SHARED_BUFFER_SIZE.
// Note: Deno compile might optimize static allocations. This needs testing.
// If this doesn't work as expected, the agent might need to export an explicit
// `_helix_alloc_memory(size: i32): i32` function that the SDK calls.
const internalSharedBuffer = new Uint8Array(SHARED_BUFFER_SIZE);


// --- WASM ABI Exports ---

/**
 * Initializes the agent instance and SDK utilities.
 * Called by the Helix WASM runtime once the module is loaded.
 *
 * The host is expected to provide:
 * 1. The WebAssembly.Memory instance (e.g., via import or a host function).
 *    For `wasm32-wasi` target with `deno compile`, memory is usually exported.
 * 2. The agent configuration (e.g., as a JSON string via a host function call,
 *    or individual config values fetched via `helix_get_config_value`).
 *
 * This export assumes `memory` is an exported global from the WASM module.
 * The host (Wasmtime) will need to get this exported memory.
 */
export async function _helix_agent_init(
    // Parameters from host to provide agent config, if any.
    // For now, we assume config is fetched via host.getConfigValue within AgentContext.
    // Or, a host function `helix_get_initial_agent_config_json_ptr_len()` could be called here.
    // Let's assume `agent_config_json_ptr` and `agent_config_json_len` are passed.
    agent_config_json_ptr: number,
    agent_config_json_len: number
): Promise<void> {
  try {
    // Step 1: Initialize WASM Memory for the SDK
    // Deno compile for wasm32-wasi typically exports 'memory'.
    // The host (Wasmtime) needs to get this export and pass it here, or the SDK needs to access it.
    // This is a placeholder for how memory is actually obtained.
    // If 'memory' is a global export from the module:
    // This function, being part_of the module, should have access to its own exports if structured correctly,
    // or the host passes it. For now, assuming `wasmMemory` is set by the host or a startup hook.
    // A common pattern is for the WASM module to export its memory, and the host to grab it.
    // The SDK then needs access to this `WebAssembly.Memory` object.
    // Let's assume the host calls an exported function like `_helix_sdk_set_memory(mem: WebAssembly.Memory)`
    // OR that `WebAssembly.table.get(0)` or similar gives access if memory is implicitly available.
    // For `deno compile`, direct access to `WebAssembly.instance.exports.memory` from within the module
    // is not straightforward. The host usually manages this.
    //
    // **Revised approach for memory init:**
    // The WASM module will export its memory. The host will get it.
    // The SDK's `initWasmMemory` needs this. We can't call it here without the host passing it.
    // So, `initWasmMemory` should ideally be called by the host after instantiation,
    // or this `_helix_agent_init` should receive the memory object or its buffer.
    //
    // Let's assume `memory` is an import provided by the "env" module, like other host functions.
    // This is not standard for `wasm32-wasi` which *exports* memory.
    //
    // **Simplification for now:** The `initWasmMemory` will be called by the host
    // *before* `_helix_agent_init` if it needs the memory instance.
    // OR, we assume the `internalSharedBuffer` approach works and its pointer is stable.

    if (!wasmMemory) {
        // This indicates a setup issue. The host should have provided memory.
        // For now, we'll try to use the `internalSharedBuffer`'s underlying buffer
        // if `WebAssembly.Memory` is not directly available here. This is a bit of a hack.
        // A robust solution needs the host to pass the memory reference.
        // console.error("WASM Memory not explicitly set for SDK. Attempting to use internal buffer's memory (less ideal).");
        // This won't work as `internalSharedBuffer.buffer` is not the *WASM instance's* memory buffer.
        //
        // **Critical:** The host must call an exported function like `_helix_sdk_provide_memory_buffer(ptr, len)`
        // or the SDK must be able to access the exported `memory` from the instance.
        // For now, `initWasmMemory` in `memory.ts` expects a `WebAssembly.Memory` object.
        // This `_helix_agent_init` cannot proceed without it.
        // The plan mentioned `initWasmMemory` being called. Let's assume the host does this.
        // If not, this init will fail.
        // The `memory.ts` `ensureMemoryInitialized` will throw if `initWasmMemory` wasn't called.
    }

    // Initialize the SDK's shared buffer using the statically allocated internal one.
    // Get the pointer to internalSharedBuffer within the WASM linear memory.
    // This is non-trivial from TS itself. Deno compile might place it at a known offset,
    // or we might need a helper in a language like AssemblyScript or Rust to get its address.
    //
    // **Alternative for shared buffer init:**
    // Export `_helix_get_internal_shared_buffer_ptr()` and `_helix_get_internal_shared_buffer_size()`
    // The host calls these, then calls `_helix_sdk_init_shared_buffer(ptr, size)` with these values.
    // This seems more robust for WASM.

    // For now, let's assume `_helix_sdk_init_shared_buffer` is called by the host
    // after getting the pointer/size of `internalSharedBuffer` via other exports.
    // If not, `getSharedBufferPtr()` will throw.

    // Step 2: Parse Agent Configuration
    // The host calls this with a pointer and length to a JSON string for AgentConfig.
    const configJson = readStringFromWasmMemory(agent_config_json_ptr, agent_config_json_len);
    const config: AgentConfig = JSON.parse(configJson);

    // Step 3: Instantiate the Agent
    if (!AGENT_CLASS_TO_INSTANTIATE) {
      throw new Error(
        "AGENT_CLASS_TO_INSTANTIATE is not set in src/exports.ts. Agent developer must set this.",
      );
    }
    agentInstance = new AGENT_CLASS_TO_INSTANTIATE(config);

    // Step 4: Call Agent's own init method, if it exists
    if (agentInstance && typeof agentInstance.init === "function") {
      await agentInstance.init();
    }
    logMessage(`Agent ${config.id} initialized successfully.`);

  } catch (e: any) {
    const errorMessage = `Agent initialization failed: ${e.message} \n ${e.stack ? e.stack : ''}`;
    logMessage(errorMessage); // Try to log, might fail if shared buffer/memory init failed
    throw e; // Re-throw to trap in host, indicating fatal init error
  }
}

/**
 * Called by the host to set the WebAssembly.Memory instance for the SDK.
 * This MUST be called by the host immediately after instantiation if the SDK
 * needs to operate on the main WASM memory.
 */
export function _helix_sdk_set_wasm_memory(memoryInstance: WebAssembly.Memory): void {
    if (memoryInstance) {
        initWasmMemory(memoryInstance);
        wasmMemory = memoryInstance; // Store it if needed for other direct operations
        logMessage("SDK: WebAssembly.Memory instance received from host.");

        // Now that memory is set, also initialize the shared buffer using the internal static buffer.
        // This requires getting the *actual runtime pointer* of `internalSharedBuffer`.
        // This is the tricky part. `internalSharedBuffer.byteOffset` might be what we need if it's part of the main buffer.
        // For a truly static buffer compiled in, its address relative to the start of linear memory
        // would be fixed. This needs verification with `deno compile`'s output.
        //
        // A more reliable way for WASI: export a function that returns the buffer's address.
        // `export function _helix_get_static_buffer_address(): number { return internalSharedBuffer.byteOffset (or actual address); }`
        // The host calls this, then calls `_helix_sdk_init_shared_buffer_with_offset`.
        //
        // Let's assume for now the agent developer provides an exported function to get this pointer.
        // Or, the host passes the pointer to `internalSharedBuffer` to `_helix_sdk_init_shared_buffer`.
        // This `_helix_sdk_set_wasm_memory` is a good place to also init the shared buffer if we can get its pointer.
    } else {
        logMessage("SDK Error: Host attempted to set null or undefined WebAssembly.Memory.");
        throw new Error("Host provided invalid WebAssembly.Memory instance to _helix_sdk_set_wasm_memory.");
    }
}

/**
 * Exports a function that the host can call to initialize the SDK's shared buffer.
 * The host should determine the pointer to a suitable memory region (e.g., by calling
 * another exported function like `_helix_get_internal_shared_buffer_details`)
 * and then call this function.
 * @param ptr Pointer to the memory region to be used as a shared buffer.
 * @param size Size of that memory region.
 */
export function _helix_sdk_initialize_shared_buffer(ptr: number, size: number): void {
    try {
        _helix_sdk_init_shared_buffer(ptr, size);
        logMessage(`SDK: Shared buffer initialized by host at offset ${ptr} with size ${size}.`);
    } catch (e: any) {
        logMessage(`SDK Error: Failed to initialize shared buffer via host: ${e.message}`);
        throw e;
    }
}

// Example of how an agent might export details of its internal static buffer:
export function _helix_get_internal_shared_buffer_details(): BigInt { // Return ptr and size packed
    // This is highly dependent on how `deno compile` lays out static data.
    // This might require AssemblyScript or similar to get the actual pointer.
    // For now, this is a conceptual placeholder.
    // If `internalSharedBuffer` is part of the main heap and `buffer.byteOffset` is usable:
    // const ptr = (internalSharedBuffer as any).buffer.byteOffset + internalSharedBuffer.byteOffset; // This is likely incorrect.
    // A common way in C/Rust compiled to WASM is to just take the address of a static array.
    // In TS, this is not directly possible.
    //
    // Safest: The WASM module exports `_helix_alloc(size)` and `_helix_free(ptr)`.
    // `_helix_agent_init` calls `_helix_alloc(SHARED_BUFFER_SIZE)` and then `_helix_sdk_init_shared_buffer`.
    // This is a common pattern for WASM modules that need to manage their own memory for such buffers.
    // This would require adding `alloc/free` to this exports file, likely implemented
    // by a simple bump allocator or by linking in a proper allocator if `deno compile` supports it.
    // For `wasm32-wasi`, `malloc` might be available if linked.
    //
    // Let's assume the host will provide the ptr/size to `_helix_sdk_initialize_shared_buffer`.
    // So, this function might not be strictly needed if the host has another way to allocate/provide memory.
    const ptr = 0; // Placeholder! This needs a real address.
    const size = SHARED_BUFFER_SIZE;
    return (BigInt(ptr) << 32n) | BigInt(size); // Pack ptr and size
}


// --- Agent Capability Exports ---

export async function _helix_run_source(): Promise<void> {
  if (!agentInstance) throw new Error("Agent not initialized.");
  if (typeof (agentInstance as any).runSource !== "function") {
    throw new Error("Agent does not implement SourceAgent or runSource method.");
  }
  await (agentInstance as SourceAgent).runSource();
}

export async function _helix_run_action(
  event_payload_json_ptr: number,
  event_payload_json_len: number,
): Promise<void> {
  if (!agentInstance) throw new Error("Agent not initialized.");
  if (typeof (agentInstance as any).runAction !== "function") {
    throw new Error("Agent does not implement ActionAgent or runAction method.");
  }
  const eventJson = readStringFromWasmMemory(event_payload_json_ptr, event_payload_json_len);
  const event: Event = JSON.parse(eventJson); // Assume host sends the full Event object stringified
  await (agentInstance as ActionAgent).runAction(event);
}

export async function _helix_run_transform(
  event_payload_json_ptr: number,
  event_payload_json_len: number,
): Promise<number> { // Returns ptr/len of the transformed event JSON, or 0/0 if null
  if (!agentInstance) throw new Error("Agent not initialized.");
  if (typeof (agentInstance as any).runTransform !== "function") {
    throw new Error("Agent does not implement TransformAgent or runTransform method.");
  }
  const eventJson = readStringFromWasmMemory(event_payload_json_ptr, event_payload_json_len);
  const event: Event = JSON.parse(eventJson);
  const transformedEvent = await (agentInstance as TransformAgent).runTransform(event);

  if (transformedEvent) {
    const transformedEventJson = JSON.stringify(transformedEvent);
    // Write to shared buffer and return its ptr and length (packed or via host call)
    // For now, assume writeStringToSharedBuffer and return length. Host uses getSharedBufferPtr.
    const len = writeStringToSharedBuffer(transformedEventJson);
    // The host will know to read `len` bytes from `getSharedBufferPtr()`.
    // A more robust ABI would return both ptr and len.
    // Let's return just len, assuming host uses the fixed shared buffer pointer.
    return len;
  }
  return 0; // Indicates null/undefined transform (event filtered)
}

// Placeholder for memory allocation functions if needed by the SDK/Agent.
// These would typically be part of the WASM module if it manages its own heap.
// For wasm32-wasi, these might be provided by the environment if linked.
// export function _helix_alloc(size: number): number {
//   // Implementation of a simple allocator or call to `malloc`
//   return 0; // Placeholder
// }
// export function _helix_free(ptr: number): void {
//   // Implementation of `free`
// }