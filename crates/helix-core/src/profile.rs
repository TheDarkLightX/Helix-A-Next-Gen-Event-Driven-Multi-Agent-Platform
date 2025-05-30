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
    /// Creates a new profile with the given parameters
    pub fn new(id: ProfileId, name: Option<String>, kind: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            name,
            kind,
            created_at: now,
            updated_at: now,
            status: default_profile_status(),
        }
    }

    /// Updates the profile name
    pub fn update_name(&mut self, name: Option<String>) {
        self.name = name;
        self.updated_at = Utc::now();
    }

    /// Updates the profile status
    pub fn update_status(&mut self, status: String) {
        self.status = status;
        self.updated_at = Utc::now();
    }

    /// Checks if the profile is active
    pub fn is_active(&self) -> bool {
        self.status == "active"
    }

    /// Checks if the profile is suspended
    pub fn is_suspended(&self) -> bool {
        self.status == "suspended"
    }

    /// Validates the profile data
    pub fn validate(&self) -> Result<(), String> {
        if self.kind.is_empty() || self.kind.trim().is_empty() {
            return Err("Profile kind cannot be empty".to_string());
        }

        if let Some(ref name) = self.name {
            if name.is_empty() || name.trim().is_empty() {
                return Err("Profile name cannot be empty if provided".to_string());
            }
            if name.len() > 255 {
                return Err("Profile name cannot exceed 255 characters".to_string());
            }
        }

        if !matches!(self.status.as_str(), "active" | "suspended" | "deleted") {
            return Err("Invalid profile status".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn create_test_profile() -> Profile {
        Profile::new(
            Uuid::new_v4(),
            Some("Test Profile".to_string()),
            "user".to_string(),
        )
    }

    #[test]
    fn test_profile_creation() {
        let profile_id = Uuid::new_v4();
        let profile_name = Some("Test User".to_string());
        let profile_kind = "user".to_string();

        let profile = Profile::new(profile_id, profile_name.clone(), profile_kind.clone());

        assert_eq!(profile.id, profile_id);
        assert_eq!(profile.name, profile_name);
        assert_eq!(profile.kind, profile_kind);
        assert_eq!(profile.status, "active");
        assert!(profile.created_at <= Utc::now());
        assert!(profile.updated_at <= Utc::now());
        assert_eq!(profile.created_at, profile.updated_at);
    }

    #[test]
    fn test_profile_creation_without_name() {
        let profile_id = Uuid::new_v4();
        let profile_kind = "organization".to_string();

        let profile = Profile::new(profile_id, None, profile_kind.clone());

        assert_eq!(profile.id, profile_id);
        assert_eq!(profile.name, None);
        assert_eq!(profile.kind, profile_kind);
        assert_eq!(profile.status, "active");
    }

    #[test]
    fn test_update_name() {
        let mut profile = create_test_profile();
        let original_updated_at = profile.updated_at;

        // Small delay to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(1));

        let new_name = Some("Updated Name".to_string());
        profile.update_name(new_name.clone());

        assert_eq!(profile.name, new_name);
        assert!(profile.updated_at > original_updated_at);
    }

    #[test]
    fn test_update_name_to_none() {
        let mut profile = create_test_profile();

        profile.update_name(None);

        assert_eq!(profile.name, None);
    }

    #[test]
    fn test_update_status() {
        let mut profile = create_test_profile();
        let original_updated_at = profile.updated_at;

        // Small delay to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(1));

        profile.update_status("suspended".to_string());

        assert_eq!(profile.status, "suspended");
        assert!(profile.updated_at > original_updated_at);
    }

    #[test]
    fn test_is_active() {
        let mut profile = create_test_profile();

        assert!(profile.is_active());

        profile.update_status("suspended".to_string());
        assert!(!profile.is_active());

        profile.update_status("active".to_string());
        assert!(profile.is_active());
    }

    #[test]
    fn test_is_suspended() {
        let mut profile = create_test_profile();

        assert!(!profile.is_suspended());

        profile.update_status("suspended".to_string());
        assert!(profile.is_suspended());

        profile.update_status("active".to_string());
        assert!(!profile.is_suspended());
    }

    #[test]
    fn test_validate_valid_profile() {
        let profile = create_test_profile();
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_kind() {
        let mut profile = create_test_profile();
        profile.kind = String::new();

        let result = profile.validate();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Profile kind cannot be empty");
    }

    #[test]
    fn test_validate_empty_name() {
        let mut profile = create_test_profile();
        profile.name = Some(String::new());

        let result = profile.validate();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Profile name cannot be empty if provided");
    }

    #[test]
    fn test_validate_long_name() {
        let mut profile = create_test_profile();
        profile.name = Some("a".repeat(256));

        let result = profile.validate();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Profile name cannot exceed 255 characters");
    }

    #[test]
    fn test_validate_invalid_status() {
        let mut profile = create_test_profile();
        profile.status = "invalid_status".to_string();

        let result = profile.validate();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Invalid profile status");
    }

    #[test]
    fn test_validate_valid_statuses() {
        let mut profile = create_test_profile();

        for status in &["active", "suspended", "deleted"] {
            profile.status = status.to_string();
            assert!(profile.validate().is_ok(), "Status '{}' should be valid", status);
        }
    }

    #[test]
    fn test_serialization() {
        let profile = create_test_profile();

        let serialized = serde_json::to_string(&profile).expect("Failed to serialize profile");
        let deserialized: Profile = serde_json::from_str(&serialized).expect("Failed to deserialize profile");

        assert_eq!(profile, deserialized);
    }

    #[test]
    fn test_default_values_in_deserialization() {
        let json = r#"{"id":"550e8400-e29b-41d4-a716-446655440000","name":"Test","kind":"user"}"#;
        let profile: Profile = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(profile.status, "active");
        // created_at and updated_at should be set to current time due to default
        assert!(profile.created_at <= Utc::now());
        assert!(profile.updated_at <= Utc::now());
    }

    #[test]
    fn test_profile_equality() {
        let profile1 = create_test_profile();
        let mut profile2 = profile1.clone();

        assert_eq!(profile1, profile2);

        profile2.update_name(Some("Different Name".to_string()));
        assert_ne!(profile1, profile2);
    }

    #[test]
    fn test_profile_debug() {
        let profile = create_test_profile();
        let debug_str = format!("{:?}", profile);

        assert!(debug_str.contains("Profile"));
        assert!(debug_str.contains(&profile.id.to_string()));
        assert!(debug_str.contains("Test Profile"));
        assert!(debug_str.contains("user"));
    }

    #[test]
    fn test_profile_with_unicode_name() {
        let profile_id = Uuid::new_v4();
        let unicode_name = Some("用户测试 🚀 Тест".to_string());
        let profile = Profile::new(profile_id, unicode_name.clone(), "user".to_string());

        assert_eq!(profile.name, unicode_name);

        // Test serialization with unicode
        let serialized = serde_json::to_string(&profile).unwrap();
        let deserialized: Profile = serde_json::from_str(&serialized).unwrap();
        assert_eq!(profile.name, deserialized.name);
    }

    #[test]
    fn test_profile_boundary_values() {
        // Test with minimum UUID
        let min_uuid = Uuid::from_bytes([0; 16]);
        let profile = Profile::new(min_uuid, Some("Min UUID".to_string()), "test".to_string());
        assert_eq!(profile.id, min_uuid);

        // Test with maximum UUID
        let max_uuid = Uuid::from_bytes([255; 16]);
        let profile = Profile::new(max_uuid, Some("Max UUID".to_string()), "test".to_string());
        assert_eq!(profile.id, max_uuid);
    }

    #[test]
    fn test_profile_name_edge_cases() {
        let profile_id = Uuid::new_v4();

        // Test with exactly 255 characters (should be valid)
        let max_valid_name = Some("a".repeat(255));
        let mut profile = Profile::new(profile_id, max_valid_name.clone(), "user".to_string());
        assert!(profile.validate().is_ok());
        assert_eq!(profile.name, max_valid_name);

        // Test with special characters
        let special_name = Some(r#"Name with "quotes" and \backslashes"#.to_string());
        profile.update_name(special_name.clone());
        assert_eq!(profile.name, special_name);

        // Test serialization with special characters
        let serialized = serde_json::to_string(&profile).unwrap();
        let deserialized: Profile = serde_json::from_str(&serialized).unwrap();
        assert_eq!(profile.name, deserialized.name);
    }

    #[test]
    fn test_profile_kind_variations() {
        let profile_id = Uuid::new_v4();
        let test_kinds = vec![
            "user",
            "organization",
            "service",
            "admin",
            "guest",
            "system",
            "api",
            "bot",
        ];

        for kind in test_kinds {
            let profile = Profile::new(profile_id, Some("Test".to_string()), kind.to_string());
            assert_eq!(profile.kind, kind);
            assert!(profile.validate().is_ok());
        }
    }

    #[test]
    fn test_profile_status_transitions() {
        let mut profile = create_test_profile();

        // Test all valid status transitions
        let status_transitions = vec![
            ("active", "suspended"),
            ("suspended", "active"),
            ("active", "deleted"),
            ("suspended", "deleted"),
        ];

        for (from_status, to_status) in status_transitions {
            profile.status = from_status.to_string();
            assert!(profile.validate().is_ok());

            profile.update_status(to_status.to_string());
            assert_eq!(profile.status, to_status);
            assert!(profile.validate().is_ok());
        }
    }

    #[test]
    fn test_profile_timestamp_precision() {
        let profile1 = create_test_profile();

        // Small delay to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(2));

        let profile2 = create_test_profile();

        // Timestamps should be different
        assert!(profile2.created_at > profile1.created_at);
        assert!(profile2.updated_at > profile1.updated_at);
    }

    #[test]
    fn test_profile_update_preserves_created_at() {
        let mut profile = create_test_profile();
        let original_created_at = profile.created_at;

        std::thread::sleep(std::time::Duration::from_millis(1));

        profile.update_name(Some("New Name".to_string()));
        profile.update_status("suspended".to_string());

        // created_at should remain unchanged
        assert_eq!(profile.created_at, original_created_at);
        // updated_at should be newer
        assert!(profile.updated_at > original_created_at);
    }

    #[test]
    fn test_profile_json_edge_cases() {
        // Test deserialization with missing optional fields
        let minimal_json = r#"{"id":"550e8400-e29b-41d4-a716-446655440000","kind":"user"}"#;
        let profile: Profile = serde_json::from_str(minimal_json).unwrap();

        assert!(profile.name.is_none());
        assert_eq!(profile.kind, "user");
        assert_eq!(profile.status, "active"); // Default value

        // Test with null name
        let null_name_json = r#"{"id":"550e8400-e29b-41d4-a716-446655440000","name":null,"kind":"user"}"#;
        let profile: Profile = serde_json::from_str(null_name_json).unwrap();
        assert!(profile.name.is_none());
    }

    #[test]
    fn test_profile_validation_comprehensive() {
        let mut profile = create_test_profile();

        // Test whitespace-only kind
        profile.kind = "   ".to_string();
        assert!(profile.validate().is_err());

        // Test whitespace-only name
        profile.kind = "user".to_string();
        profile.name = Some("   ".to_string());
        assert!(profile.validate().is_err());

        // Test case sensitivity in status
        profile.name = Some("Valid Name".to_string());
        profile.status = "ACTIVE".to_string(); // Should be lowercase
        assert!(profile.validate().is_err());

        profile.status = "Active".to_string(); // Should be lowercase
        assert!(profile.validate().is_err());
    }

    #[test]
    fn test_profile_clone_independence() {
        let original = create_test_profile();
        let mut cloned = original.clone();

        // Modify the clone
        cloned.update_name(Some("Cloned Profile".to_string()));
        cloned.update_status("suspended".to_string());

        // Original should be unchanged
        assert_eq!(original.name, Some("Test Profile".to_string()));
        assert_eq!(original.status, "active");

        // Clone should be modified
        assert_eq!(cloned.name, Some("Cloned Profile".to_string()));
        assert_eq!(cloned.status, "suspended");
    }

    #[test]
    fn test_profile_large_data_handling() {
        let profile_id = Uuid::new_v4();

        // Test with very long kind (should still validate if reasonable)
        let long_kind = "a".repeat(100);
        let profile = Profile::new(profile_id, Some("Test".to_string()), long_kind.clone());
        assert_eq!(profile.kind, long_kind);

        // Test serialization with large data
        let serialized = serde_json::to_string(&profile).unwrap();
        assert!(serialized.len() > 100);

        let deserialized: Profile = serde_json::from_str(&serialized).unwrap();
        assert_eq!(profile.kind, deserialized.kind);
    }

    #[test]
    fn test_profile_concurrent_updates() {
        use std::sync::{Arc, Mutex};
        use std::thread;

        let profile = Arc::new(Mutex::new(create_test_profile()));
        let mut handles = vec![];

        // Simulate concurrent updates
        for i in 0..5 {
            let profile_clone = Arc::clone(&profile);
            let handle = thread::spawn(move || {
                let mut p = profile_clone.lock().unwrap();
                p.update_name(Some(format!("Updated Name {}", i)));
                p.update_status("suspended".to_string());
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        let final_profile = profile.lock().unwrap();
        assert!(final_profile.name.as_ref().unwrap().starts_with("Updated Name"));
        assert_eq!(final_profile.status, "suspended");
    }

    #[test]
    fn test_profile_memory_efficiency() {
        // Test that Profile struct doesn't use excessive memory
        let profile = create_test_profile();
        let size = std::mem::size_of_val(&profile);

        // Profile should be reasonably sized (this is a rough check)
        assert!(size < 1000, "Profile struct is too large: {} bytes", size);
    }

    #[test]
    fn test_profile_hash_consistency() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let profile1 = create_test_profile();
        let profile2 = profile1.clone();

        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();

        // Note: Profile doesn't implement Hash, but we can test that equal profiles
        // would have the same hash if they did
        profile1.id.hash(&mut hasher1);
        profile2.id.hash(&mut hasher2);

        assert_eq!(hasher1.finish(), hasher2.finish());
    }

    #[test]
    fn test_profile_error_messages() {
        let mut profile = create_test_profile();

        // Test specific error messages
        profile.kind = "".to_string();
        match profile.validate() {
            Err(msg) => assert_eq!(msg, "Profile kind cannot be empty"),
            Ok(_) => panic!("Expected validation error"),
        }

        profile.kind = "user".to_string();
        profile.name = Some("".to_string());
        match profile.validate() {
            Err(msg) => assert_eq!(msg, "Profile name cannot be empty if provided"),
            Ok(_) => panic!("Expected validation error"),
        }

        profile.name = Some("a".repeat(256));
        match profile.validate() {
            Err(msg) => assert_eq!(msg, "Profile name cannot exceed 255 characters"),
            Ok(_) => panic!("Expected validation error"),
        }

        profile.name = Some("Valid Name".to_string());
        profile.status = "invalid".to_string();
        match profile.validate() {
            Err(msg) => assert_eq!(msg, "Invalid profile status"),
            Ok(_) => panic!("Expected validation error"),
        }
    }
}
