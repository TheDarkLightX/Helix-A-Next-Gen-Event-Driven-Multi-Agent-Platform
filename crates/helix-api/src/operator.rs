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

//! Operator API handlers: configuration, status, start/stop, activity log,
//! and manual cycle trigger.

use crate::{api_error_response, AppState, HelixError};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use helix_core::autopilot_guard::AutopilotMode;
use helix_core::intel_desk::CaseStatus;
use helix_operator::context::{
    DeskContext, DeskContextSnapshot, DeskContextSummary, EvidenceSummary,
    RecentDecisionSummary, WatchlistSummary,
};
use helix_operator::{
    DeskOperatorConfig, OperatorActionScope, OperatorActivityEntry, OperatorStatus,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Adapter that implements [`DeskContext`] by snapshotting the API's intel desk.
struct ApiDeskContext {
    snapshot: DeskContextSnapshot,
}

impl DeskContext for ApiDeskContext {
    fn snapshot(&self, _max_items: usize) -> DeskContextSnapshot {
        self.snapshot.clone()
    }
}

/// Builds a [`DeskContextSnapshot`] from the API's intel desk store.
fn build_desk_snapshot(
    desk: &crate::intel::IntelDeskStore,
    max_items: usize,
    recent_activity: Vec<OperatorActivityEntry>,
) -> DeskContextSnapshot {
    let source_count = desk.sources.len();
    let watchlist_count = desk.watchlists.len();
    let evidence_count = desk.evidence.len();
    let claim_count = desk.claims.len();
    let open_case_count = desk
        .cases
        .values()
        .filter(|c| !matches!(c.status, CaseStatus::Closed))
        .count();
    let escalated_case_count = desk
        .cases
        .values()
        .filter(|c| matches!(c.status, CaseStatus::Escalated))
        .count();

    // Top cases (open, sorted by evidence count as a proxy for priority)
    let mut top_cases: Vec<_> = desk
        .cases
        .values()
        .filter(|c| !matches!(c.status, CaseStatus::Closed))
        .map(|c| DeskContextSummary {
            id: c.id.clone(),
            title: c.title.clone(),
            status: format!("{:?}", c.status).to_lowercase(),
            watchlist_id: c.watchlist_id.clone(),
            primary_entity: c.primary_entity.clone(),
            evidence_count: c.evidence_ids.len(),
            claim_count: c.claim_ids.len(),
            latest_reason: c.latest_reason.clone(),
            priority_total: (c.evidence_ids.len() as u64) * 10 + (c.claim_ids.len() as u64) * 5,
        })
        .collect();
    top_cases.sort_by(|a, b| b.priority_total.cmp(&a.priority_total));
    top_cases.truncate(max_items);

    // Recent evidence
    let recent_evidence: Vec<EvidenceSummary> = desk
        .evidence
        .values()
        .rev()
        .take(max_items)
        .map(|e| {
            let trust_score = desk
                .sources
                .get(&e.source_id)
                .map(|s| s.trust_score)
                .unwrap_or(0);
            EvidenceSummary {
                id: e.id.clone(),
                title: e.title.clone(),
                source_id: e.source_id.clone(),
                trust_score,
                observed_at: e.observed_at.clone(),
                entity_labels: e.entity_labels.clone(),
                tags: e.tags.clone(),
            }
        })
        .collect();

    // Active watchlists
    let active_watchlists: Vec<WatchlistSummary> = desk
        .watchlists
        .values()
        .filter(|w| w.enabled)
        .take(max_items)
        .map(|w| WatchlistSummary {
            id: w.id.clone(),
            name: w.name.clone(),
            severity: format!("{:?}", w.severity).to_lowercase(),
            keywords: w.keywords.clone(),
            enabled: w.enabled,
        })
        .collect();

    // Recent decisions from activity log
    let recent_decisions: Vec<RecentDecisionSummary> = recent_activity
        .into_iter()
        .map(|e| RecentDecisionSummary {
            action_type: e.action_type,
            decision: if e.allowed { "allowed".into() } else { "denied".into() },
            denial_reason: e.denial_reason,
            rationale: e.rationale,
            timestamp: e.timestamp,
        })
        .collect();

    DeskContextSnapshot {
        source_count,
        watchlist_count,
        evidence_count,
        claim_count,
        open_case_count,
        escalated_case_count,
        top_cases,
        recent_evidence,
        active_watchlists,
        recent_decisions,
    }
}

/// Query params for limiting activity log results.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ActivityLogQuery {
    /// Maximum number of entries to return (newest first). Defaults to 50.
    pub limit: Option<usize>,
}

