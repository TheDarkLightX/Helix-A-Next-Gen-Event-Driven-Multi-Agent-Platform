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

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use helix_core::autopilot_guard::{
    AutopilotActionClass, AutopilotGuardConfig, AutopilotGuardDecision, AutopilotGuardInput,
    AutopilotGuardMachine, AutopilotMode, AutopilotStats,
};
use helix_core::deterministic_agent_catalog::{
    agent_catalog_quality, high_roi_agent_catalog, AgentCatalogQuality, DeterministicAgentSpec,
};
use helix_core::deterministic_agent_profiles::{
    find_agent_template, high_roi_agent_templates, DeterministicAgentTemplate,
};
use helix_core::deterministic_agents_expanded::{
    simulate_expanded_guard, TemporalGuardInput, TemporalGuardSimulation,
};
use helix_core::deterministic_policy::{
    DeterministicPolicyConfig, DeterministicPolicyEngine, PolicyCommand, PolicyDecision,
    PolicyStepResult,
};
use helix_core::onchain_intent::{
    step as onchain_step, OnchainInput, OnchainKernelError, OnchainPhase, OnchainState,
};
use helix_core::reasoning::{evaluate_reasoning, ReasoningDecision, ReasoningEvaluationRequest};
use helix_core::HelixError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::evm_rpc::{deterministic_dry_run_hash, EvmReceipt, EvmRpcClient};

#[derive(Clone)]
struct AppState {
    policy_config: Arc<RwLock<DeterministicPolicyConfig>>,
    autopilot_guard: Arc<RwLock<AutopilotGuardMachine>>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let state = AppState {
        policy_config: Arc::new(RwLock::new(DeterministicPolicyConfig::default())),
        autopilot_guard: Arc::new(RwLock::new(AutopilotGuardMachine::new(
            autopilot_config_from_env(),
        ))),
    };
    let app = app(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
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

async fn health_check() -> impl IntoResponse {
    let health = HealthStatus {
        status: "ok".to_string(),
    };
    (StatusCode::OK, Json(health))
}

async fn get_policy_config(State(state): State<AppState>) -> impl IntoResponse {
    let config = *state.policy_config.read().await;
    (StatusCode::OK, Json(PolicyConfigResponse { config }))
}

async fn put_policy_config(
    State(state): State<AppState>,
    Json(req): Json<PolicyConfigResponse>,
) -> impl IntoResponse {
    *state.policy_config.write().await = req.config;
    (
        StatusCode::OK,
        Json(PolicyConfigResponse { config: req.config }),
    )
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

async fn post_reasoning_evaluate(Json(req): Json<ReasoningEvaluationRequest>) -> Response {
    match evaluate_reasoning(req) {
        Ok(decision) => {
            (StatusCode::OK, Json(ReasoningEvaluateResponse { decision })).into_response()
        }
        Err(err) => api_error_response(err),
    }
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

    *state.policy_config.write().await = template.config;

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
) -> impl IntoResponse {
    let mut guard = state.autopilot_guard.write().await;
    let _ = guard.step(AutopilotGuardInput::SetConfig { config: req.config });
    (
        StatusCode::OK,
        Json(AutopilotStatusResponse {
            config: guard.config(),
            stats: guard.stats(),
        }),
    )
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

    let guard_decision = {
        let mut guard = state.autopilot_guard.write().await;
        guard.step(AutopilotGuardInput::Evaluate {
            action: action_class,
            confirmed_by_human: req.confirmed_by_human,
        })
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

fn autopilot_config_from_env() -> AutopilotGuardConfig {
    AutopilotGuardConfig {
        mode: parse_autopilot_mode_env("HELIX_AUTOPILOT_MODE").unwrap_or(AutopilotMode::Assist),
        allow_onchain: parse_bool_env("HELIX_AUTOPILOT_ALLOW_ONCHAIN", false),
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

fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route(
            "/api/v1/policy/config",
            get(get_policy_config).put(put_policy_config),
        )
        .route("/api/v1/policy/simulate", post(simulate_policy))
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
        .route("/api/v1/reasoning/evaluate", post(post_reasoning_evaluate))
        .route("/api/v1/autopilot/status", get(get_autopilot_status))
        .route(
            "/api/v1/autopilot/config",
            get(get_autopilot_status).put(put_autopilot_config),
        )
        .route("/api/v1/autopilot/execute", post(post_autopilot_execute))
        .route("/api/v1/onchain/send_raw", post(onchain_send_raw))
        .route("/api/v1/onchain/receipt", post(onchain_get_receipt))
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use tower::ServiceExt;

    fn test_app() -> Router {
        let state = AppState {
            policy_config: Arc::new(RwLock::new(DeterministicPolicyConfig::default())),
            autopilot_guard: Arc::new(RwLock::new(AutopilotGuardMachine::default())),
        };
        app(state)
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
