// Copyright 2024 Helix Platform
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


//! Utility functions for WASM operations, including serialization.

use crate::{WasmError, WasmRuntimeConfig}; // Added WasmRuntimeConfig
use serde::{Deserialize, Serialize};

/// Validates basic WASM bytecode structure (magic number and version).
pub fn validate_wasm(wasm_bytes: &[u8]) -> Result<(), WasmError> {
    if wasm_bytes.len() < 8 {
        return Err(WasmError::InvalidModule("WASM module too small".to_string()));
    }
    let magic = &wasm_bytes[0..4];
    if magic != b"\0asm" {
        return Err(WasmError::InvalidModule("Invalid WASM magic number".to_string()));
    }
    let version = u32::from_le_bytes([
        wasm_bytes[4], wasm_bytes[5], wasm_bytes[6], wasm_bytes[7]
    ]);
    if version != 1 {
        return Err(WasmError::InvalidModule(format!("Unsupported WASM version: {}", version)));
    }
    Ok(())
}

/// Serializes data to MessagePack format.
pub fn serialize_to_msgpack<T: Serialize>(data: &T) -> Result<Vec<u8>, WasmError> {
    rmp_serde::to_vec_named(data).map_err(|e| WasmError::SerializationError(format!("Msgpack serialization failed: {}", e)))
}

/// Deserializes data from MessagePack format.
pub fn deserialize_from_msgpack<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, WasmError> {
    rmp_serde::from_slice(bytes).map_err(|e| WasmError::SerializationError(format!("Msgpack deserialization failed: {}", e)))
}

/// Extract function names from WASM module (simplified placeholder).
/// In practice, a proper WASM parsing library should be used.
pub fn extract_function_names(wasm_bytes: &[u8]) -> Result<Vec<String>, WasmError> {
    validate_wasm(wasm_bytes)?; // Ensure basic validity first
    // This is a highly simplified placeholder.
    // A real implementation would parse the export section of the WASM module.
    Ok(vec![
        "agent_init".to_string(), // Example expected exports for an agent
        "agent_run".to_string(),
        "agent_transform".to_string(),
        "agent_execute".to_string(),
        "agent_stop".to_string(),
    ])
}

/// Estimate execution cost of WASM module (simplified placeholder).
/// In practice, this would involve more sophisticated static analysis or metering.
pub fn estimate_execution_cost(wasm_bytes: &[u8]) -> Result<u64, WasmError> {
    validate_wasm(wasm_bytes)?;
    // Simple cost estimation based on module size. This is a very rough proxy.
    // Real cost analysis would look at instruction types, loops, etc.
    Ok(wasm_bytes.len() as u64) // Simplified: 1 byte = 1 unit of cost
}

/// Check if WASM module is safe to execute based on preliminary checks (simplified placeholder).
pub fn is_safe_module(wasm_bytes: &[u8], config: &WasmRuntimeConfig) -> Result<bool, WasmError> {
    validate_wasm(wasm_bytes)?;
    
    // Example: Check against max_instructions if it's used as a proxy for complexity/size.
    // This is not a direct fuel cost but can be a simple size-based heuristic.
    let estimated_complexity = wasm_bytes.len() as u64; // Using size as complexity proxy
    if estimated_complexity > config.max_instructions { // Assuming max_instructions can be a general complexity limit
        // tracing::warn!("Module considered unsafe due to size/complexity exceeding configured limit.");
        return Ok(false);
    }

    // TODO: Add more sophisticated safety checks:
    // - Analyze imports: Ensure only allowed host functions are imported.
    //   This is usually handled by the Linker configuration (only linking allowed functions).
    // - Static analysis for known dangerous patterns (if feasible and tools exist).
    // - For WASI, ensure filesystem/network access aligns with sandbox config (done at WasiCtx setup).
    
    Ok(true)
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    struct TestStruct {
        id: u32,
        name: String,
        value: f64,
    }

    #[test]
    fn test_wasm_validation() {
        let valid_wasm = b"\0asm\x01\x00\x00\x00";
        assert!(validate_wasm(valid_wasm).is_ok());

        let invalid_magic = b"wasm\x01\x00\x00\x00";
        assert!(validate_wasm(invalid_magic).is_err());

        let invalid_version = b"\0asm\x02\x00\x00\x00";
        assert!(validate_wasm(invalid_version).is_err());

        let too_short = b"\0as";
        assert!(validate_wasm(too_short).is_err());
    }

    #[test]
    fn test_extract_function_names_placeholder() {
        let valid_wasm = b"\0asm\x01\x00\x00\x00"; // Minimal valid wasm
        let names = extract_function_names(valid_wasm).unwrap();
        assert!(names.contains(&"agent_run".to_string())); // Check one of the placeholder names
    }

    #[test]
    fn test_estimate_execution_cost_placeholder() {
        let wasm_data = b"\0asm\x01\x00\x00\x00some_code";
        let cost = estimate_execution_cost(wasm_data).unwrap();
        assert_eq!(cost, wasm_data.len() as u64);
    }

    #[test]
    fn test_is_safe_module_placeholder() {
        let mut config = WasmRuntimeConfig::default();
        config.max_instructions = 100; // Set a limit for complexity/size

        let small_wasm = b"\0asm\x01\x00\x00\x00"; // Very small
        assert!(is_safe_module(small_wasm, &config).unwrap());

        let large_wasm_data: Vec<u8> = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
            .into_iter()
            .chain((0..200).map(|_| 0u8)) // Add 200 dummy bytes
            .collect();
        assert!(!is_safe_module(&large_wasm_data, &config).unwrap());
    }


    #[test]
    fn test_msgpack_serialization_deserialization() {
        let original = TestStruct {
            id: 123,
            name: "Test".to_string(),
            value: 45.67,
        };

        let serialized = serialize_to_msgpack(&original).unwrap();
        assert!(!serialized.is_empty());

        let deserialized: TestStruct = deserialize_from_msgpack(&serialized).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_deserialize_invalid_msgpack() {
        let invalid_bytes = vec![0x01, 0x02, 0x03]; // Not valid msgpack for TestStruct
        let result: Result<TestStruct, _> = deserialize_from_msgpack(&invalid_bytes);
        assert!(result.is_err());
    }
}