/// Response for `GET /api/v1/operator/status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorStatusResponse {
    /// Current run status (stopped/running/paused).
    pub status: OperatorStatus,
    /// Whether the operator is enabled in config.
    pub enabled: bool,
    /// Current autopilot mode.
    pub autopilot_mode: AutopilotMode,
    /// Current action scope.
    pub action_scope: OperatorActionScope,
    /// Loop interval in seconds.
    pub loop_interval_secs: u64,
    /// LLM model in use.
    pub model: String,
    /// Number of cycles executed.
    pub cycle_count: u64,
    /// Number of activity log entries.
    pub activity_log_count: usize,
}

/// Request body for `PUT /api/v1/operator/config`.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateOperatorConfigRequest {
    /// Whether the operator loop is enabled.
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Autopilot mode.
    #[serde(default)]
    pub autopilot_mode: Option<AutopilotMode>,
    /// Action scope.
    #[serde(default)]
    pub action_scope: Option<OperatorActionScope>,
    /// Loop interval in seconds.
    #[serde(default)]
    pub loop_interval_secs: Option<u64>,
    /// Max actions per cycle.
    #[serde(default)]
    pub max_actions_per_cycle: Option<usize>,
    /// Max context items.
    #[serde(default)]
    pub max_context_items: Option<usize>,
    /// LLM model.
    #[serde(default)]
    pub model: Option<String>,
    /// User-defined rules text.
    #[serde(default)]
    pub rules_text: Option<String>,
    /// Whether to dispatch to federation peers.
    #[serde(default)]
    pub dispatch_to_peers: Option<bool>,
    /// Whether to log denied proposals.
    #[serde(default)]
    pub log_denied_proposals: Option<bool>,
}

/// Response for `GET /api/v1/operator/config`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorConfigResponse {
    pub config: DeskOperatorConfig,
}

/// Response for `POST /api/v1/operator/start` and `POST /api/v1/operator/stop`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorControlResponse {
    pub status: OperatorStatus,
}

/// Response for `GET /api/v1/operator/activity`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorActivityResponse {
    pub entries: Vec<OperatorActivityEntry>,
}

/// `GET /api/v1/operator/status`
pub async fn get_operator_status(State(state): State<AppState>) -> Response {
    let config = state.operator_loop.config().await;
    let status = state.operator_loop.status().await;
    let cycle_count = state.operator_loop.cycle_count().await;
    let activity_log_count = state.operator_loop.activity_log().len().await;

    Json(OperatorStatusResponse {
        status,
        enabled: config.enabled,
        autopilot_mode: config.autopilot_mode,
        action_scope: config.action_scope,
        loop_interval_secs: config.loop_interval_secs,
        model: config.model,
        cycle_count,
        activity_log_count,
    })
    .into_response()
}

/// `GET /api/v1/operator/config`
pub async fn get_operator_config(State(state): State<AppState>) -> Response {
    let config = state.operator_loop.config().await;
    Json(OperatorConfigResponse { config }).into_response()
}

/// `PUT /api/v1/operator/config`
pub async fn update_operator_config(
    State(state): State<AppState>,
    Json(req): Json<UpdateOperatorConfigRequest>,
) -> Response {
    let mut current = state.operator_loop.config().await;
    if let Some(enabled) = req.enabled {
        current.enabled = enabled;
    }
    if let Some(mode) = req.autopilot_mode {
        current.autopilot_mode = mode;
    }
    if let Some(scope) = req.action_scope {
        current.action_scope = scope;
    }
    if let Some(interval) = req.loop_interval_secs {
        current.loop_interval_secs = interval;
    }
    if let Some(max_actions) = req.max_actions_per_cycle {
        current.max_actions_per_cycle = max_actions;
    }
    if let Some(max_ctx) = req.max_context_items {
        current.max_context_items = max_ctx;
    }
    if let Some(model) = req.model {
        current.model = model;
    }
    if let Some(rules) = req.rules_text {
        current.rules_text = rules;
    }
    if let Some(dispatch) = req.dispatch_to_peers {
        current.dispatch_to_peers = dispatch;
    }
    if let Some(log_denied) = req.log_denied_proposals {
        current.log_denied_proposals = log_denied;
    }

    if let Err(e) = state.operator_loop.set_config(current.clone()).await {
        return api_error_response(HelixError::ValidationError {
            context: "config".to_string(),
            message: e.to_string(),
        });
    }

    Json(OperatorConfigResponse { config: current }).into_response()
}

