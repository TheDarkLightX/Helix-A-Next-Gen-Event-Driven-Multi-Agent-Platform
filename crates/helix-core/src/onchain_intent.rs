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

//! Pure on-chain transaction intent kernel.
//!
//! This module models transaction lifecycle state transitions without any
//! RPC, signing, or network side effects.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// On-chain transaction lifecycle phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OnchainPhase {
    /// No transaction intent in flight.
    Idle,
    /// Raw transaction submit is in progress.
    Submitting,
    /// Submit accepted and waiting for receipt confirmation.
    PendingReceipt,
    /// Transaction confirmed with successful execution.
    Confirmed,
    /// Transaction reached chain but reverted.
    Reverted,
    /// Submit or confirmation path failed.
    Failed,
}

/// Deterministic kernel state for one on-chain transaction intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnchainState {
    /// Current phase.
    pub phase: OnchainPhase,
    /// Last seen transaction hash (when available).
    pub tx_hash: Option<String>,
    /// Number of receipt polls attempted.
    pub poll_rounds: u16,
    /// Maximum permitted polls before timeout failure.
    pub max_poll_rounds: u16,
}

impl Default for OnchainState {
    fn default() -> Self {
        Self {
            phase: OnchainPhase::Idle,
            tx_hash: None,
            poll_rounds: 0,
            max_poll_rounds: 0,
        }
    }
}

impl OnchainState {
    /// Returns true when state satisfies structural invariants.
    pub fn is_valid(&self) -> bool {
        if self.phase == OnchainPhase::PendingReceipt && self.tx_hash.is_none() {
            return false;
        }
        self.poll_rounds <= self.max_poll_rounds
    }
}

/// Inputs accepted by the pure on-chain kernel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OnchainInput {
    /// Start a new transaction submission workflow.
    StartBroadcast {
        /// Signed raw transaction payload in hex.
        raw_tx_hex: String,
        /// Maximum receipt polling attempts.
        max_poll_rounds: u16,
    },
    /// RPC accepted broadcast and returned tx hash.
    SubmitAccepted {
        /// Canonical tx hash.
        tx_hash: String,
    },
    /// RPC rejected submit.
    SubmitRejected,
    /// Receipt observed and succeeded.
    ReceiptSuccess,
    /// Receipt observed but reverted.
    ReceiptReverted,
    /// Receipt not yet available in this poll round.
    ReceiptPending,
    /// Poll budget exhausted.
    PollTimeout,
    /// Reset terminal state back to idle.
    Reset,
}

/// Declarative effects emitted by kernel for imperative shell execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OnchainEffect {
    /// Submit raw transaction to chain RPC.
    SubmitRawTransaction {
        /// Signed raw transaction payload in hex.
        raw_tx_hex: String,
    },
    /// Poll receipt for tx hash.
    PollReceipt {
        /// Tx hash to query.
        tx_hash: String,
    },
}

/// Kernel transition error.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum OnchainKernelError {
    /// Start called with empty transaction hex.
    #[error("raw tx hex cannot be empty")]
    EmptyRawTx,
    /// Start called with empty poll budget.
    #[error("max poll rounds must be > 0")]
    InvalidPollBudget,
    /// Input is invalid for current phase.
    #[error("invalid transition: phase={phase:?}, input={input:?}")]
    InvalidTransition {
        /// Current phase.
        phase: OnchainPhase,
        /// Rejected input.
        input: OnchainInput,
    },
}

/// Result of one deterministic state transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnchainStepResult {
    /// Next state.
    pub state: OnchainState,
    /// Effects to execute.
    pub effects: Vec<OnchainEffect>,
}

