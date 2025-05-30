//! Error types for embedding operations

use thiserror::Error;

/// Errors that can occur during embedding operations
#[derive(Error, Debug)]
pub enum EmbeddingError {
    /// Embedding generation failed
    #[error("Embedding generation failed: {0}")]
    GenerationError(String),

    /// Search failed
    #[error("Search failed: {0}")]
    SearchError(String),

    /// Storage error
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Invalid dimensions
    #[error("Invalid dimensions: expected {expected}, got {actual}")]
    InvalidDimensions { expected: usize, actual: usize },

    /// Model not found
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Generic internal error
    #[error("Internal embedding error: {0}")]
    InternalError(String),
}

impl From<EmbeddingError> for helix_core::HelixError {
    fn from(err: EmbeddingError) -> Self {
        helix_core::HelixError::InternalError(format!("Embedding error: {}", err))
    }
}