fn spawn_operator_task(
    operator_loop: Arc<helix_operator::OperatorLoop>,
    provider: Arc<dyn helix_llm::providers::LlmProvider>,
    intel_desk: Arc<tokio::sync::RwLock<crate::intel::IntelDeskStore>>,
) -> tokio::task::JoinHandle<()> {
    let activity_log = operator_loop.activity_log().clone();

    tokio::spawn(async move {
        tracing::info!("operator loop task started");
        loop {
            let status = operator_loop.status().await;
            if status != OperatorStatus::Running {
                tracing::info!("operator loop task stopping (status={:?})", status);
                break;
            }

            let config = operator_loop.config().await;
            let recent_activity = activity_log.recent(5).await;
            let snapshot = {
                let desk = intel_desk.read().await;
                build_desk_snapshot(&desk, config.max_context_items, recent_activity)
            };
            let context = ApiDeskContext { snapshot };

            match operator_loop.run_cycle(&context, provider.as_ref()).await {
                Ok(response) => {
                    tracing::info!(
                        proposals = response.proposals.len(),
                        allowed = response.allowed_count,
                        denied = response.denied_count,
                        "operator cycle completed"
                    );
                }
                Err(e) => {
                    tracing::warn!("operator cycle error: {}", e);
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(
                config.loop_interval_secs.max(5),
            ))
            .await;
        }
        tracing::info!("operator loop task exited");
    })
}

/// `POST /api/v1/operator/start`
pub async fn start_operator(State(state): State<AppState>) -> Response {
    let Some(provider) = state.llm_provider.as_ref().map(Arc::clone) else {
        return api_error_response(HelixError::ValidationError {
            context: "llm_provider".to_string(),
            message: "operator requires a configured LLM provider".to_string(),
        });
    };

    let mut task = state.operator_task.lock().await;
    if state.operator_loop.status().await != OperatorStatus::Running {
        if let Some(handle) = task.take() {
            handle.abort();
        }
    }

    match state.operator_loop.start().await {
        Ok(()) => {
            let operator_loop = Arc::clone(&state.operator_loop);
            let intel_desk = Arc::clone(&state.intel_desk);
            *task = Some(spawn_operator_task(operator_loop, provider, intel_desk));

            let status = state.operator_loop.status().await;
            Json(OperatorControlResponse { status }).into_response()
        }
        Err(e) => api_error_response(HelixError::ValidationError {
            context: "status".to_string(),
            message: e.to_string(),
        }),
    }
}

/// `POST /api/v1/operator/stop`
pub async fn stop_operator(State(state): State<AppState>) -> Response {
    match state.operator_loop.stop().await {
        Ok(()) => {
            if let Some(handle) = state.operator_task.lock().await.take() {
                handle.abort();
            }
            let status = state.operator_loop.status().await;
            Json(OperatorControlResponse { status }).into_response()
        }
        Err(e) => api_error_response(HelixError::ValidationError {
            context: "status".to_string(),
            message: e.to_string(),
        }),
    }
}

/// `POST /api/v1/operator/pause`
pub async fn pause_operator(State(state): State<AppState>) -> Response {
    match state.operator_loop.pause().await {
        Ok(()) => {
            if let Some(handle) = state.operator_task.lock().await.take() {
                handle.abort();
            }
            let status = state.operator_loop.status().await;
            Json(OperatorControlResponse { status }).into_response()
        }
        Err(e) => api_error_response(HelixError::ValidationError {
            context: "status".to_string(),
            message: e.to_string(),
        }),
    }
}

/// `GET /api/v1/operator/activity`
pub async fn get_operator_activity(
    State(state): State<AppState>,
    Query(query): Query<ActivityLogQuery>,
) -> Response {
    let limit = query.limit.unwrap_or(50).min(500);
    let entries = state.operator_loop.activity_log().recent(limit).await;
    Json(OperatorActivityResponse { entries }).into_response()
}

/// `DELETE /api/v1/operator/activity`
///
/// Clears the activity log. Implemented by replacing the loop with a fresh
/// one — but since the loop is shared via Arc, we instead just drain via
/// reading all and note that the log is bounded and will evict naturally.
/// For now, this returns 200 with the current count (no-op clear).
pub async fn clear_operator_activity(State(state): State<AppState>) -> Response {
    // The activity log is bounded and self-evicting; we expose a no-op
    // clear endpoint for forward compatibility.
    let count = state.operator_loop.activity_log().len().await;
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "cleared": false,
            "current_count": count,
            "note": "Activity log is bounded and self-evicting; manual clear not yet implemented."
        })),
    )
        .into_response()
}