/// Deterministic step function for on-chain intent lifecycle.
pub fn step(
    state: OnchainState,
    input: OnchainInput,
) -> Result<OnchainStepResult, OnchainKernelError> {
    let result = match input.clone() {
        OnchainInput::StartBroadcast {
            raw_tx_hex,
            max_poll_rounds,
        } => {
            if state.phase != OnchainPhase::Idle {
                return Err(OnchainKernelError::InvalidTransition {
                    phase: state.phase,
                    input,
                });
            }
            if raw_tx_hex.trim().is_empty() {
                return Err(OnchainKernelError::EmptyRawTx);
            }
            if max_poll_rounds == 0 {
                return Err(OnchainKernelError::InvalidPollBudget);
            }
            OnchainStepResult {
                state: OnchainState {
                    phase: OnchainPhase::Submitting,
                    tx_hash: None,
                    poll_rounds: 0,
                    max_poll_rounds,
                },
                effects: vec![OnchainEffect::SubmitRawTransaction { raw_tx_hex }],
            }
        }
        OnchainInput::SubmitAccepted { tx_hash } => {
            if state.phase != OnchainPhase::Submitting || tx_hash.trim().is_empty() {
                return Err(OnchainKernelError::InvalidTransition {
                    phase: state.phase,
                    input,
                });
            }
            OnchainStepResult {
                state: OnchainState {
                    phase: OnchainPhase::PendingReceipt,
                    tx_hash: Some(tx_hash.clone()),
                    poll_rounds: 0,
                    max_poll_rounds: state.max_poll_rounds,
                },
                effects: vec![OnchainEffect::PollReceipt { tx_hash }],
            }
        }
        OnchainInput::SubmitRejected => {
            if state.phase != OnchainPhase::Submitting {
                return Err(OnchainKernelError::InvalidTransition {
                    phase: state.phase,
                    input,
                });
            }
            OnchainStepResult {
                state: OnchainState {
                    phase: OnchainPhase::Failed,
                    ..state
                },
                effects: vec![],
            }
        }
        OnchainInput::ReceiptSuccess => {
            if state.phase != OnchainPhase::PendingReceipt {
                return Err(OnchainKernelError::InvalidTransition {
                    phase: state.phase,
                    input,
                });
            }
            OnchainStepResult {
                state: OnchainState {
                    phase: OnchainPhase::Confirmed,
                    ..state
                },
                effects: vec![],
            }
        }
        OnchainInput::ReceiptReverted => {
            if state.phase != OnchainPhase::PendingReceipt {
                return Err(OnchainKernelError::InvalidTransition {
                    phase: state.phase,
                    input,
                });
            }
            OnchainStepResult {
                state: OnchainState {
                    phase: OnchainPhase::Reverted,
                    ..state
                },
                effects: vec![],
            }
        }
        OnchainInput::ReceiptPending => {
            if state.phase != OnchainPhase::PendingReceipt {
                return Err(OnchainKernelError::InvalidTransition {
                    phase: state.phase,
                    input,
                });
            }
            let next_round = state.poll_rounds.saturating_add(1);
            if next_round >= state.max_poll_rounds {
                OnchainStepResult {
                    state: OnchainState {
                        phase: OnchainPhase::Failed,
                        poll_rounds: state.max_poll_rounds,
                        ..state
                    },
                    effects: vec![],
                }
            } else {
                let tx_hash =
                    state
                        .tx_hash
                        .clone()
                        .ok_or(OnchainKernelError::InvalidTransition {
                            phase: state.phase,
                            input: OnchainInput::ReceiptPending,
                        })?;
                OnchainStepResult {
                    state: OnchainState {
                        poll_rounds: next_round,
                        ..state
                    },
                    effects: vec![OnchainEffect::PollReceipt { tx_hash }],
                }
            }
        }
        OnchainInput::PollTimeout => {
            if state.phase != OnchainPhase::PendingReceipt {
                return Err(OnchainKernelError::InvalidTransition {
                    phase: state.phase,
                    input,
                });
            }
            OnchainStepResult {
                state: OnchainState {
                    phase: OnchainPhase::Failed,
                    poll_rounds: state.max_poll_rounds,
                    ..state
                },
                effects: vec![],
            }
        }
        OnchainInput::Reset => {
            if !matches!(
                state.phase,
                OnchainPhase::Confirmed | OnchainPhase::Reverted | OnchainPhase::Failed
            ) {
                return Err(OnchainKernelError::InvalidTransition {
                    phase: state.phase,
                    input,
                });
            }
            OnchainStepResult {
                state: OnchainState::default(),
                effects: vec![],
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
    fn successful_broadcast_and_confirmation_path() {
        let state0 = OnchainState::default();
        let step1 = step(
            state0,
            OnchainInput::StartBroadcast {
                raw_tx_hex: "0xdeadbeef".to_string(),
                max_poll_rounds: 3,
            },
        )
        .unwrap();
        assert_eq!(step1.state.phase, OnchainPhase::Submitting);
        assert!(matches!(
            step1.effects[0],
            OnchainEffect::SubmitRawTransaction { .. }
        ));

        let step2 = step(
            step1.state,
            OnchainInput::SubmitAccepted {
                tx_hash: "0xabc".to_string(),
            },
        )
        .unwrap();
        assert_eq!(step2.state.phase, OnchainPhase::PendingReceipt);
        assert_eq!(step2.state.tx_hash.as_deref(), Some("0xabc"));

        let step3 = step(step2.state, OnchainInput::ReceiptSuccess).unwrap();
        assert_eq!(step3.state.phase, OnchainPhase::Confirmed);
    }

    #[test]
    fn pending_receipt_eventually_fails_after_budget() {
        let start = step(
            OnchainState::default(),
            OnchainInput::StartBroadcast {
                raw_tx_hex: "0x1".to_string(),
                max_poll_rounds: 2,
            },
        )
        .unwrap()
        .state;
        let pending = step(
            start,
            OnchainInput::SubmitAccepted {
                tx_hash: "0x2".to_string(),
            },
        )
        .unwrap()
        .state;

        let next = step(pending, OnchainInput::ReceiptPending).unwrap();
        assert_eq!(next.state.phase, OnchainPhase::PendingReceipt);
        assert_eq!(next.state.poll_rounds, 1);

        let timeout = step(next.state, OnchainInput::ReceiptPending).unwrap();
        assert_eq!(timeout.state.phase, OnchainPhase::Failed);
    }

    #[test]
    fn invalid_inputs_are_rejected() {
        let err = step(
            OnchainState::default(),
            OnchainInput::StartBroadcast {
                raw_tx_hex: "".to_string(),
                max_poll_rounds: 1,
            },
        )
        .unwrap_err();
        assert_eq!(err, OnchainKernelError::EmptyRawTx);

        let err2 = step(
            OnchainState::default(),
            OnchainInput::StartBroadcast {
                raw_tx_hex: "0xdead".to_string(),
                max_poll_rounds: 0,
            },
        )
        .unwrap_err();
        assert_eq!(err2, OnchainKernelError::InvalidPollBudget);
    }
}
