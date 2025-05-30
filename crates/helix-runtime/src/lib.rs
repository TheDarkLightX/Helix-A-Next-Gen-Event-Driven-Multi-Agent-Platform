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