// ---- CoPilot mode: sessions, confirmations, SSE stream ----

use helix_operator::{
    CollaborationEvent, ConfirmationRequest, JoinSessionRequest,
    OperatorSession,
};

/// Response for `POST /api/v1/operator/sessions` (join).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinSessionResponse {
    pub session: OperatorSession,
}

/// Response for `GET /api/v1/operator/sessions`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionListResponse {
    pub sessions: Vec<OperatorSession>,
    pub human_count: usize,
    pub ai_count: usize,
}

/// Response for `GET /api/v1/operator/confirmations`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmationListResponse {
    pub pending: Vec<ConfirmationRequest>,
    pub recent: Vec<ConfirmationRequest>,
}

/// Request body for confirming/denying a confirmation.
#[derive(Debug, Clone, Deserialize)]
pub struct ResolveConfirmationRequest {
    /// Session id of the human resolving this request.
    pub session_id: String,
    /// Denial reason (only used for deny).
    #[serde(default)]
    pub denial_reason: Option<String>,
}

/// Response for confirm/deny endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveConfirmationResponse {
    pub request: ConfirmationRequest,
}

/// `POST /api/v1/operator/sessions` — join the desk as a CoPilot participant.
pub async fn join_session(
    State(state): State<AppState>,
    Json(req): Json<JoinSessionRequest>,
) -> Response {
    if req.display_name.trim().is_empty() {
        return api_error_response(HelixError::ValidationError {
            context: "display_name".to_string(),
            message: "display_name must not be empty".to_string(),
        });
    }
    let session = OperatorSession::new(
        req.display_name,
        req.kind,
        req.role,
        req.location,
    );
    let _id = state.operator_registry.add(session.clone()).await;
    state.operator_loop.broadcaster().broadcast(
        CollaborationEvent::session_joined(session.clone()),
    );
    Json(JoinSessionResponse { session }).into_response()
}

/// `DELETE /api/v1/operator/sessions/:session_id` — leave the desk.
pub async fn leave_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    let removed = state.operator_registry.remove(&session_id).await;
    if let Some(ref session) = removed {
        state.operator_loop.broadcaster().broadcast(
            CollaborationEvent::session_left(session.id.clone(), session.display_name.clone()),
        );
        Json(serde_json::json!({ "removed": true, "session_id": session_id }))
            .into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "session not found" })),
        )
            .into_response()
    }
}

/// `GET /api/v1/operator/sessions` — list all active CoPilot participants.
pub async fn list_sessions(State(state): State<AppState>) -> Response {
    let sessions = state.operator_registry.list().await;
    let (human_count, ai_count) = state.operator_registry.counts_by_kind().await;
    Json(SessionListResponse {
        sessions,
        human_count,
        ai_count,
    })
    .into_response()
}

/// `POST /api/v1/operator/sessions/:session_id/heartbeat` — keep session alive.
pub async fn session_heartbeat(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    if state.operator_registry.heartbeat(&session_id).await {
        state.operator_loop.broadcaster().broadcast(
            CollaborationEvent::heartbeat(session_id),
        );
        Json(serde_json::json!({ "ok": true })).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "session not found" })),
        )
            .into_response()
    }
}

/// `GET /api/v1/operator/confirmations` — list pending and recent confirmations.
pub async fn list_confirmations(State(state): State<AppState>) -> Response {
    let pending = state.operator_loop.confirmation_queue().pending().await;
    let recent = state.operator_loop.confirmation_queue().all().await;
    Json(ConfirmationListResponse { pending, recent }).into_response()
}

