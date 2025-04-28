// crates/helix-core/src/credential.rs

use crate::types::{CredentialId, ProfileId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::HelixError;
use async_trait::async_trait;

/// Represents a stored credential for accessing external services.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Credential {
    /// Unique identifier for the credential.
    pub id: CredentialId,
    /// The ID of the profile (tenant) this credential belongs to.
    pub profile_id: ProfileId,
    /// User-defined name for the credential (e.g., "My GitHub Token").
    pub name: String,
    /// The type of credential (e.g., "api_key", "oauth2", "aws_sts").
    /// Determines how the `data` field should be interpreted and used.
    pub kind: String,
    /// The actual credential data, **MUST be stored encrypted**.
    /// The format depends on the `kind`.
    // SECURITY: This data must be encrypted at rest using a strong
    //           encryption mechanism (e.g., AES-GCM with a profile-specific key
    //           or managed key service). Decryption should only happen just-in-time
    //           when needed by the CredentialProvider.
    pub data: String, // Assuming encrypted blob (e.g., base64 encoded ciphertext)
    /// Timestamp when the credential was created.
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
    /// Timestamp when the credential was last updated.
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
    // TODO: Consider adding expiry information?
    // TODO: Consider adding metadata/tags?
}

impl Credential {
    /// Placeholder for a potential constructor.
    pub fn new(
        id: CredentialId,
        profile_id: ProfileId,
        name: String,
        kind: String,
        encrypted_data: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            profile_id,
            name,
            kind,
            data: encrypted_data, // Assume data is already encrypted
            created_at: now,
            updated_at: now,
        }
    }

    // TODO: Add methods for updating (requires re-encryption logic)?
}

// --- Placeholder Service Traits --- 
// Define interfaces needed by CredentialProvider implementations.

/// Placeholder trait for fetching raw Credential objects.
#[async_trait]
pub trait CredentialStore: Send + Sync {
    /// Retrieves a credential by its ID for a specific profile.
    async fn get_credential_by_id(
        &self,
        profile_id: &ProfileId,
        credential_id: &CredentialId,
    ) -> Result<Option<Credential>, HelixError>; // Assuming store errors map to HelixError
}

/// Placeholder trait for decrypting credential data.
#[async_trait]
pub trait EncryptionService: Send + Sync {
    /// Decrypts data associated with a specific profile.
    /// 
    /// SECURITY: Implementation MUST ensure appropriate key management.
    async fn decrypt(
        &self,
        profile_id: &ProfileId,
        encrypted_data: &str, // Matches Credential.data field type
    ) -> Result<String, HelixError>; // Returns decrypted string, maps errors
}

// --- Mock Implementations (for testing core components if needed) ---

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use std::collections::HashMap;
//     use std::sync::Mutex;

//     struct MockCredentialStore {
//         creds: Mutex<HashMap<(ProfileId, CredentialId), Credential>>,
//     }
//     // ... impl CredentialStore ...

//     struct MockEncryptionService; 
//     // ... impl EncryptionService (e.g., simple Base64 echo) ...
// }
