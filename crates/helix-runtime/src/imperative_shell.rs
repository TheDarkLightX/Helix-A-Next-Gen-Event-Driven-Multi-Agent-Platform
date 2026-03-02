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

//! Imperative shell for the pure recipe execution kernel.

use async_trait::async_trait;
use helix_core::execution_kernel::{
    step, ExecutionEffect, ExecutionInput, ExecutionState, KernelError,
};
use helix_core::HelixError;

/// Side-effect boundary for integrating `helix_core::execution_kernel` into runtime code.
#[async_trait]
pub trait ExecutionPort: Send + Sync {
    /// Applies one effect emitted by the pure kernel.
    async fn apply_effect(&self, effect: ExecutionEffect) -> Result<(), HelixError>;
}

/// Small orchestration shell that owns mutable kernel state and executes effects.
pub struct ExecutionShell<P: ExecutionPort> {
    state: ExecutionState,
    port: P,
}

impl<P: ExecutionPort> ExecutionShell<P> {
    /// Creates a shell with default `Idle` state.
    pub fn new(port: P) -> Self {
        Self {
            state: ExecutionState::default(),
            port,
        }
    }

    /// Returns the current kernel state.
    pub fn state(&self) -> ExecutionState {
        self.state
    }

    /// Applies one input through the pure kernel and then executes emitted effects.
    pub async fn apply(&mut self, input: ExecutionInput) -> Result<ExecutionState, HelixError> {
        let transition = step(self.state, input).map_err(map_kernel_error)?;
        self.state = transition.state;

        for effect in transition.effects {
            self.port.apply_effect(effect).await?;
        }

        Ok(self.state)
    }
}

fn map_kernel_error(err: KernelError) -> HelixError {
    HelixError::validation_error("execution_kernel".to_string(), err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct RecordingPort {
        effects: Arc<Mutex<Vec<ExecutionEffect>>>,
        fail_on: Option<ExecutionEffect>,
    }

    #[async_trait]
    impl ExecutionPort for RecordingPort {
        async fn apply_effect(&self, effect: ExecutionEffect) -> Result<(), HelixError> {
            if self.fail_on == Some(effect) {
                return Err(HelixError::InternalError("effect failure".to_string()));
            }
            self.effects.lock().unwrap().push(effect);
            Ok(())
        }
    }

    #[tokio::test]
    async fn applies_effects_in_order() {
        let effects = Arc::new(Mutex::new(Vec::new()));
        let port = RecordingPort {
            effects: effects.clone(),
            fail_on: None,
        };
        let mut shell = ExecutionShell::new(port);

        shell
            .apply(ExecutionInput::Start { agent_count: 1 })
            .await
            .unwrap();
        shell.apply(ExecutionInput::AgentCompleted).await.unwrap();

        let recorded = effects.lock().unwrap().clone();
        assert_eq!(
            recorded,
            vec![
                ExecutionEffect::BeginRun { agent_count: 1 },
                ExecutionEffect::Progress {
                    remaining_agents: 0
                },
                ExecutionEffect::Succeeded,
            ]
        );
    }

    #[tokio::test]
    async fn propagates_port_failures() {
        let effects = Arc::new(Mutex::new(Vec::new()));
        let port = RecordingPort {
            effects,
            fail_on: Some(ExecutionEffect::Failed),
        };
        let mut shell = ExecutionShell::new(port);

        shell
            .apply(ExecutionInput::Start { agent_count: 2 })
            .await
            .unwrap();

        let err = shell.apply(ExecutionInput::AgentFailed).await.unwrap_err();
        assert!(matches!(err, HelixError::InternalError(_)));
    }
}
