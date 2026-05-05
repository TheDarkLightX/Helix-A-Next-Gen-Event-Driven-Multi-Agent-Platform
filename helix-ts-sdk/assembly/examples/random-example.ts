import { logMessage as log } from "../host/log";
import { getRandomBytes } from "../host/random";
import { WasmHostErrorCode, BufferTooSmallError, HostFunctionError } from "../utils/errors";

// Helper function to convert Uint8Array to hex string for logging
function bytesToHexString(bytes: Uint8Array): string {
  let hex = "";
  for (let i = 0; i < bytes.length; i++) {
    let byteHex = bytes[i].toString(16);
    if (byteHex.length < 2) {
      byteHex = "0" + byteHex;
    }
    hex += byteHex;
  }
  return hex;
}

export function agent_main(): void {
  log("Random example agent started.");

  // 1. Get a small number of random bytes
  const smallBufferSize = 8;
  log("Attempting to get " + smallBufferSize.toString() + " random bytes.");
  try {
    const smallBuffer = new ArrayBuffer(smallBufferSize);
    const successSmall = getRandomBytes(smallBuffer);
    if (successSmall) {
      const randomBytesSmallView = Uint8Array.wrap(smallBuffer);
      log("Successfully retrieved " + smallBufferSize.toString() + " random bytes: 0x" + bytesToHexString(randomBytesSmallView));
    } else {
      log("Error: getRandomBytes failed for " + smallBufferSize.toString() + " bytes.");
    }
  } catch (e: any) {
    if (e instanceof BufferTooSmallError) {
      log("Error getting small random bytes (BufferTooSmallError): " + e.message);
    } else if (e instanceof HostFunctionError) {
      log("Error getting small random bytes (HostFunctionError): " + e.message + " Code: " + (e.code ? e.code.toString() : "N/A"));
    } else {
      log("An unexpected error occurred while getting small random bytes: " + e.message);
    }
  }

  // 2. Get a larger number of random bytes
  const largeBufferSize = 32;
  log("Attempting to get " + largeBufferSize.toString() + " random bytes.");
  try {
    const largeBuffer = new ArrayBuffer(largeBufferSize);
    const successLarge = getRandomBytes(largeBuffer);
    if (successLarge) {
      const randomBytesLargeView = Uint8Array.wrap(largeBuffer);
      log("Successfully retrieved " + largeBufferSize.toString() + " random bytes: 0x" + bytesToHexString(randomBytesLargeView));
    } else {
      log("Error: getRandomBytes failed for " + largeBufferSize.toString() + " bytes.");
    }
  } catch (e: any) {
    if (e instanceof BufferTooSmallError) {
      log("Error getting large random bytes (BufferTooSmallError): " + e.message);
    } else if (e instanceof HostFunctionError) {
      log("Error getting large random bytes (HostFunctionError): " + e.message + " Code: " + (e.code ? e.code.toString() : "N/A"));
    } else {
      log("An unexpected error occurred while getting large random bytes: " + e.message);
    }
  }

  // 3. Attempt to get zero random bytes
  log("Attempting to get 0 random bytes.");
  try {
    const zeroBuffer = new ArrayBuffer(0);
    const successZero = getRandomBytes(zeroBuffer); // Should return true for 0 length
    if (successZero) {
      log("Successfully called getRandomBytes(0) and it returned true, as expected.");
    } else {
      log("Error: getRandomBytes(0) returned false.");
    }
  } catch (e: any) {
      log("An unexpected error occurred while getting 0 random bytes: " + e.message);
  }

  log("Random example agent finished.");
}