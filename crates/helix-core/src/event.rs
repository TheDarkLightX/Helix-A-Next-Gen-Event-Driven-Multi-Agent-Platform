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
        assert_eq!(event.data.as_ref().unwrap(), &data);
        assert!(event.datacontenttype.is_some());
        assert_eq!(event.datacontenttype.as_ref().unwrap(), "application/json");
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

    #[test]
    fn test_event_with_all_fields() {
        let id = Uuid::new_v4();
        let correlation_id = Uuid::new_v4();
        let causation_id = Uuid::new_v4();
        let time = Utc::now();
        let data = json!({
            "user_id": 12345,
            "action": "login",
            "metadata": {
                "ip": "192.168.1.1",
                "user_agent": "Mozilla/5.0"
            }
        });

        let event = Event {
            id,
            source: "https://api.example.com/auth".to_string(),
            specversion: "1.0".to_string(),
            r#type: "com.example.user.login".to_string(),
            datacontenttype: Some("application/json".to_string()),
            subject: Some("user/12345".to_string()),
            time,
            data: Some(data.clone()),
            correlation_id: Some(correlation_id),
            causation_id: Some(causation_id),
        };

        // Verify all fields are set correctly
        assert_eq!(event.id, id);
        assert_eq!(event.source, "https://api.example.com/auth");
        assert_eq!(event.r#type, "com.example.user.login");
        assert_eq!(event.datacontenttype, Some("application/json".to_string()));
        assert_eq!(event.subject, Some("user/12345".to_string()));
        assert_eq!(event.time, time);
        assert_eq!(event.data, Some(data));
        assert_eq!(event.correlation_id, Some(correlation_id));
        assert_eq!(event.causation_id, Some(causation_id));
    }

    #[test]
    fn test_event_new_constructor() {
        let source = "test-source".to_string();
        let event_type = "test.event.type".to_string();
        let data = json!({"test": "data"});

        let event = Event::new(source.clone(), event_type.clone(), Some(data.clone()));

        assert_eq!(event.source, source);
        assert_eq!(event.r#type, event_type);
        assert_eq!(event.specversion, "1.0");
        assert_eq!(event.data, Some(data));
        assert_eq!(event.datacontenttype, Some("application/json".to_string()));
        assert!(event.subject.is_none());
        assert!(event.correlation_id.is_none());
        assert!(event.causation_id.is_none());
        assert!(!event.id.is_nil());
        assert!(event.time <= Utc::now());
    }

    #[test]
    fn test_event_new_without_data() {
        let source = "test-source".to_string();
        let event_type = "test.event.empty".to_string();

        let event = Event::new(source.clone(), event_type.clone(), None);

        assert_eq!(event.source, source);
        assert_eq!(event.r#type, event_type);
        assert!(event.data.is_none());
        assert!(event.datacontenttype.is_none());
    }

    #[test]
    fn test_event_serialization_roundtrip() {
        let original = Event::new(
            "test/source".to_string(),
            "test.roundtrip".to_string(),
            Some(json!({"complex": {"nested": [1, 2, 3]}})),
        );

        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: Event = serde_json::from_str(&serialized).unwrap();

        assert_eq!(original.id, deserialized.id);
        assert_eq!(original.source, deserialized.source);
        assert_eq!(original.r#type, deserialized.r#type);
        assert_eq!(original.data, deserialized.data);
        assert_eq!(original.datacontenttype, deserialized.datacontenttype);
    }

    #[test]
    fn test_event_with_unicode_data() {
        let unicode_data = json!({
            "message": "Hello 世界! 🌍",
            "emoji": "🚀🎉💯",
            "chinese": "你好世界",
            "japanese": "こんにちは世界",
            "arabic": "مرحبا بالعالم"
        });

        let event = Event::new(
            "unicode/test".to_string(),
            "test.unicode".to_string(),
            Some(unicode_data.clone()),
        );

        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: Event = serde_json::from_str(&serialized).unwrap();

        assert_eq!(event.data, deserialized.data);
        assert_eq!(deserialized.data, Some(unicode_data));
    }

    #[test]
    fn test_event_with_large_data() {
        let large_array: Vec<i32> = (0..2000).collect(); // Increased size
        let large_data = json!({
            "large_array": large_array,
            "repeated_string": "x".repeat(10000), // Increased size
            "additional_data": "y".repeat(5000)   // Added more data
        });

        let event = Event::new(
            "large/data/test".to_string(),
            "test.large.data".to_string(),
            Some(large_data.clone()),
        );

        let serialized = serde_json::to_string(&event).unwrap();
        assert!(serialized.len() > 10000); // Should be quite large

        let deserialized: Event = serde_json::from_str(&serialized).unwrap();
        assert_eq!(event.data, deserialized.data);
    }

    #[test]
    fn test_event_edge_case_values() {
        let edge_cases = vec![
            ("", "empty.source"),
            ("source", ""),
            ("source with spaces", "type.with.spaces"),
            ("source/with/slashes", "type/with/slashes"),
            ("source-with-dashes", "type-with-dashes"),
            ("source_with_underscores", "type_with_underscores"),
            ("source.with.dots", "type.with.dots"),
        ];

        for (source, event_type) in edge_cases {
            let event = Event::new(
                source.to_string(),
                event_type.to_string(),
                Some(json!({"test": "value"})),
            );

            assert_eq!(event.source, source);
            assert_eq!(event.r#type, event_type);

            // Ensure it can be serialized and deserialized
            let serialized = serde_json::to_string(&event).unwrap();
            let deserialized: Event = serde_json::from_str(&serialized).unwrap();
            assert_eq!(event.source, deserialized.source);
            assert_eq!(event.r#type, deserialized.r#type);
        }
    }

    #[test]
    fn test_event_json_special_characters() {
        let special_data = json!({
            "quotes": "String with \"quotes\"",
            "backslashes": "Path\\to\\file",
            "newlines": "Line 1\nLine 2\nLine 3",
            "tabs": "Column1\tColumn2\tColumn3",
            "control_chars": "\u{0001}\u{0002}\u{0003}",
            "null_char": "Before\u{0000}After"
        });

        let event = Event::new(
            "special/chars".to_string(),
            "test.special.chars".to_string(),
            Some(special_data.clone()),
        );

        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: Event = serde_json::from_str(&serialized).unwrap();

        assert_eq!(event.data, deserialized.data);
    }

    #[test]
    fn test_event_boundary_timestamps() {
        // Test with very old timestamp
        let old_time = DateTime::parse_from_rfc3339("1970-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let mut event = Event::new(
            "time/test".to_string(),
            "test.old.time".to_string(),
            None,
        );
        event.time = old_time;

        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: Event = serde_json::from_str(&serialized).unwrap();
        assert_eq!(event.time, deserialized.time);

        // Test with far future timestamp
        let future_time = DateTime::parse_from_rfc3339("2099-12-31T23:59:59Z")
            .unwrap()
            .with_timezone(&Utc);

        event.time = future_time;
        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: Event = serde_json::from_str(&serialized).unwrap();
        assert_eq!(event.time, deserialized.time);
    }

    #[test]
    fn test_event_uuid_edge_cases() {
        // Test with nil UUID
        let nil_uuid = Uuid::nil();
        let mut event = Event::new(
            "uuid/test".to_string(),
            "test.nil.uuid".to_string(),
            None,
        );
        event.id = nil_uuid;
        event.correlation_id = Some(nil_uuid);
        event.causation_id = Some(nil_uuid);

        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: Event = serde_json::from_str(&serialized).unwrap();
        assert_eq!(event.id, deserialized.id);
        assert_eq!(event.correlation_id, deserialized.correlation_id);
        assert_eq!(event.causation_id, deserialized.causation_id);

        // Test with max UUID
        let max_uuid = Uuid::from_bytes([255; 16]);
        event.id = max_uuid;
        event.correlation_id = Some(max_uuid);
        event.causation_id = Some(max_uuid);

        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: Event = serde_json::from_str(&serialized).unwrap();
        assert_eq!(event.id, deserialized.id);
        assert_eq!(event.correlation_id, deserialized.correlation_id);
        assert_eq!(event.causation_id, deserialized.causation_id);
    }

    #[test]
    fn test_event_data_types() {
        let test_cases = vec![
            ("boolean_true", json!(true)),
            ("boolean_false", json!(false)),
            ("integer_zero", json!(0)),
            ("integer_positive", json!(42)),
            ("integer_negative", json!(-42)),
            ("integer_max", json!(i64::MAX)),
            ("integer_min", json!(i64::MIN)),
            ("float_zero", json!(0.0)),
            ("float_positive", json!(std::f64::consts::PI)),
            ("float_negative", json!(-std::f64::consts::E)),
            ("string_empty", json!("")),
            ("array_empty", json!([])),
            ("object_empty", json!({})),
            ("array_mixed", json!([1, "two", true, null, {"nested": "object"}])),
        ];

        for (name, data) in test_cases {
            let event = Event::new(
                format!("data/type/{}", name),
                format!("test.data.{}", name),
                Some(data.clone()),
            );

            let serialized = serde_json::to_string(&event).unwrap();
            let deserialized: Event = serde_json::from_str(&serialized).unwrap();
            assert_eq!(event.data, deserialized.data, "Failed for test case: {}", name);
        }

        // Test null case separately since Some(json!(null)) behaves differently
        let null_event = Event::new(
            "data/type/null".to_string(),
            "test.data.null".to_string(),
            None, // Use None instead of Some(json!(null))
        );

        let serialized = serde_json::to_string(&null_event).unwrap();
        let deserialized: Event = serde_json::from_str(&serialized).unwrap();
        assert_eq!(null_event.data, deserialized.data);
        assert!(deserialized.data.is_none());
    }

    #[test]
    fn test_default_spec_version() {
        assert_eq!(default_spec_version(), "1.0");
    }

    #[test]
    fn test_event_clone() {
        let original = Event::new(
            "clone/test".to_string(),
            "test.clone".to_string(),
            Some(json!({"cloned": true})),
        );

        let cloned = original.clone();

        assert_eq!(original.id, cloned.id);
        assert_eq!(original.source, cloned.source);
        assert_eq!(original.r#type, cloned.r#type);
        assert_eq!(original.data, cloned.data);
        assert_eq!(original.time, cloned.time);
    }

    #[test]
    fn test_event_debug_format() {
        let event = Event::new(
            "debug/test".to_string(),
            "test.debug".to_string(),
            Some(json!({"debug": "info"})),
        );

        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("Event"));
        assert!(debug_str.contains("debug/test"));
        assert!(debug_str.contains("test.debug"));
    }
}
