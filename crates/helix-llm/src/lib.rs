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

use async_trait::async_trait;
use helix_core::{agent::Agent, event::Event, HelixError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    providers: HashMap<String, Box<dyn LlmProvider>>,
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
            .ok_or_else(|| LlmError::ProviderNotFound(config.provider.clone()))?;

        // Create agent implementation based on provider
        // This would be implemented with specific agent types
        todo!("Implement agent creation based on provider and config")
    }
}

impl Default for LlmAgentFactory {
    fn default() -> Self {
        Self::new()
    }
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
}
