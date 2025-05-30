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


//! Postgres-based implementation of the `StateStore` trait using SQLx and PgPool.

use async_trait::async_trait;
use helix_core::errors::HelixError;
use helix_core::state::StateStore;
use helix_core::types::{AgentId, ProfileId, CredentialId, RecipeId};
use helix_core::agent::AgentConfig; // Added import
use helix_core::recipe::Recipe; // Added import
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};

/// Postgres-backed StateStore.
#[derive(Clone, Debug)]
pub struct PostgresStateStore {
    /// Connection pool to Postgres.
    pool: PgPool,
}

impl PostgresStateStore {
    /// Creates a new PostgresStateStore with the given PgPool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Persists an agent's configuration.
    pub async fn store_agent_config(&self, config: &AgentConfig) -> Result<(), HelixError> {
        sqlx::query(
            r#"
            INSERT INTO agent_configurations (id, profile_id, name, agent_kind, config_data, credential_ids, enabled, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
            ON CONFLICT (id) DO UPDATE SET
                profile_id = EXCLUDED.profile_id,
                name = EXCLUDED.name,
                agent_kind = EXCLUDED.agent_kind,
                config_data = EXCLUDED.config_data,
                credential_ids = EXCLUDED.credential_ids,
                enabled = EXCLUDED.enabled,
                updated_at = NOW()
            "#,
        )
        .bind(config.id)
        .bind(config.profile_id)
        .bind(&config.name)
        .bind(&config.agent_kind)
        .bind(&config.config_data)
        .bind(&config.credential_ids)
        .bind(config.enabled)
        .execute(&self.pool)
        .await
        .map_err(|e| HelixError::InternalError(format!("DB store_agent_config error: {}", e)))?;
        Ok(())
    }

    /// Retrieves an agent's configuration by its ID.
    pub async fn get_agent_config(
        &self,
        agent_id: &AgentId,
    ) -> Result<Option<AgentConfig>, HelixError> {
        sqlx::query_as::<_, AgentConfig>(
            r#"
            SELECT id, profile_id, name, agent_kind, config_data, credential_ids, enabled
            FROM agent_configurations
            WHERE id = $1
            "#,
        )
        .bind(*agent_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| HelixError::InternalError(format!("DB get_agent_config error: {}", e)))
    }

    /// Lists all agent configurations for a given profile.
    pub async fn list_agent_configs_by_profile(
        &self,
        profile_id: &ProfileId,
    ) -> Result<Vec<AgentConfig>, HelixError> {
        sqlx::query_as::<_, AgentConfig>(
            r#"
            SELECT id, profile_id, name, agent_kind, config_data, credential_ids, enabled
            FROM agent_configurations
            WHERE profile_id = $1
            "#,
        )
        .bind(*profile_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| HelixError::InternalError(format!("DB list_agent_configs_by_profile error: {}", e)))
    }

    /// Deletes an agent's configuration.
    pub async fn delete_agent_config(
        &self,
        agent_id: &AgentId,
    ) -> Result<(), HelixError> {
        sqlx::query("DELETE FROM agent_configurations WHERE id = $1")
            .bind(*agent_id)
            .execute(&self.pool)
            .await
            .map_err(|e| HelixError::InternalError(format!("DB delete_agent_config error: {}", e)))?;
        Ok(())
    }

    /// Persists a recipe.
    pub async fn store_recipe(&self, recipe: &Recipe) -> Result<(), HelixError> {
        sqlx::query(
            r#"
            INSERT INTO recipes (id, profile_id, name, description, graph_definition, enabled, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
            ON CONFLICT (id) DO UPDATE SET
                profile_id = EXCLUDED.profile_id,
                name = EXCLUDED.name,
                description = EXCLUDED.description,
                graph_definition = EXCLUDED.graph_definition,
                enabled = EXCLUDED.enabled,
                updated_at = NOW()
            "#,
        )
        .bind(recipe.id)
        .bind(recipe.profile_id)
        .bind(&recipe.name)
        .bind(&recipe.description)
        .bind(serde_json::to_value(&recipe.graph_definition)
            .map_err(|e| HelixError::SerializationError(format!("Failed to serialize RecipeGraphDefinition: {}", e)))?)
        .bind(recipe.enabled)
        .execute(&self.pool)
        .await
        .map_err(|e| HelixError::InternalError(format!("DB store_recipe error: {}", e)))?;
        Ok(())
    }

    /// Retrieves a recipe by its ID.
    pub async fn get_recipe(&self, recipe_id: &RecipeId) -> Result<Option<Recipe>, HelixError> {
        sqlx::query_as::<_, Recipe>(
            r#"
            SELECT id, profile_id, name, description, graph_definition, enabled, created_at, updated_at
            FROM recipes
            WHERE id = $1
            "#,
        )
        .bind(*recipe_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| HelixError::InternalError(format!("DB get_recipe error: {}", e)))
    }

    /// Lists all recipes for a given profile.
    pub async fn list_recipes_by_profile(
        &self,
        profile_id: &ProfileId,
    ) -> Result<Vec<Recipe>, HelixError> {
        sqlx::query_as::<_, Recipe>(
            r#"
            SELECT id, profile_id, name, description, graph_definition, enabled, created_at, updated_at
            FROM recipes
            WHERE profile_id = $1
            "#,
        )
        .bind(*profile_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| HelixError::InternalError(format!("DB list_recipes_by_profile error: {}", e)))
    }

    /// Deletes a recipe by its ID.
    pub async fn delete_recipe(&self, recipe_id: &RecipeId) -> Result<(), HelixError> {
        sqlx::query("DELETE FROM recipes WHERE id = $1")
            .bind(*recipe_id)
            .execute(&self.pool)
            .await
            .map_err(|e| HelixError::InternalError(format!("DB delete_recipe error: {}", e)))?;
        Ok(())
    }
}

#[async_trait]
impl StateStore for PostgresStateStore {
    /// Retrieves the persisted state for a given agent within a profile.
    async fn get_state(
        &self,
        profile_id: &ProfileId,
        agent_id: &AgentId,
    ) -> Result<Option<JsonValue>, HelixError> {
        let row_opt = sqlx::query("SELECT data FROM agent_states WHERE profile_id = $1 AND agent_id = $2")
            .bind(*profile_id)
            .bind(*agent_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| HelixError::InternalError(format!("DB get_state error: {}", e)))?;
        if let Some(row) = row_opt {
            let value: JsonValue = row.try_get("data")
                .map_err(|e| HelixError::InternalError(format!("DB row get error: {}", e)))?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    /// Persists the state for a given agent within a profile.
    async fn set_state(
        &self,
        profile_id: &ProfileId,
        agent_id: &AgentId,
        state: JsonValue,
    ) -> Result<(), HelixError> {
        sqlx::query(r#"INSERT INTO agent_states (profile_id, agent_id, data, created_at, updated_at)
            VALUES ($1, $2, $3, NOW(), NOW()) ON CONFLICT (profile_id, agent_id)
            DO UPDATE SET data = $3, updated_at = NOW()"#)
            .bind(*profile_id)
            .bind(*agent_id)
            .bind(state)
            .execute(&self.pool)
            .await
            .map_err(|e| HelixError::InternalError(format!("DB set_state error: {}", e)))?;
        Ok(())
    }
}
