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

//! Federation error types.

use thiserror::Error;

/// Errors produced by the federation layer.
#[derive(Error, Debug)]
pub enum FederationError {
    /// A peer was not found in the registry.
    #[error("peer desk {0} not found")]
    PeerNotFound(String),
    /// A peer with the same id already exists.
    #[error("peer desk {0} already exists")]
    PeerAlreadyExists(String),
    /// Peer validation failed (bad URL, empty name, etc.).
    #[error("peer validation failed: {0}")]
    PeerValidation(String),
    /// Outbound HTTP dispatch failed.
    #[error("dispatch to peer {peer_id} failed: {message}")]
    DispatchFailed {
        /// The peer that was the target of the dispatch.
        peer_id: String,
        /// Human-readable error message.
        message: String,
    },
    /// Serialization of a federation event failed.
    #[error("serialization error: {0}")]
    Serialization(String),
    /// The dispatch log entry was not found.
    #[error("dispatch log entry {0} not found")]
    LogEntryNotFound(String),
}

impl FederationError {
    /// Whether this error is a peer-not-found variant.
    pub fn is_peer_not_found(&self) -> bool {
        matches!(self, Self::PeerNotFound(_))
    }
}
