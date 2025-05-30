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


#![warn(missing_docs)]

//! Handles messaging infrastructure, primarily NATS JetStream.

use async_nats::jetstream::{self, consumer::PullConsumer, Context as JetStreamContext};
use async_nats::Client;
use futures::stream::TryStreamExt;
use futures::StreamExt; // Added for .next() on message stream
use helix_core::event::Event;
use helix_core::types::AgentId; // Added for InMemoryEventCollector
use helix_agent_sdk::{EventPublisher, SdkError}; // Added for InMemoryEventCollector
use std::sync::{Arc, Mutex}; // Added for InMemoryEventCollector
use std::time::Duration;
use thiserror::Error;
use tokio;
use serde_json; // Added for InMemoryEventCollector

/// Errors related to messaging operations within the Helix runtime.
#[derive(Error, Debug)]
pub enum MessagingError {
    /// Failed to connect to the NATS server.
    #[error("NATS connection failed: {0}")]
    NatsConnection(#[from] async_nats::ConnectError),
    /// A NATS request (e.g., publish, subscribe) failed.
    #[error("NATS request failed: {0}")]
    NatsRequest(#[from] async_nats::RequestError),
    /// Failed to obtain the JetStream context from the NATS client.
    #[error("Failed to get JetStream context: {0}")]
    JetStreamContext(std::io::Error),
    /// Error during serialization (e.g., event to JSON).
    #[error("Serialization error: {0}")]
    SerializationError(String),
    /// Error publishing a message to a NATS subject.
    #[error("Publish error: {0}")]
    PublishError(String),
    /// Error accessing or creating a JetStream stream.
    #[error("Stream access error: {0}")]
    StreamAccessError(String),
    /// Error creating or managing a NATS subscription/consumer.
    #[error("Subscription error: {0}")]
    SubscriptionError(String),
    /// Error fetching messages from a pull consumer.
    #[error("Fetch error: {0}")]
    FetchError(String),
}

/// Configuration for the NATS client.
#[derive(Debug, Clone)]
pub struct NatsConfig {
    /// Comma-separated list of NATS server URLs.
    pub urls: String,
    // TODO: Add authentication options (token, nkey, user/pass)
}

impl Default for NatsConfig {
    fn default() -> Self {
        Self {
            urls: "nats://localhost:4222".to_string(),
        }
    }
}

/// Represents an active NATS client connection with JetStream context.
#[derive(Clone)]
pub struct NatsClient {
    /// The underlying async-nats client.
    pub client: Client,
    /// The JetStream context derived from the client.
    pub jetstream: JetStreamContext,
}

/// Configuration for a JetStream stream.
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// The name of the stream.
    pub name: String,
    /// A list of subjects the stream listens to.
    pub subjects: Vec<String>,
    /// Optional maximum number of messages the stream will retain.
    pub max_messages: Option<u64>,
    /// Optional maximum age for messages in the stream.
    pub max_age: Option<Duration>,
}

impl NatsClient {
    /// Connects to NATS and initializes the JetStream context.
    pub async fn connect(config: &NatsConfig) -> Result<Self, MessagingError> {
        tracing::info!(urls = %config.urls, "Connecting to NATS...");
        let client = async_nats::connect(&config.urls).await?;
        tracing::info!("Connected to NATS successfully.");

        let jetstream = jetstream::new(client.clone());
        tracing::info!("JetStream context created.");

        Ok(Self { client, jetstream })
    }

