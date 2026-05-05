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
export function getTime(): u64 {
  try {
    // helix_get_time is defined in Rust as:
    // fn() -> Result<u64, Trap>
    // Wasmtime will handle the Result and trap if it's an Err.
    // u64 is returned directly as u64 in AssemblyScript.
    const timeMillis = wasm_helix_get_time();
    return timeMillis;
  } catch (e: any) {
    // If wasm_helix_get_time traps, it will be caught here.
    // In AssemblyScript, catching 'any' is not standard.
    // Host function errors usually result in a trap, which aborts execution.
    // If we want to handle specific errors, the host function would need to return error codes.
    // For now, assume a trap will occur and this catch might not be effective in AS as it is in JS.
    // However, to keep structure similar and if AS has a mechanism for this:
    throw new HostFunctionError(
      // e.message might not be available in AS for all caught errors.
      `Host function helix_get_time failed.`, // Simplified message
      // Casting 'e' to 'Error' might be needed if 'e' is an object with a message.
      // u32(WasmHostErrorCode.HOST_FUNCTION_ERROR) // Example if we had error codes
    );
  }
}