const encoder = new TextEncoder();
const decoder = new TextDecoder();

/**
 * Encodes a TypeScript string into a UTF-8 Uint8Array.
 * @param str The string to encode.
 * @returns A Uint8Array containing the UTF-8 encoded string.
 */
export function encodeStringToUtf8(str: string): Uint8Array {
  return encoder.encode(str);
}

/**
 * Decodes a UTF-8 Uint8Array (or a slice of it) into a TypeScript string.
 * @param buffer The Uint8Array containing UTF-8 data.
 * @param ptr Optional offset into the buffer. Defaults to 0.
 * @param len Optional length of the slice to decode. Defaults to buffer.byteLength - ptr.
 * @returns The decoded string.
 */
export function decodeToString(
  buffer: Uint8Array,
  ptr: number = 0,
  len?: number,
): string {
  const sliceLength = len === undefined ? buffer.byteLength - ptr : len;
  if (ptr < 0 || sliceLength < 0 || ptr + sliceLength > buffer.byteLength) {
    throw new RangeError("Invalid pointer or length for buffer slice.");
  }
  return decoder.decode(buffer.subarray(ptr, ptr + sliceLength));
}

// --- WASM Memory Interaction ---

let wasmMemoryU8View: Uint8Array | null = null;

/**
 * Initializes the memory utility with the WASM instance's linear memory.
 * This should be called once the WASM module is instantiated and its memory is accessible.
 * @param memory The WebAssembly.Memory instance exported by the WASM module or obtained from the instance.
 */
export function initWasmMemory(memory: WebAssembly.Memory): void {
  if (!(memory instanceof WebAssembly.Memory)) {
    throw new Error("Invalid argument: WebAssembly.Memory instance expected.");
  }
  wasmMemoryU8View = new Uint8Array(memory.buffer);
  console.log(`TypeScript SDK: WASM memory initialized. Buffer size: ${memory.buffer.byteLength} bytes.`);
}

/**
 * Ensures that the WASM memory has been initialized and returns the Uint8Array view.
 * @throws Error if memory is not initialized.
 */
function ensureMemoryInitialized(): Uint8Array {
  if (!wasmMemoryU8View) {
    throw new Error(
      "WASM memory not initialized. Call initWasmMemory() with the WASM instance's memory first.",
    );
  }
  return wasmMemoryU8View;
}

/**
 * Reads a string directly from the WASM linear memory.
 * @param ptr The pointer (offset) in WASM memory where the string starts.
 * @param len The length of the string in bytes.
 * @returns The decoded string.
 * @throws RangeError if pointer/length is out of bounds.
 */
export function readStringFromWasmMemory(ptr: number, len: number): string {
  const memory = ensureMemoryInitialized();
  if (ptr < 0 || len < 0 || ptr + len > memory.byteLength) {
    throw new RangeError(`Invalid pointer or length for WASM memory access. ptr: ${ptr}, len: ${len}, memorySize: ${memory.byteLength}`);
  }
  return decoder.decode(memory.subarray(ptr, ptr + len));
}

/**
 * Writes a string directly to the WASM linear memory at a given pointer.
 * The caller must ensure the memory at `ptr` is valid and sufficient.
 * @param ptr The pointer (offset) in WASM memory where the string should be written.
 * @param str The string to write.
 * @returns The number of bytes written.
 * @throws RangeError if pointer is out of bounds or buffer is too small.
 */
export function writeStringToWasmMemory(ptr: number, str: string): number {
  const memory = ensureMemoryInitialized();
  const encodedString = encoder.encode(str);
  if (ptr < 0 || ptr + encodedString.byteLength > memory.byteLength) {
    throw new RangeError(`Invalid pointer or string too large for WASM memory access. ptr: ${ptr}, strLen: ${encodedString.byteLength}, memorySize: ${memory.byteLength}`);
  }
  memory.set(encodedString, ptr);
  return encodedString.byteLength;
}


// --- Shared Buffer Management for Host Communication ---
// This buffer is intended to be a region within the WASM module's own linear memory,
// which the SDK uses for passing data to/from host functions that require a guest-provided buffer.

// Default size for the shared buffer. Can be overridden by agent if needed.
const DEFAULT_SHARED_BUFFER_SIZE = 4096;
let sharedBufferPtr: number = 0;
let currentSharedBufferSize: number = 0;

