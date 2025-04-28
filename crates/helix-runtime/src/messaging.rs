#![warn(missing_docs)]

//! Handles messaging infrastructure, primarily NATS JetStream.

use async_nats::jetstream::{self, Context as JetStreamContext};
use async_nats::Client;
use futures::stream::TryStreamExt;
use helix_core::event::Event;
use std::time::Duration;
use thiserror::Error;
use tokio;

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

    /// Publishes an event to a specific subject.
    pub async fn publish_event(&self, subject: &str, event: &Event) -> Result<(), MessagingError> {
        let payload = serde_json::to_vec(event)
            .map_err(|e| MessagingError::SerializationError(e.to_string()))?;

        tracing::debug!(subject = subject, event_id = %event.id, "Publishing event");
        self.jetstream
            .publish(subject.to_string(), payload.into())
            .await
            .map_err(|e| MessagingError::PublishError(e.to_string()))?;
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

    /// Subscribes to a stream and processes incoming events.
    pub async fn subscribe_to_stream<F>(
        &self,
        stream: &str,
        callback: F,
    ) -> Result<(), MessagingError>
    where
        F: Fn(Event) + Send + Sync + 'static,
    {
        tracing::info!(stream = stream, "Subscribing to stream");

        let stream_ctx = self
            .jetstream
            .get_stream(stream)
            .await
            .map_err(|e| MessagingError::StreamAccessError(e.to_string()))?;

        let consumer = stream_ctx
            .create_consumer(jetstream::consumer::push::Config {
                durable_name: Some(format!("{}_consumer", stream)),
                ..Default::default()
            })
            .await
            .map_err(|e| MessagingError::SubscriptionError(e.to_string()))?;

        let mut messages = consumer
            .messages()
            .await
            .map_err(|e| MessagingError::SubscriptionError(e.to_string()))?;

        let _handle = tokio::spawn(async move {
            while let Ok(Some(message)) = messages.try_next().await {
                match serde_json::from_slice::<Event>(&message.payload) {
                    Ok(event) => {
                        tracing::debug!(event_id = %event.id, "Processing event");
                        callback(event);
                        if let Err(e) = message.ack().await {
                            tracing::error!("Failed to ACK message: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to deserialize event payload: {}", e);
                        if let Err(ack_err) = message.ack().await {
                            tracing::error!("Failed to ACK bad message: {}", ack_err);
                        }
                    }
                }
            }
            tracing::info!("Subscription message stream ended.");
        });

        Ok(())
    }

    /// Creates a pull-based subscription to a stream.
    pub async fn create_pull_subscription(
        &self,
        stream_name: &str,
        consumer_name: &str,
    ) -> Result<jetstream::consumer::PullConsumer, MessagingError> {
        tracing::info!(
            stream = stream_name,
            consumer = consumer_name,
            "Creating pull subscription"
        );

        let stream_ctx = self
            .jetstream
            .get_stream(stream_name)
            .await
            .map_err(|e| MessagingError::StreamAccessError(e.to_string()))?;

        let consumer = stream_ctx
            .create_consumer(jetstream::consumer::pull::Config {
                name: Some(consumer_name.to_string()),
                durable_name: Some(consumer_name.to_string()),
                ..Default::default()
            })
            .await
            .map_err(|e| MessagingError::SubscriptionError(e.to_string()))?;

        Ok(consumer)
    }

    /// Fetches messages from a pull subscription.
    pub async fn fetch_messages(
        &self,
        consumer: &jetstream::consumer::PullConsumer,
        batch_size: u32,
    ) -> Result<Vec<async_nats::jetstream::Message>, MessagingError> {
        tracing::debug!(batch_size, "Fetching messages");

        // Corrected fetch builder pattern: configure -> .messages() -> await -> map_err -> await collection
        let fetch_builder = consumer.fetch().max_messages(batch_size as usize);

        // Await the future returned by .messages(), then map the error
        let messages_stream = fetch_builder
            .messages()
            .await // Await the future first
            .map_err(|e| MessagingError::FetchError(e.to_string()))?; // Then map the error on the Result

        // Collect messages from the stream
        let messages = messages_stream
            .try_collect()
            .await
            .map_err(|e| MessagingError::FetchError(e.to_string()))?; // Map the collection error

        Ok(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_event_messaging() {
        let config = NatsConfig::default();
        let nats_result = NatsClient::connect(&config).await;
        if nats_result.is_err() {
            eprintln!(
                "NATS connection failed, skipping test_event_messaging. Is NATS running at {}?",
                config.urls
            );
            return;
        }
        let nats = nats_result.unwrap();

        let stream_name = format!("test_stream_{}", uuid::Uuid::new_v4());
        let subject_name = format!("test.{}", uuid::Uuid::new_v4());

        let stream_config = NatsClient::StreamConfig {
            name: stream_name.clone(),
            subjects: vec![format!("{}*", subject_name)],
            max_messages: Some(10),
            max_age: Some(Duration::from_secs(60)),
        };

        nats.ensure_stream(&stream_config)
            .await
            .expect("Failed to ensure test stream");

        let source_agent_id = uuid::Uuid::new_v4();
        let profile_id = uuid::Uuid::new_v4();
        let event_type = "test.runtime.event".to_string();
        let payload = serde_json::json!({ "data": "hello from runtime test" });

        let event = Event::new(source_agent_id, profile_id, event_type, payload)
            .with_subject(&subject_name);

        nats.publish_event(&subject_name, &event)
            .await
            .expect("Failed to publish test event");

        println!("Published event {} to subject {}", event.id, subject_name);

        if let Err(e) = nats.jetstream.delete_stream(&stream_name).await {
            tracing::warn!("Failed to delete test stream {}: {}", stream_name, e);
        }
    }
}
