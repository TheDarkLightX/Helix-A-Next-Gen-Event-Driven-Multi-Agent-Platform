/**
 * Helix TypeScript SDK
 *
 * This module is the main entry point for the Helix TypeScript SDK.
 * It re-exports the core classes, interfaces, types, and utilities
 * necessary for developing Helix agents in TypeScript that can be compiled to WASM.
 *
 * @module
 */

// Core agent definitions and types
export * from "./core/agent.ts";
export * from "./core/context.ts";
export * from "./core/types.ts";
// export * from "./core/capabilities.ts"; // To be created if specific capability enums/consts are needed

// Host function wrappers (exposed via AgentContext, but can be re-exported if direct use is ever needed)
// Typically, agents will use these via `AgentContext` instance.
// export * as host from "./host/index.ts";

// Utility functions (e.g., for error handling)
export * from "./utils/errors.ts";
// Memory utilities are mostly for internal SDK use or advanced scenarios.
// export * from "./utils/memory.ts";
// Serialization utilities are also mostly internal.
// export * from "./utils/serialization.ts"; // To be created if public serialization helpers are needed

// Lifecycle exports that agent developers might need to be aware of,
// though they typically don't call these directly.
// These are primarily for the WASM module's entry point (e.g., src/exports.ts).
// No direct re-export needed here for agent developers.

// Note: The `src/exports.ts` file will be the actual entry point for `deno compile`
// and will handle instantiating the specific agent and exporting the WASM ABI functions
// like `_helix_agent_init`, `_helix_run_source`, etc.