/**
 * Initializes a dedicated shared buffer within WASM linear memory.
 * This function should be called by the agent's initialization code (e.g., in `_helix_agent_init`).
 * It requires a pointer to a region of memory that the WASM module itself manages/allocates.
 *
 * @param ptr The pointer to the allocated shared buffer region in WASM memory.
 * @param size The size of the allocated shared buffer region.
 * @throws Error if memory is not initialized or if buffer region is invalid.
 */
export function _helix_sdk_init_shared_buffer(ptr: number, size: number = DEFAULT_SHARED_BUFFER_SIZE): void {
  const memory = ensureMemoryInitialized();
  if (ptr < 0 || size <= 0 || ptr + size > memory.byteLength) {
    throw new RangeError(
      `Shared buffer pointer or size is out of bounds. ptr: ${ptr}, size: ${size}, memorySize: ${memory.byteLength}`
    );
  }
  sharedBufferPtr = ptr;
  currentSharedBufferSize = size;
  // Optionally clear the buffer on init
  memory.fill(0, sharedBufferPtr, sharedBufferPtr + currentSharedBufferSize);
  console.log(`TypeScript SDK: Shared buffer initialized at offset ${ptr} with size ${size} bytes.`);
}

/**
 * Gets the pointer to the shared buffer for host communication.
 * @returns The pointer to the shared buffer.
 * @throws Error if the shared buffer has not been initialized.
 */
export function getSharedBufferPtr(): number {
  if (sharedBufferPtr === 0 && currentSharedBufferSize === 0) {
    throw new Error(
      "Shared buffer not initialized. Call _helix_sdk_init_shared_buffer() from your agent's init code, providing a pointer and size for a buffer within WASM memory."
    );
  }
  return sharedBufferPtr;
}

/**
 * Gets the current size of the shared buffer.
 * @returns The size of the shared buffer in bytes.
 */
export function getSharedBufferSize(): number {
  if (currentSharedBufferSize === 0) {
     throw new Error("Shared buffer not initialized.");
  }
  return currentSharedBufferSize;
}

/**
 * Reads a string from the shared buffer, typically after a host function has written to it.
 * @param len The number of bytes written by the host (usually returned by the host function).
 * @returns The decoded string.
 * @throws Error if len is larger than the shared buffer size or if buffer not initialized.
 */
export function readStringFromSharedBuffer(len: number): string {
  const memory = ensureMemoryInitialized();
  const ptr = getSharedBufferPtr(); // Ensures buffer is initialized
  const bufferSize = getSharedBufferSize();

  if (len < 0) {
    throw new RangeError("Length cannot be negative.");
  }
  if (len > bufferSize) {
    throw new Error(
      `Attempted to read ${len} bytes from shared buffer, but its total size is ${bufferSize}.`
    );
  }
  return decodeToString(memory, ptr, len);
}

/**
 * Writes a string into the shared buffer. This can be used to prepare data
 * that a host function might read, or for the guest to pass complex string data
 * where the host expects a ptr/len to a string in guest memory.
 * @param str The string to write.
 * @returns The number of bytes written.
 * @throws Error if the string is too large for the shared buffer or if buffer not initialized.
 */
export function writeStringToSharedBuffer(str: string): number {
  const memory = ensureMemoryInitialized();
  const ptr = getSharedBufferPtr(); // Ensures buffer is initialized
  const bufferSize = getSharedBufferSize();
  const encodedString = encoder.encode(str);

  if (encodedString.byteLength > bufferSize) {
    throw new Error(
      `String too large for shared buffer. Max size: ${bufferSize}, String size: ${encodedString.byteLength}`
    );
  }
  memory.set(encodedString, ptr);
  // Optionally clear remaining part of the buffer
  if (encodedString.byteLength < bufferSize) {
    memory.fill(0, ptr + encodedString.byteLength, ptr + bufferSize);
  }
  return encodedString.byteLength;
}

// Note: True dynamic allocation (`malloc`/`free`-like behavior) within the WASM module
// from TypeScript is complex with `deno compile`'s current capabilities without
// linking against a C allocator or using AssemblyScript for memory management.
// For now, the shared buffer approach (with a fixed or agent-defined size) is primary.
// Agent developers will need to ensure `_helix_sdk_init_shared_buffer` is called
// with a valid pointer to memory they control within their WASM module.
// This pointer could point to a static array in their TS code, for example.