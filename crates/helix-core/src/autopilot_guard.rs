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

//! Deterministic autopilot guard for LLM-operated Helix actions.

use serde::{Deserialize, Serialize};

/// Autopilot operation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutopilotMode {
    /// Deny all autonomous actions.
    Off,
    /// Require confirmation for every action.
    Assist,
    /// Allow autonomous execution within guardrails.
    Auto,
}

/// Configuration for autopilot guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutopilotGuardConfig {
    /// Autopilot mode.
    pub mode: AutopilotMode,
    /// Permit on-chain side effects.
    pub allow_onchain: bool,
    /// Require explicit human confirmation for on-chain actions (even in `auto` mode).
    pub require_onchain_confirmation: bool,
    /// Require dry-run mode for on-chain actions.
    pub require_onchain_dry_run: bool,
    /// Upper bound for policy commands in one request.
    pub max_policy_commands: u16,
}

impl Default for AutopilotGuardConfig {
    fn default() -> Self {
        Self {
            mode: AutopilotMode::Assist,
            allow_onchain: false,
            require_onchain_confirmation: true,
            require_onchain_dry_run: true,
            max_policy_commands: 128,
        }
    }
}

/// Evaluated action class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AutopilotActionClass {
    /// Policy simulation action.
    PolicySimulation {
        /// Number of commands in the simulation request.
        command_count: u16,
    },
    /// On-chain broadcast action.
    OnchainBroadcast {
        /// Whether request is dry run.
        dry_run: bool,
    },
}

/// Guard input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutopilotGuardInput {
    /// Replace entire guard config.
    SetConfig {
        /// New config.
        config: AutopilotGuardConfig,
    },
    /// Evaluate one action.
    Evaluate {
        /// Action class.
        action: AutopilotActionClass,
        /// Whether a human explicitly confirmed this action.
        confirmed_by_human: bool,
    },
}

/// Guard decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AutopilotGuardDecision {
    /// Config update accepted.
    ConfigUpdated,
    /// Action allowed.
    Allow {
        /// True when this mode requires human confirmation.
        requires_confirmation: bool,
    },
    /// Action denied with reason.
    Deny {
        /// Stable denial reason code.
        reason: String,
    },
}

/// Monotonic guard stats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutopilotStats {
    /// Number of action evaluations.
    pub evaluations: u64,
    /// Number of denied evaluations.
    pub denied: u64,
}

/// Deterministic guard machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutopilotGuardMachine {
    config: AutopilotGuardConfig,
    stats: AutopilotStats,
}

impl Default for AutopilotGuardMachine {
    fn default() -> Self {
        Self::new(AutopilotGuardConfig::default())
    }
}

impl AutopilotGuardMachine {
    /// Creates guard machine with config.
    pub fn new(config: AutopilotGuardConfig) -> Self {
        Self {
            config,
            stats: AutopilotStats {
                evaluations: 0,
                denied: 0,
            },
        }
    }

    /// Restores guard machine from a persisted deterministic snapshot.
    pub fn from_snapshot(config: AutopilotGuardConfig, stats: AutopilotStats) -> Self {
        Self { config, stats }
    }

    /// Returns current config.
    pub fn config(self) -> AutopilotGuardConfig {
        self.config
    }

    /// Returns current stats.
    pub fn stats(self) -> AutopilotStats {
        self.stats
    }

