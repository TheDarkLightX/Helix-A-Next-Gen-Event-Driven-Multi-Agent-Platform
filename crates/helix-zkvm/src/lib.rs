#![warn(missing_docs)]

//! Zero-knowledge virtual machine integration for Helix.
//!
//! This crate provides:
//! - zkVM execution environments for agent computations
//! - Zero-knowledge proof generation and verification
//! - Privacy-preserving agent state transitions
//! - Verifiable computation results
//! - Integration with RISC0 and SP1 zkVM systems

pub mod risc0;
pub mod sp1;
pub mod proofs;
pub mod circuits;
pub mod verifier;
pub mod errors;

pub use errors::ZkVmError;
pub use proofs::{ZkProof, ProofSystem, ProofRequest, ProofResponse};
pub use verifier::{ProofVerifier, VerificationResult};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Temporary agent trait to avoid circular dependency
pub mod agent {
    use async_trait::async_trait;
    use crate::errors::ZkVmError;

    #[async_trait]
    pub trait Agent: Send + Sync {
        fn id(&self) -> helix_core::types::AgentId;
        fn config(&self) -> &helix_core::agent::AgentConfig;
    }
}

/// Configuration for zkVM-powered agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkVmAgentConfig {
    /// The zkVM system to use (risc0, sp1)
    pub system: String,
    /// Circuit or program identifier
    pub program_id: String,
    /// Input parameters for the zkVM program
    pub parameters: HashMap<String, serde_json::Value>,
    /// Whether to generate proofs for all computations
    pub generate_proofs: bool,
    /// Maximum execution cycles allowed
    pub max_cycles: u64,
    /// Memory limit for the zkVM
    pub memory_limit: u64,
}

/// Trait for zkVM-powered agents that can perform verifiable computations
#[async_trait]
pub trait ZkVmAgent: agent::Agent {
    /// Execute a computation in the zkVM and optionally generate a proof
    async fn execute_zkvm(
        &mut self,
        program: &[u8],
        inputs: &[u8],
        generate_proof: bool,
    ) -> Result<ZkVmExecutionResult, ZkVmError>;

    /// Verify a zero-knowledge proof
    async fn verify_proof(
        &self,
        proof: &ZkProof,
        expected_output: &[u8],
    ) -> Result<VerificationResult, ZkVmError>;

    /// Get the current state commitment (hash of internal state)
    async fn get_state_commitment(&self) -> Result<Vec<u8>, ZkVmError>;

    /// Prove a state transition from one state to another
    async fn prove_state_transition(
        &mut self,
        old_state: &[u8],
        new_state: &[u8],
        transition_proof: &[u8],
    ) -> Result<ZkProof, ZkVmError>;
}

/// Result of zkVM execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkVmExecutionResult {
    /// Output data from the computation
    pub output: Vec<u8>,
    /// Execution statistics
    pub stats: ExecutionStats,
    /// Optional zero-knowledge proof
    pub proof: Option<ZkProof>,
    /// Receipt or execution trace
    pub receipt: Option<Vec<u8>>,
}

/// Execution statistics for zkVM runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    /// Number of execution cycles used
    pub cycles: u64,
    /// Memory usage in bytes
    pub memory_used: u64,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Proof generation time in milliseconds (if applicable)
    pub proof_time_ms: Option<u64>,
}

/// Factory for creating zkVM-powered agents
pub struct ZkVmAgentFactory {
    systems: HashMap<String, Box<dyn ZkVmSystem>>,
}

impl ZkVmAgentFactory {
    /// Create a new zkVM agent factory
    pub fn new() -> Self {
        Self {
            systems: HashMap::new(),
        }
    }

    /// Register a zkVM system
    pub fn register_system(&mut self, name: String, system: Box<dyn ZkVmSystem>) {
        self.systems.insert(name, system);
    }

    /// Create a zkVM agent with the given configuration
    pub async fn create_agent(
        &self,
        config: ZkVmAgentConfig,
    ) -> Result<Box<dyn ZkVmAgent>, ZkVmError> {
        let system = self.systems.get(&config.system)
            .ok_or_else(|| ZkVmError::SystemNotFound(config.system.clone()))?;

        // Create agent implementation based on system
        system.create_agent(config).await
    }
}

impl Default for ZkVmAgentFactory {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for zkVM systems (RISC0, SP1, etc.)
#[async_trait]
pub trait ZkVmSystem: Send + Sync {
    /// Get the name of this zkVM system
    fn name(&self) -> &str;

