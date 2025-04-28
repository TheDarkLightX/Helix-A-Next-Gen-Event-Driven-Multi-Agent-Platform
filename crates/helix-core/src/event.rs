//! Defines the standard structure for events transmitted between agents.

use crate::types::{AgentId, EventId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// Represents a unit of data flowing between agents in a Recipe.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    /// Unique identifier for this specific event instance.
    #[serde(default = "Uuid::new_v4")]
    pub id: EventId,
    /// The ID of the agent that originally emitted this event.
    pub source_agent_id: AgentId,
    /// Timestamp indicating when the event occurred (according to the source agent).
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,
    /// The data payload carried by the event.
    /// Structure is determined by the source agent and expected by target agents.
    pub payload: JsonValue,
    // TODO: Add correlation ID for tracing event flow across multiple recipes?
    // TODO: Add metadata field (Map<String, String>)? (e.g., content-type, source URI)
}

impl Event {
    /// Creates a new Event instance.
    pub fn new(source_agent_id: AgentId, payload: JsonValue) -> Self {
        Self {
            id: Uuid::new_v4(),
            source_agent_id,
            timestamp: Utc::now(),
            payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_event_creation() {
        let agent_id = Uuid::new_v4();
        let payload_data = json!({"message": "Hello, world!", "value": 42});
        let event = Event::new(agent_id, payload_data.clone());

        assert_eq!(event.source_agent_id, agent_id);
        assert_eq!(event.payload, payload_data);
        // Basic check for timestamp (should be recent)
        assert!(Utc::now().signed_duration_since(event.timestamp).num_seconds() < 5);
    }

    #[test]
    fn test_event_serialization_deserialization() {
        let agent_id = Uuid::new_v4();
        let payload_data = json!({"data": [1, 2, 3], "valid": true});
        let event = Event::new(agent_id, payload_data.clone());

        let serialized = serde_json::to_string(&event).expect("Failed to serialize event");
        let deserialized: Event = 
            serde_json::from_str(&serialized).expect("Failed to deserialize event");

        assert_eq!(event, deserialized);
        assert_eq!(deserialized.source_agent_id, agent_id);
        assert_eq!(deserialized.payload, payload_data);
    }
}
