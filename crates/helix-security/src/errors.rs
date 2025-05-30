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
