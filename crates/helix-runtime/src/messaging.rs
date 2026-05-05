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

//! Messaging and event publishing adapters (imperative shell).

use async_nats::Client;
use futures::StreamExt;
use helix_agent_sdk::{EventPublisher, SdkError};
use helix_core::event::Event;
use helix_core::types::AgentId;
use serde_json::Value as JsonValue;
use std::sync::{Arc, Mutex};
use thiserror::Error;

/// Errors related to messaging operations within the Helix runtime.
#[derive(Error, Debug)]
pub enum MessagingError {
    /// Failed to connect to NATS.
    #[error("NATS connection failed: {0}")]
    NatsConnection(#[from] async_nats::ConnectError),
    /// Failed to publish a message.
    #[error("NATS publish failed: {0}")]
    NatsPublish(String),
    /// JSON serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),
    /// Subscription error.
    #[error("Subscription error: {0}")]
    Subscription(String),
}

/// Configuration for the NATS client.
#[derive(Debug, Clone)]
pub struct NatsConfig {
    /// Comma-separated list of NATS server URLs.
    pub urls: String,
}

impl Default for NatsConfig {
    fn default() -> Self {
        Self {
            urls: "nats://localhost:4222".to_string(),
        }
    }
}

/// Configuration for a logical "stream" (subject set).
///
/// Note: The runtime currently treats streams as plain NATS subject patterns.
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Stream name (informational).
    pub name: String,
    /// Subjects to subscribe to.
    pub subjects: Vec<String>,
}

/// Simple NATS client wrapper.
#[derive(Clone)]
pub struct NatsClient {
    client: Client,
}

impl NatsClient {
    /// Connects to NATS using a URL list.
    pub async fn connect(config: &NatsConfig) -> Result<Self, MessagingError> {
        let client = async_nats::connect(&config.urls).await?;
        Ok(Self { client })
    }

    /// Publishes one event to NATS.
    ///
    /// Subject selection is deterministic:
    /// - If `event.subject` is set, use it.
    /// - Otherwise use `event.type`.
    pub async fn publish_event(&self, event: &Event) -> Result<(), MessagingError> {
        let subject = event
            .subject
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&event.r#type);

        let payload =
            serde_json::to_vec(event).map_err(|e| MessagingError::Serialization(e.to_string()))?;

        self.client
            .publish(subject.to_string(), payload.into())
            .await
            .map_err(|e| MessagingError::NatsPublish(e.to_string()))?;
        Ok(())
    }

    /// Subscribes to a subject and invokes a callback per received event.
    ///
    /// This is intentionally fail-closed: malformed payloads are dropped.
    pub async fn subscribe<F>(&self, subject: &str, callback: F) -> Result<(), MessagingError>
    where
        F: Fn(Event) + Send + Sync + 'static,
    {
        let mut sub = self
            .client
            .subscribe(subject.to_string())
            .await
            .map_err(|e| MessagingError::Subscription(e.to_string()))?;

        let cb = Arc::new(callback);
        tokio::spawn(async move {
            while let Some(msg) = sub.next().await {
                let evt = match serde_json::from_slice::<Event>(&msg.payload) {
                    Ok(e) => e,
                    Err(err) => {
                        tracing::warn!(error = %err, "Dropping malformed event payload");
                        continue;
                    }
                };
                cb(evt);
            }
        });

        Ok(())
    }
}

/// In-memory event collector implementing [`EventPublisher`].
///
/// Useful for tests and local-only runs.
#[derive(Debug, Clone, Default)]
pub struct InMemoryEventCollector {
    events: Arc<Mutex<Vec<Event>>>,
}

impl InMemoryEventCollector {
    /// Creates a new, empty collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Drains all collected events.
    pub fn drain(&self) -> Vec<Event> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }

    /// Returns a snapshot of collected events.
    pub fn snapshot(&self) -> Vec<Event> {
        self.events.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl EventPublisher for InMemoryEventCollector {
    async fn publish_event(
        &self,
        agent_id: &AgentId,
        event_payload: JsonValue,
        event_type_override: Option<String>,
    ) -> Result<(), SdkError> {
        let event_type = event_type_override.unwrap_or_else(|| "agent.event".to_string());
        let event = Event::new(
            format!("agent:{}", agent_id),
            event_type,
            Some(event_payload),
        );

        self.events.lock().unwrap().push(event);
        Ok(())
    }
}