/// `POST /api/v1/operator/confirmations/:id/confirm` — confirm a pending AI proposal.
pub async fn confirm_proposal(
    State(state): State<AppState>,
    Path(confirmation_id): Path<String>,
    Json(req): Json<ResolveConfirmationRequest>,
) -> Response {
    // Look up the session to get the resolver's name and verify permissions
    let session = state.operator_registry.get(&req.session_id).await;
    let session = match session {
        Some(s) => s,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "session not found" })),
            )
                .into_response();
        }
    };
    if !session.role.can_confirm() {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "insufficient permissions" })),
        )
            .into_response();
    }
    let resolved = state
        .operator_loop
        .confirmation_queue()
        .confirm(&confirmation_id, &session.id, &session.display_name)
        .await;
    match resolved {
        Some(request) => {
            state.operator_loop.broadcaster().broadcast(
                CollaborationEvent::confirmation_resolved(request.clone()),
            );
            Json(ResolveConfirmationResponse { request }).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "confirmation not found or already resolved" })),
        )
            .into_response(),
    }
}

/// `POST /api/v1/operator/confirmations/:id/deny` — deny a pending AI proposal.
pub async fn deny_proposal(
    State(state): State<AppState>,
    Path(confirmation_id): Path<String>,
    Json(req): Json<ResolveConfirmationRequest>,
) -> Response {
    let session = state.operator_registry.get(&req.session_id).await;
    let session = match session {
        Some(s) => s,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "session not found" })),
            )
                .into_response();
        }
    };
    if !session.role.can_confirm() {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "insufficient permissions" })),
        )
            .into_response();
    }
    let resolved = state
        .operator_loop
        .confirmation_queue()
        .deny(&confirmation_id, &session.id, &session.display_name, req.denial_reason)
        .await;
    match resolved {
        Some(request) => {
            state.operator_loop.broadcaster().broadcast(
                CollaborationEvent::confirmation_resolved(request.clone()),
            );
            Json(ResolveConfirmationResponse { request }).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "confirmation not found or already resolved" })),
        )
            .into_response(),
    }
}

/// `GET /api/v1/operator/stream` — SSE stream of collaboration events.
pub async fn operator_stream(State(state): State<AppState>) -> Response {
    use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};

    let rx = state.operator_loop.broadcaster().subscribe();
    let stream = futures::stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Ok(event) => {
                let json = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string());
                let kind = match &event {
                    CollaborationEvent::SessionJoined { .. } => "session_joined",
                    CollaborationEvent::SessionLeft { .. } => "session_left",
                    CollaborationEvent::Heartbeat { .. } => "heartbeat",
                    CollaborationEvent::LoopStarted { .. } => "loop_started",
                    CollaborationEvent::LoopStopped { .. } => "loop_stopped",
                    CollaborationEvent::LoopPaused { .. } => "loop_paused",
                    CollaborationEvent::CycleCompleted { .. } => "cycle_completed",
                    CollaborationEvent::ConfirmationRequested { .. } => "confirmation_requested",
                    CollaborationEvent::ConfirmationResolved { .. } => "confirmation_resolved",
                    CollaborationEvent::OperatorDecision { .. } => "operator_decision",
                    CollaborationEvent::ConfigChanged { .. } => "config_changed",
                    CollaborationEvent::SessionPruned { .. } => "session_pruned",
                };
                Some((
                    Ok::<SseEvent, std::convert::Infallible>(SseEvent::default().event(kind).data(json)),
                    rx,
                ))
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => None,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                // Skip missed events and continue
                Some((
                    Ok::<SseEvent, std::convert::Infallible>(SseEvent::default().comment("missed events")),
                    rx,
                ))
            }
        }
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keep-alive"),
    )
    .into_response()
}

#[cfg(test)]
mod tests {
    // Integration tests live in `main.rs`'s `#[cfg(test)] mod tests` block,
    // where `default_app_state` and `app` are accessible. This empty module
    // exists so that `cargo test -p helix-api` continues to discover the
    // crate's test target without warnings about a missing tests module.
}

// Integration tests live in `main.rs`'s `#[cfg(test)] mod tests` block, where
// `default_app_state` and `app` are accessible.
