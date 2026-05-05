//! Deterministic event-to-recipe trigger planning.

use crate::rules::{plan_recipe_triggers, RecipeTriggerPlan, Rule};
use helix_core::event::Event;

/// Pure rule listener kernel.
///
/// Side-effect adapters such as NATS consumers call this kernel with decoded
/// events, then pass returned trigger plans to a recipe runner.
#[derive(Debug, Clone, Default)]
pub struct RuleEngineEventListener {
    rules: Vec<Rule>,
}

impl RuleEngineEventListener {
    /// Creates a listener from an ordered rule set.
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    /// Returns the loaded rules.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// Evaluates one event and returns deterministic recipe trigger plans.
    pub fn handle_event(&self, event: &Event) -> Vec<RecipeTriggerPlan> {
        plan_recipe_triggers(event, &self.rules)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Action, Condition, FieldCondition, Operator, ParameterValue};
    use serde_json::json;
    use std::collections::HashMap;
    use uuid::Uuid;

    #[test]
    fn handle_event_returns_trigger_plan_for_matching_rule() {
        let recipe_id = Uuid::parse_str("30000000-0000-0000-0000-000000000001").unwrap();
        let event = Event::new(
            "intel".to_string(),
            "intel.case.opened".to_string(),
            Some(json!({
                "case_id": "case_42",
                "severity": "critical"
            })),
        );
        let mut parameters = HashMap::new();
        parameters.insert(
            "case_id".to_string(),
            ParameterValue::FromEvent("event.data.case_id".to_string()),
        );
        let rule = Rule {
            id: Uuid::parse_str("30000000-0000-0000-0000-000000000002").unwrap(),
            name: "Critical case automation".to_string(),
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
        };

        let listener = RuleEngineEventListener::new(vec![rule]);
        let plans = listener.handle_event(&event);

        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].recipe_id, Some(recipe_id));
        assert_eq!(plans[0].parameters.get("case_id"), Some(&json!("case_42")));
    }
}
