#![warn(missing_docs)]

//! Handles messaging infrastructure, primarily NATS JetStream.

use async_nats::jetstream::{self, Context as JetStreamContext};
use async_nats::Client;
use thiserror::Error;
use chrono;
use serde::{Serialize, Deserialize};
use serde_json;
use tokio;

#[derive(Error, Debug)]
pub enum MessagingError {
    #[error("NATS connection failed: {0}")]
    NatsConnection(#[from] async_nats::ConnectError),
    #[error("NATS request failed: {0}")]
    NatsRequest(#[from] async_nats::RequestError),
    #[error("Failed to get JetStream context: {0}")]
    JetStreamContext(std::io::Error), // Often IO error from NATS client
    // TODO: Add more specific errors (e.g., publish error, subscribe error)
}

/// Configuration for the NATS client.
#[derive(Debug, Clone)] // TODO: Load from config file/env vars
pub struct NatsConfig {
    pub urls: String, // Comma-separated list of NATS server URLs
    // TODO: Add authentication options (token, nkey, user/pass)
}

impl Default for NatsConfig {
    fn default() -> Self {
        Self {
            urls: "nats://localhost:4222".to_string(),
        }
    }
}

/// Represents the connection to the NATS messaging system.
#[derive(Clone)]
pub struct NatsClient {
    pub client: Client,
    pub jetstream: JetStreamContext,
}

impl NatsClient {
    /// Connects to NATS and initializes the JetStream context.
    pub async fn connect(config: &NatsConfig) -> Result<Self, MessagingError> {
        tracing::info!(urls = %config.urls, "Connecting to NATS...");
        let client = async_nats::connect(&config.urls).await?;
        tracing::info!("Connected to NATS successfully.");

        let jetstream = jetstream::new(client.clone());
        tracing::info!("JetStream context created.");

        Ok(Self {
            client,
            jetstream,
        })
    }

    /// Represents an event in the system.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Event {
        pub id: String,
        pub timestamp: chrono::DateTime<chrono::Utc>,
        pub agent_id: String,
        pub kind: String,
        pub payload: serde_json::Value,
    }

    /// Configuration for a JetStream stream.
    #[derive(Debug, Clone)]
    pub struct StreamConfig {
        pub name: String,
        pub subjects: Vec<String>,
        pub max_messages: Option<u64>,
        pub max_age: Option<std::time::Duration>,
    }

    /// Publishes an event to a specific subject.
    pub async fn publish_event(&self, subject: &str, event: &Event) -> Result<(), MessagingError> {
        let payload = serde_json::to_vec(event)
            .map_err(|e| MessagingError::JetStreamContext(e.into()))?;
        
        tracing::debug!(subject, "Publishing event");
        self.jetstream.publish(subject, payload).await?;
        Ok(())
    }

    /// Creates a JetStream stream if it doesn't exist.
    pub async fn ensure_stream(&self, config: &StreamConfig) -> Result<(), MessagingError> {
        tracing::debug!(stream = %config.name, "Ensuring stream exists");
        
        let stream = self.jetstream.stream(config.name.clone()).await;
        if stream.is_err() {
            tracing::info!(stream = %config.name, "Creating stream");
            self.jetstream
                .add_stream(
                    jetstream::stream::Config {
                        name: config.name.clone(),
                        subjects: config.subjects.clone(),
                        max_messages: config.max_messages,
                        max_age: config.max_age.map(|d| d.as_secs()),
                        ..Default::default()
                    },
                )
                .await?;
        }
        Ok(())
    }

    /// Subscribes to a stream and processes incoming events.
    pub async fn subscribe_to_stream<F>(&self, stream: &str, callback: F) -> Result<(), MessagingError>
    where
        F: Fn(Event) + Send + Sync + 'static,
    {
        tracing::info!(stream, "Subscribing to stream");
        
        let consumer = self.jetstream.stream(stream).await?;
        let subscription = consumer.subscribe().await?;

        let handle = tokio::spawn(async move {
            while let Ok(message) = subscription.next().await {
                let event: Event = serde_json::from_slice(&message.payload)
                    .map_err(|e| MessagingError::JetStreamContext(e.into()))?;
                    
                tracing::debug!(event_id = %event.id, "Processing event");
                callback(event);
                message.ack().await?;
            }
            Ok::<(), MessagingError>(())
        });

        // Store the handle somewhere if you want to be able to cancel it later
        // For now, we'll just let it run
        Ok(())
    }

    /// Creates a pull-based subscription to a stream.
    pub async fn create_pull_subscription(
        &self,
        stream: &str,
        consumer_name: &str,
    ) -> Result<jetstream::consumer::PullConsumer, MessagingError> {
        tracing::info!(stream, consumer = consumer_name, "Creating pull subscription");
        
        let consumer = self.jetstream
            .consumer(
                stream,
                jetstream::consumer::pull::Config {
                    name: Some(consumer_name.to_string()),
                    ..Default::default()
                },
            )
            .await?;

        Ok(consumer)
    }

    /// Fetches messages from a pull subscription.
    pub async fn fetch_messages(
        &self,
        consumer: &jetstream::consumer::PullConsumer,
        batch_size: u32,
    ) -> Result<Vec<jetstream::consumer::Message>, MessagingError> {
        tracing::debug!(batch_size, "Fetching messages");
        
        let messages = consumer.fetch(batch_size).await?;
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
        let nats = NatsClient::connect(&config).await.unwrap();

        let stream_config = NatsClient::StreamConfig {
            name: "test_stream".to_string(),
            subjects: vec!["test.*".to_string()],
            max_messages: Some(1000),
            max_age: Some(Duration::from_secs(3600)),
        };

        nats.ensure_stream(&stream_config).await.unwrap();

        let event = NatsClient::Event {
            id: "test_event_1".to_string(),
            timestamp: chrono::Utc::now(),
            agent_id: "test_agent".to_string(),
            kind: "test_event".to_string(),
            payload: serde_json::json!({ "data": "test" }),
        };

        nats.publish_event("test.event", &event).await.unwrap();
        
        // Add more tests for subscription and pull-based consumption
    }
}
