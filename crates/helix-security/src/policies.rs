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

//! Fail-closed policy evaluation primitives.

use crate::errors::SecurityError;

/// Stable policy decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDecision {
    /// Access is allowed.
    Allow,
    /// Access is denied.
    Deny,
}

/// Policy rule effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyEffect {
    /// Matching requests are allowed unless another matching rule denies them.
    Allow,
    /// Matching requests are denied.
    Deny,
}

/// Policy evaluation request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRequest {
    /// Subject identifier, such as an operator or service account.
    pub subject: String,
    /// Action identifier, such as `read` or `execute`.
    pub action: String,
    /// Resource identifier, such as an API route or case id.
    pub resource: String,
}

impl PolicyRequest {
    /// Creates a validated policy request.
    pub fn new(subject: &str, action: &str, resource: &str) -> Result<Self, SecurityError> {
        Ok(Self {
            subject: normalize_component("subject", subject)?,
            action: normalize_component("action", action)?,
            resource: normalize_component("resource", resource)?,
        })
    }
}

/// Exact-or-wildcard policy rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRule {
    subject: String,
    action: String,
    resource: String,
    effect: PolicyEffect,
}

impl PolicyRule {
    /// Creates a validated policy rule. Use `*` as a wildcard component.
    pub fn new(
        subject: &str,
        action: &str,
        resource: &str,
        effect: PolicyEffect,
    ) -> Result<Self, SecurityError> {
        Ok(Self {
            subject: normalize_rule_component("subject", subject)?,
            action: normalize_rule_component("action", action)?,
            resource: normalize_rule_component("resource", resource)?,
            effect,
        })
    }

    fn matches(&self, request: &PolicyRequest) -> bool {
        component_matches(&self.subject, &request.subject)
            && component_matches(&self.action, &request.action)
            && component_matches(&self.resource, &request.resource)
    }
}

/// Deterministic fail-closed policy engine.
#[derive(Debug, Clone)]
pub struct PolicyEngine {
    rules: Vec<PolicyRule>,
}

impl PolicyEngine {
    /// Creates a policy engine from validated rules.
    pub fn new(rules: Vec<PolicyRule>) -> Self {
        Self { rules }
    }

    /// Creates an engine with no rules. Empty policy means deny.
    pub fn empty() -> Self {
        Self { rules: Vec::new() }
    }

    /// Evaluates a request. Deny rules dominate allow rules.
    pub fn evaluate(&self, request: &PolicyRequest) -> PolicyDecision {
        let mut saw_allow = false;
        for rule in &self.rules {
            if !rule.matches(request) {
                continue;
            }
            match rule.effect {
                PolicyEffect::Deny => return PolicyDecision::Deny,
                PolicyEffect::Allow => saw_allow = true,
            }
        }

        if saw_allow {
            PolicyDecision::Allow
        } else {
            PolicyDecision::Deny
        }
    }

    /// Check if an action is allowed. Invalid requests fail closed.
    pub fn is_allowed(&self, subject: &str, action: &str, resource: &str) -> bool {
        let Ok(request) = PolicyRequest::new(subject, action, resource) else {
            return false;
        };
        self.evaluate(&request) == PolicyDecision::Allow
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::empty()
    }
}

fn normalize_component(kind: &str, value: &str) -> Result<String, SecurityError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(SecurityError::PolicyViolation(format!(
            "{kind} cannot be empty"
        )));
    }
    if value == "*" {
        return Err(SecurityError::PolicyViolation(format!(
            "{kind} cannot be a wildcard in a request"
        )));
    }
    Ok(value.to_ascii_lowercase())
}

fn normalize_rule_component(kind: &str, value: &str) -> Result<String, SecurityError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(SecurityError::PolicyViolation(format!(
            "{kind} cannot be empty"
        )));
    }
    if value == "*" {
        Ok(value.to_string())
    } else {
        Ok(value.to_ascii_lowercase())
    }
}

fn component_matches(rule_value: &str, request_value: &str) -> bool {
    rule_value == "*" || rule_value == request_value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_policy_denies_by_default() {
        let engine = PolicyEngine::default();
        assert!(!engine.is_allowed("operator", "read", "case:1"));
    }

    #[test]
    fn allow_rule_can_permit_matching_request() {
        let engine = PolicyEngine::new(vec![PolicyRule::new(
            "operator",
            "read",
            "case:1",
            PolicyEffect::Allow,
        )
        .unwrap()]);
        assert!(engine.is_allowed("operator", "read", "case:1"));
        assert!(!engine.is_allowed("operator", "write", "case:1"));
    }

    #[test]
    fn deny_rules_dominate_allow_rules() {
        let engine = PolicyEngine::new(vec![
            PolicyRule::new("operator", "*", "case:1", PolicyEffect::Allow).unwrap(),
            PolicyRule::new("operator", "delete", "case:1", PolicyEffect::Deny).unwrap(),
        ]);
        assert!(engine.is_allowed("operator", "read", "case:1"));
        assert!(!engine.is_allowed("operator", "delete", "case:1"));
    }

    #[test]
    fn invalid_policy_requests_fail_closed() {
        let engine = PolicyEngine::new(vec![
            PolicyRule::new("*", "*", "*", PolicyEffect::Allow).unwrap()
        ]);
        assert!(!engine.is_allowed("", "read", "case:1"));
        assert!(!engine.is_allowed("*", "read", "case:1"));
    }

    #[test]
    fn rule_components_are_validated() {
        assert!(PolicyRule::new("", "read", "case:1", PolicyEffect::Allow).is_err());
        assert!(PolicyRule::new("operator", "", "case:1", PolicyEffect::Allow).is_err());
        assert!(PolicyRule::new("operator", "read", "", PolicyEffect::Allow).is_err());
    }
}
