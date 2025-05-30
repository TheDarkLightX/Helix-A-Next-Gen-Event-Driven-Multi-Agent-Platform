//! Error types for zkVM operations

use thiserror::Error;

/// Errors that can occur during zkVM operations
#[derive(Error, Debug)]
pub enum ZkVmError {
    /// zkVM system not found or not registered
    #[error("zkVM system not found: {0}")]
    SystemNotFound(String),

    /// Program compilation failed
    #[error("Program compilation failed: {0}")]
    CompilationError(String),

    /// Execution failed
    #[error("Execution failed: {0}")]
    ExecutionError(String),

    /// Proof generation failed
    #[error("Proof generation failed: {0}")]
    ProofGenerationError(String),

    /// Proof verification failed
    #[error("Proof verification failed: {0}")]
    ProofVerificationError(String),

    /// Invalid program or bytecode
    #[error("Invalid program: {0}")]
    InvalidProgram(String),

    /// Resource limit exceeded (cycles, memory, etc.)
    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Invalid input data
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Invalid output data
    #[error("Invalid output: {0}")]
    InvalidOutput(String),

    /// Circuit constraint violation
    #[error("Circuit constraint violation: {0}")]
    ConstraintViolation(String),

    /// Unsupported operation
    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// System resource error (out of memory, etc.)
    #[error("System resource error: {0}")]
    SystemResourceError(String),

    /// Timeout during execution or proof generation
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Generic internal error
    #[error("Internal zkVM error: {0}")]
    InternalError(String),
}

impl From<ZkVmError> for helix_core::HelixError {
    fn from(err: ZkVmError) -> Self {
        helix_core::HelixError::InternalError(format!("zkVM error: {}", err))
    }
}
