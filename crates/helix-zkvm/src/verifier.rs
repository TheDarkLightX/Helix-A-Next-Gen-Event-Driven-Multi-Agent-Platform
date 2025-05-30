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


//! Proof verification utilities

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::{ZkProof, ProofSystem, errors::ZkVmError};

/// Result of proof verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether the proof is valid
    pub is_valid: bool,
    /// Verification time in milliseconds
    pub verification_time_ms: u64,
    /// Error message if verification failed
    pub error: Option<String>,
    /// Additional verification metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Trait for proof verifiers
#[async_trait]
pub trait ProofVerifier: Send + Sync {
    /// Verify a zero-knowledge proof
    async fn verify(&self, proof: &ZkProof) -> Result<VerificationResult, ZkVmError>;

    /// Batch verify multiple proofs
    async fn batch_verify(&self, proofs: &[ZkProof]) -> Result<Vec<VerificationResult>, ZkVmError> {
        let mut results = Vec::new();
        for proof in proofs {
            results.push(self.verify(proof).await?);
        }
        Ok(results)
    }

    /// Get the supported proof systems
    fn supported_systems(&self) -> Vec<ProofSystem>;

    /// Check if a proof system is supported
    fn supports_system(&self, system: &ProofSystem) -> bool {
        self.supported_systems().contains(system)
    }
}

/// Universal proof verifier that can handle multiple proof systems
pub struct UniversalVerifier {
    verifiers: HashMap<String, Box<dyn ProofVerifier>>,
}

impl UniversalVerifier {
    /// Create a new universal verifier
    pub fn new() -> Self {
        Self {
            verifiers: HashMap::new(),
        }
    }

    /// Register a verifier for a specific proof system
    pub fn register_verifier(&mut self, system: ProofSystem, verifier: Box<dyn ProofVerifier>) {
        let key = format!("{:?}", system).to_lowercase();
        self.verifiers.insert(key, verifier);
    }

    /// Get a verifier for a specific proof system
    fn get_verifier(&self, system: &ProofSystem) -> Option<&dyn ProofVerifier> {
        let key = format!("{:?}", system).to_lowercase();
        self.verifiers.get(&key).map(|v| v.as_ref())
    }
}

#[async_trait]
impl ProofVerifier for UniversalVerifier {
    async fn verify(&self, proof: &ZkProof) -> Result<VerificationResult, ZkVmError> {
        let verifier = self.get_verifier(&proof.system)
            .ok_or_else(|| ZkVmError::UnsupportedOperation(
                format!("No verifier registered for proof system: {:?}", proof.system)
            ))?;

        verifier.verify(proof).await
    }

