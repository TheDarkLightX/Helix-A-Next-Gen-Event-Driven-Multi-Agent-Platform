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

//! Pure execution kernel for recipe lifecycle state transitions.
//!
//! This module is intentionally side-effect free. It is designed to be
//! model-checked and mirrored by ESSO artifacts.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Upper bound for concurrent agents tracked by the kernel.
pub const MAX_KERNEL_AGENTS: u16 = 16;

/// High-level execution phases for a recipe run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionPhase {
    /// No run in progress.
    Idle,
    /// Run in progress; at least one agent still pending.
    Running,
    /// Run completed without failures.
    Succeeded,
    /// Run terminated due to at least one failure.
    Failed,
}

/// Kernel state that is updated by the pure step function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionState {
    /// Current phase.
    pub phase: ExecutionPhase,
    /// Remaining agents not yet reported complete.
    pub remaining_agents: u16,
    /// Failure latch.
    pub failed: bool,
}

impl Default for ExecutionState {
    fn default() -> Self {
        Self {
            phase: ExecutionPhase::Idle,
            remaining_agents: 0,
            failed: false,
        }
    }
}

impl ExecutionState {
    /// Returns true when state satisfies kernel invariants.
    pub fn is_valid(self) -> bool {
        match self.phase {
            ExecutionPhase::Idle => self.remaining_agents == 0 && !self.failed,
            ExecutionPhase::Running => self.remaining_agents > 0 && !self.failed,
            ExecutionPhase::Succeeded => self.remaining_agents == 0 && !self.failed,
            ExecutionPhase::Failed => self.remaining_agents == 0 && self.failed,
        }
    }
}

/// Deterministic input events accepted by the kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionInput {
    /// Start a new run with a fixed number of agents.
    Start { agent_count: u16 },
    /// One agent completed successfully.
    AgentCompleted,
    /// At least one agent failed.
    AgentFailed,
    /// Reset terminal state to `Idle`.
    Reset,
}

/// Declarative effects that the imperative shell should execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionEffect {
    /// Begin run and fan out side-effectful agent starts.
    BeginRun { agent_count: u16 },
    /// Persist progress telemetry.
    Progress { remaining_agents: u16 },
    /// Emit successful completion.
    Succeeded,
    /// Emit failure and stop outstanding work.
    Failed,
    /// Mark run cleanup complete.
    Reset,
}

/// Invalid transitions or invalid kernel inputs.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum KernelError {
    /// `Start` was called with zero or out-of-range agent count.
    #[error("invalid start count: {0}, expected 1..={max}", max = MAX_KERNEL_AGENTS)]
    InvalidStartCount(u16),
    /// Input is not legal from current phase.
    #[error("invalid transition: phase={phase:?}, input={input:?}")]
    InvalidTransition {
        /// Current phase.
        phase: ExecutionPhase,
        /// Disallowed input.
        input: ExecutionInput,
    },
}

/// Result of one deterministic transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepResult {
    /// Next state.
    pub state: ExecutionState,
    /// Effects for imperative shell.
    pub effects: Vec<ExecutionEffect>,
}

