//! Defines the core Event structure used throughout the Helix system.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Represents an event that occurs within the Helix system or is ingested from external sources.
///
/// This structure is inspired by the CloudEvents specification (v1.0) to promote interoperability.
/// See: https://github.com/cloudevents/spec/blob/v1.0.2/cloudevents/spec.md
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Event {
    /// Unique identifier for this event. Typically a UUID.
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    /// Identifies the context in which an event happened.
    /// Often a URI, URN, or other identifier unique to the source.
    /// Example: `https://github.com/repos/my-org/my-repo`, `/my-app/my-service`.
    pub source: String,

    /// The version of the CloudEvents specification which the event uses.
    /// Defaults to "1.0".
    #[serde(default = "default_spec_version")]
    pub specversion: String,

    /// Describes the type of event related to the originating occurrence.
    /// Often uses a reverse-DNS style name.
    /// Example: `com.github.pull_request.opened`, `com.example.user.created`.
    pub r#type: String, // Use raw identifier `r#` because `type` is a keyword

    /// Optional. Content type of the `data` value (e.g., "application/json").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datacontenttype: Option<String>,

    /// Optional. Describes the subject of the event in the context of the event producer (source).
    /// Example: The specific resource URI being acted upon.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,

    /// Timestamp of when the occurrence happened (UTC).
    #[serde(default = "Utc::now")]
    pub time: DateTime<Utc>,

    /// Optional. The event payload.
    /// The format is described by `datacontenttype`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,

    /// Optional. Custom extension attribute for correlating events across different contexts or systems.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<Uuid>,

    /// Optional. Custom extension attribute identifying the event that caused this event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<Uuid>,
}

/// Helper function to provide default specversion.
fn default_spec_version() -> String {
    "1.0".to_string()
}

impl Event {
    /// Creates a new basic event with minimal required fields.
    ///
    /// # Arguments
    ///
    /// * `source` - The source identifier.
    /// * `event_type` - The type identifier.
    /// * `data` - The optional event payload.
    ///
    /// # Returns
    ///
    /// A new `Event` instance with generated ID and current timestamp.
    pub fn new(source: String, event_type: String, data: Option<Value>) -> Self {
        Event {
            id: Uuid::new_v4(),
            source,
            specversion: default_spec_version(),
            r#type: event_type,
            datacontenttype: data.as_ref().map(|_| "application/json".to_string()),
            subject: None,
            time: Utc::now(),
            data,
            correlation_id: None,
            causation_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn event_creation_and_serialization() {
        let source = "/test/source".to_string();
        let event_type = "com.example.test".to_string();
        let data = json!({ "key": "value" });

        let event = Event::new(source.clone(), event_type.clone(), Some(data.clone()));

        assert_eq!(event.source, source);
        assert_eq!(event.r#type, event_type);
        assert_eq!(event.specversion, "1.0");
        assert!(event.data.is_some());
        assert_eq!(event.data.unwrap(), data);
        assert!(event.datacontenttype.is_some());
        assert_eq!(event.datacontenttype.unwrap(), "application/json");
        assert!(event.time <= Utc::now());
        assert!(!event.id.is_nil());

        // Test serialization
        let serialized = serde_json::to_string(&event).expect("Failed to serialize");
        println!("Serialized Event: {}", serialized); // For debugging
        assert!(serialized.contains(&source));
        assert!(serialized.contains(&event_type));
        assert!(serialized.contains("\"key\":\"value\"")); // Check data serialization

        // Test deserialization
        let deserialized: Event = serde_json::from_str(&serialized).expect("Failed to deserialize");
        assert_eq!(deserialized.id, event.id);
        assert_eq!(deserialized.source, event.source);
        assert_eq!(deserialized.r#type, event.r#type);
        assert_eq!(deserialized.data, Some(data));
    }

    #[test]
    fn event_with_optional_fields() {
        let event = Event {
            id: Uuid::new_v4(),
            source: "/another/source".to_string(),
            specversion: "1.0".to_string(),
            r#type: "com.example.minimal".to_string(),
            datacontenttype: None,
            subject: Some("test-subject".to_string()),
            time: Utc::now(),
            data: None,
            correlation_id: Some(Uuid::new_v4()),
            causation_id: None,
        };

        assert!(event.data.is_none());
        assert!(event.datacontenttype.is_none());
        assert!(event.subject.is_some());
        assert!(event.correlation_id.is_some());
        assert!(event.causation_id.is_none());

        let serialized = serde_json::to_string(&event).expect("Failed to serialize");
        println!("Serialized Minimal Event: {}", serialized); // For debugging
        assert!(!serialized.contains("\"data\":")); // Ensure optional fields are omitted when None
        assert!(serialized.contains("\"subject\":\"test-subject\""));

        let deserialized: Event = serde_json::from_str(&serialized).expect("Failed to deserialize");
        assert_eq!(deserialized.id, event.id);
        assert_eq!(deserialized.subject, event.subject);
        assert_eq!(deserialized.correlation_id, event.correlation_id);
        assert!(deserialized.data.is_none());
    }
}
