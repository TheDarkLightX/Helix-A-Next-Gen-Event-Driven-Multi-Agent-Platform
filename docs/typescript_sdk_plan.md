# Plan: TypeScript SDK for Helix WASM Agents

## 1. Overview

This plan outlines the development of a TypeScript SDK that enables developers to write Helix agents in TypeScript. These agents will be compiled to WASM using `deno compile --unstable --target wasm` and run within the Helix `helix-wasm` runtime. The SDK aims to provide a developer experience similar to the Rust-based `helix-agent-sdk`, abstracting away the complexities of WASM interoperability and host function communication.

## 2. SDK Structure

A new directory, `helix-ts-sdk` (or similar, e.g., `sdk/typescript`), will be created at the root of the `helix-platform` repository or as a separate package if preferred.

Proposed directory structure:

```
helix-ts-sdk/
├── src/
│   ├── core/                  # Core types, interfaces, and base classes
│   │   ├── agent.ts           # Base Agent class/interface
│   │   ├── capabilities.ts    # Definitions for Source, Transform, Action
│   │   ├── context.ts         # AgentContext definition
│   │   └── types.ts           # Core Helix types (Event, Config, etc.)
│   ├── host/                  # Wrappers for host functions
│   │   ├── index.ts           # Exports all host function wrappers
│   │   ├── log.ts             # helix_log_message wrapper
│   │   ├── event.ts           # helix_emit_event wrapper
│   │   ├── config.ts          # helix_get_config_value wrapper
│   │   ├── state.ts           # helix_get_state, helix_set_state wrappers
│   │   ├── credential.ts      # helix_get_credential wrapper
│   │   ├── time.ts            # helix_get_time wrapper
│   │   └── random.ts          # helix_random wrapper
│   ├── utils/                 # Utility functions
│   │   ├── serialization.ts   # JSON serialization/deserialization helpers
│   │   ├── memory.ts          # Helpers for string/buffer management with WASM
│   │   └── errors.ts          # Custom error types for the SDK
│   ├── mod.ts                 # Main entry point for the SDK (Deno convention)
│   └── exports.ts             # Explicitly defined exports for WASM module
├── examples/                  # Example TypeScript agents
│   ├── simple_source_agent.ts
│   └── simple_action_agent.ts
├── deno.json                  # Deno configuration file (for tasks, imports)
├── README.md                  # SDK documentation
└── LICENSE                    # License file
```

### 2.1 SDK Interaction Diagram

```mermaid
graph TD
    subgraph DeveloperSpace [Developer Space]
        DevAgent[Developer's TypeScript Agent Code (e.g., MySourceAgent.ts)]
        DenoCompile[deno compile --target wasm]
    end

    subgraph HelixTsSdk [helix-ts-sdk]
        SdkCore[src/core (agent.ts, types.ts, context.ts)]
        SdkHost[src/host (Host Function Wrappers)]
        SdkUtils[src/utils (memory.ts, serialization.ts, errors.ts)]
        SdkExports[src/exports.ts (_helix_agent_init, _helix_run_source, etc.)]
    end

    subgraph CompiledWasmModule [Compiled WASM Module (my_agent.wasm)]
        WasmAgentLogic[Agent Logic (from DevAgent & SdkCore)]
        WasmSdkWrappers[SDK Wrappers (from SdkHost & SdkUtils)]
        WasmExports[Exported Functions (from SdkExports)]
        WasmMemory[WASM Linear Memory]
    end

    subgraph HelixWasmRuntime [Helix WASM Runtime (Rust Host)]
        WasmtimeEngine[Wasmtime Engine]
        HostFunctions[Host Functions (e.g., helix_log_message_impl)]
        HostState[HostState (AgentConfig, EventPublisher, etc.)]
        AgentRunner[AgentRunner (manages WASM lifecycle)]
    end

    DevAgent -- Uses --> SdkCore
    DevAgent -- Uses --> SdkHost
    SdkCore -- Used by --> SdkExports
    SdkHost -- Uses --> SdkUtils
    SdkHost -- Defines Imports for --> HostFunctions
    SdkUtils -- Manages --> WasmMemory

    DenoCompile -- Compiles --> DevAgent
    DenoCompile -- Compiles --> SdkCore
    DenoCompile -- Compiles --> SdkHost
    DenoCompile -- Compiles --> SdkUtils
    DenoCompile -- Compiles --> SdkExports
    DenoCompile -- Produces --> CompiledWasmModule

    WasmAgentLogic -- Contained in --> CompiledWasmModule
    WasmSdkWrappers -- Contained in --> CompiledWasmModule
    WasmExports -- Contained in --> CompiledWasmModule
    WasmMemory -- Part of --> CompiledWasmModule

    AgentRunner -- Loads & Calls --> WasmExports
    WasmtimeEngine -- Executes --> WasmAgentLogic
    WasmtimeEngine -- Executes --> WasmSdkWrappers
    
    WasmSdkWrappers -- Calls (ABI) --> HostFunctions
    HostFunctions -- Access --> HostState
    HostFunctions -- Interact with --> WasmMemory

    WasmExports -- Calls --> WasmAgentLogic
    WasmAgentLogic -- Calls --> WasmSdkWrappers
```

