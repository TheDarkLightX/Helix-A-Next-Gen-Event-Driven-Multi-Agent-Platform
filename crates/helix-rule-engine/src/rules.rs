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

//! Deterministic rule model and evaluation kernel for Helix automation.
//!
//! Rules are pure data: a condition tree over event fields plus a list of
//! actions. Evaluation is total and deterministic. The kernel never panics on
//! malformed input; missing fields or type mismatches simply fail the
//! condition (fail-closed).

use chrono::{DateTime, Utc};
use helix_core::event::Event;
use helix_core::types::RecipeId;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

/// A deterministic automation rule.
///
/// Rules are evaluated against incoming events. When the condition matches,
/// each action produces a [`RecipeTriggerPlan`] that the imperative shell may
/// execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// Stable unique identifier for the rule.
    pub id: Uuid,
    /// Human-readable name.
    pub name: String,
    /// Optional long-form description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Semantic version of the rule definition.
    #[serde(default = "default_version")]
    pub version: String,
    /// Optional author attribution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Whether the rule is active. Disabled rules never produce plans.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Free-form tags for grouping and filtering.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Arbitrary metadata preserved verbatim.
    #[serde(default)]
    pub metadata: HashMap<String, Value>,
    /// The condition tree evaluated against an event.
    pub condition: Condition,
    /// Actions to take when the condition matches.
    #[serde(default)]
    pub actions: Vec<Action>,
    /// Creation timestamp (set by the persistence layer).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
    /// Last update timestamp (set by the persistence layer).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

fn default_enabled() -> bool {
    true
}

/// A condition tree over event fields.
///
/// Serializes as an untagged enum so the JSON shape mirrors the field
/// condition object (`{"field": ..., "operator": ..., "value": ...}`) or a
/// logical combinator (`{"and": [...]}`, `{"or": [...]}`, `{"not": ...}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Condition {
    /// A leaf condition testing a single event field.
    Field(Box<FieldCondition>),
    /// Logical AND over a list of sub-conditions.
    And(ConditionList),
    /// Logical OR over a list of sub-conditions.
    Or(ConditionList),
    /// Logical NOT over a single sub-condition.
    Not(Box<Condition>),
}

/// Wrapper for the logical combinator variants so the JSON shape is
/// `{"and": [...]}` / `{"or": [...]}` with a single key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionList {
    /// The list of sub-conditions under the `and` key.
    #[serde(rename = "and", default)]
    pub and: Vec<Condition>,
    /// The list of sub-conditions under the `or` key.
    #[serde(rename = "or", default)]
    pub or: Vec<Condition>,
}

/// A leaf condition testing a single event field against a value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldCondition {
    /// Dotted path into the event, e.g. `event.data.severity` or `event.source`.
    pub field: String,
    /// Comparison operator.
    pub operator: Operator,
    /// Literal comparison value. Ignored for existence/null/boolean/empty
    /// operators.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// Pull the comparison value from the event at the given path instead of
    /// using a literal `value`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_from_event: Option<String>,
    /// Whether string comparisons are case-sensitive. Defaults to `true`.
    #[serde(default = "default_case_sensitive")]
    pub case_sensitive: bool,
}

fn default_case_sensitive() -> bool {
    true
}

/// Comparison operators supported by [`FieldCondition`].
///
/// Serializes as the lowercase snake_case name (e.g. `equals`,
/// `greater_than_or_equals`, `regex_matches`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    Equals,
    NotEquals,
    GreaterThan,
    GreaterThanOrEquals,
    LessThan,
    LessThanOrEquals,
    Contains,
    NotContains,
    StartsWith,
    EndsWith,
    RegexMatches,
    Exists,
    NotExists,
    IsNull,
    IsNotNull,
    IsTrue,
    IsFalse,
    IsEmpty,
    IsNotEmpty,
    In,
    NotIn,
    TypeIs,
}

