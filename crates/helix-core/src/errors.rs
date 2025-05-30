//! Defines common error types for the Helix Core library.

use crate::event::Event;
use thiserror::Error;
use tokio::sync::mpsc;

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

    /// Represents an encryption error.
    #[error("Encryption error: {0}")]
    EncryptionError(String),
}

// Implement From for MPSC SendError
impl From<mpsc::error::SendError<Event>> for HelixError {
    fn from(err: mpsc::error::SendError<Event>) -> Self {
        HelixError::MpscSendError(format!("Failed to send event on MPSC channel: {}", err))
    }
}

impl HelixError {
    /// Creates a new configuration error
    pub fn config_error<S: Into<String>>(message: S) -> Self {
        Self::ConfigError(message.into())
    }

    /// Creates a new emit error
    pub fn emit_error<S: Into<String>>(message: S) -> Self {
        Self::EmitError(message.into())
    }

    /// Creates a new agent error
    pub fn agent_error<S: Into<String>>(message: S) -> Self {
        Self::AgentError(message.into())
    }

    /// Creates a new policy violation error
    pub fn policy_violation<S: Into<String>>(message: S) -> Self {
        Self::PolicyViolation(message.into())
    }

    /// Creates a new not found error
    pub fn not_found<S: Into<String>>(resource: S) -> Self {
        Self::NotFound(resource.into())
    }

    /// Creates a new validation error
    pub fn validation_error<S: Into<String>>(context: S, message: S) -> Self {
        Self::ValidationError {
            context: context.into(),
            message: message.into(),
        }
    }

    /// Creates a new external service error
    pub fn external_service_error<S: Into<String>>(service: S, details: S) -> Self {
        Self::ExternalServiceError {
            service: service.into(),
            details: details.into(),
        }
    }

    /// Creates a new WASM error
    pub fn wasm_error<S: Into<String>>(message: S) -> Self {
        Self::WasmError(message.into())
    }

    /// Creates a new internal error
    pub fn internal_error<S: Into<String>>(message: S) -> Self {
        Self::InternalError(message.into())
    }

    /// Creates a new encryption error
    pub fn encryption_error<S: Into<String>>(message: S) -> Self {
        Self::EncryptionError(message.into())
    }

    /// Checks if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::ConfigError(_) => false,
            Self::IoError(_) => true,
            Self::SerdeError(_) => false,
            Self::EmitError(_) => true,
            Self::AgentError(_) => true,
            Self::PolicyViolation(_) => false,
            Self::NotFound(_) => false,
            Self::ValidationError { .. } => false,
            Self::ExternalServiceError { .. } => true,
            Self::WasmError(_) => true,
            Self::InternalError(_) => false,
            Self::MpscSendError(_) => true,
            Self::EncryptionError(_) => false,
        }
    }

    /// Gets the error category for logging/monitoring
    pub fn category(&self) -> &'static str {
        match self {
            Self::ConfigError(_) => "configuration",
            Self::IoError(_) => "io",
            Self::SerdeError(_) => "serialization",
            Self::EmitError(_) => "event",
            Self::AgentError(_) => "agent",
            Self::PolicyViolation(_) => "policy",
            Self::NotFound(_) => "resource",
            Self::ValidationError { .. } => "validation",
            Self::ExternalServiceError { .. } => "external",
            Self::WasmError(_) => "wasm",
            Self::InternalError(_) => "internal",
            Self::MpscSendError(_) => "channel",
            Self::EncryptionError(_) => "encryption",
        }
    }

    /// Gets the severity level of the error
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::ConfigError(_) => ErrorSeverity::Critical,
            Self::IoError(_) => ErrorSeverity::Medium,
            Self::SerdeError(_) => ErrorSeverity::Medium,
            Self::EmitError(_) => ErrorSeverity::Low,
            Self::AgentError(_) => ErrorSeverity::Medium,
            Self::PolicyViolation(_) => ErrorSeverity::High,
            Self::NotFound(_) => ErrorSeverity::Low,
            Self::ValidationError { .. } => ErrorSeverity::Medium,
            Self::ExternalServiceError { .. } => ErrorSeverity::Medium,
            Self::WasmError(_) => ErrorSeverity::High,
            Self::InternalError(_) => ErrorSeverity::Critical,
            Self::MpscSendError(_) => ErrorSeverity::Medium,
            Self::EncryptionError(_) => ErrorSeverity::High,
        }
    }
}

