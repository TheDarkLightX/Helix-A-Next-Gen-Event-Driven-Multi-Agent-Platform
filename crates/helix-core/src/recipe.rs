//! Defines the structure and components of a Recipe.

use crate::agent::{AgentConfig, AgentRuntime};
use crate::types::{AgentId, ProfileId, RecipeId};
use crate::HelixError;
use std::collections::{HashMap, HashSet, VecDeque};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::types::Json;

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

/// Represents the graph structure of a recipe, containing agents and their connections.
/// This part is intended to be stored as JSONB in the database.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecipeGraphDefinition {
    /// List of agent configurations included in this recipe.
    pub agents: Vec<AgentConfig>,
    // Connections are now defined as dependencies within AgentConfig
}

/// Represents a workflow definition, connecting multiple agents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, FromRow)]
pub struct Recipe {
    /// Unique identifier for the recipe.
    pub id: RecipeId,
    /// The ID of the profile (tenant) this recipe belongs to.
    pub profile_id: ProfileId,
    /// User-defined name for the recipe.
    pub name: String,
    /// Optional description of the recipe's purpose.
    pub description: Option<String>,
    /// How the recipe is triggered to start execution. Stored as JSONB.
    pub trigger: Option<Json<Trigger>>,
    /// The definition of the agent graph (agents and connections). Stored as JSONB.
    #[sqlx(rename = "graph_definition")] // Ensure this matches the column name if different
    pub graph: Json<RecipeGraphDefinition>,
    /// Whether the recipe is currently active and should be executed.
    pub enabled: bool,
    // TODO: Add versioning information?
    // TODO: Add tags or labels?
    // Timestamps for database tracking, typically handled by sqlx default or direct mapping
    // pub created_at: chrono::DateTime<chrono::Utc>,
    // pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Recipe {
    /// Creates a new recipe with the given parameters
    pub fn new(
        id: RecipeId,
        profile_id: ProfileId,
        name: String,
        description: Option<String>,
        graph: RecipeGraphDefinition,
    ) -> Self {
        Self {
            id,
            profile_id,
            name,
            description,
            trigger: None,
            graph: Json(graph),
            enabled: true,
        }
    }

    /// Updates the recipe name
    pub fn update_name(&mut self, name: String) {
        self.name = name;
    }

    /// Updates the recipe description
    pub fn update_description(&mut self, description: Option<String>) {
        self.description = description;
    }

    /// Sets the recipe trigger
    pub fn set_trigger(&mut self, trigger: Option<Trigger>) {
        self.trigger = trigger.map(Json);
    }

    /// Enables or disables the recipe
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Gets the number of agents in the recipe
    pub fn agent_count(&self) -> usize {
        self.graph.agents.len()
    }

    // connection_count is removed as connections are now implicit via AgentConfig.dependencies

    /// Checks if the recipe has a trigger configured
    pub fn has_trigger(&self) -> bool {
        self.trigger.is_some()
    }

    /// Gets all agent IDs in the recipe
    pub fn agent_ids(&self) -> Vec<AgentId> {
        self.graph.agents.iter().map(|agent| agent.id).collect()
    }

    /// Finds an agent by ID
    pub fn find_agent(&self, agent_id: &AgentId) -> Option<&AgentConfig> {
        self.graph.agents.iter().find(|agent| &agent.id == agent_id)
    }

