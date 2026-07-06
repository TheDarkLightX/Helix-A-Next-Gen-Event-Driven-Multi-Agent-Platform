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

//! Federation API handlers: peer CRUD, dispatch log, inbound receive,
//! and federation status.

use crate::intel::IngestEvidenceRequest;
use crate::{api_error_response, AppState, HelixError};
use helix_core::intel_desk::{SourceDefinition, SourceKind};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use helix_federation::{
    compute_overview, DispatchLogEntry, FederationEvent, FederationEventKind,
    OutboundDispatcher, PeerDesk, PeerRegistryQuery,
};
use serde::{Deserialize, Serialize};

/// Query params for listing dispatch log entries.
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct DispatchLogQuery {
    pub(crate) limit: Option<usize>,
}

/// Request to create or update a peer desk.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct UpsertPeerRequest {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) endpoint_url: String,
    pub(crate) auth_token: String,
    pub(crate) trust_score: u8,
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
}

/// Response for a single peer.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct PeerResponse {
    pub(crate) peer: PeerDesk,
}

/// Response for listing peers.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct PeerListResponse {
    pub(crate) peers: Vec<PeerDesk>,
}

/// Response for the dispatch log.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct DispatchLogResponse {
    pub(crate) entries: Vec<DispatchLogEntry>,
}

/// Response for the inbound receive endpoint.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct FederationReceiveResponse {
    pub(crate) accepted: bool,
    pub(crate) evidence_id: Option<String>,
    pub(crate) message: String,
}

/// Request to manually broadcast an event to peers.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ManualBroadcastRequest {
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) content: String,
    #[serde(default)]
    pub(crate) url: Option<String>,
    #[serde(default)]
    pub(crate) entity_labels: Vec<String>,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
}

/// Response for a manual broadcast.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ManualBroadcastResponse {
    pub(crate) event_id: String,
    pub(crate) dispatch_count: usize,
    pub(crate) results: Vec<DispatchLogEntry>,
}

// --- Handlers ---

pub(crate) async fn get_federation_overview(State(state): State<AppState>) -> Response {
    let desk_id = state.federation_desk_id.clone();
    let overview = compute_overview(&desk_id, &state.federation_registry, &state.federation_log).await;
    (StatusCode::OK, Json(overview)).into_response()
}

pub(crate) async fn list_peers(
    State(state): State<AppState>,
    Query(query): Query<PeerRegistryQuery>,
) -> Response {
    let peers = state.federation_registry.list(&query).await;
    (StatusCode::OK, Json(PeerListResponse { peers })).into_response()
}

pub(crate) async fn get_peer(
    State(state): State<AppState>,
    Path(peer_id): Path<String>,
) -> Response {
    match state.federation_registry.get(&peer_id).await {
        Some(peer) => (StatusCode::OK, Json(PeerResponse { peer })).into_response(),
        None => api_error_response(HelixError::not_found(format!("peer {peer_id}"))),
    }
}

pub(crate) async fn upsert_peer(
    State(state): State<AppState>,
    Json(request): Json<UpsertPeerRequest>,
) -> Response {
    let now = chrono::Utc::now().to_rfc3339();
    let existing = state.federation_registry.get(&request.id).await;
    let peer = PeerDesk {
        id: request.id,
        name: request.name,
        endpoint_url: request.endpoint_url,
        auth_token: request.auth_token,
        trust_score: request.trust_score,
        enabled: request.enabled,
        tags: request.tags,
        created_at: existing
            .as_ref()
            .map(|p| p.created_at.clone())
            .unwrap_or_else(|| now.clone()),
        updated_at: now,
    };

    match state.federation_registry.upsert(peer.clone()).await {
        Ok(saved) => {
            let status = if existing.is_some() {
                StatusCode::OK
            } else {
                StatusCode::CREATED
            };
            (status, Json(PeerResponse { peer: saved })).into_response()
        }
        Err(err) => api_error_response(HelixError::validation_error("peer", &err.to_string())),
    }
}