/// A parameter value for a rule action: either a literal JSON value or a
/// reference to a path in the triggering event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterValue {
    /// A literal JSON value.
    Literal(Value),
    /// Pull the value from the triggering event at the given dotted path.
    FromEvent(String),
}

/// An action to take when a rule's condition matches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// The action type. Currently only `trigger_recipe` is supported.
    #[serde(rename = "type")]
    pub r#type: String,
    /// Target recipe by id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_id: Option<RecipeId>,
    /// Target recipe by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_name: Option<String>,
    /// Parameters to pass to the recipe.
    #[serde(default)]
    pub parameters: HashMap<String, ParameterValue>,
    /// Optional delay before executing the action (ISO 8601 duration).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay: Option<String>,
    /// What to do on failure: `log`, `retry`, or `escalate`. Defaults to `log`.
    #[serde(default = "default_on_failure")]
    pub on_failure: String,
    /// Optional stable id for correlating trigger plans back to a specific
    /// action within a rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
}

fn default_on_failure() -> String {
    "log".to_string()
}

/// A deterministic plan to trigger a recipe, produced when a rule matches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeTriggerPlan {
    /// The id of the rule that produced this plan.
    pub rule_id: Uuid,
    /// The name of the rule that produced this plan.
    pub rule_name: String,
    /// The action id, if the source action specified one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    /// The target recipe id, if the action specified one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_id: Option<RecipeId>,
    /// The target recipe name, if the action specified one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_name: Option<String>,
    /// Resolved parameter values (literals or values pulled from the event).
    #[serde(default)]
    pub parameters: HashMap<String, Value>,
}

/// Evaluate a single rule's condition against an event.
///
/// Disabled rules never match. The evaluation is total: any error in path
/// resolution or type coercion fails the condition (fail-closed).
pub fn evaluate_rule(event: &Event, rule: &Rule) -> bool {
    if !rule.enabled {
        return false;
    }
    evaluate_condition(event, &rule.condition)
}

/// Evaluate a condition tree against an event.
pub fn evaluate_condition(event: &Event, condition: &Condition) -> bool {
    match condition {
        Condition::Field(field) => evaluate_field(event, field),
        Condition::And(list) => list.and.iter().all(|c| evaluate_condition(event, c)),
        Condition::Or(list) => list.or.iter().any(|c| evaluate_condition(event, c)),
        Condition::Not(inner) => !evaluate_condition(event, inner),
    }
}

