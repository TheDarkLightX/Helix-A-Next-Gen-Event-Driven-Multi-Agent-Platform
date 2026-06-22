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

//! LLM desk operator: autonomous intelligence desk operation within
//! deterministic guardrails, with CoPilot mode for multi-participant
//! collaboration.
//!
//! The operator runs an observe-think-act loop:
//!
//! 1. **Observe** — [`DeskContext`] assembles a structured snapshot of the
//!    desk's current state (cases, evidence, watchlists, recent decisions).
//! 2. **Think** — the LLM receives the context and a system prompt encoding
//!    the user's rules, then proposes one or more actions.
//! 3. **Act** — each proposed action is routed through the
//!    [`AutopilotGuardMachine`] for deterministic evaluation. Only actions
//!    the guard allows are executed. Denied actions are logged.
//! 4. **Remember** — the operator records its decisions and outcomes for
//!    audit and future context.
//!
//! The LLM never bypasses the guard. The user defines the rules (via
//! [`DeskOperatorConfig`] and [`AutopilotGuardConfig`]); the guard enforces
//! them deterministically; the LLM operates freely within those bounds.
//!
//! ## CoPilot Mode
//!
//! In CoPilot mode, multiple humans and AI copilots can co-operate the same
//! desk together. The [`OperatorRegistry`] tracks who's connected, the
//! [`ConfirmationQueue`] holds AI proposals awaiting human review (in assist
//! mode), and the [`CollaborationBroadcaster`] pushes real-time events to all
//! connected participants via Server-Sent Events.

pub mod collaboration;
pub mod config;
pub mod confirmation;
pub mod context;
pub mod errors;
pub mod loop_runner;
pub mod proposals;
pub mod registry;
pub mod session;

pub use collaboration::{CollaborationBroadcaster, CollaborationEvent};
pub use config::{DeskOperatorConfig, OperatorActionScope, OperatorStatus};
pub use confirmation::{ConfirmationQueue, ConfirmationRequest, ConfirmationStatus};
pub use context::{DeskContext, DeskContextSnapshot, DeskContextSummary};
pub use errors::OperatorError;
pub use loop_runner::{OperatorActivityEntry, OperatorActivityLog, OperatorLoop};
pub use proposals::{
    OperatorAction, OperatorDecision, OperatorProposal, OperatorProposalResponse,
    parse_operator_proposals,
};
pub use registry::OperatorRegistry;
pub use session::{
    JoinSessionRequest, OperatorSession, SessionKind, SessionRole, SessionStatus,
};

#[cfg(test)]
mod tests;