    /// Applies one guard input.
    pub fn step(&mut self, input: AutopilotGuardInput) -> AutopilotGuardDecision {
        match input {
            AutopilotGuardInput::SetConfig { config } => {
                self.config = config;
                AutopilotGuardDecision::ConfigUpdated
            }
            AutopilotGuardInput::Evaluate {
                action,
                confirmed_by_human,
            } => {
                self.stats.evaluations = self.stats.evaluations.saturating_add(1);

                if self.config.mode == AutopilotMode::Off {
                    self.stats.denied = self.stats.denied.saturating_add(1);
                    return AutopilotGuardDecision::Deny {
                        reason: "mode_off".to_string(),
                    };
                }

                if self.config.mode == AutopilotMode::Assist && !confirmed_by_human {
                    self.stats.denied = self.stats.denied.saturating_add(1);
                    return AutopilotGuardDecision::Deny {
                        reason: "assist_requires_confirmation".to_string(),
                    };
                }

                match action {
                    AutopilotActionClass::PolicySimulation { command_count } => {
                        if command_count == 0 || command_count > self.config.max_policy_commands {
                            self.stats.denied = self.stats.denied.saturating_add(1);
                            return AutopilotGuardDecision::Deny {
                                reason: "policy_command_limit".to_string(),
                            };
                        }
                    }
                    AutopilotActionClass::OnchainBroadcast { dry_run } => {
                        if !self.config.allow_onchain {
                            self.stats.denied = self.stats.denied.saturating_add(1);
                            return AutopilotGuardDecision::Deny {
                                reason: "onchain_disabled".to_string(),
                            };
                        }
                        if self.config.require_onchain_confirmation && !confirmed_by_human {
                            self.stats.denied = self.stats.denied.saturating_add(1);
                            return AutopilotGuardDecision::Deny {
                                reason: "onchain_requires_confirmation".to_string(),
                            };
                        }
                        if self.config.require_onchain_dry_run && !dry_run {
                            self.stats.denied = self.stats.denied.saturating_add(1);
                            return AutopilotGuardDecision::Deny {
                                reason: "dry_run_required".to_string(),
                            };
                        }
                    }
                }

                AutopilotGuardDecision::Allow {
                    requires_confirmation: self.config.mode == AutopilotMode::Assist,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assist_mode_requires_confirmation() {
        let mut machine = AutopilotGuardMachine::default();
        let denied = machine.step(AutopilotGuardInput::Evaluate {
            action: AutopilotActionClass::PolicySimulation { command_count: 3 },
            confirmed_by_human: false,
        });
        assert!(matches!(
            denied,
            AutopilotGuardDecision::Deny { reason } if reason == "assist_requires_confirmation"
        ));

        let allowed = machine.step(AutopilotGuardInput::Evaluate {
            action: AutopilotActionClass::PolicySimulation { command_count: 3 },
            confirmed_by_human: true,
        });
        assert!(matches!(
            allowed,
            AutopilotGuardDecision::Allow {
                requires_confirmation: true
            }
        ));
    }

    #[test]
    fn snapshot_restore_preserves_config_and_stats() {
        let config = AutopilotGuardConfig {
            mode: AutopilotMode::Auto,
            allow_onchain: true,
            require_onchain_confirmation: false,
            require_onchain_dry_run: true,
            max_policy_commands: 8,
        };
        let stats = AutopilotStats {
            evaluations: 5,
            denied: 2,
        };
        let machine = AutopilotGuardMachine::from_snapshot(config, stats);
        assert_eq!(machine.config(), config);
        assert_eq!(machine.stats(), stats);
    }

    #[test]
    fn onchain_rules_are_fail_closed() {
        let mut machine = AutopilotGuardMachine::new(AutopilotGuardConfig {
            mode: AutopilotMode::Auto,
            allow_onchain: true,
            require_onchain_confirmation: false,
            require_onchain_dry_run: true,
            max_policy_commands: 10,
        });
        let denied = machine.step(AutopilotGuardInput::Evaluate {
            action: AutopilotActionClass::OnchainBroadcast { dry_run: false },
            confirmed_by_human: true,
        });
        assert!(matches!(
            denied,
            AutopilotGuardDecision::Deny { reason } if reason == "dry_run_required"
        ));

        let allowed = machine.step(AutopilotGuardInput::Evaluate {
            action: AutopilotActionClass::OnchainBroadcast { dry_run: true },
            confirmed_by_human: false,
        });
        assert!(matches!(
            allowed,
            AutopilotGuardDecision::Allow {
                requires_confirmation: false
            }
        ));
    }

    #[test]
    fn onchain_requires_confirmation_when_configured() {
        let mut machine = AutopilotGuardMachine::new(AutopilotGuardConfig {
            mode: AutopilotMode::Auto,
            allow_onchain: true,
            require_onchain_confirmation: true,
            require_onchain_dry_run: false,
            max_policy_commands: 10,
        });
        let denied = machine.step(AutopilotGuardInput::Evaluate {
            action: AutopilotActionClass::OnchainBroadcast { dry_run: true },
            confirmed_by_human: false,
        });
        assert!(matches!(
            denied,
            AutopilotGuardDecision::Deny { reason } if reason == "onchain_requires_confirmation"
        ));
    }

    #[test]
    fn policy_command_count_boundary_checks() {
        let mut machine = AutopilotGuardMachine::new(AutopilotGuardConfig {
            mode: AutopilotMode::Auto,
            allow_onchain: false,
            require_onchain_confirmation: true,
            require_onchain_dry_run: true,
            max_policy_commands: 3,
        });

        let zero = machine.step(AutopilotGuardInput::Evaluate {
            action: AutopilotActionClass::PolicySimulation { command_count: 0 },
            confirmed_by_human: false,
        });
        assert!(matches!(
            zero,
            AutopilotGuardDecision::Deny { reason } if reason == "policy_command_limit"
        ));

        let at_limit = machine.step(AutopilotGuardInput::Evaluate {
            action: AutopilotActionClass::PolicySimulation { command_count: 3 },
            confirmed_by_human: false,
        });
        assert!(matches!(
            at_limit,
            AutopilotGuardDecision::Allow {
                requires_confirmation: false
            }
        ));

        let over_limit = machine.step(AutopilotGuardInput::Evaluate {
            action: AutopilotActionClass::PolicySimulation { command_count: 4 },
            confirmed_by_human: false,
        });
        assert!(matches!(
            over_limit,
            AutopilotGuardDecision::Deny { reason } if reason == "policy_command_limit"
        ));
    }
}
