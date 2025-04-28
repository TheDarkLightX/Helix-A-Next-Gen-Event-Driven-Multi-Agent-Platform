//! Common type definitions used throughout Helix.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::fmt;

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

// TODO: Consider using UUIDs directly or a more robust ID scheme.

/// Represents a generic resource identifier (e.g., for Agents, Recipes).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId(String);

impl ResourceId {
    /// Creates a new ResourceId from a string-like type.
    pub fn new(kind: impl Into<String>) -> Self {
        ResourceId(kind.into())
    }
}

impl fmt::Display for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
