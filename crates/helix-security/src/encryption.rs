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


//! Encryption and decryption utilities
use crate::errors::SecurityError;
use async_trait::async_trait;
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use hex;
use std::env;

const KEY_ENV_VAR: &str = "HELIX_ENCRYPTION_KEY";
const KEY_LENGTH_BYTES: usize = 32; // AES-256 key length
const NONCE_LENGTH_BYTES: usize = 12; // AES-GCM standard nonce length (96 bits)

/// Trait for credential encryption and decryption.
#[async_trait]
pub trait CredentialEncrypterDecrypter: Send + Sync {
    /// Encrypts a plaintext credential part.
    ///
    /// # Arguments
    ///
    /// * `plaintext_credential_part` - The plaintext string to encrypt.
    ///
    /// # Returns
    ///
    /// A `Result` containing the base64 encoded ciphertext string (nonce prepended)
    /// or a `SecurityError` if encryption fails.
    async fn encrypt(&self, plaintext_credential_part: &str) -> Result<String, SecurityError>;

    /// Decrypts a ciphertext credential part.
    ///
    /// # Arguments
    ///
    /// * `ciphertext_credential_part` - The base64 encoded ciphertext string (nonce prepended) to decrypt.
    ///
    /// # Returns
    ///
    /// A `Result` containing the plaintext string or a `SecurityError` if decryption fails.
    async fn decrypt(&self, ciphertext_credential_part: &str) -> Result<String, SecurityError>;
}

/// An implementation of `CredentialEncrypterDecrypter` using AES-GCM.
pub struct AesGcmCredentialEncrypterDecrypter {
    key: Vec<u8>,
}

impl AesGcmCredentialEncrypterDecrypter {
    /// Creates a new `AesGcmCredentialEncrypterDecrypter`.
    ///
    /// The encryption key is loaded from the `HELIX_ENCRYPTION_KEY` environment variable.
    /// The key must be a hex-encoded string representing 32 bytes (256 bits).
    ///
    /// # Returns
    ///
    /// A `Result` containing the new instance or a `SecurityError` if the key
    /// cannot be loaded or is invalid.
    pub fn new() -> Result<Self, SecurityError> {
        let hex_key = env::var(KEY_ENV_VAR).map_err(|e| {
            SecurityError::KeyNotFound(format!(
                "Environment variable {} not found: {}",
                KEY_ENV_VAR, e
            ))
        })?;

        let key = hex::decode(hex_key).map_err(|e| {
            SecurityError::InvalidKey(format!("Key is not valid hex: {}", e))
        })?;

        if key.len() != KEY_LENGTH_BYTES {
            return Err(SecurityError::InvalidKey(format!(
                "Invalid key length: expected {} bytes, got {}",
                KEY_LENGTH_BYTES,
                key.len()
            )));
        }
        Ok(Self { key })
    }

    fn get_cipher(&self) -> Result<Aes256Gcm, SecurityError> {
        Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| SecurityError::InvalidKey(format!("Failed to initialize AES-GCM cipher: {}", e)))
    }
}

#[async_trait]
impl CredentialEncrypterDecrypter for AesGcmCredentialEncrypterDecrypter {
    async fn encrypt(&self, plaintext_credential_part: &str) -> Result<String, SecurityError> {
        let cipher = self.get_cipher()?;
        let nonce_bytes = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bit nonce

        let ciphertext = cipher
            .encrypt(&nonce_bytes, plaintext_credential_part.as_bytes())
            .map_err(|e| SecurityError::EncryptionError(format!("AES-GCM encryption failed: {}", e)))?;

        let mut result = Vec::with_capacity(NONCE_LENGTH_BYTES + ciphertext.len());
        result.extend_from_slice(nonce_bytes.as_slice());
        result.extend_from_slice(&ciphertext);

        Ok(BASE64_STANDARD.encode(result))
    }

    async fn decrypt(&self, ciphertext_credential_part: &str) -> Result<String, SecurityError> {
        let combined_bytes = BASE64_STANDARD
            .decode(ciphertext_credential_part)
            .map_err(|e| SecurityError::DecryptionError(format!("Base64 decoding failed: {}", e)))?;

        if combined_bytes.len() < NONCE_LENGTH_BYTES {
            return Err(SecurityError::DecryptionError(
                "Ciphertext is too short to contain a nonce".to_string(),
            ));
        }

        let (nonce_bytes, ciphertext) = combined_bytes.split_at(NONCE_LENGTH_BYTES);
        let nonce = Nonce::from_slice(nonce_bytes);

        let cipher = self.get_cipher()?;
        let plaintext_bytes = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| SecurityError::DecryptionError(format!("AES-GCM decryption failed: {}", e)))?;

