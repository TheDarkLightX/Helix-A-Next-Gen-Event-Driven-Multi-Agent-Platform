//! Defines common error types for the Helix Core library.

use thiserror::Error;
use tokio::sync::mpsc;
use crate::event::Event;

/// The primary error type for Helix operations.
#[derive(Error, Debug)]
pub enum HelixError {
    /// Error related to configuration loading or validation.
    #[error("Configuration Error: {0}")]
    ConfigError(String),

    /// Error during file or network I/O operations.
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),

    /// Error during serialization or deserialization (e.g., JSON parsing).
    #[error("Serialization/Deserialization Error: {0}")]
    SerdeError(#[from] serde_json::Error),

    /// Error occurred while trying to emit an event.
    #[error("Event Emission Failed: {0}")]
    EmitError(String),

    /// Error occurred during the execution of an agent's logic.
    #[error("Agent Execution Failed: {0}")]
    AgentError(String),

    /// Error indicating a violation of a configured policy.
    #[error("Policy Violation: {0}")]
    PolicyViolation(String),

    /// Error indicating a requested resource was not found.
    #[error("Resource Not Found: {0}")]
    NotFound(String),

    /// Error indicating a failure during data validation.
    #[error("Validation Error ({context}): {message}")]
    ValidationError {
        /// Context or field where validation failed.
        context: String,
        /// Specific validation failure message.
        message: String,
    },

    /// Error reported by an external service.
    #[error("External Service Error ({service}): {details}")]
    ExternalServiceError {
        /// The name of the external service reporting the error.
        service: String,
        /// Detailed error message from the service.
        details: String,
    },

    /// Error related to WebAssembly module loading or execution.
    #[error("WASM Runtime Error: {0}")]
    WasmError(String), // Consider specific WASM error types

    /// Represents an unexpected internal error.
    #[error("Internal Error: {0}")]
    InternalError(String),

    /// Error sending an event via MPSC channel.
    #[error("MPSC send error: {0}")]
    MpscSendError(String), // Add variant to hold MPSC error string
}

// Implement From for MPSC SendError
impl From<mpsc::error::SendError<Event>> for HelixError {
    fn from(err: mpsc::error::SendError<Event>) -> Self {
        HelixError::MpscSendError(format!("Failed to send event on MPSC channel: {}", err))
    }
}

// Example of converting a specific library error
// impl From<some_library::Error> for HelixError {
//     fn from(err: some_library::Error) -> Self {
//         HelixError::InternalError(err.to_string())
//     }
// }