/// Evaluate a leaf field condition against an event.
pub fn evaluate_field(event: &Event, cond: &FieldCondition) -> bool {
    let actual = resolve_event_path(event, &cond.field);
    let expected = cond
        .value_from_event
        .as_deref()
        .and_then(|path| resolve_event_path(event, path))
        .or(cond.value.clone());

    match cond.operator {
        Operator::Exists => actual.is_some(),
        Operator::NotExists => actual.is_none(),
        Operator::IsNull => matches!(actual.as_ref(), Some(Value::Null) | None),
        Operator::IsNotNull => !matches!(actual.as_ref(), Some(Value::Null) | None),
        Operator::IsTrue => actual.as_ref() == Some(&Value::Bool(true)),
        Operator::IsFalse => actual.as_ref() == Some(&Value::Bool(false)),
        Operator::IsEmpty => match actual.as_ref() {
            Some(Value::String(s)) => s.is_empty(),
            Some(Value::Array(a)) => a.is_empty(),
            Some(Value::Object(o)) => o.is_empty(),
            Some(Value::Null) | None => true,
            _ => false,
        },
        Operator::IsNotEmpty => match actual.as_ref() {
            Some(Value::String(s)) => !s.is_empty(),
            Some(Value::Array(a)) => !a.is_empty(),
            Some(Value::Object(o)) => !o.is_empty(),
            Some(Value::Null) | None => false,
            _ => true,
        },
        Operator::TypeIs => match (actual.as_ref(), expected.as_ref()) {
            (Some(actual_val), Some(expected_val)) => type_matches(actual_val, expected_val),
            _ => false,
        },
        Operator::In => match (actual.as_ref(), expected.as_ref()) {
            (Some(actual_val), Some(Value::Array(items))) => items
                .iter()
                .any(|item| values_equal(actual_val, item, cond.case_sensitive)),
            _ => false,
        },
        Operator::NotIn => match (actual.as_ref(), expected.as_ref()) {
            (Some(actual_val), Some(Value::Array(items))) => !items
                .iter()
                .any(|item| values_equal(actual_val, item, cond.case_sensitive)),
            _ => false,
        },
        Operator::Equals => match (actual.as_ref(), expected.as_ref()) {
            (Some(a), Some(b)) => values_equal(a, b, cond.case_sensitive),
            _ => false,
        },
        Operator::NotEquals => match (actual.as_ref(), expected.as_ref()) {
            (Some(a), Some(b)) => !values_equal(a, b, cond.case_sensitive),
            _ => true,
        },
        Operator::GreaterThan
        | Operator::GreaterThanOrEquals
        | Operator::LessThan
        | Operator::LessThanOrEquals => match (actual.as_ref(), expected.as_ref()) {
            (Some(a), Some(b)) => compare_numbers(a, b, cond.operator),
            _ => false,
        },
        Operator::Contains => match (actual.as_ref(), expected.as_ref()) {
            (Some(Value::String(haystack)), Some(Value::String(needle))) => {
                string_contains(haystack, needle, cond.case_sensitive)
            }
            (Some(Value::Array(haystack)), Some(needle)) => haystack
                .iter()
                .any(|item| values_equal(item, needle, cond.case_sensitive)),
            _ => false,
        },
        Operator::NotContains => match (actual.as_ref(), expected.as_ref()) {
            (Some(Value::String(haystack)), Some(Value::String(needle))) => {
                !string_contains(haystack, needle, cond.case_sensitive)
            }
            (Some(Value::Array(haystack)), Some(needle)) => !haystack
                .iter()
                .any(|item| values_equal(item, needle, cond.case_sensitive)),
            _ => true,
        },
        Operator::StartsWith => match (actual.as_ref(), expected.as_ref()) {
            (Some(Value::String(haystack)), Some(Value::String(needle))) => {
                string_starts_with(haystack, needle, cond.case_sensitive)
            }
            _ => false,
        },
        Operator::EndsWith => match (actual.as_ref(), expected.as_ref()) {
            (Some(Value::String(haystack)), Some(Value::String(needle))) => {
                string_ends_with(haystack, needle, cond.case_sensitive)
            }
            _ => false,
        },
        Operator::RegexMatches => match (actual.as_ref(), expected.as_ref()) {
            (Some(Value::String(haystack)), Some(Value::String(pattern))) => {
                regex_matches(haystack, pattern)
            }
            _ => false,
        },
    }
}

