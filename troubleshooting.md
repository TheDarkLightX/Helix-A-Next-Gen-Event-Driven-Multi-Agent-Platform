# Troubleshooting Log

## 2025-04-28

*   **Issue:** Attempted to create/edit files within `crates/helix-runtime` (specifically `src/credentials.rs` for `RuntimeCredentialProvider`) but failed because the path is excluded by `.gitignore`.
*   **Context:** This occurred during the task "Design and implement `CredentialProvider` logic".
*   **Workaround:** Defined placeholder traits (`CredentialStore`, `EncryptionService`) required by the provider within `helix-core/src/credential.rs` instead.
*   **Next Steps:** Need clarification on where the concrete `CredentialProvider` implementation should reside, or if `crates/helix-runtime` should be removed from `.gitignore`.
