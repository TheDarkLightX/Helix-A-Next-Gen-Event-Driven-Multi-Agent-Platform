/// Represents the lifecycle status of an agent.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AgentStatus {
    /// The agent is being initialized.
    Initializing,
    /// The agent is currently running.
    Running,
    /// The agent has been stopped.
    Stopped,
    /// The agent has encountered an error.
    Errored,
    /// The agent has completed its execution successfully.
    Completed,
}
#![warn(missing_docs)]

//! The core runtime engine for Helix, managing agent execution and event flow.

pub use messaging::{NatsConfig, NatsClient, StreamConfig, MessagingError};

pub mod agent_runner;
pub mod agent_registry; // Added agent_registry module
pub mod messaging;
