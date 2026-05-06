// Copyright 2026 DarkLightX
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

//! Helix REST API.

mod evm_rpc;
mod intel;

use crate::intel::{
    collect_source_handler, create_source, create_watchlist, export_autopilot_review_packet,
    export_market_brief_packet_handler, file_import_handler, generate_market_intel_brief_handler,
    get_autopilot_review_queue, get_intel_overview, get_market_intel_overview, ingest_evidence,
    list_cases, list_claims, list_evidence, list_sources, list_watchlists, review_claim_handler,
    transition_case_handler, webhook_ingest_handler, AutopilotReviewKind,
    AutopilotReviewQueueEntry, IntelDeskPostgresStore, IntelDeskStore,
};
use axum::{
    extract::{Path, Query, Request, State},
    http::{header::AUTHORIZATION, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Json, Response},
    routing::{delete, get, post},
    Router,
};
use helix_agent_sdk::{AgentContext, EventPublisher, SdkAgent, SdkError};
use helix_core::agent::{Agent, AgentConfig, AgentRuntime};
use helix_core::autopilot_guard::{
    AutopilotActionClass, AutopilotGuardConfig, AutopilotGuardDecision, AutopilotGuardInput,
    AutopilotGuardMachine, AutopilotMode, AutopilotStats,
};
use helix_core::credential::{Credential, CredentialProvider, EnvCredentialProvider};
use helix_core::deterministic_agent_catalog::{
    agent_catalog_quality, high_roi_agent_catalog, AgentCatalogQuality, DeterministicAgentSpec,
};
use helix_core::deterministic_agent_profiles::{
    find_agent_template, high_roi_agent_templates, DeterministicAgentTemplate,
};
use helix_core::deterministic_agents_expanded::{simulate_expanded_guard, TemporalGuardInput};
use helix_core::deterministic_policy::{
    DeterministicPolicyConfig, DeterministicPolicyEngine, PolicyCommand, PolicyStepResult,
};
use helix_core::event::Event;
use helix_core::onchain_intent::{
    step as onchain_step, OnchainInput, OnchainKernelError, OnchainPhase, OnchainState,
};
use helix_core::reasoning::{
    compile_symbolic_program, evaluate_compiled_neuro_symbolic_reasoning,
    evaluate_compiled_symbolic_reasoning, evaluate_reasoning, fingerprint_symbolic_program,
    CompiledSymbolicProgram, KrrTriple, ReasoningDecision, ReasoningEvaluationRequest,
    SymbolicRule,
};
use helix_core::recipe::Recipe;
use helix_core::state::{InMemoryStateStore, StateStore};
use helix_core::types::{AgentId, CredentialId, ProfileId};
use helix_core::HelixError;
use helix_llm::providers::{LlmProvider, LlmRequest, Message, MessageRole, OpenAiProvider};
use helix_rule_engine::event_listener::RuleEngineEventListener;
use helix_rule_engine::rules::{ParameterValue, RecipeTriggerPlan, Rule};
use helix_runtime::agent_registry::AgentRegistry;
use helix_runtime::agent_runner::AgentRunner;
use helix_runtime::InMemoryEventCollector;
use helix_security::encryption::{
    AesGcmCredentialEncrypterDecrypter, CredentialEncrypterDecrypter,
};
use helix_security::{ApiTokenAuthConfig, AuthDecision, AuthService};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value};
use sqlx::{postgres::PgPoolOptions, postgres::PgRow, PgPool, Row};
use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::evm_rpc::{deterministic_dry_run_hash, EvmReceipt, EvmRpcClient};

#[derive(Clone)]
pub(crate) struct AppState {
    policy_config: Arc<RwLock<DeterministicPolicyConfig>>,
    autopilot_guard: Arc<RwLock<AutopilotGuardMachine>>,
    intel_desk: Arc<RwLock<IntelDeskStore>>,
    intel_persistence: Option<Arc<IntelDeskPostgresStore>>,
    state_persistence: Option<Arc<AppPostgresStore>>,
    automation_rules: Arc<RwLock<Vec<Rule>>>,
    recipes: Arc<RwLock<Vec<Recipe>>>,
    symbolic_program_cache: Arc<RwLock<SymbolicProgramCache>>,
    llm_provider: Option<Arc<dyn LlmProvider>>,
    llm_model: Option<String>,
    auth_service: Arc<AuthService>,
}

const SYMBOLIC_PROGRAM_CACHE_CAPACITY: usize = 128;
const HELIX_API_ADDR_ENV: &str = "HELIX_API_ADDR";
const HELIX_UI_DIST_ENV: &str = "HELIX_UI_DIST";
const HELIX_AUTH_REQUIRED_ENV: &str = "HELIX_AUTH_REQUIRED";
const HELIX_API_TOKEN_ENV: &str = "HELIX_API_TOKEN";
const DATABASE_URL_ENV: &str = "DATABASE_URL";
const HELIX_AUTO_MIGRATE_ENV: &str = "HELIX_AUTO_MIGRATE";
const SYSTEM_AUDIT_SUBJECT: &str = "api";
const HELIX_CORE_MIGRATION_SQL: &str = include_str!("../../../migrations/001_helix_core.sql");
const MAX_CREDENTIAL_NAME_LEN: usize = 128;
const MAX_CREDENTIAL_KIND_LEN: usize = 64;
const MAX_CREDENTIAL_SECRET_LEN: usize = 16 * 1024;
const MAX_CREDENTIAL_METADATA_ENTRIES: usize = 32;
const MAX_CREDENTIAL_METADATA_KEY_LEN: usize = 128;
const MAX_CREDENTIAL_METADATA_VALUE_LEN: usize = 512;

#[derive(Debug, Default)]
struct SymbolicProgramCache {
    entries: HashMap<String, Arc<CompiledSymbolicProgram>>,
    order: Vec<String>,
    capacity: usize,
}

