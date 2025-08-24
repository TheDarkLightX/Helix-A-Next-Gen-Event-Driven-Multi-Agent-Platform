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

//! Natural language parsing utilities for Helix

use crate::errors::LlmError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parsed intent from natural language input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedIntent {
    /// The primary action/intent
    pub action: String,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,
    /// Extracted entities
    pub entities: HashMap<String, Entity>,
    /// Parsed conditions
    pub conditions: Vec<Condition>,
    /// Suggested agent types
    pub suggested_agents: Vec<String>,
}

/// An extracted entity from natural language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    /// Entity type (e.g., "time", "service", "value")
    pub entity_type: String,
    /// Extracted value
    pub value: String,
    /// Confidence score
    pub confidence: f32,
    /// Original text span
    pub span: TextSpan,
}

/// A text span indicating position in original text
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSpan {
    /// Start character position
    pub start: usize,
    /// End character position
    pub end: usize,
}

/// A parsed condition from natural language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    /// Left operand
    pub left: String,
    /// Operator (e.g., "equals", "greater_than", "contains")
    pub operator: String,
    /// Right operand
    pub right: String,
    /// Confidence score
    pub confidence: f32,
}

/// Natural language parser for automation requests
pub struct AutomationParser {
    /// Common patterns for different automation types
    patterns: HashMap<String, Vec<String>>,
}

impl AutomationParser {
    /// Create a new automation parser
    pub fn new() -> Self {
        let mut patterns = HashMap::new();

        // Email automation patterns
        patterns.insert(
            "email".to_string(),
            vec![
                r"send.*email".to_string(),
                r"email.*when".to_string(),
                r"notify.*email".to_string(),
            ],
        );

        // Webhook patterns
        patterns.insert(
            "webhook".to_string(),
            vec![
                r"when.*webhook".to_string(),
                r"http.*request".to_string(),
                r"api.*call".to_string(),
            ],
        );

        // Schedule patterns
        patterns.insert(
            "schedule".to_string(),
            vec![
                r"every.*\d+.*minutes?".to_string(),
                r"daily.*at".to_string(),
                r"weekly.*on".to_string(),
                r"cron".to_string(),
            ],
        );

        Self { patterns }
    }

    /// Parse natural language automation request
    pub fn parse(&self, input: &str) -> Result<ParsedIntent, LlmError> {
        let input_lower = input.to_lowercase();
        let mut suggested_agents = Vec::new();
        let mut confidence = 0.0;
        let mut action = "unknown".to_string();

        // Simple pattern matching for demonstration
        for (agent_type, patterns) in &self.patterns {
            for pattern in patterns {
                if let Ok(regex) = regex::Regex::new(pattern) {
                    if regex.is_match(&input_lower) {
                        suggested_agents.push(agent_type.clone());
                        confidence = 0.8; // Simple confidence score
                        action = agent_type.clone();
                        break;
                    }
                }
            }
        }

        // Extract entities (simplified)
        let entities = self.extract_entities(&input_lower)?;

        // Extract conditions (simplified)
        let conditions = self.extract_conditions(&input_lower)?;

        Ok(ParsedIntent {
            action,
            confidence,
            entities,
            conditions,
            suggested_agents,
        })
    }

    /// Extract entities from text
    fn extract_entities(&self, input: &str) -> Result<HashMap<String, Entity>, LlmError> {
        let mut entities = HashMap::new();

        // Extract time expressions
        if let Ok(time_regex) = regex::Regex::new(r"\b(\d{1,2}:\d{2})\b") {
            for cap in time_regex.captures_iter(input) {
                if let Some(time_match) = cap.get(1) {
                    entities.insert(
                        "time".to_string(),
                        Entity {
                            entity_type: "time".to_string(),
                            value: time_match.as_str().to_string(),
                            confidence: 0.9,
                            span: TextSpan {
                                start: time_match.start(),
                                end: time_match.end(),
                            },
                        },
                    );
                }
            }
        }

        // Extract email addresses
        if let Ok(email_regex) =
            regex::Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b")
        {
            for cap in email_regex.captures_iter(input) {
                if let Some(email_match) = cap.get(0) {
                    entities.insert(
                        "email".to_string(),
                        Entity {
                            entity_type: "email".to_string(),
                            value: email_match.as_str().to_string(),
                            confidence: 0.95,
                            span: TextSpan {
                                start: email_match.start(),
                                end: email_match.end(),
                            },
                        },
                    );
                }
            }
        }

        // Extract numbers
        if let Ok(number_regex) = regex::Regex::new(r"\b\d+\b") {
            for cap in number_regex.captures_iter(input) {
                if let Some(number_match) = cap.get(0) {
                    entities.insert(
                        "number".to_string(),
                        Entity {
                            entity_type: "number".to_string(),
                            value: number_match.as_str().to_string(),
                            confidence: 0.8,
                            span: TextSpan {
                                start: number_match.start(),
                                end: number_match.end(),
                            },
                        },
                    );
                }
            }
        }

        Ok(entities)
    }

