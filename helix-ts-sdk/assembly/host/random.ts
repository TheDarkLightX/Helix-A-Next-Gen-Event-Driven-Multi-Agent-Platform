// @ts-ignore: decorator
@external("env", "host_random_get_bytes")
declare function host_random_get_bytes(buf_ptr: usize, buf_len: usize): i32; // Returns 0 on success, error code otherwise

/**
 * Fills the provided buffer with random bytes.
 * The host is expected to provide cryptographically secure random numbers if required.
 * @param buffer The ArrayBuffer to fill with random bytes.
 * @returns True if successful, false otherwise.
 */
export function getRandomBytes(buffer: ArrayBuffer): bool {
  if (buffer.byteLength == 0) {
    return true; // Nothing to fill
  }
  const result = host_random_get_bytes(changetype<usize>(buffer), buffer.byteLength);
  return result == 0;
}

/**
 * Generates a random u64 number.
 * Note: This is a convenience function. For multiple random numbers or specific distributions,
 * consider using getRandomBytes and then processing the buffer.
 * @returns A u64 random number, or 0 if an error occurs.
 */
export function getRandomU64(): u64 {
  const buffer = new ArrayBuffer(8);
  if (!getRandomBytes(buffer)) {
    return 0; // Error getting random bytes
  }
  // Assuming little-endian, adjust if host provides big-endian
  return load<u64>(changetype<usize>(buffer));
}

/**
 * Generates a random f64 number between 0 (inclusive) and 1 (exclusive).
 * Note: This is a basic implementation for convenience.
 * For more robust or cryptographically secure random floating point numbers,
 * ensure the host's `host_random_get_bytes` is suitable and consider more advanced generation techniques.
 * @returns A f64 random number, or NaN if an error occurs.
 */
export function getRandomF64(): f64 {
  const randomU64 = getRandomU64();
  if (randomU64 == 0 && !getRandomBytes(new ArrayBuffer(1))) { // Check if getRandomU64 failed due to underlying error
      return NaN;
  }
  // Normalize u64 to f64 in [0, 1)
  // 0x1.0p-53 is 2^-53, the smallest f64 > 0 such that 1.0 + 2^-53 != 1.0
  return (randomU64 as f64) * (1.0 / (U64.MAX_VALUE as f64 + 1.0));
}