    async fn batch_verify(&self, proofs: &[ZkProof]) -> Result<Vec<VerificationResult>, ZkVmError> {
        // Group proofs by system for efficient batch verification
        let mut grouped_proofs: HashMap<String, Vec<ZkProof>> = HashMap::new();

        for proof in proofs {
            let key = format!("{:?}", proof.system).to_lowercase();
            grouped_proofs.entry(key).or_default().push(proof.clone());
        }

        let mut all_results = Vec::new();

        for (system_key, system_proofs) in grouped_proofs {
            if let Some(verifier) = self.verifiers.get(&system_key) {
                let results = verifier.batch_verify(&system_proofs).await?;
                all_results.extend(results);
            } else {
                // Return error for unsupported systems
                for _ in &system_proofs {
                    all_results.push(VerificationResult {
                        is_valid: false,
                        verification_time_ms: 0,
                        error: Some(format!("Unsupported proof system: {}", system_key)),
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        Ok(all_results)
    }

    fn supported_systems(&self) -> Vec<ProofSystem> {
        let mut systems = Vec::new();
        for verifier in self.verifiers.values() {
            systems.extend(verifier.supported_systems());
        }
        systems.sort_by_key(|s| format!("{:?}", s));
        systems.dedup_by_key(|s| format!("{:?}", s));
        systems
    }
}

impl Default for UniversalVerifier {
    fn default() -> Self {
        Self::new()
    }
}

/// STARK proof verifier
pub struct StarkVerifier {
    // Configuration for STARK verification
    config: StarkConfig,
}

/// Configuration for STARK verification
#[derive(Debug, Clone)]
pub struct StarkConfig {
    /// Security level (bits)
    pub security_level: u32,
    /// Field size
    pub field_size: u32,
    /// Maximum degree
    pub max_degree: u32,
}

impl StarkVerifier {
    /// Create a new STARK verifier
    pub fn new(config: StarkConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ProofVerifier for StarkVerifier {
    async fn verify(&self, proof: &ZkProof) -> Result<VerificationResult, ZkVmError> {
        let start_time = std::time::Instant::now();

        // Verify that this is a STARK proof
        if !matches!(proof.system, ProofSystem::Stark) {
            return Ok(VerificationResult {
                is_valid: false,
                verification_time_ms: 0,
                error: Some("Not a STARK proof".to_string()),
                metadata: HashMap::new(),
            });
        }

        // Basic validation
        if !proof.is_valid() {
            return Ok(VerificationResult {
                is_valid: false,
                verification_time_ms: start_time.elapsed().as_millis() as u64,
                error: Some("Invalid proof structure".to_string()),
                metadata: HashMap::new(),
            });
        }

        // TODO: Implement actual STARK verification
        // This would involve:
        // 1. Parsing the proof data
        // 2. Reconstructing the execution trace
        // 3. Verifying polynomial commitments
        // 4. Checking FRI proofs
        
        // For now, return a mock verification
        let is_valid = proof.proof_data.len() > 32; // Simple check
        
        let mut metadata = HashMap::new();
        metadata.insert("security_level".to_string(), 
                        serde_json::Value::Number(self.config.security_level.into()));
        metadata.insert("field_size".to_string(), 
                        serde_json::Value::Number(self.config.field_size.into()));

        Ok(VerificationResult {
            is_valid,
            verification_time_ms: start_time.elapsed().as_millis() as u64,
            error: if is_valid { None } else { Some("Proof verification failed".to_string()) },
            metadata,
        })
    }

    fn supported_systems(&self) -> Vec<ProofSystem> {
        vec![ProofSystem::Stark]
    }
}

/// SNARK proof verifier
pub struct SnarkVerifier {
    config: SnarkConfig,
}

/// Configuration for SNARK verification
#[derive(Debug, Clone)]
pub struct SnarkConfig {
    /// Curve type (BN254, BLS12-381, etc.)
    pub curve: String,
    /// Trusted setup parameters
    pub setup_params: Vec<u8>,
}

impl SnarkVerifier {
    /// Create a new SNARK verifier
    pub fn new(config: SnarkConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ProofVerifier for SnarkVerifier {
    async fn verify(&self, proof: &ZkProof) -> Result<VerificationResult, ZkVmError> {
        let start_time = std::time::Instant::now();

        // Verify that this is a SNARK proof
        if !matches!(proof.system, ProofSystem::Snark | ProofSystem::Groth16 | ProofSystem::Plonk) {
            return Ok(VerificationResult {
                is_valid: false,
                verification_time_ms: 0,
                error: Some("Not a SNARK proof".to_string()),
                metadata: HashMap::new(),
            });
        }

        // Basic validation
        if !proof.is_valid() {
            return Ok(VerificationResult {
                is_valid: false,
                verification_time_ms: start_time.elapsed().as_millis() as u64,
                error: Some("Invalid proof structure".to_string()),
                metadata: HashMap::new(),
            });
        }

        // TODO: Implement actual SNARK verification
        // This would involve:
        // 1. Parsing the proof elements (A, B, C for Groth16)
        // 2. Verifying the pairing equation
        // 3. Checking public inputs
        
        // For now, return a mock verification
        let is_valid = proof.proof_data.len() >= 96; // Groth16 proofs are ~96 bytes
        
        let mut metadata = HashMap::new();
        metadata.insert("curve".to_string(), 
                        serde_json::Value::String(self.config.curve.clone()));
        metadata.insert("proof_system".to_string(), 
                        serde_json::Value::String(format!("{:?}", proof.system)));

        Ok(VerificationResult {
            is_valid,
            verification_time_ms: start_time.elapsed().as_millis() as u64,
            error: if is_valid { None } else { Some("Proof verification failed".to_string()) },
            metadata,
        })
    }

    fn supported_systems(&self) -> Vec<ProofSystem> {
        vec![ProofSystem::Snark, ProofSystem::Groth16, ProofSystem::Plonk]
    }
}

impl Default for StarkConfig {
    fn default() -> Self {
        Self {
            security_level: 128,
            field_size: 256,
            max_degree: 1024,
        }
    }
}

impl Default for SnarkConfig {
    fn default() -> Self {
        Self {
            curve: "BN254".to_string(),
            setup_params: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ZkProof;

    #[tokio::test]
    async fn test_stark_verifier() {
        let verifier = StarkVerifier::new(StarkConfig::default());
        
        let proof = ZkProof::new(
            ProofSystem::Stark,
            vec![0u8; 64], // 64-byte proof
            vec![1, 2, 3],
            vec![4, 5, 6],
            "test".to_string(),
        );

        let result = verifier.verify(&proof).await.unwrap();
        assert!(result.is_valid);
        assert!(result.verification_time_ms > 0);
    }

    #[tokio::test]
    async fn test_snark_verifier() {
        let verifier = SnarkVerifier::new(SnarkConfig::default());
        
        let proof = ZkProof::new(
            ProofSystem::Groth16,
            vec![0u8; 96], // 96-byte Groth16 proof
            vec![1, 2, 3],
            vec![4, 5, 6],
            "test".to_string(),
        );

        let result = verifier.verify(&proof).await.unwrap();
        assert!(result.is_valid);
    }

    #[tokio::test]
    async fn test_universal_verifier() {
        let mut universal = UniversalVerifier::new();
        universal.register_verifier(
            ProofSystem::Stark,
            Box::new(StarkVerifier::new(StarkConfig::default()))
        );
        universal.register_verifier(
            ProofSystem::Groth16,
            Box::new(SnarkVerifier::new(SnarkConfig::default()))
        );

        let stark_proof = ZkProof::new(
            ProofSystem::Stark,
            vec![0u8; 64],
            vec![1, 2, 3],
            vec![4, 5, 6],
            "test".to_string(),
        );

        let result = universal.verify(&stark_proof).await.unwrap();
        assert!(result.is_valid);

        let supported = universal.supported_systems();
        assert!(supported.contains(&ProofSystem::Stark));
        assert!(supported.contains(&ProofSystem::Groth16));
    }
}