    /// Extract conditions from text
    fn extract_conditions(&self, input: &str) -> Result<Vec<Condition>, LlmError> {
        let mut conditions = Vec::new();

        // Simple condition patterns
        let condition_patterns = vec![
            (r"(\w+)\s+equals?\s+(\w+)", "equals"),
            (r"(\w+)\s+is\s+greater\s+than\s+(\w+)", "greater_than"),
            (r"(\w+)\s+is\s+less\s+than\s+(\w+)", "less_than"),
            (r"(\w+)\s+contains?\s+(\w+)", "contains"),
        ];

        for (pattern, operator) in condition_patterns {
            if let Ok(regex) = regex::Regex::new(pattern) {
                for cap in regex.captures_iter(input) {
                    if let (Some(left), Some(right)) = (cap.get(1), cap.get(2)) {
                        conditions.push(Condition {
                            left: left.as_str().to_string(),
                            operator: operator.to_string(),
                            right: right.as_str().to_string(),
                            confidence: 0.7,
                        });
                    }
                }
            }
        }

        Ok(conditions)
    }
}

impl Default for AutomationParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Recipe generator from natural language
pub struct RecipeGenerator;

impl RecipeGenerator {
    /// Generate a recipe structure from parsed intent
    pub fn generate_recipe(intent: &ParsedIntent) -> Result<serde_json::Value, LlmError> {
        let mut recipe = serde_json::json!({
            "name": format!("{} automation", intent.action),
            "description": format!("Generated automation for {}", intent.action),
            "agents": [],
            "connections": []
        });

        // Add agents based on suggested types
        let agents = recipe["agents"].as_array_mut().unwrap();
        for (i, agent_type) in intent.suggested_agents.iter().enumerate() {
            agents.push(serde_json::json!({
                "id": format!("agent_{}", i),
                "type": agent_type,
                "config": Self::generate_agent_config(agent_type, &intent.entities)
            }));
        }

        // Add connections if multiple agents
        if intent.suggested_agents.len() > 1 {
            let connections = recipe["connections"].as_array_mut().unwrap();
            for i in 0..intent.suggested_agents.len() - 1 {
                connections.push(serde_json::json!({
                    "from": format!("agent_{}", i),
                    "to": format!("agent_{}", i + 1)
                }));
            }
        }

        Ok(recipe)
    }

    /// Generate agent configuration based on type and entities
    fn generate_agent_config(
        agent_type: &str,
        entities: &HashMap<String, Entity>,
    ) -> serde_json::Value {
        match agent_type {
            "email" => {
                let mut config = serde_json::json!({
                    "smtp_server": "smtp.gmail.com",
                    "port": 587,
                    "use_tls": true
                });

                if let Some(email_entity) = entities.get("email") {
                    config["to"] = serde_json::Value::String(email_entity.value.clone());
                }

                config
            }
            "webhook" => {
                serde_json::json!({
                    "method": "POST",
                    "headers": {
                        "Content-Type": "application/json"
                    }
                })
            }
            "schedule" => {
                let mut config = serde_json::json!({
                    "type": "interval",
                    "interval": "5m"
                });

                if let Some(time_entity) = entities.get("time") {
                    config["time"] = serde_json::Value::String(time_entity.value.clone());
                    config["type"] = serde_json::Value::String("daily".to_string());
                }

                config
            }
            _ => serde_json::json!({}),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_automation_parser_creation() {
        let parser = AutomationParser::new();
        assert!(!parser.patterns.is_empty());
    }

    #[test]
    fn test_email_pattern_matching() {
        let parser = AutomationParser::new();
        let result = parser
            .parse("Send me an email when the price changes")
            .unwrap();

        assert!(result.suggested_agents.contains(&"email".to_string()));
        assert!(result.confidence > 0.0);
    }

    #[test]
    fn test_entity_extraction() {
        let parser = AutomationParser::new();
        let result = parser
            .parse("Send email to user@example.com at 15:30")
            .unwrap();

        assert!(result.entities.contains_key("email"));
        assert!(result.entities.contains_key("time"));

        let email_entity = &result.entities["email"];
        assert_eq!(email_entity.value, "user@example.com");

        let time_entity = &result.entities["time"];
        assert_eq!(time_entity.value, "15:30");
    }

    #[test]
    fn test_recipe_generation() {
        let intent = ParsedIntent {
            action: "email".to_string(),
            confidence: 0.8,
            entities: HashMap::new(),
            conditions: Vec::new(),
            suggested_agents: vec!["email".to_string()],
        };

        let recipe = RecipeGenerator::generate_recipe(&intent).unwrap();
        assert!(recipe["agents"].is_array());
        assert_eq!(recipe["agents"].as_array().unwrap().len(), 1);
    }
}
