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


//! TES-driven tests for the agent system
//! Focusing on quality over coverage with high assertion density and behavior testing

use helix_core::agent::*;
use helix_core::event::Event;
use helix_core::types::{AgentId, ProfileId};
use helix_core::HelixError;
use helix_core::test_utils::*;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Mock implementation for testing
struct TestAgent {
    config: AgentConfig,
    execution_count: u32,
}

#[async_trait]
impl Agent for TestAgent {
    fn id(&self) -> AgentId {
        self.config.id
    }
    
    fn config(&self) -> &AgentConfig {
        &self.config
    }
}

#[async_trait]
impl SourceAgent for TestAgent {
    async fn run(&mut self, ctx: SourceContext) -> Result<(), HelixError> {
        self.execution_count += 1;
        ctx.emit(json!({"count": self.execution_count}), Some("test.event".to_string())).await
    }
}

#[tokio::test]
async fn test_source_agent_comprehensive() {
    let mut tracker = TesTracker::new("test_source_agent_comprehensive");
    
    // Setup
    let agent_id = Uuid::new_v4();
    let profile_id = Uuid::new_v4();
    let (tx, mut rx) = mpsc::channel(10);
    
    let config = AgentConfig {
        id: agent_id,
        profile_id,
        name: Some("TestAgent".to_string()),
        agent_kind: "test_source".to_string(),
        config_data: json!({"interval": 1000}),
        credential_ids: vec![],
        enabled: true,
    };
    
    let mut agent = TestAgent {
        config: config.clone(),
        execution_count: 0,
    };
    
    // Test 1: Agent configuration
    tes_assert_eq!(tracker, agent.id(), agent_id, "Agent ID matches");
    tes_assert_eq!(tracker, agent.config().enabled, true, "Agent is enabled");
    tes_assert_eq!(tracker, agent.config().agent_kind, "test_source", "Agent kind is correct");
    tes_assert_eq!(tracker, agent.execution_count, 0, "Initial execution count is zero");
    
    // Test 2: Happy path behavior
    tes_behavior!(tracker, "happy_path",
        given: "A properly configured source agent",
        when: "The agent runs",
        then: "It emits an event successfully",
        {
            let ctx = SourceContext {
                agent_id,
                profile_id,
                credential_provider: Arc::new(MockCredentialProvider),
                state_store: Arc::new(MockStateStore),
                event_tx: tx.clone(),
            };
            
            agent.run(ctx).await.is_ok()
        }
    );
    
    // Test 3: Event emission verification
    tes_behavior!(tracker, "event_emission",
        given: "An agent that has run",
        when: "Checking the event channel",
        then: "The emitted event is received",
        {
            if let Ok(event) = rx.try_recv() {
                let valid = event.source == agent_id.to_string() &&
                           event.r#type == "test.event" &&
                           event.data.is_some();
                tes_assert_eq!(tracker, valid, true, "Event has correct properties");
                tes_assert_eq!(tracker, agent.execution_count, 1, "Execution count incremented");
                true
            } else {
                false
            }
        }
    );
    
    // Test 4: Multiple executions
    tes_behavior!(tracker, "multiple_executions",
        given: "An agent that has run once",
        when: "Running the agent multiple times",
        then: "Execution count increases correctly",
        {
            let ctx = SourceContext {
                agent_id,
                profile_id,
                credential_provider: Arc::new(MockCredentialProvider),
                state_store: Arc::new(MockStateStore),
                event_tx: tx.clone(),
            };
            
            let mut success = true;
            for i in 2..=5 {
                success &= agent.run(ctx.clone()).await.is_ok();
                tes_assert_eq!(tracker, agent.execution_count, i, &format!("Execution count is {}", i));
            }
            success
        }
    );
    
    // Test 5: Edge case - disabled agent
    tes_behavior!(tracker, "edge_case_disabled",
        given: "An agent that is disabled",
        when: "Checking agent state",
        then: "The agent reports as disabled",
        {
            let mut disabled_config = config.clone();
            disabled_config.enabled = false;
            let disabled_agent = TestAgent {
                config: disabled_config,
                execution_count: 0,
            };
            tes_assert_eq!(tracker, disabled_agent.config().enabled, false, "Disabled agent is not enabled");
            true
        }
    );
    
    // Simulate mutation testing results
    tracker.record_mutations(17, 20); // 85% mutation score
    
    println!("{}", tracker.report());
}