/// Functional kernel entrypoint.
pub fn step(state: ExecutionState, input: ExecutionInput) -> Result<StepResult, KernelError> {
    let result = match input {
        ExecutionInput::Start { agent_count } => {
            if agent_count == 0 || agent_count > MAX_KERNEL_AGENTS {
                return Err(KernelError::InvalidStartCount(agent_count));
            }
            if state.phase != ExecutionPhase::Idle {
                return Err(KernelError::InvalidTransition {
                    phase: state.phase,
                    input,
                });
            }

            StepResult {
                state: ExecutionState {
                    phase: ExecutionPhase::Running,
                    remaining_agents: agent_count,
                    failed: false,
                },
                effects: vec![ExecutionEffect::BeginRun { agent_count }],
            }
        }
        ExecutionInput::AgentCompleted => {
            if state.phase != ExecutionPhase::Running || state.remaining_agents == 0 {
                return Err(KernelError::InvalidTransition {
                    phase: state.phase,
                    input,
                });
            }

            let remaining = state.remaining_agents - 1;
            if remaining == 0 {
                StepResult {
                    state: ExecutionState {
                        phase: ExecutionPhase::Succeeded,
                        remaining_agents: 0,
                        failed: false,
                    },
                    effects: vec![
                        ExecutionEffect::Progress {
                            remaining_agents: 0,
                        },
                        ExecutionEffect::Succeeded,
                    ],
                }
            } else {
                StepResult {
                    state: ExecutionState {
                        phase: ExecutionPhase::Running,
                        remaining_agents: remaining,
                        failed: false,
                    },
                    effects: vec![ExecutionEffect::Progress {
                        remaining_agents: remaining,
                    }],
                }
            }
        }
        ExecutionInput::AgentFailed => {
            if state.phase != ExecutionPhase::Running {
                return Err(KernelError::InvalidTransition {
                    phase: state.phase,
                    input,
                });
            }

            StepResult {
                state: ExecutionState {
                    phase: ExecutionPhase::Failed,
                    remaining_agents: 0,
                    failed: true,
                },
                effects: vec![ExecutionEffect::Failed],
            }
        }
        ExecutionInput::Reset => {
            if !matches!(state.phase, ExecutionPhase::Succeeded | ExecutionPhase::Failed) {
                return Err(KernelError::InvalidTransition {
                    phase: state.phase,
                    input,
                });
            }

            StepResult {
                state: ExecutionState::default(),
                effects: vec![ExecutionEffect::Reset],
            }
        }
    };

    debug_assert!(result.state.is_valid());
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_run_path_is_deterministic() {
        let state0 = ExecutionState::default();
        let step1 = step(state0, ExecutionInput::Start { agent_count: 2 }).unwrap();
        assert_eq!(step1.state.phase, ExecutionPhase::Running);
        assert_eq!(
            step1.effects,
            vec![ExecutionEffect::BeginRun { agent_count: 2 }]
        );

        let step2 = step(step1.state, ExecutionInput::AgentCompleted).unwrap();
        assert_eq!(step2.state.phase, ExecutionPhase::Running);
        assert_eq!(step2.state.remaining_agents, 1);

        let step3 = step(step2.state, ExecutionInput::AgentCompleted).unwrap();
        assert_eq!(step3.state.phase, ExecutionPhase::Succeeded);
        assert_eq!(step3.state.remaining_agents, 0);
        assert_eq!(
            step3.effects,
            vec![
                ExecutionEffect::Progress {
                    remaining_agents: 0,
                },
                ExecutionEffect::Succeeded,
            ]
        );
    }

    #[test]
    fn failure_path_sets_failed_latch() {
        let state0 = step(ExecutionState::default(), ExecutionInput::Start { agent_count: 3 })
            .unwrap()
            .state;
        let failed = step(state0, ExecutionInput::AgentFailed).unwrap();
        assert_eq!(failed.state.phase, ExecutionPhase::Failed);
        assert!(failed.state.failed);
        assert_eq!(failed.state.remaining_agents, 0);
        assert_eq!(failed.effects, vec![ExecutionEffect::Failed]);
    }

    #[test]
    fn reset_only_allowed_from_terminal_states() {
        let running = step(ExecutionState::default(), ExecutionInput::Start { agent_count: 1 })
            .unwrap()
            .state;
        assert!(matches!(
            step(running, ExecutionInput::Reset),
            Err(KernelError::InvalidTransition { .. })
        ));

        let succeeded = step(running, ExecutionInput::AgentCompleted).unwrap().state;
        let reset = step(succeeded, ExecutionInput::Reset).unwrap();
        assert_eq!(reset.state, ExecutionState::default());
        assert_eq!(reset.effects, vec![ExecutionEffect::Reset]);
    }

    #[test]
    fn start_rejects_invalid_counts() {
        assert_eq!(
            step(ExecutionState::default(), ExecutionInput::Start { agent_count: 0 }),
            Err(KernelError::InvalidStartCount(0))
        );
        assert_eq!(
            step(
                ExecutionState::default(),
                ExecutionInput::Start {
                    agent_count: MAX_KERNEL_AGENTS + 1,
                }
            ),
            Err(KernelError::InvalidStartCount(MAX_KERNEL_AGENTS + 1))
        );
    }

    #[test]
    fn invariants_hold_after_every_step() {
        let mut state = ExecutionState::default();
        let sequence = [
            ExecutionInput::Start { agent_count: 3 },
            ExecutionInput::AgentCompleted,
            ExecutionInput::AgentCompleted,
            ExecutionInput::AgentCompleted,
            ExecutionInput::Reset,
        ];

        for input in sequence {
            let next = step(state, input).unwrap();
            assert!(next.state.is_valid(), "state invalid after input {:?}", input);
            state = next.state;
        }
    }
}
