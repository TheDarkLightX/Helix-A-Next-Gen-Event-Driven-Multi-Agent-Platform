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


use helix_agent_sdk::{ActionSdkAgent, AgentContext, SdkError}; // SdkAgent is brought in by ActionSdkAgent
use helix_agent_sdk_macros::action_agent;
use helix_core::{
    agent::{Agent, AgentConfig},
    event::Event as HelixEvent,
    types::{AgentId, EventId, RecipeId},
};
use serde_json::json;
use std::sync::Arc;

#[action_agent]
pub struct MyValidActionAgent {
    pub agent_config: Arc<AgentConfig>,
    // Add other fields specific to your agent
}

impl MyValidActionAgent {
    pub fn new(id_str: &str) -> Self {
        Self {
            agent_config: Arc::new(AgentConfig {
                id: AgentId::new(id_str),
                name: "My Valid Action Agent".to_string(),
                class_name: "MyValidActionAgent".to_string(),
                config: json!({}),
                recipe_id: RecipeId::new("test-recipe"),
                credentials: None,
            }),
        }
    }

    // The macro expects this inherent method
    pub async fn execute(
        &mut self,
        _context: &AgentContext,
        event: HelixEvent,
    ) -> Result<(), SdkError> {
        println!(
            "MyValidActionAgent executing for event: {}",
            event.id.inner()
        );
        Ok(())
    }
}

fn main() {
    let _agent = MyValidActionAgent::new("test-action");
    // Dummy event for testing compilation
    let _event = HelixEvent {
        id: EventId::new_v4(),
        source_agent_id: AgentId::new("dummy-source"),
        recipe_id: RecipeId::new("dummy-recipe"),
        event_type: "dummy.event".to_string(),
        data: json!({}),
        metadata: None,
        timestamp: chrono::Utc::now(),
    };
    // In a real test, you might construct an AgentContext and call agent.execute,
    // but for trybuild, successful compilation is the main goal.
    println!("MyValidActionAgent compiles!");
}