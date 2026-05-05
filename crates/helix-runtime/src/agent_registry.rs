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

//! Runtime registry mapping `agent_kind` strings to SDK agent factories.

use helix_agent_sdk::{AgentFactory, SdkAgent, SdkError, AGENT_FACTORIES};
use helix_core::agent::AgentConfig;
use std::collections::HashMap;
use std::sync::Arc;

/// Factory registry for SDK agents.
#[derive(Default)]
pub struct AgentRegistry {
    factories: HashMap<String, AgentFactory>,
}

impl AgentRegistry {
    /// Builds a registry by consuming statically registered factories emitted by `helix-agent-sdk-macros`.
    ///
    /// Duplicate registrations for the same `agent_kind` are treated as configuration errors.
    pub fn new() -> Result<Self, SdkError> {
        let mut reg = Self::default();

        for reg_fn in AGENT_FACTORIES {
            let (kind, factory) = reg_fn();
            if reg.factories.contains_key(kind) {
                return Err(SdkError::ConfigurationError(format!(
                    "duplicate agent factory registration for kind '{}'",
                    kind
                )));
            }
            reg.factories.insert(kind.to_string(), factory);
        }

        Ok(reg)
    }

    /// Registers one factory, overriding is denied.
    pub fn register(
        &mut self,
        kind: impl Into<String>,
        factory: AgentFactory,
    ) -> Result<(), SdkError> {
        let kind = kind.into();
        if self.factories.contains_key(&kind) {
            return Err(SdkError::ConfigurationError(format!(
                "agent kind '{}' already registered",
                kind
            )));
        }
        self.factories.insert(kind, factory);
        Ok(())
    }

    /// Creates an SDK agent instance from an `AgentConfig`.
    pub fn create_agent(
        &self,
        agent_config: Arc<AgentConfig>,
    ) -> Result<Box<dyn SdkAgent>, SdkError> {
        let kind = agent_config.agent_kind.as_str();
        let factory = self.factories.get(kind).ok_or_else(|| {
            SdkError::ConfigurationError(format!("no factory registered for agent_kind '{}'", kind))
        })?;

        factory((*agent_config).clone())
    }

    /// Returns the number of registered agent kinds.
    pub fn len(&self) -> usize {
        self.factories.len()
    }

    /// Returns true if a factory is registered for the supplied agent kind.
    pub fn contains_kind(&self, kind: &str) -> bool {
        self.factories.contains_key(kind)
    }

    /// Returns true if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.factories.is_empty()
    }
}
