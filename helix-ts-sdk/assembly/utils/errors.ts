/**
 * Base error class for errors originating from the Helix WASM SDK or host interactions.
 */
export class HelixWasmError extends Error {
  /**
   * Error code, typically corresponding to codes from the host.
   * Defaults to 0 if not specified, indicating success or no specific host error code.
   */
  public code: number;

  constructor(message: string, code: number = 0) { // Default code to 0
    super(message);
    this.name = "HelixWasmError";
    this.code = code;
    // Object.setPrototypeOf is not standard in AssemblyScript and often not needed
    // for instanceof to work with custom errors if class hierarchy is clear.
  }
}

/**
 * Error indicating that a buffer provided to a host function was too small.
 */
export class BufferTooSmallError extends HelixWasmError {
  constructor(message: string = "Buffer too small for host function call.", code: number = WasmHostErrorCode.BUFFER_TOO_SMALL) {
    super(message, code);
    this.name = "BufferTooSmallError";
  }
}

/**
 * Error indicating that a requested value was not found by a host function.
 */
export class ValueNotFoundError extends HelixWasmError {
  constructor(message: string = "Value not found by host function.", code: number = WasmHostErrorCode.VALUE_NOT_FOUND) {
    super(message, code);
    this.name = "ValueNotFoundError";
  }
}

/**
 * Generic error for host function failures.
 */
export class HostFunctionError extends HelixWasmError {
    constructor(message: string = "Host function execution failed.", code: number = WasmHostErrorCode.HOST_FUNCTION_ERROR) {
        super(message, code);
        this.name = "HostFunctionError";
    }
}

/**
 * Known error codes from the WASM host.
 * These should align with `WasmError` in `crates/helix-wasm/src/errors.rs`.
 */
export enum WasmHostErrorCode {
  /** Generic error in host function execution. */
  HOST_FUNCTION_ERROR = -1,
  /** Requested value or resource not found. */
  VALUE_NOT_FOUND = -2,
  /** Provided buffer by guest was too small for the result. */
  BUFFER_TOO_SMALL = -3,
  /** Invalid argument provided by the guest to a host function. */
  INVALID_ARGUMENT = -4,
  /** Operation resulted in an I/O error on the host. */
  IO_ERROR = -5,
  /** Serialization or deserialization error during host-guest data exchange. */
  SERIALIZATION_ERROR = -6,
  // Adding a SUCCESS code for clarity, though host functions often return 0 or positive length for success.
  // This is more for SDK-internal logic if needed, not directly from host error returns.
  SUCCESS = 0
}