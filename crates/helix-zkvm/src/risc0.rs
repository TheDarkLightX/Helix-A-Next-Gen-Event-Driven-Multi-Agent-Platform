// Copyright 2026 DarkLightX
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

//! RISC0 zkVM integration.
//!
//! This adapter is honest about the current build boundary: real RISC Zero
//! proving and verification dependencies are not wired in yet. Until they are,
//! source compilation and proof operations fail closed. A tiny deterministic
//! bytecode kernel remains for local receipt plumbing tests.

use crate::{
    errors::ZkVmError, ExecutionStats, ProgramLanguage, VerificationResult, ZkProof, ZkVmAgent,
    ZkVmAgentConfig, ZkVmCapabilities, ZkVmExecutionResult, ZkVmSystem,
};
use async_trait::async_trait;
use helix_core::{agent::AgentConfig, types::AgentId};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

/// Bytecode accepted by this build's deterministic test kernel to echo inputs.
pub const RISC0_DETERMINISTIC_IDENTITY_PROGRAM: &[u8] = b"HELIX_RISC0_DETERMINISTIC_V1:IDENTITY";

/// Bytecode accepted by this build's deterministic test kernel to hash inputs.
pub const RISC0_DETERMINISTIC_SHA256_PROGRAM: &[u8] = b"HELIX_RISC0_DETERMINISTIC_V1:SHA256";

const DEFAULT_RISC0_PROFILE_ID: &str = "50000000-0000-0000-0000-000000000020";

/// RISC0 zkVM system implementation.
pub struct Risc0System {
    config: Risc0Config,
}

/// Configuration for RISC0 system.
#[derive(Debug, Clone)]
pub struct Risc0Config {
    /// Maximum cycles allowed.
    pub max_cycles: u64,
    /// Memory limit in bytes.
    pub memory_limit: u64,
    /// Whether to enable proof generation by default.
    pub enable_proofs: bool,
}

impl Risc0System {
    /// Create a new RISC0 system.
    pub fn new(config: Risc0Config) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ZkVmSystem for Risc0System {
    fn name(&self) -> &str {
        "risc0"
    }

    async fn create_agent(&self, config: ZkVmAgentConfig) -> Result<Box<dyn ZkVmAgent>, ZkVmError> {
        Ok(Box::new(Risc0Agent::new(config, self.config.clone())?))
    }

    async fn compile_program(
        &self,
        source_code: &str,
        language: ProgramLanguage,
    ) -> Result<Vec<u8>, ZkVmError> {
        match language {
            ProgramLanguage::Bytecode => validate_risc0_bytecode(source_code.as_bytes())
                .map(|_| source_code.as_bytes().to_vec()),
            ProgramLanguage::Rust | ProgramLanguage::C | ProgramLanguage::Assembly => {
                Err(ZkVmError::UnsupportedOperation(format!(
                    "{language:?} compilation requires the real RISC Zero toolchain, which is not wired into this build"
                )))
            }
        }
    }

    fn get_capabilities(&self) -> ZkVmCapabilities {
        ZkVmCapabilities {
            max_cycles: self.config.max_cycles,
            max_memory: self.config.memory_limit,
            supported_languages: vec![ProgramLanguage::Bytecode],
            supports_proofs: false,
            supports_recursion: false,
            proof_system: "risc0-unavailable".to_string(),
        }
    }
}

/// RISC0 agent implementation.
pub struct Risc0Agent {
    config: ZkVmAgentConfig,
    system_config: Risc0Config,
    /// Core agent configuration used for integration with helix-core.
    agent_config: AgentConfig,
}

impl Risc0Agent {
    /// Create a new RISC0 agent.
    pub fn new(config: ZkVmAgentConfig, system_config: Risc0Config) -> Result<Self, ZkVmError> {
        validate_risc0_agent_config(&config, &system_config)?;
        let agent_config = risc0_agent_config(&config)?;

        Ok(Self {
            config,
            system_config,
            agent_config,
        })
    }
}

#[async_trait]
impl crate::agent::Agent for Risc0Agent {
    fn id(&self) -> AgentId {
        self.agent_config.id
    }

