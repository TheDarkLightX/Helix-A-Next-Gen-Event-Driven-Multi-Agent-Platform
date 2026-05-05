// @ts-ignore: Deno global for WASM imports. Host functions are imported from "env".
import { helix_get_credential as wasm_helix_get_credential } from "env";
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
 * Retrieves a credential value from the Helix host environment by its name.
 *
 * The credential name is written to the shared buffer. The host then writes
 * the credential value back into the shared buffer.
 *
 * @param name The name of the credential to retrieve.
 * @returns The credential value as a String, or `null` if not found.
 *          Returns an empty string if the credential exists but is empty.
 * @throws BufferTooSmallError if the shared buffer is too small for the name or the returned value.
 * @throws ValueNotFoundError if the credential name is not found.
 * @throws HostFunctionError for other host-side errors.
 * @throws Error if the shared buffer is not initialized.
 */
export function getCredential(name: string): String | null {
  let namePtr: number;
  let nameByteLength: number;

  try {
    nameByteLength = writeStringToSharedBuffer(name);
    namePtr = getSharedBufferPtr();
  } catch (e: any) {
    if (e instanceof Error && e.message.includes("String too large for shared buffer")) {
      throw new BufferTooSmallError(`Credential name "${name}" is too large for the shared SDK buffer.`, WasmHostErrorCode.BUFFER_TOO_SMALL);
    }
    // Assuming 'assert' in writeStringToSharedBuffer throws Error for not initialized
    if (e instanceof Error && e.message.includes("Shared buffer not initialized")) {
        throw e; // Re-throw initialization error
    }
    throw new HostFunctionError(`Error preparing credential name "${name}" for host call: ${e.message}`);
  }

  const valueBufferPtr = getSharedBufferPtr(); // Host will write the value here, potentially overwriting name
  const valueBufferSize = getSharedBufferSize();

  // Call the host function.
  // Expected signature: fn(name_ptr: i32, name_len: i32, value_buf_ptr: i32, value_buf_len: i32) -> Result<i32, Trap>
  // Returns bytes written for value, or an error code.
  const bytesWrittenOrErrorCode = wasm_helix_get_credential(
    namePtr,
    nameByteLength,
    valueBufferPtr, // Host writes value here
    valueBufferSize,
  );

  if (bytesWrittenOrErrorCode === WasmHostErrorCode.VALUE_NOT_FOUND) {
    return null;
  }
  if (bytesWrittenOrErrorCode === WasmHostErrorCode.BUFFER_TOO_SMALL) {
    throw new BufferTooSmallError(
      `Shared buffer (size: ${valueBufferSize}) is too small for the credential value associated with name "${name}".`,
      bytesWrittenOrErrorCode
    );
  }
  if (bytesWrittenOrErrorCode < 0) { // Other negative values are generic host errors
    throw new HostFunctionError(
      `Host function helix_get_credential failed for name "${name}" with error code: ${bytesWrittenOrErrorCode}.`,
      bytesWrittenOrErrorCode
    );
  }

  // Success, bytesWrittenOrErrorCode contains the length of the credential value string.
  if (bytesWrittenOrErrorCode == 0) {
    return ""; // Credential exists but is empty
  }

  try {
    const valueStr = readStringFromSharedBuffer(bytesWrittenOrErrorCode);
    return valueStr;
  } catch (e: any) {
    // This catch is for errors from readStringFromSharedBuffer (e.g., invalid UTF-8 if host wrote bad data)
    throw new HostFunctionError(
      `Failed to read or decode credential value for name "${name}": ${e.message}. Bytes written by host: ${bytesWrittenOrErrorCode}`,
      WasmHostErrorCode.SERIALIZATION_ERROR
    );
  }
}