    /// Validates the recipe structure.
    ///
    /// Checks for:
    /// - At least one agent in the graph definition.
    /// - All dependency agent IDs in `AgentConfig.dependencies` exist within the recipe's agents list.
    /// - The dependency graph is a valid DAG (Directed Acyclic Graph - no cycles).
    /// - Recipe name is not empty.
    /// - No duplicate agent IDs.
    /// - No agent depends on itself.
    pub fn validate(&self) -> Result<(), HelixError> {
        // Check recipe name
        if self.name.trim().is_empty() {
            return Err(HelixError::ValidationError {
                context: "Recipe.name".to_string(),
                message: "Recipe name cannot be empty".to_string(),
            });
        }

        // Check for at least one agent
        if self.graph.agents.is_empty() {
            return Err(HelixError::ValidationError {
                context: "Recipe.graph.agents".to_string(),
                message: "Recipe must contain at least one agent".to_string(),
            });
        }

        // Check for duplicate agent IDs
        let agent_ids_set: HashSet<AgentId> = self.graph.agents.iter().map(|a| a.id).collect();
        if agent_ids_set.len() != self.graph.agents.len() {
            return Err(HelixError::ValidationError {
                context: "Recipe.graph.agents".to_string(),
                message: "Recipe contains duplicate agent IDs".to_string(),
            });
        }

        // Validate agent dependencies
        for agent_config in &self.graph.agents {
            for dep_id in &agent_config.dependencies {
                if !agent_ids_set.contains(dep_id) {
                    return Err(HelixError::ValidationError {
                        context: format!("Recipe.graph.agents[id={}].dependencies", agent_config.id),
                        message: format!("Dependency on non-existent agent ID: {}", dep_id),
                    });
                }
                if dep_id == &agent_config.id {
                    return Err(HelixError::ValidationError {
                        context: format!("Recipe.graph.agents[id={}].dependencies", agent_config.id),
                        message: format!("Agent {} cannot depend on itself.", agent_config.id),
                    });
                }
            }
        }

        // Validate DAG structure (no cycles)
        self.validate_dag()?;

        Ok(())
    }

    /// Validates that the recipe graph is a Directed Acyclic Graph (DAG)
    fn validate_dag(&self) -> Result<(), HelixError> {
        let mut adj: HashMap<AgentId, Vec<AgentId>> = HashMap::new();
        let mut in_degree: HashMap<AgentId, usize> = HashMap::new();
        let agent_ids: HashSet<AgentId> = self.graph.agents.iter().map(|a| a.id).collect();

        for agent_id in &agent_ids {
            adj.insert(*agent_id, Vec::new());
            in_degree.insert(*agent_id, 0);
        }

        for agent_config in &self.graph.agents {
            for dep_id in &agent_config.dependencies {
                // dep_id is a prerequisite for agent_config.id
                // So, edge is from dep_id -> agent_config.id
                if let Some(neighbors) = adj.get_mut(dep_id) {
                    neighbors.push(agent_config.id);
                }
                *in_degree.entry(agent_config.id).or_insert(0) += 1;
            }
        }

        // Kahn's algorithm for cycle detection
        let mut queue: VecDeque<AgentId> = VecDeque::new();
        let mut processed = 0;

        // Start with nodes that have no incoming edges
        for (agent_id, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(*agent_id);
            }
        }

        while let Some(current) = queue.pop_front() {
            processed += 1;

            // Process all neighbors (agents that depend on `current`)
            if let Some(dependents) = adj.get(&current) {
                for &dependent_id in dependents {
                    if let Some(degree) = in_degree.get_mut(&dependent_id) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dependent_id);
                        }
                    }
                }
            }
        }

        // If we processed all nodes, it's a DAG
        if processed == self.graph.agents.len() {
            Ok(())
        } else {
            Err(HelixError::ValidationError {
                context: "Recipe.graph".to_string(),
                message: "Recipe graph contains cycles (not a valid DAG)".to_string(),
            })
        }
    }
}