        String::from_utf8(plaintext_bytes)
            .map_err(|e| SecurityError::DecryptionError(format!("UTF-8 conversion failed: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    const TEST_KEY_HEX: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"; // 32 bytes

    fn setup_key_env_var() {
        env::set_var(KEY_ENV_VAR, TEST_KEY_HEX);
    }

    fn clear_key_env_var() {
        env::remove_var(KEY_ENV_VAR);
    }

    #[tokio::test]
    async fn test_encrypt_decrypt_success() {
        setup_key_env_var();
        let encrypter = AesGcmCredentialEncrypterDecrypter::new().unwrap();
        let plaintext = "my_super_secret_password";

        let encrypted = encrypter.encrypt(plaintext).await.unwrap();
        let decrypted = encrypter.decrypt(&encrypted).await.unwrap();

        assert_ne!(plaintext, encrypted);
        assert_eq!(plaintext, decrypted);
        clear_key_env_var();
    }

    #[test]
    fn test_new_key_loading_success() {
        setup_key_env_var();
        assert!(AesGcmCredentialEncrypterDecrypter::new().is_ok());
        clear_key_env_var();
    }

    #[test]
    fn test_new_key_env_var_not_set() {
        clear_key_env_var(); // Ensure it's not set
        let result = AesGcmCredentialEncrypterDecrypter::new();
        assert!(result.is_err());
        match result.err().unwrap() {
            SecurityError::KeyNotFound(_) => {} // Expected
            e => panic!("Unexpected error type: {:?}", e),
        }
    }

    #[test]
    fn test_new_invalid_hex_key() {
        env::set_var(KEY_ENV_VAR, "not_a_hex_key");
        let result = AesGcmCredentialEncrypterDecrypter::new();
        assert!(result.is_err());
        match result.err().unwrap() {
            SecurityError::InvalidKey(msg) => assert!(msg.contains("Key is not valid hex")),
            e => panic!("Unexpected error type: {:?}", e),
        }
        clear_key_env_var();
    }

    #[test]
    fn test_new_invalid_key_length() {
        env::set_var(KEY_ENV_VAR, "00112233"); // Too short
        let result = AesGcmCredentialEncrypterDecrypter::new();
        assert!(result.is_err());
        match result.err().unwrap() {
            SecurityError::InvalidKey(msg) => assert!(msg.contains("Invalid key length")),
            e => panic!("Unexpected error type: {:?}", e),
        }
        clear_key_env_var();
    }

    #[tokio::test]
    async fn test_decrypt_tampered_ciphertext() {
        setup_key_env_var();
        let encrypter = AesGcmCredentialEncrypterDecrypter::new().unwrap();
        let plaintext = "another_secret";
        let encrypted = encrypter.encrypt(plaintext).await.unwrap();

        // Tamper the ciphertext (e.g., flip a bit in the base64 string)
        let mut tampered_encrypted_chars: Vec<char> = encrypted.chars().collect();
        if !tampered_encrypted_chars.is_empty() {
            tampered_encrypted_chars[0] = if tampered_encrypted_chars[0] == 'A' { 'B' } else { 'A' };
        }
        let tampered_encrypted: String = tampered_encrypted_chars.into_iter().collect();


        let result = encrypter.decrypt(&tampered_encrypted).await;
        assert!(result.is_err());
        match result.err().unwrap() {
            SecurityError::DecryptionError(msg) => {
                // Error could be base64 decoding or AES-GCM decryption
                assert!(msg.contains("Base64 decoding failed") || msg.contains("AES-GCM decryption failed"));
            }
            e => panic!("Unexpected error type: {:?}", e),
        }
        clear_key_env_var();
    }

    #[tokio::test]
    async fn test_decrypt_too_short_ciphertext() {
        setup_key_env_var();
        let encrypter = AesGcmCredentialEncrypterDecrypter::new().unwrap();
        // Nonce is 12 bytes, base64 encoded will be 16 chars.
        // Let's provide something shorter than what a nonce would be.
        let too_short_ciphertext = "short";
        let result = encrypter.decrypt(too_short_ciphertext).await;
        assert!(result.is_err());
        match result.err().unwrap() {
            SecurityError::DecryptionError(msg) => {
                 assert!(msg.contains("Base64 decoding failed") || msg.contains("Ciphertext is too short"));
            }
            e => panic!("Unexpected error type: {:?}", e),
        }
        clear_key_env_var();
    }

    #[tokio::test]
    async fn test_decrypt_with_different_key_fails_conceptually() {
        setup_key_env_var(); // Key A
        let encrypter_a = AesGcmCredentialEncrypterDecrypter::new().unwrap();
        let plaintext = "secret_for_key_a";
        let encrypted_by_a = encrypter_a.encrypt(plaintext).await.unwrap();

        // Simulate a different key by creating a new encrypter instance
        // after changing the env var to a different valid key.
        env::set_var(KEY_ENV_VAR, "1f1e1d1c1b1a191817161514131211100f0e0d0c0b0a09080706050403020100"); // Key B
        let encrypter_b = AesGcmCredentialEncrypterDecrypter::new().unwrap();

        let result = encrypter_b.decrypt(&encrypted_by_a).await;
        assert!(result.is_err(), "Decryption with a different key should fail");
        match result.err().unwrap() {
            SecurityError::DecryptionError(msg) => {
                assert!(msg.contains("AES-GCM decryption failed"));
            }
            e => panic!("Unexpected error type: {:?}", e),
        }
        clear_key_env_var();
    }
}