    fn config(&self) -> &AgentConfig {
        &self.agent_config
    }
}

#[async_trait]
impl ZkVmAgent for Risc0Agent {
    async fn execute_zkvm(
        &mut self,
        program: &[u8],
        inputs: &[u8],
        generate_proof: bool,
    ) -> Result<ZkVmExecutionResult, ZkVmError> {
        if generate_proof || self.config.generate_proofs {
            return Err(ZkVmError::UnsupportedOperation(
                "RISC0 proof generation requires the real RISC Zero prover, which is not wired into this build".to_string(),
            ));
        }

        validate_risc0_execution_bounds(program, inputs, &self.config, &self.system_config)?;
        let output = execute_deterministic_risc0_program(program, inputs)?;
        let receipt = build_risc0_receipt(
            &self.config.program_id,
            &self.agent_config.id,
            program,
            inputs,
            &output,
        );

        Ok(ZkVmExecutionResult {
            output,
            stats: ExecutionStats {
                cycles: risc0_cycle_estimate(program, inputs)?,
                memory_used: inputs.len() as u64 + program.len() as u64,
                execution_time_ms: 0,
                proof_time_ms: None,
            },
            proof: None,
            receipt: Some(receipt),
        })
    }

    async fn verify_proof(
        &self,
        proof: &ZkProof,
        expected_output: &[u8],
    ) -> Result<VerificationResult, ZkVmError> {
        let mut metadata = HashMap::new();
        metadata.insert(
            "proof_system".to_string(),
            Value::String("STARK".to_string()),
        );
        metadata.insert("risc0_prover_available".to_string(), Value::Bool(false));
        metadata.insert(
            "proof_hash".to_string(),
            Value::String(hex_digest(&proof.hash())),
        );
        metadata.insert(
            "expected_output_hash".to_string(),
            Value::String(hex_digest(&sha256(expected_output))),
        );

        Ok(VerificationResult {
            is_valid: false,
            verification_time_ms: 0,
            error: Some(
                "RISC0 proof verification requires the real RISC Zero verifier, which is not wired into this build".to_string(),
            ),
            metadata,
        })
    }

    async fn get_state_commitment(&self) -> Result<Vec<u8>, ZkVmError> {
        let mut hasher = Sha256::new();
        hasher.update(b"helix-risc0-agent-state-v1");
        hasher.update(self.agent_config.id.as_bytes());
        hasher.update(self.config.system.as_bytes());
        hasher.update(self.config.program_id.as_bytes());
        hasher.update(self.config.max_cycles.to_le_bytes());
        hasher.update(self.config.memory_limit.to_le_bytes());
        Ok(hasher.finalize().to_vec())
    }

    async fn prove_state_transition(
        &mut self,
        _old_state: &[u8],
        _new_state: &[u8],
        _transition_proof: &[u8],
    ) -> Result<ZkProof, ZkVmError> {
        Err(ZkVmError::UnsupportedOperation(
            "RISC0 state-transition proofs require the real RISC Zero prover, which is not wired into this build".to_string(),
        ))
    }
}

impl Default for Risc0Config {
    fn default() -> Self {
        Self {
            max_cycles: 1_000_000,
            memory_limit: 64 * 1024 * 1024,
            enable_proofs: false,
        }
    }
}

fn validate_risc0_agent_config(
    config: &ZkVmAgentConfig,
    system_config: &Risc0Config,
) -> Result<(), ZkVmError> {
    if config.system != "risc0" {
        return Err(ZkVmError::ConfigurationError(
            "RISC0 agent config must use system 'risc0'".to_string(),
        ));
    }
    if config.program_id.trim().is_empty() {
        return Err(ZkVmError::ConfigurationError(
            "program_id must not be empty".to_string(),
        ));
    }
    if config.max_cycles == 0 || system_config.max_cycles == 0 {
        return Err(ZkVmError::ConfigurationError(
            "max_cycles must be greater than zero".to_string(),
        ));
    }
    if config.memory_limit == 0 || system_config.memory_limit == 0 {
        return Err(ZkVmError::ConfigurationError(
            "memory_limit must be greater than zero".to_string(),
        ));
    }
    if config.max_cycles > system_config.max_cycles {
        return Err(ZkVmError::ResourceLimitExceeded(format!(
            "agent max_cycles {} exceeds system limit {}",
            config.max_cycles, system_config.max_cycles
        )));
    }
    if config.memory_limit > system_config.memory_limit {
        return Err(ZkVmError::ResourceLimitExceeded(format!(
            "agent memory_limit {} exceeds system limit {}",
            config.memory_limit, system_config.memory_limit
        )));
    }
    if config.generate_proofs || system_config.enable_proofs {
        return Err(ZkVmError::UnsupportedOperation(
            "RISC0 proof generation is unavailable until real RISC Zero prover dependencies are enabled".to_string(),
        ));
    }
    Ok(())
}

fn risc0_agent_config(config: &ZkVmAgentConfig) -> Result<AgentConfig, ZkVmError> {
    let agent_id = match parameter_string(&config.parameters, "agent_id")? {
        Some(value) => parse_uuid_parameter("agent_id", value)?,
        None => deterministic_risc0_agent_id(config),
    };
    let profile_id = match parameter_string(&config.parameters, "profile_id")? {
        Some(value) => parse_uuid_parameter("profile_id", value)?,
        None => Uuid::parse_str(DEFAULT_RISC0_PROFILE_ID)
            .map_err(|error| ZkVmError::ConfigurationError(error.to_string()))?,
    };
    let name = parameter_string(&config.parameters, "agent_name")?.map(str::to_string);
    let config_data = serde_json::to_value(config)
        .map_err(|error| ZkVmError::SerializationError(error.to_string()))?;

    Ok(AgentConfig::new(
        agent_id,
        profile_id,
        name,
        "risc0".to_string(),
        config_data,
    ))
}

fn parameter_string<'a>(
    parameters: &'a HashMap<String, Value>,
    key: &str,
) -> Result<Option<&'a str>, ZkVmError> {
    parameters
        .get(key)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| ZkVmError::ConfigurationError(format!("{key} must be a string")))
        })
        .transpose()
}

