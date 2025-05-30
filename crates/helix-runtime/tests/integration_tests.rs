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


#[cfg(test)]
mod tests {
    use helix_core::{
        agent::{AgentConfig, AgentId, AgentKind, ProfileId},
        event::Event, // Keep Event for InMemoryEventCollector's internal use if it constructs full Events
        recipe::{Recipe, RecipeGraphDefinition, RecipeId, RecipeNode},
    };
    use helix_runtime::{
        agent_registry::AgentRegistry, agent_runner::AgentRunner,
        messaging::InMemoryEventCollector,
    };
    use helix_storage::postgres_state_store::PostgresStateStore;
    use sqlx::PgPool;
    use std::collections::HashMap;
    use testcontainers::clients;
    use testcontainers_modules::postgres::Postgres;
    use uuid::Uuid;
    use serde_json::json;
    use helix_agent_sdk::{ SdkAgent, AgentContext, SdkError, EventPublisher as SdkEventPublisher}; // AgentStatus is not in helix_agent_sdk
    use helix_core::agent::AgentStatus; // AgentStatus is from helix_core
    use std::sync::{Arc, Mutex};
    
    // Declare the common module
    mod common;
    // Use the agents from the common module
    use common::test_agents::{DummySourceAgent, DummyActionAgent};

    async fn setup_db(pool: &PgPool) {
        // SQL for creating agent_configurations table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS agent_configurations (
                id UUID PRIMARY KEY,
                profile_id UUID NOT NULL,
                kind VARCHAR(255) NOT NULL,
                name VARCHAR(255),
                config JSONB,
                created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )
        .execute(pool)
        .await
        .expect("Failed to create agent_configurations table");

