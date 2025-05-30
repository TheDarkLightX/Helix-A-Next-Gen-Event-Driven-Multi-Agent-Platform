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


//! Zero-knowledge proof types and utilities

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::errors::ZkVmError;

/// A zero-knowledge proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkProof {
    /// Proof system used (STARK, SNARK, etc.)
    pub system: ProofSystem,
    /// The actual proof data
    pub proof_data: Vec<u8>,
    /// Public inputs/outputs
    pub public_inputs: Vec<u8>,
    /// Verification key or circuit identifier
    pub verification_key: Vec<u8>,
    /// Metadata about the proof
    pub metadata: ProofMetadata,
}

/// Supported proof systems
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProofSystem {
    /// STARK (Scalable Transparent ARgument of Knowledge)
    Stark,
    /// SNARK (Succinct Non-interactive ARgument of Knowledge)
    Snark,
    /// Plonk
    Plonk,
    /// Groth16
    Groth16,
    /// Custom proof system
    Custom(String),
}

/// Metadata about a proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofMetadata {
    /// When the proof was generated
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Program or circuit that generated the proof
    pub program_id: String,
    /// Proof generation time in milliseconds
    pub generation_time_ms: u64,
    /// Size of the proof in bytes
    pub proof_size: usize,
    /// Number of constraints in the circuit
    pub constraint_count: Option<u64>,
    /// Additional metadata
    pub extra: HashMap<String, serde_json::Value>,
}

/// Request to generate a proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofRequest {
    /// Program or circuit to execute
    pub program: Vec<u8>,
    /// Private inputs
    pub private_inputs: Vec<u8>,
    /// Public inputs
    pub public_inputs: Vec<u8>,
    /// Proof system to use
    pub system: ProofSystem,
    /// Additional parameters
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Response from proof generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofResponse {
    /// Generated proof
    pub proof: ZkProof,
    /// Execution output
    pub output: Vec<u8>,
    /// Execution statistics
    pub stats: ExecutionStats,
}

/// Statistics from proof generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    /// Total execution time
    pub total_time_ms: u64,
    /// Time spent on witness generation
    pub witness_time_ms: u64,
    /// Time spent on proof generation
    pub proof_time_ms: u64,
    /// Memory usage in bytes
    pub memory_used: u64,
    /// Number of execution cycles
    pub cycles: u64,
}

impl ZkProof {
    /// Create a new zero-knowledge proof
    pub fn new(
        system: ProofSystem,
        proof_data: Vec<u8>,
        public_inputs: Vec<u8>,
        verification_key: Vec<u8>,
        program_id: String,
    ) -> Self {
        Self {
            system,
            proof_data: proof_data.clone(),
            public_inputs,
            verification_key,
            metadata: ProofMetadata {
                created_at: chrono::Utc::now(),
                program_id,
                generation_time_ms: 0,
                proof_size: proof_data.len(),
                constraint_count: None,
                extra: HashMap::new(),
            },
        }
    }

    /// Get the size of the proof in bytes
    pub fn size(&self) -> usize {
        self.proof_data.len()
    }

    /// Check if the proof is valid (basic structure validation)
    pub fn is_valid(&self) -> bool {
        !self.proof_data.is_empty() && !self.verification_key.is_empty()
    }

    /// Get a hash of the proof for identification
    pub fn hash(&self) -> Vec<u8> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(&self.proof_data);
        hasher.update(&self.public_inputs);
        hasher.finalize().to_vec()
    }

    /// Serialize the proof to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, ZkVmError> {
        serde_json::to_vec(self)
            .map_err(|e| ZkVmError::SerializationError(e.to_string()))
    }

    /// Deserialize a proof from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, ZkVmError> {
        serde_json::from_slice(data)
            .map_err(|e| ZkVmError::SerializationError(e.to_string()))
    }
}

impl ProofRequest {
    /// Create a new proof request
    pub fn new(
        program: Vec<u8>,
        private_inputs: Vec<u8>,
        public_inputs: Vec<u8>,
        system: ProofSystem,
    ) -> Self {
        Self {
            program,
            private_inputs,
            public_inputs,
            system,
            parameters: HashMap::new(),
        }
    }

    /// Add a parameter to the request
    pub fn with_parameter(mut self, key: String, value: serde_json::Value) -> Self {
        self.parameters.insert(key, value);
        self
    }

