#![warn(missing_docs)]

//! Defines the Policy model, representing a Cedar policy document.

use crate::types::{PolicyId, ProfileId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents a Cedar policy document used for authorization.
///
/// Spec: "Cedar doc controlling data/agent access."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Policy {
    /// Unique identifier for this policy (typically user-defined, following Cedar naming conventions).
    pub id: PolicyId,
    /// ID of the profile (tenant) this policy belongs to.
    pub profile_id: ProfileId,
    /// Optional friendly name for the policy.
    pub name: Option<String>,
    /// Description of the policy's purpose.
    pub description: Option<String>,
    /// The actual content of the Cedar policy document.
    pub content: String,
    /// Timestamp when the policy was created.
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
    /// Timestamp when the policy was last updated.
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
    /// Version number for tracking changes.
    #[serde(default)]
    pub version: u32,
}

impl Policy {
    // TODO: Add validation methods (e.g., using cedar-policy parser)?
}