/// Resolve a dotted path like `event.data.severity` against an event.
///
/// Returns `None` if any segment is missing or the path cannot be traversed.
/// The leading `event.` prefix is optional.
pub fn resolve_event_path(event: &Event, path: &str) -> Option<Value> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut segments: Vec<&str> = trimmed.split('.').collect();
    if segments.first().is_some_and(|s| *s == "event") {
        segments.remove(0);
    }

    let Some(first) = segments.first() else {
        return None;
    };
    let mut current = match *first {
        "id" => Some(Value::String(event.id.to_string())),
        "source" => Some(Value::String(event.source.clone())),
        "type" => Some(Value::String(event.r#type.clone())),
        "specversion" => Some(Value::String(event.specversion.clone())),
        "datacontenttype" => event.datacontenttype.clone().map(Value::String),
        "subject" => event.subject.clone().map(Value::String),
        "time" => Some(Value::String(event.time.to_rfc3339())),
        "correlation_id" => event.correlation_id.map(|id| Value::String(id.to_string())),
        "causation_id" => event.causation_id.map(|id| Value::String(id.to_string())),
        "data" => event.data.clone(),
        other => event
            .data
            .as_ref()
            .and_then(|data| data.get(other).cloned()),
    };

    for segment in segments.iter().skip(1) {
        current = current.and_then(|value| value.get(segment).cloned());
    }

    current
}

/// Compare two JSON values for equality, with optional case-insensitivity for
/// strings.
fn values_equal(a: &Value, b: &Value, case_sensitive: bool) -> bool {
    match (a, b) {
        (Value::String(a), Value::String(b)) => {
            if case_sensitive {
                a == b
            } else {
                a.eq_ignore_ascii_case(b)
            }
        }
        (Value::Number(a), Value::Number(b)) => {
            a.as_f64().zip(b.as_f64()).is_some_and(|(x, y)| x == y)
        }
        _ => a == b,
    }
}

/// Compare two JSON values as numbers with a relational operator.
fn compare_numbers(a: &Value, b: &Value, op: Operator) -> bool {
    let Some(a) = value_as_f64(a) else {
        return false;
    };
    let Some(b) = value_as_f64(b) else {
        return false;
    };
    match op {
        Operator::GreaterThan => a > b,
        Operator::GreaterThanOrEquals => a >= b,
        Operator::LessThan => a < b,
        Operator::LessThanOrEquals => a <= b,
        _ => false,
    }
}

/// Coerce a JSON value to an f64, supporting numbers and numeric strings.
fn value_as_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// Case-aware string containment check.
fn string_contains(haystack: &str, needle: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        haystack.contains(needle)
    } else {
        haystack
            .to_ascii_lowercase()
            .contains(&needle.to_ascii_lowercase())
    }
}

/// Case-aware string prefix check.
fn string_starts_with(haystack: &str, needle: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        haystack.starts_with(needle)
    } else {
        haystack
            .to_ascii_lowercase()
            .starts_with(&needle.to_ascii_lowercase())
    }
}

/// Case-aware string suffix check.
fn string_ends_with(haystack: &str, needle: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        haystack.ends_with(needle)
    } else {
        haystack
            .to_ascii_lowercase()
            .ends_with(&needle.to_ascii_lowercase())
    }
}

/// Check whether the actual JSON value matches a named type.
///
/// The expected value is a string naming the type: `string`, `number`,
/// `boolean`, `array`, `object`, `null`, `integer`.
fn type_matches(actual: &Value, expected: &Value) -> bool {
    let Some(type_name) = expected.as_str() else {
        return false;
    };
    match type_name {
        "string" => actual.is_string(),
        "number" => actual.is_number(),
        "integer" => actual.as_number().and_then(serde_json::Number::as_i64).is_some(),
        "boolean" => actual.is_boolean(),
        "array" => actual.is_array(),
        "object" => actual.is_object(),
        "null" => actual.is_null(),
        _ => false,
    }
}

/// Compile and match a regex pattern. Returns `false` on invalid patterns
/// (fail-closed).
fn regex_matches(haystack: &str, pattern: &str) -> bool {
    match Regex::new(pattern) {
        Ok(re) => re.is_match(haystack),
        Err(_) => false,
    }
}

/// Resolve a parameter value against an event.
pub fn resolve_parameter(event: &Event, value: &ParameterValue) -> Value {
    match value {
        ParameterValue::Literal(v) => v.clone(),
        ParameterValue::FromEvent(path) => {
            resolve_event_path(event, path).unwrap_or(Value::Null)
        }
    }
}

