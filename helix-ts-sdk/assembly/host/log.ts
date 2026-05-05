// @ts-ignore: decorator
@external("env", "helix_log_message")
declare function wasm_helix_log_message(ptr: i32, len: i32): void; // Assuming void return based on usage

import { writeStringToSharedBuffer, getSharedBufferPtr } from "../utils/memory";
import { HostFunctionError } from "../utils/errors";

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
export function logMessage(message: string): void { // Changed String to string for consistency
  // Removing try...catch to see if it resolves AS100.
  // Errors from writeStringToSharedBuffer or getSharedBufferPtr will propagate.
  const messageByteLength = writeStringToSharedBuffer(message);
  const messagePtr = getSharedBufferPtr();

  // Call the host function.
  // helix_log_message is defined in Rust as:
  // fn(mut caller: Caller<'_, HostState>, ptr: i32, len: i32)
  // It does not return a value indicating success/failure in the current host_functions.rs,
  // so we assume success if no trap occurs.
  wasm_helix_log_message(messagePtr, messageByteLength);
  // If wasm_helix_log_message could trap and we wanted to report,
  // we'd need it to return a code, or the try...catch would be essential.
  // For now, assuming it doesn't trap in a way AS needs to catch for this simple log.
}