    /// Publishes an event, deriving the NATS subject from the event's `subject` or `type`.
    pub async fn publish_event(&self, event: &Event) -> Result<(), MessagingError> {
        let nats_subject = event
            .subject
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| s.clone())
            .unwrap_or_else(|| event.r#type.clone());

        if nats_subject.is_empty() {
            return Err(MessagingError::PublishError(
                "NATS subject derived from event is empty. Event must have a non-empty 'subject' or 'type'."
                    .to_string(),
            ));
        }

        let payload = serde_json::to_vec(event)
            .map_err(|e| MessagingError::SerializationError(e.to_string()))?;

        tracing::debug!(subject = %nats_subject, event_id = %event.id, source = %event.source, type = %event.r#type, "Publishing event");
        self.jetstream
            .publish(nats_subject, payload.into())
            .await
            .map_err(|e| MessagingError::PublishError(format!("NATS publish failed: {}", e)))?;
        Ok(())
    }

    /// Creates a JetStream stream if it doesn't exist.
    pub async fn ensure_stream(&self, config: &StreamConfig) -> Result<(), MessagingError> {
        tracing::debug!(stream = %config.name, "Ensuring stream exists");

        match self.jetstream.get_stream(&config.name).await {
            Ok(_) => {
                tracing::debug!(stream = %config.name, "Stream already exists.");
                Ok(())
            }
            Err(_) => {
                tracing::info!(stream = %config.name, "Creating stream");
                self.jetstream
                    .create_stream(jetstream::stream::Config {
                        name: config.name.clone(),
                        subjects: config.subjects.clone(),
                        max_messages: config.max_messages.map(|v| v as i64).unwrap_or(-1),
                        max_age: config.max_age.unwrap_or(Duration::ZERO),
                        ..Default::default()
                    })
                    .await
                    .map_err(|e| MessagingError::StreamAccessError(e.to_string()))?;
                Ok(())
            }
        }
    }

    /// Subscribes to a stream using a push-based consumer and processes incoming events.
    ///
    /// The `consumer_suffix` is appended to the stream name to create a durable consumer name.
    /// This allows for multiple independent consumers on the same stream if different suffixes are used.
    pub async fn subscribe_to_stream<F>(
        &self,
        stream_name: &str,
        consumer_suffix: &str,
        callback: F,
    ) -> Result<(), MessagingError>
    where
        F: Fn(Event) + Send + Sync + 'static,
    {
        let durable_name = format!("{}_consumer_{}", stream_name, consumer_suffix);
        tracing::info!(stream = %stream_name, consumer = %durable_name, "Subscribing to stream (push consumer)");

        let stream_ctx = self
            .jetstream
            .get_stream(stream_name)
            .await
            .map_err(|e| MessagingError::StreamAccessError(format!("Failed to get stream '{}': {}", stream_name, e)))?;

        let consumer_config = jetstream::consumer::push::Config {
            durable_name: Some(durable_name.clone()),
            // Consider making other options configurable, e.g., deliver_policy, ack_policy
            ..Default::default()
        };

        let consumer = stream_ctx
            .create_consumer(consumer_config)
            .await
            .map_err(|e| MessagingError::SubscriptionError(format!("Failed to create push consumer '{}': {}", durable_name, e)))?;

        let mut messages = consumer
            .messages()
            .await
            .map_err(|e| MessagingError::SubscriptionError(format!("Failed to get message stream for consumer '{}': {}", durable_name, e)))?;

        tracing::info!(consumer = %durable_name, "Successfully subscribed. Listening for messages...");

        let _handle = tokio::spawn(async move {
            tracing::debug!(consumer = %durable_name, "Message processing loop started.");
            while let Some(message_result) = messages.next().await { // Using `next` from `futures::StreamExt`
                match message_result {
                    Ok(message) => {
                        tracing::trace!(consumer = %durable_name, msg_id = ?message.sequence, subject = %message.subject, "Received message");
                        match serde_json::from_slice::<Event>(&message.payload) {
                            Ok(event) => {
                                tracing::debug!(consumer = %durable_name, event_id = %event.id, event_type = %event.r#type, "Processing event");
                                callback(event); // User-provided callback
                                if let Err(e) = message.ack().await {
                                    tracing::error!(consumer = %durable_name, event_id = %event.id, "Failed to ACK message: {}", e);
                                } else {
                                    tracing::trace!(consumer = %durable_name, event_id = %event.id, "Message ACKed successfully.");
                                }
                            }
                            Err(e) => {
                                // Decide on ack/nack strategy for deserialization errors.
                                // For now, log and ACK to prevent redelivery of a malformed message.
                                // Consider moving to a dead-letter queue (DLQ) in the future.
                                tracing::error!(
                                    consumer = %durable_name,
                                    subject = %message.subject,
                                    payload_len = message.payload.len(),
                                    "Failed to deserialize event payload: {}. Message will be ACKed to prevent redelivery.", e
                                );
                                if let Err(ack_err) = message.ack().await {
                                     tracing::error!(consumer = %durable_name, "Failed to ACK malformed message: {}", ack_err);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        // This error typically indicates a problem with the subscription itself, not just a single message.
                        tracing::error!(consumer = %durable_name, "Error receiving message from stream: {}. Subscription might be compromised.", e);
                        // Depending on the error, might need to break the loop or attempt to re-establish.
                        // For now, we log and continue, but this could lead to a tight loop if the error persists.
                        // A brief delay could be added here.
                        // tokio::time::sleep(Duration::from_secs(1)).await; // Example delay
                    }
                }
            }
            tracing::info!(consumer = %durable_name, "Subscription message stream ended.");
        });

        Ok(())
    }

    /// Creates or gets a pull-based consumer for a stream.
    ///
    /// The `durable_name` is crucial for pull consumers to maintain their state across restarts.
    /// `filter_subject` can be used to only consume messages matching a specific subject pattern within the stream.
    pub async fn create_or_get_pull_consumer(
        &self,
        stream_name: &str,
        durable_name: &str,
        filter_subject: Option<String>,
        // Potentially add other jetstream::consumer::pull::Config options here
    ) -> Result<jetstream::consumer::PullConsumer, MessagingError> {
        tracing::info!(
            stream = %stream_name,
            consumer = %durable_name,
            filter = ?filter_subject,
            "Creating/getting pull consumer"
        );

        let stream_ctx = self
            .jetstream
            .get_stream(stream_name)
            .await
            .map_err(|e| MessagingError::StreamAccessError(format!("Failed to get stream '{}': {}", stream_name, e)))?;

        // Attempt to get existing consumer first, then create if not found.
        // Note: async-nats `get_consumer` might not exist directly on stream context,
        // or `create_consumer` might act as get-or-create with the right config.
        // For now, we assume `create_consumer` with a durable name will fetch if exists or create.
        // If specific "get" is needed and available, that would be preferred.

        let consumer_config = jetstream::consumer::pull::Config {
            name: Some(durable_name.to_string()), // Optional: name can be different from durable_name
            durable_name: Some(durable_name.to_string()),
            filter_subject: filter_subject.unwrap_or_default(), // Empty string means no filter
            // ack_policy: jetstream::consumer::AckPolicy::Explicit, // Default, good for pull
            ..Default::default()
        };

        // create_consumer with a durable name will get an existing one or create a new one.
        let consumer: PullConsumer = stream_ctx // Specify type for clarity
            .create_consumer(consumer_config)
            .await
            .map_err(|e| MessagingError::SubscriptionError(format!("Failed to create/get pull consumer '{}' for stream '{}': {}", durable_name, stream_name, e)))?;

        tracing::info!(stream = %stream_name, consumer = %durable_name, "Pull consumer ready.");
        Ok(consumer)
    }

    /// Fetches messages from a pull subscription.
    pub async fn fetch_messages(
        &self,
        consumer: &jetstream::consumer::PullConsumer,
        batch_size: u32,
    ) -> Result<Vec<async_nats::jetstream::Message>, MessagingError> {
        tracing::debug!(consumer_name = ?consumer.name(), batch_size, "Fetching messages from pull consumer");

        // The fetch builder pattern seems correct.
        // Adding more detailed error context.
        let fetch_builder = consumer
            .fetch()
            .max_messages(batch_size as usize)
            .expires(Duration::from_secs(5)); // Add a timeout for the fetch operation itself

        let messages_stream_result = fetch_builder.messages().await;

        match messages_stream_result {
            Ok(messages_stream) => {
                tracing::trace!(consumer_name = ?consumer.name(), "Message stream obtained, collecting messages.");
                let messages = messages_stream
                    .try_collect()
                    .await
                    .map_err(|e| MessagingError::FetchError(format!("Error collecting messages from pull consumer '{}': {}", consumer.name().unwrap_or_default("unknown"), e)))?;
                tracing::debug!(consumer_name = ?consumer.name(), count = messages.len(), "Fetched {} messages", messages.len());
                Ok(messages)
            }
            Err(e) => {
                tracing::error!(consumer_name = ?consumer.name(), "Failed to initiate message fetch: {}", e);
                Err(MessagingError::FetchError(format!("Failed to initiate message fetch for consumer '{}': {}", consumer.name().unwrap_or_default("unknown"), e)))
            }
        }
    }
}

// --- In-Memory Event Collector for testing and simple scenarios ---
/// An in-memory event collector that implements `EventPublisher`.
/// Useful for testing or scenarios where a full message broker is not needed.
#[derive(Debug, Clone)]
pub struct InMemoryEventCollector {
    events: Arc<Mutex<Vec<Event>>>,
}

impl InMemoryEventCollector {
    /// Creates a new, empty `InMemoryEventCollector`.
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Retrieves all collected events, consuming them from the collector.
    pub async fn get_events(&self) -> Vec<Event> {
        let mut guard = self.events.lock().expect("Failed to lock event collector mutex for get_events");
        std::mem::take(&mut *guard)
    }

    /// Clears all events from the collector.
    pub async fn clear_events(&self) {
        let mut guard = self.events.lock().expect("Failed to lock event collector mutex for clear_events");
        guard.clear();
    }
}

#[async_trait::async_trait]
impl EventPublisher for InMemoryEventCollector {
    async fn publish_event(
        &self,
        agent_id: &AgentId,
        event_payload: serde_json::Value,
        event_type_override: Option<String>,
    ) -> Result<(), SdkError> {
        let event_r#type = event_type_override.unwrap_or_else(|| "DefaultAgentEvent".to_string());
        // Assuming AgentId has a Display impl that gives its string value, or use agent_id.0.to_string()
        let source = format!("agent:{}", agent_id.to_string());

        tracing::info!(
            source = %source,
            event_type = %event_r#type,
            payload_size = event_payload.to_string().len(), // Log size instead of full payload for brevity
            "InMemoryEventCollector: Collecting event"
        );
        
        // Construct helix_core::event::Event according to its definition
        let event = Event {
            id: helix_core::types::EventId::new_v4(), // Assuming EventId is Uuid from helix_core::types
            source,
            specversion: "1.0".to_string(),
            r#type: event_r#type,
            datacontenttype: Some("application/json".to_string()),
            subject: None,
            time: chrono::Utc::now(),
            data: Some(event_payload),
            correlation_id: None,
            causation_id: None,
        };
        
        match self.events.lock() {
            Ok(mut guard) => guard.push(event),
            Err(e) => {
                tracing::error!("Failed to lock event collector mutex for writing: {}", e);
                return Err(SdkError::InternalError(format!("Failed to lock event collector: {}", e)));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::atomic::{AtomicUsize, Ordering}, time::Duration};
    use tokio::sync::mpsc;
    use uuid::Uuid; // Ensure Uuid is imported for tests

    // Helper to initialize NATS client for tests
    async fn init_test_nats() -> Option<NatsClient> {
        let config = NatsConfig::default();
        match NatsClient::connect(&config).await {
            Ok(client) => Some(client),
            Err(e) => {
                eprintln!(
                    "NATS connection failed ({}): {}. Skipping NATS-dependent test. Is NATS running?",
                    config.urls, e
                );
                None
            }
        }
    }

    // Helper StreamConfig with Default implementation
    impl Default for StreamConfig {
        fn default() -> Self {
            Self {
                name: format!("default_stream_{}", Uuid::new_v4()),
                subjects: vec!["default.subject".to_string()],
                max_messages: Some(100),
                max_age: Some(Duration::from_secs(3600)),
            }
        }
    }


    #[tokio::test]
    async fn test_publish_and_push_subscribe() {
        let nats = match init_test_nats().await {
            Some(n) => n,
            None => return,
        };

        let stream_name = format!("test_push_stream_{}", Uuid::new_v4());
        let nats_subject = format!("test.push.subject.{}", Uuid::new_v4());
        let event_type = "com.helix.test.push_event".to_string();

        let stream_config = StreamConfig {
            name: stream_name.clone(),
            subjects: vec![format!("{}.*", nats_subject)], // Listen to the specific subject and its children
            max_messages: Some(10),
            max_age: Some(Duration::from_secs(60)),
        };

        nats.ensure_stream(&stream_config)
            .await
            .expect("Failed to ensure test stream for push subscribe");

        let (tx, mut rx) = mpsc::channel::<Event>(1);
        let received_count = Arc::new(AtomicUsize::new(0));
        let received_count_clone = received_count.clone();

        let callback_tx = tx.clone();
        nats.subscribe_to_stream(&stream_name, "test_consumer", move |event| {
            let tx_clone = callback_tx.clone();
            received_count_clone.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                if let Err(e) = tx_clone.send(event).await {
                    tracing::error!("Failed to send event over mpsc channel: {}", e);
                }
            });
        })
        .await
        .expect("Failed to subscribe to stream");

        // Give subscriber a moment to connect
        tokio::time::sleep(Duration::from_millis(500)).await;

        let original_event = Event {
            id: Uuid::new_v4(),
            source: "test_publisher_push".to_string(),
            specversion: "1.0".to_string(),
            r#type: event_type.clone(),
            datacontenttype: Some("application/json".to_string()),
            subject: Some(nats_subject.clone()), // This will be used as NATS subject
            time: chrono::Utc::now(),
            data: Some(serde_json::json!({ "message": "hello from push test" })),
            correlation_id: None,
            causation_id: None,
        };

        nats.publish_event(&original_event)
            .await
            .expect("Failed to publish test event");
        tracing::info!("Published event {} to NATS subject {}", original_event.id, nats_subject);

        match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
            Ok(Some(received_event)) => {
                assert_eq!(received_event.id, original_event.id);
                assert_eq!(received_event.r#type, original_event.r#type);
                assert_eq!(received_event.data, original_event.data);
                assert_eq!(received_count.load(Ordering::SeqCst), 1);
                tracing::info!("Successfully received event {} via push consumer", received_event.id);
            }
            Ok(None) => panic!("Subscriber channel closed prematurely"),
            Err(_) => panic!("Timed out waiting for event from push subscriber"),
        }

        if let Err(e) = nats.jetstream.delete_stream(&stream_name).await {
            tracing::warn!("Failed to delete test stream {}: {}", stream_name, e);
        }
    }

    #[tokio::test]
    async fn test_publish_and_pull_subscribe() {
        let nats = match init_test_nats().await {
            Some(n) => n,
            None => return,
        };

        let stream_name = format!("test_pull_stream_{}", Uuid::new_v4());
        let nats_subject = format!("test.pull.subject.{}", Uuid::new_v4());
        let event_type = "com.helix.test.pull_event".to_string();
        let durable_consumer_name = format!("pull_consumer_{}", Uuid::new_v4());

        let stream_config = StreamConfig {
            name: stream_name.clone(),
            subjects: vec![nats_subject.clone()], // Exact subject match for this test
            max_messages: Some(10),
            max_age: Some(Duration::from_secs(60)),
        };
        nats.ensure_stream(&stream_config)
            .await
            .expect("Failed to ensure test stream for pull subscribe");

        let original_event = Event {
            id: Uuid::new_v4(),
            source: "test_publisher_pull".to_string(),
            specversion: "1.0".to_string(),
            r#type: event_type.clone(),
            datacontenttype: Some("application/json".to_string()),
            subject: Some(nats_subject.clone()), // NATS subject for publishing
            time: chrono::Utc::now(),
            data: Some(serde_json::json!({ "message": "hello from pull test" })),
            correlation_id: None,
            causation_id: None,
        };

        nats.publish_event(&original_event)
            .await
            .expect("Failed to publish test event for pull");
        tracing::info!("Published event {} for pull consumer", original_event.id);

        // Create pull consumer
        let consumer = nats
            .create_or_get_pull_consumer(&stream_name, &durable_consumer_name, Some(nats_subject.clone()))
            .await
            .expect("Failed to create pull consumer");

        // Fetch messages
        // Allow some time for message to be available in stream
        tokio::time::sleep(Duration::from_millis(200)).await;

        let messages = nats
            .fetch_messages(&consumer, 5)
            .await
            .expect("Failed to fetch messages");

        assert_eq!(messages.len(), 1, "Expected one message to be fetched");
        let msg = messages.first().unwrap();

        let received_event: Event = serde_json::from_slice(&msg.payload)
            .expect("Failed to deserialize fetched message into Event");

        assert_eq!(received_event.id, original_event.id);
        assert_eq!(received_event.r#type, original_event.r#type);
        assert_eq!(received_event.data, original_event.data);
        tracing::info!("Successfully received event {} via pull consumer", received_event.id);

        msg.ack().await.expect("Failed to ACK message");

        // Try fetching again, should be no new messages
        let messages_after_ack = nats
            .fetch_messages(&consumer, 5)
            .await
            .expect("Failed to fetch messages after ack");
         assert!(messages_after_ack.is_empty(), "Expected no messages after ack and fetch");


        if let Err(e) = nats.jetstream.delete_stream(&stream_name).await {
            tracing::warn!("Failed to delete test stream {}: {}", stream_name, e);
        }
    }

    #[tokio::test]
    async fn test_inmemory_event_collector() {
        let collector = InMemoryEventCollector::new();
        let agent_id_val = Uuid::new_v4(); // Use Uuid directly for AgentId
        let agent_id = AgentId(agent_id_val);
        let payload = serde_json::json!({"data": "test data"});
        let event_type = "test.inmemory.event".to_string();

        collector.publish_event(&agent_id, payload.clone(), Some(event_type.clone()))
            .await
            .expect("Failed to publish to InMemoryEventCollector");

        let events = collector.get_events().await;
        assert_eq!(events.len(), 1);
        let event = &events[0];
        // The InMemoryEventCollector creates a helix_core::Event.
        // Check the source field which we construct with agent_id.
        assert_eq!(event.source, format!("agent:{}", agent_id.to_string()));
        assert_eq!(event.r#type, event_type);
        assert_eq!(event.data, Some(payload));

        // Check if it's empty after get_events
        let events_after_get = collector.get_events().await;
        assert!(events_after_get.is_empty());
    }

    // Remove the old test_event_messaging or adapt it if still needed.
    // For now, it's removed as its functionality is covered by the new tests.
}
