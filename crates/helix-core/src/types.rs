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


//! Common type definitions used throughout Helix.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique identifier for an Agent.
pub type AgentId = Uuid;

/// Unique identifier for a Recipe.
pub type RecipeId = Uuid;

/// Unique identifier for a Profile (tenant).
pub type ProfileId = Uuid;

/// Unique identifier for a specific event instance.
pub type EventId = Uuid;

/// Unique identifier for a Credential.
pub type CredentialId = Uuid;

/// Unique identifier for a Policy.
pub type PolicyId = String; // Cedar policy IDs are strings

/// Represents the kind or category of an event.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventKind(String);

impl EventKind {
    /// Creates a new EventKind from a string-like type.
    pub fn new(kind: impl Into<String>) -> Self {
        Self(kind.into())
    }
}

impl From<String> for EventKind {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for EventKind {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for EventKind {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Represents a generic resource identifier using UUIDs for uniqueness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId(Uuid);

impl ResourceId {
    /// Creates a new ResourceId from a UUID.
    pub fn new(id: Uuid) -> Self {
        ResourceId(id)
    }

    /// Access the underlying UUID.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Display for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for ResourceId {
    fn from(id: Uuid) -> Self {
        ResourceId(id)
    }
}

impl From<ResourceId> for Uuid {
    fn from(id: ResourceId) -> Self {
        id.0
    }
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_agent_id_type() {
        let agent_id: AgentId = Uuid::new_v4();
        assert_eq!(agent_id.to_string().len(), 36); // UUID string length
    }

    #[test]
    fn test_recipe_id_type() {
        let recipe_id: RecipeId = Uuid::new_v4();
        assert_eq!(recipe_id.to_string().len(), 36);
    }

    #[test]
    fn test_profile_id_type() {
        let profile_id: ProfileId = Uuid::new_v4();
        assert_eq!(profile_id.to_string().len(), 36);
    }

    #[test]
    fn test_event_id_type() {
        let event_id: EventId = Uuid::new_v4();
        assert_eq!(event_id.to_string().len(), 36);
    }

    #[test]
    fn test_credential_id_type() {
        let credential_id: CredentialId = Uuid::new_v4();
        assert_eq!(credential_id.to_string().len(), 36);
    }

    #[test]
    fn test_policy_id_type() {
        let policy_id: PolicyId = "test_policy_123".to_string();
        assert_eq!(policy_id, "test_policy_123");
    }

    #[test]
    fn test_event_kind_creation() {
        let kind1 = EventKind::new("user.created");
        let kind2 = EventKind::new("system.shutdown".to_string());

        assert_eq!(kind1.as_ref(), "user.created");
        assert_eq!(kind2.as_ref(), "system.shutdown");
    }

    #[test]
    fn test_event_kind_from_string() {
        let kind: EventKind = "data.processed".to_string().into();
        assert_eq!(kind.as_ref(), "data.processed");
    }

    #[test]
    fn test_event_kind_from_str() {
        let kind: EventKind = "agent.started".into();
        assert_eq!(kind.as_ref(), "agent.started");
    }

    #[test]
    fn test_event_kind_as_ref() {
        let kind = EventKind::new("test.event");
        let s: &str = kind.as_ref();
        assert_eq!(s, "test.event");
    }

    #[test]
    fn test_event_kind_display() {
        let kind = EventKind::new("display.test");
        assert_eq!(format!("{}", kind), "display.test");
    }

    #[test]
    fn test_event_kind_equality() {
        let kind1 = EventKind::new("same.event");
        let kind2 = EventKind::new("same.event");
        let kind3 = EventKind::new("different.event");

        assert_eq!(kind1, kind2);
        assert_ne!(kind1, kind3);
    }

    #[test]
    fn test_event_kind_clone() {
        let kind1 = EventKind::new("clone.test");
        let kind2 = kind1.clone();

        assert_eq!(kind1, kind2);
    }

    #[test]
    fn test_event_kind_debug() {
        let kind = EventKind::new("debug.test");
        let debug_str = format!("{:?}", kind);

        assert!(debug_str.contains("EventKind"));
        assert!(debug_str.contains("debug.test"));
    }

    #[test]
    fn test_event_kind_serialization() {
        let kind = EventKind::new("serialize.test");

        let serialized = serde_json::to_string(&kind).expect("Failed to serialize");
        let deserialized: EventKind = serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(kind, deserialized);
    }

    #[test]
    fn test_resource_id_creation_and_display() {
        let uuid = Uuid::new_v4();
        let id = ResourceId::new(uuid);
        assert_eq!(id.as_uuid(), uuid);
        assert_eq!(format!("{}", id), uuid.to_string());
    }

    #[test]
    fn test_resource_id_serialization() {
        let uuid = Uuid::new_v4();
        let id = ResourceId::new(uuid);
        let serialized = serde_json::to_string(&id).expect("Failed to serialize");
        let deserialized: ResourceId = serde_json::from_str(&serialized).expect("Failed to deserialize");
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_event_kind_hash() {
        use std::collections::HashMap;

        let mut map = HashMap::new();
        let kind = EventKind::new("hash.test");

        map.insert(kind.clone(), "value");
        assert_eq!(map.get(&kind), Some(&"value"));
    }

    #[test]
    fn test_resource_id_hash() {
        use std::collections::HashMap;

        let mut map = HashMap::new();
        let id = ResourceId::new(Uuid::new_v4());

        map.insert(id, "value");
        assert_eq!(map.get(&id), Some(&"value"));
    }

    #[test]
    fn test_empty_event_kind() {
        let kind = EventKind::new("");
        assert_eq!(kind.as_ref(), "");
    }

    #[test]
    fn test_special_characters_event_kind() {
        let kind = EventKind::new("event.with-special_chars@123");
        assert_eq!(kind.as_ref(), "event.with-special_chars@123");
    }

    #[test]
    fn test_unicode_event_kind() {
        let kind = EventKind::new("événement.créé");
        assert_eq!(kind.as_ref(), "événement.créé");
    }
}
