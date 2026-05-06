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

//! SP1 zkVM integration.
//!
//! This adapter is intentionally fail-closed until real SP1 prover and verifier
//! dependencies are wired into Helix. Source compilation and proof operations
//! return explicit unsupported-operation errors instead of mock proofs.

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

/// Bytecode accepted by this build's deterministic SP1 test kernel to echo inputs.
pub const SP1_DETERMINISTIC_IDENTITY_PROGRAM: &[u8] = b"HELIX_SP1_DETERMINISTIC_V1:IDENTITY";

/// Bytecode accepted by this build's deterministic SP1 test kernel to hash inputs.
pub const SP1_DETERMINISTIC_SHA256_PROGRAM: &[u8] = b"HELIX_SP1_DETERMINISTIC_V1:SHA256";

const DEFAULT_SP1_PROFILE_ID: &str = "50000000-0000-0000-0000-000000000021";

/// SP1 zkVM system implementation.
pub struct Sp1System {
    config: Sp1Config,
}

/// Configuration for SP1 system.
#[derive(Debug, Clone)]
pub struct Sp1Config {
    /// Maximum cycles allowed.
    pub max_cycles: u64,
    /// Memory limit in bytes.
    pub memory_limit: u64,
    /// Whether to enable proof generation by default.
    pub enable_proofs: bool,
    /// SP1 specific configuration.
    pub sp1_config: HashMap<String, Value>,
}

