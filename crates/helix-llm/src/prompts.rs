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

//! Prompt templates and engineering utilities

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A prompt template with variables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    /// Template name
    pub name: String,
    /// Template content with placeholders
    pub template: String,
    /// Variable definitions
    pub variables: HashMap<String, VariableDefinition>,
    /// Template metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Definition of a template variable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDefinition {
    /// Variable name
    pub name: String,
    /// Variable description
    pub description: String,
    /// Variable type
    pub var_type: VariableType,
    /// Whether the variable is required
    pub required: bool,
    /// Default value if not provided
    pub default: Option<String>,
}

/// Type of a template variable
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VariableType {
    /// String value
    String,
    /// Number value
    Number,
    /// Boolean value
    Boolean,
    /// Array of values
    Array,
    /// Object/map value
    Object,
}

impl PromptTemplate {
    /// Create a new prompt template
    pub fn new(name: String, template: String) -> Self {
        Self {
            name,
            template,
            variables: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add a variable definition
    pub fn add_variable(&mut self, var: VariableDefinition) {
        self.variables.insert(var.name.clone(), var);
    }

    /// Render the template with provided values
    pub fn render(&self, values: &HashMap<String, String>) -> Result<String, String> {
        let mut result = self.template.clone();

        // Replace all variables in the template
        for (name, definition) in &self.variables {
            let placeholder = format!("{{{}}}", name);

            if let Some(value) = values.get(name) {
                result = result.replace(&placeholder, value);
            } else if definition.required {
                return Err(format!("Required variable '{}' not provided", name));
            } else if let Some(default) = &definition.default {
                result = result.replace(&placeholder, default);
            } else {
                result = result.replace(&placeholder, "");
            }
        }

        Ok(result)
    }
}

/// Common prompt templates for Helix agents
pub struct CommonPrompts;

impl CommonPrompts {
    /// System prompt for recipe generation
    pub fn recipe_generation() -> PromptTemplate {
        let mut template = PromptTemplate::new(
            "recipe_generation".to_string(),
            r#"You are a Helix automation expert. Generate a recipe (workflow) based on the user's description.

User Request: {user_request}

Available Agent Types:
{available_agents}

Generate a JSON recipe with the following structure:
```json
{
  "name": "Recipe Name",
  "description": "Brief description",
  "agents": [
    {
      "id": "unique_id",
      "type": "agent_type",
      "config": {...}
    }
  ],
  "connections": [
    {
      "from": "source_agent_id",
      "to": "target_agent_id",
      "condition": "optional_condition"
    }
  ]
}
```

Ensure the recipe is practical and follows best practices."#.to_string(),
        );

        template.add_variable(VariableDefinition {
            name: "user_request".to_string(),
            description: "The user's automation request".to_string(),
            var_type: VariableType::String,
            required: true,
            default: None,
        });

        template.add_variable(VariableDefinition {
            name: "available_agents".to_string(),
            description: "List of available agent types".to_string(),
            var_type: VariableType::String,
            required: true,
            default: None,
        });

        template
    }

    /// System prompt for event analysis
    pub fn event_analysis() -> PromptTemplate {
        let mut template = PromptTemplate::new(
            "event_analysis".to_string(),
            r#"You are analyzing an event in the Helix automation system.

Event Details:
- Type: {event_type}
- Source: {event_source}
- Data: {event_data}
- Timestamp: {event_timestamp}

Context:
{context}

Analyze this event and provide:
1. A summary of what happened
2. Potential actions that could be triggered
3. Any anomalies or concerns
4. Suggested follow-up actions

Format your response as structured JSON."#
                .to_string(),
        );

        template.add_variable(VariableDefinition {
            name: "event_type".to_string(),
            description: "Type of the event".to_string(),
            var_type: VariableType::String,
            required: true,
            default: None,
        });

        template.add_variable(VariableDefinition {
            name: "event_source".to_string(),
            description: "Source of the event".to_string(),
            var_type: VariableType::String,
            required: true,
            default: None,
        });

        template.add_variable(VariableDefinition {
            name: "event_data".to_string(),
            description: "Event payload data".to_string(),
            var_type: VariableType::String,
            required: true,
            default: None,
        });

        template.add_variable(VariableDefinition {
            name: "event_timestamp".to_string(),
            description: "When the event occurred".to_string(),
            var_type: VariableType::String,
            required: true,
            default: None,
        });

        template.add_variable(VariableDefinition {
            name: "context".to_string(),
            description: "Additional context about the system state".to_string(),
            var_type: VariableType::String,
            required: false,
            default: Some("No additional context available".to_string()),
        });

        template
    }

    /// System prompt for agent debugging
    pub fn agent_debugging() -> PromptTemplate {
        let mut template = PromptTemplate::new(
            "agent_debugging".to_string(),
            r#"You are debugging a Helix agent that encountered an error.

Agent Information:
- ID: {agent_id}
- Type: {agent_type}
- Configuration: {agent_config}

Error Details:
- Error: {error_message}
- Stack Trace: {stack_trace}
- Context: {error_context}

Recent Events:
{recent_events}

Analyze the error and provide:
1. Root cause analysis
2. Suggested fixes
3. Prevention strategies
4. Code changes if applicable

Be specific and actionable in your recommendations."#
                .to_string(),
        );

        // Add variable definitions...
        template.add_variable(VariableDefinition {
            name: "agent_id".to_string(),
            description: "ID of the failing agent".to_string(),
            var_type: VariableType::String,
            required: true,
            default: None,
        });

        template.add_variable(VariableDefinition {
            name: "agent_type".to_string(),
            description: "Type of the failing agent".to_string(),
            var_type: VariableType::String,
            required: true,
            default: None,
        });

        template.add_variable(VariableDefinition {
            name: "agent_config".to_string(),
            description: "Agent configuration".to_string(),
            var_type: VariableType::String,
            required: true,
            default: None,
        });

        template.add_variable(VariableDefinition {
            name: "error_message".to_string(),
            description: "The error message".to_string(),
            var_type: VariableType::String,
            required: true,
            default: None,
        });

        template.add_variable(VariableDefinition {
            name: "stack_trace".to_string(),
            description: "Stack trace of the error".to_string(),
            var_type: VariableType::String,
            required: false,
            default: Some("No stack trace available".to_string()),
        });

        template.add_variable(VariableDefinition {
            name: "error_context".to_string(),
            description: "Context when the error occurred".to_string(),
            var_type: VariableType::String,
            required: false,
            default: Some("No additional context".to_string()),
        });

        template.add_variable(VariableDefinition {
            name: "recent_events".to_string(),
            description: "Recent events that might be related".to_string(),
            var_type: VariableType::String,
            required: false,
            default: Some("No recent events".to_string()),
        });

        template
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_template_creation() {
        let template = PromptTemplate::new("test".to_string(), "Hello {name}!".to_string());

        assert_eq!(template.name, "test");
        assert_eq!(template.template, "Hello {name}!");
    }

    #[test]
    fn test_template_rendering() {
        let mut template = PromptTemplate::new(
            "test".to_string(),
            "Hello {name}, you are {age} years old!".to_string(),
        );

        template.add_variable(VariableDefinition {
            name: "name".to_string(),
            description: "Person's name".to_string(),
            var_type: VariableType::String,
            required: true,
            default: None,
        });

        template.add_variable(VariableDefinition {
            name: "age".to_string(),
            description: "Person's age".to_string(),
            var_type: VariableType::Number,
            required: false,
            default: Some("unknown".to_string()),
        });

        let mut values = HashMap::new();
        values.insert("name".to_string(), "Alice".to_string());

        let result = template.render(&values).unwrap();
        assert_eq!(result, "Hello Alice, you are unknown years old!");
    }

    #[test]
    fn test_missing_required_variable() {
        let mut template = PromptTemplate::new("test".to_string(), "Hello {name}!".to_string());

        template.add_variable(VariableDefinition {
            name: "name".to_string(),
            description: "Person's name".to_string(),
            var_type: VariableType::String,
            required: true,
            default: None,
        });

        let values = HashMap::new();
        let result = template.render(&values);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Required variable 'name' not provided"));
    }
}
