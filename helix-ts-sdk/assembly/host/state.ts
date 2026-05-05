// Host function declarations using @external
// @ts-ignore: decorator
@external("env", "helix_state_get_data")
declare function wasm_helix_state_get_data(key_ptr: i32, key_len: i32, value_buf_ptr: i32, value_buf_len: i32): i32;

// @ts-ignore: decorator
@external("env", "helix_state_set_data")
declare function wasm_helix_state_set_data(key_ptr: i32, key_len: i32, value_ptr: i32, value_len: i32): i32;

// @ts-ignore: decorator
@external("env", "helix_state_delete_data")
declare function wasm_helix_state_delete_data(key_ptr: i32, key_len: i32): i32;

import {
  writeStringToSharedBuffer,
  readStringFromSharedBuffer,
  getSharedBufferPtr,
  getSharedBufferSize,
} from "../utils/memory"; // .ts extension removed
import {
  HostFunctionError,
  BufferTooSmallError,
  ValueNotFoundError,
  WasmHostErrorCode,
} from "../utils/errors"; // .ts extension removed

/**
 * Retrieves a state value by its key using the shared buffer.
 * @param key The key of the state value to retrieve.
 * @returns The state value as a String, or `null` if not found.
 *          Returns an empty string if the state value exists but is empty.
 * @throws BufferTooSmallError, ValueNotFoundError, HostFunctionError, Error.
 */
export function getState(key: string): String | null {
  let keyPtr: number;
  let keyByteLength: number;

  try {
    keyByteLength = writeStringToSharedBuffer(key);
    keyPtr = getSharedBufferPtr();
  } catch (e: any) {
    if (e instanceof Error && e.message.includes("String too large for shared buffer")) {
      throw new BufferTooSmallError(`State key "${key}" is too large for the shared SDK buffer.`, WasmHostErrorCode.BUFFER_TOO_SMALL);
    }
    if (e instanceof Error && e.message.includes("Shared buffer not initialized")) {
        throw e;
    }
    throw new HostFunctionError(`Error preparing state key "${key}" for host call: ${e.message}`);
  }

  const valueBufferPtr = getSharedBufferPtr();
  const valueBufferSize = getSharedBufferSize();

  const bytesWrittenOrErrorCode = wasm_helix_state_get_data(
    keyPtr,
    keyByteLength,
    valueBufferPtr,
    valueBufferSize,
  );

  if (bytesWrittenOrErrorCode === WasmHostErrorCode.VALUE_NOT_FOUND) {
    return null;
  }
  if (bytesWrittenOrErrorCode === WasmHostErrorCode.BUFFER_TOO_SMALL) {
    throw new BufferTooSmallError(
      `Shared buffer (size: ${valueBufferSize}) is too small for the state value associated with key "${key}".`,
      bytesWrittenOrErrorCode
    );
  }
  if (bytesWrittenOrErrorCode < 0) {
    throw new HostFunctionError(
      `Host function helix_state_get_data failed for key "${key}" with error code: ${bytesWrittenOrErrorCode}.`,
      bytesWrittenOrErrorCode
    );
  }

  if (bytesWrittenOrErrorCode == 0) {
    return ""; // Key exists but value is empty
  }

  try {
    return readStringFromSharedBuffer(bytesWrittenOrErrorCode);
  } catch (e: any) {
    throw new HostFunctionError(
      `Failed to read or decode state value for key "${key}": ${e.message}. Bytes written by host: ${bytesWrittenOrErrorCode}`,
      WasmHostErrorCode.SERIALIZATION_ERROR
    );
  }
}

/**
 * Sets a state value for a given key using the shared buffer.
 * @param key The key of the state value to set.
 * @param value The string value to set.
 * @returns True if successful, false otherwise (though typically throws on error).
 * @throws BufferTooSmallError, HostFunctionError, Error.
 */
export function setState(key: string, value: string): bool {
  let keyPtr: number;
  let keyByteLength: number;
  let valuePtr: number;
  let valueByteLength: number;

  try {
    keyByteLength = writeStringToSharedBuffer(key);
    keyPtr = getSharedBufferPtr();
  } catch (e: any) {
    if (e instanceof Error && e.message.includes("String too large for shared buffer")) {
      throw new BufferTooSmallError(`State key "${key}" is too large for the shared SDK buffer.`, WasmHostErrorCode.BUFFER_TOO_SMALL);
    }
    if (e instanceof Error && e.message.includes("Shared buffer not initialized")) {
        throw e;
    }
    throw new HostFunctionError(`Error preparing state key "${key}" for host call: ${e.message}`);
  }

  try {
    valueByteLength = writeStringToSharedBuffer(value);
    valuePtr = getSharedBufferPtr();
  } catch (e: any) {
     if (e instanceof Error && e.message.includes("String too large for shared buffer")) {
      throw new BufferTooSmallError(`State value for key "${key}" is too large for the shared SDK buffer.`, WasmHostErrorCode.BUFFER_TOO_SMALL);
    }
     if (e instanceof Error && e.message.includes("Shared buffer not initialized")) {
        throw e;
    }
    throw new HostFunctionError(`Error preparing state value for key "${key}" for host call: ${e.message}`);
  }

  // Assuming host reads key then value, allowing shared buffer reuse.
  const resultCode = wasm_helix_state_set_data(
    keyPtr,
    keyByteLength,
    valuePtr,
    valueByteLength
  );

  if (resultCode < 0) {
    throw new HostFunctionError(
      `Host function helix_state_set_data failed for key "${key}" with error code: ${resultCode}.`,
      resultCode
    );
  }
  return resultCode == 0;
}

/**
 * Deletes a state value by its key using the shared buffer.
 * @param key The key of the state value to delete.
 * @returns True if successful, false otherwise (though typically throws on error).
 * @throws BufferTooSmallError, HostFunctionError, Error.
 */
export function deleteState(key: string): bool {
  let keyPtr: number;
  let keyByteLength: number;

  try {
    keyByteLength = writeStringToSharedBuffer(key);
    keyPtr = getSharedBufferPtr();
  } catch (e: any) {
    if (e instanceof Error && e.message.includes("String too large for shared buffer")) {
      throw new BufferTooSmallError(`State key "${key}" is too large for the shared SDK buffer.`, WasmHostErrorCode.BUFFER_TOO_SMALL);
    }
    if (e instanceof Error && e.message.includes("Shared buffer not initialized")) {
        throw e;
    }
    throw new HostFunctionError(`Error preparing state key "${key}" for host call: ${e.message}`);
  }

  const resultCode = wasm_helix_state_delete_data(keyPtr, keyByteLength);

  if (resultCode < 0) {
     throw new HostFunctionError(
      `Host function helix_state_delete_data failed for key "${key}" with error code: ${resultCode}.`,
      resultCode
    );
  }
  return resultCode == 0;
}