impl Sp1System {
    /// Create a new SP1 system.
    pub fn new(config: Sp1Config) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ZkVmSystem for Sp1System {
    fn name(&self) -> &str {
        "sp1"
    }

    async fn create_agent(&self, config: ZkVmAgentConfig) -> Result<Box<dyn ZkVmAgent>, ZkVmError> {
        Ok(Box::new(Sp1Agent::new(config, self.config.clone())?))
    }

    async fn compile_program(
        &self,
        source_code: &str,
        language: ProgramLanguage,
    ) -> Result<Vec<u8>, ZkVmError> {
        match language {
            ProgramLanguage::Bytecode => {
                validate_sp1_bytecode(source_code.as_bytes()).map(|_| source_code.as_bytes().to_vec())
            }
            ProgramLanguage::Rust | ProgramLanguage::Assembly | ProgramLanguage::C => {
                Err(ZkVmError::UnsupportedOperation(format!(
                    "{language:?} compilation requires the real SP1 toolchain, which is not wired into this build"
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
            proof_system: "sp1-unavailable".to_string(),
        }
    }
}

/// SP1 agent implementation.
pub struct Sp1Agent {
    config: ZkVmAgentConfig,
    system_config: Sp1Config,
    /// Core agent configuration used for integration with helix-core.
    agent_config: AgentConfig,
}

impl Sp1Agent {
    /// Create a new SP1 agent.
    pub fn new(config: ZkVmAgentConfig, system_config: Sp1Config) -> Result<Self, ZkVmError> {
        validate_sp1_agent_config(&config, &system_config)?;
        let agent_config = sp1_agent_config(&config)?;

        Ok(Self {
            config,
            system_config,
            agent_config,
        })
    }
}

#[async_trait]
impl crate::agent::Agent for Sp1Agent {
    fn id(&self) -> AgentId {
        self.agent_config.id
    }

    fn config(&self) -> &AgentConfig {
        &self.agent_config
    }
}

#[async_trait]
impl ZkVmAgent for Sp1Agent {
    async fn execute_zkvm(
        &mut self,
        program: &[u8],
        inputs: &[u8],
        generate_proof: bool,
    ) -> Result<ZkVmExecutionResult, ZkVmError> {
        if generate_proof || self.config.generate_proofs {
            return Err(ZkVmError::UnsupportedOperation(
                "SP1 proof generation requires the real SP1 prover, which is not wired into this build".to_string(),
            ));
        }

        validate_sp1_execution_bounds(program, inputs, &self.config, &self.system_config)?;
        let output = execute_deterministic_sp1_program(program, inputs)?;
        let receipt = build_sp1_receipt(
            &self.config.program_id,
            &self.agent_config.id,
            program,
            inputs,
            &output,
        );

        Ok(ZkVmExecutionResult {
            output,
            stats: ExecutionStats {
                cycles: sp1_cycle_estimate(program, inputs)?,
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
            Value::String("PLONK".to_string()),
        );
        metadata.insert("sp1_prover_available".to_string(), Value::Bool(false));
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
                "SP1 proof verification requires the real SP1 verifier, which is not wired into this build".to_string(),
            ),
            metadata,
        })
    }

    async fn get_state_commitment(&self) -> Result<Vec<u8>, ZkVmError> {
        let mut hasher = Sha256::new();
        hasher.update(b"helix-sp1-agent-state-v1");
        hasher.update(self.agent_config.id.as_bytes());
        hasher.update(self.config.system.as_bytes());
        hasher.update(self.config.program_id.as_bytes());
        hasher.update(self.config.max_cycles.to_le_bytes());
        hasher.update(self.config.memory_limit.to_le_bytes());
        hash_json_map(&mut hasher, &self.system_config.sp1_config)?;
        Ok(hasher.finalize().to_vec())
    }

    async fn prove_state_transition(
        &mut self,
        _old_state: &[u8],
        _new_state: &[u8],
        _transition_proof: &[u8],
    ) -> Result<ZkProof, ZkVmError> {
        Err(ZkVmError::UnsupportedOperation(
            "SP1 state-transition proofs require the real SP1 prover, which is not wired into this build".to_string(),
        ))
    }
}

impl Default for Sp1Config {
    fn default() -> Self {
        Self {
            max_cycles: 10_000_000,
            memory_limit: 128 * 1024 * 1024,
            enable_proofs: false,
            sp1_config: HashMap::new(),
        }
    }
}

fn validate_sp1_agent_config(
    config: &ZkVmAgentConfig,
    system_config: &Sp1Config,
) -> Result<(), ZkVmError> {
    if config.system != "sp1" {
        return Err(ZkVmError::ConfigurationError(
            "SP1 agent config must use system 'sp1'".to_string(),
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
            "SP1 proof generation is unavailable until real SP1 prover dependencies are enabled"
                .to_string(),
        ));
    }
    Ok(())
}

fn sp1_agent_config(config: &ZkVmAgentConfig) -> Result<AgentConfig, ZkVmError> {
    let agent_id = match parameter_string(&config.parameters, "agent_id")? {
        Some(value) => parse_uuid_parameter("agent_id", value)?,
        None => deterministic_sp1_agent_id(config)?,
    };
    let profile_id = match parameter_string(&config.parameters, "profile_id")? {
        Some(value) => parse_uuid_parameter("profile_id", value)?,
        None => Uuid::parse_str(DEFAULT_SP1_PROFILE_ID)
            .map_err(|error| ZkVmError::ConfigurationError(error.to_string()))?,
    };
    let name = parameter_string(&config.parameters, "agent_name")?.map(str::to_string);
    let config_data = serde_json::to_value(config)
        .map_err(|error| ZkVmError::SerializationError(error.to_string()))?;

    Ok(AgentConfig::new(
        agent_id,
        profile_id,
        name,
        "sp1".to_string(),
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

fn deterministic_sp1_agent_id(config: &ZkVmAgentConfig) -> Result<AgentId, ZkVmError> {
    let mut hasher = Sha256::new();
    hasher.update(b"helix-sp1-agent-id-v1");
    hasher.update(config.system.as_bytes());
    hasher.update(config.program_id.as_bytes());
    hasher.update(config.max_cycles.to_le_bytes());
    hasher.update(config.memory_limit.to_le_bytes());
    hash_json_map(&mut hasher, &config.parameters)?;
    Ok(uuid_from_digest(&hasher.finalize()))
}

fn validate_sp1_execution_bounds(
    program: &[u8],
    inputs: &[u8],
    config: &ZkVmAgentConfig,
    system_config: &Sp1Config,
) -> Result<(), ZkVmError> {
    validate_sp1_bytecode(program)?;
    let memory_used = sp1_memory_estimate(program, inputs)?;
    let memory_limit = config.memory_limit.min(system_config.memory_limit);
    if memory_used > memory_limit {
        return Err(ZkVmError::ResourceLimitExceeded(format!(
            "execution requires {memory_used} bytes, limit is {memory_limit}"
        )));
    }

    let cycles = sp1_cycle_estimate(program, inputs)?;
    let cycle_limit = config.max_cycles.min(system_config.max_cycles);
    if cycles > cycle_limit {
        return Err(ZkVmError::ResourceLimitExceeded(format!(
            "execution requires {cycles} cycles, limit is {cycle_limit}"
        )));
    }

    Ok(())
}

fn validate_sp1_bytecode(program: &[u8]) -> Result<(), ZkVmError> {
    if program.is_empty() {
        return Err(ZkVmError::InvalidProgram(
            "SP1 bytecode must not be empty".to_string(),
        ));
    }
    Ok(())
}

fn execute_deterministic_sp1_program(program: &[u8], inputs: &[u8]) -> Result<Vec<u8>, ZkVmError> {
    match program {
        SP1_DETERMINISTIC_IDENTITY_PROGRAM => Ok(inputs.to_vec()),
        SP1_DETERMINISTIC_SHA256_PROGRAM => Ok(sha256(inputs)),
        _ => Err(ZkVmError::UnsupportedOperation(
            "SP1 execution requires real SP1 bytecode; this build only supports deterministic receipt test programs".to_string(),
        )),
    }
}

fn sp1_memory_estimate(program: &[u8], inputs: &[u8]) -> Result<u64, ZkVmError> {
    (program.len() as u64)
        .checked_add(inputs.len() as u64)
        .ok_or_else(|| ZkVmError::ResourceLimitExceeded("memory estimate overflow".to_string()))
}

fn sp1_cycle_estimate(program: &[u8], inputs: &[u8]) -> Result<u64, ZkVmError> {
    let memory = sp1_memory_estimate(program, inputs)?;
    memory
        .checked_add(1)
        .ok_or_else(|| ZkVmError::ResourceLimitExceeded("cycle estimate overflow".to_string()))
}

fn build_sp1_receipt(
    program_id: &str,
    agent_id: &AgentId,
    program: &[u8],
    inputs: &[u8],
    output: &[u8],
) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(b"helix-sp1-deterministic-receipt-v1");
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

fn hash_json_map(hasher: &mut Sha256, map: &HashMap<String, Value>) -> Result<(), ZkVmError> {
    let mut entries = map.iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(right.0));
    for (key, value) in entries {
        hasher.update(key.as_bytes());
        let bytes = serde_json::to_vec(value)
            .map_err(|error| ZkVmError::SerializationError(error.to_string()))?;
        hasher.update(bytes);
    }
    Ok(())
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
            system: "sp1".to_string(),
            program_id: "test_sp1_program".to_string(),
            parameters: HashMap::new(),
            generate_proofs: false,
            max_cycles: 50_000,
            memory_limit: 2 * 1024 * 1024,
        }
    }

    #[test]
    fn test_sp1_system_creation() {
        let system = Sp1System::new(Sp1Config::default());
        assert_eq!(system.name(), "sp1");

        let capabilities = system.get_capabilities();
        assert!(!capabilities.supports_proofs);
        assert!(!capabilities.supports_recursion);
        assert_eq!(capabilities.supported_languages.len(), 1);
        assert_eq!(capabilities.proof_system, "sp1-unavailable");
    }

    #[tokio::test]
    async fn test_sp1_program_compilation_fails_closed_for_source() {
        let system = Sp1System::new(Sp1Config::default());

        let result = system
            .compile_program("fn main() {}", ProgramLanguage::Rust)
            .await;

        assert!(matches!(result, Err(ZkVmError::UnsupportedOperation(_))));
    }

    #[tokio::test]
    async fn test_sp1_bytecode_compilation() {
        let system = Sp1System::new(Sp1Config::default());

        let result = system
            .compile_program(
                "HELIX_SP1_DETERMINISTIC_V1:IDENTITY",
                ProgramLanguage::Bytecode,
            )
            .await
            .unwrap();

        assert_eq!(result, SP1_DETERMINISTIC_IDENTITY_PROGRAM);
    }

    #[tokio::test]
    async fn test_sp1_agent_execution() {
        let mut agent = Sp1Agent::new(test_config(), Sp1Config::default()).unwrap();

        let result = agent
            .execute_zkvm(SP1_DETERMINISTIC_IDENTITY_PROGRAM, b"test_sp1_input", false)
            .await
            .unwrap();

        assert_eq!(result.output, b"test_sp1_input");
        assert!(result.proof.is_none());
        assert_eq!(result.receipt.as_ref().unwrap().len(), 32);
        assert!(result.stats.cycles > 0);
        assert_eq!(result.stats.execution_time_ms, 0);
    }

    #[tokio::test]
    async fn test_sp1_agent_id_is_stable() {
        let agent = Sp1Agent::new(test_config(), Sp1Config::default()).unwrap();

        assert_eq!(agent.id(), agent.id());
        assert_eq!(agent.id(), agent.config().id);
        assert!(!agent.id().is_nil());
    }

    #[tokio::test]
    async fn test_sp1_proof_generation_fails_closed() {
        let mut agent = Sp1Agent::new(test_config(), Sp1Config::default()).unwrap();

        let result = agent
            .execute_zkvm(SP1_DETERMINISTIC_SHA256_PROGRAM, b"test_input", true)
            .await;

        assert!(matches!(result, Err(ZkVmError::UnsupportedOperation(_))));
    }

    #[tokio::test]
    async fn test_sp1_proof_verification_fails_closed() {
        let agent = Sp1Agent::new(test_config(), Sp1Config::default()).unwrap();
        let proof = ZkProof::new(
            ProofSystem::Plonk,
            vec![1; 256],
            b"public".to_vec(),
            vec![2; 48],
            "test_sp1_program".to_string(),
        );

        let result = agent.verify_proof(&proof, b"output").await.unwrap();

        assert!(!result.is_valid);
        assert_eq!(result.verification_time_ms, 0);
        assert!(result.error.unwrap().contains("real SP1 verifier"));
    }

    #[tokio::test]
    async fn test_sp1_state_transition_proofs_fail_closed() {
        let mut agent = Sp1Agent::new(test_config(), Sp1Config::default()).unwrap();

        let result = agent.prove_state_transition(b"old", b"new", b"proof").await;

        assert!(matches!(result, Err(ZkVmError::UnsupportedOperation(_))));
    }
}
