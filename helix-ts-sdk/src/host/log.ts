// @ts-ignore: Deno global for WASM imports. Host functions are imported from "env".
import { helix_log_message as wasm_helix_log_message } from "env";
import { writeStringToSharedBuffer, getSharedBufferPtr } from "../utils/memory.ts";
import { HostFunctionError } from "../utils/errors.ts";

/**
 * Logs a message via the Helix host environment.
 *
 * This function writes the message to the SDK's shared buffer and then calls
 * the `helix_log_message` host function with a pointer to and length of the message
 * within that shared buffer.
 *
 * **Note:** The shared buffer must be initialized via `_helix_sdk_init_shared_buffer()`
 * during agent initialization before this function can be used.
 *
 * @param message The string message to log.
 * @throws Error if the shared buffer is not initialized or if the message is too large for the buffer.
 * @throws HostFunctionError if the underlying host function call fails (though `helix_log_message` typically doesn't return an error code).
 */
export function logMessage(message: string): void {
  try {
    const messageByteLength = writeStringToSharedBuffer(message);
    const messagePtr = getSharedBufferPtr();

    // Call the host function.
    // helix_log_message is defined in Rust as:
    // fn(mut caller: Caller<'_, HostState>, ptr: i32, len: i32)
    // It does not return a value indicating success/failure in the current host_functions.rs,
    // so we assume success if no trap occurs.
    wasm_helix_log_message(messagePtr, messageByteLength);
  } catch (e: any) {
    // Re-throw SDK-specific errors, wrap others.
    if (e instanceof Error && (e.name === "Error" || e.name === "RangeError") && e.message.includes("Shared buffer")) {
        throw e; // Propagate shared buffer related errors
    }
    // If it's not a known SDK error, wrap it as a HostFunctionError or let it propagate if it's already one.
    // This path is less likely for logMessage unless writeStringToSharedBuffer or getSharedBufferPtr throws.
    throw new HostFunctionError(
      `Failed to log message due to an SDK or host error: ${e.message}`,
      undefined // No specific error code from helix_log_message itself
    );
  }
}