/// Produce deterministic recipe trigger plans for all matching rules.
///
/// Rules are evaluated in order. Each matching action produces a separate
/// plan. Disabled rules are skipped.
pub fn plan_recipe_triggers(event: &Event, rules: &[Rule]) -> Vec<RecipeTriggerPlan> {
    let mut plans = Vec::new();
    for rule in rules {
        if !rule.enabled {
            continue;
        }
        if !evaluate_condition(event, &rule.condition) {
            continue;
        }
        for action in &rule.actions {
            if action.r#type != "trigger_recipe" {
                continue;
            }
            let parameters = action
                .parameters
                .iter()
                .map(|(key, value)| (key.clone(), resolve_parameter(event, value)))
                .collect();
            plans.push(RecipeTriggerPlan {
                rule_id: rule.id,
                rule_name: rule.name.clone(),
                action_id: action.action_id.clone(),
                recipe_id: action.recipe_id,
                recipe_name: action.recipe_name.clone(),
                parameters,
            });
        }
    }
    plans
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn make_event(data: Value) -> Event {
        Event::new("intel".to_string(), "intel.case.opened".to_string(), Some(data))
    }

    fn field_condition(field: &str, operator: Operator, value: Option<Value>) -> Condition {
        Condition::Field(Box::new(FieldCondition {
            field: field.to_string(),
            operator,
            value,
            value_from_event: None,
            case_sensitive: true,
        }))
    }

    fn make_rule(condition: Condition) -> Rule {
        Rule {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            description: None,
            version: "1.0.0".to_string(),
            author: None,
            enabled: true,
            tags: Vec::new(),
            metadata: HashMap::new(),
            condition,
            actions: Vec::new(),
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn evaluate_rule_returns_false_for_disabled_rule() {
        let event = make_event(json!({ "severity": "critical" }));
        let mut rule = make_rule(field_condition(
            "event.data.severity",
            Operator::Equals,
            Some(json!("critical")),
        ));
        rule.enabled = false;
        assert!(!evaluate_rule(&event, &rule));
    }

    #[test]
    fn evaluate_rule_matches_equals_on_string() {
        let event = make_event(json!({ "severity": "critical" }));
        let rule = make_rule(field_condition(
            "event.data.severity",
            Operator::Equals,
            Some(json!("critical")),
        ));
        assert!(evaluate_rule(&event, &rule));
    }

    #[test]
    fn resolve_event_path_supports_top_level_fields() {
        let event = make_event(json!({ "severity": "critical" }));
        assert_eq!(
            resolve_event_path(&event, "event.source"),
            Some(Value::String("intel".to_string()))
        );
        assert_eq!(
            resolve_event_path(&event, "event.type"),
            Some(Value::String("intel.case.opened".to_string()))
        );
    }

    #[test]
    fn resolve_event_path_supports_data_without_prefix() {
        let event = make_event(json!({ "severity": "critical" }));
        assert_eq!(
            resolve_event_path(&event, "severity"),
            Some(Value::String("critical".to_string()))
        );
    }

    #[test]
    fn resolve_event_path_returns_none_for_missing() {
        let event = make_event(json!({ "severity": "critical" }));
        assert_eq!(resolve_event_path(&event, "event.data.missing"), None);
    }

    #[test]
    fn operator_exists_and_not_exists() {
        let event = make_event(json!({ "severity": "critical" }));
        let exists = field_condition("event.data.severity", Operator::Exists, None);
        let missing = field_condition("event.data.missing", Operator::Exists, None);
        assert!(evaluate_condition(&event, &exists));
        assert!(!evaluate_condition(&event, &missing));

        let not_exists = field_condition("event.data.missing", Operator::NotExists, None);
        assert!(evaluate_condition(&event, &not_exists));
    }

    #[test]
    fn operator_greater_than_compares_numbers() {
        let event = make_event(json!({ "score": 75 }));
        let gt = field_condition("event.data.score", Operator::GreaterThan, Some(json!(50)));
        let lt = field_condition("event.data.score", Operator::LessThan, Some(json!(50)));
        assert!(evaluate_condition(&event, &gt));
        assert!(!evaluate_condition(&event, &lt));
    }

    #[test]
    fn operator_greater_than_compares_numeric_strings() {
        let event = make_event(json!({ "score": "75" }));
        let gt = field_condition("event.data.score", Operator::GreaterThan, Some(json!(50)));
        assert!(evaluate_condition(&event, &gt));
    }

    #[test]
    fn operator_contains_on_string() {
        let event = make_event(json!({ "message": "hello world" }));
        let contains =
            field_condition("event.data.message", Operator::Contains, Some(json!("world")));
        assert!(evaluate_condition(&event, &contains));
    }

    #[test]
    fn operator_contains_case_insensitive() {
        let event = make_event(json!({ "message": "Hello World" }));
        let cond = Condition::Field(Box::new(FieldCondition {
            field: "event.data.message".to_string(),
            operator: Operator::Contains,
            value: Some(json!("world")),
            value_from_event: None,
            case_sensitive: false,
        }));
        assert!(evaluate_condition(&event, &cond));
    }

    #[test]
    fn operator_regex_matches() {
        let event = make_event(json!({ "email": "user@example.com" }));
        let regex = field_condition(
            "event.data.email",
            Operator::RegexMatches,
            Some(json!("^[^@]+@[^@]+$")),
        );
        assert!(evaluate_condition(&event, &regex));
    }

    #[test]
    fn operator_in_array() {
        let event = make_event(json!({ "status": "open" }));
        let cond = field_condition(
            "event.data.status",
            Operator::In,
            Some(json!(["open", "pending"])),
        );
        assert!(evaluate_condition(&event, &cond));
    }

    #[test]
    fn operator_type_is() {
        let event = make_event(json!({ "count": 42, "name": "test" }));
        let is_number =
            field_condition("event.data.count", Operator::TypeIs, Some(json!("number")));
        let is_string =
            field_condition("event.data.count", Operator::TypeIs, Some(json!("string")));
        assert!(evaluate_condition(&event, &is_number));
        assert!(!evaluate_condition(&event, &is_string));
    }

    #[test]
    fn logical_and_or_not() {
        let event = make_event(json!({ "severity": "critical", "source": "intel" }));

        let and = Condition::And(ConditionList {
            and: vec![
                field_condition("event.data.severity", Operator::Equals, Some(json!("critical"))),
                field_condition("event.data.source", Operator::Equals, Some(json!("intel"))),
            ],
            or: Vec::new(),
        });
        assert!(evaluate_condition(&event, &and));

        let or = Condition::Or(ConditionList {
            and: Vec::new(),
            or: vec![
                field_condition("event.data.severity", Operator::Equals, Some(json!("low"))),
                field_condition("event.data.source", Operator::Equals, Some(json!("intel"))),
            ],
        });
        assert!(evaluate_condition(&event, &or));

        let not = Condition::Not(Box::new(field_condition(
            "event.data.severity",
            Operator::Equals,
            Some(json!("low")),
        )));
        assert!(evaluate_condition(&event, &not));
    }

    #[test]
    fn plan_recipe_triggers_produces_plans_for_matching_rules() {
        let recipe_id: RecipeId =
            Uuid::parse_str("30000000-0000-0000-0000-000000000001").unwrap();
        let event = make_event(json!({ "severity": "critical", "case_id": "case_42" }));

        let mut parameters = HashMap::new();
        parameters.insert(
            "case_id".to_string(),
            ParameterValue::FromEvent("event.data.case_id".to_string()),
        );
        parameters.insert(
            "mode".to_string(),
            ParameterValue::Literal(json!("prepare_brief")),
        );

        let mut rule = make_rule(field_condition(
            "event.data.severity",
            Operator::Equals,
            Some(json!("critical")),
        ));
        rule.actions = vec![Action {
            r#type: "trigger_recipe".to_string(),
            recipe_id: Some(recipe_id),
            recipe_name: None,
            parameters,
            delay: None,
            on_failure: "log".to_string(),
            action_id: None,
        }];

        let plans = plan_recipe_triggers(&event, &[rule]);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].recipe_id, Some(recipe_id));
        assert_eq!(plans[0].parameters.get("case_id"), Some(&json!("case_42")));
        assert_eq!(plans[0].parameters.get("mode"), Some(&json!("prepare_brief")));
    }

    #[test]
    fn rule_deserializes_from_compact_json() {
        let json = serde_json::json!({
            "id": "50000000-0000-0000-0000-000000000001",
            "name": "Critical case automation",
            "condition": {
                "field": "event.data.severity",
                "operator": "equals",
                "value": "critical"
            },
            "actions": [
                {
                    "type": "trigger_recipe",
                    "recipe_id": "50000000-0000-0000-0000-000000000002",
                    "parameters": {
                        "case_id": { "from_event": "event.data.case_id" },
                        "mode": { "literal": "prepare_brief" }
                    }
                }
            ]
        });
        let rule: Rule = serde_json::from_value(json).unwrap();
        assert_eq!(rule.name, "Critical case automation");
        assert_eq!(rule.version, "1.0.0");
        assert!(rule.enabled);
        assert_eq!(rule.actions.len(), 1);
        assert_eq!(rule.actions[0].on_failure, "log");
    }

    #[test]
    fn rule_serializes_and_round_trips() {
        let rule = make_rule(field_condition(
            "event.data.severity",
            Operator::Equals,
            Some(json!("critical")),
        ));
        let serialized = serde_json::to_string(&rule).unwrap();
        let deserialized: Rule = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.id, rule.id);
        assert_eq!(deserialized.name, rule.name);
    }

    #[test]
    fn logical_condition_deserializes_from_json() {
        let json = serde_json::json!({
            "and": [
                {
                    "field": "event.data.severity",
                    "operator": "equals",
                    "value": "critical"
                },
                {
                    "or": [
                        {
                            "field": "event.data.source",
                            "operator": "equals",
                            "value": "intel"
                        },
                        {
                            "field": "event.data.source",
                            "operator": "equals",
                            "value": "osint"
                        }
                    ]
                }
            ]
        });
        let condition: Condition = serde_json::from_value(json).unwrap();
        let event = make_event(json!({ "severity": "critical", "source": "osint" }));
        assert!(evaluate_condition(&event, &condition));
    }

    #[test]
    fn value_from_event_resolves_dynamically() {
        let event = make_event(json!({ "severity": "critical", "expected": "critical" }));
        let cond = Condition::Field(Box::new(FieldCondition {
            field: "event.data.severity".to_string(),
            operator: Operator::Equals,
            value: None,
            value_from_event: Some("event.data.expected".to_string()),
            case_sensitive: true,
        }));
        assert!(evaluate_condition(&event, &cond));
    }

    #[test]
    fn operator_is_null_and_is_not_null() {
        let event = make_event(json!({ "present": "value", "absent": null }));
        let is_null = field_condition("event.data.absent", Operator::IsNull, None);
        let is_not_null = field_condition("event.data.present", Operator::IsNotNull, None);
        assert!(evaluate_condition(&event, &is_null));
        assert!(evaluate_condition(&event, &is_not_null));
    }

    #[test]
    fn operator_is_empty_and_is_not_empty() {
        let event = make_event(json!({ "empty_str": "", "items": [1, 2] }));
        let is_empty = field_condition("event.data.empty_str", Operator::IsEmpty, None);
        let is_not_empty = field_condition("event.data.items", Operator::IsNotEmpty, None);
        assert!(evaluate_condition(&event, &is_empty));
        assert!(evaluate_condition(&event, &is_not_empty));
    }

    #[test]
    fn operator_starts_with_and_ends_with() {
        let event = make_event(json!({ "path": "/api/v1/cases" }));
        let starts = field_condition("event.data.path", Operator::StartsWith, Some(json!("/api")));
        let ends = field_condition("event.data.path", Operator::EndsWith, Some(json!("cases")));
        assert!(evaluate_condition(&event, &starts));
        assert!(evaluate_condition(&event, &ends));
    }

    #[test]
    fn invalid_regex_fails_closed() {
        let event = make_event(json!({ "value": "test" }));
        let cond = field_condition("event.data.value", Operator::RegexMatches, Some(json!("(")));
        assert!(!evaluate_condition(&event, &cond));
    }
}