fn parse_uuid_parameter(context: &str, value: &str) -> Result<Uuid, ZkVmError> {
    let parsed = Uuid::parse_str(value.trim())
        .map_err(|_| ZkVmError::ConfigurationError(format!("{context} must be a valid UUID")))?;
    if parsed.is_nil() {
        return Err(ZkVmError::ConfigurationError(format!(
            "{context} must not be nil"
        )));
    }
    Ok(parsed)
}

fn deterministic_risc0_agent_id(config: &ZkVmAgentConfig) -> AgentId {
    let mut hasher = Sha256::new();
    hasher.update(b"helix-risc0-agent-id-v1");
    hasher.update(config.system.as_bytes());
    hasher.update(config.program_id.as_bytes());
    hasher.update(config.max_cycles.to_le_bytes());
    hasher.update(config.memory_limit.to_le_bytes());
    uuid_from_digest(&hasher.finalize())
}

fn validate_risc0_execution_bounds(
    program: &[u8],
    inputs: &[u8],
    config: &ZkVmAgentConfig,
    system_config: &Risc0Config,
) -> Result<(), ZkVmError> {
    validate_risc0_bytecode(program)?;
    let memory_used = risc0_memory_estimate(program, inputs)?;
    let memory_limit = config.memory_limit.min(system_config.memory_limit);
    if memory_used > memory_limit {
        return Err(ZkVmError::ResourceLimitExceeded(format!(
            "execution requires {memory_used} bytes, limit is {memory_limit}"
        )));
    }

    let cycles = risc0_cycle_estimate(program, inputs)?;
    let cycle_limit = config.max_cycles.min(system_config.max_cycles);
    if cycles > cycle_limit {
        return Err(ZkVmError::ResourceLimitExceeded(format!(
            "execution requires {cycles} cycles, limit is {cycle_limit}"
        )));
    }

    Ok(())
}

fn validate_risc0_bytecode(program: &[u8]) -> Result<(), ZkVmError> {
    if program.is_empty() {
        return Err(ZkVmError::InvalidProgram(
            "RISC0 bytecode must not be empty".to_string(),
        ));
    }
    Ok(())
}

fn execute_deterministic_risc0_program(
    program: &[u8],
    inputs: &[u8],
) -> Result<Vec<u8>, ZkVmError> {
    match program {
        RISC0_DETERMINISTIC_IDENTITY_PROGRAM => Ok(inputs.to_vec()),
        RISC0_DETERMINISTIC_SHA256_PROGRAM => Ok(sha256(inputs)),
        _ => Err(ZkVmError::UnsupportedOperation(
            "RISC0 execution requires real RISC Zero bytecode; this build only supports deterministic receipt test programs".to_string(),
        )),
    }
}

fn risc0_memory_estimate(program: &[u8], inputs: &[u8]) -> Result<u64, ZkVmError> {
    (program.len() as u64)
        .checked_add(inputs.len() as u64)
        .ok_or_else(|| ZkVmError::ResourceLimitExceeded("memory estimate overflow".to_string()))
}

fn risc0_cycle_estimate(program: &[u8], inputs: &[u8]) -> Result<u64, ZkVmError> {
    let memory = risc0_memory_estimate(program, inputs)?;
    memory
        .checked_add(1)
        .ok_or_else(|| ZkVmError::ResourceLimitExceeded("cycle estimate overflow".to_string()))
}

