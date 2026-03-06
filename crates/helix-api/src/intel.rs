use crate::{api_error_response, AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use helix_core::intel_desk::{
    canonicalize_claims, canonicalize_evidence, canonicalize_source, canonicalize_watchlist,
    evaluate_watchlists, new_case, transition_case, CaseCommand, CaseDecision, CaseFile,
    CaseStatus, CaseTransition, ClaimRecord, ClaimReviewStatus, EvidenceDraft, EvidenceItem,
    ProposedClaim, SourceDefinition, SourceKind, Watchlist, WatchlistHit, WatchlistSeverity,
};
use helix_core::HelixError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct IntelDeskOverviewResponse {
    pub(crate) source_count: usize,
    pub(crate) watchlist_count: usize,
    pub(crate) evidence_count: usize,
    pub(crate) claim_count: usize,
    pub(crate) open_case_count: usize,
    pub(crate) escalated_case_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SourceCatalogResponse {
    pub(crate) sources: Vec<SourceDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CreateSourceRequest {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) kind: SourceKind,
    pub(crate) cadence_minutes: u16,
    pub(crate) trust_score: u8,
    pub(crate) enabled: bool,
    pub(crate) tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SourceResponse {
    pub(crate) source: SourceDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WatchlistCatalogResponse {
    pub(crate) watchlists: Vec<Watchlist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CreateWatchlistRequest {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) keywords: Vec<String>,
    pub(crate) entities: Vec<String>,
    pub(crate) min_source_trust: u8,
    pub(crate) severity: WatchlistSeverity,
    pub(crate) enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WatchlistResponse {
    pub(crate) watchlist: Watchlist,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvidenceCatalogResponse {
    pub(crate) evidence: Vec<EvidenceItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ClaimCatalogResponse {
    pub(crate) claims: Vec<ClaimRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ClaimReviewRequest {
    pub(crate) status: ClaimReviewStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ClaimResponse {
    pub(crate) claim: ClaimRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CaseCatalogResponse {
    pub(crate) cases: Vec<CaseFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct IngestEvidenceRequest {
    pub(crate) source_id: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) content: String,
    pub(crate) url: Option<String>,
    pub(crate) observed_at: String,
    pub(crate) tags: Vec<String>,
    pub(crate) entity_labels: Vec<String>,
    pub(crate) proposed_claims: Vec<ProposedClaim>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct IngestEvidenceResponse {
    pub(crate) duplicate: bool,
    pub(crate) evidence: EvidenceItem,
    pub(crate) claims: Vec<ClaimRecord>,
    pub(crate) hits: Vec<WatchlistHit>,
    pub(crate) case_updates: Vec<CaseTransition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CaseTransitionRequest {
    pub(crate) command: CaseCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CaseTransitionResponse {
    pub(crate) transition: CaseTransition,
}

#[derive(Debug, Clone)]
pub(crate) struct IntelDeskStore {
    sources: BTreeMap<String, SourceDefinition>,
    watchlists: BTreeMap<String, Watchlist>,
    evidence: BTreeMap<String, EvidenceItem>,
    claims: BTreeMap<String, ClaimRecord>,
    cases: BTreeMap<String, CaseFile>,
}

impl Default for IntelDeskStore {
    fn default() -> Self {
        Self::seeded()
    }
}

impl IntelDeskStore {
    pub(crate) fn seeded() -> Self {
        let mut store = Self {
            sources: BTreeMap::new(),
            watchlists: BTreeMap::new(),
            evidence: BTreeMap::new(),
            claims: BTreeMap::new(),
            cases: BTreeMap::new(),
        };

        let sources = [
            SourceDefinition {
                id: "rss_national_security".to_string(),
                name: "National Security RSS".to_string(),
                description: "Trusted feed for security, diplomacy, and leadership movements."
                    .to_string(),
                kind: SourceKind::RssFeed,
                cadence_minutes: 30,
                trust_score: 88,
                enabled: true,
                tags: vec!["osint".to_string(), "security".to_string()],
            },
            SourceDefinition {
                id: "website_orion_dynamics".to_string(),
                name: "Orion Dynamics Website Diff".to_string(),
                description: "Website diff watcher for executive, facilities, and product signals."
                    .to_string(),
                kind: SourceKind::WebsiteDiff,
                cadence_minutes: 120,
                trust_score: 76,
                enabled: true,
                tags: vec!["company".to_string(), "website".to_string()],
            },
        ];
        for source in sources {
            let source = canonicalize_source(source).expect("seed source should be valid");
            store.sources.insert(source.id.clone(), source);
        }

        let watchlists = [
            Watchlist {
                id: "watch_exec_moves".to_string(),
                name: "Executive Movements".to_string(),
                description: "Track executive changes, detentions, and resignations.".to_string(),
                keywords: vec![
                    "resigned".to_string(),
                    "detained".to_string(),
                    "appointed".to_string(),
                ],
                entities: vec!["alice north".to_string(), "orion dynamics".to_string()],
                min_source_trust: 60,
                severity: WatchlistSeverity::High,
                enabled: true,
            },
            Watchlist {
                id: "watch_site_activity".to_string(),
                name: "Facility Activity".to_string(),
                description: "Monitor facility openings, closures, and unusual movements."
                    .to_string(),
                keywords: vec![
                    "facility".to_string(),
                    "warehouse".to_string(),
                    "explosion".to_string(),
                ],
                entities: vec!["orion dynamics".to_string(), "north harbor".to_string()],
                min_source_trust: 50,
                severity: WatchlistSeverity::Critical,
                enabled: true,
            },
        ];
        for watchlist in watchlists {
            let watchlist = canonicalize_watchlist(watchlist).expect("seed watchlist should be valid");
            store.watchlists.insert(watchlist.id.clone(), watchlist);
        }

        store
    }

    fn overview(&self) -> IntelDeskOverviewResponse {
        IntelDeskOverviewResponse {
            source_count: self.sources.len(),
            watchlist_count: self.watchlists.len(),
            evidence_count: self.evidence.len(),
            claim_count: self.claims.len(),
            open_case_count: self
                .cases
                .values()
                .filter(|case| {
                    matches!(
                        case.status,
                        CaseStatus::Open | CaseStatus::Monitoring | CaseStatus::BriefReady
                    )
                })
                .count(),
            escalated_case_count: self
                .cases
                .values()
                .filter(|case| case.status == CaseStatus::Escalated)
                .count(),
        }
    }

    fn create_source(&mut self, request: CreateSourceRequest) -> Result<SourceDefinition, HelixError> {
        let source = canonicalize_source(SourceDefinition {
            id: slugify(&request.name),
            name: request.name,
            description: request.description,
            kind: request.kind,
            cadence_minutes: request.cadence_minutes,
            trust_score: request.trust_score,
            enabled: request.enabled,
            tags: request.tags,
        })?;
        self.sources.insert(source.id.clone(), source.clone());
        Ok(source)
    }

    fn create_watchlist(
        &mut self,
        request: CreateWatchlistRequest,
    ) -> Result<Watchlist, HelixError> {
        let watchlist = canonicalize_watchlist(Watchlist {
            id: slugify(&request.name),
            name: request.name,
            description: request.description,
            keywords: request.keywords,
            entities: request.entities,
            min_source_trust: request.min_source_trust,
            severity: request.severity,
            enabled: request.enabled,
        })?;
        self.watchlists.insert(watchlist.id.clone(), watchlist.clone());
        Ok(watchlist)
    }

    fn ingest_evidence(
        &mut self,
        request: IngestEvidenceRequest,
    ) -> Result<IngestEvidenceResponse, HelixError> {
        let source = self
            .sources
            .get(&request.source_id)
            .cloned()
            .ok_or_else(|| HelixError::not_found(format!("source {}", request.source_id)))?;
        if !source.enabled {
            return Err(HelixError::validation_error(
                "source",
                "source is disabled",
            ));
        }

        let evidence_id = stable_id(
            "evidence",
            &[
                &request.source_id,
                &request.title,
                &request.summary,
                &request.observed_at,
                request.url.as_deref().unwrap_or(""),
            ],
        );
        if let Some(existing) = self.evidence.get(&evidence_id).cloned() {
            let claims = self.claims_for_evidence(&existing.id);
            let hits = evaluate_watchlists(
                &source,
                &existing,
                &claims,
                &self.watchlists.values().cloned().collect::<Vec<_>>(),
            );
            return Ok(IngestEvidenceResponse {
                duplicate: true,
                evidence: existing,
                claims,
                hits,
                case_updates: Vec::new(),
            });
        }

        let provenance_hash = provenance_hash(&request);
        let proposed_claims = request.proposed_claims.clone();
        let evidence = canonicalize_evidence(
            evidence_id,
            provenance_hash,
            EvidenceDraft {
                source_id: request.source_id,
                title: request.title,
                summary: request.summary,
                content: request.content,
                url: request.url,
                observed_at: request.observed_at,
                tags: request.tags,
                entity_labels: request.entity_labels,
                proposed_claims: request.proposed_claims,
            },
        )?;
        let claim_drafts = canonicalize_claims(&evidence, proposed_claims)?;
        let claims = claim_drafts
            .into_iter()
            .map(|claim| self.materialize_claim(&evidence, claim))
            .collect::<Vec<_>>();
        let hits = evaluate_watchlists(
            &source,
            &evidence,
            &claims,
            &self.watchlists.values().cloned().collect::<Vec<_>>(),
        );

        self.evidence.insert(evidence.id.clone(), evidence.clone());
        for claim in &claims {
            self.claims.insert(claim.id.clone(), claim.clone());
        }

        let case_updates = self.apply_watchlist_hits(&evidence, &claims, &hits)?;

        Ok(IngestEvidenceResponse {
            duplicate: false,
            evidence,
            claims,
            hits,
            case_updates,
        })
    }

    fn transition_case(
        &mut self,
        case_id: &str,
        command: CaseCommand,
    ) -> Result<CaseTransition, HelixError> {
        let case = self
            .cases
            .get(case_id)
            .cloned()
            .ok_or_else(|| HelixError::not_found(format!("case {}", case_id)))?;
        let transition = transition_case(&case, command)?;
        self.cases
            .insert(transition.case.id.clone(), transition.case.clone());
        Ok(transition)
    }

    fn review_claim(
        &mut self,
        claim_id: &str,
        status: ClaimReviewStatus,
    ) -> Result<ClaimRecord, HelixError> {
        let claim = self
            .claims
            .get_mut(claim_id)
            .ok_or_else(|| HelixError::not_found(format!("claim {}", claim_id)))?;
        claim.review_status = status;
        Ok(claim.clone())
    }

    fn materialize_claim(&self, evidence: &EvidenceItem, proposed: ProposedClaim) -> ClaimRecord {
        ClaimRecord {
            id: stable_id(
                "claim",
                &[&evidence.id, &proposed.subject, &proposed.predicate, &proposed.object],
            ),
            evidence_id: evidence.id.clone(),
            subject: proposed.subject,
            predicate: proposed.predicate,
            object: proposed.object,
            confidence_bps: proposed.confidence_bps,
            review_status: ClaimReviewStatus::NeedsReview,
            rationale: proposed
                .rationale
                .unwrap_or_else(|| "operator_provided_or_deterministically_derived".to_string()),
        }
    }

    fn claims_for_evidence(&self, evidence_id: &str) -> Vec<ClaimRecord> {
        self.claims
            .values()
            .filter(|claim| claim.evidence_id == evidence_id)
            .cloned()
            .collect()
    }

    fn apply_watchlist_hits(
        &mut self,
        evidence: &EvidenceItem,
        claims: &[ClaimRecord],
        hits: &[WatchlistHit],
    ) -> Result<Vec<CaseTransition>, HelixError> {
        let mut transitions = Vec::new();

        for hit in hits {
            let primary_entity = hit
                .matched_entities
                .first()
                .cloned()
                .or_else(|| evidence.entity_labels.first().cloned());
            let claim_ids = claims.iter().map(|claim| claim.id.clone()).collect::<Vec<_>>();

            let existing_case_id = self
                .cases
                .values()
                .find(|case| {
                    case.status != CaseStatus::Closed
                        && case.watchlist_id == hit.watchlist_id
                        && case.primary_entity == primary_entity
                })
                .map(|case| case.id.clone());

            let transition = if let Some(case_id) = existing_case_id {
                let case = self
                    .cases
                    .get(&case_id)
                    .cloned()
                    .ok_or_else(|| HelixError::internal_error("case missing during update"))?;
                let updated = transition_case(
                    &case,
                    CaseCommand::AppendEvidence {
                        evidence_id: evidence.id.clone(),
                        claim_ids: claim_ids.clone(),
                        reason: hit.reason.clone(),
                    },
                )?;
                maybe_escalate_case(updated, hit)?
            } else {
                let opened = new_case(CaseCommand::Open {
                    case_id: stable_id(
                        "case",
                        &[
                            &hit.watchlist_id,
                            primary_entity.as_deref().unwrap_or(evidence.id.as_str()),
                        ],
                    ),
                    title: build_case_title(hit, evidence, primary_entity.as_deref()),
                    watchlist_id: hit.watchlist_id.clone(),
                    primary_entity,
                    evidence_id: evidence.id.clone(),
                    claim_ids: claim_ids.clone(),
                    reason: hit.reason.clone(),
                })?;
                maybe_escalate_case(opened, hit)?
            };

            self.cases
                .insert(transition.case.id.clone(), transition.case.clone());
            transitions.push(transition);
        }

        transitions.sort_by(|left, right| left.case.id.cmp(&right.case.id));
        Ok(transitions)
    }
}

fn build_case_title(hit: &WatchlistHit, evidence: &EvidenceItem, primary_entity: Option<&str>) -> String {
    match primary_entity {
        Some(entity) => format!("{}: {}", hit.watchlist_name, entity),
        None => format!("{}: {}", hit.watchlist_name, evidence.title),
    }
}

fn maybe_escalate_case(
    transition: CaseTransition,
    hit: &WatchlistHit,
) -> Result<CaseTransition, HelixError> {
    if hit.severity.weight() < WatchlistSeverity::High.weight() {
        return Ok(transition);
    }

    match transition.decision {
        CaseDecision::Denied { .. } => Ok(transition),
        _ => transition_case(
            &transition.case,
            CaseCommand::Escalate {
                reason: format!("{}:{}", hit.watchlist_name, hit.reason),
            },
        ),
    }
}

fn provenance_hash(request: &IngestEvidenceRequest) -> String {
    stable_hash(&[
        &request.source_id,
        &request.title,
        &request.summary,
        &request.content,
        &request.observed_at,
        request.url.as_deref().unwrap_or(""),
    ])
}

fn stable_id(prefix: &str, parts: &[&str]) -> String {
    format!("{}_{}", prefix, &stable_hash(parts)[..12])
}

fn stable_hash(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0x1f]);
    }
    format!("{:x}", hasher.finalize())
}

fn slugify(input: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in input.trim().chars() {
        let next = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '-'
        };
        if next == '-' {
            if prev_dash {
                continue;
            }
            prev_dash = true;
        } else {
            prev_dash = false;
        }
        slug.push(next);
    }
    slug.trim_matches('-').to_string()
}

pub(crate) async fn get_intel_overview(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.intel_desk.read().await;
    (StatusCode::OK, Json(store.overview()))
}

pub(crate) async fn list_sources(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.intel_desk.read().await;
    (
        StatusCode::OK,
        Json(SourceCatalogResponse {
            sources: store.sources.values().cloned().collect(),
        }),
    )
}

pub(crate) async fn create_source(
    State(state): State<AppState>,
    Json(request): Json<CreateSourceRequest>,
) -> Response {
    let result = state.intel_desk.write().await.create_source(request);
    match result {
        Ok(source) => (StatusCode::CREATED, Json(SourceResponse { source })).into_response(),
        Err(error) => api_error_response(error),
    }
}

pub(crate) async fn list_watchlists(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.intel_desk.read().await;
    (
        StatusCode::OK,
        Json(WatchlistCatalogResponse {
            watchlists: store.watchlists.values().cloned().collect(),
        }),
    )
}

pub(crate) async fn create_watchlist(
    State(state): State<AppState>,
    Json(request): Json<CreateWatchlistRequest>,
) -> Response {
    let result = state.intel_desk.write().await.create_watchlist(request);
    match result {
        Ok(watchlist) => (StatusCode::CREATED, Json(WatchlistResponse { watchlist })).into_response(),
        Err(error) => api_error_response(error),
    }
}

pub(crate) async fn list_evidence(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.intel_desk.read().await;
    (
        StatusCode::OK,
        Json(EvidenceCatalogResponse {
            evidence: store.evidence.values().cloned().collect(),
        }),
    )
}

pub(crate) async fn list_claims(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.intel_desk.read().await;
    (
        StatusCode::OK,
        Json(ClaimCatalogResponse {
            claims: store.claims.values().cloned().collect(),
        }),
    )
}

pub(crate) async fn review_claim_handler(
    State(state): State<AppState>,
    Path(claim_id): Path<String>,
    Json(request): Json<ClaimReviewRequest>,
) -> Response {
    let result = state
        .intel_desk
        .write()
        .await
        .review_claim(&claim_id, request.status);
    match result {
        Ok(claim) => (StatusCode::OK, Json(ClaimResponse { claim })).into_response(),
        Err(error) => api_error_response(error),
    }
}

pub(crate) async fn ingest_evidence(
    State(state): State<AppState>,
    Json(request): Json<IngestEvidenceRequest>,
) -> Response {
    let result = state.intel_desk.write().await.ingest_evidence(request);
    match result {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(error) => api_error_response(error),
    }
}

pub(crate) async fn list_cases(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.intel_desk.read().await;
    (
        StatusCode::OK,
        Json(CaseCatalogResponse {
            cases: store.cases.values().cloned().collect(),
        }),
    )
}

pub(crate) async fn transition_case_handler(
    State(state): State<AppState>,
    Path(case_id): Path<String>,
    Json(request): Json<CaseTransitionRequest>,
) -> Response {
    let result = state
        .intel_desk
        .write()
        .await
        .transition_case(&case_id, request.command);
    match result {
        Ok(transition) => (StatusCode::OK, Json(CaseTransitionResponse { transition })).into_response(),
        Err(error) => api_error_response(error),
    }
}
