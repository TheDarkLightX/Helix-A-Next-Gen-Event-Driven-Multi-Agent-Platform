// @ts-ignore: Deno global for WASM imports. Host functions are imported from "env".
import { helix_get_config_value as wasm_helix_get_config_value } from "env";
import {
  writeStringToSharedBuffer,
  readStringFromSharedBuffer,
  getSharedBufferPtr,
  getSharedBufferSize,
} from "../utils/memory.ts";
import {
  HostFunctionError,
  BufferTooSmallError,
  ValueNotFoundError,
  WasmHostErrorCode,
} from "../utils/errors.ts";

/**
 * Retrieves a configuration value from the Helix host environment for the current agent.
 *
 * The key is written to the shared buffer, and its pointer and length are passed to the host.
 * The host then writes the JSON serialized config value (if found) back into the shared buffer.
 * The SDK reads this JSON string and parses it.
 *
 * **Note:** The shared buffer must be initialized via `_helix_sdk_init_shared_buffer()`
 * during agent initialization before this function can be used.
 *
 * @param key The configuration key to retrieve.
 * @returns The parsed configuration value, or `null` if the key is not found.
 * @template T The expected type of the configuration value.
 * @throws BufferTooSmallError if the shared buffer is too small for the key or the returned value.
 * @throws ValueNotFoundError if the configuration key is not found.
 * @throws HostFunctionError for other host-side errors.
 * @throws Error if the shared buffer is not initialized.
 */
export function getConfigValue<T>(key: string): T | null {
  let keyPtr: number;
  let keyByteLength: number;

  try {
    // Write the key to the shared buffer first to pass it to the host.
    // This assumes the key itself isn't excessively long for the shared buffer.
    // If keys can be very long, a separate, smaller buffer for outgoing short strings might be needed,
    // or the host API would need to change to accept key strings directly if the ABI supports it well.
    keyByteLength = writeStringToSharedBuffer(key);
    keyPtr = getSharedBufferPtr();
  } catch (e: any) {
    if (e instanceof Error && e.message.includes("String too large for shared buffer")) {
      throw new BufferTooSmallError(`Configuration key "${key}" is too large for the shared SDK buffer.`, WasmHostErrorCode.BUFFER_TOO_SMALL);
    }
    throw e; // Re-throw other shared buffer errors (e.g., not initialized)
  }

  const resultBufferPtr = getSharedBufferPtr(); // Host will write the value here
  const resultBufferSize = getSharedBufferSize();

  // Call the host function.
  // helix_get_config_value is defined in Rust as:
  // fn(mut caller: Caller<'_, HostState>, key_ptr: i32, key_len: i32, result_buf_ptr: i32, result_buf_len: i32) -> Result<i32, Trap>
  // It returns the number of bytes written, or an error code.
  const bytesWrittenOrErrorCode = wasm_helix_get_config_value(
    keyPtr,
    keyByteLength,
    resultBufferPtr,
    resultBufferSize,
  );

  if (bytesWrittenOrErrorCode === WasmHostErrorCode.VALUE_NOT_FOUND) {
    return null;
  }
  if (bytesWrittenOrErrorCode === WasmHostErrorCode.BUFFER_TOO_SMALL) {
    // This indicates the host's result_buf_len was too small for the config value.
    throw new BufferTooSmallError(
      `Shared buffer (size: ${resultBufferSize}) is too small for the configuration value associated with key "${key}".`,
      bytesWrittenOrErrorCode
    );
  }
  if (bytesWrittenOrErrorCode < 0) {
    // Other negative values are generic host errors.
    throw new HostFunctionError(
      `Host function helix_get_config_value failed for key "${key}" with error code: ${bytesWrittenOrErrorCode}.`,
      bytesWrittenOrErrorCode
    );
  }

  // Success, bytesWrittenOrErrorCode contains the length of the JSON string.
  try {
    const valueJson = readStringFromSharedBuffer(bytesWrittenOrErrorCode);
    return JSON.parse(valueJson) as T;
  } catch (e: any) {
    throw new HostFunctionError(
      `Failed to parse configuration value for key "${key}" from JSON: ${e.message}. Received JSON: "${readStringFromSharedBuffer(bytesWrittenOrErrorCode)}"`,
      WasmHostErrorCode.HOST_FUNCTION_ERROR // Or a new specific parsing error code
    );
  }
}