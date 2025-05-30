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


#![warn(missing_docs)]

//! Defines the core Policy structures, evaluation context, and related traits for Helix.
//!
//! This module provides the foundational components for policy definition,
//! evaluation, and storage within the Helix platform. It aims to create a flexible
//! system that can potentially integrate with external policy engines like Cedar
//! in the future, while providing a clear, structured approach to authorization.

use crate::errors::HelixError;
use crate::types::PolicyId;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Effect of a policy (Allow or Deny).
///
/// Determines whether a policy, when matched, permits or prohibits an action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PolicyEffect {
    /// The policy allows the action.
    Allow,
    /// The policy denies the action.
    Deny,
}

/// Represents a policy for controlling access and actions within the Helix platform.
///
/// This structure defines the core components of a policy, including its effect,
/// the actions it governs, the resources it applies to, the principals it affects,
/// and any conditions that must be met.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Policy {
    /// Unique identifier for this policy.
    pub policy_id: PolicyId,
    /// The effect of this policy (Allow or Deny).
    pub effect: PolicyEffect,
    /// A list of actions this policy applies to (e.g., "agent:run", "recipe:execute").
    /// Actions are typically represented as strings.
    pub actions: Vec<String>,
    /// A list of resources this policy applies to (e.g., "agent_id:*", "recipe_id:my_recipe").
    /// Resource identifiers can be specific or use wildcards for broader application.
    pub resources: Vec<String>,
    /// A list of principals (e.g., users, agent types, specific agent instances)
    /// this policy applies to.
    pub principals: Vec<String>,
    /// Optional conditions for the policy, represented as a JSON value.
    /// This allows for more advanced, context-aware policies, similar to those
    /// expressible in systems like Cedar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Value>,
}

/// Contextual information required for evaluating a policy.
///
/// When a permission check is requested, this context provides all necessary
/// details about the attempted action, the entity performing it, the target resource,
/// and any other relevant environmental attributes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyEvaluationContext {
    /// The principal (e.g., user ID, agent ID) attempting the action.
    pub principal: String,
    /// The action being attempted (e.g., "agent:run", "credential:read").
    pub action: String,
    /// The resource being accessed (e.g., "agent_id:123", "recipe_id:my_recipe").
    pub resource: String,
    /// Other relevant attributes for evaluation, represented as a JSON value.
    /// This can include things like time of day, request parameters, IP address, etc.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Value>,
}

/// Trait for components that can evaluate policies.
///
/// Policy Enforcement Points (PEPs) within the Helix system (e.g., API gateways,
/// service handlers) would use an implementation of this trait to determine
/// whether a requested action should be allowed or denied based on the defined policies
/// and the current evaluation context.
#[async_trait::async_trait]
pub trait PolicyEvaluator {
    /// Checks if a principal has permission to perform an action on a resource,
    /// given the current context.
    ///
    /// # Arguments
    /// * `context` - The [`PolicyEvaluationContext`](PolicyEvaluationContext) for the current request.
    ///
    /// # Returns
    /// * `Ok(true)` if permission is granted.
    /// * `Ok(false)` if permission is denied.
    /// * `Err(HelixError)` if an error occurs during evaluation (e.g., policy store unavailable).
    async fn check_permission(&self, context: &PolicyEvaluationContext) -> Result<bool, HelixError>;
}

/// Trait for managing the storage and retrieval of policies.
///
/// Implementations of this trait will handle the persistence of [`Policy`](Policy) objects,
/// allowing them to be loaded, queried, and managed. This could be backed by
/// various storage mechanisms (e.g., database, in-memory store, configuration files).
#[async_trait::async_trait]
pub trait PolicyStore {
    /// Retrieves a specific policy by its unique identifier.
    ///
    /// # Arguments
    /// * `policy_id` - The ID of the policy to retrieve.
    ///
    /// # Returns
    /// * `Ok(Some(Policy))` if the policy is found.
    /// * `Ok(None)` if no policy with the given ID exists.
    /// * `Err(HelixError)` if an error occurs during retrieval.
    async fn get_policy(&self, policy_id: &PolicyId) -> Result<Option<Policy>, HelixError>;

    /// Retrieves all policies currently stored.
    ///
    /// # Returns
    /// * `Ok(Vec<Policy>)` containing all policies.
    /// * `Err(HelixError)` if an error occurs during retrieval.
    async fn get_all_policies(&self) -> Result<Vec<Policy>, HelixError>;

    /// Stores (creates or updates) a policy.
    ///
    /// If a policy with the same `policy_id` already exists, it should be updated.
    /// Otherwise, a new policy is created.
    ///
    /// # Arguments
    /// * `policy` - The [`Policy`](Policy) object to store.
    ///
    /// # Returns
    /// * `Ok(())` if the policy was stored successfully.
    /// * `Err(HelixError)` if an error occurs during storage.
    async fn store_policy(&self, policy: &Policy) -> Result<(), HelixError>;

    /// Deletes a policy by its unique identifier.
    ///
    /// # Arguments
    /// * `policy_id` - The ID of the policy to delete.
    ///
    /// # Returns
    /// * `Ok(true)` if the policy was found and deleted.
    /// * `Ok(false)` if no policy with the given ID was found.
    /// * `Err(HelixError)` if an error occurs during deletion.
    async fn delete_policy(&self, policy_id: &PolicyId) -> Result<bool, HelixError>;

