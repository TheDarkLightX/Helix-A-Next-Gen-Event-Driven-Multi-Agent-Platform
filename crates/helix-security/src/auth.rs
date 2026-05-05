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

//! Deterministic authentication primitives.

use crate::errors::SecurityError;
use sha2::{Digest, Sha256};

/// Minimum API token length accepted by the built-in bearer-token gate.
pub const MIN_API_TOKEN_LEN: usize = 16;

/// Bearer-token authentication configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiTokenAuthConfig {
    required: bool,
    token_sha256_hex: Option<String>,
}

impl ApiTokenAuthConfig {
    /// Builds a disabled auth configuration for trusted local development.
    pub fn disabled() -> Self {
        Self {
            required: false,
            token_sha256_hex: None,
        }
    }

    /// Builds a required auth configuration from a plaintext token.
    pub fn required_from_plaintext(token: &str) -> Result<Self, SecurityError> {
        let token = normalize_configured_token(token)?;
        Ok(Self {
            required: true,
            token_sha256_hex: Some(hash_token(token)),
        })
    }

    /// Builds a configuration from a required flag and optional plaintext token.
    pub fn from_optional_plaintext(
        required: bool,
        token: Option<&str>,
    ) -> Result<Self, SecurityError> {
        match (
            required,
            token.map(str::trim).filter(|token| !token.is_empty()),
        ) {
            (false, None) => Ok(Self::disabled()),
            (_, Some(token)) => Self::required_from_plaintext(token),
            (true, None) => Err(SecurityError::AuthenticationError(
                "API auth is required but no token is configured".to_string(),
            )),
        }
    }

    /// Returns whether this config requires bearer-token authentication.
    pub fn required(&self) -> bool {
        self.required
    }

    fn expected_hash(&self) -> Option<&str> {
        self.token_sha256_hex.as_deref()
    }
}

/// Deterministic authentication decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthDecision {
    /// Request is authenticated as the given subject.
    Allow {
        /// Stable subject identifier for the authenticated operator.
        subject: String,
    },
    /// Request is denied with a stable reason.
    Deny {
        /// Machine-stable denial reason.
        reason: AuthDenyReason,
    },
}

/// Stable denial reasons emitted by the built-in auth gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthDenyReason {
    /// Auth was required but no expected token hash is configured.
    AuthMisconfigured,
    /// The `Authorization` header is missing.
    MissingAuthorization,
    /// The header is not `Bearer <token>`.
    MalformedAuthorization,
    /// The supplied bearer token does not match.
    InvalidToken,
}

impl AuthDenyReason {
    /// Stable string representation for API responses and audit logs.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuthMisconfigured => "auth_misconfigured",
            Self::MissingAuthorization => "missing_authorization",
            Self::MalformedAuthorization => "malformed_authorization",
            Self::InvalidToken => "invalid_token",
        }
    }
}

/// API bearer-token authentication service.
#[derive(Debug, Clone)]
pub struct AuthService {
    config: ApiTokenAuthConfig,
}

impl AuthService {
    /// Creates a new auth service.
    pub fn new(config: ApiTokenAuthConfig) -> Self {
        Self { config }
    }

    /// Creates a disabled auth service for local development and tests.
    pub fn disabled() -> Self {
        Self::new(ApiTokenAuthConfig::disabled())
    }

    /// Returns whether bearer-token authentication is required.
    pub fn is_required(&self) -> bool {
        self.config.required()
    }

    /// Evaluates an optional `Authorization` header.
    pub fn evaluate_bearer_header(&self, authorization: Option<&str>) -> AuthDecision {
        if !self.config.required {
            return AuthDecision::Allow {
                subject: "local_dev".to_string(),
            };
        }

        let Some(expected_hash) = self.config.expected_hash() else {
            return AuthDecision::Deny {
                reason: AuthDenyReason::AuthMisconfigured,
            };
        };
        let Some(header) = authorization else {
            return AuthDecision::Deny {
                reason: AuthDenyReason::MissingAuthorization,
            };
        };
        let Some(token) = parse_bearer_token(header) else {
            return AuthDecision::Deny {
                reason: AuthDenyReason::MalformedAuthorization,
            };
        };

        if constant_time_eq(hash_token(token).as_bytes(), expected_hash.as_bytes()) {
            AuthDecision::Allow {
                subject: "api_token_operator".to_string(),
            }
        } else {
            AuthDecision::Deny {
                reason: AuthDenyReason::InvalidToken,
            }
        }
    }

    /// Legacy username/password entrypoint. This intentionally fails closed.
    pub fn authenticate(&self, _username: &str, _password: &str) -> bool {
        false
    }
}

fn normalize_configured_token(token: &str) -> Result<&str, SecurityError> {
    let token = token.trim();
    if token.len() < MIN_API_TOKEN_LEN {
        return Err(SecurityError::AuthenticationError(format!(
            "API token must be at least {MIN_API_TOKEN_LEN} characters"
        )));
    }
    if token.chars().any(char::is_control) {
        return Err(SecurityError::AuthenticationError(
            "API token contains a control character".to_string(),
        ));
    }
    Ok(token)
}

fn parse_bearer_token(header: &str) -> Option<&str> {
    let header = header.trim();
    let token = header.strip_prefix("Bearer ")?;
    let token = token.trim();
    if token.is_empty() || token.chars().any(char::is_whitespace) {
        None
    } else {
        Some(token)
    }
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "operator-token-123";

    #[test]
    fn disabled_auth_allows_missing_header() {
        let service = AuthService::disabled();
        assert_eq!(
            service.evaluate_bearer_header(None),
            AuthDecision::Allow {
                subject: "local_dev".to_string()
            }
        );
    }

    #[test]
    fn required_auth_accepts_matching_bearer_token() {
        let service = AuthService::new(ApiTokenAuthConfig::required_from_plaintext(TOKEN).unwrap());
        assert_eq!(
            service.evaluate_bearer_header(Some("Bearer operator-token-123")),
            AuthDecision::Allow {
                subject: "api_token_operator".to_string()
            }
        );
    }

    #[test]
    fn required_auth_denies_missing_malformed_and_invalid_tokens() {
        let service = AuthService::new(ApiTokenAuthConfig::required_from_plaintext(TOKEN).unwrap());
        assert_eq!(
            service.evaluate_bearer_header(None),
            AuthDecision::Deny {
                reason: AuthDenyReason::MissingAuthorization
            }
        );
        assert_eq!(
            service.evaluate_bearer_header(Some("Basic operator-token-123")),
            AuthDecision::Deny {
                reason: AuthDenyReason::MalformedAuthorization
            }
        );
        assert_eq!(
            service.evaluate_bearer_header(Some("Bearer wrong-token-12345")),
            AuthDecision::Deny {
                reason: AuthDenyReason::InvalidToken
            }
        );
    }

    #[test]
    fn required_auth_config_rejects_missing_or_short_tokens() {
        assert!(ApiTokenAuthConfig::from_optional_plaintext(true, None).is_err());
        assert!(ApiTokenAuthConfig::required_from_plaintext("short").is_err());
    }

    #[test]
    fn legacy_password_auth_fails_closed() {
        let service = AuthService::disabled();
        assert!(!service.authenticate("admin", "password"));
    }
}
