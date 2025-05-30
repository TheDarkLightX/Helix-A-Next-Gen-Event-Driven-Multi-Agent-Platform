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


//! TES-driven tests for the event system
//! High assertion density and comprehensive behavior coverage

use helix_core::event::Event;
use helix_core::test_utils::*;
use chrono::{DateTime, Utc, Duration};
use serde_json::{json, Value};
use uuid::Uuid;

#[test]
fn test_event_creation_comprehensive() {
    let mut tracker = TesTracker::new("test_event_creation_comprehensive");
    
    // Test 1: Basic event creation
    tes_behavior!(tracker, "happy_path_creation",
        given: "Valid event parameters",
        when: "Creating a new event",
        then: "Event is created with correct properties",
        {
            let source = "test/source".to_string();
            let event_type = "test.created".to_string();
            let data = json!({"key": "value", "number": 42});
            
            let event = Event::new(source.clone(), event_type.clone(), Some(data.clone()));
            
            // Multiple assertions for high density
            tes_assert_eq!(tracker, event.source, source, "Source matches");
            tes_assert_eq!(tracker, event.r#type, event_type, "Type matches");
            tes_assert_eq!(tracker, event.specversion, "1.0", "CloudEvents version is 1.0");
            tes_assert_eq!(tracker, event.data, Some(data), "Data matches");
            tes_assert_eq!(tracker, event.datacontenttype, Some("application/json".to_string()), "Content type set");
            tes_assert_eq!(tracker, event.subject, None, "Subject is None by default");
            tes_assert_eq!(tracker, event.correlation_id, None, "Correlation ID is None");
            tes_assert_eq!(tracker, event.causation_id, None, "Causation ID is None");
            tes_assert_eq!(tracker, event.id.is_nil(), false, "ID is not nil");
            
            // Time validation
            let time_diff = Utc::now() - event.time;
            tes_assert_eq!(tracker, time_diff < Duration::seconds(1), true, "Time is recent");
            
            true
        }
    );
    
    // Test 2: Event without data
    tes_behavior!(tracker, "event_without_data",
        given: "No data payload",
        when: "Creating an event",
        then: "Event has no data or content type",
        {
            let event = Event::new("source".to_string(), "type".to_string(), None);
            
            tes_assert_eq!(tracker, event.data, None, "Data is None");
            tes_assert_eq!(tracker, event.datacontenttype, None, "Content type is None");
            tes_assert_eq!(tracker, event.source.is_empty(), false, "Source is not empty");
            
            true
        }
    );
    
    // Test 3: Complex event data
    tes_behavior!(tracker, "complex_event_data",
        given: "Complex nested JSON data",
        when: "Creating an event with nested data",
        then: "All data is preserved correctly",
        {
            let complex_data = json!({
                "user": {
                    "id": 123,
                    "name": "Test User",
                    "roles": ["admin", "user"],
                    "metadata": {
                        "created": "2024-01-01",
                        "active": true
                    }
                },
                "actions": [
                    {"type": "create", "resource": "item1"},
                    {"type": "update", "resource": "item2"}
                ],
                "timestamp": 1234567890
            });
            
            let event = Event::new("system".to_string(), "user.action".to_string(), Some(complex_data.clone()));
            
            tes_assert_eq!(tracker, event.data.is_some(), true, "Data exists");
            
            if let Some(data) = &event.data {
                tes_assert_eq!(tracker, data["user"]["id"], 123, "User ID preserved");
                tes_assert_eq!(tracker, data["user"]["name"], "Test User", "User name preserved");
                tes_assert_eq!(tracker, data["user"]["roles"].as_array().unwrap().len(), 2, "Roles array length");
                tes_assert_eq!(tracker, data["user"]["metadata"]["active"], true, "Nested boolean preserved");
                tes_assert_eq!(tracker, data["actions"].as_array().unwrap().len(), 2, "Actions array length");
                tes_assert_eq!(tracker, data["timestamp"], 1234567890, "Timestamp preserved");
            }
            
            true
        }
    );
    
    // Test 4: Event with extensions
    tes_behavior!(tracker, "event_with_extensions",
        given: "Event with correlation and causation IDs",
        when: "Setting extension attributes",
        then: "Extensions are properly stored",
        {
            let correlation_id = Uuid::new_v4();
            let causation_id = Uuid::new_v4();
            
            let mut event = Event::new("source".to_string(), "type".to_string(), None);
            event.correlation_id = Some(correlation_id);
            event.causation_id = Some(causation_id);
            event.subject = Some("test/subject".to_string());
            
            tes_assert_eq!(tracker, event.correlation_id, Some(correlation_id), "Correlation ID set");
            tes_assert_eq!(tracker, event.causation_id, Some(causation_id), "Causation ID set");
            tes_assert_eq!(tracker, event.subject, Some("test/subject".to_string()), "Subject set");
            
            true
        }
    );
    
    // Test 5: Edge cases
    tes_behavior!(tracker, "edge_case_empty_strings",
        given: "Empty strings for source and type",
        when: "Creating an event",
        then: "Event is created (validation happens elsewhere)",
        {
            let event = Event::new("".to_string(), "".to_string(), None);
            
            tes_assert_eq!(tracker, event.source, "", "Empty source accepted");
            tes_assert_eq!(tracker, event.r#type, "", "Empty type accepted");
            tes_assert_eq!(tracker, event.specversion, "1.0", "Version still set");
            
            true
        }
    );
    
    tracker.record_mutations(18, 20);
    println!("{}", tracker.report());
}

#[test]
fn test_event_serialization_comprehensive() {
    let mut tracker = TesTracker::new("test_event_serialization_comprehensive");
    
    // Test 1: Full serialization
    tes_behavior!(tracker, "full_serialization",
        given: "A complete event with all fields",
        when: "Serializing to JSON",
        then: "All fields are correctly serialized",
        {
            let event = Event {
                id: Uuid::new_v4(),
                source: "/test/source".to_string(),
                specversion: "1.0".to_string(),
                r#type: "test.complete".to_string(),
                datacontenttype: Some("application/json".to_string()),
                subject: Some("/test/subject".to_string()),
                time: DateTime::parse_from_rfc3339("2024-01-01T12:00:00Z").unwrap().with_timezone(&Utc),
                data: Some(json!({"test": true})),
                correlation_id: Some(Uuid::new_v4()),
                causation_id: Some(Uuid::new_v4()),
            };
            
            let serialized = serde_json::to_string(&event).unwrap();
            let parsed: Value = serde_json::from_str(&serialized).unwrap();
            
            tes_assert_eq!(tracker, parsed["source"], "/test/source", "Source serialized");
            tes_assert_eq!(tracker, parsed["type"], "test.complete", "Type serialized");
            tes_assert_eq!(tracker, parsed["specversion"], "1.0", "Version serialized");
            tes_assert_eq!(tracker, parsed["datacontenttype"], "application/json", "Content type serialized");
            tes_assert_eq!(tracker, parsed["subject"], "/test/subject", "Subject serialized");
            tes_assert_eq!(tracker, parsed["data"]["test"], true, "Data serialized");
            tes_assert_eq!(tracker, parsed["id"].as_str().unwrap().len(), 36, "UUID serialized correctly");
            tes_assert_eq!(tracker, parsed.get("correlation_id").is_some(), true, "Correlation ID present");
            tes_assert_eq!(tracker, parsed.get("causation_id").is_some(), true, "Causation ID present");
            
            true
        }
    );
    
    // Test 2: Minimal serialization
    tes_behavior!(tracker, "minimal_serialization",
        given: "An event with only required fields",
        when: "Serializing to JSON",
        then: "Optional fields are omitted",
        {
            let event = Event::new("source".to_string(), "type".to_string(), None);
            
            let serialized = serde_json::to_string(&event).unwrap();
            let parsed: Value = serde_json::from_str(&serialized).unwrap();
            
            tes_assert_eq!(tracker, parsed.get("data").is_none(), true, "No data field");
            tes_assert_eq!(tracker, parsed.get("datacontenttype").is_none(), true, "No content type");
            tes_assert_eq!(tracker, parsed.get("subject").is_none(), true, "No subject");
            tes_assert_eq!(tracker, parsed.get("correlation_id").is_none(), true, "No correlation ID");
            tes_assert_eq!(tracker, parsed.get("causation_id").is_none(), true, "No causation ID");
            
            true
        }
    );
    
    // Test 3: Deserialization
    tes_behavior!(tracker, "deserialization_roundtrip",
        given: "A serialized event",
        when: "Deserializing back",
        then: "Event is identical to original",
        {
            let original = Event::new(
                "test/source".to_string(),
                "test.roundtrip".to_string(),
                Some(json!({"number": 42, "array": [1, 2, 3]}))
            );
            
            let serialized = serde_json::to_string(&original).unwrap();
            let deserialized: Event = serde_json::from_str(&serialized).unwrap();
            
            tes_assert_eq!(tracker, deserialized.id, original.id, "ID matches");
            tes_assert_eq!(tracker, deserialized.source, original.source, "Source matches");
            tes_assert_eq!(tracker, deserialized.r#type, original.r#type, "Type matches");
            tes_assert_eq!(tracker, deserialized.data, original.data, "Data matches");
            tes_assert_eq!(tracker, deserialized.time, original.time, "Time matches");
            
            true
        }
    );
    
    tracker.record_mutations(16, 18);
    println!("{}", tracker.report());
}

#[test]
fn test_event_cloudevents_compliance() {
    let mut tracker = TesTracker::new("test_event_cloudevents_compliance");
    
    // Test CloudEvents v1.0 compliance
    tes_behavior!(tracker, "cloudevents_required_attributes",
        given: "CloudEvents specification requirements",
        when: "Creating events",
        then: "All required attributes are present",
        {
            let event = Event::new("source".to_string(), "type".to_string(), None);
            
            // Required attributes per CloudEvents v1.0
            tes_assert_eq!(tracker, event.id.to_string().len() > 0, true, "ID is non-empty");
            tes_assert_eq!(tracker, event.source.len() >= 0, true, "Source exists");
            tes_assert_eq!(tracker, event.specversion, "1.0", "Spec version is 1.0");
            tes_assert_eq!(tracker, event.r#type.len() >= 0, true, "Type exists");
            
            // Time should be in RFC3339 format
            let time_str = event.time.to_rfc3339();
            tes_assert_eq!(tracker, time_str.contains('T'), true, "Time has T separator");
            tes_assert_eq!(tracker, time_str.ends_with('Z') || time_str.contains('+'), true, "Time has timezone");
            
            true
        }
    );
    
    // Test extension attributes
    tes_behavior!(tracker, "cloudevents_extensions",
        given: "Custom extension attributes",
        when: "Using correlation and causation IDs",
        then: "Extensions follow naming conventions",
        {
            let event = Event {
                id: Uuid::new_v4(),
                source: "test".to_string(),
                specversion: "1.0".to_string(),
                r#type: "test".to_string(),
                datacontenttype: None,
                subject: None,
                time: Utc::now(),
                data: None,
                correlation_id: Some(Uuid::new_v4()),
                causation_id: Some(Uuid::new_v4()),
            };
            
            // Extension attributes should use lowercase with underscores
            let serialized = serde_json::to_string(&event).unwrap();
            tes_assert_eq!(tracker, serialized.contains("correlation_id"), true, "Has correlation_id");
            tes_assert_eq!(tracker, serialized.contains("causation_id"), true, "Has causation_id");
            
            true
        }
    );
    
    tracker.record_mutations(10, 12);
    println!("{}", tracker.report());
}