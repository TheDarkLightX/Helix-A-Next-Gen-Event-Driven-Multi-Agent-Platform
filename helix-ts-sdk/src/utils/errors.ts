/**
 * Base error class for errors originating from the Helix WASM SDK or host interactions.
 */
export class HelixWasmError extends Error {
  /**
   * Optional error code, typically corresponding to codes from the host.
   */
  public code?: number;

  constructor(message: string, code?: number) {
    super(message);
    this.name = "HelixWasmError";
    this.code = code;
    // Set the prototype explicitly to allow instanceof checks
    Object.setPrototypeOf(this, HelixWasmError.prototype);
  }
}

/**
 * Error indicating that a buffer provided to a host function was too small.
 */
export class BufferTooSmallError extends HelixWasmError {
  constructor(message: string = "Buffer too small for host function call.", code?: number) {
    super(message, code);
    this.name = "BufferTooSmallError";
    Object.setPrototypeOf(this, BufferTooSmallError.prototype);
  }
}

/**
 * Error indicating that a requested value was not found by a host function.
 */
export class ValueNotFoundError extends HelixWasmError {
  constructor(message: string = "Value not found by host function.", code?: number) {
    super(message, code);
    this.name = "ValueNotFoundError";
    Object.setPrototypeOf(this, ValueNotFoundError.prototype);
  }
}

/**
 * Generic error for host function failures.
 */
export class HostFunctionError extends HelixWasmError {
    constructor(message: string = "Host function execution failed.", code?: number) {
        super(message, code);
        this.name = "HostFunctionError";
        Object.setPrototypeOf(this, HostFunctionError.prototype);
    }
}

/**
 * Known error codes from the WASM host.
 * These should align with `WasmError` in `crates/helix-wasm/src/errors.rs`.
 */
export const WasmHostErrorCode = {
  /** Generic error in host function execution. */
  HOST_FUNCTION_ERROR: -1,
  /** Requested value or resource not found. */
  VALUE_NOT_FOUND: -2,
  /** Provided buffer by guest was too small for the result. */
  BUFFER_TOO_SMALL: -3,
  /** Invalid argument provided by the guest to a host function. */
  INVALID_ARGUMENT: -4,
  /** Operation resulted in an I/O error on the host. */
  IO_ERROR: -5,
  /** Serialization or deserialization error during host-guest data exchange. */
  SERIALIZATION_ERROR: -6,
};