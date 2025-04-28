//! Defines the structure and components of a Recipe.

use crate::agent::AgentConfig; // Import AgentConfig
use crate::types::{AgentId, ProfileId, RecipeId}; // Import necessary ID types
use crate::HelixError;
use serde::{Deserialize, Serialize};

/// Defines how a recipe is triggered.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Trigger {
    /// Triggered based on a CRON schedule.
    Schedule {
        /// Standard CRON expression (e.g., "0 * * * * *").
        cron_expression: String,
    },
    // TODO: Add other trigger types (Webhook, Event, Manual)
}

/// Represents a workflow definition, connecting multiple agents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Recipe {
    /// Unique identifier for the recipe.
    pub id: RecipeId,
    /// The ID of the profile (tenant) this recipe belongs to.
    pub profile_id: ProfileId,
    /// User-defined name for the recipe.
    pub name: String,
    /// Optional description of the recipe's purpose.
    pub description: Option<String>,
    /// How the recipe is triggered to start execution.
    pub trigger: Option<Trigger>,
    /// List of agent configurations included in this recipe.
    pub agents: Vec<AgentConfig>,
    /// List of connections defining the data flow between agents.
    pub connections: Vec<Connection>,
    /// Whether the recipe is currently active and should be executed.
    pub enabled: bool,
    // TODO: Add versioning information?
    // TODO: Add tags or labels?
}

impl Recipe {
    /// Validates the recipe structure.
    ///
    /// Checks for:
    /// - At least one agent.
    /// - All connection agent IDs exist within the recipe's agents list.
    /// - The connection graph is a valid DAG (Directed Acyclic Graph - no cycles).
    /// - TODO: Any other structural rules?
    pub fn validate(&self) -> Result<(), HelixError> {
        if self.agents.is_empty() {
            return Err(HelixError::ValidationError {
                context: "Recipe.agents".to_string(), // Be specific about context
                message: "Recipe must contain at least one agent".to_string(),
            });
        }

        // TODO: Validate agent IDs in connections
        // TODO: Validate DAG structure (no cycles)

        Ok(())
    }
}

/// Represents a connection between two agents in a Recipe DAG.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Connection {
    /// The ID of the agent where the edge originates.
    pub source_agent_id: AgentId,
    /// The ID of the agent where the edge terminates.
    pub target_agent_id: AgentId,
    // pub config: Option<JsonValue>, // Optional edge configuration (e.g., filtering)
}

// --- Example of how a Recipe might be constructed (for illustration) ---

/*
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentKind;
    use uuid::Uuid;

    #[test]
    fn create_sample_recipe() {
        let agent1_id = AgentId::new_v4();
        let agent2_id = AgentId::new_v4();

        let recipe = Recipe {
            id: Uuid::new_v4(),
            profile_id: Uuid::new_v4(),
            name: "Sample Webhook to Slack".to_string(),
            description: Some("Receives a webhook and posts to Slack".to_string()),
            trigger: None,
            enabled: true,
            agents: vec![
                AgentConfig {
                    id: agent1_id,
                    name: Some("Webhook Receiver".to_string()),
                    kind: AgentKind::Source,
                    plugin_id: "webhook-plugin-v1".to_string(),
                    schedule: None,
                    options: serde_json::json!({ "path": "/hook/sample" }),
                    credentials: vec![],
                },
                AgentConfig {
                    id: agent2_id,
                    name: Some("Slack Notifier".to_string()),
                    kind: AgentKind::Action,
                    plugin_id: "slack-plugin-v1".to_string(),
                    schedule: None,
                    options: serde_json::json!({ "channel": "#general" }),
                    credentials: vec!["slack-credential-abc".to_string()],
                },
            ],
            connections: vec![Connection {
                source_agent_id: agent1_id, // Now uses AgentId
                target_agent_id: agent2_id,
            }],
        };

        assert_eq!(recipe.name, "Sample Webhook to Slack");
        assert_eq!(recipe.agents.len(), 2);
        assert_eq!(recipe.connections.len(), 1);
    }
}
*/
