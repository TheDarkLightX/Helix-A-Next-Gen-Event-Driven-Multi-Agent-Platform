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


use helix_core::agent::{AgentConfig, AgentId, AgentKind}; // AgentKind is used in AgentConfig
use helix_agent_sdk_macros::{action_agent, source_agent}; // Correct path to macros
use helix_agent_sdk::{AgentContext, SdkError, SdkAgent}; // AgentStatus is not directly part of SdkAgent context from sdk crate
use helix_core::agent::AgentStatus; // AgentStatus is from helix_core
// EventPublisher is not directly used here, AgentContext provides event_publisher field.
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use serde_json::json;

// Common trait for test agents to provide new and status, macros might provide the rest
#[async_trait]
pub trait HelixAgentTestSupport {
    fn new_test(config: AgentConfig, status_arc: Arc<Mutex<AgentStatus>>) -> Self where Self: Sized;
    fn status_test(&self) -> Arc<Mutex<AgentStatus>>;
    // These test methods might need to use AgentContext if they are to mimic SdkAgent lifecycle
    async fn start_test(&mut self, context: &AgentContext) -> Result<(), SdkError>;
    async fn stop_test(&mut self, context: &AgentContext) -> Result<(), SdkError>;
}

// The #[agent(...)] attribute is not standard and not used by the current macros.
// AgentKind will be derived from struct name by the macro.
// The `agent_config` field must be `pub agent_config: std::sync::Arc<helix_core::agent::AgentConfig>;`
// for the macro-generated `Agent` trait implementation.
// Adding Default derive for macro compatibility if other fields are not set in macro factory.
#[source_agent]
#[derive(Clone, Debug, Default)]
pub struct DummySourceAgent {
    pub agent_config: Arc<helix_core::agent::AgentConfig>,
    pub status: Arc<Mutex<AgentStatus>>,
}

impl DummySourceAgent {
    // This new is for manual instantiation in tests if needed,
    // but the macro will generate a factory that effectively does:
    // Self { agent_config: Arc::new(config), ..Default::default() }
    pub fn new(config: helix_core::agent::AgentConfig) -> Self {
        Self {
            agent_config: Arc::new(config),
            status: Arc::new(Mutex::new(AgentStatus::Stopped)),
        }
    }

    // This is the inherent `run` method the macro expects.
    pub async fn run(&mut self, context: &AgentContext) -> Result<(), SdkError> {
        println!("DummySourceAgent [{:?}] producing event data via inherent run.", self.agent_config.id);
        let data = json!({"data": "from_source", "source_id": self.agent_config.id.to_string()});
        context.emit_event(data, Some("dummy.source.output".to_string())).await?;
        Ok(())
    }

    // Kept for direct test usage if needed, but not part of SdkAgent flow.
    pub async fn produce_event_data_legacy(&self) -> Result<Option<serde_json::Value>, SdkError> {
        println!("DummySourceAgent [{:?}] producing event data.", self.config.id);
        Ok(Some(json!({"data": "from_source", "source_id": self.config.id.to_string()})))
    }
}

#[async_trait]
impl HelixAgentTestSupport for DummySourceAgent {
    fn new_test(config: AgentConfig, status_arc: Arc<Mutex<AgentStatus>>) -> Self {
         Self { config, status: status_arc }
    }
    fn status_test(&self) -> Arc<Mutex<AgentStatus>> { self.status.clone() }
    // SdkAgent's start/stop methods take AgentContext.
    // The macro provides default implementations. If custom logic is needed for tests
    // that bypasses the macro's default, these would need to be distinct or the
    // SdkAgent trait methods would be implemented directly on the struct.
    // For now, assuming macro defaults are sufficient or tests will use SdkAgent trait calls.
    async fn start_test(&mut self, context: &AgentContext) -> Result<(), SdkError> {
        // This would call the SdkAgent::start if mimicking the runtime
        // self.start(context).await
        *self.status.lock().unwrap() = AgentStatus::Running;
        println!("DummySourceAgent (test support) started via start_test.");
        Ok(())
    }
    async fn stop_test(&mut self, context: &AgentContext) -> Result<(), SdkError> {
        // self.stop(context).await
        *self.status.lock().unwrap() = AgentStatus::Stopped;
        println!("DummySourceAgent (test support) stopped via stop_test.");
        Ok(())
    }
}

#[action_agent]
#[derive(Clone, Debug, Default)]
pub struct DummyActionAgent {
    pub agent_config: Arc<helix_core::agent::AgentConfig>,
    pub status: Arc<Mutex<AgentStatus>>,
    pub received_payloads: Arc<Mutex<Vec<serde_json::Value>>>,
}

impl DummyActionAgent {
     pub fn new(config: helix_core::agent::AgentConfig) -> Self {
        Self {
            agent_config: Arc::new(config),
            status: Arc::new(Mutex::new(AgentStatus::Stopped)),
            received_payloads: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn new_with_probe(config: helix_core::agent::AgentConfig, probe: Arc<Mutex<Vec<serde_json::Value>>>) -> Self {
        Self {
            agent_config: Arc::new(config),
            status: Arc::new(Mutex::new(AgentStatus::Stopped)),
            received_payloads: probe,
        }
    }

    // This is the inherent `execute` method the macro expects.
    pub async fn execute(&mut self, _context: &AgentContext, event: helix_core::event::Event) -> Result<(), SdkError> {
        println!("DummyActionAgent [{:?}] received payload via inherent execute: {:?}", self.agent_config.id, event.data);
        self.received_payloads.lock().unwrap().push(event.data.clone());
        Ok(())
    }
}

#[async_trait]
impl HelixAgentTestSupport for DummyActionAgent {
     fn new_test(config: AgentConfig, status_arc: Arc<Mutex<AgentStatus>>) -> Self {
         Self {
             agent_config: Arc::new(config), // Ensure it's Arc'd
             status: status_arc,
             received_payloads: Arc::new(Mutex::new(Vec::new()))
         }
    }
    fn status_test(&self) -> Arc<Mutex<AgentStatus>> { self.status.clone() }
    async fn start_test(&mut self, _context: &AgentContext) -> Result<(), SdkError> {
        *self.status.lock().unwrap() = AgentStatus::Running;
        println!("DummyActionAgent (test support) started via start_test.");
        Ok(())
    }
    async fn stop_test(&mut self, _context: &AgentContext) -> Result<(), SdkError> {
        *self.status.lock().unwrap() = AgentStatus::Stopped;
        println!("DummyActionAgent (test support) stopped via stop_test.");
        Ok(())
    }
}