use helix_agent_sdk::{AgentContext, EventPublisher, SdkAgent, SdkError};
use helix_core::agent::{Agent, AgentConfig};
use helix_core::credential::{CredentialProvider, EnvCredentialProvider};
use helix_core::event::Event;
use helix_core::recipe::{Recipe, RecipeGraphDefinition};
use helix_core::state::{InMemoryStateStore, StateStore};
use helix_core::types::AgentId;
use helix_rule_engine::event_listener::RuleEngineEventListener;
use helix_rule_engine::rules::{Action, Condition, FieldCondition, Operator, ParameterValue, Rule};
use helix_runtime::agent_registry::AgentRegistry;
use helix_runtime::agent_runner::AgentRunner;
use helix_runtime::{AgentStatus, InMemoryEventCollector};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

struct NoopAgent {
    agent_config: Arc<AgentConfig>,
}

#[async_trait::async_trait]
impl Agent for NoopAgent {
    fn id(&self) -> AgentId {
        self.agent_config.id
    }

    fn config(&self) -> &AgentConfig {
        &self.agent_config
    }
}

#[async_trait::async_trait]
impl SdkAgent for NoopAgent {
    async fn init(&mut self, _context: &AgentContext) -> Result<(), SdkError> {
        Ok(())
    }

    async fn start(&mut self, _context: &AgentContext) -> Result<(), SdkError> {
        Ok(())
    }

    async fn stop(&mut self, _context: &AgentContext) -> Result<(), SdkError> {
        Ok(())
    }
}

fn registry_with_noop() -> Arc<AgentRegistry> {
    let mut registry = AgentRegistry::new().unwrap_or_else(|_| AgentRegistry::default());
    registry
        .register(
            "noop",
            Box::new(|config: AgentConfig| {
                Ok(Box::new(NoopAgent {
                    agent_config: Arc::new(config),
                }))
            }),
        )
        .unwrap();
    Arc::new(registry)
}

fn critical_case_rule(recipe_id: Uuid) -> Rule {
    let mut parameters = HashMap::new();
    parameters.insert(
        "case_id".to_string(),
        ParameterValue::FromEvent("event.data.case_id".to_string()),
    );
    parameters.insert(
        "mode".to_string(),
        ParameterValue::Literal(json!("prepare_brief")),
    );

    Rule {
        id: Uuid::parse_str("40000000-0000-0000-0000-000000000001").unwrap(),
        name: "Critical case recipe".to_string(),
        description: None,
        version: "1.0.0".to_string(),
        author: None,
        enabled: true,
        tags: Vec::new(),
        metadata: HashMap::new(),
        condition: Condition::Field(Box::new(FieldCondition {
            field: "event.data.severity".to_string(),
            operator: Operator::Equals,
            value: Some(json!("critical")),
            value_from_event: None,
            case_sensitive: true,
        })),
        actions: vec![Action {
            r#type: "trigger_recipe".to_string(),
            recipe_id: Some(recipe_id),
            recipe_name: None,
            parameters,
            delay: None,
            on_failure: "log".to_string(),
            action_id: None,
        }],
        created_at: None,
        updated_at: None,
    }
}

#[tokio::test]
async fn event_rule_recipe_agent_execution_is_deterministic() {
    let profile_id = Uuid::new_v4();
    let recipe_id = Uuid::parse_str("40000000-0000-0000-0000-000000000002").unwrap();
    let first_agent_id = Uuid::parse_str("40000000-0000-0000-0000-000000000003").unwrap();
    let second_agent_id = Uuid::parse_str("40000000-0000-0000-0000-000000000004").unwrap();

    let first_agent = AgentConfig::new(
        first_agent_id,
        profile_id,
        Some("Normalize case".to_string()),
        "noop".to_string(),
        json!({}),
    );
    let mut second_agent = AgentConfig::new(
        second_agent_id,
        profile_id,
        Some("Prepare brief".to_string()),
        "noop".to_string(),
        json!({}),
    );
    second_agent.dependencies = vec![first_agent_id];

    let recipe = Recipe::new(
        recipe_id,
        profile_id,
        "Critical Case Response".to_string(),
        None,
        RecipeGraphDefinition {
            agents: vec![second_agent, first_agent],
        },
    );

    let listener = RuleEngineEventListener::new(vec![critical_case_rule(recipe_id)]);
    let event = Event::new(
        "intel".to_string(),
        "intel.case.opened".to_string(),
        Some(json!({
            "case_id": "case_9000",
            "severity": "critical"
        })),
    );

    let plans = listener.handle_event(&event);
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].recipe_id, Some(recipe_id));
    assert_eq!(
        plans[0].parameters.get("case_id"),
        Some(&json!("case_9000"))
    );
    assert_eq!(
        plans[0].parameters.get("mode"),
        Some(&json!("prepare_brief"))
    );

    let publisher: Arc<dyn EventPublisher> = Arc::new(InMemoryEventCollector::new());
    let creds: Arc<dyn CredentialProvider> = Arc::new(EnvCredentialProvider::default());
    let state: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::new());
    let mut runner = AgentRunner::new_native(registry_with_noop(), publisher, creds, state);

    let started = runner.run_recipe(&recipe).await.unwrap();

    assert_eq!(started, vec![first_agent_id, second_agent_id]);
    assert_eq!(
        runner.agent_status(&first_agent_id),
        Some(AgentStatus::Running)
    );
    assert_eq!(
        runner.agent_status(&second_agent_id),
        Some(AgentStatus::Running)
    );
}