## 3. Core Functionality

### 3.1. Agent Definition

Agents will be defined as TypeScript classes that extend a base `HelixAgent` class or implement a `HelixAgent` interface. This base class/interface will provide common functionality and define required methods for different agent capabilities.

```typescript
// src/core/agent.ts (Conceptual)
import { AgentContext } from "./context.ts";
import { Event } from "./types.ts";

export interface HelixAgent {
  readonly agentId: string; // Provided by context or config
  readonly context: AgentContext;

  // Optional initialization
  init?(): Promise<void>;
}

export interface SourceAgent extends HelixAgent {
  // Called by the runtime to pull data
  runSource(): Promise<void>;
}

export interface TransformAgent extends HelixAgent {
  // Called by the runtime to process an event
  runTransform(event: Event): Promise<Event | null>;
}

export interface ActionAgent extends HelixAgent {
  // Called by theruntime to perform an action based on an event
  runAction(event: Event): Promise<void>;
}
```

### 3.2. Capability Registration

Similar to the Rust SDK, agents will declare their capabilities (Source, Transform, Action). This could be achieved through:
*   **Decorators (Preferred if feasible with `deno compile`):**
    ```typescript
    // @sourceAgent() // or @agent({ type: "source" })
    // class MySourceAgent extends BaseSourceAgent { ... }
    ```
*   **Static properties or methods:**
    ```typescript
    // class MySourceAgent extends BaseSourceAgent {
    //   static agentType = "source";
    //   ...
    // }
    ```
The SDK will need to export specific, well-known functions that the `helix-wasm` runtime can call to trigger agent logic (e.g., `_helix_run_source`, `_helix_execute_event_transform`, `_helix_execute_event_action`). These exported functions will instantiate the agent class and invoke the appropriate methods.

### 3.3. Host Function Interaction

The SDK will provide strongly-typed TypeScript wrappers for each host function defined in `crates/helix-wasm/src/host_functions.rs`. These wrappers will handle:
*   **ABI differences:** Translating TypeScript types to/from the `(pointer, length)` and `i32` return codes used by the WASM host functions.
*   **Memory management:** Using utility functions (e.g., in `src/utils/memory.ts`) to:
    *   Encode TypeScript strings to UTF-8 `Uint8Array`s.
    *   Allocate memory within the WASM module (if Deno/TS WASM target supports this easily, otherwise rely on host-provided buffers or a pre-allocated shared buffer). For functions like `helix_get_config_value`, the SDK will need to manage a buffer, pass its pointer and length to the host, and then decode the result.
    *   Decode UTF-8 `Uint8Array`s from WASM memory back to TypeScript strings.
*   **Serialization:** Ensuring data (like event payloads, config values, state) is correctly serialized to JSON strings before being passed to the host, and deserialized from JSON strings when received from the host.

**Example Wrapper (Conceptual):**

