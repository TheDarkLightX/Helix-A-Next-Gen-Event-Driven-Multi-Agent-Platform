#![warn(missing_docs)]

//! Defines the Profile model, representing a multi-tenant namespace.

use crate::types::ProfileId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents a multi-tenant namespace (e.g., a user account or an organization).
///
/// Profiles provide isolation for resources like Agents, Recipes, Credentials, and Policies.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Profile {
    /// Unique identifier for this profile.
    pub id: ProfileId,
    /// Optional friendly name for the profile (e.g., username, org name).
    pub name: Option<String>,
    /// Type of profile (e.g., "user", "organization").
    pub kind: String,
    /// Timestamp when the profile was created.
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
    /// Timestamp when the profile was last updated.
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
    /// Status of the profile (e.g., "active", "suspended").
    #[serde(default = "default_profile_status")]
    pub status: String,
    // TODO: Add profile-specific settings or metadata?
}

fn default_profile_status() -> String {
    "active".to_string()
}

impl Profile {
    // TODO: Add methods related to profile management?
}