pub(crate) async fn delete_peer(
    State(state): State<AppState>,
    Path(peer_id): Path<String>,
) -> Response {
    match state.federation_registry.remove(&peer_id).await {
        Ok(peer) => (StatusCode::OK, Json(PeerResponse { peer })).into_response(),
        Err(err) => api_error_response(HelixError::not_found(&err.to_string())),
    }
}

pub(crate) async fn get_dispatch_log(
    State(state): State<AppState>,
    Query(query): Query<DispatchLogQuery>,
) -> Response {
    let limit = query.limit.unwrap_or(50).min(500);
    let entries = state.federation_log.recent(limit).await;
    (StatusCode::OK, Json(DispatchLogResponse { entries })).into_response()
}

pub(crate) async fn manual_broadcast(
    State(state): State<AppState>,
    Json(request): Json<ManualBroadcastRequest>,
) -> Response {
    let desk_id = state.federation_desk_id.clone();
    let mut event = FederationEvent::new(
        &desk_id,
        FederationEventKind::ManualBroadcast,
        &request.title,
        &request.summary,
        &request.content,
        80,
    );
    event.url = request.url;
    event.entity_labels = request.entity_labels;
    event.tags = request.tags;

    let dispatcher = OutboundDispatcher::new(
        desk_id,
        state.federation_registry.clone(),
        state.federation_log.clone(),
    );
    let results = dispatcher.dispatch(&event).await;

    (
        StatusCode::OK,
        Json(ManualBroadcastResponse {
            event_id: event.id,
            dispatch_count: results.len(),
            results,
        }),
    )
        .into_response()
}

/// Inbound federation receive endpoint.
///
/// Accepts a [`FederationEvent`] from a peer desk and ingests it as evidence
/// with provenance pointing back to the source desk. A federation source is
/// auto-registered if it does not already exist.
pub(crate) async fn receive_federation_event(
    State(state): State<AppState>,
    Json(event): Json<FederationEvent>,
) -> Response {
    let source_id = format!("federation-{}", event.source_desk_id);

    // Auto-register a federation source if it doesn't exist
    {
        let store = state.intel_desk.read().await;
        if !store.has_source(&source_id) {
            drop(store);
            let federation_source = SourceDefinition {
                id: source_id.clone(),
                profile_id: "50000000-0000-0000-0000-000000000010".to_string(),
                name: format!("Federation: {}", event.source_desk_id),
                description: format!(
                    "Inbound intelligence from peer desk {} (trust: {})",
                    event.source_desk_id, event.trust_score
                ),
                kind: SourceKind::WebhookIngest,
                endpoint_url: None,
                credential_id: None,
                credential_header_name: "Authorization".to_string(),
                credential_header_prefix: Some("Bearer".to_string()),
                cadence_minutes: 0,
                trust_score: event.trust_score,
                enabled: true,
                tags: vec!["federation".to_string()],
            };
            let mut store = state.intel_desk.write().await;
            store.upsert_source(federation_source);
        }
    }

    // Build an evidence ingest request from the federation event
    let mut tags = event.tags.clone();
    tags.push(format!("federation-{}", event.source_desk_id));
    tags.push(event.event_type.clone());

    let ingest_request = IngestEvidenceRequest {
        source_id,
        title: event.title.clone(),
        summary: event.summary.clone(),
        content: event.content.clone(),
        url: event.url.clone(),
        observed_at: event.observed_at.clone(),
        tags,
        entity_labels: event.entity_labels.clone(),
        proposed_claims: vec![],
    };

    // Ingest the evidence through the standard pipeline
    let result = crate::intel::ingest_evidence_internal(&state, ingest_request).await;

    match result {
        Ok(response) => {
            // Record audit event
            let _ = crate::record_audit_event(
                &state,
                crate::AuditEvent::allow(
                    "federation.receive",
                    format!("evidence/{}", response.evidence.id),
                    serde_json::json!({
                        "source_desk_id": event.source_desk_id,
                        "event_id": event.id,
                        "event_type": event.event_type,
                        "evidence_id": response.evidence.id,
                        "duplicate": response.duplicate,
                        "trust_score": event.trust_score,
                    }),
                ),
            )
            .await;

            (
                StatusCode::CREATED,
                Json(FederationReceiveResponse {
                    accepted: true,
                    evidence_id: Some(response.evidence.id),
                    message: format!(
                        "Evidence ingested from desk {} (trust: {})",
                        event.source_desk_id, event.trust_score
                    ),
                }),
            )
                .into_response()
        }
        Err(error) => {
            let _ = crate::record_audit_event(
                &state,
                crate::AuditEvent::deny(
                    "federation.receive",
                    "federation/receive",
                    &error.to_string(),
                    serde_json::json!({
                        "source_desk_id": event.source_desk_id,
                        "event_id": event.id,
                        "error": error.to_string(),
                    }),
                ),
            )
            .await;
            api_error_response(error)
        }
    }
}