```typescript
// src/host/log.ts
// @ts-ignore: Deno global for WASM imports
import { helix_log_message as wasm_helix_log_message } from "env";
import { encodeString, writeStringToMemory } from "../utils/memory.ts"; // Hypothetical

export function logMessage(message: string): void {
  // Simplified: Assumes guest allocates and passes ptr/len
  // A more robust version would use a shared buffer or guest-managed memory
  const { ptr, len } = writeStringToMemory(message); // This needs careful design
  wasm_helix_log_message(ptr, len);
  // Potentially free memory if guest-allocated
}

// src/host/config.ts
// @ts-ignore: Deno global for WASM imports
import { helix_get_config_value as wasm_helix_get_config_value } from "env";
import { encodeString, readStringFromMemory, ensureBuffer } from "../utils/memory.ts";
import { WasmError } from "../utils/errors.ts";

const CONFIG_BUFFER_SIZE = 1024; // Initial buffer size, can be dynamic
let configBufferPtr = 0; // Pointer to a pre-allocated buffer in WASM memory
// This buffer needs to be allocated by the WASM module itself upon initialization.

// This function would be called during agent init to allocate the buffer.
// export function _helix_sdk_init_buffers() { /* ... allocate ... */ configBufferPtr = ...; }


export function getConfigValue<T>(key: string): T | null {
  if (!configBufferPtr) {
    throw new Error("SDK buffers not initialized. Call _helix_sdk_init_buffers first.");
  }

  const keyBytes = encodeString(key);
  // For simplicity, assuming keyPtr/keyLen are handled by a helper that allocates/writes
  // In reality, the SDK needs to manage memory for these transient strings too.
  const { ptr: keyPtr, len: keyLen } = writeStringToMemory(keyBytes);


  const bytesWritten = wasm_helix_get_config_value(keyPtr, keyLen, configBufferPtr, CONFIG_BUFFER_SIZE);

  if (bytesWritten === WasmError.VALUE_NOT_FOUND_CODE) {
    return null;
  }
  if (bytesWritten === WasmError.BUFFER_TOO_SMALL_CODE) {
    // Handle buffer too small (e.g., reallocate or throw specific error)
    // This might involve a host function to query required size.
    throw new Error(`Buffer too small for config key: ${key}. Required size unknown from this call.`);
  }
  if (bytesWritten < 0) { // Other host function errors
      throw new Error(`Host function error for helix_get_config_value: code ${bytesWritten}`);
  }


  const valueStr = readStringFromMemory(configBufferPtr, bytesWritten);
  return JSON.parse(valueStr) as T;
}
```
The `src/utils/memory.ts` module will be crucial. It might involve:
*   Functions to allocate and free memory within the WASM module's linear memory (if Deno's WASM support allows direct manipulation or if AssemblyScript/other tools are used for this part).
*   A strategy for managing shared buffers passed to host functions that write data back. The WASM module might export functions like `_helix_alloc_buffer(size: i32): i32` and `_helix_free_buffer(ptr: i32)`.

### 3.4. Serialization

JSON is the chosen serialization format.
*   The SDK's host function wrappers will automatically handle `JSON.stringify()` for data sent to the host and `JSON.parse()` for data received from the host.
*   This abstraction means agent developers work directly with TypeScript objects.
*   `src/utils/serialization.ts` might contain helpers if custom revivers/replacers are needed, but standard JSON methods should suffice.

## 4. Agent Lifecycle

The `helix-wasm` runtime will call specific exported functions from the compiled WASM module. The TypeScript SDK needs to define and export these.

**Example Exported Functions (in `src/exports.ts` or similar):**

```typescript
// This file would be the entry point for `deno compile`
import { MySourceAgent } from "../examples/simple_source_agent.ts"; // Example
import { MyActionAgent } from "../examples/simple_action_agent.ts"; // Example
// ... import other agent implementations

// Global agent instance (or a factory to create them)
// This needs careful thought: how does the host specify WHICH agent to run if multiple are in one WASM?
// For now, assume one agent per WASM module, or a default one.
let agentInstance: any; // HelixAgent | SourceAgent | ActionAgent etc.

// Called by the host to initialize the agent.
// The host might pass agent_id and config via host functions before calling this.
export async function _helix_agent_init(): Promise<void> {
  // Determine which agent to instantiate based on some mechanism
  // (e.g., an environment variable set by Deno compile, or a single expected agent)
  // For this example, hardcoding one:
  agentInstance = new MySourceAgent(/* pass context created from host functions */);
  if (agentInstance.init) {
    await agentInstance.init();
  }
}

export async function _helix_run_source(): Promise<void> {
  if (!agentInstance || typeof agentInstance.runSource !== 'function') {
    throw new Error("Agent not initialized or not a SourceAgent");
  }
  await (agentInstance as SourceAgent).runSource();
}

export async function _helix_execute_event_transform(eventPayloadPtr: number, eventPayloadLen: number): Promise<number> {
  // 1. Read event payload string from memory (using eventPayloadPtr, eventPayloadLen)
  // 2. JSON.parse() it into an Event object
  // 3. Call agentInstance.runTransform(event)
  // 4. If result, JSON.stringify() it
  // 5. Write result string to a pre-allocated buffer (or allocate, write, return ptr/len)
  // 6. Return length written, or error code.
  // This function will need to manage memory for the return payload.
  // It might return a pointer/length pair packed into a single number, or use another host call.
  // For simplicity, let's assume it writes to a known buffer and returns length.
  if (!agentInstance || typeof agentInstance.runTransform !== 'function') {
    throw new Error("Agent not initialized or not a TransformAgent");
  }
  const eventStr = readStringFromMemory(eventPayloadPtr, eventPayloadLen);
  const event: Event = JSON.parse(eventStr);
  const resultEvent = await (agentInstance as TransformAgent).runTransform(event);
  if (resultEvent) {
    const resultStr = JSON.stringify(resultEvent);
    return writeStringToSharedBuffer(resultStr); // Returns length written or error code
  }
  return 0; // Indicate no event emitted / null transform
}

// Similar function for _helix_execute_event_action
```
The `AgentContext` provided to agents will be populated by calling host functions (`helix_get_config_value`, etc.) during the agent's initialization or lazily.

