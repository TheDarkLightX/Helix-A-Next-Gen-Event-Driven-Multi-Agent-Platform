//! Defines the standard structure for events transmitted between agents.

use crate::types::{AgentId, EventId, ProfileId}; // Ensure ProfileId is imported
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// Represents an event flowing through the Helix system.
/// Inspired by the CloudEvents specification (https://cloudevents.io/)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    /// Unique identifier for this event instance. (CloudEvents: id)
    #[serde(default = "Uuid::new_v4")]
    pub id: EventId,

    /// Identifier of the agent that originally produced the event. (CloudEvents: source - mapped conceptually)
    pub source_agent_id: AgentId,

    /// Identifier of the user profile context associated with this event. Crucial for multi-tenancy.
    pub profile_id: ProfileId,

    /// Timestamp indicating when the event was generated. (CloudEvents: time)
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,

    /// Type of occurrence which has happened. Should be namespaced. (CloudEvents: type)
    /// Example: "github.push", "webhook.received", "schedule.tick"
    pub event_type: String,

    /// The actual event data/payload. (CloudEvents: data)
    pub payload: JsonValue,

    /// Optional: Subject of the event in the context of the source. (CloudEvents: subject)
    /// Example: A specific file name, user ID, resource identifier. Useful for partitioning/filtering.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,

    /// Optional: Correlation ID for tracking related events across a workflow/request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<Uuid>,

    /// Optional: ID of the event that caused this event to occur (for tracing causality).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<Uuid>,

    /// Optional: Identifier for the specific recipe run instance processing this event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipe_instance_id: Option<Uuid>,

    /// Optional: URI referencing the schema for the event payload. (CloudEvents: dataschema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_schema: Option<String>, // Using String for URI flexibility

    /// Version of the Helix Event specification which this event adheres to. (CloudEvents: specversion)
    #[serde(default = "default_spec_version")]
    pub spec_version: String,
}

// Helper for default spec_version
fn default_spec_version() -> String {
    "1.0".to_string()
}

impl Event {
    /// Creates a new minimal event with required fields.
    pub fn new(
        source_agent_id: AgentId,
        profile_id: ProfileId,
        event_type: String,
        payload: JsonValue,
    ) -> Self {
        Event {
            id: Uuid::new_v4(),
            source_agent_id,
            profile_id,
            created_at: Utc::now(),
            event_type,
            payload,
            subject: None,
            correlation_id: None,
            causation_id: None,
            recipe_instance_id: None,
            data_schema: None,
            spec_version: default_spec_version(),
        }
    }

    // --- Builder methods for optional fields ---

    /// Sets the subject of the event.
    pub fn with_subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    /// Sets the correlation ID for the event.
    pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    /// Sets the causation ID for the event.
    pub fn with_causation_id(mut self, causation_id: Uuid) -> Self {
        self.causation_id = Some(causation_id);
        self
    }

    /// Sets the recipe instance ID for the event.
    pub fn with_recipe_instance_id(mut self, recipe_instance_id: Uuid) -> Self {
        self.recipe_instance_id = Some(recipe_instance_id);
        self
    }

    /// Sets the data schema URI for the event payload.
    pub fn with_data_schema(mut self, data_schema: impl Into<String>) -> Self {
        self.data_schema = Some(data_schema.into());
        self
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn test_event_creation() {
        let source_agent_id = Uuid::new_v4();
        let profile_id = Uuid::new_v4();
        let event_type = "test.event".to_string();
        let payload = json!({ "data": "test_payload" });

        let event = Event::new(source_agent_id, profile_id, event_type.clone(), payload.clone());

        assert_eq!(event.source_agent_id, source_agent_id);
        assert_eq!(event.profile_id, profile_id);
        assert_eq!(event.event_type, event_type);
        assert_eq!(event.payload, payload);
        assert_eq!(event.spec_version, "1.0");
        assert!(event.subject.is_none());
        assert!(event.correlation_id.is_none());
        // Check created_at is recent (within a few seconds)
        let time_diff = Utc::now() - event.created_at;
        assert!(time_diff.num_seconds() >= 0 && time_diff.num_seconds() < 5);
    }

    #[test]
    fn test_event_creation_with_options() {
        let source_agent_id = Uuid::new_v4();
        let profile_id = Uuid::new_v4();
        let event_type = "test.event.opts".to_string();
        let payload = json!({ "data": 123 });
        let correlation_id = Uuid::new_v4();
        let causation_id = Uuid::new_v4();

        let event = Event::new(source_agent_id, profile_id, event_type.clone(), payload.clone())
            .with_subject("test-subject")
            .with_correlation_id(correlation_id)
            .with_causation_id(causation_id)
            .with_data_schema("schema://test");

        assert_eq!(event.source_agent_id, source_agent_id);
        assert_eq!(event.profile_id, profile_id);
        assert_eq!(event.event_type, event_type);
        assert_eq!(event.payload, payload);
        assert_eq!(event.subject, Some("test-subject".to_string()));
        assert_eq!(event.correlation_id, Some(correlation_id));
        assert_eq!(event.causation_id, Some(causation_id));
        assert_eq!(event.data_schema, Some("schema://test".to_string()));
        assert!(event.recipe_instance_id.is_none());
        assert_eq!(event.spec_version, "1.0");
    }

    #[test]
    fn test_event_serialization_deserialization() {
        let source_agent_id = Uuid::new_v4();
        let profile_id = Uuid::new_v4();
        let event_type = "serialization.test".to_string();
        let payload = json!({ "complex": [1, "two", { "three": true }] });
        let correlation_id = Uuid::new_v4();

        let original_event = Event::new(source_agent_id, profile_id, event_type, payload)
            .with_subject("serialize_me")
            .with_correlation_id(correlation_id);

        // Serialize
        let serialized = serde_json::to_string(&original_event).expect("Serialization failed");
        println!("Serialized: {}", serialized); // For debugging

        // Deserialize
        let deserialized_event: Event = serde_json::from_str(&serialized).expect("Deserialization failed");

        // Assert equality
        assert_eq!(original_event, deserialized_event);

        // Check that optional fields omitted in creation are None after deserialization
        let minimal_event = Event::new(Uuid::new_v4(), Uuid::new_v4(), "minimal".to_string(), json!({}));
        let minimal_serialized = serde_json::to_string(&minimal_event).unwrap();
        assert!(!minimal_serialized.contains("subject"));
        assert!(!minimal_serialized.contains("correlation_id"));
        let minimal_deserialized: Event = serde_json::from_str(&minimal_serialized).unwrap();
        assert!(minimal_deserialized.subject.is_none());
        assert!(minimal_deserialized.correlation_id.is_none());
    }
}
