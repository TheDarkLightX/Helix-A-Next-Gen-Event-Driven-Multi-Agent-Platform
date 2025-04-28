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

/// Provides access to decrypted credentials for agent execution contexts.
///
/// This trait abstracts the underlying storage (`CredentialStore`) and 
/// decryption (`EncryptionService`) mechanisms.
#[async_trait]
pub trait CredentialProvider: Send + Sync {
    /// Retrieves and decrypts the data for a specific credential within a profile.
    ///
    /// Implementations should fetch the encrypted `Credential` using a `CredentialStore`
    /// and then decrypt its `data` field using an `EncryptionService`.
    async fn get_decrypted_data(
        &self,
        profile_id: &ProfileId,
        credential_id: &CredentialId,
    ) -> Result<String, HelixError>; // Returns decrypted data as String
}

// --- Mock Implementations (for testing core components if needed) ---

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use uuid::Uuid;
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;

    // Mock CredentialStore Implementation
    struct MockCredentialStore {
        creds: Mutex<HashMap<(ProfileId, CredentialId), Credential>>,
    }

    impl MockCredentialStore {
        fn new() -> Self {
            Self { creds: Mutex::new(HashMap::new()) }
        }
        fn add_cred(&self, cred: Credential) {
            let mut store = self.creds.lock().unwrap();
            store.insert((cred.profile_id, cred.id), cred);
        }
    }

    #[async_trait]
    impl CredentialStore for MockCredentialStore {
        async fn get_credential_by_id(
            &self,
            profile_id: &ProfileId,
            credential_id: &CredentialId,
        ) -> Result<Option<Credential>, HelixError> {
            let store = self.creds.lock().unwrap();
            Ok(store.get(&(*profile_id, *credential_id)).cloned())
        }
    }

    // Mock EncryptionService Implementation (Simple Base64 echo)
    struct MockEncryptionService;

    #[async_trait]
    impl EncryptionService for MockEncryptionService {
        async fn decrypt(
            &self,
            _profile_id: &ProfileId,
            encrypted_data: &str,
        ) -> Result<String, HelixError> {
            // Super simple mock: assume base64 encoded, just decode it
            // In reality, this would involve keys and proper crypto.
            STANDARD.decode(encrypted_data)
                .map_err(|e| HelixError::EncryptionError(format!("Mock decrypt failed: {}", e)))
                .and_then(|bytes| {
                    String::from_utf8(bytes).map_err(|e| {
                        HelixError::EncryptionError(format!("Mock decrypt UTF8 failed: {}", e))
                    })
                })
        }
    }

    // TODO: Add mock implementation for CredentialProvider if needed for other tests.

    #[tokio::test]
    async fn test_placeholder_credential_provider_logic() {
        // This test doesn't test the provider trait directly yet, 
        // but sets up mocks needed for a potential provider implementation test.

        let profile_id = Uuid::new_v4();
        let cred_id = Uuid::new_v4();
        let secret_data = "my_secret_api_key";
        let encrypted_data = STANDARD.encode(secret_data);

        let cred = Credential::new(
            cred_id,
            profile_id,
            "Test Key".to_string(),
            "api_key".to_string(),
            encrypted_data.clone(),
        );

        let store = MockCredentialStore::new();
        store.add_cred(cred.clone());

        let service = MockEncryptionService;

        // --- Test CredentialStore --- 
        let fetched_cred = store
            .get_credential_by_id(&profile_id, &cred_id)
            .await
            .unwrap();
        assert_eq!(fetched_cred, Some(cred));

        // --- Test EncryptionService --- 
        let decrypted = service
            .decrypt(&profile_id, &encrypted_data)
            .await
            .unwrap();
        assert_eq!(decrypted, secret_data);

        // Next step would be to implement a ConcreteCredentialProvider
        // using MockCredentialStore and MockEncryptionService, then test it here.
        // Example:
        // let provider = ConcreteCredentialProvider::new(Arc::new(store), Arc::new(service));
        // let decrypted_via_provider = provider.get_decrypted_data(&profile_id, &cred_id).await.unwrap();
        // assert_eq!(decrypted_via_provider, secret_data);
        assert!(true); // Placeholder assertion
    }
}
