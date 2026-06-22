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
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use helix_core::autopilot_guard::AutopilotMode;
use helix_operator::{
    DeskOperatorConfig, OperatorActionScope, OperatorActivityEntry, OperatorStatus,
};
use serde::{Deserialize, Serialize};

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

/// `POST /api/v1/operator/start`
pub async fn start_operator(State(state): State<AppState>) -> Response {
    match state.operator_loop.start().await {
        Ok(()) => {
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

#[cfg(test)]
mod tests {
    // Integration tests live in `main.rs`'s `#[cfg(test)] mod tests` block,
    // where `default_app_state` and `app` are accessible. This empty module
    // exists so that `cargo test -p helix-api` continues to discover the
    // crate's test target without warnings about a missing tests module.
}

// Integration tests live in `main.rs`'s `#[cfg(test)] mod tests` block, where
// `default_app_state` and `app` are accessible.
