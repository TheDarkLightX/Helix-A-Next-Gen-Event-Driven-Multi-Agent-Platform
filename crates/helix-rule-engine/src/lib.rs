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

//! Deterministic rule matching for Helix.

pub mod event_listener;
pub mod rules;

#[cfg(test)]
mod tests {
    use crate::rules::{evaluate_rule, Condition, FieldCondition, Operator, Rule};
    use helix_core::event::Event;
    use serde_json::json;
    use std::collections::HashMap;
    use uuid::Uuid;

    #[test]
    fn crate_exports_deterministic_rule_matching() {
        let event = Event::new(
            "test".to_string(),
            "intel.case.opened".to_string(),
            Some(json!({ "severity": "critical" })),
        );
        let rule = Rule {
            id: Uuid::new_v4(),
            name: "Critical case".to_string(),
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
            actions: Vec::new(),
            created_at: None,
            updated_at: None,
        };

        assert!(evaluate_rule(&event, &rule));
    }
}
