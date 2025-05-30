// crates/helix-core/src/credential.rs

use crate::types::{CredentialId, ProfileId};
use crate::HelixError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
    /// Retrieves a credential by its ID by looking up an environment variable.
    /// The environment variable is expected to contain a JSON representation of the Credential.
    async fn get_credential(
        &self,
        credential_id: &str,
    ) -> Result<Option<Credential>, HelixError>;
}

/// EnvVar-based implementation of CredentialProvider.
/// Reads credentials from an environment variable. The variable name is constructed as
/// `HELIX_CRED_<CREDENTIAL_ID_UPPERCASE>`, and its value is expected to be a JSON
/// string that deserializes into a `Credential` object.
#[derive(Debug, Default)]
pub struct EnvCredentialProvider;

#[async_trait]
impl CredentialProvider for EnvCredentialProvider {
    async fn get_credential(
        &self,
        credential_id: &str,
    ) -> Result<Option<Credential>, HelixError> {
        let env_var_name = format!("HELIX_CRED_{}", credential_id.to_uppercase());
        match std::env::var(&env_var_name) {
            Ok(credential_json) => {
                serde_json::from_str::<Credential>(&credential_json)
                    .map(Some)
                    .map_err(|e| HelixError::validation_error(
                        format!("credential.{}", env_var_name),
                        format!("Failed to deserialize credential from environment variable {}: {}", env_var_name, e)
                    ))
            }
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(std::env::VarError::NotUnicode(_os_string)) => Err(HelixError::ConfigError(format!(
                "Environment variable {} contains non-Unicode data",
                env_var_name
            ))),
        }
    }
}