/// Error severity levels for monitoring and alerting
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorSeverity {
    /// Low severity - informational, doesn't affect functionality
    Low,
    /// Medium severity - may affect some functionality
    Medium,
    /// High severity - affects important functionality
    High,
    /// Critical severity - system-breaking error
    Critical,
}

impl std::fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "LOW"),
            Self::Medium => write!(f, "MEDIUM"),
            Self::High => write!(f, "HIGH"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use tokio::sync::mpsc;

    #[test]
    fn test_config_error_creation() {
        let error = HelixError::config_error("Invalid configuration");
        assert!(matches!(error, HelixError::ConfigError(_)));
        assert_eq!(error.to_string(), "Configuration Error: Invalid configuration");
        assert_eq!(error.category(), "configuration");
        assert_eq!(error.severity(), ErrorSeverity::Critical);
        assert!(!error.is_recoverable());
    }

    #[test]
    fn test_emit_error_creation() {
        let error = HelixError::emit_error("Failed to emit event");
        assert!(matches!(error, HelixError::EmitError(_)));
        assert_eq!(error.to_string(), "Event Emission Failed: Failed to emit event");
        assert_eq!(error.category(), "event");
        assert_eq!(error.severity(), ErrorSeverity::Low);
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_agent_error_creation() {
        let error = HelixError::agent_error("Agent execution failed");
        assert!(matches!(error, HelixError::AgentError(_)));
        assert_eq!(error.to_string(), "Agent Execution Failed: Agent execution failed");
        assert_eq!(error.category(), "agent");
        assert_eq!(error.severity(), ErrorSeverity::Medium);
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_policy_violation_creation() {
        let error = HelixError::policy_violation("Access denied");
        assert!(matches!(error, HelixError::PolicyViolation(_)));
        assert_eq!(error.to_string(), "Policy Violation: Access denied");
        assert_eq!(error.category(), "policy");
        assert_eq!(error.severity(), ErrorSeverity::High);
        assert!(!error.is_recoverable());
    }

    #[test]
    fn test_not_found_error_creation() {
        let error = HelixError::not_found("User with ID 123");
        assert!(matches!(error, HelixError::NotFound(_)));
        assert_eq!(error.to_string(), "Resource Not Found: User with ID 123");
        assert_eq!(error.category(), "resource");
        assert_eq!(error.severity(), ErrorSeverity::Low);
        assert!(!error.is_recoverable());
    }

    #[test]
    fn test_validation_error_creation() {
        let error = HelixError::validation_error("email", "Invalid email format");
        assert!(matches!(error, HelixError::ValidationError { .. }));
        assert_eq!(error.to_string(), "Validation Error (email): Invalid email format");
        assert_eq!(error.category(), "validation");
        assert_eq!(error.severity(), ErrorSeverity::Medium);
        assert!(!error.is_recoverable());
    }

    #[test]
    fn test_external_service_error_creation() {
        let error = HelixError::external_service_error("payment_api", "Connection timeout");
        assert!(matches!(error, HelixError::ExternalServiceError { .. }));
        assert_eq!(error.to_string(), "External Service Error (payment_api): Connection timeout");
        assert_eq!(error.category(), "external");
        assert_eq!(error.severity(), ErrorSeverity::Medium);
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_wasm_error_creation() {
        let error = HelixError::wasm_error("Module compilation failed");
        assert!(matches!(error, HelixError::WasmError(_)));
        assert_eq!(error.to_string(), "WASM Runtime Error: Module compilation failed");
        assert_eq!(error.category(), "wasm");
        assert_eq!(error.severity(), ErrorSeverity::High);
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_internal_error_creation() {
        let error = HelixError::internal_error("Unexpected state");
        assert!(matches!(error, HelixError::InternalError(_)));
        assert_eq!(error.to_string(), "Internal Error: Unexpected state");
        assert_eq!(error.category(), "internal");
        assert_eq!(error.severity(), ErrorSeverity::Critical);
        assert!(!error.is_recoverable());
    }

    #[test]
    fn test_encryption_error_creation() {
        let error = HelixError::encryption_error("Key derivation failed");
        assert!(matches!(error, HelixError::EncryptionError(_)));
        assert_eq!(error.to_string(), "Encryption error: Key derivation failed");
        assert_eq!(error.category(), "encryption");
        assert_eq!(error.severity(), ErrorSeverity::High);
        assert!(!error.is_recoverable());
    }

    #[test]
    fn test_io_error_conversion() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "File not found");
        let helix_error: HelixError = io_error.into();

        assert!(matches!(helix_error, HelixError::IoError(_)));
        assert_eq!(helix_error.category(), "io");
        assert_eq!(helix_error.severity(), ErrorSeverity::Medium);
        assert!(helix_error.is_recoverable());
    }

    #[test]
    fn test_serde_error_conversion() {
        let json_str = r#"{"invalid": json"#;
        let serde_error = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let helix_error: HelixError = serde_error.into();

        assert!(matches!(helix_error, HelixError::SerdeError(_)));
        assert_eq!(helix_error.category(), "serialization");
        assert_eq!(helix_error.severity(), ErrorSeverity::Medium);
        assert!(!helix_error.is_recoverable());
    }

    #[test]
    fn test_mpsc_error_conversion() {
        let (tx, rx) = mpsc::channel::<Event>(1);
        drop(rx); // Close receiver to cause send error

        let event = Event::new(
            "/test/source".to_string(),
            "com.example.test".to_string(),
            Some(serde_json::json!({})),
        );

        let send_result = tx.try_send(event);
        assert!(send_result.is_err());

        // Test the conversion (we can't directly convert try_send error, but we can test the pattern)
        let helix_error = HelixError::MpscSendError("Test send error".to_string());
        assert!(matches!(helix_error, HelixError::MpscSendError(_)));
        assert_eq!(helix_error.category(), "channel");
        assert_eq!(helix_error.severity(), ErrorSeverity::Medium);
        assert!(helix_error.is_recoverable());
    }

    #[test]
    fn test_error_severity_ordering() {
        assert!(ErrorSeverity::Low < ErrorSeverity::Medium);
        assert!(ErrorSeverity::Medium < ErrorSeverity::High);
        assert!(ErrorSeverity::High < ErrorSeverity::Critical);
    }

    #[test]
    fn test_error_severity_display() {
        assert_eq!(ErrorSeverity::Low.to_string(), "LOW");
        assert_eq!(ErrorSeverity::Medium.to_string(), "MEDIUM");
        assert_eq!(ErrorSeverity::High.to_string(), "HIGH");
        assert_eq!(ErrorSeverity::Critical.to_string(), "CRITICAL");
    }

    #[test]
    fn test_error_categories_comprehensive() {
        let errors = vec![
            HelixError::config_error("test"),
            HelixError::emit_error("test"),
            HelixError::agent_error("test"),
            HelixError::policy_violation("test"),
            HelixError::not_found("test"),
            HelixError::validation_error("field", "message"),
            HelixError::external_service_error("service", "details"),
            HelixError::wasm_error("test"),
            HelixError::internal_error("test"),
            HelixError::encryption_error("test"),
            HelixError::MpscSendError("test".to_string()),
        ];

        let expected_categories = [
            "configuration", "event", "agent", "policy", "resource",
            "validation", "external", "wasm", "internal", "encryption", "channel"
        ];

        for (error, expected_category) in errors.iter().zip(expected_categories.iter()) {
            assert_eq!(error.category(), *expected_category);
        }
    }

    #[test]
    fn test_recoverable_errors() {
        let recoverable_errors = vec![
            HelixError::IoError(io::Error::other("test")),
            HelixError::emit_error("test"),
            HelixError::agent_error("test"),
            HelixError::external_service_error("service", "details"),
            HelixError::wasm_error("test"),
            HelixError::MpscSendError("test".to_string()),
        ];

        for error in recoverable_errors {
            assert!(error.is_recoverable(), "Error should be recoverable: {:?}", error);
        }
    }

    #[test]
    fn test_non_recoverable_errors() {
        let non_recoverable_errors = vec![
            HelixError::config_error("test"),
            HelixError::policy_violation("test"),
            HelixError::not_found("test"),
            HelixError::validation_error("field", "message"),
            HelixError::internal_error("test"),
            HelixError::encryption_error("test"),
        ];

        for error in non_recoverable_errors {
            assert!(!error.is_recoverable(), "Error should not be recoverable: {:?}", error);
        }
    }

    #[test]
    fn test_error_debug_format() {
        let error = HelixError::validation_error("email", "Invalid format");
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("ValidationError"));
        assert!(debug_str.contains("email"));
        assert!(debug_str.contains("Invalid format"));
    }

    #[test]
    fn test_error_equality() {
        let error1 = HelixError::config_error("same message");
        let error2 = HelixError::config_error("same message");
        let error3 = HelixError::config_error("different message");

        // Note: HelixError doesn't implement PartialEq, so we test string representation
        assert_eq!(error1.to_string(), error2.to_string());
        assert_ne!(error1.to_string(), error3.to_string());
    }
}