// --- Outbound dispatch hooks ---

/// Dispatches a federation event for a case escalation.
/// Called after a case transitions to a higher severity.
pub(crate) async fn dispatch_case_escalated(
    state: &AppState,
    case_id: &str,
    case_title: &str,
    case_summary: &str,
) {
    dispatch_event(
        state,
        FederationEventKind::CaseEscalated,
        &format!("Case escalated: {case_title}"),
        case_summary,
        &format!("Case {case_id} has been escalated."),
        Some(case_id),
        None,
    )
    .await;
}

/// Dispatches a federation event for a new case opening.
pub(crate) async fn dispatch_case_opened(
    state: &AppState,
    case_id: &str,
    case_title: &str,
    case_summary: &str,
) {
    dispatch_event(
        state,
        FederationEventKind::CaseOpened,
        &format!("Case opened: {case_title}"),
        case_summary,
        &format!("Case {case_id} has been opened."),
        Some(case_id),
        None,
    )
    .await;
}

/// Dispatches a federation event for a watchlist hit.
pub(crate) async fn dispatch_watchlist_hit(
    state: &AppState,
    watchlist_id: &str,
    evidence_title: &str,
    evidence_summary: &str,
) {
    dispatch_event(
        state,
        FederationEventKind::WatchlistHit,
        &format!("Watchlist hit: {watchlist_id}"),
        evidence_summary,
        &format!("Evidence '{evidence_title}' triggered watchlist {watchlist_id}."),
        None,
        Some(watchlist_id),
    )
    .await;
}

/// Dispatches a federation event for new evidence ingestion.
pub(crate) async fn dispatch_evidence_ingested(
    state: &AppState,
    evidence_title: &str,
    evidence_summary: &str,
    source_id: &str,
) {
    dispatch_event(
        state,
        FederationEventKind::EvidenceIngested,
        &format!("New evidence: {evidence_title}"),
        evidence_summary,
        &format!("Evidence ingested from source {source_id}."),
        None,
        None,
    )
    .await;
}

/// Core dispatch helper. Builds a FederationEvent and sends it to all enabled
/// peers via the outbound dispatcher. Failures are logged but never propagated
/// — federation is best-effort and must not block local operations.
async fn dispatch_event(
    state: &AppState,
    kind: FederationEventKind,
    title: &str,
    summary: &str,
    content: &str,
    case_id: Option<&str>,
    watchlist_id: Option<&str>,
) {
    // Skip if no enabled peers
    if state.federation_registry.enabled_count().await == 0 {
        return;
    }

    let mut event = FederationEvent::new(&state.federation_desk_id, kind, title, summary, content, 80);
    event.case_id = case_id.map(|s| s.to_string());
    event.watchlist_id = watchlist_id.map(|s| s.to_string());

    let dispatcher = OutboundDispatcher::new(
        state.federation_desk_id.clone(),
        state.federation_registry.clone(),
        state.federation_log.clone(),
    );
    let results = dispatcher.dispatch(&event).await;

    if !results.is_empty() {
        let delivered = results
            .iter()
            .filter(|r| r.status == helix_federation::DispatchStatus::Delivered)
            .count();
        let failed = results.len() - delivered;
        tracing::info!(
            event_id = %event.id,
            event_type = %event.event_type,
            delivered = delivered,
            failed = failed,
            "federation dispatch completed"
        );
    }
}
