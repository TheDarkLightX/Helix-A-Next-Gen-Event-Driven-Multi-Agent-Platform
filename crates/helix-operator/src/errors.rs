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

//! Operator error types.

use thiserror::Error;

/// Errors produced by the desk operator.
#[derive(Error, Debug)]
pub enum OperatorError {
    /// The LLM provider is not configured.
    #[error("LLM provider not configured")]
    LlmNotConfigured,
    /// The LLM returned an error.
    #[error("LLM error: {0}")]
    LlmError(String),
    /// The LLM response could not be parsed into proposals.
    #[error("parse error: {0}")]
    ParseError(String),
    /// The operator is not running.
    #[error("operator not running")]
    NotRunning,
    /// The operator is already running.
    #[error("operator already running")]
    AlreadyRunning,
    /// The guard denied all proposed actions.
    #[error("all {0} proposals denied by guard")]
    AllDenied(usize),
    /// Configuration validation failed.
    #[error("config validation failed: {0}")]
    ConfigValidation(String),
    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(String),
}