#[derive(Debug, Clone)]
struct AppPostgresStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AuditEvent {
    subject: String,
    action: String,
    resource: String,
    decision: String,
    reason: Option<String>,
    metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuditLogEntry {
    id: i64,
    subject: String,
    action: String,
    resource: String,
    decision: String,
    reason: Option<String>,
    metadata: Value,
    created_at: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct AuditLogQuery {
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuditLogResponse {
    persistence_enabled: bool,
    entries: Vec<AuditLogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredentialUpsertRequest {
    id: Option<String>,
    profile_id: String,
    name: String,
    kind: String,
    secret: String,
    #[serde(default)]
    metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct CredentialQuery {
    profile_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredentialMetadataEntry {
    id: String,
    profile_id: String,
    name: String,
    kind: String,
    metadata: BTreeMap<String, String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredentialResponse {
    persistence_enabled: bool,
    credential: Option<CredentialMetadataEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredentialCatalogResponse {
    persistence_enabled: bool,
    credentials: Vec<CredentialMetadataEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredentialDeleteResponse {
    persistence_enabled: bool,
    deleted: bool,
}

#[derive(Debug, Clone)]
struct PendingCredentialUpsert {
    id: CredentialId,
    profile_id: ProfileId,
    name: String,
    kind: String,
    secret: String,
    metadata: BTreeMap<String, String>,
}

impl AuditEvent {
    pub(crate) fn allow(
        action: impl Into<String>,
        resource: impl Into<String>,
        metadata: Value,
    ) -> Self {
        Self {
            subject: SYSTEM_AUDIT_SUBJECT.to_string(),
            action: action.into(),
            resource: resource.into(),
            decision: "allow".to_string(),
            reason: None,
            metadata,
        }
    }

    pub(crate) fn deny(
        action: impl Into<String>,
        resource: impl Into<String>,
        reason: impl Into<String>,
        metadata: Value,
    ) -> Self {
        Self {
            subject: SYSTEM_AUDIT_SUBJECT.to_string(),
            action: action.into(),
            resource: resource.into(),
            decision: "deny".to_string(),
            reason: Some(reason.into()),
            metadata,
        }
    }
}

struct BuiltInRecipeAgent {
    agent_config: Arc<AgentConfig>,
}

#[helix_agent_sdk::async_trait]
impl Agent for BuiltInRecipeAgent {
    fn id(&self) -> AgentId {
        self.agent_config.id
    }

    fn config(&self) -> &AgentConfig {
        &self.agent_config
    }
}

#[helix_agent_sdk::async_trait]
impl SdkAgent for BuiltInRecipeAgent {
    async fn init(&mut self, _context: &AgentContext) -> Result<(), SdkError> {
        match self.agent_config.agent_kind.as_str() {
            "noop" => Ok(()),
            "emit_event" => {
                let event_type =
                    optional_string_field(&self.agent_config.config_data, "event_type")
                        .unwrap_or_else(|| "helix.recipe.agent.completed".to_string());
                if event_type.trim().is_empty() {
                    return Err(SdkError::ConfigurationError(
                        "emit_event.event_type must not be empty".to_string(),
                    ));
                }
                Ok(())
            }
            "record_state" => Ok(()),
            kind => Err(SdkError::ConfigurationError(format!(
                "unsupported built-in recipe agent kind '{}'",
                kind
            ))),
        }
    }

    async fn start(&mut self, context: &AgentContext) -> Result<(), SdkError> {
        match self.agent_config.agent_kind.as_str() {
            "noop" => Ok(()),
            "emit_event" => {
                let event_type =
                    optional_string_field(&self.agent_config.config_data, "event_type")
                        .unwrap_or_else(|| "helix.recipe.agent.completed".to_string());
                let payload = self
                    .agent_config
                    .config_data
                    .get("payload")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                context
                    .emit_event(
                        serde_json::json!({
                            "agent_id": self.agent_config.id,
                            "agent_kind": self.agent_config.agent_kind,
                            "agent_name": self.agent_config.name,
                            "payload": payload,
                        }),
                        Some(event_type),
                    )
                    .await
            }
            "record_state" => {
                let record = self
                    .agent_config
                    .config_data
                    .get("state")
                    .cloned()
                    .unwrap_or_else(|| self.agent_config.config_data.clone());
                context
                    .state_store()
                    .set_state(
                        &self.agent_config.profile_id,
                        &self.agent_config.id,
                        serde_json::json!({
                            "agent_id": self.agent_config.id,
                            "agent_kind": self.agent_config.agent_kind,
                            "agent_name": self.agent_config.name,
                            "record": record,
                        }),
                    )
                    .await
                    .map_err(SdkError::from)
            }
            kind => Err(SdkError::ConfigurationError(format!(
                "unsupported built-in recipe agent kind '{}'",
                kind
            ))),
        }
    }

    async fn stop(&mut self, _context: &AgentContext) -> Result<(), SdkError> {
        Ok(())
    }
}

fn optional_string_field(config: &Value, field: &str) -> Option<String> {
    config
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn built_in_recipe_agent_factory(config: AgentConfig) -> Result<Box<dyn SdkAgent>, SdkError> {
    Ok(Box::new(BuiltInRecipeAgent {
        agent_config: Arc::new(config),
    }))
}

impl AppPostgresStore {
    fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn load_policy_config(&self) -> Result<Option<DeterministicPolicyConfig>, HelixError> {
        let row =
            sqlx::query("SELECT config FROM policy_config_snapshots ORDER BY id DESC LIMIT 1")
                .fetch_optional(&self.pool)
                .await
                .map_err(app_db_error)?;
        row.map(|row| {
            serde_json::from_value(row.get::<Value, _>("config")).map_err(HelixError::from)
        })
        .transpose()
    }

    async fn save_policy_config(
        &self,
        config: &DeterministicPolicyConfig,
    ) -> Result<(), HelixError> {
        sqlx::query("INSERT INTO policy_config_snapshots (config) VALUES ($1)")
            .bind(serde_json::to_value(config).map_err(HelixError::from)?)
            .execute(&self.pool)
            .await
            .map_err(app_db_error)?;
        Ok(())
    }

    async fn load_autopilot_guard(&self) -> Result<Option<AutopilotGuardMachine>, HelixError> {
        let row = sqlx::query(
            "SELECT config, stats FROM autopilot_guard_snapshots ORDER BY id DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(app_db_error)?;

        row.map(|row| {
            let config: AutopilotGuardConfig =
                serde_json::from_value(row.get::<Value, _>("config"))?;
            let stats: AutopilotStats = serde_json::from_value(row.get::<Value, _>("stats"))?;
            Ok::<_, HelixError>(AutopilotGuardMachine::from_snapshot(config, stats))
        })
        .transpose()
    }

    async fn save_autopilot_guard(&self, guard: &AutopilotGuardMachine) -> Result<(), HelixError> {
        sqlx::query("INSERT INTO autopilot_guard_snapshots (config, stats) VALUES ($1, $2)")
            .bind(serde_json::to_value(guard.config()).map_err(HelixError::from)?)
            .bind(serde_json::to_value(guard.stats()).map_err(HelixError::from)?)
            .execute(&self.pool)
            .await
            .map_err(app_db_error)?;
        Ok(())
    }

    async fn load_recipes(&self) -> Result<Vec<Recipe>, HelixError> {
        sqlx::query_as::<_, Recipe>(
            "SELECT id, profile_id, name, description, trigger, graph_definition, enabled, version, tags \
             FROM recipes ORDER BY id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(app_db_error)
    }

    async fn upsert_recipe(&self, recipe: &Recipe) -> Result<(), HelixError> {
        sqlx::query(
            "INSERT INTO recipes \
             (id, profile_id, name, description, trigger, graph_definition, enabled, version, tags) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
             ON CONFLICT (id) DO UPDATE SET \
             profile_id = EXCLUDED.profile_id, name = EXCLUDED.name, description = EXCLUDED.description, \
             trigger = EXCLUDED.trigger, graph_definition = EXCLUDED.graph_definition, \
             enabled = EXCLUDED.enabled, version = EXCLUDED.version, tags = EXCLUDED.tags, updated_at = now()",
        )
        .bind(recipe.id)
        .bind(recipe.profile_id)
        .bind(&recipe.name)
        .bind(&recipe.description)
        .bind(&recipe.trigger)
        .bind(&recipe.graph)
        .bind(recipe.enabled)
        .bind(&recipe.version)
        .bind(&recipe.tags)
        .execute(&self.pool)
        .await
        .map_err(app_db_error)?;
        Ok(())
    }

    async fn load_automation_rules(&self) -> Result<Vec<Rule>, HelixError> {
        let rows = sqlx::query("SELECT record FROM automation_rules ORDER BY id ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(app_db_error)?;

        rows.into_iter()
            .map(|row| {
                serde_json::from_value(row.get::<Value, _>("record")).map_err(HelixError::from)
            })
            .collect()
    }

    async fn upsert_automation_rule(&self, rule: &Rule) -> Result<(), HelixError> {
        sqlx::query(
            "INSERT INTO automation_rules (id, name, enabled, record) VALUES ($1, $2, $3, $4) \
             ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, enabled = EXCLUDED.enabled, record = EXCLUDED.record, updated_at = now()",
        )
        .bind(rule.id)
        .bind(&rule.name)
        .bind(rule.enabled)
        .bind(serde_json::to_value(rule).map_err(HelixError::from)?)
        .execute(&self.pool)
        .await
            .map_err(app_db_error)?;
        Ok(())
    }

    async fn insert_automation_rule_evaluation(
        &self,
        event: &Event,
        rule_count: usize,
        trigger_plans: &[RecipeTriggerPlan],
    ) -> Result<AutomationRuleEvaluationEntry, HelixError> {
        let row = sqlx::query(
            "INSERT INTO automation_rule_evaluations \
             (event_id, event_type, event_source, event, rule_count, trigger_plan_count, trigger_plans) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             RETURNING id, event_id::text AS event_id, event_type, event_source, event, rule_count, trigger_plan_count, trigger_plans, created_at::text AS created_at",
        )
        .bind(event.id)
        .bind(&event.r#type)
        .bind(&event.source)
        .bind(serde_json::to_value(event).map_err(HelixError::from)?)
        .bind(rule_count as i32)
        .bind(trigger_plans.len() as i32)
        .bind(serde_json::to_value(trigger_plans).map_err(HelixError::from)?)
        .fetch_one(&self.pool)
        .await
        .map_err(app_db_error)?;

        automation_rule_evaluation_from_row(row)
    }

    async fn list_automation_rule_evaluations(
        &self,
        limit: usize,
    ) -> Result<Vec<AutomationRuleEvaluationEntry>, HelixError> {
        let rows = sqlx::query(
            "SELECT id, event_id::text AS event_id, event_type, event_source, event, rule_count, trigger_plan_count, trigger_plans, created_at::text AS created_at \
             FROM automation_rule_evaluations ORDER BY id DESC LIMIT $1",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(app_db_error)?;

        rows.into_iter()
            .map(automation_rule_evaluation_from_row)
            .collect()
    }

    async fn insert_recipe_run(
        &self,
        evaluation_id: Option<i64>,
        plan: &RecipeTriggerPlan,
        recipe: Option<&Recipe>,
        status: &str,
        runtime_output: &RecipeRuntimeOutput,
        error: Option<&str>,
    ) -> Result<RecipeRunEntry, HelixError> {
        let row = sqlx::query(
            "INSERT INTO recipe_runs \
             (evaluation_id, rule_id, action_id, requested_recipe_id, requested_recipe_name, \
              resolved_recipe_id, resolved_recipe_name, trigger_plan, parameters, status, started_agent_ids, emitted_events, state_snapshots, error) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) \
             RETURNING id, evaluation_id, rule_id::text AS rule_id, action_id, \
             requested_recipe_id::text AS requested_recipe_id, requested_recipe_name, \
             resolved_recipe_id::text AS resolved_recipe_id, resolved_recipe_name, \
             trigger_plan, parameters, status, started_agent_ids, emitted_events, state_snapshots, error, created_at::text AS created_at",
        )
        .bind(evaluation_id)
        .bind(plan.rule_id)
        .bind(&plan.action_id)
        .bind(plan.recipe_id)
        .bind(&plan.recipe_name)
        .bind(recipe.map(|recipe| recipe.id))
        .bind(recipe.map(|recipe| recipe.name.as_str()))
        .bind(serde_json::to_value(plan).map_err(HelixError::from)?)
        .bind(serde_json::to_value(&plan.parameters).map_err(HelixError::from)?)
        .bind(status)
        .bind(runtime_output.started_agent_ids.clone())
        .bind(Value::Array(runtime_output.emitted_events.clone()))
        .bind(runtime_output.state_snapshots.clone())
        .bind(error)
        .fetch_one(&self.pool)
        .await
        .map_err(app_db_error)?;

        recipe_run_from_row(row)
    }

    async fn list_recipe_runs(&self, limit: usize) -> Result<Vec<RecipeRunEntry>, HelixError> {
        let rows = sqlx::query(
            "SELECT id, evaluation_id, rule_id::text AS rule_id, action_id, \
             requested_recipe_id::text AS requested_recipe_id, requested_recipe_name, \
             resolved_recipe_id::text AS resolved_recipe_id, resolved_recipe_name, \
             trigger_plan, parameters, status, started_agent_ids, emitted_events, state_snapshots, error, created_at::text AS created_at \
             FROM recipe_runs ORDER BY id DESC LIMIT $1",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(app_db_error)?;

        rows.into_iter().map(recipe_run_from_row).collect()
    }

    async fn upsert_credential(
        &self,
        credential: &Credential,
    ) -> Result<CredentialMetadataEntry, HelixError> {
        let row = sqlx::query(
            "INSERT INTO credentials (id, profile_id, name, kind, encrypted_data, metadata) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (profile_id, name) DO UPDATE SET \
             kind = EXCLUDED.kind, encrypted_data = EXCLUDED.encrypted_data, \
             metadata = EXCLUDED.metadata, updated_at = now() \
             RETURNING id::text AS id, profile_id::text AS profile_id, name, kind, metadata, \
             created_at::text AS created_at, updated_at::text AS updated_at",
        )
        .bind(credential.id)
        .bind(credential.profile_id)
        .bind(&credential.name)
        .bind(&credential.kind)
        .bind(&credential.data)
        .bind(serde_json::to_value(&credential.metadata).map_err(HelixError::from)?)
        .fetch_one(&self.pool)
        .await
        .map_err(app_db_error)?;

        credential_metadata_from_row(row)
    }

    pub(crate) async fn encrypted_credential_data(
        &self,
        profile_id: &ProfileId,
        credential_id: &CredentialId,
    ) -> Result<Option<String>, HelixError> {
        let row =
            sqlx::query("SELECT encrypted_data FROM credentials WHERE profile_id = $1 AND id = $2")
                .bind(profile_id)
                .bind(credential_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(app_db_error)?;

        Ok(row.map(|row| row.get("encrypted_data")))
    }

    async fn list_credentials(
        &self,
        profile_id: &ProfileId,
    ) -> Result<Vec<CredentialMetadataEntry>, HelixError> {
        let rows = sqlx::query(
            "SELECT id::text AS id, profile_id::text AS profile_id, name, kind, metadata, \
             created_at::text AS created_at, updated_at::text AS updated_at \
             FROM credentials WHERE profile_id = $1 ORDER BY name ASC, id ASC",
        )
        .bind(profile_id)
        .fetch_all(&self.pool)
        .await
        .map_err(app_db_error)?;

        rows.into_iter().map(credential_metadata_from_row).collect()
    }

    async fn delete_credential(
        &self,
        profile_id: &ProfileId,
        credential_id: &CredentialId,
    ) -> Result<bool, HelixError> {
        let result = sqlx::query("DELETE FROM credentials WHERE profile_id = $1 AND id = $2")
            .bind(profile_id)
            .bind(credential_id)
            .execute(&self.pool)
            .await
            .map_err(app_db_error)?;

        Ok(result.rows_affected() > 0)
    }

    async fn insert_audit_event(&self, event: &AuditEvent) -> Result<(), HelixError> {
        sqlx::query(
            "INSERT INTO audit_log (subject, action, resource, decision, reason, metadata) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&event.subject)
        .bind(&event.action)
        .bind(&event.resource)
        .bind(&event.decision)
        .bind(&event.reason)
        .bind(&event.metadata)
        .execute(&self.pool)
        .await
        .map_err(app_db_error)?;
        Ok(())
    }

    async fn list_audit_events(&self, limit: usize) -> Result<Vec<AuditLogEntry>, HelixError> {
        let rows = sqlx::query(
            "SELECT id, subject, action, resource, decision, reason, metadata, created_at::text AS created_at FROM audit_log ORDER BY id DESC LIMIT $1",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(app_db_error)?;

        Ok(rows
            .into_iter()
            .map(|row| AuditLogEntry {
                id: row.get("id"),
                subject: row.get("subject"),
                action: row.get("action"),
                resource: row.get("resource"),
                decision: row.get("decision"),
                reason: row.get("reason"),
                metadata: row.get("metadata"),
                created_at: row.get("created_at"),
            })
            .collect())
    }
}

impl SymbolicProgramCache {
    fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: Vec::new(),
            capacity,
        }
    }

    fn get(&self, fingerprint: &str) -> Option<Arc<CompiledSymbolicProgram>> {
        self.entries.get(fingerprint).cloned()
    }

    fn insert(&mut self, program: Arc<CompiledSymbolicProgram>) {
        let fingerprint = program.fingerprint().to_string();
        self.order.retain(|existing| existing != &fingerprint);
        self.order.push(fingerprint.clone());
        self.entries.insert(fingerprint.clone(), program);

        while self.order.len() > self.capacity {
            let evicted = self.order.remove(0);
            self.entries.remove(&evicted);
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let (llm_provider, llm_model) = match llm_provider_from_env() {
        Some((provider, model)) => (Some(provider), Some(model)),
        None => (None, None),
    };
    let postgres_pool = postgres_pool_from_env().await;
    if let Some(pool) = postgres_pool.as_ref() {
        run_startup_migrations_if_enabled(pool)
            .await
            .expect("failed to run Helix startup migrations");
    }
    let intel_persistence = postgres_pool
        .as_ref()
        .map(|pool| Arc::new(IntelDeskPostgresStore::new(pool.clone())));
    let state_persistence = postgres_pool
        .as_ref()
        .map(|pool| Arc::new(AppPostgresStore::new(pool.clone())));
    let intel_desk = match intel_persistence.as_ref() {
        Some(persistence) => persistence
            .load_or_seed()
            .await
            .expect("failed to load persisted intelligence desk state"),
        None => IntelDeskStore::default(),
    };
    let policy_config = match state_persistence.as_ref() {
        Some(persistence) => persistence
            .load_policy_config()
            .await
            .expect("failed to load persisted policy config")
            .unwrap_or_default(),
        None => DeterministicPolicyConfig::default(),
    };
    let autopilot_guard = match state_persistence.as_ref() {
        Some(persistence) => persistence
            .load_autopilot_guard()
            .await
            .expect("failed to load persisted autopilot guard state")
            .unwrap_or_else(|| AutopilotGuardMachine::new(autopilot_config_from_env())),
        None => AutopilotGuardMachine::new(autopilot_config_from_env()),
    };
    let automation_rules = match state_persistence.as_ref() {
        Some(persistence) => persistence
            .load_automation_rules()
            .await
            .expect("failed to load persisted automation rules"),
        None => Vec::new(),
    };
    let recipes = match state_persistence.as_ref() {
        Some(persistence) => persistence
            .load_recipes()
            .await
            .expect("failed to load persisted recipes"),
        None => Vec::new(),
    };

    let state = AppState {
        policy_config: Arc::new(RwLock::new(policy_config)),
        autopilot_guard: Arc::new(RwLock::new(autopilot_guard)),
        intel_desk: Arc::new(RwLock::new(intel_desk)),
        intel_persistence,
        state_persistence,
        automation_rules: Arc::new(RwLock::new(automation_rules)),
        recipes: Arc::new(RwLock::new(recipes)),
        symbolic_program_cache: Arc::new(RwLock::new(SymbolicProgramCache::new(
            SYMBOLIC_PROGRAM_CACHE_CAPACITY,
        ))),
        llm_provider,
        llm_model,
        auth_service: Arc::new(api_auth_from_env()),
    };
    let app = app_with_optional_static_ui(state);

    let addr = api_addr_from_env();
    tracing::info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Serialize)]
struct HealthStatus {
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PolicyConfigResponse {
    config: DeterministicPolicyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SimulationRequest {
    commands: Vec<PolicyCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SimulationResponse {
    steps: Vec<PolicyStepResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentCatalogResponse {
    agents: Vec<DeterministicAgentSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentCatalogQualityResponse {
    quality: AgentCatalogQuality,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GuardSimulationRequest {
    agent_id: String,
    threshold: Option<u32>,
    strike_limit: Option<u8>,
    cooldown_ticks: Option<u8>,
    commands: Vec<GuardSimulationCommand>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum GuardSimulationCommand {
    Evaluate { value: u32 },
    Tick,
    Reset,
}

impl From<GuardSimulationCommand> for TemporalGuardInput {
    fn from(value: GuardSimulationCommand) -> Self {
        match value {
            GuardSimulationCommand::Evaluate { value } => TemporalGuardInput::Evaluate { value },
            GuardSimulationCommand::Tick => TemporalGuardInput::Tick,
            GuardSimulationCommand::Reset => TemporalGuardInput::Reset,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentTemplateCatalogResponse {
    templates: Vec<DeterministicAgentTemplate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentTemplateResponse {
    template: DeterministicAgentTemplate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AutomationRuleCatalogResponse {
    persistence_enabled: bool,
    rules: Vec<Rule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AutomationRuleUpsertRequest {
    rule: Rule,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AutomationRuleResponse {
    persistence_enabled: bool,
    rule: Rule,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AutomationRuleEvaluateRequest {
    event: Event,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AutomationRuleEvaluationEntry {
    id: i64,
    event_id: String,
    event_type: String,
    event_source: String,
    event: Value,
    rule_count: usize,
    trigger_plan_count: usize,
    trigger_plans: Vec<RecipeTriggerPlan>,
    created_at: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct AutomationRuleEvaluationQuery {
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AutomationRuleEvaluationCatalogResponse {
    persistence_enabled: bool,
    entries: Vec<AutomationRuleEvaluationEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AutomationRuleEvaluateResponse {
    rule_count: usize,
    trigger_plans: Vec<RecipeTriggerPlan>,
    evaluation: Option<AutomationRuleEvaluationEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecipeCatalogResponse {
    persistence_enabled: bool,
    recipes: Vec<Recipe>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecipeUpsertRequest {
    recipe: Recipe,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecipeResponse {
    persistence_enabled: bool,
    recipe: Recipe,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecipeTriggerRunRequest {
    plan: RecipeTriggerPlan,
    evaluation_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecipeTriggerRunResponse {
    persistence_enabled: bool,
    run: Option<RecipeRunEntry>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RecipeRunQuery {
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecipeRunCatalogResponse {
    persistence_enabled: bool,
    entries: Vec<RecipeRunEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecipeRunEntry {
    id: i64,
    evaluation_id: Option<i64>,
    rule_id: String,
    action_id: Option<String>,
    requested_recipe_id: Option<String>,
    requested_recipe_name: Option<String>,
    resolved_recipe_id: Option<String>,
    resolved_recipe_name: Option<String>,
    trigger_plan: RecipeTriggerPlan,
    parameters: Value,
    status: String,
    started_agent_ids: Vec<String>,
    emitted_events: Vec<Value>,
    state_snapshots: Value,
    error: Option<String>,
    created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RecipeRuntimeOutput {
    started_agent_ids: Vec<AgentId>,
    emitted_events: Vec<Value>,
    state_snapshots: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ApplyAgentTemplateRequest {
    run_bootstrap_simulation: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApplyAgentTemplateResponse {
    template: DeterministicAgentTemplate,
    config: DeterministicPolicyConfig,
    bootstrap_steps: Option<Vec<PolicyStepResult>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OnchainBroadcastRequest {
    rpc_url: String,
    raw_tx_hex: String,
    await_receipt: Option<bool>,
    max_poll_rounds: Option<u16>,
    poll_interval_ms: Option<u64>,
    dry_run: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OnchainBroadcastResponse {
    phase: OnchainPhase,
    tx_hash: Option<String>,
    poll_rounds: u16,
    max_poll_rounds: u16,
    receipt: Option<EvmReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OnchainReceiptRequest {
    rpc_url: String,
    tx_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OnchainReceiptResponse {
    found: bool,
    receipt: Option<EvmReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiErrorResponse {
    error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReasoningEvaluateResponse {
    decision: ReasoningDecision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AutopilotStatusResponse {
    config: AutopilotGuardConfig,
    stats: AutopilotStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AutopilotConfigUpdateRequest {
    config: AutopilotGuardConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AutopilotActionRequest {
    PolicySimulation { commands: Vec<PolicyCommand> },
    OnchainBroadcast { request: OnchainBroadcastRequest },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AutopilotExecuteRequest {
    confirmed_by_human: bool,
    action: AutopilotActionRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AutopilotExecuteResponse {
    allowed: bool,
    reason: Option<String>,
    requires_confirmation: bool,
    result: Option<Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AutopilotProposeKind {
    PolicySimulation,
    OnchainBroadcast,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AutopilotProposeRequest {
    goal: String,
    kind: AutopilotProposeKind,
    rpc_url: Option<String>,
    raw_tx_hex: Option<String>,
    dry_run: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AutopilotReviewProposeRequest {
    review_kind: AutopilotReviewKind,
    item_id: String,
    kind: AutopilotProposeKind,
    rpc_url: Option<String>,
    raw_tx_hex: Option<String>,
    dry_run: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AutopilotGuardPreview {
    action_class: AutopilotActionClass,
    decision_unconfirmed: AutopilotGuardDecision,
    decision_confirmed: AutopilotGuardDecision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AutopilotProposeResponse {
    model: String,
    raw: String,
    action: AutopilotActionRequest,
    guard_preview: AutopilotGuardPreview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AutopilotProposeErrorResponse {
    error: String,
    model: Option<String>,
    raw: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AutopilotReviewProposeResponse {
    review_item: AutopilotReviewQueueEntry,
    proposal: AutopilotProposeResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum ProposedAction {
    PolicySimulation {
        commands: Vec<PolicyCommand>,
    },
    OnchainBroadcast {
        request: ProposedOnchainBroadcastRequest,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposedOnchainBroadcastRequest {
    rpc_url: String,
    raw_tx_hex: String,
    await_receipt: Option<bool>,
    max_poll_rounds: Option<u16>,
    poll_interval_ms: Option<u64>,
    dry_run: Option<bool>,
}

impl From<ProposedOnchainBroadcastRequest> for OnchainBroadcastRequest {
    fn from(value: ProposedOnchainBroadcastRequest) -> Self {
        Self {
            rpc_url: value.rpc_url,
            raw_tx_hex: value.raw_tx_hex,
            await_receipt: value.await_receipt,
            max_poll_rounds: value.max_poll_rounds,
            poll_interval_ms: value.poll_interval_ms,
            dry_run: value.dry_run,
        }
    }
}

async fn health_check() -> impl IntoResponse {
    let health = HealthStatus {
        status: "ok".to_string(),
    };
    (StatusCode::OK, Json(health))
}

async fn get_audit_log(
    State(state): State<AppState>,
    Query(query): Query<AuditLogQuery>,
) -> Response {
    let limit = match normalize_audit_limit(query.limit) {
        Ok(limit) => limit,
        Err(error) => return api_error_response(error),
    };

    let Some(persistence) = state.state_persistence.as_ref() else {
        return (
            StatusCode::OK,
            Json(AuditLogResponse {
                persistence_enabled: false,
                entries: Vec::new(),
            }),
        )
            .into_response();
    };

    match persistence.list_audit_events(limit).await {
        Ok(entries) => (
            StatusCode::OK,
            Json(AuditLogResponse {
                persistence_enabled: true,
                entries,
            }),
        )
            .into_response(),
        Err(error) => api_error_response(error),
    }
}

async fn list_credentials(
    State(state): State<AppState>,
    Query(query): Query<CredentialQuery>,
) -> Response {
    let profile_id = match parse_profile_id_query(query.profile_id) {
        Ok(profile_id) => profile_id,
        Err(error) => return api_error_response(error),
    };

    let Some(persistence) = state.state_persistence.as_ref() else {
        return (
            StatusCode::OK,
            Json(CredentialCatalogResponse {
                persistence_enabled: false,
                credentials: Vec::new(),
            }),
        )
            .into_response();
    };

    match persistence.list_credentials(&profile_id).await {
        Ok(credentials) => (
            StatusCode::OK,
            Json(CredentialCatalogResponse {
                persistence_enabled: true,
                credentials,
            }),
        )
            .into_response(),
        Err(error) => api_error_response(error),
    }
}

async fn upsert_credential(
    State(state): State<AppState>,
    Json(req): Json<CredentialUpsertRequest>,
) -> Response {
    let Some(persistence) = state.state_persistence.as_ref() else {
        return credential_service_unavailable("credential vault requires DATABASE_URL");
    };

    let pending = match validate_credential_upsert(req) {
        Ok(pending) => pending,
        Err(error) => return api_error_response(error),
    };
    let encrypter = match credential_encrypter_from_env() {
        Ok(encrypter) => encrypter,
        Err(error) => return credential_service_unavailable(error.to_string()),
    };
    let encrypted_data = match encrypter.encrypt(&pending.secret).await {
        Ok(encrypted_data) => encrypted_data,
        Err(error) => return api_error_response(HelixError::encryption_error(error.to_string())),
    };

    let mut credential = Credential::new(
        pending.id,
        pending.profile_id,
        pending.name,
        pending.kind,
        encrypted_data,
    );
    credential.metadata = pending.metadata.into_iter().collect();

    let credential = match persistence.upsert_credential(&credential).await {
        Ok(credential) => credential,
        Err(error) => return api_error_response(error),
    };
    let metadata_keys: Vec<String> = credential.metadata.keys().cloned().collect();
    let audit_event = AuditEvent::allow(
        "credential.upsert",
        format!("credentials/{}", credential.id),
        serde_json::json!({
            "credential_id": &credential.id,
            "profile_id": &credential.profile_id,
            "name": &credential.name,
            "kind": &credential.kind,
            "metadata_keys": metadata_keys
        }),
    );
    if let Err(error) = record_audit_event(&state, audit_event).await {
        return api_error_response(error);
    }

    (
        StatusCode::OK,
        Json(CredentialResponse {
            persistence_enabled: true,
            credential: Some(credential),
        }),
    )
        .into_response()
}

async fn delete_credential_handler(
    Path((profile_id, credential_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Response {
    let profile_id = match parse_profile_id_value(&profile_id) {
        Ok(profile_id) => profile_id,
        Err(error) => return api_error_response(error),
    };
    let credential_id = match parse_credential_id_value(&credential_id) {
        Ok(credential_id) => credential_id,
        Err(error) => return api_error_response(error),
    };

    let Some(persistence) = state.state_persistence.as_ref() else {
        return credential_service_unavailable("credential vault requires DATABASE_URL");
    };

    let deleted = match persistence
        .delete_credential(&profile_id, &credential_id)
        .await
    {
        Ok(deleted) => deleted,
        Err(error) => return api_error_response(error),
    };
    if !deleted {
        let audit_event = AuditEvent::deny(
            "credential.delete",
            format!("credentials/{credential_id}"),
            "credential not found for profile",
            serde_json::json!({
                "credential_id": credential_id,
                "profile_id": profile_id
            }),
        );
        if let Err(error) = record_audit_event(&state, audit_event).await {
            return api_error_response(error);
        }
        return api_error_response(HelixError::not_found("credential"));
    }

    let audit_event = AuditEvent::allow(
        "credential.delete",
        format!("credentials/{credential_id}"),
        serde_json::json!({
            "credential_id": credential_id,
            "profile_id": profile_id
        }),
    );
    if let Err(error) = record_audit_event(&state, audit_event).await {
        return api_error_response(error);
    }

    (
        StatusCode::OK,
        Json(CredentialDeleteResponse {
            persistence_enabled: true,
            deleted: true,
        }),
    )
        .into_response()
}

async fn get_policy_config(State(state): State<AppState>) -> impl IntoResponse {
    let config = *state.policy_config.read().await;
    (StatusCode::OK, Json(PolicyConfigResponse { config }))
}

async fn put_policy_config(
    State(state): State<AppState>,
    Json(req): Json<PolicyConfigResponse>,
) -> Response {
    match set_policy_config(&state, req.config).await {
        Ok(()) => (
            StatusCode::OK,
            Json(PolicyConfigResponse { config: req.config }),
        )
            .into_response(),
        Err(error) => api_error_response(error),
    }
}

async fn simulate_policy(
    State(state): State<AppState>,
    Json(req): Json<SimulationRequest>,
) -> impl IntoResponse {
    let config = *state.policy_config.read().await;
    let mut engine = DeterministicPolicyEngine::new(config);
    let steps = engine.simulate(&req.commands);
    (StatusCode::OK, Json(SimulationResponse { steps }))
}

async fn get_agent_catalog() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(AgentCatalogResponse {
            agents: high_roi_agent_catalog(),
        }),
    )
}

async fn get_agent_catalog_quality() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(AgentCatalogQualityResponse {
            quality: agent_catalog_quality(),
        }),
    )
}

async fn post_simulate_guard_agent(Json(req): Json<GuardSimulationRequest>) -> Response {
    let commands: Vec<TemporalGuardInput> = req.commands.into_iter().map(Into::into).collect();
    match simulate_expanded_guard(
        &req.agent_id,
        req.threshold,
        req.strike_limit,
        req.cooldown_ticks,
        &commands,
    ) {
        Ok(simulation) => (StatusCode::OK, Json(simulation)).into_response(),
        Err(message) => (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse { error: message }),
        )
            .into_response(),
    }
}

async fn post_reasoning_evaluate(
    State(state): State<AppState>,
    Json(req): Json<ReasoningEvaluationRequest>,
) -> Response {
    let decision = match req {
        ReasoningEvaluationRequest::KrrSymbolic {
            query,
            facts,
            rules,
            triples,
            consistency_scope,
            max_rounds,
        } => get_or_compile_symbolic_program(&state, rules, triples)
            .await
            .and_then(|program| {
                evaluate_compiled_symbolic_reasoning(
                    query,
                    facts,
                    program.as_ref(),
                    consistency_scope,
                    max_rounds,
                )
            }),
        ReasoningEvaluationRequest::NeuroSymbolic {
            query,
            facts,
            rules,
            triples,
            features,
            model,
            min_probability,
            consistency_scope,
            max_rounds,
        } => get_or_compile_symbolic_program(&state, rules, triples)
            .await
            .and_then(|program| {
                evaluate_compiled_neuro_symbolic_reasoning(
                    query,
                    facts,
                    program.as_ref(),
                    features,
                    model,
                    min_probability,
                    consistency_scope,
                    max_rounds,
                )
            }),
        other => evaluate_reasoning(other),
    };

    match decision {
        Ok(decision) => {
            (StatusCode::OK, Json(ReasoningEvaluateResponse { decision })).into_response()
        }
        Err(err) => api_error_response(err),
    }
}

async fn get_or_compile_symbolic_program(
    state: &AppState,
    rules: Vec<SymbolicRule>,
    triples: Vec<KrrTriple>,
) -> Result<Arc<CompiledSymbolicProgram>, HelixError> {
    let fingerprint = fingerprint_symbolic_program(&rules, &triples)?;
    if let Some(program) = state
        .symbolic_program_cache
        .read()
        .await
        .get(fingerprint.as_str())
    {
        return Ok(program);
    }

    let program = Arc::new(compile_symbolic_program(rules, triples)?);
    let mut cache = state.symbolic_program_cache.write().await;
    if let Some(existing) = cache.get(fingerprint.as_str()) {
        return Ok(existing);
    }
    cache.insert(program.clone());
    Ok(program)
}

async fn get_agent_templates() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(AgentTemplateCatalogResponse {
            templates: high_roi_agent_templates(),
        }),
    )
}

async fn get_agent_template(Path(template_id): Path<String>) -> Response {
    match find_agent_template(&template_id) {
        Some(template) => {
            (StatusCode::OK, Json(AgentTemplateResponse { template })).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiErrorResponse {
                error: format!("unknown agent template: {template_id}"),
            }),
        )
            .into_response(),
    }
}

async fn list_recipes(State(state): State<AppState>) -> impl IntoResponse {
    let recipes = state.recipes.read().await.clone();
    (
        StatusCode::OK,
        Json(RecipeCatalogResponse {
            persistence_enabled: state.state_persistence.is_some(),
            recipes,
        }),
    )
}

async fn upsert_recipe(
    State(state): State<AppState>,
    Json(req): Json<RecipeUpsertRequest>,
) -> Response {
    match set_recipe(&state, req.recipe).await {
        Ok(recipe) => (
            StatusCode::OK,
            Json(RecipeResponse {
                persistence_enabled: state.state_persistence.is_some(),
                recipe,
            }),
        )
            .into_response(),
        Err(error) => api_error_response(error),
    }
}

async fn list_recipe_runs(
    State(state): State<AppState>,
    Query(query): Query<RecipeRunQuery>,
) -> Response {
    let limit = match normalize_audit_limit(query.limit) {
        Ok(limit) => limit,
        Err(error) => return api_error_response(error),
    };

    let Some(persistence) = state.state_persistence.as_ref() else {
        return (
            StatusCode::OK,
            Json(RecipeRunCatalogResponse {
                persistence_enabled: false,
                entries: Vec::new(),
            }),
        )
            .into_response();
    };

    match persistence.list_recipe_runs(limit).await {
        Ok(entries) => (
            StatusCode::OK,
            Json(RecipeRunCatalogResponse {
                persistence_enabled: true,
                entries,
            }),
        )
            .into_response(),
        Err(error) => api_error_response(error),
    }
}

async fn run_recipe_trigger_plan(
    State(state): State<AppState>,
    Json(req): Json<RecipeTriggerRunRequest>,
) -> Response {
    let Some(persistence) = state.state_persistence.as_ref() else {
        return (
            StatusCode::OK,
            Json(RecipeTriggerRunResponse {
                persistence_enabled: false,
                run: None,
            }),
        )
            .into_response();
    };

    let recipes = state.recipes.read().await.clone();
    let resolved_recipe = match resolve_trigger_plan_recipe(&recipes, &req.plan) {
        Ok(recipe) => Some(recipe.clone()),
        Err(_) => None,
    };

    let (status, runtime_output, error) = match resolved_recipe.as_ref() {
        Some(recipe) => match run_recipe_via_api_runtime(recipe).await {
            Ok(output) => ("completed".to_string(), output, None),
            Err(error) => (
                "failed".to_string(),
                RecipeRuntimeOutput::default(),
                Some(error.to_string()),
            ),
        },
        None => {
            let error = resolve_trigger_plan_recipe(&recipes, &req.plan)
                .err()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "recipe resolution failed".to_string());
            (
                "failed".to_string(),
                RecipeRuntimeOutput::default(),
                Some(error),
            )
        }
    };

    let run = match persistence
        .insert_recipe_run(
            req.evaluation_id,
            &req.plan,
            resolved_recipe.as_ref(),
            &status,
            &runtime_output,
            error.as_deref(),
        )
        .await
    {
        Ok(run) => run,
        Err(error) => return api_error_response(error),
    };

    let audit_event = if status == "completed" {
        AuditEvent::allow(
            "automation.recipe.run",
            format!(
                "automation/recipes/{}",
                run.resolved_recipe_id
                    .clone()
                    .unwrap_or_else(|| "unresolved".to_string())
            ),
            serde_json::json!({
                "run_id": run.id,
                "evaluation_id": run.evaluation_id,
                "rule_id": &run.rule_id,
                "status": &run.status,
                "started_agent_count": run.started_agent_ids.len()
            }),
        )
    } else {
        AuditEvent::deny(
            "automation.recipe.run",
            run.requested_recipe_id
                .clone()
                .or_else(|| run.requested_recipe_name.clone())
                .unwrap_or_else(|| "unresolved".to_string()),
            run.error
                .clone()
                .unwrap_or_else(|| "recipe run failed".to_string()),
            serde_json::json!({
                "run_id": run.id,
                "evaluation_id": run.evaluation_id,
                "rule_id": &run.rule_id,
                "status": &run.status
            }),
        )
    };
    if let Err(error) = record_audit_event(&state, audit_event).await {
        return api_error_response(error);
    }

    (
        StatusCode::OK,
        Json(RecipeTriggerRunResponse {
            persistence_enabled: true,
            run: Some(run),
        }),
    )
        .into_response()
}

async fn list_automation_rules(State(state): State<AppState>) -> impl IntoResponse {
    let rules = state.automation_rules.read().await.clone();
    (
        StatusCode::OK,
        Json(AutomationRuleCatalogResponse {
            persistence_enabled: state.state_persistence.is_some(),
            rules,
        }),
    )
}

async fn upsert_automation_rule(
    State(state): State<AppState>,
    Json(req): Json<AutomationRuleUpsertRequest>,
) -> Response {
    match set_automation_rule(&state, req.rule).await {
        Ok(rule) => (
            StatusCode::OK,
            Json(AutomationRuleResponse {
                persistence_enabled: state.state_persistence.is_some(),
                rule,
            }),
        )
            .into_response(),
        Err(error) => api_error_response(error),
    }
}

async fn list_automation_rule_evaluations(
    State(state): State<AppState>,
    Query(query): Query<AutomationRuleEvaluationQuery>,
) -> Response {
    let limit = match normalize_audit_limit(query.limit) {
        Ok(limit) => limit,
        Err(error) => return api_error_response(error),
    };

    let Some(persistence) = state.state_persistence.as_ref() else {
        return (
            StatusCode::OK,
            Json(AutomationRuleEvaluationCatalogResponse {
                persistence_enabled: false,
                entries: Vec::new(),
            }),
        )
            .into_response();
    };

    match persistence.list_automation_rule_evaluations(limit).await {
        Ok(entries) => (
            StatusCode::OK,
            Json(AutomationRuleEvaluationCatalogResponse {
                persistence_enabled: true,
                entries,
            }),
        )
            .into_response(),
        Err(error) => api_error_response(error),
    }
}

async fn evaluate_automation_rules(
    State(state): State<AppState>,
    Json(req): Json<AutomationRuleEvaluateRequest>,
) -> Response {
    let rules = state.automation_rules.read().await.clone();
    let listener = RuleEngineEventListener::new(rules.clone());
    let trigger_plans = listener.handle_event(&req.event);
    let evaluation = match state.state_persistence.as_ref() {
        Some(persistence) => match persistence
            .insert_automation_rule_evaluation(&req.event, rules.len(), &trigger_plans)
            .await
        {
            Ok(evaluation) => Some(evaluation),
            Err(error) => return api_error_response(error),
        },
        None => None,
    };
    let event = AuditEvent::allow(
        "automation.rules.evaluate",
        "automation/rules",
        serde_json::json!({
            "event_id": req.event.id,
            "event_type": req.event.r#type,
            "rule_count": rules.len(),
            "trigger_plan_count": trigger_plans.len(),
            "evaluation_id": evaluation.as_ref().map(|entry| entry.id)
        }),
    );
    if let Err(error) = record_audit_event(&state, event).await {
        return api_error_response(error);
    }

    (
        StatusCode::OK,
        Json(AutomationRuleEvaluateResponse {
            rule_count: rules.len(),
            trigger_plans,
            evaluation,
        }),
    )
        .into_response()
}

async fn post_apply_agent_template(
    Path(template_id): Path<String>,
    State(state): State<AppState>,
    Json(req): Json<ApplyAgentTemplateRequest>,
) -> Response {
    let Some(template) = find_agent_template(&template_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiErrorResponse {
                error: format!("unknown agent template: {template_id}"),
            }),
        )
            .into_response();
    };

    if let Err(error) = set_policy_config(&state, template.config).await {
        return api_error_response(error);
    }

    let bootstrap_steps = if req.run_bootstrap_simulation.unwrap_or(false) {
        let mut engine = DeterministicPolicyEngine::new(template.config);
        Some(engine.simulate(&template.bootstrap_commands))
    } else {
        None
    };

    (
        StatusCode::OK,
        Json(ApplyAgentTemplateResponse {
            config: template.config,
            template,
            bootstrap_steps,
        }),
    )
        .into_response()
}

async fn onchain_send_raw(Json(req): Json<OnchainBroadcastRequest>) -> Response {
    match run_onchain_broadcast(req).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => api_error_response(err),
    }
}

async fn onchain_get_receipt(Json(req): Json<OnchainReceiptRequest>) -> Response {
    let result = async {
        let client = EvmRpcClient::new(req.rpc_url)?;
        let receipt = client.get_transaction_receipt(&req.tx_hash).await?;
        Ok::<_, HelixError>(OnchainReceiptResponse {
            found: receipt.is_some(),
            receipt,
        })
    }
    .await;

    match result {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => api_error_response(err),
    }
}

fn api_error_response(error: HelixError) -> Response {
    let status = match error {
        HelixError::NotFound(_) => StatusCode::NOT_FOUND,
        HelixError::ValidationError { .. } => StatusCode::BAD_REQUEST,
        HelixError::ExternalServiceError { .. } => StatusCode::BAD_GATEWAY,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (
        status,
        Json(ApiErrorResponse {
            error: error.to_string(),
        }),
    )
        .into_response()
}

fn app_db_error(error: sqlx::Error) -> HelixError {
    HelixError::InternalError(format!("app state persistence error: {error}"))
}

fn automation_rule_evaluation_from_row(
    row: PgRow,
) -> Result<AutomationRuleEvaluationEntry, HelixError> {
    let rule_count: i32 = row.get("rule_count");
    let trigger_plan_count: i32 = row.get("trigger_plan_count");
    let trigger_plans: Vec<RecipeTriggerPlan> =
        serde_json::from_value(row.get::<Value, _>("trigger_plans"))?;

    Ok(AutomationRuleEvaluationEntry {
        id: row.get("id"),
        event_id: row.get("event_id"),
        event_type: row.get("event_type"),
        event_source: row.get("event_source"),
        event: row.get("event"),
        rule_count: rule_count.max(0) as usize,
        trigger_plan_count: trigger_plan_count.max(0) as usize,
        trigger_plans,
        created_at: row.get("created_at"),
    })
}

fn recipe_run_from_row(row: PgRow) -> Result<RecipeRunEntry, HelixError> {
    let trigger_plan: RecipeTriggerPlan =
        serde_json::from_value(row.get::<Value, _>("trigger_plan"))?;
    let started_agent_ids: Vec<AgentId> = row.get("started_agent_ids");
    let emitted_events = match row.get::<Value, _>("emitted_events") {
        Value::Array(events) => events,
        value => {
            return Err(HelixError::InternalError(format!(
                "recipe run emitted_events must be an array, got {value}"
            )))
        }
    };

    Ok(RecipeRunEntry {
        id: row.get("id"),
        evaluation_id: row.get("evaluation_id"),
        rule_id: row.get("rule_id"),
        action_id: row.get("action_id"),
        requested_recipe_id: row.get("requested_recipe_id"),
        requested_recipe_name: row.get("requested_recipe_name"),
        resolved_recipe_id: row.get("resolved_recipe_id"),
        resolved_recipe_name: row.get("resolved_recipe_name"),
        trigger_plan,
        parameters: row.get("parameters"),
        status: row.get("status"),
        started_agent_ids: started_agent_ids
            .into_iter()
            .map(|agent_id| agent_id.to_string())
            .collect(),
        emitted_events,
        state_snapshots: row.get("state_snapshots"),
        error: row.get("error"),
        created_at: row.get("created_at"),
    })
}

fn credential_metadata_from_row(row: PgRow) -> Result<CredentialMetadataEntry, HelixError> {
    let metadata: BTreeMap<String, String> =
        serde_json::from_value(row.get::<Value, _>("metadata"))?;
    Ok(CredentialMetadataEntry {
        id: row.get("id"),
        profile_id: row.get("profile_id"),
        name: row.get("name"),
        kind: row.get("kind"),
        metadata,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn validate_credential_upsert(
    req: CredentialUpsertRequest,
) -> Result<PendingCredentialUpsert, HelixError> {
    let profile_id = parse_profile_id_value(&req.profile_id)?;
    let id = match req.id {
        Some(value) => parse_credential_id_value(&value)?,
        None => CredentialId::new_v4(),
    };
    let name = normalize_bounded_text("credential.name", &req.name, MAX_CREDENTIAL_NAME_LEN)?;
    let kind = normalize_bounded_text("credential.kind", &req.kind, MAX_CREDENTIAL_KIND_LEN)?;
    if req.secret.is_empty() {
        return Err(HelixError::validation_error(
            "credential.secret",
            "must not be empty",
        ));
    }
    if req.secret.len() > MAX_CREDENTIAL_SECRET_LEN {
        return Err(HelixError::validation_error(
            "credential.secret",
            "must be at most 16384 bytes",
        ));
    }
    let metadata = normalize_credential_metadata(req.metadata)?;

    Ok(PendingCredentialUpsert {
        id,
        profile_id,
        name,
        kind,
        secret: req.secret,
        metadata,
    })
}

fn parse_profile_id_query(value: Option<String>) -> Result<ProfileId, HelixError> {
    let Some(value) = value else {
        return Err(HelixError::validation_error(
            "profile_id",
            "query parameter is required",
        ));
    };
    parse_profile_id_value(&value)
}

fn parse_profile_id_value(value: &str) -> Result<ProfileId, HelixError> {
    let profile_id: ProfileId = value
        .parse()
        .map_err(|_| HelixError::validation_error("profile_id", "must be a valid UUID"))?;
    if profile_id.is_nil() {
        return Err(HelixError::validation_error(
            "profile_id",
            "must not be nil",
        ));
    }
    Ok(profile_id)
}

fn parse_credential_id_value(value: &str) -> Result<CredentialId, HelixError> {
    let credential_id: CredentialId = value
        .parse()
        .map_err(|_| HelixError::validation_error("credential_id", "must be a valid UUID"))?;
    if credential_id.is_nil() {
        return Err(HelixError::validation_error(
            "credential_id",
            "must not be nil",
        ));
    }
    Ok(credential_id)
}

fn normalize_bounded_text(
    context: &str,
    value: &str,
    max_len: usize,
) -> Result<String, HelixError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(HelixError::validation_error(context, "must not be empty"));
    }
    if value.len() > max_len {
        return Err(HelixError::validation_error(
            context.to_string(),
            format!("must be at most {max_len} bytes"),
        ));
    }
    Ok(value.to_string())
}

fn normalize_credential_metadata(
    metadata: BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, HelixError> {
    if metadata.len() > MAX_CREDENTIAL_METADATA_ENTRIES {
        return Err(HelixError::validation_error(
            "credential.metadata",
            "must contain at most 32 entries",
        ));
    }

    let mut normalized = BTreeMap::new();
    for (key, value) in metadata {
        let key = normalize_bounded_text(
            "credential.metadata.key",
            &key,
            MAX_CREDENTIAL_METADATA_KEY_LEN,
        )?;
        if value.len() > MAX_CREDENTIAL_METADATA_VALUE_LEN {
            return Err(HelixError::validation_error(
                "credential.metadata.value",
                "must be at most 512 bytes",
            ));
        }
        normalized.insert(key, value);
    }
    Ok(normalized)
}

pub(crate) fn credential_encrypter_from_env(
) -> Result<AesGcmCredentialEncrypterDecrypter, HelixError> {
    AesGcmCredentialEncrypterDecrypter::new().map_err(|error| {
        HelixError::config_error(format!("credential encryption unavailable: {error}"))
    })
}

fn credential_service_unavailable(message: impl Into<String>) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ApiErrorResponse {
            error: message.into(),
        }),
    )
        .into_response()
}

fn normalize_audit_limit(limit: Option<usize>) -> Result<usize, HelixError> {
    let limit = limit.unwrap_or(50);
    if limit == 0 || limit > 200 {
        return Err(HelixError::validation_error(
            "limit",
            "must be between 1 and 200",
        ));
    }
    Ok(limit)
}

pub(crate) async fn record_audit_event(
    state: &AppState,
    event: AuditEvent,
) -> Result<(), HelixError> {
    if let Some(persistence) = state.state_persistence.as_ref() {
        persistence.insert_audit_event(&event).await?;
    }
    Ok(())
}

async fn persist_policy_config(
    state: &AppState,
    config: &DeterministicPolicyConfig,
) -> Result<(), HelixError> {
    if let Some(persistence) = state.state_persistence.as_ref() {
        persistence.save_policy_config(config).await?;
    }
    Ok(())
}

async fn persist_recipe(state: &AppState, recipe: &Recipe) -> Result<(), HelixError> {
    if let Some(persistence) = state.state_persistence.as_ref() {
        persistence.upsert_recipe(recipe).await?;
    }
    Ok(())
}

async fn set_recipe(state: &AppState, recipe: Recipe) -> Result<Recipe, HelixError> {
    validate_recipe_definition(&recipe)?;
    persist_recipe(state, &recipe).await?;
    record_audit_event(
        state,
        AuditEvent::allow(
            "automation.recipe.upsert",
            format!("automation/recipes/{}", recipe.id),
            serde_json::json!({
                "recipe_id": recipe.id,
                "name": &recipe.name,
                "enabled": recipe.enabled,
                "agent_count": recipe.agent_count()
            }),
        ),
    )
    .await?;

    let mut recipes = state.recipes.write().await;
    if let Some(existing) = recipes.iter_mut().find(|existing| existing.id == recipe.id) {
        *existing = recipe.clone();
    } else {
        recipes.push(recipe.clone());
    }
    recipes.sort_by_key(|recipe| recipe.id);
    Ok(recipe)
}

fn validate_recipe_definition(recipe: &Recipe) -> Result<(), HelixError> {
    if recipe.id.is_nil() {
        return Err(HelixError::validation_error("recipe.id", "must not be nil"));
    }
    if recipe.profile_id.is_nil() {
        return Err(HelixError::validation_error(
            "recipe.profile_id",
            "must not be nil",
        ));
    }
    recipe.validate()?;
    for agent in &recipe.graph.agents {
        agent.validate()?;
    }
    Ok(())
}

async fn persist_automation_rule(state: &AppState, rule: &Rule) -> Result<(), HelixError> {
    if let Some(persistence) = state.state_persistence.as_ref() {
        persistence.upsert_automation_rule(rule).await?;
    }
    Ok(())
}

async fn set_automation_rule(state: &AppState, rule: Rule) -> Result<Rule, HelixError> {
    validate_automation_rule(&rule)?;
    persist_automation_rule(state, &rule).await?;
    record_audit_event(
        state,
        AuditEvent::allow(
            "automation.rule.upsert",
            format!("automation/rules/{}", rule.id),
            serde_json::json!({
                "rule_id": rule.id,
                "name": &rule.name,
                "enabled": rule.enabled,
                "action_count": rule.actions.len()
            }),
        ),
    )
    .await?;

    let mut rules = state.automation_rules.write().await;
    if let Some(existing) = rules.iter_mut().find(|existing| existing.id == rule.id) {
        *existing = rule.clone();
    } else {
        rules.push(rule.clone());
    }
    rules.sort_by_key(|rule| rule.id);
    Ok(rule)
}

fn validate_automation_rule(rule: &Rule) -> Result<(), HelixError> {
    if rule.id.is_nil() {
        return Err(HelixError::validation_error("rule.id", "must not be nil"));
    }
    if rule.name.trim().is_empty() {
        return Err(HelixError::validation_error(
            "rule.name",
            "must not be empty",
        ));
    }
    if rule.version.trim().is_empty() {
        return Err(HelixError::validation_error(
            "rule.version",
            "must not be empty",
        ));
    }
    if rule.actions.is_empty() {
        return Err(HelixError::validation_error(
            "rule.actions",
            "must include at least one action",
        ));
    }

    for (index, action) in rule.actions.iter().enumerate() {
        let context = format!("rule.actions[{index}]");
        if action.r#type != "trigger_recipe" {
            return Err(HelixError::validation_error(
                context,
                "only trigger_recipe actions are supported".to_string(),
            ));
        }
        if action.delay.is_some() {
            return Err(HelixError::validation_error(
                context,
                "delayed rule actions are not supported".to_string(),
            ));
        }

        let has_recipe_id = action.recipe_id.is_some();
        let has_recipe_name = action
            .recipe_name
            .as_deref()
            .is_some_and(|name| !name.trim().is_empty());
        if has_recipe_id == has_recipe_name {
            return Err(HelixError::validation_error(
                context,
                "must target exactly one recipe_id or recipe_name".to_string(),
            ));
        }

        for (key, value) in &action.parameters {
            if key.trim().is_empty() {
                return Err(HelixError::validation_error(
                    context,
                    "parameter keys must not be empty".to_string(),
                ));
            }
            if let ParameterValue::FromEvent(path) = value {
                if path.trim().is_empty() {
                    return Err(HelixError::validation_error(
                        context,
                        "from_event parameter paths must not be empty".to_string(),
                    ));
                }
            }
        }
    }

    Ok(())
}

fn resolve_trigger_plan_recipe<'a>(
    recipes: &'a [Recipe],
    plan: &RecipeTriggerPlan,
) -> Result<&'a Recipe, HelixError> {
    let recipe_name = plan
        .recipe_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty());

    match (plan.recipe_id, recipe_name) {
        (Some(recipe_id), None) => recipes
            .iter()
            .find(|recipe| recipe.id == recipe_id)
            .ok_or_else(|| HelixError::not_found(format!("recipe {}", recipe_id))),
        (None, Some(name)) => {
            let mut matches = recipes.iter().filter(|recipe| recipe.name == name);
            let Some(recipe) = matches.next() else {
                return Err(HelixError::not_found(format!("recipe name {}", name)));
            };
            if matches.next().is_some() {
                return Err(HelixError::validation_error(
                    "plan.recipe_name",
                    "matched more than one recipe",
                ));
            }
            Ok(recipe)
        }
        _ => Err(HelixError::validation_error(
            "plan.recipe",
            "must target exactly one recipe_id or recipe_name",
        )),
    }
}

async fn run_recipe_via_api_runtime(recipe: &Recipe) -> Result<RecipeRuntimeOutput, HelixError> {
    let mut registry = AgentRegistry::new().map_err(|error| {
        HelixError::config_error(format!("agent registry initialization failed: {error}"))
    })?;
    register_api_builtin_agent(&mut registry, "noop")?;
    register_api_builtin_agent(&mut registry, "emit_event")?;
    register_api_builtin_agent(&mut registry, "record_state")?;

    validate_recipe_runnable_by_api(recipe, &registry)?;

    let publisher = Arc::new(InMemoryEventCollector::new());
    let publisher_trait: Arc<dyn EventPublisher> = publisher.clone();
    let credentials: Arc<dyn CredentialProvider> = Arc::new(EnvCredentialProvider);
    let state_store = Arc::new(InMemoryStateStore::new());
    let state_store_trait: Arc<dyn StateStore> = state_store.clone();
    let mut runner = AgentRunner::new_native(
        Arc::new(registry),
        publisher_trait,
        credentials,
        state_store_trait,
    );
    let started_agent_ids = runner.run_recipe(recipe).await?;
    let emitted_events = publisher
        .snapshot()
        .into_iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()?;
    let state_snapshots = collect_recipe_state_snapshots(recipe, state_store.as_ref()).await?;

    Ok(RecipeRuntimeOutput {
        started_agent_ids,
        emitted_events,
        state_snapshots,
    })
}

fn register_api_builtin_agent(registry: &mut AgentRegistry, kind: &str) -> Result<(), HelixError> {
    if !registry.contains_kind(kind) {
        registry
            .register(kind, Box::new(built_in_recipe_agent_factory))
            .map_err(|error| {
                HelixError::config_error(format!(
                    "built-in agent registration failed for '{kind}': {error}"
                ))
            })?;
    }
    Ok(())
}

async fn collect_recipe_state_snapshots(
    recipe: &Recipe,
    state_store: &InMemoryStateStore,
) -> Result<Value, HelixError> {
    let mut snapshots = BTreeMap::new();
    for agent in recipe.execution_order()? {
        if let Some(state) = state_store.get_state(&recipe.profile_id, &agent.id).await? {
            snapshots.insert(agent.id.to_string(), state);
        }
    }

    let mut object = JsonMap::new();
    for (agent_id, state) in snapshots {
        object.insert(agent_id, state);
    }
    Ok(Value::Object(object))
}

fn validate_recipe_runnable_by_api(
    recipe: &Recipe,
    registry: &AgentRegistry,
) -> Result<(), HelixError> {
    validate_recipe_definition(recipe)?;
    for agent in recipe.execution_order()? {
        if agent.agent_runtime != AgentRuntime::Native {
            return Err(HelixError::validation_error(
                format!("Recipe.graph.agents[id={}].agent_runtime", agent.id),
                "API recipe runner currently supports native agents only".to_string(),
            ));
        }
        if !registry.contains_kind(agent.agent_kind.as_str()) {
            return Err(HelixError::validation_error(
                format!("Recipe.graph.agents[id={}].agent_kind", agent.id),
                format!("no registered runtime factory for '{}'", agent.agent_kind),
            ));
        }
    }
    Ok(())
}

async fn set_policy_config(
    state: &AppState,
    config: DeterministicPolicyConfig,
) -> Result<(), HelixError> {
    persist_policy_config(state, &config).await?;
    record_audit_event(
        state,
        AuditEvent::allow(
            "policy.config.update",
            "policy/config",
            serde_json::json!({ "config": config }),
        ),
    )
    .await?;
    *state.policy_config.write().await = config;
    Ok(())
}

async fn persist_autopilot_guard(
    state: &AppState,
    guard: &AutopilotGuardMachine,
) -> Result<(), HelixError> {
    if let Some(persistence) = state.state_persistence.as_ref() {
        persistence.save_autopilot_guard(guard).await?;
    }
    Ok(())
}

async fn set_autopilot_config(
    state: &AppState,
    config: AutopilotGuardConfig,
) -> Result<AutopilotGuardMachine, HelixError> {
    let mut guard = *state.autopilot_guard.read().await;
    let _ = guard.step(AutopilotGuardInput::SetConfig { config });
    persist_autopilot_guard(state, &guard).await?;
    record_audit_event(
        state,
        AuditEvent::allow(
            "autopilot.config.update",
            "autopilot/config",
            serde_json::json!({ "config": guard.config(), "stats": guard.stats() }),
        ),
    )
    .await?;
    *state.autopilot_guard.write().await = guard;
    Ok(guard)
}

async fn evaluate_autopilot_guard(
    state: &AppState,
    action: AutopilotActionClass,
    confirmed_by_human: bool,
) -> Result<(AutopilotGuardMachine, AutopilotGuardDecision), HelixError> {
    let mut guard = *state.autopilot_guard.read().await;
    let decision = guard.step(AutopilotGuardInput::Evaluate {
        action,
        confirmed_by_human,
    });
    persist_autopilot_guard(state, &guard).await?;
    let event = match &decision {
        AutopilotGuardDecision::Deny { reason } => AuditEvent::deny(
            "autopilot.execute.evaluate",
            "autopilot/execute",
            reason.clone(),
            serde_json::json!({
                "action_class": action,
                "confirmed_by_human": confirmed_by_human,
                "stats": guard.stats()
            }),
        ),
        AutopilotGuardDecision::Allow {
            requires_confirmation,
        } => AuditEvent::allow(
            "autopilot.execute.evaluate",
            "autopilot/execute",
            serde_json::json!({
                "action_class": action,
                "confirmed_by_human": confirmed_by_human,
                "requires_confirmation": requires_confirmation,
                "stats": guard.stats()
            }),
        ),
        AutopilotGuardDecision::ConfigUpdated => AuditEvent::allow(
            "autopilot.execute.evaluate",
            "autopilot/execute",
            serde_json::json!({
                "action_class": action,
                "confirmed_by_human": confirmed_by_human,
                "stats": guard.stats()
            }),
        ),
    };
    record_audit_event(state, event).await?;
    *state.autopilot_guard.write().await = guard;
    Ok((guard, decision))
}

async fn run_onchain_broadcast(
    req: OnchainBroadcastRequest,
) -> Result<OnchainBroadcastResponse, HelixError> {
    let max_poll_rounds = req.max_poll_rounds.unwrap_or(20).max(1);
    let poll_interval_ms = req.poll_interval_ms.unwrap_or(500).clamp(50, 60_000);
    let await_receipt = req.await_receipt.unwrap_or(true);
    let dry_run = req.dry_run.unwrap_or(false);

    let mut state = OnchainState::default();
    let start = onchain_step(
        state,
        OnchainInput::StartBroadcast {
            raw_tx_hex: req.raw_tx_hex.clone(),
            max_poll_rounds,
        },
    )
    .map_err(map_onchain_kernel_error)?;
    state = start.state;

    let tx_hash = if dry_run {
        deterministic_dry_run_hash(&req.raw_tx_hex)?
    } else {
        let client = EvmRpcClient::new(req.rpc_url.clone())?;
        client.send_raw_transaction(&req.raw_tx_hex).await?
    };

    state = onchain_step(
        state,
        OnchainInput::SubmitAccepted {
            tx_hash: tx_hash.clone(),
        },
    )
    .map_err(map_onchain_kernel_error)?
    .state;

    let mut receipt: Option<EvmReceipt> = None;
    if await_receipt && !dry_run {
        let client = EvmRpcClient::new(req.rpc_url)?;
        while state.phase == OnchainPhase::PendingReceipt {
            if state.poll_rounds >= state.max_poll_rounds {
                state = onchain_step(state, OnchainInput::PollTimeout)
                    .map_err(map_onchain_kernel_error)?
                    .state;
                break;
            }

            sleep(Duration::from_millis(poll_interval_ms)).await;
            let current_receipt = client.get_transaction_receipt(&tx_hash).await?;
            match current_receipt {
                Some(found) => {
                    let input = match found.execution_success() {
                        Some(true) => OnchainInput::ReceiptSuccess,
                        Some(false) => OnchainInput::ReceiptReverted,
                        None => OnchainInput::ReceiptPending,
                    };
                    receipt = Some(found);
                    state = onchain_step(state, input)
                        .map_err(map_onchain_kernel_error)?
                        .state;
                }
                None => {
                    state = onchain_step(state, OnchainInput::ReceiptPending)
                        .map_err(map_onchain_kernel_error)?
                        .state;
                }
            }
        }
    }

    Ok(OnchainBroadcastResponse {
        phase: state.phase,
        tx_hash: state.tx_hash,
        poll_rounds: state.poll_rounds,
        max_poll_rounds: state.max_poll_rounds,
        receipt,
    })
}

fn map_onchain_kernel_error(error: OnchainKernelError) -> HelixError {
    HelixError::validation_error("onchain_intent".to_string(), error.to_string())
}

async fn get_autopilot_status(State(state): State<AppState>) -> impl IntoResponse {
    let guard = *state.autopilot_guard.read().await;
    (
        StatusCode::OK,
        Json(AutopilotStatusResponse {
            config: guard.config(),
            stats: guard.stats(),
        }),
    )
}

async fn put_autopilot_config(
    State(state): State<AppState>,
    Json(req): Json<AutopilotConfigUpdateRequest>,
) -> Response {
    match set_autopilot_config(&state, req.config).await {
        Ok(guard) => (
            StatusCode::OK,
            Json(AutopilotStatusResponse {
                config: guard.config(),
                stats: guard.stats(),
            }),
        )
            .into_response(),
        Err(error) => api_error_response(error),
    }
}

async fn complete_autopilot_proposal(
    state: &AppState,
    req: AutopilotProposeRequest,
) -> Result<AutopilotProposeResponse, (StatusCode, AutopilotProposeErrorResponse)> {
    let Some(provider) = state.llm_provider.as_ref().map(Arc::clone) else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            AutopilotProposeErrorResponse {
                error: "llm_not_configured".to_string(),
                model: None,
                raw: None,
            },
        ));
    };

    let model = state
        .llm_model
        .clone()
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| "gpt-4o-mini".to_string());

    let guard_config = {
        let guard = *state.autopilot_guard.read().await;
        guard.config()
    };
    let policy_config = *state.policy_config.read().await;

    let system_prompt = build_autopilot_propose_system_prompt(req.kind, guard_config);

    let user_payload = serde_json::json!({
        "goal": req.goal,
        "kind": req.kind,
        "autopilot_guard_config": guard_config,
        "policy_config": policy_config,
        "hints": {
            "rpc_url": req.rpc_url,
            "raw_tx_hex": req.raw_tx_hex,
            "dry_run": req.dry_run,
        }
    })
    .to_string();

    let mut parameters = HashMap::new();
    parameters.insert("model".to_string(), Value::String(model.clone()));

    let llm_request = LlmRequest {
        system_prompt: Some(system_prompt),
        messages: vec![Message {
            role: MessageRole::User,
            content: user_payload,
            function_call: None,
        }],
        max_tokens: Some(512),
        temperature: Some(0.0),
        top_p: Some(1.0),
        functions: None,
        parameters,
    };

    let llm_response = match provider.complete(llm_request).await {
        Ok(resp) => resp,
        Err(err) => {
            return Err((
                StatusCode::BAD_GATEWAY,
                AutopilotProposeErrorResponse {
                    error: format!("llm_error: {}", err),
                    model: None,
                    raw: None,
                },
            ));
        }
    };

    let mut action = match parse_llm_action_proposal(&llm_response.content) {
        Ok(proposed) => match proposed {
            ProposedAction::PolicySimulation { commands } => {
                AutopilotActionRequest::PolicySimulation { commands }
            }
            ProposedAction::OnchainBroadcast { request } => {
                AutopilotActionRequest::OnchainBroadcast {
                    request: request.into(),
                }
            }
        },
        Err(err) => {
            return Err((
                StatusCode::BAD_GATEWAY,
                AutopilotProposeErrorResponse {
                    error: format!("llm_invalid_json: {}", err),
                    model: Some(llm_response.model),
                    raw: Some(llm_response.content),
                },
            ));
        }
    };

    if let AutopilotActionRequest::OnchainBroadcast { request } = &mut action {
        if let Some(rpc_url) = req.rpc_url {
            request.rpc_url = rpc_url;
        }
        if let Some(raw_tx_hex) = req.raw_tx_hex {
            request.raw_tx_hex = raw_tx_hex;
        }
        if let Some(dry_run) = req.dry_run {
            request.dry_run = Some(dry_run);
        }
    }

    let action_class = match &action {
        AutopilotActionRequest::PolicySimulation { commands } => {
            let count = commands.len().min(usize::from(u16::MAX)) as u16;
            AutopilotActionClass::PolicySimulation {
                command_count: count,
            }
        }
        AutopilotActionRequest::OnchainBroadcast { request } => {
            AutopilotActionClass::OnchainBroadcast {
                dry_run: request.dry_run.unwrap_or(false),
            }
        }
    };

    let guard = *state.autopilot_guard.read().await;
    let decision_unconfirmed = {
        let mut preview = guard;
        preview.step(AutopilotGuardInput::Evaluate {
            action: action_class,
            confirmed_by_human: false,
        })
    };
    let decision_confirmed = {
        let mut preview = guard;
        preview.step(AutopilotGuardInput::Evaluate {
            action: action_class,
            confirmed_by_human: true,
        })
    };

    Ok(AutopilotProposeResponse {
        model: llm_response.model,
        raw: llm_response.content,
        action,
        guard_preview: AutopilotGuardPreview {
            action_class,
            decision_unconfirmed,
            decision_confirmed,
        },
    })
}

async fn post_autopilot_propose(
    State(state): State<AppState>,
    Json(req): Json<AutopilotProposeRequest>,
) -> Response {
    if req.goal.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                error: "goal is required".to_string(),
            }),
        )
            .into_response();
    }

    match complete_autopilot_proposal(&state, req).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err((status, error)) => (status, Json(error)).into_response(),
    }
}

async fn post_autopilot_review_propose(
    State(state): State<AppState>,
    Json(req): Json<AutopilotReviewProposeRequest>,
) -> Response {
    if req.item_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                error: "item_id is required".to_string(),
            }),
        )
            .into_response();
    }

    let review_item = {
        let store = state.intel_desk.read().await;
        match store.autopilot_review_item(req.review_kind, req.item_id.trim()) {
            Ok(item) => item,
            Err(error) => return api_error_response(error),
        }
    };

    let goal = format!(
        "{} Context: title='{}'; summary='{}'; label='{}'; route='{}'; priority_total={}.",
        review_item.goal_hint,
        review_item.title,
        review_item.summary,
        review_item.context_label,
        review_item.route,
        review_item.priority.total
    );

    let propose_request = AutopilotProposeRequest {
        goal,
        kind: req.kind,
        rpc_url: req.rpc_url,
        raw_tx_hex: req.raw_tx_hex,
        dry_run: req.dry_run,
    };

    match complete_autopilot_proposal(&state, propose_request).await {
        Ok(proposal) => (
            StatusCode::OK,
            Json(AutopilotReviewProposeResponse {
                review_item,
                proposal,
            }),
        )
            .into_response(),
        Err((status, error)) => (status, Json(error)).into_response(),
    }
}

async fn post_autopilot_execute(
    State(state): State<AppState>,
    Json(req): Json<AutopilotExecuteRequest>,
) -> Response {
    let action_class = match &req.action {
        AutopilotActionRequest::PolicySimulation { commands } => {
            let count = commands.len().min(usize::from(u16::MAX)) as u16;
            AutopilotActionClass::PolicySimulation {
                command_count: count,
            }
        }
        AutopilotActionRequest::OnchainBroadcast { request } => {
            AutopilotActionClass::OnchainBroadcast {
                dry_run: request.dry_run.unwrap_or(false),
            }
        }
    };

    let guard_decision =
        match evaluate_autopilot_guard(&state, action_class, req.confirmed_by_human).await {
            Ok((_, decision)) => decision,
            Err(error) => return api_error_response(error),
        };

    match guard_decision {
        AutopilotGuardDecision::Deny { reason } => (
            StatusCode::OK,
            Json(AutopilotExecuteResponse {
                allowed: false,
                reason: Some(reason),
                requires_confirmation: false,
                result: None,
            }),
        )
            .into_response(),
        AutopilotGuardDecision::Allow {
            requires_confirmation,
        } => {
            let result = async {
                let value = match req.action {
                    AutopilotActionRequest::PolicySimulation { commands } => {
                        let config = *state.policy_config.read().await;
                        let mut engine = DeterministicPolicyEngine::new(config);
                        let steps = engine.simulate(&commands);
                        serde_json::to_value(SimulationResponse { steps })?
                    }
                    AutopilotActionRequest::OnchainBroadcast { request } => {
                        let response = run_onchain_broadcast(request).await?;
                        serde_json::to_value(response)?
                    }
                };
                Ok::<_, HelixError>(value)
            }
            .await;

            match result {
                Ok(value) => (
                    StatusCode::OK,
                    Json(AutopilotExecuteResponse {
                        allowed: true,
                        reason: None,
                        requires_confirmation,
                        result: Some(value),
                    }),
                )
                    .into_response(),
                Err(err) => api_error_response(err),
            }
        }
        AutopilotGuardDecision::ConfigUpdated => api_error_response(HelixError::internal_error(
            "unexpected config decision during execute".to_string(),
        )),
    }
}

fn build_autopilot_propose_system_prompt(
    kind: AutopilotProposeKind,
    guard_config: AutopilotGuardConfig,
) -> String {
    let policy_schema = format!(
        concat!(
            "You are Helix Autopilot.\n",
            "Return ONLY a single JSON object. No prose. No markdown.\n",
            "\n",
            "Schema (policy simulation):\n",
            "{{\"type\":\"policy_simulation\",\"commands\":[<PolicyCommand>, ...]}}\n",
            "\n",
            "Constraints:\n",
            "- commands length must be between 1 and {max_policy_commands}\n",
            "- use only the command variants listed below\n",
            "\n",
            "PolicyCommand variants:\n",
            "- {{\"type\":\"tick\"}}\n",
            "- {{\"type\":\"request\",\"fingerprint\":<u64>,\"cost\":<u16>}}\n",
            "- {{\"type\":\"success\"}}\n",
            "- {{\"type\":\"failure\"}}\n",
            "- {{\"type\":\"retry\"}}\n",
            "- {{\"type\":\"reset_retry\"}}\n",
            "- {{\"type\":\"approve\"}}\n",
            "- {{\"type\":\"reject\"}}\n",
            "- {{\"type\":\"reset_approvals\"}}\n",
            "- {{\"type\":\"start_sla_window\"}}\n",
            "- {{\"type\":\"complete_sla_window\"}}\n",
            "- {{\"type\":\"reset_sla_window\"}}\n",
            "- {{\"type\":\"enqueue_backpressure\",\"count\":<u16>}}\n",
            "- {{\"type\":\"dequeue_backpressure\",\"count\":<u16>}}\n",
            "- {{\"type\":\"reset_dlq\"}}\n",
            "- {{\"type\":\"nonce_reserve\"}}\n",
            "- {{\"type\":\"nonce_confirm\",\"nonce\":<u64>}}\n",
            "- {{\"type\":\"nonce_reconcile\",\"chain_next_nonce\":<u64>}}\n",
            "- {{\"type\":\"fee_update_base_fee\",\"base_fee\":<u64>}}\n",
            "- {{\"type\":\"fee_quote\",\"urgent\":<bool>}}\n",
            "- {{\"type\":\"fee_rejected\"}}\n",
            "- {{\"type\":\"fee_confirmed\"}}\n",
            "- {{\"type\":\"finality_observe_depth\",\"depth\":<u16>}}\n",
            "- {{\"type\":\"finality_mark_reorg\"}}\n",
            "- {{\"type\":\"finality_reset\"}}\n",
            "- {{\"type\":\"allowlist_evaluate\",\"chain_id\":<u32>,\"contract_tag\":<u64>,\"method_tag\":<u32>}}\n",
            "- {{\"type\":\"allowlist_pause\"}}\n",
            "- {{\"type\":\"allowlist_resume\"}}\n"
        ),
        max_policy_commands = guard_config.max_policy_commands
    );

    let onchain_schema = concat!(
        "You are Helix Autopilot.\n",
        "Return ONLY a single JSON object. No prose. No markdown.\n",
        "\n",
        "Schema (onchain broadcast):\n",
        "{\"type\":\"onchain_broadcast\",\"request\":{",
        "\"rpc_url\":\"<url>\",",
        "\"raw_tx_hex\":\"0x<hex>\",",
        "\"await_receipt\":<bool>,",
        "\"dry_run\":<bool>,",
        "\"max_poll_rounds\":<u16>,",
        "\"poll_interval_ms\":<u64>",
        "}}\n",
        "\n",
        "Constraints:\n",
        "- rpc_url must be non-empty\n",
        "- raw_tx_hex must start with 0x and contain an even number of hex chars\n",
        "- default dry_run to true unless explicitly asked to broadcast\n",
        "- keep poll_interval_ms between 50 and 60000\n",
        "- keep max_poll_rounds >= 1\n"
    )
    .to_string();

    match kind {
        AutopilotProposeKind::PolicySimulation => policy_schema,
        AutopilotProposeKind::OnchainBroadcast => onchain_schema,
    }
}

fn parse_llm_action_proposal(content: &str) -> Result<ProposedAction, String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err("empty llm response".to_string());
    }

    let mut candidates: Vec<&str> = Vec::new();
    candidates.push(trimmed);

    // Try fenced ```json blocks.
    if let Some(block) = extract_fenced_block(trimmed, "json") {
        candidates.push(block);
    }
    // Try any fenced block.
    if let Some(block) = extract_any_fenced_block(trimmed) {
        candidates.push(block);
    }
    // Try the outermost {...} span.
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start < end {
            candidates.push(&trimmed[start..=end]);
        }
    }

    for candidate in candidates {
        if let Ok(action) = serde_json::from_str::<ProposedAction>(candidate) {
            return Ok(action);
        }
    }

    Err("response did not match expected JSON schema".to_string())
}

fn extract_fenced_block<'a>(content: &'a str, lang: &str) -> Option<&'a str> {
    // ```json\n...\n```
    let needle = format!("```{}", lang);
    let start = content.to_ascii_lowercase().find(&needle)?;
    let after = &content[start + needle.len()..];
    let after = after.strip_prefix('\n').unwrap_or(after);
    let end = after.find("```")?;
    Some(after[..end].trim())
}

fn extract_any_fenced_block<'a>(content: &'a str) -> Option<&'a str> {
    let start = content.find("```")?;
    let after = &content[start + 3..];
    let after = after
        .split_once('\n')
        .map(|(_, rest)| rest)
        .unwrap_or(after);
    let end = after.find("```")?;
    Some(after[..end].trim())
}

fn llm_provider_from_env() -> Option<(Arc<dyn LlmProvider>, String)> {
    let model = std::env::var("HELIX_AUTOPILOT_LLM_MODEL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "gpt-4o-mini".to_string());

    let configs = [
        (
            "OPENROUTER_API_KEY",
            "OPENROUTER_BASE_URL",
            "https://openrouter.ai/api/v1",
        ),
        (
            "OPENAI_API_KEY",
            "OPENAI_BASE_URL",
            "https://api.openai.com/v1",
        ),
        ("LLM_API_KEY", "LLM_BASE_URL", "https://api.openai.com/v1"),
    ];

    for (key_env, url_env, default_url) in configs {
        let Ok(api_key) = std::env::var(key_env) else {
            continue;
        };
        let base_url = std::env::var(url_env).unwrap_or_else(|_| default_url.to_string());
        let provider = OpenAiProvider::with_base_url(api_key, base_url);
        return Some((Arc::new(provider), model));
    }

    None
}

fn autopilot_config_from_env() -> AutopilotGuardConfig {
    AutopilotGuardConfig {
        mode: parse_autopilot_mode_env("HELIX_AUTOPILOT_MODE").unwrap_or(AutopilotMode::Assist),
        allow_onchain: parse_bool_env("HELIX_AUTOPILOT_ALLOW_ONCHAIN", false),
        require_onchain_confirmation: parse_bool_env(
            "HELIX_AUTOPILOT_REQUIRE_ONCHAIN_CONFIRMATION",
            true,
        ),
        require_onchain_dry_run: parse_bool_env("HELIX_AUTOPILOT_REQUIRE_DRY_RUN", true),
        max_policy_commands: parse_u16_env("HELIX_AUTOPILOT_MAX_POLICY_COMMANDS", 128),
    }
}

fn parse_autopilot_mode_env(key: &str) -> Option<AutopilotMode> {
    let value = std::env::var(key).ok()?.trim().to_ascii_lowercase();
    match value.as_str() {
        "off" => Some(AutopilotMode::Off),
        "assist" => Some(AutopilotMode::Assist),
        "auto" => Some(AutopilotMode::Auto),
        _ => None,
    }
}

fn parse_bool_env(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}

fn parse_u16_env(key: &str, default: u16) -> u16 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<u16>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

fn api_addr_from_env() -> SocketAddr {
    std::env::var(HELIX_API_ADDR_ENV)
        .ok()
        .and_then(|value| value.parse::<SocketAddr>().ok())
        .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 3000)))
}

fn api_auth_from_env() -> AuthService {
    let required = parse_bool_env(HELIX_AUTH_REQUIRED_ENV, false);
    let token = std::env::var(HELIX_API_TOKEN_ENV).ok();
    let token_ref = token.as_deref();
    let config = ApiTokenAuthConfig::from_optional_plaintext(required, token_ref)
        .unwrap_or_else(|err| panic!("invalid Helix API auth configuration: {err}"));
    AuthService::new(config)
}

async fn postgres_pool_from_env() -> Option<PgPool> {
    let database_url = std::env::var(DATABASE_URL_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    Some(
        PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .expect("failed to connect to DATABASE_URL for Postgres persistence"),
    )
}

async fn run_startup_migrations_if_enabled(pool: &PgPool) -> Result<(), HelixError> {
    if !parse_bool_env(HELIX_AUTO_MIGRATE_ENV, false) {
        return Ok(());
    }

    sqlx::raw_sql(HELIX_CORE_MIGRATION_SQL)
        .execute(pool)
        .await
        .map_err(|error| {
            HelixError::internal_error(format!("startup migration failed: {error}"))
        })?;
    Ok(())
}

fn ui_dist_dir_from_env() -> Option<PathBuf> {
    let dir = std::env::var_os(HELIX_UI_DIST_ENV).map(PathBuf::from)?;
    let index = dir.join("index.html");
    if index.is_file() {
        Some(dir)
    } else {
        tracing::warn!(
            env = HELIX_UI_DIST_ENV,
            path = %dir.display(),
            "static UI directory ignored because index.html is missing"
        );
        None
    }
}

fn app_with_optional_static_ui(state: AppState) -> Router {
    let app = app(state);
    let Some(ui_dist_dir) = ui_dist_dir_from_env() else {
        return app;
    };
    let index = ui_dist_dir.join("index.html");
    tracing::info!(path = %ui_dist_dir.display(), "serving static UI");
    app.fallback_service(ServeDir::new(ui_dist_dir).fallback(ServeFile::new(index)))
}

fn app(state: AppState) -> Router {
    let api_routes = api_router().route_layer(middleware::from_fn_with_state(
        state.clone(),
        require_api_auth,
    ));

    Router::new()
        .route("/health", get(health_check))
        .merge(api_routes)
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
}

fn api_router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/policy/config",
            get(get_policy_config).put(put_policy_config),
        )
        .route("/api/v1/policy/simulate", post(simulate_policy))
        .route("/api/v1/audit", get(get_audit_log))
        .route(
            "/api/v1/credentials",
            get(list_credentials).post(upsert_credential),
        )
        .route(
            "/api/v1/credentials/:profile_id/:credential_id",
            delete(delete_credential_handler),
        )
        .route("/api/v1/agents", get(get_agent_catalog))
        .route("/api/v1/agents/quality", get(get_agent_catalog_quality))
        .route(
            "/api/v1/agents/guards/simulate",
            post(post_simulate_guard_agent),
        )
        .route("/api/v1/agents/templates", get(get_agent_templates))
        .route(
            "/api/v1/agents/templates/:template_id",
            get(get_agent_template).post(post_apply_agent_template),
        )
        .route("/api/v1/recipes", get(list_recipes).post(upsert_recipe))
        .route("/api/v1/recipe-runs", get(list_recipe_runs))
        .route(
            "/api/v1/rules",
            get(list_automation_rules).post(upsert_automation_rule),
        )
        .route(
            "/api/v1/rules/evaluations",
            get(list_automation_rule_evaluations),
        )
        .route("/api/v1/rules/evaluate", post(evaluate_automation_rules))
        .route(
            "/api/v1/rules/trigger-plans/run",
            post(run_recipe_trigger_plan),
        )
        .route("/api/v1/intel/overview", get(get_intel_overview))
        .route(
            "/api/v1/market-intel/overview",
            get(get_market_intel_overview),
        )
        .route(
            "/api/v1/market-intel/cases/:case_id/brief",
            post(generate_market_intel_brief_handler),
        )
        .route(
            "/api/v1/market-intel/cases/:case_id/export",
            get(export_market_brief_packet_handler),
        )
        .route("/api/v1/sources", get(list_sources).post(create_source))
        .route(
            "/api/v1/sources/:source_id/collect",
            post(collect_source_handler),
        )
        .route(
            "/api/v1/sources/:source_id/webhook",
            post(webhook_ingest_handler),
        )
        .route(
            "/api/v1/sources/:source_id/import",
            post(file_import_handler),
        )
        .route(
            "/api/v1/watchlists",
            get(list_watchlists).post(create_watchlist),
        )
        .route("/api/v1/evidence", get(list_evidence))
        .route("/api/v1/evidence/ingest", post(ingest_evidence))
        .route("/api/v1/claims", get(list_claims))
        .route(
            "/api/v1/claims/:claim_id/review",
            post(review_claim_handler),
        )
        .route("/api/v1/cases", get(list_cases))
        .route(
            "/api/v1/cases/:case_id/transition",
            post(transition_case_handler),
        )
        .route("/api/v1/reasoning/evaluate", post(post_reasoning_evaluate))
        .route("/api/v1/autopilot/status", get(get_autopilot_status))
        .route(
            "/api/v1/autopilot/config",
            get(get_autopilot_status).put(put_autopilot_config),
        )
        .route(
            "/api/v1/autopilot/review-queue",
            get(get_autopilot_review_queue),
        )
        .route(
            "/api/v1/autopilot/review-queue/export",
            get(export_autopilot_review_packet),
        )
        .route(
            "/api/v1/autopilot/review-queue/propose",
            post(post_autopilot_review_propose),
        )
        .route("/api/v1/autopilot/propose", post(post_autopilot_propose))
        .route("/api/v1/autopilot/execute", post(post_autopilot_execute))
        .route("/api/v1/onchain/send_raw", post(onchain_send_raw))
        .route("/api/v1/onchain/receipt", post(onchain_get_receipt))
}

async fn require_api_auth(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if req.method() == Method::OPTIONS {
        return next.run(req).await;
    }

    let authorization = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok());

    match state.auth_service.evaluate_bearer_header(authorization) {
        AuthDecision::Allow { .. } => next.run(req).await,
        AuthDecision::Deny { reason } => (
            StatusCode::UNAUTHORIZED,
            Json(ApiErrorResponse {
                error: reason.as_str().to_string(),
            }),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intel::{
        AutopilotReviewExportPacketResponse, AutopilotReviewQueueResponse, CaseCatalogResponse,
        CaseTransitionRequest, CaseTransitionResponse, ClaimCatalogResponse, ClaimResponse,
        ClaimReviewRequest, CollectSourceResponse, CreateSourceRequest, CreateWatchlistRequest,
        FileImportResponse, GenerateMarketIntelBriefRequest, GenerateMarketIntelBriefResponse,
        IngestEvidenceRequest, IngestEvidenceResponse, IntelDeskOverviewResponse,
        MarketIntelBriefExportPacketResponse, MarketIntelOverviewResponse, SourceCatalogResponse,
        SourceResponse, WatchlistResponse, WebhookIngestResponse,
    };
    use async_trait::async_trait;
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use helix_core::deterministic_agents_expanded::TemporalGuardSimulation;
    use helix_core::deterministic_policy::PolicyDecision;
    use helix_llm::errors::LlmError;
    use std::collections::HashMap;
    use tower::ServiceExt;

    fn default_app_state(
        llm_provider: Option<Arc<dyn LlmProvider>>,
        llm_model: Option<String>,
    ) -> AppState {
        AppState {
            policy_config: Arc::new(RwLock::new(DeterministicPolicyConfig::default())),
            autopilot_guard: Arc::new(RwLock::new(AutopilotGuardMachine::default())),
            intel_desk: Arc::new(RwLock::new(IntelDeskStore::default())),
            intel_persistence: None,
            state_persistence: None,
            automation_rules: Arc::new(RwLock::new(Vec::new())),
            recipes: Arc::new(RwLock::new(Vec::new())),
            symbolic_program_cache: Arc::new(RwLock::new(SymbolicProgramCache::new(
                SYMBOLIC_PROGRAM_CACHE_CAPACITY,
            ))),
            llm_provider,
            llm_model,
            auth_service: Arc::new(AuthService::disabled()),
        }
    }

    fn test_app() -> Router {
        app(default_app_state(None, None))
    }

    fn test_app_with_required_auth() -> Router {
        let mut state = default_app_state(None, None);
        state.auth_service = Arc::new(AuthService::new(
            ApiTokenAuthConfig::required_from_plaintext("test-token-12345").unwrap(),
        ));
        app(state)
    }

    fn test_app_with_llm(provider: Arc<dyn LlmProvider>, model: String) -> Router {
        app(default_app_state(Some(provider), Some(model)))
    }

    async fn spawn_text_server(path: &'static str, body: &'static str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new().route(path, get(move || async move { body }));
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}{path}")
    }

    async fn create_test_source(app: Router, name: &str) -> SourceResponse {
        let request = CreateSourceRequest {
            profile_id: None,
            name: name.to_string(),
            description: "Source used by API endpoint tests.".to_string(),
            kind: helix_core::intel_desk::SourceKind::JsonApi,
            endpoint_url: None,
            credential_id: None,
            credential_header_name: None,
            credential_header_prefix: None,
            cadence_minutes: 30,
            trust_score: 91,
            enabled: true,
            tags: vec!["test".to_string()],
        };

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sources")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    async fn ingest_test_evidence(app: Router, request: IngestEvidenceRequest) {
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/evidence/ingest")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[derive(Clone)]
    struct StubLlmProvider {
        content: String,
        model: String,
    }

    #[async_trait]
    impl LlmProvider for StubLlmProvider {
        fn name(&self) -> &str {
            "stub"
        }

        async fn get_models(&self) -> Result<Vec<helix_llm::providers::ModelConfig>, LlmError> {
            Ok(Vec::new())
        }

        async fn complete(
            &self,
            _request: LlmRequest,
        ) -> Result<helix_llm::providers::LlmResponse, LlmError> {
            Ok(helix_llm::providers::LlmResponse {
                content: self.content.clone(),
                function_call: None,
                usage: helix_llm::providers::TokenUsage {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                },
                model: self.model.clone(),
                finish_reason: helix_llm::providers::FinishReason::Stop,
                metadata: HashMap::new(),
            })
        }

        async fn stream_complete(
            &self,
            _request: LlmRequest,
        ) -> Result<
            Box<dyn futures::Stream<Item = Result<String, LlmError>> + Unpin + Send>,
            LlmError,
        > {
            Err(LlmError::ModelNotSupported("streaming".into()))
        }

        async fn health_check(&self) -> Result<(), LlmError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn health_endpoint_works() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_auth_allows_health_but_protects_api_routes_when_required() {
        let app = test_app_with_required_auth();

        let health_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health_response.status(), StatusCode::OK);

        let missing_auth_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/policy/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing_auth_response.status(), StatusCode::UNAUTHORIZED);

        let invalid_auth_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/policy/config")
                    .header("authorization", "Bearer wrong-token-12345")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(invalid_auth_response.status(), StatusCode::UNAUTHORIZED);

        let valid_auth_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/policy/config")
                    .header("authorization", "Bearer test-token-12345")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(valid_auth_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn audit_endpoint_reports_disabled_without_postgres() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/audit")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: AuditLogResponse = serde_json::from_slice(&body).unwrap();
        assert!(!payload.persistence_enabled);
        assert!(payload.entries.is_empty());
    }

    #[tokio::test]
    async fn audit_endpoint_rejects_zero_limit() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/audit?limit=0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn credentials_endpoint_reports_disabled_without_postgres() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/credentials?profile_id=50000000-0000-0000-0000-000000000010")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: CredentialCatalogResponse = serde_json::from_slice(&body).unwrap();
        assert!(!payload.persistence_enabled);
        assert!(payload.credentials.is_empty());
    }

    #[tokio::test]
    async fn credentials_endpoint_rejects_invalid_profile_query() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/credentials?profile_id=not-a-uuid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn credential_upsert_fails_closed_without_postgres() {
        let request = CredentialUpsertRequest {
            id: None,
            profile_id: "50000000-0000-0000-0000-000000000010".to_string(),
            name: "GitHub Token".to_string(),
            kind: "api_key".to_string(),
            secret: "test-secret".to_string(),
            metadata: BTreeMap::from([("provider".to_string(), "github".to_string())]),
        };

        let response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/credentials")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: ApiErrorResponse = serde_json::from_slice(&body).unwrap();
        assert!(payload.error.contains("DATABASE_URL"));
    }

    #[test]
    fn credential_upsert_validation_rejects_boundaries() {
        let valid_profile_id = "50000000-0000-0000-0000-000000000010".to_string();
        let mut oversized_metadata = BTreeMap::new();
        for index in 0..=MAX_CREDENTIAL_METADATA_ENTRIES {
            oversized_metadata.insert(format!("key-{index}"), "value".to_string());
        }

        let empty_secret = validate_credential_upsert(CredentialUpsertRequest {
            id: None,
            profile_id: valid_profile_id.clone(),
            name: "GitHub Token".to_string(),
            kind: "api_key".to_string(),
            secret: String::new(),
            metadata: BTreeMap::new(),
        });
        assert!(matches!(
            empty_secret,
            Err(HelixError::ValidationError { .. })
        ));

        let oversized_secret = validate_credential_upsert(CredentialUpsertRequest {
            id: None,
            profile_id: valid_profile_id.clone(),
            name: "GitHub Token".to_string(),
            kind: "api_key".to_string(),
            secret: "x".repeat(MAX_CREDENTIAL_SECRET_LEN + 1),
            metadata: BTreeMap::new(),
        });
        assert!(matches!(
            oversized_secret,
            Err(HelixError::ValidationError { .. })
        ));

        let oversized_metadata = validate_credential_upsert(CredentialUpsertRequest {
            id: None,
            profile_id: valid_profile_id,
            name: "GitHub Token".to_string(),
            kind: "api_key".to_string(),
            secret: "test-secret".to_string(),
            metadata: oversized_metadata,
        });
        assert!(matches!(
            oversized_metadata,
            Err(HelixError::ValidationError { .. })
        ));
    }

    fn critical_case_rule(recipe_id: &str) -> Rule {
        serde_json::from_value(serde_json::json!({
            "id": "50000000-0000-0000-0000-000000000001",
            "name": "Critical case automation",
            "condition": {
                "field": "event.data.severity",
                "operator": "equals",
                "value": "critical"
            },
            "actions": [
                {
                    "type": "trigger_recipe",
                    "recipe_id": recipe_id,
                    "parameters": {
                        "case_id": {
                            "from_event": "event.data.case_id"
                        },
                        "mode": {
                            "literal": "prepare_brief"
                        }
                    }
                }
            ]
        }))
        .unwrap()
    }

    fn critical_case_recipe(recipe_id: &str) -> Recipe {
        let profile_id: helix_core::types::ProfileId =
            "50000000-0000-0000-0000-000000000010".parse().unwrap();
        let first_agent_id: AgentId = "50000000-0000-0000-0000-000000000011".parse().unwrap();
        let second_agent_id: AgentId = "50000000-0000-0000-0000-000000000012".parse().unwrap();
        let mut second_agent = AgentConfig::new(
            second_agent_id,
            profile_id,
            Some("Prepare brief".to_string()),
            "emit_event".to_string(),
            serde_json::json!({
                "event_type": "helix.recipe.case_brief.prepared",
                "payload": {
                    "case_id": "case_critical",
                    "mode": "prepare_brief"
                }
            }),
        );
        second_agent.dependencies = vec![first_agent_id];

        Recipe::new(
            recipe_id.parse().unwrap(),
            profile_id,
            "Critical Case Response".to_string(),
            Some("Deterministic demo recipe for case response automation.".to_string()),
            helix_core::recipe::RecipeGraphDefinition {
                agents: vec![
                    second_agent,
                    AgentConfig::new(
                        first_agent_id,
                        profile_id,
                        Some("Normalize case".to_string()),
                        "record_state".to_string(),
                        serde_json::json!({
                            "state": {
                                "case_id": "case_critical",
                                "normalized": true
                            }
                        }),
                    ),
                ],
            },
        )
    }

    #[tokio::test]
    async fn recipes_endpoint_creates_and_lists_recipe() {
        let app = test_app();
        let recipe = critical_case_recipe("50000000-0000-0000-0000-000000000020");

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/recipes")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&RecipeUpsertRequest { recipe }).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::OK);
        let create_body = to_bytes(create_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let created: RecipeResponse = serde_json::from_slice(&create_body).unwrap();
        assert!(!created.persistence_enabled);
        assert_eq!(created.recipe.name, "Critical Case Response");

        let list_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/recipes")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_response.status(), StatusCode::OK);
        let list_body = to_bytes(list_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let listed: RecipeCatalogResponse = serde_json::from_slice(&list_body).unwrap();
        assert_eq!(listed.recipes.len(), 1);
        assert_eq!(listed.recipes[0].agent_count(), 2);
    }

    #[tokio::test]
    async fn run_recipe_via_api_runtime_runs_builtin_agents_in_dependency_order() {
        let recipe = critical_case_recipe("50000000-0000-0000-0000-000000000021");

        let output = run_recipe_via_api_runtime(&recipe).await.unwrap();

        assert_eq!(
            output.started_agent_ids,
            vec![
                "50000000-0000-0000-0000-000000000011"
                    .parse::<AgentId>()
                    .unwrap(),
                "50000000-0000-0000-0000-000000000012"
                    .parse::<AgentId>()
                    .unwrap(),
            ]
        );
        assert_eq!(output.emitted_events.len(), 1);
        assert_eq!(
            output.emitted_events[0]["type"],
            "helix.recipe.case_brief.prepared"
        );
        assert_eq!(
            output.state_snapshots["50000000-0000-0000-0000-000000000011"]["record"]["normalized"],
            true
        );
    }

    #[tokio::test]
    async fn run_recipe_via_api_runtime_rejects_unregistered_agent_kind() {
        let mut recipe = critical_case_recipe("50000000-0000-0000-0000-000000000022");
        recipe.graph.agents[0].agent_kind = "unknown_kind".to_string();

        let result = run_recipe_via_api_runtime(&recipe).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("no registered runtime factory"));
    }

    #[tokio::test]
    async fn rules_endpoint_creates_lists_and_evaluates_rule() {
        let app = test_app();
        let recipe_id = "50000000-0000-0000-0000-000000000002";
        let rule = critical_case_rule(recipe_id);

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/rules")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&AutomationRuleUpsertRequest { rule }).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::OK);
        let create_body = to_bytes(create_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let created: AutomationRuleResponse = serde_json::from_slice(&create_body).unwrap();
        assert!(!created.persistence_enabled);
        assert_eq!(created.rule.name, "Critical case automation");

        let list_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/rules")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_response.status(), StatusCode::OK);
        let list_body = to_bytes(list_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let listed: AutomationRuleCatalogResponse = serde_json::from_slice(&list_body).unwrap();
        assert_eq!(listed.rules.len(), 1);

        let event = Event::new(
            "intel".to_string(),
            "intel.case.opened".to_string(),
            Some(serde_json::json!({
                "case_id": "case_500",
                "severity": "critical"
            })),
        );
        let evaluate_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/rules/evaluate")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&AutomationRuleEvaluateRequest { event }).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(evaluate_response.status(), StatusCode::OK);
        let evaluate_body = to_bytes(evaluate_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let evaluated: AutomationRuleEvaluateResponse =
            serde_json::from_slice(&evaluate_body).unwrap();
        assert_eq!(evaluated.rule_count, 1);
        assert_eq!(evaluated.trigger_plans.len(), 1);
        assert_eq!(
            evaluated.trigger_plans[0].recipe_id.unwrap().to_string(),
            recipe_id
        );
        assert_eq!(
            evaluated.trigger_plans[0].parameters.get("case_id"),
            Some(&serde_json::json!("case_500"))
        );
    }

    #[tokio::test]
    async fn rules_endpoint_rejects_action_without_recipe_target() {
        let app = test_app();
        let mut rule = critical_case_rule("50000000-0000-0000-0000-000000000002");
        rule.actions[0].recipe_id = None;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/rules")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&AutomationRuleUpsertRequest { rule }).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn rule_evaluations_endpoint_reports_disabled_without_postgres() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/rules/evaluations")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: AutomationRuleEvaluationCatalogResponse =
            serde_json::from_slice(&body).unwrap();
        assert!(!payload.persistence_enabled);
        assert!(payload.entries.is_empty());
    }

    #[tokio::test]
    async fn rule_evaluations_endpoint_rejects_zero_limit() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/rules/evaluations?limit=0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn recipe_runs_endpoint_reports_disabled_without_postgres() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/recipe-runs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: RecipeRunCatalogResponse = serde_json::from_slice(&body).unwrap();
        assert!(!payload.persistence_enabled);
        assert!(payload.entries.is_empty());
    }

    #[tokio::test]
    async fn recipe_runs_endpoint_rejects_zero_limit() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/recipe-runs?limit=0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn policy_config_roundtrip_works() {
        let app = test_app();

        let update = PolicyConfigResponse {
            config: DeterministicPolicyConfig {
                dedup_window_ticks: 3,
                rate_max_tokens: 6,
                rate_refill_per_tick: 2,
                breaker_failure_threshold: 2,
                breaker_open_duration_ticks: 2,
                retry_budget: 3,
                approval_quorum: 2,
                approval_reviewers: 4,
                backpressure_soft_limit: 3,
                backpressure_hard_limit: 5,
                sla_deadline_ticks: 2,
                dlq_max_consecutive_failures: 2,
                nonce_start: 10,
                nonce_max_in_flight: 32,
                fee_base_fee: 100,
                fee_priority_fee: 2,
                fee_bump_bps: 500,
                fee_max_fee_cap: 10000,
                finality_required_depth: 2,
                allowlist_chain_id: 1,
                allowlist_contract_tag: 55,
                allowlist_method_tag: 0xdeadbeef,
            },
        };

        let put_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/v1/policy/config")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&update).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(put_response.status(), StatusCode::OK);

        let get_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/policy/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(get_response.status(), StatusCode::OK);

        let body = to_bytes(get_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let response: PolicyConfigResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(response.config.rate_max_tokens, 6);
        assert_eq!(response.config.approval_reviewers, 4);
        assert_eq!(response.config.sla_deadline_ticks, 2);
    }

    #[tokio::test]
    async fn policy_simulator_returns_steps() {
        let app = test_app();

        let req = SimulationRequest {
            commands: vec![
                PolicyCommand::Request {
                    fingerprint: 1,
                    cost: 1,
                },
                PolicyCommand::Request {
                    fingerprint: 1,
                    cost: 1,
                },
            ],
        };

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/policy/simulate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&req).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let response: SimulationResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(response.steps.len(), 2);
        assert!(matches!(
            response.steps[0].decision,
            PolicyDecision::RequestAccepted
        ));
    }

    #[tokio::test]
    async fn agents_catalog_endpoint_returns_items() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/agents")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let response: AgentCatalogResponse = serde_json::from_slice(&body).unwrap();
        assert!(response.agents.len() > 68);
    }

    #[tokio::test]
    async fn agents_quality_endpoint_reports_baseline_win() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/agents/quality")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: AgentCatalogQualityResponse = serde_json::from_slice(&body).unwrap();
        assert!(payload.quality.exceeds_huginn);
        assert!(payload.quality.expanded_categories >= 6);
    }

    #[tokio::test]
    async fn agents_guard_simulation_endpoint_returns_temporal_trace() {
        let request = GuardSimulationRequest {
            agent_id: "oracle_deviation_guard".to_string(),
            threshold: Some(3),
            strike_limit: Some(2),
            cooldown_ticks: Some(2),
            commands: vec![
                GuardSimulationCommand::Evaluate { value: 4 },
                GuardSimulationCommand::Evaluate { value: 5 },
                GuardSimulationCommand::Tick,
                GuardSimulationCommand::Tick,
                GuardSimulationCommand::Evaluate { value: 1 },
            ],
        };

        let response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/agents/guards/simulate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: TemporalGuardSimulation = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.steps.len(), 5);
        assert_eq!(
            payload.steps[0].decision,
            helix_core::deterministic_agents_expanded::TemporalGuardDecision::Warn
        );
        assert_eq!(
            payload.steps[1].decision,
            helix_core::deterministic_agents_expanded::TemporalGuardDecision::Block
        );
    }

    #[tokio::test]
    async fn reasoning_endpoint_supports_neuro_symbolic_backend() {
        let request = ReasoningEvaluationRequest::NeuroSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec!["trusted(user)".to_string()],
            rules: vec![],
            triples: vec![],
            features: std::collections::BTreeMap::from([("risk".to_string(), 10.0)]),
            model: helix_core::reasoning::LinearModel {
                bias: 0.0,
                weights: std::collections::BTreeMap::from([("risk".to_string(), 1.0)]),
                allow_threshold: 0.8,
                review_threshold: 0.5,
            },
            min_probability: Some(0.8),
            consistency_scope: None,
            max_rounds: Some(8),
        };

        let response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/reasoning/evaluate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: ReasoningEvaluateResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            payload.decision.backend,
            helix_core::reasoning::ReasoningBackend::NeuroSymbolic
        );
        assert_eq!(
            payload.decision.verdict,
            helix_core::reasoning::ReasoningVerdict::Deny
        );
    }

    #[tokio::test]
    async fn symbolic_reasoning_endpoint_returns_support_graph_and_reuses_cache() {
        let state = default_app_state(None, None);
        let router = app(state.clone());
        let request = ReasoningEvaluationRequest::KrrSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec!["trusted(user)".to_string(), "kyc_passed(user)".to_string()],
            rules: vec![SymbolicRule {
                id: "r1".to_string(),
                antecedents: vec!["trusted(user)".to_string(), "kyc_passed(user)".to_string()],
                consequent: "allow(tx)".to_string(),
            }],
            triples: vec![],
            consistency_scope: None,
            max_rounds: Some(8),
        };

        for _ in 0..2 {
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/v1/reasoning/evaluate")
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&request).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
            let payload: ReasoningEvaluateResponse = serde_json::from_slice(&body).unwrap();
            assert_eq!(payload.decision.trace.support_graph.len(), 3);
            assert_eq!(
                payload.decision.trace.symbolic_status,
                Some(helix_core::reasoning::ReasoningSymbolicStatus::Saturated)
            );
            assert_eq!(payload.decision.trace.pending_rule_count, Some(0));
            assert!(payload.decision.trace.program_fingerprint.is_some());
            assert_eq!(
                payload.decision.trace.consistency_scope,
                Some(helix_core::reasoning::ReasoningConsistencyScope::Global)
            );
            assert!(payload.decision.trace.blocking_contradictions.is_empty());
            assert_eq!(
                payload.decision.trace.query_support,
                vec![
                    "allow(tx)".to_string(),
                    "kyc_passed(user)".to_string(),
                    "trusted(user)".to_string(),
                ]
            );
            assert!(payload
                .decision
                .trace
                .support_graph
                .iter()
                .any(|node| node.fact == "allow(tx)" && node.rule_id.as_deref() == Some("r1")));
        }

        let cache = state.symbolic_program_cache.read().await;
        assert_eq!(cache.len(), 1);
    }

    #[tokio::test]
    async fn symbolic_reasoning_endpoint_denies_contradictions() {
        let request = ReasoningEvaluationRequest::KrrSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec!["allow(tx)".to_string(), "!allow(tx)".to_string()],
            rules: vec![],
            triples: vec![],
            consistency_scope: None,
            max_rounds: Some(4),
        };

        let response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/reasoning/evaluate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: ReasoningEvaluateResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            payload.decision.verdict,
            helix_core::reasoning::ReasoningVerdict::Deny
        );
        assert_eq!(payload.decision.trace.contradictions.len(), 1);
    }

    #[tokio::test]
    async fn symbolic_reasoning_endpoint_supports_query_support_scope() {
        let request = ReasoningEvaluationRequest::KrrSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec![
                "trusted(user)".to_string(),
                "noise".to_string(),
                "!noise".to_string(),
            ],
            rules: vec![SymbolicRule {
                id: "permit".to_string(),
                antecedents: vec!["trusted(user)".to_string()],
                consequent: "allow(tx)".to_string(),
            }],
            triples: vec![],
            consistency_scope: Some(helix_core::reasoning::ReasoningConsistencyScope::QuerySupport),
            max_rounds: Some(4),
        };

        let response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/reasoning/evaluate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: ReasoningEvaluateResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            payload.decision.verdict,
            helix_core::reasoning::ReasoningVerdict::Allow
        );
        assert_eq!(payload.decision.trace.contradictions.len(), 1);
        assert!(payload.decision.trace.blocking_contradictions.is_empty());
        assert_eq!(
            payload.decision.trace.consistency_scope,
            Some(helix_core::reasoning::ReasoningConsistencyScope::QuerySupport)
        );
    }

    #[tokio::test]
    async fn symbolic_reasoning_endpoint_reports_truncation_metadata() {
        let request = ReasoningEvaluationRequest::KrrSymbolic {
            query: "c".to_string(),
            facts: vec!["a".to_string()],
            rules: vec![
                SymbolicRule {
                    id: "r1".to_string(),
                    antecedents: vec!["a".to_string()],
                    consequent: "b".to_string(),
                },
                SymbolicRule {
                    id: "r2".to_string(),
                    antecedents: vec!["b".to_string()],
                    consequent: "c".to_string(),
                },
            ],
            triples: vec![],
            consistency_scope: None,
            max_rounds: Some(1),
        };

        let response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/reasoning/evaluate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: ReasoningEvaluateResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            payload.decision.verdict,
            helix_core::reasoning::ReasoningVerdict::Deny
        );
        assert_eq!(
            payload.decision.trace.symbolic_status,
            Some(helix_core::reasoning::ReasoningSymbolicStatus::Truncated)
        );
        assert_eq!(payload.decision.trace.pending_rule_count, Some(1));
        assert!(payload.decision.rationale.contains("truncated"));
    }

    #[tokio::test]
    async fn agent_templates_endpoint_returns_items() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/agents/templates")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let response: AgentTemplateCatalogResponse = serde_json::from_slice(&body).unwrap();
        assert!(response.templates.len() >= 4);
    }

    #[tokio::test]
    async fn agent_template_detail_endpoint_returns_known_template() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/agents/templates/secure_onchain_executor")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let response: AgentTemplateResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(response.template.id, "secure_onchain_executor");
        assert!(!response.template.bootstrap_commands.is_empty());
    }

    #[tokio::test]
    async fn apply_agent_template_updates_policy_config() {
        let app = test_app();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/agents/templates/latency_slo_protection")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&ApplyAgentTemplateRequest {
                            run_bootstrap_simulation: Some(true),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: ApplyAgentTemplateResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.template.id, "latency_slo_protection");
        assert_eq!(payload.config.sla_deadline_ticks, 2);
        assert!(payload.bootstrap_steps.as_ref().is_some());
        assert!(!payload.bootstrap_steps.unwrap().is_empty());

        let config_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/policy/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(config_response.status(), StatusCode::OK);

        let body = to_bytes(config_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let response: PolicyConfigResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(response.config.sla_deadline_ticks, 2);
        assert_eq!(response.config.backpressure_soft_limit, 3);
    }

    #[tokio::test]
    async fn unknown_agent_template_returns_not_found() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/agents/templates/does_not_exist")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn intel_overview_reports_seeded_assets() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/intel/overview")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: IntelDeskOverviewResponse = serde_json::from_slice(&body).unwrap();
        assert!(payload.source_count >= 2);
        assert!(payload.watchlist_count >= 2);
    }

    #[tokio::test]
    async fn market_intel_overview_reports_seeded_market_coverage() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/market-intel/overview")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: MarketIntelOverviewResponse = serde_json::from_slice(&body).unwrap();
        assert!(payload.market_source_count >= 4);
        assert!(payload.market_watchlist_count >= 4);
        assert!(payload.tracked_company_count >= 4);
        assert!(!payload.case_briefs.is_empty());
        assert_eq!(payload.playbooks.len(), 4);
        assert!(payload
            .theme_cards
            .windows(2)
            .all(|window| window[0].priority.total >= window[1].priority.total));
        assert!(payload
            .company_cards
            .windows(2)
            .all(|window| window[0].priority.total >= window[1].priority.total));
        assert!(payload
            .case_briefs
            .windows(2)
            .all(|window| window[0].priority.total >= window[1].priority.total));
        assert!(payload
            .case_briefs
            .iter()
            .all(|brief| brief.priority.total > 0));
    }

    #[tokio::test]
    async fn cases_endpoint_returns_priority_ranked_queue() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/cases")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: CaseCatalogResponse = serde_json::from_slice(&body).unwrap();
        assert!(!payload.cases.is_empty());
        assert!(payload
            .cases
            .windows(2)
            .all(|window| window[0].priority.total >= window[1].priority.total));
        assert!(payload
            .cases
            .iter()
            .all(|entry| !entry.watchlist_name.is_empty()));
        assert!(payload.cases.iter().all(|entry| entry.priority.total > 0));
    }

    #[tokio::test]
    async fn cases_endpoint_breaks_equal_priorities_by_latest_signal() {
        let app = test_app();

        for (name, keyword, entity) in [
            ("Signal Tie Alpha", "tiealpha", "entity-alpha"),
            ("Signal Tie Beta", "tiebeta", "entity-beta"),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/v1/watchlists")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            serde_json::to_vec(&CreateWatchlistRequest {
                                name: name.to_string(),
                                description: "tie break regression".to_string(),
                                keywords: vec![keyword.to_string()],
                                entities: vec![entity.to_string()],
                                min_source_trust: 40,
                                severity: helix_core::intel_desk::WatchlistSeverity::Medium,
                                enabled: true,
                            })
                            .unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::CREATED);
        }

        let older = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/evidence/ingest")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&IngestEvidenceRequest {
                            source_id: "rss_national_security".to_string(),
                            title: "Tie alpha signal".to_string(),
                            summary: "older tie-break signal".to_string(),
                            content: "tiealpha observed for entity-alpha".to_string(),
                            url: None,
                            observed_at: "2026-03-10T10:30:00Z".to_string(),
                            tags: vec!["tie".to_string()],
                            entity_labels: vec!["entity-alpha".to_string()],
                            proposed_claims: Vec::new(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(older.status(), StatusCode::CREATED);
        let body = to_bytes(older.into_body(), 1024 * 1024).await.unwrap();
        let older_payload: IngestEvidenceResponse = serde_json::from_slice(&body).unwrap();
        let older_case_id = older_payload.case_updates[0].case.id.clone();

        let newer = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/evidence/ingest")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&IngestEvidenceRequest {
                            source_id: "rss_national_security".to_string(),
                            title: "Tie beta signal".to_string(),
                            summary: "newer tie-break signal".to_string(),
                            content: "tiebeta observed for entity-beta".to_string(),
                            url: None,
                            observed_at: "2026-03-10T11:30:00Z".to_string(),
                            tags: vec!["tie".to_string()],
                            entity_labels: vec!["entity-beta".to_string()],
                            proposed_claims: Vec::new(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(newer.status(), StatusCode::CREATED);
        let body = to_bytes(newer.into_body(), 1024 * 1024).await.unwrap();
        let newer_payload: IngestEvidenceResponse = serde_json::from_slice(&body).unwrap();
        let newer_case_id = newer_payload.case_updates[0].case.id.clone();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/cases")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: CaseCatalogResponse = serde_json::from_slice(&body).unwrap();
        let older_index = payload
            .cases
            .iter()
            .position(|entry| entry.case.id == older_case_id)
            .unwrap();
        let newer_index = payload
            .cases
            .iter()
            .position(|entry| entry.case.id == newer_case_id)
            .unwrap();
        let older_entry = &payload.cases[older_index];
        let newer_entry = &payload.cases[newer_index];

        assert_eq!(older_entry.priority.total, newer_entry.priority.total);
        assert!(newer_entry.latest_signal_at > older_entry.latest_signal_at);
        assert!(newer_index < older_index);
    }

    #[tokio::test]
    async fn cases_endpoint_filters_by_status_and_limit() {
        let app = test_app();
        let ingest = IngestEvidenceRequest {
            source_id: "rss_national_security".to_string(),
            title: "Escalated filter regression".to_string(),
            summary: "status filter coverage".to_string(),
            content: "Alice North resigned after a detention report.".to_string(),
            url: None,
            observed_at: "2026-03-10T12:15:00Z".to_string(),
            tags: vec!["security".to_string()],
            entity_labels: vec!["alice north".to_string(), "orion dynamics".to_string()],
            proposed_claims: vec![helix_core::intel_desk::ProposedClaim {
                subject: "alice north".to_string(),
                predicate: "resigned_from".to_string(),
                object: "orion dynamics".to_string(),
                confidence_bps: 9_100,
                rationale: Some("explicitly stated".to_string()),
            }],
        };

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/evidence/ingest")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&ingest).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/cases?status=escalated&limit=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: CaseCatalogResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.cases.len(), 1);
        assert_eq!(
            payload.cases[0].case.status,
            helix_core::intel_desk::CaseStatus::Escalated
        );
    }

    #[tokio::test]
    async fn cases_endpoint_rejects_zero_limit() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/cases?limit=0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn evidence_endpoint_filters_by_min_trust_and_limit() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/evidence?min_trust=80&limit=2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: crate::intel::EvidenceCatalogResponse = serde_json::from_slice(&body).unwrap();
        assert!(payload.evidence.len() <= 2);
        assert!(payload
            .evidence
            .windows(2)
            .all(|window| window[0].priority.total >= window[1].priority.total));
        assert!(payload
            .evidence
            .iter()
            .all(|entry| entry.source_trust_score >= 80));
    }

    #[tokio::test]
    async fn evidence_endpoint_rejects_invalid_min_trust() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/evidence?min_trust=101")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn evidence_endpoint_semantic_query_ranks_relevant_evidence() {
        let app = test_app();
        let source = create_test_source(app.clone(), "Semantic Retrieval Evidence Source").await;
        let source_id = source.source.id;

        ingest_test_evidence(
            app.clone(),
            IngestEvidenceRequest {
                source_id: source_id.clone(),
                title: "Orbital telemetry launch expansion".to_string(),
                summary: "Satellite operations expansion signal.".to_string(),
                content: "Orion telemetry team prepared satellite orbit launch operations."
                    .to_string(),
                url: None,
                observed_at: "2026-03-10T11:00:00Z".to_string(),
                tags: vec!["space".to_string()],
                entity_labels: vec!["orion telemetry".to_string()],
                proposed_claims: Vec::new(),
            },
        )
        .await;
        ingest_test_evidence(
            app.clone(),
            IngestEvidenceRequest {
                source_id: source_id.clone(),
                title: "Enterprise pricing renewal change".to_string(),
                summary: "Commercial packaging update.".to_string(),
                content: "Renewal discounting and annual pricing package language changed."
                    .to_string(),
                url: None,
                observed_at: "2026-03-10T12:00:00Z".to_string(),
                tags: vec!["pricing".to_string()],
                entity_labels: vec!["renewal desk".to_string()],
                proposed_claims: Vec::new(),
            },
        )
        .await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/v1/evidence?source_id={source_id}&q=satellite%20orbit%20telemetry&limit=1"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: crate::intel::EvidenceCatalogResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.evidence.len(), 1);
        assert_eq!(
            payload.evidence[0].evidence.title,
            "Orbital telemetry launch expansion"
        );
        assert!(payload.evidence[0].semantic_score_bps.is_some());
    }

    #[tokio::test]
    async fn evidence_endpoint_rejects_invalid_semantic_query() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/evidence?q=!!!")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let long_query = "a".repeat(513);
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/evidence?q={long_query}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn claims_endpoint_filters_by_review_status_and_limit() {
        let app = test_app();
        let ingest = IngestEvidenceRequest {
            source_id: "rss_national_security".to_string(),
            title: "Claim filter review path".to_string(),
            summary: "Claim review coverage".to_string(),
            content: "Alice North was appointed to a new role at Orion Dynamics.".to_string(),
            url: None,
            observed_at: "2026-03-10T13:00:00Z".to_string(),
            tags: vec!["leadership".to_string()],
            entity_labels: vec!["alice north".to_string(), "orion dynamics".to_string()],
            proposed_claims: vec![helix_core::intel_desk::ProposedClaim {
                subject: "alice north".to_string(),
                predicate: "appointed_to".to_string(),
                object: "orion dynamics".to_string(),
                confidence_bps: 9300,
                rationale: Some("explicitly stated".to_string()),
            }],
        };

        let ingest_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/evidence/ingest")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&ingest).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ingest_response.status(), StatusCode::CREATED);

        let claims_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/claims?review_status=needs_review&predicate=appointed_to")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(claims_response.status(), StatusCode::OK);
        let body = to_bytes(claims_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let payload: ClaimCatalogResponse = serde_json::from_slice(&body).unwrap();
        let claim = payload
            .claims
            .iter()
            .find(|entry| {
                entry.claim.subject == "alice north" && entry.claim.predicate == "appointed_to"
            })
            .expect("claim should exist");

        let review_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!("/api/v1/claims/{}/review", claim.claim.id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&ClaimReviewRequest {
                            status: helix_core::intel_desk::ClaimReviewStatus::Corroborated,
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(review_response.status(), StatusCode::OK);

        let corroborated_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/claims?review_status=corroborated&limit=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(corroborated_response.status(), StatusCode::OK);
        let body = to_bytes(corroborated_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let payload: ClaimCatalogResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.claims.len(), 1);
        assert_eq!(
            payload.claims[0].claim.review_status,
            helix_core::intel_desk::ClaimReviewStatus::Corroborated
        );
    }

    #[tokio::test]
    async fn claims_endpoint_semantic_query_ranks_relevant_claims() {
        let app = test_app();
        let source = create_test_source(app.clone(), "Semantic Retrieval Claim Source").await;
        let source_id = source.source.id;

        ingest_test_evidence(
            app.clone(),
            IngestEvidenceRequest {
                source_id: source_id.clone(),
                title: "Orbital telemetry launch expansion".to_string(),
                summary: "Satellite operations expansion signal.".to_string(),
                content: "Telemetry team prepared satellite orbit launch operations.".to_string(),
                url: None,
                observed_at: "2026-03-10T11:00:00Z".to_string(),
                tags: vec!["space".to_string()],
                entity_labels: vec!["orion telemetry".to_string()],
                proposed_claims: vec![helix_core::intel_desk::ProposedClaim {
                    subject: "orion telemetry".to_string(),
                    predicate: "semantic_retrieval_marker".to_string(),
                    object: "satellite orbit launch".to_string(),
                    confidence_bps: 9300,
                    rationale: Some("orbital launch telemetry signal".to_string()),
                }],
            },
        )
        .await;
        ingest_test_evidence(
            app.clone(),
            IngestEvidenceRequest {
                source_id,
                title: "Enterprise pricing renewal change".to_string(),
                summary: "Commercial packaging update.".to_string(),
                content: "Renewal discounting and annual pricing package language changed."
                    .to_string(),
                url: None,
                observed_at: "2026-03-10T12:00:00Z".to_string(),
                tags: vec!["pricing".to_string()],
                entity_labels: vec!["renewal desk".to_string()],
                proposed_claims: vec![helix_core::intel_desk::ProposedClaim {
                    subject: "renewal desk".to_string(),
                    predicate: "semantic_retrieval_marker".to_string(),
                    object: "pricing package discount".to_string(),
                    confidence_bps: 9300,
                    rationale: Some("annual renewal pricing language".to_string()),
                }],
            },
        )
        .await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri(
                        "/api/v1/claims?predicate=semantic_retrieval_marker&q=satellite%20orbit%20telemetry&limit=1",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: ClaimCatalogResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.claims.len(), 1);
        assert_eq!(payload.claims[0].claim.subject, "orion telemetry");
        assert!(payload.claims[0].semantic_score_bps.is_some());
    }

    #[tokio::test]
    async fn claims_endpoint_rejects_invalid_min_confidence() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/claims?min_confidence_bps=10001")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn market_intel_brief_endpoint_attaches_summary_to_case() {
        let app = test_app();
        let overview_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/market-intel/overview")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(overview_response.status(), StatusCode::OK);

        let body = to_bytes(overview_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let overview: MarketIntelOverviewResponse = serde_json::from_slice(&body).unwrap();
        let brief_case = overview
            .case_briefs
            .first()
            .expect("seeded market brief should exist");

        let brief_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!(
                        "/api/v1/market-intel/cases/{}/brief",
                        brief_case.case_id
                    ))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&GenerateMarketIntelBriefRequest {
                            attach_to_case: true,
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(brief_response.status(), StatusCode::OK);

        let body = to_bytes(brief_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let payload: GenerateMarketIntelBriefResponse = serde_json::from_slice(&body).unwrap();
        assert!(payload.briefing.summary.contains("signal for"));
        assert!(payload.briefing.attached_to_case);
        assert!(payload.transition.is_some());
        assert_eq!(
            payload.transition.unwrap().case.status,
            helix_core::intel_desk::CaseStatus::BriefReady
        );
    }

    #[tokio::test]
    async fn market_intel_brief_export_endpoint_returns_packet() {
        let app = test_app();
        let overview_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/market-intel/overview")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(overview_response.status(), StatusCode::OK);

        let body = to_bytes(overview_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let overview: MarketIntelOverviewResponse = serde_json::from_slice(&body).unwrap();
        let brief = overview
            .case_briefs
            .first()
            .expect("seeded market brief should exist");

        let response = app
            .oneshot(
                Request::builder()
                    .uri(&format!(
                        "/api/v1/market-intel/cases/{}/export",
                        brief.case_id
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let packet: MarketIntelBriefExportPacketResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(packet.briefing.case_id, brief.case_id);
        assert_eq!(packet.case_file.id, brief.case_id);
        assert!(!packet.packet_id.is_empty());
        assert!(!packet.evidence.is_empty());
        assert!(!packet.claims.is_empty());
    }

    #[tokio::test]
    async fn sources_endpoint_creates_and_lists_source() {
        let app = test_app();
        let request = CreateSourceRequest {
            profile_id: None,
            name: "Field Notes".to_string(),
            description: "Manual analyst import".to_string(),
            kind: helix_core::intel_desk::SourceKind::FileImport,
            endpoint_url: None,
            credential_id: None,
            credential_header_name: None,
            credential_header_prefix: None,
            cadence_minutes: 45,
            trust_score: 73,
            enabled: true,
            tags: vec!["manual".to_string(), "notes".to_string()],
        };

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sources")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::CREATED);

        let body = to_bytes(create_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let created: SourceResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            created.source.profile_id,
            "50000000-0000-0000-0000-000000000010"
        );
        assert!(created.source.credential_id.is_none());
        assert_eq!(created.source.credential_header_name, "Authorization");
        assert_eq!(created.source.trust_score, 73);
        assert_eq!(
            created.source.tags,
            vec!["manual".to_string(), "notes".to_string()]
        );

        let list_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/sources")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_response.status(), StatusCode::OK);
        let body = to_bytes(list_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let payload: SourceCatalogResponse = serde_json::from_slice(&body).unwrap();
        assert!(payload
            .sources
            .iter()
            .any(|source| source.name == "Field Notes"));
    }

    #[tokio::test]
    async fn source_collect_endpoint_fetches_json_and_opens_case() {
        let feed_url = spawn_text_server(
            "/pricing.json",
            r#"{
              "items": [
                {
                  "title": "Boreal Cloud pricing bundle changed",
                  "summary": "Boreal Cloud introduced a pricing bundle for enterprise seats.",
                  "content": "Boreal Cloud pricing changed with a new enterprise bundle and seat discount.",
                  "url": "https://example.org/boreal/pricing",
                  "observed_at": "2026-04-01T10:00:00Z",
                  "tags": ["pricing"],
                  "entity_labels": ["boreal cloud"],
                  "proposed_claims": [
                    {
                      "subject": "boreal cloud",
                      "predicate": "changed_pricing",
                      "object": "enterprise bundle",
                      "confidence_bps": 8800,
                      "rationale": "The source states a bundle and discount change."
                    }
                  ]
                }
              ]
            }"#,
        )
        .await;
        let app = test_app();
        let create = CreateSourceRequest {
            profile_id: None,
            name: "Boreal Pricing Feed".to_string(),
            description: "Live pricing changes from a JSON feed".to_string(),
            kind: helix_core::intel_desk::SourceKind::JsonApi,
            endpoint_url: Some(feed_url),
            credential_id: None,
            credential_header_name: None,
            credential_header_prefix: None,
            cadence_minutes: 30,
            trust_score: 90,
            enabled: true,
            tags: vec!["market-intel".to_string(), "pricing".to_string()],
        };

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sources")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&create).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let body = to_bytes(create_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let created: SourceResponse = serde_json::from_slice(&body).unwrap();

        let collect_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sources/{}/collect", created.source.id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"observed_at":"2026-04-01T10:05:00Z","max_items":5}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(collect_response.status(), StatusCode::CREATED);
        let body = to_bytes(collect_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let payload: CollectSourceResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.collected_count, 1);
        assert_eq!(payload.duplicate_count, 0);
        assert_eq!(payload.results[0].claims.len(), 1);
        assert!(!payload.results[0].hits.is_empty());
        assert!(!payload.results[0].case_updates.is_empty());

        let cases_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/cases?watchlist_id=market_pricing_moves")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cases_response.status(), StatusCode::OK);
        let body = to_bytes(cases_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let cases: CaseCatalogResponse = serde_json::from_slice(&body).unwrap();
        assert!(cases
            .cases
            .iter()
            .any(|entry| entry.case.primary_entity.as_deref() == Some("boreal cloud")));
    }

    #[tokio::test]
    async fn source_webhook_endpoint_ingests_payload_and_opens_case() {
        let app = test_app();
        let create = CreateSourceRequest {
            profile_id: None,
            name: "Boreal Webhook".to_string(),
            description: "Inbound partner intelligence webhook".to_string(),
            kind: helix_core::intel_desk::SourceKind::WebhookIngest,
            endpoint_url: None,
            credential_id: None,
            credential_header_name: None,
            credential_header_prefix: None,
            cadence_minutes: 15,
            trust_score: 88,
            enabled: true,
            tags: vec!["market-intel".to_string(), "webhook".to_string()],
        };

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sources")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&create).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let body = to_bytes(create_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let created: SourceResponse = serde_json::from_slice(&body).unwrap();

        let webhook_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sources/{}/webhook", created.source.id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                          "observed_at": "2026-04-02T10:00:00Z",
                          "items": [
                            {
                              "title": "Boreal Cloud webhook reports a bundle discount",
                              "summary": "Inbound partner webhook says Boreal Cloud is discounting an enterprise bundle.",
                              "content": "Boreal Cloud pricing changed after a new enterprise bundle discount was pushed to partners.",
                              "url": "https://example.org/webhook/boreal-bundle",
                              "tags": ["pricing"],
                              "entity_labels": ["boreal cloud"],
                              "proposed_claims": [
                                {
                                  "subject": "boreal cloud",
                                  "predicate": "discounted",
                                  "object": "enterprise bundle",
                                  "confidence_bps": 9000,
                                  "rationale": "The webhook payload states the partner discount."
                                }
                              ]
                            }
                          ]
                        }"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(webhook_response.status(), StatusCode::CREATED);
        let body = to_bytes(webhook_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let payload: WebhookIngestResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.accepted_count, 1);
        assert_eq!(payload.duplicate_count, 0);
        assert_eq!(payload.results[0].claims.len(), 1);
        assert!(!payload.results[0].hits.is_empty());
        assert!(!payload.results[0].case_updates.is_empty());
        assert!(payload.results[0]
            .evidence
            .tags
            .contains(&"webhook".to_string()));
    }

    #[tokio::test]
    async fn source_webhook_endpoint_rejects_non_webhook_source() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sources/rss_national_security/webhook")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"observed_at":"2026-04-02T10:00:00Z","items":[{"title":"Denied","summary":"Denied","content":"Denied"}]}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn source_webhook_endpoint_rejects_payload_boundaries() {
        let app = test_app();
        let create = CreateSourceRequest {
            profile_id: None,
            name: "Boundary Webhook".to_string(),
            description: "Webhook boundary source".to_string(),
            kind: helix_core::intel_desk::SourceKind::WebhookIngest,
            endpoint_url: None,
            credential_id: None,
            credential_header_name: None,
            credential_header_prefix: None,
            cadence_minutes: 15,
            trust_score: 88,
            enabled: true,
            tags: vec!["boundary".to_string()],
        };
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sources")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&create).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let body = to_bytes(create_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let created: SourceResponse = serde_json::from_slice(&body).unwrap();

        let empty_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sources/{}/webhook", created.source.id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"observed_at":"2026-04-02T10:00:00Z","items":[]}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(empty_response.status(), StatusCode::BAD_REQUEST);

        let too_many_items = serde_json::json!({
            "observed_at": "2026-04-02T10:00:00Z",
            "items": (0..=50)
                .map(|index| serde_json::json!({
                    "title": format!("Webhook item {index}"),
                    "summary": "summary",
                    "content": "content"
                }))
                .collect::<Vec<_>>()
        });
        let too_many_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sources/{}/webhook", created.source.id))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&too_many_items).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(too_many_response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn source_file_import_endpoint_ingests_file_and_opens_case() {
        let app = test_app();
        let create = CreateSourceRequest {
            profile_id: None,
            name: "Boreal File Drop".to_string(),
            description: "Operator-uploaded competitive intelligence files".to_string(),
            kind: helix_core::intel_desk::SourceKind::FileImport,
            endpoint_url: None,
            credential_id: None,
            credential_header_name: None,
            credential_header_prefix: None,
            cadence_minutes: 60,
            trust_score: 86,
            enabled: true,
            tags: vec!["market-intel".to_string(), "file".to_string()],
        };

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sources")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&create).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let body = to_bytes(create_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let created: SourceResponse = serde_json::from_slice(&body).unwrap();

        let import_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sources/{}/import", created.source.id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                          "file_name": "boreal-pricing-note.md",
                          "title": "Boreal Cloud file note reports a bundle discount",
                          "content": "Boreal Cloud pricing changed after an enterprise bundle discount was distributed in a partner note.",
                          "observed_at": "2026-04-03T10:00:00Z",
                          "tags": ["pricing"],
                          "entity_labels": ["boreal cloud"],
                          "proposed_claims": [
                            {
                              "subject": "boreal cloud",
                              "predicate": "discounted",
                              "object": "enterprise bundle",
                              "confidence_bps": 8700,
                              "rationale": "The imported note states the bundle discount."
                            }
                          ]
                        }"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(import_response.status(), StatusCode::CREATED);
        let body = to_bytes(import_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let payload: FileImportResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.result.claims.len(), 1);
        assert!(!payload.result.hits.is_empty());
        assert!(!payload.result.case_updates.is_empty());
        assert!(payload.result.evidence.tags.contains(&"file".to_string()));
    }

    #[tokio::test]
    async fn source_file_import_endpoint_rejects_kind_and_content_boundaries() {
        let wrong_kind_response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sources/rss_national_security/import")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"file_name":"note.txt","content":"content","observed_at":"2026-04-03T10:00:00Z"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(wrong_kind_response.status(), StatusCode::BAD_REQUEST);

        let app = test_app();
        let create = CreateSourceRequest {
            profile_id: None,
            name: "Boundary File Drop".to_string(),
            description: "File boundary source".to_string(),
            kind: helix_core::intel_desk::SourceKind::FileImport,
            endpoint_url: None,
            credential_id: None,
            credential_header_name: None,
            credential_header_prefix: None,
            cadence_minutes: 60,
            trust_score: 86,
            enabled: true,
            tags: vec!["boundary".to_string()],
        };
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sources")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&create).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let body = to_bytes(create_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let created: SourceResponse = serde_json::from_slice(&body).unwrap();

        let empty_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sources/{}/import", created.source.id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"file_name":"empty.txt","content":"","observed_at":"2026-04-03T10:00:00Z"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(empty_response.status(), StatusCode::BAD_REQUEST);

        let oversized = serde_json::json!({
            "file_name": "large.txt",
            "content": "x".repeat(16_385),
            "observed_at": "2026-04-03T10:00:00Z"
        });
        let oversized_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sources/{}/import", created.source.id))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&oversized).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(oversized_response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn source_collect_endpoint_rejects_zero_limit() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sources/rss_national_security/collect")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"observed_at":"2026-04-01T10:05:00Z","max_items":0}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn watchlists_endpoint_creates_watchlist() {
        let request = CreateWatchlistRequest {
            name: "Launch Monitor".to_string(),
            description: "Track launches and facilities".to_string(),
            keywords: vec!["launch".to_string()],
            entities: vec!["orion dynamics".to_string()],
            min_source_trust: 55,
            severity: helix_core::intel_desk::WatchlistSeverity::Medium,
            enabled: true,
        };

        let response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/watchlists")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: WatchlistResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.watchlist.min_source_trust, 55);
    }

    #[tokio::test]
    async fn evidence_ingest_creates_claims_hits_and_case() {
        let app = test_app();
        let request = IngestEvidenceRequest {
            source_id: "rss_national_security".to_string(),
            title: "Alice North resigned from Orion Dynamics".to_string(),
            summary: "Leadership change at Orion".to_string(),
            content: "Alice North resigned after a brief detention, according to the report."
                .to_string(),
            url: Some("https://example.org/report".to_string()),
            observed_at: "2026-03-06T12:00:00Z".to_string(),
            tags: vec!["leadership".to_string(), "security".to_string()],
            entity_labels: vec!["alice north".to_string(), "orion dynamics".to_string()],
            proposed_claims: vec![helix_core::intel_desk::ProposedClaim {
                subject: "alice north".to_string(),
                predicate: "resigned_from".to_string(),
                object: "orion dynamics".to_string(),
                confidence_bps: 9100,
                rationale: Some("explicitly stated in the report".to_string()),
            }],
        };

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/evidence/ingest")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: IngestEvidenceResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.claims.len(), 1);
        assert!(!payload.hits.is_empty());
        assert!(!payload.case_updates.is_empty());
        assert_eq!(
            payload.case_updates[0].case.status,
            helix_core::intel_desk::CaseStatus::Escalated
        );

        let cases_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/cases")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cases_response.status(), StatusCode::OK);
        let body = to_bytes(cases_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let cases: CaseCatalogResponse = serde_json::from_slice(&body).unwrap();
        assert!(cases
            .cases
            .iter()
            .any(|entry| entry.case.id == payload.case_updates[0].case.id));
    }

    #[tokio::test]
    async fn case_transition_endpoint_updates_status() {
        let app = test_app();
        let ingest = IngestEvidenceRequest {
            source_id: "rss_national_security".to_string(),
            title: "Alice North appointed at Orion Dynamics".to_string(),
            summary: "Leadership change".to_string(),
            content: "Alice North was appointed to a new role at Orion Dynamics.".to_string(),
            url: None,
            observed_at: "2026-03-06T12:30:00Z".to_string(),
            tags: vec!["leadership".to_string()],
            entity_labels: vec!["alice north".to_string()],
            proposed_claims: vec![helix_core::intel_desk::ProposedClaim {
                subject: "alice north".to_string(),
                predicate: "appointed_to".to_string(),
                object: "orion dynamics".to_string(),
                confidence_bps: 8700,
                rationale: Some("appointment notice".to_string()),
            }],
        };

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/evidence/ingest")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&ingest).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let ingest_payload: IngestEvidenceResponse = serde_json::from_slice(&body).unwrap();
        let case_id = ingest_payload.case_updates[0].case.id.clone();

        let transition_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!("/api/v1/cases/{case_id}/transition"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&CaseTransitionRequest {
                            command: helix_core::intel_desk::CaseCommand::Close,
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(transition_response.status(), StatusCode::OK);

        let body = to_bytes(transition_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let payload: CaseTransitionResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            payload.transition.case.status,
            helix_core::intel_desk::CaseStatus::Closed
        );
    }

    #[tokio::test]
    async fn claim_review_endpoint_updates_review_status() {
        let app = test_app();
        let ingest = IngestEvidenceRequest {
            source_id: "rss_national_security".to_string(),
            title: "Alice North appointment verified".to_string(),
            summary: "Leadership update".to_string(),
            content: "Alice North was appointed to a new role at Orion Dynamics.".to_string(),
            url: Some("https://example.org/appointment".to_string()),
            observed_at: "2026-03-06T14:00:00Z".to_string(),
            tags: vec!["leadership".to_string()],
            entity_labels: vec!["alice north".to_string(), "orion dynamics".to_string()],
            proposed_claims: vec![helix_core::intel_desk::ProposedClaim {
                subject: "alice north".to_string(),
                predicate: "appointed_to".to_string(),
                object: "orion dynamics".to_string(),
                confidence_bps: 9200,
                rationale: Some("appointment notice".to_string()),
            }],
        };

        let ingest_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/evidence/ingest")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&ingest).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ingest_response.status(), StatusCode::CREATED);

        let claims_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/claims")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(claims_response.status(), StatusCode::OK);

        let body = to_bytes(claims_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let payload: ClaimCatalogResponse = serde_json::from_slice(&body).unwrap();
        let claim = payload
            .claims
            .iter()
            .find(|entry| {
                entry.claim.subject == "alice north" && entry.claim.predicate == "appointed_to"
            })
            .expect("claim should exist");

        let review_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!("/api/v1/claims/{}/review", claim.claim.id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&ClaimReviewRequest {
                            status: helix_core::intel_desk::ClaimReviewStatus::Corroborated,
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(review_response.status(), StatusCode::OK);

        let body = to_bytes(review_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let payload: ClaimResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            payload.claim.review_status,
            helix_core::intel_desk::ClaimReviewStatus::Corroborated
        );
    }

    #[tokio::test]
    async fn autopilot_review_queue_endpoint_returns_ranked_items() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/autopilot/review-queue?limit=12")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: AutopilotReviewQueueResponse = serde_json::from_slice(&body).unwrap();
        assert!(!payload.items.is_empty());
        assert!(payload
            .items
            .iter()
            .any(|item| item.kind == AutopilotReviewKind::Case));
        assert!(payload
            .items
            .iter()
            .any(|item| item.kind == AutopilotReviewKind::Claim));
        assert!(payload
            .items
            .iter()
            .any(|item| item.kind == AutopilotReviewKind::Evidence));
        assert!(payload
            .items
            .iter()
            .all(|item| item.case_status != Some(helix_core::intel_desk::CaseStatus::Closed)));

        for pair in payload.items.windows(2) {
            let left = &pair[0];
            let right = &pair[1];
            let left_key = (
                std::cmp::Reverse(left.priority.total),
                std::cmp::Reverse(left.latest_signal_at.clone()),
                match left.kind {
                    AutopilotReviewKind::Case => 0u8,
                    AutopilotReviewKind::Claim => 1u8,
                    AutopilotReviewKind::Evidence => 2u8,
                },
                left.item_id.clone(),
            );
            let right_key = (
                std::cmp::Reverse(right.priority.total),
                std::cmp::Reverse(right.latest_signal_at.clone()),
                match right.kind {
                    AutopilotReviewKind::Case => 0u8,
                    AutopilotReviewKind::Claim => 1u8,
                    AutopilotReviewKind::Evidence => 2u8,
                },
                right.item_id.clone(),
            );
            assert!(left_key <= right_key);
        }
    }

    #[tokio::test]
    async fn autopilot_review_queue_endpoint_filters_by_kind_and_limit() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/autopilot/review-queue?kind=claim&limit=2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: AutopilotReviewQueueResponse = serde_json::from_slice(&body).unwrap();
        assert!(payload.items.len() <= 2);
        assert!(payload
            .items
            .iter()
            .all(|item| item.kind == AutopilotReviewKind::Claim));
        assert!(payload.items.iter().all(|item| item.claim_review_status
            != Some(helix_core::intel_desk::ClaimReviewStatus::Rejected)));
    }

    #[tokio::test]
    async fn autopilot_review_queue_endpoint_rejects_zero_limit() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/autopilot/review-queue?limit=0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn autopilot_review_propose_endpoint_uses_ranked_item_context() {
        let provider = StubLlmProvider {
            content: "{\"type\":\"policy_simulation\",\"commands\":[{\"type\":\"tick\"}]}"
                .to_string(),
            model: "stub-model".to_string(),
        };
        let app = test_app_with_llm(Arc::new(provider), "stub-model".to_string());

        let queue_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/autopilot/review-queue?kind=claim&limit=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(queue_response.status(), StatusCode::OK);

        let body = to_bytes(queue_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let queue: AutopilotReviewQueueResponse = serde_json::from_slice(&body).unwrap();
        let item = queue.items.first().expect("seeded claim review item");

        let request = AutopilotReviewProposeRequest {
            review_kind: AutopilotReviewKind::Claim,
            item_id: item.item_id.clone(),
            kind: AutopilotProposeKind::PolicySimulation,
            rpc_url: None,
            raw_tx_hex: None,
            dry_run: None,
        };

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/autopilot/review-queue/propose")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: AutopilotReviewProposeResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.review_item.kind, AutopilotReviewKind::Claim);
        assert_eq!(payload.review_item.item_id, item.item_id.clone());
        assert_eq!(payload.proposal.model, "stub-model");

        match payload.proposal.action {
            AutopilotActionRequest::PolicySimulation { commands } => {
                assert_eq!(commands.len(), 1);
                assert!(matches!(commands[0], PolicyCommand::Tick));
            }
            other => panic!("unexpected proposal action: {:?}", other),
        }
    }

    #[tokio::test]
    async fn autopilot_review_export_endpoint_returns_deterministic_packet() {
        let app = test_app();
        let queue_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/autopilot/review-queue?kind=claim&limit=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(queue_response.status(), StatusCode::OK);

        let body = to_bytes(queue_response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let queue: AutopilotReviewQueueResponse = serde_json::from_slice(&body).unwrap();
        let item = queue.items.first().expect("seeded claim review item");

        let response = app
            .oneshot(
                Request::builder()
                    .uri(&format!(
                        "/api/v1/autopilot/review-queue/export?review_kind=claim&item_id={}",
                        item.item_id
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let packet: AutopilotReviewExportPacketResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(packet.kind, AutopilotReviewKind::Claim);
        assert_eq!(packet.item.item_id, item.item_id.clone());
        assert!(!packet.packet_id.is_empty());
        assert!(!packet.supporting_claims.is_empty());
        assert!(!packet.supporting_evidence.is_empty());
    }

    #[tokio::test]
    async fn autopilot_status_endpoint_works() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/autopilot/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let status: AutopilotStatusResponse = serde_json::from_slice(&body).unwrap();
        assert!(status.config.max_policy_commands > 0);
    }

    #[tokio::test]
    async fn autopilot_propose_denies_when_llm_not_configured() {
        let app = test_app();
        let request = AutopilotProposeRequest {
            goal: "simulate a safe tick".to_string(),
            kind: AutopilotProposeKind::PolicySimulation,
            rpc_url: None,
            raw_tx_hex: None,
            dry_run: None,
        };

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/autopilot/propose")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: AutopilotProposeErrorResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.error, "llm_not_configured");
    }

    #[tokio::test]
    async fn autopilot_propose_parses_action_and_previews_guard() {
        let provider = StubLlmProvider {
            content: "{\"type\":\"policy_simulation\",\"commands\":[{\"type\":\"tick\"}]}"
                .to_string(),
            model: "stub-model".to_string(),
        };
        let app = test_app_with_llm(Arc::new(provider), "stub-model".to_string());

        let request = AutopilotProposeRequest {
            goal: "simulate a safe tick".to_string(),
            kind: AutopilotProposeKind::PolicySimulation,
            rpc_url: None,
            raw_tx_hex: None,
            dry_run: None,
        };

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/autopilot/propose")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: AutopilotProposeResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.model, "stub-model");

        match payload.action {
            AutopilotActionRequest::PolicySimulation { commands } => {
                assert_eq!(commands.len(), 1);
                assert!(matches!(commands[0], PolicyCommand::Tick));
            }
            other => panic!("unexpected proposal action: {:?}", other),
        }

        assert!(matches!(
            payload.guard_preview.decision_unconfirmed,
            AutopilotGuardDecision::Deny { reason } if reason == "assist_requires_confirmation"
        ));
        assert!(matches!(
            payload.guard_preview.decision_confirmed,
            AutopilotGuardDecision::Allow {
                requires_confirmation: true
            }
        ));
    }

    #[tokio::test]
    async fn autopilot_execute_denies_unconfirmed_assist_request() {
        let app = test_app();
        let request = AutopilotExecuteRequest {
            confirmed_by_human: false,
            action: AutopilotActionRequest::PolicySimulation {
                commands: vec![PolicyCommand::Tick],
            },
        };
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/autopilot/execute")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: AutopilotExecuteResponse = serde_json::from_slice(&body).unwrap();
        assert!(!payload.allowed);
        assert_eq!(
            payload.reason.as_deref(),
            Some("assist_requires_confirmation")
        );
    }

    #[tokio::test]
    async fn autopilot_execute_allows_confirmed_policy_simulation() {
        let app = test_app();
        let request = AutopilotExecuteRequest {
            confirmed_by_human: true,
            action: AutopilotActionRequest::PolicySimulation {
                commands: vec![PolicyCommand::Tick],
            },
        };
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/autopilot/execute")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: AutopilotExecuteResponse = serde_json::from_slice(&body).unwrap();
        assert!(payload.allowed);
        assert!(payload.result.is_some());
    }

    #[tokio::test]
    async fn onchain_send_raw_dry_run_returns_pending_hash() {
        let app = test_app();
        let request = OnchainBroadcastRequest {
            rpc_url: "https://rpc.example.org".to_string(),
            raw_tx_hex: "0xdeadbeef".to_string(),
            await_receipt: Some(true),
            max_poll_rounds: Some(3),
            poll_interval_ms: Some(10),
            dry_run: Some(true),
        };

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/onchain/send_raw")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let payload: OnchainBroadcastResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.phase, OnchainPhase::PendingReceipt);
        assert_eq!(payload.tx_hash.as_ref().map(|h| h.len()), Some(66));
        assert_eq!(payload.poll_rounds, 0);
    }

    #[tokio::test]
    async fn onchain_receipt_validation_fail_returns_bad_request() {
        let app = test_app();
        let request = OnchainReceiptRequest {
            rpc_url: "https://rpc.example.org".to_string(),
            tx_hash: "0xabc".to_string(),
        };

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/onchain/receipt")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
