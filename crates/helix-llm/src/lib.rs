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

#![warn(missing_docs)]

//! LLM integration and natural language processing capabilities for Helix.
//!
//! This crate provides:
//! - Multiple LLM provider integrations (OpenAI, Anthropic, local models)
//! - Natural language rule parsing and generation
//! - Agent behavior synthesis from text descriptions
//! - Context-aware prompt engineering
//! - Token management and cost optimization

pub mod agents;
pub mod context;
pub mod errors;
/// Utilities for parsing intent facets from natural language prompts.
pub mod intent_lattice;
pub mod parsers;
pub mod prompts;
pub mod providers;

pub use context::{AgentContext, ConversationContext};
pub use errors::LlmError;
pub use providers::{LlmProvider, LlmRequest, LlmResponse, ModelConfig};

use crate::agents::{LlmActionAgent, LlmSourceAgent, LlmTransformerAgent};
use async_trait::async_trait;
use helix_core::{
    agent::{Agent, AgentConfig},
    event::Event,
    types::AgentId,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

const DEFAULT_LLM_PROFILE_ID: &str = "50000000-0000-0000-0000-000000000010";

/// Configuration for LLM-powered agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmAgentConfig {
    /// The LLM provider to use (openai, anthropic, local)
    pub provider: String,
    /// Model name (gpt-4, claude-3, llama-2-7b, etc.)
    pub model: String,
    /// System prompt template
    pub system_prompt: String,
    /// Maximum tokens for responses
    pub max_tokens: u32,
    /// Temperature for response generation
    pub temperature: f32,
    /// Custom parameters for the model
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Trait for LLM-powered agents that can understand and generate natural language
#[async_trait]
pub trait LlmAgent: Agent {
    /// Process natural language input and generate appropriate responses or actions
    async fn process_natural_language(
        &mut self,
        input: &str,
        context: &AgentContext,
    ) -> Result<LlmResponse, LlmError>;

    /// Generate a recipe from natural language description
    async fn synthesize_recipe(
        &mut self,
        description: &str,
        context: &AgentContext,
    ) -> Result<helix_core::recipe::Recipe, LlmError>;

    /// Analyze an event and suggest appropriate actions
    async fn analyze_event(
        &mut self,
        event: &Event,
        context: &AgentContext,
    ) -> Result<Vec<String>, LlmError>;
}

/// Factory for creating LLM-powered agents
pub struct LlmAgentFactory {
    providers: HashMap<String, Arc<dyn LlmProvider>>,
}

impl LlmAgentFactory {
    /// Create a new LLM agent factory
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Register an LLM provider
    pub fn register_provider(&mut self, name: String, provider: Box<dyn LlmProvider>) {
        self.providers.insert(name, Arc::from(provider));
    }

    /// Register a shared LLM provider
    pub fn register_shared_provider(&mut self, name: String, provider: Arc<dyn LlmProvider>) {
        self.providers.insert(name, provider);
    }

    /// Create an LLM agent with the given configuration
    pub async fn create_agent(
        &self,
        config: LlmAgentConfig,
    ) -> Result<Box<dyn LlmAgent>, LlmError> {
        let provider = self
            .providers
            .get(&config.provider)
            .cloned()
            .ok_or_else(|| LlmError::ProviderNotFound(config.provider.clone()))?;
        let agent_role = llm_agent_role(&config)?;
        let agent_config = llm_agent_config(&config, &agent_role)?;

        match agent_role.as_str() {
            "source" => Ok(Box::new(LlmSourceAgent::new(
                agent_config,
                config,
                provider,
            ))),
            "transformer" => Ok(Box::new(LlmTransformerAgent::new(
                agent_config,
                config,
                provider,
            ))),
            "action" => Ok(Box::new(LlmActionAgent::new(
                agent_config,
                config,
                provider,
            ))),
            _ => Err(LlmError::ModelNotSupported(format!(
                "unsupported llm agent role: {agent_role}"
            ))),
        }
    }
}

impl Default for LlmAgentFactory {
    fn default() -> Self {
        Self::new()
    }
}

fn llm_agent_role(config: &LlmAgentConfig) -> Result<String, LlmError> {
    let role = config
        .parameters
        .get("agent_role")
        .or_else(|| config.parameters.get("agent_type"))
        .or_else(|| config.parameters.get("agent_kind"))
        .map(|value| {
            value.as_str().ok_or_else(|| {
                LlmError::ConfigurationError(
                    "agent_role, agent_type, or agent_kind must be a string".to_string(),
                )
            })
        })
        .transpose()?
        .unwrap_or("source")
        .trim()
        .to_ascii_lowercase();

    match role.as_str() {
        "source" | "transformer" | "action" => Ok(role),
        _ => Err(LlmError::ModelNotSupported(format!(
            "unsupported llm agent role: {role}"
        ))),
    }
}

fn llm_agent_config(config: &LlmAgentConfig, role: &str) -> Result<AgentConfig, LlmError> {
    let agent_id = match parameter_string(&config.parameters, "agent_id")? {
        Some(value) => parse_uuid_parameter("agent_id", value)?,
        None => deterministic_llm_agent_id(config, role),
    };
    let profile_id = match parameter_string(&config.parameters, "profile_id")? {
        Some(value) => parse_uuid_parameter("profile_id", value)?,
        None => Uuid::parse_str(DEFAULT_LLM_PROFILE_ID).map_err(|error| {
            LlmError::ConfigurationError(format!("invalid default profile id: {error}"))
        })?,
    };
    let name = parameter_string(&config.parameters, "agent_name")?.map(str::to_string);
    let config_data = serde_json::to_value(config)?;

    Ok(AgentConfig::new(
        agent_id,
        profile_id,
        name,
        format!("llm_{role}"),
        config_data,
    ))
}

fn parameter_string<'a>(
    parameters: &'a HashMap<String, serde_json::Value>,
    key: &str,
) -> Result<Option<&'a str>, LlmError> {
    parameters
        .get(key)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| LlmError::ConfigurationError(format!("{key} must be a string")))
        })
        .transpose()
}

