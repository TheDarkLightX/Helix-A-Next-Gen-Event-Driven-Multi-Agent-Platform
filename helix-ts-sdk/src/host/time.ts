// @ts-ignore: Deno global for WASM imports. Host functions are imported from "env".
import { helix_get_time as wasm_helix_get_time } from "env";
import { HostFunctionError } from "../utils/errors.ts";

/**
 * Gets the current system time from the Helix host environment.
 * The time is returned as milliseconds since the UNIX epoch.
 *
 * This function calls the `helix_get_time` host function.
 *
 * @returns A BigInt representing the number of milliseconds since the UNIX epoch.
 *          Wasmtime typically returns u64 as BigInt in JavaScript.
 * @throws HostFunctionError if the underlying host function call fails or traps.
 */
export function getTime(): bigint {
  try {
    // helix_get_time is defined in Rust as:
    // fn() -> Result<u64, Trap>
    // Wasmtime will handle the Result and trap if it's an Err.
    // u64 is typically represented as BigInt in JS when returned from WASM.
    const timeMillis = wasm_helix_get_time();
    return timeMillis;
  } catch (e: any) {
    // If wasm_helix_get_time traps, it will be caught here.
    throw new HostFunctionError(
      `Host function helix_get_time failed: ${e.message}`,
      undefined // No specific error code defined by helix_get_time itself beyond trapping
    );
  }
}