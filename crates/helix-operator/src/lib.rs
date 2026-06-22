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
//! deterministic guardrails.
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

pub mod config;
pub mod context;
pub mod errors;
pub mod loop_runner;
pub mod proposals;

pub use config::{DeskOperatorConfig, OperatorActionScope, OperatorStatus};
pub use context::{DeskContext, DeskContextSnapshot, DeskContextSummary};
pub use errors::OperatorError;
pub use loop_runner::{OperatorActivityLog, OperatorActivityEntry, OperatorLoop};
pub use proposals::{
    OperatorAction, OperatorDecision, OperatorProposal, OperatorProposalResponse,
    parse_operator_proposals,
};

#[cfg(test)]
mod tests;