fn parse_uuid_parameter(context: &str, value: &str) -> Result<Uuid, LlmError> {
    let parsed = Uuid::parse_str(value.trim())
        .map_err(|_| LlmError::ConfigurationError(format!("{context} must be a valid UUID")))?;
    if parsed.is_nil() {
        return Err(LlmError::ConfigurationError(format!(
            "{context} must not be nil"
        )));
    }
    Ok(parsed)
}

fn deterministic_llm_agent_id(config: &LlmAgentConfig, role: &str) -> AgentId {
    let mut hasher = Sha256::new();
    hasher.update(config.provider.as_bytes());
    hasher.update(config.model.as_bytes());
    hasher.update(config.system_prompt.as_bytes());
    hasher.update(role.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

/// Utility functions for LLM integration
pub mod utils {
    use super::*;

    /// Count tokens in a text string (rough approximation)
    pub fn count_tokens(text: &str, _model: &str) -> Result<usize, LlmError> {
        // Rough approximation: 1 token ≈ 4 characters for English text
        // This would be replaced with actual tiktoken implementation when available
        Ok(text.len().div_ceil(4))
    }

    /// Truncate text to fit within token limit
    pub fn truncate_to_tokens(
        text: &str,
        model: &str,
        max_tokens: usize,
    ) -> Result<String, LlmError> {
        let estimated_tokens = count_tokens(text, model)?;

        if estimated_tokens <= max_tokens {
            return Ok(text.to_string());
        }

        // Rough truncation based on character count
        let chars_per_token = text.len() / estimated_tokens.max(1);
        let max_chars = max_tokens * chars_per_token;

        if max_chars < text.len() {
            Ok(text[..max_chars].to_string())
        } else {
            Ok(text.to_string())
        }
    }

    /// Extract structured data from LLM response using regex patterns
    pub fn extract_structured_data(
        response: &str,
        patterns: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut extracted = HashMap::new();

        for (key, pattern) in patterns {
            if let Ok(regex) = regex::Regex::new(pattern) {
                if let Some(captures) = regex.captures(response) {
                    if let Some(matched) = captures.get(1) {
                        extracted.insert(key.clone(), matched.as_str().to_string());
                    }
                }
            }
        }

        extracted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{FinishReason, TokenUsage};
    use futures::stream;

    struct EchoProvider;

    #[async_trait]
    impl LlmProvider for EchoProvider {
        fn name(&self) -> &str {
            "echo"
        }

        async fn get_models(&self) -> Result<Vec<ModelConfig>, LlmError> {
            Ok(vec![ModelConfig {
                name: "echo-model".to_string(),
                max_context_length: 8_192,
                input_cost_per_1k: 0.0,
                output_cost_per_1k: 0.0,
                supports_functions: false,
                supports_vision: false,
            }])
        }

        async fn complete(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
            let content = request
                .messages
                .last()
                .map(|message| message.content.clone())
                .unwrap_or_else(|| "empty".to_string());
            Ok(LlmResponse {
                content,
                function_call: None,
                usage: TokenUsage {
                    prompt_tokens: 1,
                    completion_tokens: 1,
                    total_tokens: 2,
                },
                model: "echo-model".to_string(),
                finish_reason: FinishReason::Stop,
                metadata: HashMap::new(),
            })
        }

        async fn stream_complete(
            &self,
            _request: LlmRequest,
        ) -> Result<
            Box<dyn futures::Stream<Item = Result<String, LlmError>> + Unpin + Send>,
            LlmError,
        > {
            Ok(Box::new(stream::empty()))
        }

        async fn health_check(&self) -> Result<(), LlmError> {
            Ok(())
        }
    }

    #[test]
    fn test_llm_agent_config_serialization() {
        let config = LlmAgentConfig {
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
            system_prompt: "You are a helpful assistant".to_string(),
            max_tokens: 1000,
            temperature: 0.7,
            parameters: HashMap::new(),
        };

        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: LlmAgentConfig = serde_json::from_str(&serialized).unwrap();

        assert_eq!(config.provider, deserialized.provider);
        assert_eq!(config.model, deserialized.model);
    }

    #[tokio::test]
    async fn test_llm_agent_factory_creation() {
        let factory = LlmAgentFactory::new();
        assert_eq!(factory.providers.len(), 0);
    }

    #[tokio::test]
    async fn llm_agent_factory_creates_registered_source_agent() {
        let mut factory = LlmAgentFactory::new();
        factory.register_provider("echo".to_string(), Box::new(EchoProvider));
        let mut parameters = HashMap::new();
        parameters.insert("agent_role".to_string(), serde_json::json!("source"));
        parameters.insert("agent_name".to_string(), serde_json::json!("Echo source"));

        let agent_config = LlmAgentConfig {
            provider: "echo".to_string(),
            model: "echo-model".to_string(),
            system_prompt: "echo".to_string(),
            max_tokens: 64,
            temperature: 0.0,
            parameters,
        };

        let mut agent = factory.create_agent(agent_config).await.unwrap();
        assert_eq!(agent.config().agent_kind, "llm_source");
        assert_eq!(agent.config().name.as_deref(), Some("Echo source"));

        let context = AgentContext::new(agent.id(), agent.config().profile_id);
        let response = agent
            .process_natural_language("hello", &context)
            .await
            .unwrap();
        assert_eq!(response.content, "hello");
    }

    #[tokio::test]
    async fn llm_agent_factory_fails_closed_for_unknown_role() {
        let mut factory = LlmAgentFactory::new();
        factory.register_provider("echo".to_string(), Box::new(EchoProvider));
        let mut parameters = HashMap::new();
        parameters.insert("agent_role".to_string(), serde_json::json!("scheduler"));

        let agent_config = LlmAgentConfig {
            provider: "echo".to_string(),
            model: "echo-model".to_string(),
            system_prompt: "echo".to_string(),
            max_tokens: 64,
            temperature: 0.0,
            parameters,
        };

        let error = match factory.create_agent(agent_config).await {
            Ok(_) => panic!("unknown role should fail closed"),
            Err(error) => error,
        };
        assert!(matches!(error, LlmError::ModelNotSupported(_)));
    }
}
