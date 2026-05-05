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

#![warn(missing_docs)]

//! Helix runtime: imperative orchestration for executing SDK agents.
//!
//! The runtime is intentionally an *imperative shell* around the deterministic cores
//! in `helix-core`. It owns side effects (messaging, plugin execution, IO) and treats
//! all non-deterministic inputs (e.g., LLM outputs) as untrusted data that must be
//! gated by deterministic kernels.

pub mod agent_registry;
pub mod agent_runner;
pub mod imperative_shell;
pub mod messaging;

pub use messaging::{InMemoryEventCollector, MessagingError, NatsClient, NatsConfig, StreamConfig};

/// Lifecycle status for a managed agent instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentStatus {
    /// The runtime is setting up the agent.
    Initializing,
    /// The agent is running.
    Running,
    /// The agent has been requested to stop.
    Stopping,
    /// The agent is stopped.
    Stopped,
    /// The agent errored and is no longer running.
    Errored,
    /// The agent completed successfully (for finite agents).
    Completed,
}