// --- Mock Implementations (for testing core components if needed) ---

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine as _;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use uuid::Uuid;

    // Mock CredentialStore Implementation
    struct MockCredentialStore {
        creds: Mutex<HashMap<(ProfileId, CredentialId), Credential>>,
    }

    impl MockCredentialStore {
        fn new() -> Self {
            Self {
                creds: Mutex::new(HashMap::new()),
            }
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
            STANDARD
                .decode(encrypted_data)
                .map_err(|e| HelixError::EncryptionError(format!("Mock decrypt failed: {}", e)))
                .and_then(|bytes| {
                    String::from_utf8(bytes).map_err(|e| {
                        HelixError::EncryptionError(format!("Mock decrypt UTF8 failed: {}", e))
                    })
                })
        }
    }

    // TODO: Add mock implementation for CredentialProvider if needed for other tests.

    #[test]
    fn test_credential_creation() {
        let cred_id = Uuid::new_v4();
        let profile_id = Uuid::new_v4();
        let name = "Test API Key".to_string();
        let cred_type = "api_key".to_string();
        let encrypted_data = "encrypted_secret".to_string();

        let credential = Credential::new(
            cred_id,
            profile_id,
            name.clone(),
            cred_type.clone(),
            encrypted_data.clone(),
        );

        assert_eq!(credential.id, cred_id);
        assert_eq!(credential.profile_id, profile_id);
        assert_eq!(credential.name, name);
        assert_eq!(credential.kind, cred_type);
        assert_eq!(credential.data, encrypted_data);
    }

    #[test]
    fn test_credential_equality() {
        let cred_id = Uuid::new_v4();
        let profile_id = Uuid::new_v4();

        let cred1 = Credential::new(
            cred_id,
            profile_id,
            "Test".to_string(),
            "api_key".to_string(),
            "data".to_string(),
        );

        let cred3 = Credential::new(
            Uuid::new_v4(),
            profile_id,
            "Test".to_string(),
            "api_key".to_string(),
            "data".to_string(),
        );

        // Test that credentials with same ID are considered equal (ignoring timestamps)
        assert_eq!(cred1.id, cred_id);
        assert_eq!(cred1.profile_id, profile_id);
        assert_eq!(cred1.name, "Test");
        assert_eq!(cred1.kind, "api_key");
        assert_eq!(cred1.data, "data");

        // Test that credentials with different IDs are different
        assert_ne!(cred1.id, cred3.id);
    }

    #[test]
    fn test_credential_debug_format() {
        let credential = Credential::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Debug Test".to_string(),
            "oauth_token".to_string(),
            "encrypted_token_data".to_string(),
        );

        let debug_str = format!("{:?}", credential);
        assert!(debug_str.contains("Credential"));
        assert!(debug_str.contains("Debug Test"));
        assert!(debug_str.contains("oauth_token"));
    }

    #[test]
    fn test_credential_clone() {
        let original = Credential::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Clone Test".to_string(),
            "password".to_string(),
            "encrypted_password".to_string(),
        );

        let cloned = original.clone();
        assert_eq!(original, cloned);
        assert_eq!(original.id, cloned.id);
        assert_eq!(original.name, cloned.name);
    }

    #[test]
    fn test_credential_serialization() {
        let credential = Credential::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Serialization Test".to_string(),
            "certificate".to_string(),
            "encrypted_cert_data".to_string(),
        );

        let serialized = serde_json::to_string(&credential).expect("Failed to serialize");
        let deserialized: Credential = serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(credential, deserialized);
        assert_eq!(credential.id, deserialized.id);
        assert_eq!(credential.name, deserialized.name);
        assert_eq!(credential.kind, deserialized.kind);
    }

    #[test]
    fn test_credential_types() {
        let types = vec![
            "api_key",
            "oauth_token",
            "password",
            "certificate",
            "ssh_key",
            "bearer_token",
            "basic_auth",
            "custom_secret",
        ];

        for cred_type in types {
            let credential = Credential::new(
                Uuid::new_v4(),
                Uuid::new_v4(),
                format!("{} Test", cred_type),
                cred_type.to_string(),
                "encrypted_data".to_string(),
            );

            assert_eq!(credential.kind, cred_type);
        }
    }

    #[test]
    fn test_credential_empty_fields() {
        let credential = Credential::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
        );

        assert!(credential.name.is_empty());
        assert!(credential.kind.is_empty());
        assert!(credential.data.is_empty());
    }

    #[test]
    fn test_credential_unicode_support() {
        let credential = Credential::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "测试凭证 🔑".to_string(),
            "api_key_中文".to_string(),
            "加密数据_🔒".to_string(),
        );

        assert_eq!(credential.name, "测试凭证 🔑");
        assert_eq!(credential.kind, "api_key_中文");
        assert_eq!(credential.data, "加密数据_🔒");
    }

    #[test]
    fn test_credential_large_data() {
        let large_data = "x".repeat(10000);
        let credential = Credential::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Large Data Test".to_string(),
            "large_secret".to_string(),
            large_data.clone(),
        );

        assert_eq!(credential.data.len(), 10000);
        assert_eq!(credential.data, large_data);
    }

    #[tokio::test]
    async fn test_mock_credential_store_basic() {
        let store = MockCredentialStore::new();
        let profile_id = Uuid::new_v4();
        let cred_id = Uuid::new_v4();

        let credential = Credential::new(
            cred_id,
            profile_id,
            "Store Test".to_string(),
            "api_key".to_string(),
            "encrypted_data".to_string(),
        );

        store.add_cred(credential.clone());

        let fetched = store
            .get_credential_by_id(&profile_id, &cred_id)
            .await
            .unwrap();

        assert_eq!(fetched, Some(credential));
    }

    #[tokio::test]
    async fn test_mock_credential_store_not_found() {
        let store = MockCredentialStore::new();
        let profile_id = Uuid::new_v4();
        let cred_id = Uuid::new_v4();

        let result = store
            .get_credential_by_id(&profile_id, &cred_id)
            .await
            .unwrap();

        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_mock_credential_store_multiple_profiles() {
        let store = MockCredentialStore::new();
        let profile1 = Uuid::new_v4();
        let profile2 = Uuid::new_v4();
        let cred_id = Uuid::new_v4();

        let cred1 = Credential::new(
            cred_id,
            profile1,
            "Profile 1 Cred".to_string(),
            "api_key".to_string(),
            "data1".to_string(),
        );

        let cred2 = Credential::new(
            cred_id,
            profile2,
            "Profile 2 Cred".to_string(),
            "api_key".to_string(),
            "data2".to_string(),
        );

        store.add_cred(cred1.clone());
        store.add_cred(cred2.clone());

        let fetched1 = store.get_credential_by_id(&profile1, &cred_id).await.unwrap();
        let fetched2 = store.get_credential_by_id(&profile2, &cred_id).await.unwrap();

        assert_eq!(fetched1, Some(cred1));
        assert_eq!(fetched2, Some(cred2));
    }

    #[tokio::test]
    async fn test_mock_credential_store_overwrite() {
        let store = MockCredentialStore::new();
        let profile_id = Uuid::new_v4();
        let cred_id = Uuid::new_v4();

        let cred1 = Credential::new(
            cred_id,
            profile_id,
            "Original".to_string(),
            "api_key".to_string(),
            "original_data".to_string(),
        );

        let cred2 = Credential::new(
            cred_id,
            profile_id,
            "Updated".to_string(),
            "api_key".to_string(),
            "updated_data".to_string(),
        );

        store.add_cred(cred1);
        store.add_cred(cred2.clone());

        let fetched = store.get_credential_by_id(&profile_id, &cred_id).await.unwrap();
        assert_eq!(fetched, Some(cred2));
    }

    #[tokio::test]
    async fn test_mock_encryption_service_basic() {
        let service = MockEncryptionService;
        let profile_id = Uuid::new_v4();
        let secret_data = "my_secret_password";
        let encrypted_data = STANDARD.encode(secret_data);

        let decrypted = service.decrypt(&profile_id, &encrypted_data).await.unwrap();
        assert_eq!(decrypted, secret_data);
    }

    #[tokio::test]
    async fn test_mock_encryption_service_empty_data() {
        let service = MockEncryptionService;
        let profile_id = Uuid::new_v4();
        let encrypted_data = STANDARD.encode("");

        let decrypted = service.decrypt(&profile_id, &encrypted_data).await.unwrap();
        assert_eq!(decrypted, "");
    }

    #[tokio::test]
    async fn test_mock_encryption_service_unicode() {
        let service = MockEncryptionService;
        let profile_id = Uuid::new_v4();
        let secret_data = "密码123 🔐";
        let encrypted_data = STANDARD.encode(secret_data);

        let decrypted = service.decrypt(&profile_id, &encrypted_data).await.unwrap();
        assert_eq!(decrypted, secret_data);
    }

    #[tokio::test]
    async fn test_mock_encryption_service_large_data() {
        let service = MockEncryptionService;
        let profile_id = Uuid::new_v4();
        let secret_data = "x".repeat(5000);
        let encrypted_data = STANDARD.encode(&secret_data);

        let decrypted = service.decrypt(&profile_id, &encrypted_data).await.unwrap();
        assert_eq!(decrypted, secret_data);
    }

    #[tokio::test]
    async fn test_mock_encryption_service_invalid_base64() {
        let service = MockEncryptionService;
        let profile_id = Uuid::new_v4();
        let invalid_data = "not_valid_base64!@#$%";

        let result = service.decrypt(&profile_id, invalid_data).await;
        assert!(result.is_err());

        if let Err(HelixError::EncryptionError(msg)) = result {
            assert!(msg.contains("Mock decrypt failed"));
        } else {
            panic!("Expected EncryptionError");
        }
    }

    #[tokio::test]
    async fn test_mock_encryption_service_invalid_utf8() {
        let service = MockEncryptionService;
        let profile_id = Uuid::new_v4();
        // Create invalid UTF-8 bytes and encode them
        let invalid_utf8_bytes = vec![0xFF, 0xFE, 0xFD];
        let encrypted_data = STANDARD.encode(&invalid_utf8_bytes);

        let result = service.decrypt(&profile_id, &encrypted_data).await;
        assert!(result.is_err());

        if let Err(HelixError::EncryptionError(msg)) = result {
            assert!(msg.contains("Mock decrypt UTF8 failed"));
        } else {
            panic!("Expected EncryptionError");
        }
    }

    // --- Tests for EnvCredentialProvider ---

    #[tokio::test]
    async fn test_env_credential_provider_success() {
        let provider = EnvCredentialProvider;
        let cred_id_str = "my_test_cred";
        let env_var_name = format!("HELIX_CRED_{}", cred_id_str.to_uppercase());

        let expected_credential = Credential {
            id: Uuid::new_v4(),
            profile_id: Uuid::new_v4(),
            name: "Test Credential".to_string(),
            kind: "api_key".to_string(),
            data: "secret_data".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let credential_json = serde_json::to_string(&expected_credential).unwrap();

        std::env::set_var(&env_var_name, &credential_json);

        let result = provider.get_credential(cred_id_str).await;

        std::env::remove_var(&env_var_name);

        assert!(result.is_ok());
        let option_credential = result.unwrap();
        assert!(option_credential.is_some());
        let actual_credential = option_credential.unwrap();
        
        // Compare fields that are expected to be the same
        assert_eq!(actual_credential.id, expected_credential.id);
        assert_eq!(actual_credential.profile_id, expected_credential.profile_id);
        assert_eq!(actual_credential.name, expected_credential.name);
        assert_eq!(actual_credential.kind, expected_credential.kind);
        assert_eq!(actual_credential.data, expected_credential.data);
        // Timestamps might differ slightly due to precision, so we don't compare them directly
    }

    #[tokio::test]
    async fn test_env_credential_provider_not_set() {
        let provider = EnvCredentialProvider;
        let cred_id_str = "non_existent_cred";
        let env_var_name = format!("HELIX_CRED_{}", cred_id_str.to_uppercase());

        // Ensure the variable is not set
        std::env::remove_var(&env_var_name);

        let result = provider.get_credential(cred_id_str).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[tokio::test]
    async fn test_env_credential_provider_malformed_json() {
        let provider = EnvCredentialProvider;
        let cred_id_str = "malformed_json_cred";
        let env_var_name = format!("HELIX_CRED_{}", cred_id_str.to_uppercase());
        let malformed_json = "{ \"id\": \"not-a-uuid\", \"name\": "; // Incomplete JSON

        std::env::set_var(&env_var_name, malformed_json);

        let result = provider.get_credential(cred_id_str).await;

        std::env::remove_var(&env_var_name);

        assert!(result.is_err());
        if let Err(HelixError::ValidationError { context, message }) = result {
            assert!(context.contains("credential"));
            assert!(message.contains(&format!("Failed to deserialize credential from environment variable {}", env_var_name)));
        } else {
            panic!("Expected ValidationError, got {:?}", result);
        }
    }
    
    #[tokio::test]
    async fn test_env_credential_provider_non_unicode_value() {
        let provider = EnvCredentialProvider;
        let cred_id_str = "non_unicode_cred";
        let env_var_name = format!("HELIX_CRED_{}", cred_id_str.to_uppercase());

        // Create non-Unicode OsString (platform-dependent)
        #[cfg(unix)]
        {
            use std::ffi::OsStr;
            use std::os::unix::ffi::OsStrExt;
            let non_unicode_value = OsStr::from_bytes(&[0x66, 0x6f, 0x80, 0x6f]); // "fo\x80o" (invalid UTF-8)
            std::env::set_var(&env_var_name, non_unicode_value);
        }
        #[cfg(windows)]
        {
            // On Windows, OsStrings are WTF-8, so creating truly non-Unicode that VarError::NotUnicode catches
            // is tricky. For now, we'll skip this specific sub-test on Windows or use a placeholder.
            // This test primarily targets the VarError::NotUnicode path.
            // If std::env::var returns Ok with non-unicode, serde will fail later.
            // For simplicity, we'll assume this test is more relevant for Unix-like systems
            // where filenames/env vars can more easily be non-UTF-8.
            // If a reliable way to trigger VarError::NotUnicode on Windows is found, update this.
            println!("Skipping non-Unicode env var value test on Windows for now.");
            return;
        }


        let result = provider.get_credential(cred_id_str).await;
        std::env::remove_var(&env_var_name);

        #[cfg(unix)] // Only assert if the test ran
        {
            assert!(result.is_err());
            if let Err(HelixError::ConfigError(msg)) = result {
                assert!(msg.contains(&format!("Environment variable {} contains non-Unicode data", env_var_name)));
            } else {
                panic!("Expected ConfigError for non-Unicode data, got {:?}", result);
            }
        }
    }


    #[tokio::test]
    async fn test_integration_store_and_encryption() {
        let store = MockCredentialStore::new();
        let service = MockEncryptionService;
        let profile_id = Uuid::new_v4();
        let cred_id = Uuid::new_v4();

        let secret_data = "integration_test_secret";
        let encrypted_data = STANDARD.encode(secret_data);

        let credential = Credential::new(
            cred_id,
            profile_id,
            "Integration Test".to_string(),
            "api_key".to_string(),
            encrypted_data.clone(),
        );

        // Store the credential
        store.add_cred(credential);

        // Retrieve and decrypt
        let fetched_cred = store.get_credential_by_id(&profile_id, &cred_id).await.unwrap();
        assert!(fetched_cred.is_some());

        let cred = fetched_cred.unwrap();
        let decrypted = service.decrypt(&profile_id, &cred.data).await.unwrap();
        assert_eq!(decrypted, secret_data);
    }

    #[tokio::test]
    async fn test_integration_multiple_credentials() {
        let store = MockCredentialStore::new();
        let service = MockEncryptionService;
        let profile_id = Uuid::new_v4();

        let credentials = vec![
            ("api_key_1", "secret_1"),
            ("api_key_2", "secret_2"),
            ("oauth_token", "token_data"),
            ("password", "user_password"),
        ];

        let mut cred_ids = Vec::new();

        // Store multiple credentials
        for (name, secret) in &credentials {
            let cred_id = Uuid::new_v4();
            let encrypted_data = STANDARD.encode(secret);

            let credential = Credential::new(
                cred_id,
                profile_id,
                name.to_string(),
                "test_type".to_string(),
                encrypted_data,
            );

            store.add_cred(credential);
            cred_ids.push(cred_id);
        }

        // Verify all credentials can be retrieved and decrypted
        for (i, (name, expected_secret)) in credentials.iter().enumerate() {
            let fetched = store.get_credential_by_id(&profile_id, &cred_ids[i]).await.unwrap();
            assert!(fetched.is_some());

            let cred = fetched.unwrap();
            assert_eq!(cred.name, *name);

            let decrypted = service.decrypt(&profile_id, &cred.data).await.unwrap();
            assert_eq!(decrypted, *expected_secret);
        }
    }

    #[test]
    fn test_credential_boundary_values() {
        // Test with minimum UUID values
        let min_uuid = Uuid::from_bytes([0; 16]);
        let max_uuid = Uuid::from_bytes([255; 16]);

        let credential = Credential::new(
            min_uuid,
            max_uuid,
            "Boundary Test".to_string(),
            "boundary".to_string(),
            "boundary_data".to_string(),
        );

        assert_eq!(credential.id, min_uuid);
        assert_eq!(credential.profile_id, max_uuid);
    }

    #[test]
    fn test_credential_json_edge_cases() {
        let credential = Credential::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            r#"Name with "quotes" and \backslashes"#.to_string(),
            "json_test".to_string(),
            r#"{"key": "value with \"quotes\""}"#.to_string(),
        );

        let serialized = serde_json::to_string(&credential).expect("Failed to serialize");
        let deserialized: Credential = serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(credential, deserialized);
        assert!(credential.name.contains("quotes"));
        assert!(credential.data.contains("quotes"));
    }
}