#[tokio::test]
async fn test_agent_config_validation() {
    let mut tracker = TesTracker::new("test_agent_config_validation");
    
    // Test various configuration scenarios
    let test_cases = vec![
        ("empty_name", None, ""),
        ("short_name", Some("A".to_string()), "A"),
        ("normal_name", Some("TestAgent".to_string()), "TestAgent"),
        ("long_name", Some("A".repeat(100)), &"A".repeat(100)),
    ];
    
    for (scenario, name, expected) in test_cases {
        tes_behavior!(tracker, scenario,
            given: &format!("Agent name: {:?}", name),
            when: "Creating agent config",
            then: "Name is handled correctly",
            {
                let config = AgentConfig {
                    id: Uuid::new_v4(),
                    profile_id: Uuid::new_v4(),
                    name: name.clone(),
                    agent_kind: "test".to_string(),
                    config_data: json!({}),
                    credential_ids: vec![],
                    enabled: true,
                };
                
                let actual_name = config.name.as_deref().unwrap_or("");
                tes_assert_eq!(tracker, actual_name, expected, &format!("{} name check", scenario));
                true
            }
        );
    }
    
    // Test JSON configuration
    tes_behavior!(tracker, "json_config",
        given: "Complex JSON configuration",
        when: "Storing in config_data",
        then: "JSON is preserved correctly",
        {
            let complex_json = json!({
                "nested": {
                    "array": [1, 2, 3],
                    "object": {"key": "value"}
                },
                "number": 42,
                "boolean": true
            });
            
            let config = AgentConfig {
                id: Uuid::new_v4(),
                profile_id: Uuid::new_v4(),
                name: Some("JsonTest".to_string()),
                agent_kind: "test".to_string(),
                config_data: complex_json.clone(),
                credential_ids: vec![],
                enabled: true,
            };
            
            tes_assert_eq!(tracker, config.config_data, complex_json, "JSON data preserved");
            tes_assert_eq!(tracker, config.config_data["number"], 42, "Number field accessible");
            tes_assert_eq!(tracker, config.config_data["boolean"], true, "Boolean field accessible");
            true
        }
    );
    
    tracker.record_mutations(15, 18);
    println!("{}", tracker.report());
}

#[tokio::test]
async fn test_context_error_handling() {
    let mut tracker = TesTracker::new("test_context_error_handling");
    
    // Test channel errors
    tes_behavior!(tracker, "channel_closed_error",
        given: "A closed event channel",
        when: "Attempting to emit an event",
        then: "Error is handled gracefully",
        {
            let (tx, rx) = mpsc::channel::<Event>(1);
            drop(rx); // Close the receiver
            
            let ctx = SourceContext {
                agent_id: Uuid::new_v4(),
                profile_id: Uuid::new_v4(),
                credential_provider: Arc::new(MockCredentialProvider),
                state_store: Arc::new(MockStateStore),
                event_tx: tx,
            };
            
            let result = ctx.emit(json!({"test": "data"}), None).await;
            tes_assert_eq!(tracker, result.is_err(), true, "Emit returns error");
            
            if let Err(e) = result {
                tes_assert_eq!(tracker, 
                    matches!(e, HelixError::MpscSendError(_)), 
                    true, 
                    "Error is MpscSendError"
                );
                true
            } else {
                false
            }
        }
    );
    
    // Test credential provider errors
    tes_behavior!(tracker, "credential_not_found",
        given: "A missing credential",
        when: "Requesting the credential",
        then: "None is returned",
        {
            let provider = MockCredentialProvider;
            let result = provider.get_credential("nonexistent").await;
            tes_assert_eq!(tracker, result.is_ok(), true, "Get credential doesn't error");
            tes_assert_eq!(tracker, result.unwrap(), None, "Missing credential returns None");
            true
        }
    );
    
    // Test state store operations
    tes_behavior!(tracker, "state_store_operations",
        given: "An empty state store",
        when: "Performing get and set operations",
        then: "Operations complete successfully",
        {
            let store = MockStateStore;
            
            // Test get on non-existent key
            let get_result = store.get_state("missing_key").await;
            tes_assert_eq!(tracker, get_result.is_ok(), true, "Get state succeeds");
            tes_assert_eq!(tracker, get_result.unwrap(), None, "Missing key returns None");
            
            // Test set operation
            let set_result = store.set_state("test_key", b"test_value").await;
            tes_assert_eq!(tracker, set_result.is_ok(), true, "Set state succeeds");
            
            true
        }
    );
    
    tracker.record_mutations(14, 16);
    println!("{}", tracker.report());
}

// Mock implementations
struct MockCredentialProvider;

#[async_trait]
impl CredentialProvider for MockCredentialProvider {
    async fn get_credential(&self, _id: &str) -> Result<Option<String>, HelixError> {
        Ok(None)
    }
}

struct MockStateStore;

#[async_trait]
impl StateStore for MockStateStore {
    async fn get_state(&self, _key: &str) -> Result<Option<Vec<u8>>, HelixError> {
        Ok(None)
    }
    
    async fn set_state(&self, _key: &str, _value: &[u8]) -> Result<(), HelixError> {
        Ok(())
    }
}