## 5. Compilation and Build Process

1.  **Development:** Developers write their agent in TypeScript using the `helix-ts-sdk`.
2.  **Configuration (`deno.json`):**
    ```json
    {
      "tasks": {
        "build:wasm": "deno compile --unstable --allow-env --allow-read --output my_agent.wasm --target wasm32-wasi src/exports.ts"
        // --allow-env might be needed if config is passed via env vars at compile time
        // --allow-read if agents need to read local files during build (unlikely for typical agents)
        // Permissions for host function access are granted by the WASM runtime, not Deno compile flags.
      },
      "compilerOptions": {
        "lib": ["deno.ns", "esnext"] // Ensure Deno specific APIs are available if needed
      },
      "importMap": "./import_map.json" // Optional, for managing dependencies
    }
    ```
3.  **Compilation:** Developer runs `deno task build:wasm`.
    *   This command compiles the TypeScript code (starting from `src/exports.ts` which imports the agent implementation) into a `my_agent.wasm` file.
    *   The target `wasm32-wasi` is crucial for compatibility with `wasmtime-wasi`.
4.  **Deployment:** The resulting `.wasm` file is then deployed and configured within the Helix Platform like any other WASM agent.

**Considerations:**
*   **Bundle Size:** `deno compile` bundles all dependencies. The SDK should be mindful of its size. Tree-shaking should help.
*   **WASI Imports:** The `wasm32-wasi` target implies certain WASI imports will be expected by the compiled module (e.g., for console logging if `console.log` is used, file access if Deno's FFI for files is used, etc.). The `helix-wasm` runtime already links WASI.
*   **Memory Management for Strings/Buffers:** This is the most complex part. The SDK needs to provide utilities for TypeScript code to interact with the WASM linear memory and pass/receive data to/from host functions. This might involve:
    *   Exporting memory from the WASM module.
    *   The SDK having functions like `allocate(size: number): number` (returns pointer) and `deallocate(ptr: number)` that are part of the compiled WASM and callable from TS. AssemblyScript is often used for such low-level memory management in WASM, but the goal is to use pure Deno/TS if possible. If not, a small AssemblyScript helper module compiled to WASM and then imported by the Deno TS code might be an option, though this adds complexity.
    *   Alternatively, relying on fixed-size buffers allocated at startup, or a host function to request temporary buffer allocation.

## 6. Type Definitions

Core Helix types (Event, AgentConfig, Credential, StateData, etc.) defined in Rust's `helix-core` need to be available in TypeScript.
Options:
1.  **Manual Definition (Initial Approach):** Define corresponding TypeScript interfaces in `helix-ts-sdk/src/core/types.ts`. This is simpler to start but requires manual synchronization if Rust types change.
    ```typescript
    // src/core/types.ts
    export interface EventHeader {
      id: string;
      timestamp: string; // ISO 8601
      agent_id: string;
      kind?: string;
    }

    export interface Event {
      header: EventHeader;
      payload: any; // Or a generic type T
    }

    export interface AgentConfig {
      id: string;
      profile_id: string;
      config: Record<string, any>;
      // ... other fields
    }
    ```
2.  **Type Generation (Future Enhancement):** Explore tools like `ts-rs` (if it can be adapted or a similar tool exists for Deno/WASM context) or custom scripts to generate TypeScript type definitions from Rust source code. This improves accuracy and reduces manual effort.

## 7. Error Handling

*   Host functions return `i32` status codes. `0` for success, negative values for errors (e.g., `WasmError::BUFFER_TOO_SMALL_CODE`, `WasmError::VALUE_NOT_FOUND_CODE`, `WasmError::HOST_FUNCTION_ERROR_CODE` from `crates/helix-wasm/src/errors.rs`).
*   The SDK's host function wrappers will check these return codes and throw specific TypeScript errors.
    ```typescript
    // src/utils/errors.ts
    export class HelixWasmError extends Error {
      constructor(message: string, public code?: number) {
        super(message);
        this.name = "HelixWasmError";
      }
    }

    export class BufferTooSmallError extends HelixWasmError { /* ... */ }
    export class ValueNotFoundError extends HelixWasmError { /* ... */ }
    // etc.
    ```
*   Errors originating within the TypeScript agent code should be standard JavaScript/TypeScript errors. The WASM runtime (Wasmtime) will trap on unhandled exceptions from the guest. The SDK should encourage try/catch within agent logic where appropriate.
*   The `_helix_...` exported functions should catch errors from agent code and potentially log them via `helix_log_message` before letting the error propagate (which would trap in the host).

## 8. Development Roadmap/Phases (Condensed)

**Phase 1: Foundational SDK & End-to-End Test (MVP)**
1.  **Setup & Core Types:**
    *   Initialize `helix-ts-sdk` directory, `deno.json`.
    *   Manually define essential Helix types (`Event`, `AgentConfig`, basic error types).
2.  **Memory & Host Function Basics:**
    *   Implement core memory management utilities (`src/utils/memory.ts`) for string/buffer handling (e.g., UTF-8 encoding/decoding, a simple shared buffer strategy allocated by the WASM module).
    *   Implement wrappers for critical host functions: `helix_log_message`, `helix_get_config_value`, `helix_emit_event`, `helix_get_time`. Ensure basic error code handling.
3.  **Agent Definition & Lifecycle Exports:**
    *   Define `AgentContext`, base `HelixAgent` class/interface, and capability interfaces (`SourceAgent`, `ActionAgent`).
    *   Implement the primary lifecycle exports: `_helix_agent_init`, `_helix_run_source`, `_helix_execute_event_action`.
4.  **Simple Agent & Compilation:**
    *   Create a basic Source agent (e.g., emits a timestamped event with a config value) and a basic Action agent (e.g., logs an event payload).
    *   Set up `deno compile` task and successfully compile these agents to WASM.
5.  **Initial Integration & Testing:**
    *   Test the compiled WASM agents within the Helix platform, verifying basic lifecycle and host function interaction (logging, config, event emission).

**Phase 2: Full Host Function Coverage, Advanced Features & DX**
1.  **Complete Host Function Wrappers:**
    *   Implement robust wrappers for all remaining host functions: `helix_get_state`, `helix_set_state`, `helix_get_credential`, `helix_random`.
    *   Ensure comprehensive error handling (all `WasmError` codes) and JSON serialization/deserialization.
2.  **Refine Agent Capabilities & Context:**
    *   Implement `TransformAgent` interface and `_helix_execute_event_transform` export.
    *   Flesh out `AgentContext` to provide easy access to all host functions.
3.  **Examples & Documentation:**
    *   Develop more comprehensive example agents for each type (Source, Transform, Action).
    *   Write initial `README.md` and basic API documentation for the SDK.
4.  **Developer Experience (DX) & Refinements:**
    *   Investigate and implement (if feasible) decorators for agent/capability definition.
    *   Refine memory management strategies based on learnings from Phase 1.
    *   Improve error reporting and diagnostics.
    *   Explore options for automated TypeScript type generation from Rust sources (e.g., `ts-rs` or custom tooling) as a longer-term goal.

## 9. Open Questions & Challenges

*   **Memory Management:** This is the most significant challenge. Efficiently and safely managing memory for strings and buffers passed between TS (in WASM) and the Rust host requires careful design. The `deno compile` target for WASM is relatively new, and its capabilities/limitations for low-level memory interop need to be fully understood.
    *   How does TS code compiled to WASM allocate memory that the host can write to?
    *   How are pointers to this memory obtained and passed?
    *   Is there a standard way to `malloc`/`free` from TS-in-WASM, or does the SDK need to implement its own simple allocator on top of a large `ArrayBuffer`?
*   **Agent Identification:** If a single `.wasm` file can contain multiple agent definitions, how does the host specify which one to initialize and run? (The current plan assumes one primary agent or a default).
*   **Debugging:** How will developers debug TypeScript agents running as WASM? Source map support with `deno compile` and `wasmtime` would be ideal but might be limited.
*   **Performance:** Overhead of JSON serialization/deserialization and string conversions between UTF-8 (Rust/WASM) and UTF-16 (JavaScript strings internally, though Deno might optimize). For high-throughput agents, this could be a concern.

This plan provides a starting point. The initial phases will likely uncover more detailed requirements and challenges, especially around memory management and the specifics of the `deno compile --target wasm` output.