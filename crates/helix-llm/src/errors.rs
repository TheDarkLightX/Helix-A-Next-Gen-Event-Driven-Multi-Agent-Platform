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


//! Error types for LLM operations

use thiserror::Error;

/// Errors that can occur during LLM operations
#[derive(Error, Debug)]
pub enum LlmError {
    /// Provider not found or not registered
    #[error("LLM provider not found: {0}")]
    ProviderNotFound(String),

    /// API request failed
    #[error("API request failed: {0}")]
    ApiError(String),

    /// Authentication failed
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {0}")]
    RateLimitError(String),

    /// Invalid model configuration
    #[error("Invalid model configuration: {0}")]
    ConfigurationError(String),

    /// Tokenization error
    #[error("Tokenization error: {0}")]
    TokenizationError(String),

    /// Parsing error when extracting structured data
    #[error("Parsing error: {0}")]
    ParsingError(String),

    /// Context too large for model
    #[error("Context too large: {0} tokens, max: {1}")]
    ContextTooLarge(usize, usize),

    /// Model not supported
    #[error("Model not supported: {0}")]
    ModelNotSupported(String),

    /// Network error
    #[error("Network error: {0}")]
    NetworkError(String),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// HTTP request error
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    /// Generic internal error
    #[error("Internal LLM error: {0}")]
    InternalError(String),
}

impl From<LlmError> for helix_core::HelixError {
    fn from(err: LlmError) -> Self {
        helix_core::HelixError::InternalError(format!("LLM error: {}", err))
    }
}
