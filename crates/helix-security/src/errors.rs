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


//! Error types for security operations

use thiserror::Error;

/// Errors that can occur during security operations
#[derive(Error, Debug)]
pub enum SecurityError {
    /// Encryption failed
    #[error("Encryption failed: {0}")]
    EncryptionError(String),

    /// Decryption failed
    #[error("Decryption failed: {0}")]
    DecryptionError(String),

    /// Authentication failed
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    /// Authorization failed
    #[error("Authorization failed: {0}")]
    AuthorizationError(String),

    /// Policy violation
    #[error("Policy violation: {0}")]
    PolicyViolation(String),

    /// Invalid key
    #[error("Invalid key: {0}")]
    InvalidKey(String),

    /// Key not found
    #[error("Key not found: {0}")]
    KeyNotFound(String),

    /// Generic internal error
    #[error("Internal security error: {0}")]
    InternalError(String),
}

impl From<SecurityError> for helix_core::HelixError {
    fn from(err: SecurityError) -> Self {
        helix_core::HelixError::InternalError(format!("Security error: {}", err))
    }
}