    /// Create an agent using this zkVM system
    async fn create_agent(
        &self,
        config: ZkVmAgentConfig,
    ) -> Result<Box<dyn ZkVmAgent>, ZkVmError>;

    /// Compile a program for this zkVM system
    async fn compile_program(
        &self,
        source_code: &str,
        language: ProgramLanguage,
    ) -> Result<Vec<u8>, ZkVmError>;

    /// Get system capabilities and limits
    fn get_capabilities(&self) -> ZkVmCapabilities;
}

/// Programming language for zkVM programs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProgramLanguage {
    /// Rust source code
    Rust,
    /// Assembly language
    Assembly,
    /// C source code
    C,
    /// Pre-compiled bytecode
    Bytecode,
}

/// Capabilities of a zkVM system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkVmCapabilities {
    /// Maximum number of execution cycles
    pub max_cycles: u64,
    /// Maximum memory in bytes
    pub max_memory: u64,
    /// Supported programming languages
    pub supported_languages: Vec<ProgramLanguage>,
    /// Whether the system supports proof generation
    pub supports_proofs: bool,
    /// Whether the system supports recursive proofs
    pub supports_recursion: bool,
    /// Proof system used (STARK, SNARK, etc.)
    pub proof_system: String,
}

/// Utility functions for zkVM operations
pub mod utils {
    use super::*;

    /// Hash data using a cryptographically secure hash function
    pub fn hash_data(data: &[u8]) -> Vec<u8> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }

    /// Serialize data for zkVM input
    pub fn serialize_for_zkvm<T: Serialize>(data: &T) -> Result<Vec<u8>, ZkVmError> {
        // For now, use JSON serialization as bincode is not in dependencies
        serde_json::to_vec(data)
            .map_err(|e| ZkVmError::SerializationError(e.to_string()))
    }

    /// Deserialize data from zkVM output
    pub fn deserialize_from_zkvm<T: for<'de> Deserialize<'de>>(data: &[u8]) -> Result<T, ZkVmError> {
        // For now, use JSON serialization as bincode is not in dependencies
        serde_json::from_slice(data)
            .map_err(|e| ZkVmError::SerializationError(e.to_string()))
    }

    /// Generate a commitment to a value using a Merkle tree
    pub fn generate_commitment(values: &[Vec<u8>]) -> Vec<u8> {
        if values.is_empty() {
            return hash_data(&[]);
        }

        if values.len() == 1 {
            return hash_data(&values[0]);
        }

        // Simple binary Merkle tree implementation
        let mut level = values.iter().map(|v| hash_data(v)).collect::<Vec<_>>();
        
        while level.len() > 1 {
            let mut next_level = Vec::new();
            for chunk in level.chunks(2) {
                if chunk.len() == 2 {
                    let mut combined = chunk[0].clone();
                    combined.extend_from_slice(&chunk[1]);
                    next_level.push(hash_data(&combined));
                } else {
                    next_level.push(chunk[0].clone());
                }
            }
            level = next_level;
        }

        level[0].clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zkvm_agent_config_serialization() {
        let config = ZkVmAgentConfig {
            system: "risc0".to_string(),
            program_id: "fibonacci".to_string(),
            parameters: HashMap::new(),
            generate_proofs: true,
            max_cycles: 1000000,
            memory_limit: 1024 * 1024,
        };

        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: ZkVmAgentConfig = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(config.system, deserialized.system);
        assert_eq!(config.program_id, deserialized.program_id);
        assert_eq!(config.generate_proofs, deserialized.generate_proofs);
    }

    #[test]
    fn test_hash_data() {
        let data = b"hello world";
        let hash1 = utils::hash_data(data);
        let hash2 = utils::hash_data(data);
        
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 32); // SHA256 produces 32-byte hashes
    }

    #[test]
    fn test_generate_commitment() {
        let values = vec![
            b"value1".to_vec(),
            b"value2".to_vec(),
            b"value3".to_vec(),
        ];
        
        let commitment = utils::generate_commitment(&values);
        assert_eq!(commitment.len(), 32);
        
        // Same values should produce same commitment
        let commitment2 = utils::generate_commitment(&values);
        assert_eq!(commitment, commitment2);
    }

    #[tokio::test]
    async fn test_zkvm_agent_factory_creation() {
        let factory = ZkVmAgentFactory::new();
        assert_eq!(factory.systems.len(), 0);
    }
}
