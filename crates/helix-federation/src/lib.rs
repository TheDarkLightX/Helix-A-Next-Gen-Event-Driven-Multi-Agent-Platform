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

//! Federation layer for networking Helix intelligence desks.
//!
//! This crate provides the primitives for swarming multiple Helix instances:
//!
//! - [`PeerDesk`] represents a trusted remote desk with an endpoint, auth token,
//!   and trust score.
//! - [`PeerRegistry`] holds the set of configured peers and supports CRUD.
//! - [`FederationEvent`] is the CloudEvents-compatible payload dispatched to
//!   peers when significant intelligence events occur.
//! - [`OutboundDispatcher`] sends federation events to peers via HTTP POST and
//!   records delivery outcomes in a [`DispatchLog`].
//! - [`DispatchLogEntry`] records each outbound dispatch attempt with status
//!   and latency.
//!
//! All operations are deterministic and fail-closed: a peer that cannot be
//! reached is recorded as failed, and the local desk continues operating
//! without blocking on federation delivery.

pub mod dispatcher;
pub mod errors;
pub mod peer;
pub mod types;

pub use dispatcher::{DispatchLog, DispatchLogEntry, DispatchStatus, OutboundDispatcher};
pub use errors::FederationError;
pub use peer::{PeerDesk, PeerRegistry, PeerRegistryQuery};
pub use types::{
    compute_overview, desk_id_from_env, FederationEvent, FederationEventKind,
    FederationOverview, FederationStatus,
};

#[cfg(test)]
mod tests;