// The Connection struct is removed as dependencies are now part of AgentConfig.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentConfig; // Keep this
    use serde_json::json;
    use uuid::Uuid;

    // Helper to create AgentConfig for tests, now includes dependencies
    fn create_test_agent_config(
        id: AgentId,
        name: &str,
        agent_kind: &str,
        dependencies: Vec<AgentId>,
    ) -> AgentConfig {
        AgentConfig {
            id,
            profile_id: Uuid::new_v4(), // Example profile_id
            name: Some(name.to_string()),
            agent_kind: agent_kind.to_string(),
            agent_runtime: AgentRuntime::Native,
            wasm_module_path: None,
            config_data: json!({}), // Example config_data
            credential_ids: Vec::new(),
            enabled: true,
            dependencies,
        }
    }

    // Updated simple recipe
    fn create_simple_recipe() -> Recipe {
        let agent1_id = Uuid::new_v4();
        let agent2_id = Uuid::new_v4();

        let agent1 = create_test_agent_config(agent1_id, "SourceAgent", "source", vec![]);
        let agent2 = create_test_agent_config(agent2_id, "ActionAgent", "action", vec![agent1_id]);

        let graph_def = RecipeGraphDefinition {
            agents: vec![agent1, agent2],
        };

        Recipe::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Test Recipe".to_string(),
            Some("A simple test recipe".to_string()),
            graph_def,
        )
    }

    #[test]
    fn test_recipe_creation_and_basic_properties() {
        let recipe_id = Uuid::new_v4();
        let profile_id = Uuid::new_v4();
        let agent1_id = Uuid::new_v4();

        let agent1 = create_test_agent_config(agent1_id, "Agent1", "typeA", vec![]);
        let graph_def = RecipeGraphDefinition { agents: vec![agent1] };

        let recipe = Recipe::new(
            recipe_id,
            profile_id,
            "My Recipe".to_string(),
            None,
            graph_def,
        );

        assert_eq!(recipe.id, recipe_id);
        assert_eq!(recipe.profile_id, profile_id);
        assert_eq!(recipe.name, "My Recipe");
        assert!(recipe.enabled);
        assert_eq!(recipe.agent_count(), 1);
        // assert_eq!(recipe.connection_count(), 0); // This method is removed
    }

    #[test]
    fn test_recipe_update_name() {
        let mut recipe = create_simple_recipe();
        let new_name = "Updated Recipe Name".to_string();

        recipe.update_name(new_name.clone());

        assert_eq!(recipe.name, new_name);
    }

    #[test]
    fn test_recipe_update_description() {
        let mut recipe = create_simple_recipe();
        let new_description = Some("Updated description".to_string());

        recipe.update_description(new_description.clone());

        assert_eq!(recipe.description, new_description);
    }

    #[test]
    fn test_recipe_update_description_to_none() {
        let mut recipe = create_simple_recipe();

        recipe.update_description(None);

        assert_eq!(recipe.description, None);
    }

    #[test]
    fn test_recipe_set_trigger() {
        let mut recipe = create_simple_recipe();
        let trigger = Trigger::Schedule {
            cron_expression: "0 * * * * *".to_string(),
        };

        assert!(!recipe.has_trigger());

        recipe.set_trigger(Some(trigger.clone()));

        assert!(recipe.has_trigger());
        assert_eq!(recipe.trigger.as_ref().unwrap().0, trigger);
    }

    #[test]
    fn test_recipe_set_enabled() {
        let mut recipe = create_simple_recipe();

        assert!(recipe.enabled);

        recipe.set_enabled(false);
        assert!(!recipe.enabled);

        recipe.set_enabled(true);
        assert!(recipe.enabled);
    }

    #[test]
    fn test_recipe_agent_ids() {
        let recipe = create_simple_recipe();
        let agent_ids = recipe.agent_ids();

        assert_eq!(agent_ids.len(), 2);
        assert!(agent_ids.contains(&recipe.graph.agents[0].id));
        assert!(agent_ids.contains(&recipe.graph.agents[1].id));
    }

    #[test]
    fn test_recipe_find_agent() {
        let recipe = create_simple_recipe();
        let first_agent_id = recipe.graph.agents[0].id;
        let non_existent_id = Uuid::new_v4();

        let found_agent = recipe.find_agent(&first_agent_id);
        assert!(found_agent.is_some());
        assert_eq!(found_agent.unwrap().id, first_agent_id);

        let not_found = recipe.find_agent(&non_existent_id);
        assert!(not_found.is_none());
    }

    #[test]
    fn test_recipe_validate_success() {
        let recipe = create_simple_recipe();
        assert!(recipe.validate().is_ok());
    }

    #[test]
    fn test_recipe_validate_empty_name() {
        let mut recipe = create_simple_recipe();
        recipe.name = "".to_string();

        let result = recipe.validate();
        assert!(result.is_err());

        if let Err(HelixError::ValidationError { context, message }) = result {
            assert_eq!(context, "Recipe.name");
            assert_eq!(message, "Recipe name cannot be empty");
        } else {
            panic!("Expected ValidationError");
        }
    }

    #[test]
    fn test_recipe_validate_whitespace_name() {
        let mut recipe = create_simple_recipe();
        recipe.name = "   ".to_string();

        let result = recipe.validate();
        assert!(result.is_err());

        if let Err(HelixError::ValidationError { context, message }) = result {
            assert_eq!(context, "Recipe.name");
            assert_eq!(message, "Recipe name cannot be empty");
        } else {
            panic!("Expected ValidationError");
        }
    }

    #[test]
    fn test_recipe_validate_no_agents() {
        let mut recipe = create_simple_recipe();
        recipe.graph.0.agents.clear();

        let result = recipe.validate();
        assert!(result.is_err());

        if let Err(HelixError::ValidationError { context, message }) = result {
            assert_eq!(context, "Recipe.graph.agents");
            assert_eq!(message, "Recipe must contain at least one agent");
        } else {
            panic!("Expected ValidationError");
        }
    }

    #[test]
    fn test_recipe_validate_duplicate_agent_ids() {
        let agent_id = Uuid::new_v4();
        let agent1 = create_test_agent_config(agent_id, "Agent1", "typeA", vec![]);
        let agent2 = create_test_agent_config(agent_id, "Agent2WithSameId", "typeB", vec![]); // Duplicate ID

        let graph_def = RecipeGraphDefinition { agents: vec![agent1, agent2] };
        let recipe = Recipe::new(Uuid::new_v4(), Uuid::new_v4(), "DupID Recipe".to_string(), None, graph_def);

        let result = recipe.validate();
        assert!(result.is_err());
        if let Err(HelixError::ValidationError { context, message }) = result {
            assert_eq!(context, "Recipe.graph.agents");
            assert_eq!(message, "Recipe contains duplicate agent IDs");
        } else {
            panic!("Expected ValidationError for duplicate agent IDs, got {:?}", result);
        }
    }

    #[test]
    fn test_recipe_validate_dependency_on_non_existent_agent() {
        let agent1_id = Uuid::new_v4();
        let non_existent_agent_id = Uuid::new_v4();

        let agent1 = create_test_agent_config(agent1_id, "Agent1", "typeA", vec![non_existent_agent_id]);
        let graph_def = RecipeGraphDefinition { agents: vec![agent1] };
        let recipe = Recipe::new(Uuid::new_v4(), Uuid::new_v4(), "InvalidDep Recipe".to_string(), None, graph_def);

        let result = recipe.validate();
        assert!(result.is_err());
        if let Err(HelixError::ValidationError { context, message }) = result {
            assert_eq!(context, format!("Recipe.graph.agents[id={}].dependencies", agent1_id));
            assert!(message.contains("Dependency on non-existent agent ID"));
        } else {
            panic!("Expected ValidationError for non-existent dependency, got {:?}", result);
        }
    }

    #[test]
    fn test_recipe_validate_agent_depends_on_self() {
        let agent1_id = Uuid::new_v4();
        let agent1 = create_test_agent_config(agent1_id, "Agent1", "typeA", vec![agent1_id]); // Depends on self

        let graph_def = RecipeGraphDefinition { agents: vec![agent1] };
        let recipe = Recipe::new(Uuid::new_v4(), Uuid::new_v4(), "SelfLoop Recipe".to_string(), None, graph_def);

        let result = recipe.validate();
        assert!(result.is_err());
        if let Err(HelixError::ValidationError { context, message }) = result {
            assert_eq!(context, format!("Recipe.graph.agents[id={}].dependencies", agent1_id));
            assert!(message.contains("cannot depend on itself"));
        } else {
            panic!("Expected ValidationError for self-dependency, got {:?}", result);
        }
    }

    #[test]
    fn test_recipe_validate_cycle_detection() {
        let agent_a_id = Uuid::new_v4();
        let agent_b_id = Uuid::new_v4();
        let agent_c_id = Uuid::new_v4();

        // A -> B, B -> C, C -> A (cycle)
        let agent_a = create_test_agent_config(agent_a_id, "AgentA", "typeA", vec![agent_c_id]);
        let agent_b = create_test_agent_config(agent_b_id, "AgentB", "typeB", vec![agent_a_id]);
        let agent_c = create_test_agent_config(agent_c_id, "AgentC", "typeC", vec![agent_b_id]);

        let graph_def = RecipeGraphDefinition { agents: vec![agent_a, agent_b, agent_c] };
        let recipe = Recipe::new(Uuid::new_v4(), Uuid::new_v4(), "Cyclic Recipe".to_string(), None, graph_def);

        let result = recipe.validate();
        assert!(result.is_err());
        if let Err(HelixError::ValidationError { context, message }) = result {
            assert_eq!(context, "Recipe.graph");
            assert_eq!(message, "Recipe graph contains cycles (not a valid DAG)");
        } else {
            panic!("Expected ValidationError for cycle detection, got {:?}", result);
        }
    }

    #[test]
    fn test_recipe_validate_complex_dag_success() {
        let agent_a_id = Uuid::new_v4();
        let agent_b_id = Uuid::new_v4();
        let agent_c_id = Uuid::new_v4();
        let agent_d_id = Uuid::new_v4();

        // A -> B, A -> C, B -> D, C -> D
        let agent_a = create_test_agent_config(agent_a_id, "AgentA", "typeA", vec![]);
        let agent_b = create_test_agent_config(agent_b_id, "AgentB", "typeB", vec![agent_a_id]);
        let agent_c = create_test_agent_config(agent_c_id, "AgentC", "typeC", vec![agent_a_id]);
        let agent_d = create_test_agent_config(agent_d_id, "AgentD", "typeD", vec![agent_b_id, agent_c_id]);

        let graph_def = RecipeGraphDefinition { agents: vec![agent_a, agent_b, agent_c, agent_d] };
        let recipe = Recipe::new(Uuid::new_v4(), Uuid::new_v4(), "Complex DAG".to_string(), None, graph_def);

        assert!(recipe.validate().is_ok());
        assert_eq!(recipe.agent_count(), 4);
    }


    #[test]
    fn test_trigger_schedule_creation() {
        let trigger = Trigger::Schedule {
            cron_expression: "0 0 * * * *".to_string(),
        };

        let Trigger::Schedule { cron_expression } = trigger;
        assert_eq!(cron_expression, "0 0 * * * *");
    }

    // test_connection_creation is removed as Connection struct is removed.

    #[test]
    fn test_recipe_serialization_deserialization() {
        let original_recipe = create_simple_recipe();

        let serialized = serde_json::to_string_pretty(&original_recipe).expect("Serialization failed");
        let deserialized: Recipe = serde_json::from_str(&serialized).expect("Deserialization failed");

        assert_eq!(original_recipe.id, deserialized.id);
        assert_eq!(original_recipe.name, deserialized.name);
        assert_eq!(original_recipe.graph.agents.len(), deserialized.graph.agents.len());
        // Detailed check of agent properties including dependencies
        for i in 0..original_recipe.graph.agents.len() {
            assert_eq!(original_recipe.graph.agents[i].id, deserialized.graph.agents[i].id);
            assert_eq!(original_recipe.graph.agents[i].dependencies, deserialized.graph.agents[i].dependencies);
        }
    }

    #[test]
    fn test_recipe_graph_definition_equality() {
        let agent_id = Uuid::new_v4();
        let profile_id = Uuid::new_v4(); // Use same profile ID for both agents

        // Create agents with same profile_id manually to ensure equality
        let agent1 = AgentConfig {
            id: agent_id,
            profile_id,
            name: Some("Agent".to_string()),
            agent_kind: "kind".to_string(),
            agent_runtime: AgentRuntime::Native,
            wasm_module_path: None,
            config_data: json!({}),
            credential_ids: Vec::new(),
            enabled: true,
            dependencies: vec![],
        };

        let agent2 = AgentConfig {
            id: agent_id,
            profile_id,
            name: Some("Agent".to_string()),
            agent_kind: "kind".to_string(),
            agent_runtime: AgentRuntime::Native,
            wasm_module_path: None,
            config_data: json!({}),
            credential_ids: Vec::new(),
            enabled: true,
            dependencies: vec![],
        };

        let graph1 = RecipeGraphDefinition { agents: vec![agent1] };
        let graph2 = RecipeGraphDefinition { agents: vec![agent2] };

        assert_eq!(graph1, graph2);

        let agent3_dep_id = Uuid::new_v4();
        let agent3 = AgentConfig {
            id: agent_id,
            profile_id,
            name: Some("Agent".to_string()),
            agent_kind: "kind".to_string(),
            agent_runtime: AgentRuntime::Native,
            wasm_module_path: None,
            config_data: json!({}),
            credential_ids: Vec::new(),
            enabled: true,
            dependencies: vec![agent3_dep_id],
        };
        let graph3 = RecipeGraphDefinition { agents: vec![agent3] };
        assert_ne!(graph1, graph3); // Different due to dependencies
    }

    #[test]
    fn test_recipe_debug_format() {
        let recipe = create_simple_recipe();
        let debug_str = format!("{:?}", recipe);

        assert!(debug_str.contains("Recipe"));
        assert!(debug_str.contains("Test Recipe"));
    }
}