    /// Retrieves all policies that are applicable to a given principal, action, and resource.
    ///
    /// This method is crucial for the policy evaluation process. Implementations will
    /// need to handle matching logic, potentially including wildcard support in
    /// policy definitions for actions, resources, and principals.
    ///
    /// # Arguments
    /// * `principal` - The principal identifier.
    /// * `action` - The action identifier.
    /// * `resource` - The resource identifier.
    ///
    /// # Returns
    /// * `Ok(Vec<Policy>)` containing all applicable policies.
    /// * `Err(HelixError)` if an error occurs during retrieval.
    async fn get_applicable_policies(
        &self,
        principal: &str,
        action: &str,
        resource: &str,
    ) -> Result<Vec<Policy>, HelixError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use uuid::Uuid; // For generating unique policy_ids in tests

    // Helper to create unique policy IDs for tests
    fn create_test_policy_id() -> PolicyId {
        Uuid::new_v4().to_string()
    }

    #[test]
    fn test_policy_effect_serialization_deserialization() {
        let allow_effect = PolicyEffect::Allow;
        let serialized_allow = serde_json::to_string(&allow_effect).unwrap();
        assert_eq!(serialized_allow, "\"Allow\"");
        let deserialized_allow: PolicyEffect = serde_json::from_str(&serialized_allow).unwrap();
        assert_eq!(deserialized_allow, PolicyEffect::Allow);

        let deny_effect = PolicyEffect::Deny;
        let serialized_deny = serde_json::to_string(&deny_effect).unwrap();
        assert_eq!(serialized_deny, "\"Deny\"");
        let deserialized_deny: PolicyEffect = serde_json::from_str(&serialized_deny).unwrap();
        assert_eq!(deserialized_deny, PolicyEffect::Deny);
    }

    #[test]
    fn test_policy_serialization_deserialization_basic() {
        let policy = Policy {
            policy_id: create_test_policy_id(),
            effect: PolicyEffect::Allow,
            actions: vec!["agent:run".to_string(), "recipe:execute".to_string()],
            resources: vec!["agent_id:*".to_string()],
            principals: vec!["user:alice".to_string()],
            conditions: None,
        };

        let serialized = serde_json::to_string_pretty(&policy).expect("Failed to serialize Policy");
        let deserialized: Policy = serde_json::from_str(&serialized).expect("Failed to deserialize Policy");

        assert_eq!(policy, deserialized);
    }

    #[test]
    fn test_policy_serialization_deserialization_with_conditions() {
        let policy_id = create_test_policy_id();
        let conditions_json = json!({
            "ip_range": "192.168.1.0/24",
            "time_of_day": {
                "after": "09:00",
                "before": "17:00"
            }
        });

        let policy = Policy {
            policy_id: policy_id.clone(),
            effect: PolicyEffect::Deny,
            actions: vec!["credential:read".to_string()],
            resources: vec!["credential_id:secret_key".to_string()],
            principals: vec!["group:auditors".to_string()],
            conditions: Some(conditions_json.clone()),
        };

        let serialized = serde_json::to_string_pretty(&policy).expect("Failed to serialize Policy");
        let deserialized: Policy = serde_json::from_str(&serialized).expect("Failed to deserialize Policy");

        assert_eq!(policy.policy_id, deserialized.policy_id);
        assert_eq!(policy.effect, deserialized.effect);
        assert_eq!(policy.actions, deserialized.actions);
        assert_eq!(policy.resources, deserialized.resources);
        assert_eq!(policy.principals, deserialized.principals);
        
        // Compare Option<Value> by converting to string for robust comparison
        assert_eq!(
            policy.conditions.map(|v| v.to_string()),
            deserialized.conditions.map(|v| v.to_string())
        );
    }

    #[test]
    fn test_policy_serialization_deserialization_conditions_absent() {
        // Test that `conditions: None` is handled correctly (omitted if skip_serializing_if is used)
        let policy_json_no_conditions = format!(
            r#"{{
                "policy_id": "{}",
                "effect": "Allow",
                "actions": ["action:read"],
                "resources": ["resource:*"],
                "principals": ["principal:any"]
            }}"#,
            create_test_policy_id()
        );

        let deserialized: Policy = serde_json::from_str(&policy_json_no_conditions)
            .expect("Failed to deserialize Policy without conditions");

        assert!(deserialized.conditions.is_none());

        // Serialize it back and check if conditions field is absent
        let reserialized = serde_json::to_string(&deserialized).unwrap();
        assert!(!reserialized.contains("conditions"));
    }

    #[test]
    fn test_policy_evaluation_context_serialization_deserialization_full() {
        let context = PolicyEvaluationContext {
            principal: "user:bob".to_string(),
            action: "file:read".to_string(),
            resource: "file_id:doc.txt".to_string(),
            attributes: Some(json!({"request_id": "12345", "location": "US"})),
        };

        let serialized = serde_json::to_string_pretty(&context).expect("Failed to serialize PolicyEvaluationContext");
        let deserialized: PolicyEvaluationContext = serde_json::from_str(&serialized)
            .expect("Failed to deserialize PolicyEvaluationContext");

        assert_eq!(context, deserialized);
    }

    #[test]
    fn test_policy_evaluation_context_serialization_deserialization_no_attributes() {
        let context = PolicyEvaluationContext {
            principal: "user:charlie".to_string(),
            action: "db:query".to_string(),
            resource: "table:customers".to_string(),
            attributes: None,
        };

        let serialized = serde_json::to_string_pretty(&context).expect("Failed to serialize PolicyEvaluationContext");
        let deserialized: PolicyEvaluationContext = serde_json::from_str(&serialized)
            .expect("Failed to deserialize PolicyEvaluationContext");

        assert_eq!(context, deserialized);
        
        // Check that attributes field is absent in JSON when None
        let reserialized_json: Value = serde_json::from_str(&serialized).unwrap();
        assert!(reserialized_json.get("attributes").is_none());
    }
}