    /// Validate the request
    pub fn validate(&self) -> Result<(), ZkVmError> {
        if self.program.is_empty() {
            return Err(ZkVmError::InvalidInput("Program cannot be empty".to_string()));
        }

        // Additional validation based on proof system
        match self.system {
            ProofSystem::Stark => {
                // STARK-specific validation
            }
            ProofSystem::Snark => {
                // SNARK-specific validation
            }
            _ => {
                // Generic validation
            }
        }

        Ok(())
    }
}

/// Utility functions for proof operations
pub mod utils {
    use super::*;

    /// Generate a commitment to private inputs
    pub fn commit_private_inputs(inputs: &[u8]) -> Vec<u8> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(inputs);
        hasher.finalize().to_vec()
    }

    /// Verify that public inputs match a commitment
    pub fn verify_commitment(
        _public_inputs: &[u8],
        commitment: &[u8],
        private_inputs: &[u8],
    ) -> bool {
        let computed_commitment = commit_private_inputs(private_inputs);
        computed_commitment == commitment
    }

    /// Estimate proof size based on circuit complexity
    pub fn estimate_proof_size(system: &ProofSystem, constraint_count: u64) -> usize {
        match system {
            ProofSystem::Stark => {
                // STARK proofs are typically logarithmic in circuit size
                (constraint_count as f64).log2() as usize * 32
            }
            ProofSystem::Snark | ProofSystem::Groth16 => {
                // SNARK proofs are typically constant size
                256 // ~256 bytes for Groth16
            }
            ProofSystem::Plonk => {
                // PLONK proofs are also relatively constant
                512
            }
            ProofSystem::Custom(_) => {
                // Conservative estimate
                1024
            }
        }
    }

    /// Check if a proof system supports recursion
    pub fn supports_recursion(system: &ProofSystem) -> bool {
        matches!(system, ProofSystem::Stark | ProofSystem::Plonk)
    }

    /// Get the security level of a proof system
    pub fn security_level(system: &ProofSystem) -> u32 {
        match system {
            ProofSystem::Stark => 128,
            ProofSystem::Snark | ProofSystem::Groth16 => 128,
            ProofSystem::Plonk => 128,
            ProofSystem::Custom(_) => 80, // Conservative estimate
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proof_creation() {
        let proof = ZkProof::new(
            ProofSystem::Stark,
            vec![1, 2, 3, 4],
            vec![5, 6],
            vec![7, 8, 9],
            "test_program".to_string(),
        );

        assert_eq!(proof.size(), 4);
        assert!(proof.is_valid());
        assert_eq!(proof.metadata.program_id, "test_program");
    }

    #[test]
    fn test_proof_serialization() {
        let proof = ZkProof::new(
            ProofSystem::Stark,
            vec![1, 2, 3, 4],
            vec![5, 6],
            vec![7, 8, 9],
            "test_program".to_string(),
        );

        let bytes = proof.to_bytes().unwrap();
        let deserialized = ZkProof::from_bytes(&bytes).unwrap();

        assert_eq!(proof.proof_data, deserialized.proof_data);
        assert_eq!(proof.public_inputs, deserialized.public_inputs);
    }

    #[test]
    fn test_proof_request_validation() {
        let request = ProofRequest::new(
            vec![1, 2, 3],
            vec![4, 5],
            vec![6, 7],
            ProofSystem::Stark,
        );

        assert!(request.validate().is_ok());

        let empty_request = ProofRequest::new(
            vec![],
            vec![4, 5],
            vec![6, 7],
            ProofSystem::Stark,
        );

        assert!(empty_request.validate().is_err());
    }

    #[test]
    fn test_commitment_verification() {
        let private_inputs = b"secret data";
        let commitment = utils::commit_private_inputs(private_inputs);
        
        assert!(utils::verify_commitment(&[], &commitment, private_inputs));
        assert!(!utils::verify_commitment(&[], &commitment, b"wrong data"));
    }

    #[test]
    fn test_proof_size_estimation() {
        let stark_size = utils::estimate_proof_size(&ProofSystem::Stark, 1000);
        let snark_size = utils::estimate_proof_size(&ProofSystem::Groth16, 1000);
        
        assert!(stark_size > 0);
        assert!(snark_size > 0);
        // STARK proofs should generally be larger than SNARK proofs for small circuits
        assert!(stark_size > snark_size);
    }
}