fn build_risc0_receipt(
    program_id: &str,
    agent_id: &AgentId,
    program: &[u8],
    inputs: &[u8],
    output: &[u8],
) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(b"helix-risc0-deterministic-receipt-v1");
    hasher.update(program_id.as_bytes());
    hasher.update(agent_id.as_bytes());
    hasher.update(sha256(program));
    hasher.update(sha256(inputs));
    hasher.update(sha256(output));
    hasher.finalize().to_vec()
}

fn sha256(bytes: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().to_vec()
}

fn uuid_from_digest(digest: &[u8]) -> Uuid {
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use crate::ProofSystem;

    fn test_config() -> ZkVmAgentConfig {
        ZkVmAgentConfig {
            system: "risc0".to_string(),
            program_id: "test_program".to_string(),
            parameters: HashMap::new(),
            generate_proofs: false,
            max_cycles: 10_000,
            memory_limit: 1024 * 1024,
        }
    }

    #[test]
    fn test_risc0_system_creation() {
        let system = Risc0System::new(Risc0Config::default());
        assert_eq!(system.name(), "risc0");

        let capabilities = system.get_capabilities();
        assert!(!capabilities.supports_proofs);
        assert!(!capabilities.supports_recursion);
        assert_eq!(capabilities.supported_languages.len(), 1);
        assert_eq!(capabilities.proof_system, "risc0-unavailable");
    }

    #[tokio::test]
    async fn test_risc0_program_compilation_fails_closed_for_source() {
        let system = Risc0System::new(Risc0Config::default());

        let result = system
            .compile_program("fn main() {}", ProgramLanguage::Rust)
            .await;

        assert!(matches!(result, Err(ZkVmError::UnsupportedOperation(_))));
    }

    #[tokio::test]
    async fn test_risc0_bytecode_compilation() {
        let system = Risc0System::new(Risc0Config::default());

        let result = system
            .compile_program(
                "HELIX_RISC0_DETERMINISTIC_V1:IDENTITY",
                ProgramLanguage::Bytecode,
            )
            .await
            .unwrap();

        assert_eq!(result, RISC0_DETERMINISTIC_IDENTITY_PROGRAM);
    }

    #[tokio::test]
    async fn test_risc0_agent_execution() {
        let mut agent = Risc0Agent::new(test_config(), Risc0Config::default()).unwrap();

        let result = agent
            .execute_zkvm(RISC0_DETERMINISTIC_IDENTITY_PROGRAM, b"test_input", false)
            .await
            .unwrap();

        assert_eq!(result.output, b"test_input");
        assert!(result.proof.is_none());
        assert_eq!(result.receipt.as_ref().unwrap().len(), 32);
        assert!(result.stats.cycles > 0);
        assert_eq!(result.stats.execution_time_ms, 0);
    }

    #[tokio::test]
    async fn test_risc0_agent_id_is_stable() {
        let agent = Risc0Agent::new(test_config(), Risc0Config::default()).unwrap();

        assert_eq!(agent.id(), agent.id());
        assert_eq!(agent.id(), agent.config().id);
        assert!(!agent.id().is_nil());
    }

    #[tokio::test]
    async fn test_risc0_proof_generation_fails_closed() {
        let mut agent = Risc0Agent::new(test_config(), Risc0Config::default()).unwrap();

        let result = agent
            .execute_zkvm(RISC0_DETERMINISTIC_SHA256_PROGRAM, b"test_input", true)
            .await;

        assert!(matches!(result, Err(ZkVmError::UnsupportedOperation(_))));
    }

    #[tokio::test]
    async fn test_risc0_proof_verification_fails_closed() {
        let agent = Risc0Agent::new(test_config(), Risc0Config::default()).unwrap();
        let proof = ZkProof::new(
            ProofSystem::Stark,
            vec![1; 128],
            b"public".to_vec(),
            vec![2; 32],
            "test_program".to_string(),
        );

        let result = agent.verify_proof(&proof, b"output").await.unwrap();

        assert!(!result.is_valid);
        assert_eq!(result.verification_time_ms, 0);
        assert!(result.error.unwrap().contains("real RISC Zero verifier"));
    }

    #[tokio::test]
    async fn test_risc0_state_transition_proofs_fail_closed() {
        let mut agent = Risc0Agent::new(test_config(), Risc0Config::default()).unwrap();

        let result = agent.prove_state_transition(b"old", b"new", b"proof").await;

        assert!(matches!(result, Err(ZkVmError::UnsupportedOperation(_))));
    }
}
