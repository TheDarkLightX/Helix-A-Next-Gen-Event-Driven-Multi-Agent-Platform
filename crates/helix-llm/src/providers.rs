//! LLM provider implementations for various services

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::errors::LlmError;

/// Configuration for an LLM model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Model name/identifier
    pub name: String,
    /// Maximum context length in tokens
    pub max_context_length: usize,
    /// Cost per 1K input tokens (in USD)
    pub input_cost_per_1k: f64,
    /// Cost per 1K output tokens (in USD)
    pub output_cost_per_1k: f64,
    /// Whether the model supports function calling
    pub supports_functions: bool,
    /// Whether the model supports vision/image inputs
    pub supports_vision: bool,
}

/// Request to an LLM provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    /// System prompt
    pub system_prompt: Option<String>,
    /// User messages
    pub messages: Vec<Message>,
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// Temperature for randomness (0.0 to 2.0)
    pub temperature: Option<f32>,
    /// Top-p sampling parameter
    pub top_p: Option<f32>,
    /// Functions available for calling
    pub functions: Option<Vec<Function>>,
    /// Additional model-specific parameters
    pub parameters: HashMap<String, serde_json::Value>,
}

/// A message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender
    pub role: MessageRole,
    /// Content of the message
    pub content: String,
    /// Optional function call information
    pub function_call: Option<FunctionCall>,
}

/// Role of a message sender
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System message (instructions)
    System,
    /// User message
    User,
    /// Assistant message
    Assistant,
    /// Function call result
    Function,
}

/// Function definition for function calling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    /// Function name
    pub name: String,
    /// Function description
    pub description: String,
    /// JSON schema for parameters
    pub parameters: serde_json::Value,
}

/// Function call made by the model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// Name of the function to call
    pub name: String,
    /// Arguments as JSON string
    pub arguments: String,
}

/// Response from an LLM provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    /// Generated content
    pub content: String,
    /// Function call if requested
    pub function_call: Option<FunctionCall>,
    /// Token usage statistics
    pub usage: TokenUsage,
    /// Model used for generation
    pub model: String,
    /// Finish reason
    pub finish_reason: FinishReason,
    /// Response metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Tokens in the prompt
    pub prompt_tokens: u32,
    /// Tokens in the completion
    pub completion_tokens: u32,
    /// Total tokens used
    pub total_tokens: u32,
}

/// Reason why generation finished
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    /// Natural completion
    Stop,
    /// Hit token limit
    Length,
    /// Function call requested
    FunctionCall,
    /// Content filtered
    ContentFilter,
    /// Error occurred
    Error,
}

/// Trait for LLM providers
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Get the name of this provider
    fn name(&self) -> &str;

    /// Get available models
    async fn get_models(&self) -> Result<Vec<ModelConfig>, LlmError>;

    /// Generate a completion
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse, LlmError>;

    /// Stream a completion (returns async iterator)
    async fn stream_complete(
        &self,
        request: LlmRequest,
    ) -> Result<Box<dyn futures::Stream<Item = Result<String, LlmError>> + Unpin + Send>, LlmError>;

    /// Check if the provider is healthy
    async fn health_check(&self) -> Result<(), LlmError>;
}

/// OpenAI provider implementation
pub struct OpenAiProvider {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl OpenAiProvider {
    /// Create a new OpenAI provider
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.openai.com/v1".to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Create a new OpenAI provider with custom base URL
    pub fn with_base_url(api_key: String, base_url: String) -> Self {
        Self {
            api_key,
            base_url,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    async fn get_models(&self) -> Result<Vec<ModelConfig>, LlmError> {
        // Return predefined OpenAI models
        Ok(vec![
            ModelConfig {
                name: "gpt-4".to_string(),
                max_context_length: 8192,
                input_cost_per_1k: 0.03,
                output_cost_per_1k: 0.06,
                supports_functions: true,
                supports_vision: false,
            },
            ModelConfig {
                name: "gpt-4-turbo".to_string(),
                max_context_length: 128000,
                input_cost_per_1k: 0.01,
                output_cost_per_1k: 0.03,
                supports_functions: true,
                supports_vision: true,
            },
            ModelConfig {
                name: "gpt-3.5-turbo".to_string(),
                max_context_length: 16384,
                input_cost_per_1k: 0.0015,
                output_cost_per_1k: 0.002,
                supports_functions: true,
                supports_vision: false,
            },
        ])
    }

    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        // Implementation would make actual API call to OpenAI
        // For now, return a mock response
        Ok(LlmResponse {
            content: "Mock response from OpenAI".to_string(),
            function_call: None,
            usage: TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            },
            model: "gpt-4".to_string(),
            finish_reason: FinishReason::Stop,
            metadata: HashMap::new(),
        })
    }

    async fn stream_complete(
        &self,
        _request: LlmRequest,
    ) -> Result<Box<dyn futures::Stream<Item = Result<String, LlmError>> + Unpin + Send>, LlmError> {
        // Implementation would return streaming response
        todo!("Implement streaming completion")
    }

    async fn health_check(&self) -> Result<(), LlmError> {
        // Make a simple API call to check health
        let response = self.client
            .get(&format!("{}/models", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(LlmError::ApiError(format!("Health check failed: {}", response.status())))
        }
    }
}

/// Anthropic provider implementation
pub struct AnthropicProvider {
    api_key: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn get_models(&self) -> Result<Vec<ModelConfig>, LlmError> {
        Ok(vec![
            ModelConfig {
                name: "claude-3-opus".to_string(),
                max_context_length: 200000,
                input_cost_per_1k: 0.015,
                output_cost_per_1k: 0.075,
                supports_functions: true,
                supports_vision: true,
            },
            ModelConfig {
                name: "claude-3-sonnet".to_string(),
                max_context_length: 200000,
                input_cost_per_1k: 0.003,
                output_cost_per_1k: 0.015,
                supports_functions: true,
                supports_vision: true,
            },
        ])
    }

    async fn complete(&self, _request: LlmRequest) -> Result<LlmResponse, LlmError> {
        // Mock implementation
        Ok(LlmResponse {
            content: "Mock response from Anthropic".to_string(),
            function_call: None,
            usage: TokenUsage {
                prompt_tokens: 12,
                completion_tokens: 8,
                total_tokens: 20,
            },
            model: "claude-3-sonnet".to_string(),
            finish_reason: FinishReason::Stop,
            metadata: HashMap::new(),
        })
    }

    async fn stream_complete(
        &self,
        _request: LlmRequest,
    ) -> Result<Box<dyn futures::Stream<Item = Result<String, LlmError>> + Unpin + Send>, LlmError> {
        todo!("Implement streaming completion")
    }

    async fn health_check(&self) -> Result<(), LlmError> {
        // Implementation would check Anthropic API health
        Ok(())
    }
}
