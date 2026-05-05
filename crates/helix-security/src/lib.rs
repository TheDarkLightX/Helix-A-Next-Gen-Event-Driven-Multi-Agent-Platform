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

#![warn(missing_docs)]

//! Security and encryption utilities for Helix.
//!
//! This crate provides:
//! - Credential encryption and decryption
//! - Policy-based access control
//! - Secure key management
//! - Authentication and authorization
//! - Audit logging

pub mod audit;
pub mod auth;
pub mod encryption;
pub mod errors;
pub mod policies;

pub use auth::{ApiTokenAuthConfig, AuthDecision, AuthDenyReason, AuthService};
pub use errors::SecurityError;
pub use policies::{PolicyDecision, PolicyEffect, PolicyEngine, PolicyRequest, PolicyRule};