        // SQL for creating recipes table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS recipes (
                id UUID PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                profile_id UUID NOT NULL,
                graph_definition JSONB NOT NULL,
                created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )
        .execute(pool)
        .await
        .expect("Failed to create recipes table");
    }

    #[tokio::test]
    async fn test_simple_recipe_integration() {
        let docker = clients::Cli::default();
        let postgres_image = Postgres::default().with_version(13);
        let node = docker.run(postgres_image);
        let connection_string = &format!(
            "postgres://postgres:postgres@127.0.0.1:{}/postgres",
            node.get_host_port_ipv4(5432)
        );

        let pool = PgPool::connect(connection_string)
            .await
            .expect("Failed to connect to test Postgres");

        setup_db(&pool).await;

        let state_store = PostgresStateStore::new(pool.clone());

        let profile_id = ProfileId::new_v4();
        let source_agent_id = AgentId::new_v4();
        let action_agent_id = AgentId::new_v4();

        // 1. Create and Store Agent Configurations
        let source_agent_config = AgentConfig {
            id: source_agent_id.clone(),
            profile_id: profile_id.clone(),
            // Kind should match the struct name for automatic registration by macros
            kind: AgentKind::new("DummySourceAgent"),
            name: Some("Test Source Agent".to_string()),
            config: json!({ "source_param": "value1" }),
            ..Default::default()
        };

        let action_agent_config = AgentConfig {
            id: action_agent_id.clone(),
            profile_id: profile_id.clone(),
            // Kind should match the struct name for automatic registration by macros
            kind: AgentKind::new("DummyActionAgent"),
            name: Some("Test Action Agent".to_string()),
            config: json!({ "action_param": "value2" }),
            ..Default::default()
        };

        state_store
            .store_agent_config(source_agent_config.clone())
            .await
            .expect("Failed to store source agent config");
        state_store
            .store_agent_config(action_agent_config.clone())
            .await
            .expect("Failed to store action agent config");

        // 2. Create and Store a Recipe
        let recipe_id = RecipeId::new_v4();
        let recipe = Recipe {
            id: recipe_id.clone(),
            name: "Test Recipe".to_string(),
            profile_id: profile_id.clone(),
            graph_definition: RecipeGraphDefinition {
                agents: vec![
                    RecipeNode { agent_id: source_agent_id.clone(), node_type: "source".to_string(), depends_on: vec![] },
                    RecipeNode { agent_id: action_agent_id.clone(), node_type: "action".to_string(), depends_on: vec![source_agent_id.to_string()] },
                ],
                connections: vec![HashMap::from([(
                    source_agent_id.clone(),
                    vec![action_agent_id.clone()],
                )])],
            },
            ..Default::default()
        };

        state_store
            .store_recipe(recipe.clone())
            .await
            .expect("Failed to store recipe");

        // 3. Initialize and Run AgentRunner with probe-instrumented agents
        let event_collector = Arc::new(InMemoryEventCollector::new());
        
        let action_agent_received_payloads_probe = Arc::new(Mutex::new(Vec::new()));
        let action_agent_received_payloads_clone = action_agent_received_payloads_probe.clone();

        // AgentRegistry::new() now automatically populates from discovered agents
        let agent_registry_for_test = AgentRegistry::new();
        
        // We need to ensure DummyActionAgent can still be created with a probe for this test.
        // The automatic registration will register a factory that calls `DummyActionAgent::default()` essentially.
        // For this specific test that needs a probe, we might need to temporarily manually register
        // a factory that uses `new_with_probe`, or adapt the test.
        // For now, let's assume the test needs to be adapted or the DummyActionAgent's default
        // behavior is sufficient for most registry tests, and this specific probe test might need
        // a different setup if we strictly rely on auto-registration for this flow.
        //
        // Let's try to create a version of DummyActionAgent that can accept the probe via its config
        // or a setter, so the default factory can still be used.
        // OR, the test can retrieve the created agent and set the probe.
        // For now, this test's manual registration of DummyActionAgent will be an issue if we want to test
        // the full auto-registration flow for it.
        //
        // Let's adjust the test to use the auto-registered DummyActionAgent and then
        // find a way to assert its behavior, perhaps by checking its internal state if made accessible,
        // or by having it emit a specific event that the event_collector can catch.
        //
        // For the purpose of this diff, I will remove the manual registrations.
        // The `DummyActionAgent::new_with_probe` is problematic for the generic factory.
        // The test will need to be refactored if we want to keep the probe logic
        // while using the auto-registered factory.
        //
        // A simpler approach for now: the `DummyActionAgent` in `test_agents.rs`
        // already has `received_payloads: Arc<Mutex<Vec<serde_json::Value>>>`.
        // The default factory will create it with an empty Vec.
        // We can retrieve this Arc from the instantiated agent if the AgentRunner allows access to agents.
        // AgentRunner's `instantiate_agents_from_configs` stores agents in `self.agents`.
        // We'd need a way to get a handle to the specific `DummyActionAgent` instance.

        // For this test, we will rely on the fact that DummyActionAgent's `received_payloads`
        // is public and can be cloned if we get a reference to the agent.
        // The AgentRunner would need to provide a way to get a specific agent instance.
        // This is getting complex for this specific test.
        //
        // Let's simplify: the test will verify agents can be *created*.
        // The specific payload checking for DummyActionAgent might need a dedicated test
        // that manually constructs it with the probe, separate from testing the main recipe run with auto-registry.

        // Create AgentRunner with the auto-populated registry
        let mut agent_runner = AgentRunner::new(
            pool.clone(),
            Arc::new(agent_registry_for_test), // This registry is now auto-populated
            event_collector.clone() as Arc<dyn SdkEventPublisher>,
            None, // No NATS connection for this test
        );

        // Retrieve the probe from the instantiated DummyActionAgent
        // This requires AgentRunner to expose its managed agents, or for DummyActionAgent
        // to register its probe globally (which is not ideal).
        // For now, this specific probe assertion part of the test will be challenging
        // without further refactoring of AgentRunner or DummyActionAgent for testability
        // with the auto-registration.
        //
        // The original test relied on a custom factory for DummyActionAgent.
        // To keep the test working with minimal changes while testing auto-registration for DummySourceAgent:
        // We can auto-register DummySourceAgent and manually register the special DummyActionAgent for this test.
        // This is a compromise. A better solution would be to refactor DummyActionAgent.

        let mut agent_registry_compromise = AgentRegistry::new(); // Populates DummySourceAgent
        // Manually register the special DummyActionAgent for this test's probe
        agent_registry_compromise.register_agent_factory(
            AgentKind::new("DummyActionAgent"), // Matches config
             Box::new(move |config: AgentConfig| { // AgentFactory takes AgentConfig directly
                let agent = DummyActionAgent::new_with_probe(config, action_agent_received_payloads_clone.clone());
                Ok(Box::new(agent) as Box<dyn SdkAgent>)
            }),
        ).expect("Failed to register action agent factory for test (compromise)");


        let mut agent_runner_compromised = AgentRunner::new(
            pool.clone(),
            Arc::new(agent_registry_compromise),
            event_collector.clone() as Arc<dyn SdkEventPublisher>,
            None,
        );


        agent_runner_compromised
            .load_configs_from_db(&profile_id)
            .await
            .expect("Failed to load agent configs from DB for test runner");

        agent_runner
            .instantiate_agents_from_configs()
            .await
            .expect("Failed to instantiate agents for test runner");

        event_collector.clear_events().await;
        action_agent_received_payloads_probe.lock().unwrap().clear();

        agent_runner
            .run_recipe(&recipe_id, None)
            .await
            .expect("Failed to run recipe");

        // 4. Verification
        let collected_events_after_run = event_collector.get_events().await;
        assert_eq!(collected_events_after_run.len(), 1, "Expected one event to be collected after recipe run");
        
        let event_from_collector = collected_events_after_run.first().unwrap();
        assert_eq!(event_from_collector.event_type, "dummy.source.output"); 
        assert_eq!(event_from_collector.source_agent_id, source_agent_id);
        let expected_source_payload = json!({"data": "from_source", "source_id": source_agent_id.to_string()});
        assert_eq!(event_from_collector.data, expected_source_payload);

        let received_payloads_by_action = action_agent_received_payloads_probe.lock().unwrap();
        assert_eq!(received_payloads_by_action.len(), 1, "DummyActionAgent should have received one payload");
        let received_payload = received_payloads_by_action.first().unwrap();
        assert_eq!(*received_payload, expected_source_payload);

        agent_runner_compromised.stop_all_agents().await.expect("Failed to stop agents");
        // Testcontainers automatically cleans up the Docker container when `node` goes out of scope.
    }

    #[tokio::test]
    async fn test_agent_registry_auto_population() {
        // This test verifies that agents defined with macros are auto-registered.
        let registry = AgentRegistry::new(); // Should auto-populate

        // Create a dummy EventPublisher for AgentContext (not strictly used by factory, but create_agent needs it)
        // We need a concrete type that implements EventPublisher for the test.
        // Using InMemoryEventCollector as it's available and implements the trait.
        let dummy_event_publisher = Arc::new(InMemoryEventCollector::new()) as Arc<dyn SdkEventPublisher>;


        // Test if DummySourceAgent can be created
        let source_config = Arc::new(AgentConfig {
            id: AgentId::new_v4(),
            profile_id: ProfileId::new_v4(),
            kind: AgentKind::new("DummySourceAgent"), // Must match struct name
            name: Some("AutoReg Source".to_string()),
            config: json!({}),
            ..Default::default()
        });
        let source_agent_result = registry.create_agent(source_config, Arc::clone(&dummy_event_publisher));
        assert!(source_agent_result.is_ok(), "Failed to create DummySourceAgent via auto-registration: {:?}", source_agent_result.err());

        // Test if DummyActionAgent can be created
        let action_config = Arc::new(AgentConfig {
            id: AgentId::new_v4(),
            profile_id: ProfileId::new_v4(),
            kind: AgentKind::new("DummyActionAgent"), // Must match struct name
            name: Some("AutoReg Action".to_string()),
            config: json!({}),
            ..Default::default()
        });
        let action_agent_result = registry.create_agent(action_config, dummy_event_publisher);
        assert!(action_agent_result.is_ok(), "Failed to create DummyActionAgent via auto-registration: {:?}", action_agent_result.err());

        // Test for a non-existent agent kind
        let non_existent_config = Arc::new(AgentConfig {
            id: AgentId::new_v4(),
            profile_id: ProfileId::new_v4(),
            kind: AgentKind::new("NonExistentAgent"),
            name: Some("Non Existent".to_string()),
            config: json!({}),
            ..Default::default()
        });
        let non_existent_result = registry.create_agent(non_existent_config, Arc::new(InMemoryEventCollector::new()));
        assert!(non_existent_result.is_err(), "Should have failed to create NonExistentAgent");
        if let Err(SdkError::ConfigurationError(msg)) = non_existent_result {
            assert!(msg.contains("No factory registered for agent kind 'NonExistentAgent'"));
        } else {
            panic!("Expected ConfigurationError for NonExistentAgent");
        }
    }
}

// Need to add this to helix-runtime/src/lib.rs or a new tests/mod.rs
// pub mod integration_tests;
// If tests is a directory, then a tests/mod.rs would be:
// pub mod integration_tests;
// And this file would be tests/integration_tests.rs
// For a single file `tests/integration_tests.rs`, Cargo should pick it up automatically.
// However, the module structure inside the file needs `mod tests { ... }`.