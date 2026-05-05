use crate::{
    api_error_response, credential_encrypter_from_env, record_audit_event, AppState, AuditEvent,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use helix_core::intel_desk::{
    canonicalize_claims, canonicalize_evidence, canonicalize_source, canonicalize_watchlist,
    evaluate_watchlists, new_case, transition_case, CaseCommand, CaseDecision, CaseFile,
    CaseStatus, CaseTransition, ClaimRecord, ClaimReviewStatus, EvidenceDraft, EvidenceItem,
    ProposedClaim, SourceDefinition, SourceKind, Watchlist, WatchlistHit, WatchlistSeverity,
};
use helix_core::intel_priority::{
    score_case, score_claim, score_evidence, CasePriorityInput, ClaimPriorityInput,
    EvidencePriorityInput, IntelPriorityBreakdown, IntelSignalWindow,
};
use helix_core::market_intel::{
    score_market_company, score_market_theme, MarketCompanyPriorityInput, MarketSignalWindow,
    MarketThemePriorityInput,
};
use helix_core::types::{CredentialId, ProfileId};
use helix_core::HelixError;
use helix_embeddings::{cosine_similarity, EmbeddingGenerator};
use helix_security::encryption::CredentialEncrypterDecrypter;
use reqwest::header::{HeaderName, HeaderValue};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::collections::{BTreeMap, BTreeSet};

const MAX_COLLECT_ITEMS: usize = 50;
const MAX_SOURCE_FETCH_BYTES: usize = 1_048_576;
const MAX_COLLECT_CONTENT_LEN: usize = 16_384;
const MAX_SEMANTIC_QUERY_LEN: usize = 512;

#[derive(Debug, Clone)]
struct SourceFetchAuth {
    credential_id: String,
    header_name: HeaderName,
    header_value: HeaderValue,
}

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
pub(crate) struct MarketIntelThemeCard {
    pub(crate) theme_id: String,
    pub(crate) name: String,
    pub(crate) summary: String,
    pub(crate) priority: IntelPriorityBreakdown,
    pub(crate) watchlist_count: usize,
    pub(crate) evidence_count: usize,
    pub(crate) active_case_count: usize,
    pub(crate) escalated_case_count: usize,
    pub(crate) top_entities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MarketIntelCompanyCard {
    pub(crate) company: String,
    pub(crate) priority: IntelPriorityBreakdown,
    pub(crate) mention_count: usize,
    pub(crate) claim_count: usize,
    pub(crate) active_case_count: usize,
    pub(crate) themes: Vec<String>,
    pub(crate) latest_signal_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MarketIntelPlaybook {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) objective: String,
    pub(crate) signals: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MarketIntelOverviewResponse {
    pub(crate) market_source_count: usize,
    pub(crate) market_watchlist_count: usize,
    pub(crate) tracked_company_count: usize,
    pub(crate) active_case_count: usize,
    pub(crate) theme_cards: Vec<MarketIntelThemeCard>,
    pub(crate) company_cards: Vec<MarketIntelCompanyCard>,
    pub(crate) case_briefs: Vec<MarketIntelCaseBrief>,
    pub(crate) playbooks: Vec<MarketIntelPlaybook>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MarketIntelCaseBrief {
    pub(crate) case_id: String,
    pub(crate) title: String,
    pub(crate) company: Option<String>,
    pub(crate) theme_id: String,
    pub(crate) theme_name: String,
    pub(crate) priority: IntelPriorityBreakdown,
    pub(crate) status: CaseStatus,
    pub(crate) latest_signal_at: Option<String>,
    pub(crate) evidence_count: usize,
    pub(crate) claim_count: usize,
    pub(crate) attached_to_case: bool,
    pub(crate) summary: String,
    pub(crate) key_claims: Vec<String>,
    pub(crate) recommended_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GenerateMarketIntelBriefRequest {
    pub(crate) attach_to_case: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GenerateMarketIntelBriefResponse {
    pub(crate) briefing: MarketIntelCaseBrief,
    pub(crate) transition: Option<CaseTransition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SourceCatalogResponse {
    pub(crate) sources: Vec<SourceDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CreateSourceRequest {
    #[serde(default)]
    pub(crate) profile_id: Option<String>,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) kind: SourceKind,
    #[serde(default)]
    pub(crate) endpoint_url: Option<String>,
    #[serde(default)]
    pub(crate) credential_id: Option<String>,
    #[serde(default)]
    pub(crate) credential_header_name: Option<String>,
    #[serde(default)]
    pub(crate) credential_header_prefix: Option<String>,
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
pub(crate) struct CollectSourceRequest {
    pub(crate) observed_at: String,
    #[serde(default)]
    pub(crate) max_items: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CollectSourceResponse {
    pub(crate) source: SourceDefinition,
    pub(crate) fetched_url: String,
    pub(crate) collected_count: usize,
    pub(crate) duplicate_count: usize,
    pub(crate) results: Vec<IngestEvidenceResponse>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum JsonCollectionPayload {
    Envelope {
        items: Vec<CollectedEvidencePayload>,
    },
    Array(Vec<CollectedEvidencePayload>),
    Single(CollectedEvidencePayload),
}

#[derive(Debug, Clone, Default, Deserialize)]
struct CollectedEvidencePayload {
    title: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    observed_at: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    entity_labels: Vec<String>,
    #[serde(default)]
    proposed_claims: Vec<ProposedClaim>,
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
    pub(crate) evidence: Vec<EvidenceQueueEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ClaimCatalogResponse {
    pub(crate) claims: Vec<ClaimQueueEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvidenceQueueEntry {
    pub(crate) evidence: EvidenceItem,
    pub(crate) source_name: String,
    pub(crate) source_trust_score: u8,
    pub(crate) priority: IntelPriorityBreakdown,
    pub(crate) linked_case_count: usize,
    pub(crate) linked_claim_count: usize,
    pub(crate) max_linked_severity: Option<WatchlistSeverity>,
    pub(crate) semantic_score_bps: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ClaimQueueEntry {
    pub(crate) claim: ClaimRecord,
    pub(crate) evidence_title: String,
    pub(crate) evidence_observed_at: String,
    pub(crate) source_name: String,
    pub(crate) source_trust_score: u8,
    pub(crate) priority: IntelPriorityBreakdown,
    pub(crate) linked_case_count: usize,
    pub(crate) max_linked_severity: Option<WatchlistSeverity>,
    pub(crate) semantic_score_bps: Option<i32>,
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
    pub(crate) cases: Vec<CaseQueueEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CaseQueueEntry {
    pub(crate) case: CaseFile,
    pub(crate) watchlist_name: String,
    pub(crate) severity: WatchlistSeverity,
    pub(crate) priority: IntelPriorityBreakdown,
    pub(crate) latest_signal_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct CaseQueueFilterQuery {
    pub(crate) status: Option<CaseStatus>,
    pub(crate) severity: Option<WatchlistSeverity>,
    pub(crate) watchlist_id: Option<String>,
    pub(crate) primary_entity: Option<String>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct EvidenceQueueFilterQuery {
    pub(crate) source_id: Option<String>,
    pub(crate) tag: Option<String>,
    pub(crate) entity: Option<String>,
    pub(crate) linked_status: Option<CaseStatus>,
    pub(crate) min_trust: Option<u8>,
    #[serde(default, alias = "semantic_query")]
    pub(crate) q: Option<String>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ClaimQueueFilterQuery {
    pub(crate) review_status: Option<ClaimReviewStatus>,
    pub(crate) predicate: Option<String>,
    pub(crate) subject: Option<String>,
    pub(crate) linked_status: Option<CaseStatus>,
    pub(crate) min_confidence_bps: Option<u16>,
    #[serde(default, alias = "semantic_query")]
    pub(crate) q: Option<String>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AutopilotReviewKind {
    Case,
    Claim,
    Evidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AutopilotReviewQueueResponse {
    pub(crate) items: Vec<AutopilotReviewQueueEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AutopilotReviewQueueEntry {
    pub(crate) kind: AutopilotReviewKind,
    pub(crate) item_id: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) context_label: String,
    pub(crate) route: String,
    pub(crate) goal_hint: String,
    pub(crate) priority: IntelPriorityBreakdown,
    pub(crate) latest_signal_at: Option<String>,
    pub(crate) severity: Option<WatchlistSeverity>,
    pub(crate) case_status: Option<CaseStatus>,
    pub(crate) claim_review_status: Option<ClaimReviewStatus>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct AutopilotReviewQueueQuery {
    pub(crate) kind: Option<AutopilotReviewKind>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AutopilotReviewExportPacketResponse {
    pub(crate) packet_id: String,
    pub(crate) kind: AutopilotReviewKind,
    pub(crate) item: AutopilotReviewQueueEntry,
    pub(crate) narrative: String,
    pub(crate) supporting_cases: Vec<CaseQueueEntry>,
    pub(crate) supporting_claims: Vec<ClaimQueueEntry>,
    pub(crate) supporting_evidence: Vec<EvidenceQueueEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AutopilotReviewExportQuery {
    pub(crate) review_kind: AutopilotReviewKind,
    pub(crate) item_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MarketIntelBriefExportPacketResponse {
    pub(crate) packet_id: String,
    pub(crate) narrative: String,
    pub(crate) briefing: MarketIntelCaseBrief,
    pub(crate) case_file: CaseFile,
    pub(crate) watchlist: Watchlist,
    pub(crate) evidence: Vec<EvidenceQueueEntry>,
    pub(crate) claims: Vec<ClaimQueueEntry>,
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

#[derive(Debug, Clone)]
pub(crate) struct IntelDeskPostgresStore {
    pool: PgPool,
}

impl Default for IntelDeskStore {
    fn default() -> Self {
        Self::seeded()
    }
}

impl IntelDeskPostgresStore {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub(crate) async fn load_or_seed(&self) -> Result<IntelDeskStore, HelixError> {
        let store = IntelDeskStore {
            sources: load_records(&self.pool, "intel_sources").await?,
            watchlists: load_records(&self.pool, "intel_watchlists").await?,
            evidence: load_records(&self.pool, "intel_evidence").await?,
            claims: load_records(&self.pool, "intel_claims").await?,
            cases: load_records(&self.pool, "intel_cases").await?,
        };

        if store.is_empty() {
            let seeded = IntelDeskStore::seeded();
            self.save(&seeded).await?;
            Ok(seeded)
        } else {
            Ok(store)
        }
    }

    pub(crate) async fn save(&self, store: &IntelDeskStore) -> Result<(), HelixError> {
        let mut tx = self.pool.begin().await.map_err(db_error)?;

        sqlx::query("DELETE FROM intel_cases")
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        sqlx::query("DELETE FROM intel_claims")
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        sqlx::query("DELETE FROM intel_evidence")
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        sqlx::query("DELETE FROM intel_watchlists")
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        sqlx::query("DELETE FROM intel_sources")
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;

        for source in store.sources.values() {
            sqlx::query(
                "INSERT INTO intel_sources (id, record, updated_at) VALUES ($1, $2, now())",
            )
            .bind(&source.id)
            .bind(serde_json::to_value(source).map_err(serde_error)?)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        }

        for watchlist in store.watchlists.values() {
            sqlx::query(
                "INSERT INTO intel_watchlists (id, record, updated_at) VALUES ($1, $2, now())",
            )
            .bind(&watchlist.id)
            .bind(serde_json::to_value(watchlist).map_err(serde_error)?)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        }

        for evidence in store.evidence.values() {
            sqlx::query(
                "INSERT INTO intel_evidence (id, record, source_id, observed_at, updated_at) VALUES ($1, $2, $3, $4, now())",
            )
            .bind(&evidence.id)
            .bind(serde_json::to_value(evidence).map_err(serde_error)?)
            .bind(&evidence.source_id)
            .bind(&evidence.observed_at)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        }

        for claim in store.claims.values() {
            sqlx::query(
                "INSERT INTO intel_claims (id, record, evidence_id, review_status, updated_at) VALUES ($1, $2, $3, $4, now())",
            )
            .bind(&claim.id)
            .bind(serde_json::to_value(claim).map_err(serde_error)?)
            .bind(&claim.evidence_id)
            .bind(json_string(&claim.review_status))
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        }

        for case in store.cases.values() {
            sqlx::query(
                "INSERT INTO intel_cases (id, record, status, watchlist_id, updated_at) VALUES ($1, $2, $3, $4, now())",
            )
            .bind(&case.id)
            .bind(serde_json::to_value(case).map_err(serde_error)?)
            .bind(json_string(&case.status))
            .bind(&case.watchlist_id)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        }

        tx.commit().await.map_err(db_error)
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
                profile_id: "50000000-0000-0000-0000-000000000010".to_string(),
                name: "National Security RSS".to_string(),
                description: "Trusted feed for security, diplomacy, and leadership movements."
                    .to_string(),
                kind: SourceKind::RssFeed,
                endpoint_url: None,
                credential_id: None,
                credential_header_name: "Authorization".to_string(),
                credential_header_prefix: Some("Bearer".to_string()),
                cadence_minutes: 30,
                trust_score: 88,
                enabled: true,
                tags: vec!["osint".to_string(), "security".to_string()],
            },
            SourceDefinition {
                id: "website_orion_dynamics".to_string(),
                profile_id: "50000000-0000-0000-0000-000000000010".to_string(),
                name: "Orion Dynamics Website Diff".to_string(),
                description: "Website diff watcher for executive, facilities, and product signals."
                    .to_string(),
                kind: SourceKind::WebsiteDiff,
                endpoint_url: None,
                credential_id: None,
                credential_header_name: "Authorization".to_string(),
                credential_header_prefix: Some("Bearer".to_string()),
                cadence_minutes: 120,
                trust_score: 76,
                enabled: true,
                tags: vec!["company".to_string(), "website".to_string()],
            },
            SourceDefinition {
                id: "json_api_cloud_pricing".to_string(),
                profile_id: "50000000-0000-0000-0000-000000000010".to_string(),
                name: "Cloud Pricing API".to_string(),
                description: "Normalized pricing snapshots for competitor packaging, discounting, and seat changes."
                    .to_string(),
                kind: SourceKind::JsonApi,
                endpoint_url: None,
                credential_id: None,
                credential_header_name: "Authorization".to_string(),
                credential_header_prefix: Some("Bearer".to_string()),
                cadence_minutes: 240,
                trust_score: 82,
                enabled: true,
                tags: vec![
                    "market-intel".to_string(),
                    "pricing".to_string(),
                    "competitor".to_string(),
                ],
            },
            SourceDefinition {
                id: "website_vector_launches".to_string(),
                profile_id: "50000000-0000-0000-0000-000000000010".to_string(),
                name: "Vector Works Release Diff".to_string(),
                description: "Website diff feed for product launches, beta announcements, and packaging changes."
                    .to_string(),
                kind: SourceKind::WebsiteDiff,
                endpoint_url: None,
                credential_id: None,
                credential_header_name: "Authorization".to_string(),
                credential_header_prefix: Some("Bearer".to_string()),
                cadence_minutes: 180,
                trust_score: 74,
                enabled: true,
                tags: vec![
                    "market-intel".to_string(),
                    "product".to_string(),
                    "release".to_string(),
                ],
            },
            SourceDefinition {
                id: "rss_partner_ecosystem".to_string(),
                profile_id: "50000000-0000-0000-0000-000000000010".to_string(),
                name: "Partner Ecosystem Feed".to_string(),
                description: "Partnership, channel, and ecosystem signal feed for market mapping."
                    .to_string(),
                kind: SourceKind::RssFeed,
                endpoint_url: None,
                credential_id: None,
                credential_header_name: "Authorization".to_string(),
                credential_header_prefix: Some("Bearer".to_string()),
                cadence_minutes: 180,
                trust_score: 71,
                enabled: true,
                tags: vec![
                    "market-intel".to_string(),
                    "partnerships".to_string(),
                    "ecosystem".to_string(),
                ],
            },
            SourceDefinition {
                id: "rss_gtm_hiring_tracker".to_string(),
                profile_id: "50000000-0000-0000-0000-000000000010".to_string(),
                name: "GTM Hiring Tracker".to_string(),
                description: "Hiring and expansion signal feed for sales, success, and channel roles."
                    .to_string(),
                kind: SourceKind::RssFeed,
                endpoint_url: None,
                credential_id: None,
                credential_header_name: "Authorization".to_string(),
                credential_header_prefix: Some("Bearer".to_string()),
                cadence_minutes: 360,
                trust_score: 68,
                enabled: true,
                tags: vec![
                    "market-intel".to_string(),
                    "hiring".to_string(),
                    "go-to-market".to_string(),
                ],
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
            Watchlist {
                id: "market_pricing_moves".to_string(),
                name: "Pricing Moves".to_string(),
                description: "Track competitor price increases, discounting, bundling, and free-tier changes."
                    .to_string(),
                keywords: vec![
                    "pricing".to_string(),
                    "discount".to_string(),
                    "bundle".to_string(),
                    "seat".to_string(),
                ],
                entities: vec![
                    "boreal cloud".to_string(),
                    "northstar analytics".to_string(),
                ],
                min_source_trust: 65,
                severity: WatchlistSeverity::High,
                enabled: true,
            },
            Watchlist {
                id: "market_product_launches".to_string(),
                name: "Product Launches".to_string(),
                description: "Track competitor launches, release candidates, beta rollouts, and new SKUs."
                    .to_string(),
                keywords: vec![
                    "launch".to_string(),
                    "release".to_string(),
                    "beta".to_string(),
                    "ga".to_string(),
                ],
                entities: vec!["vector works".to_string(), "atlas crm".to_string()],
                min_source_trust: 60,
                severity: WatchlistSeverity::Medium,
                enabled: true,
            },
            Watchlist {
                id: "market_partnerships".to_string(),
                name: "Partnerships".to_string(),
                description: "Track reseller, integration, channel, and ecosystem partnership announcements."
                    .to_string(),
                keywords: vec![
                    "partnership".to_string(),
                    "integration".to_string(),
                    "reseller".to_string(),
                    "ecosystem".to_string(),
                ],
                entities: vec!["atlas crm".to_string(), "nebula retail".to_string()],
                min_source_trust: 55,
                severity: WatchlistSeverity::Medium,
                enabled: true,
            },
            Watchlist {
                id: "market_hiring_push".to_string(),
                name: "Hiring Push".to_string(),
                description: "Track competitor hiring bursts that signal expansion in sales, product, or channel coverage."
                    .to_string(),
                keywords: vec![
                    "hiring".to_string(),
                    "headcount".to_string(),
                    "recruiting".to_string(),
                    "open role".to_string(),
                ],
                entities: vec!["boreal cloud".to_string(), "vector works".to_string()],
                min_source_trust: 50,
                severity: WatchlistSeverity::Low,
                enabled: true,
            },
        ];
        for watchlist in watchlists {
            let watchlist =
                canonicalize_watchlist(watchlist).expect("seed watchlist should be valid");
            store.watchlists.insert(watchlist.id.clone(), watchlist);
        }

        store.seed_market_activity_demo();

        store
    }

    fn is_empty(&self) -> bool {
        self.sources.is_empty()
            && self.watchlists.is_empty()
            && self.evidence.is_empty()
            && self.claims.is_empty()
            && self.cases.is_empty()
    }

    fn seed_market_activity_demo(&mut self) {
        let seed_requests = [
            IngestEvidenceRequest {
                source_id: "json_api_cloud_pricing".to_string(),
                title: "Boreal Cloud adds bundled enterprise seat discount".to_string(),
                summary: "Pricing page update lowers cost for annual enterprise bundles.".to_string(),
                content: "Boreal Cloud introduced a bundled pricing discount for annual enterprise seat packages and updated bundle language on the pricing page.".to_string(),
                url: Some("https://example.org/boreal/pricing".to_string()),
                observed_at: "2026-03-06T08:00:00Z".to_string(),
                tags: vec!["market-intel".to_string(), "pricing".to_string()],
                entity_labels: vec!["boreal cloud".to_string()],
                proposed_claims: vec![ProposedClaim {
                    subject: "boreal cloud".to_string(),
                    predicate: "discounted".to_string(),
                    object: "enterprise bundle".to_string(),
                    confidence_bps: 8800,
                    rationale: Some("pricing page diff".to_string()),
                }],
            },
            IngestEvidenceRequest {
                source_id: "website_vector_launches".to_string(),
                title: "Vector Works beta launch expands forecasting module".to_string(),
                summary: "New beta launch adds AI forecasting and workflow automations.".to_string(),
                content: "Vector Works announced a beta launch for its forecasting module and highlighted release plans for workflow automation features.".to_string(),
                url: Some("https://example.org/vector/releases".to_string()),
                observed_at: "2026-03-06T09:15:00Z".to_string(),
                tags: vec!["market-intel".to_string(), "launch".to_string()],
                entity_labels: vec!["vector works".to_string()],
                proposed_claims: vec![ProposedClaim {
                    subject: "vector works".to_string(),
                    predicate: "launched_beta".to_string(),
                    object: "forecasting module".to_string(),
                    confidence_bps: 8600,
                    rationale: Some("release diff".to_string()),
                }],
            },
            IngestEvidenceRequest {
                source_id: "rss_partner_ecosystem".to_string(),
                title: "Atlas CRM expands reseller partnership with Nebula Retail".to_string(),
                summary: "Ecosystem announcement adds a new retail channel partnership.".to_string(),
                content: "Atlas CRM announced an ecosystem partnership and reseller program expansion with Nebula Retail.".to_string(),
                url: Some("https://example.org/atlas/partners".to_string()),
                observed_at: "2026-03-06T10:10:00Z".to_string(),
                tags: vec!["market-intel".to_string(), "partnership".to_string()],
                entity_labels: vec!["atlas crm".to_string(), "nebula retail".to_string()],
                proposed_claims: vec![ProposedClaim {
                    subject: "atlas crm".to_string(),
                    predicate: "partnered_with".to_string(),
                    object: "nebula retail".to_string(),
                    confidence_bps: 8300,
                    rationale: Some("partner ecosystem feed".to_string()),
                }],
            },
        ];

        for request in seed_requests {
            self.ingest_evidence(request)
                .expect("market intel demo seed should be valid");
        }
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

    fn market_intelligence_overview(&self) -> MarketIntelOverviewResponse {
        let market_sources = self
            .sources
            .values()
            .filter(|source| is_market_source(source))
            .cloned()
            .collect::<Vec<_>>();
        let market_source_ids = market_sources
            .iter()
            .map(|source| source.id.clone())
            .collect::<BTreeSet<_>>();
        let market_watchlists = self
            .watchlists
            .values()
            .filter(|watchlist| is_market_watchlist(watchlist))
            .cloned()
            .collect::<Vec<_>>();
        let active_market_cases = self
            .cases
            .values()
            .filter(|case| {
                case.status != CaseStatus::Closed
                    && self
                        .watchlists
                        .get(&case.watchlist_id)
                        .map(is_market_watchlist)
                        .unwrap_or(false)
            })
            .cloned()
            .collect::<Vec<_>>();
        let signal_window = self.market_signal_window(&market_source_ids);

        let mut theme_cards = market_theme_descriptors()
            .iter()
            .map(|(theme_id, name, summary)| {
                let theme_watchlists = market_watchlists
                    .iter()
                    .filter(|watchlist| market_theme_id_for_watchlist(watchlist) == Some(*theme_id))
                    .collect::<Vec<_>>();
                let theme_watchlist_ids = theme_watchlists
                    .iter()
                    .map(|watchlist| watchlist.id.as_str())
                    .collect::<BTreeSet<_>>();
                let theme_cases = active_market_cases
                    .iter()
                    .filter(|case| theme_watchlist_ids.contains(case.watchlist_id.as_str()))
                    .collect::<Vec<_>>();
                let theme_evidence_ids = theme_cases
                    .iter()
                    .flat_map(|case| case.evidence_ids.iter().cloned())
                    .collect::<BTreeSet<_>>();
                let theme_evidence = theme_evidence_ids
                    .iter()
                    .filter_map(|evidence_id| self.evidence.get(evidence_id).cloned())
                    .collect::<Vec<_>>();
                let theme_claims = theme_cases
                    .iter()
                    .flat_map(|case| case.claim_ids.iter())
                    .filter_map(|claim_id| self.claims.get(claim_id).cloned())
                    .collect::<Vec<_>>();
                let latest_signal_at = theme_evidence
                    .iter()
                    .map(|item| item.observed_at.as_str())
                    .max();
                let source_trust_scores = theme_evidence
                    .iter()
                    .filter_map(|item| {
                        self.sources
                            .get(&item.source_id)
                            .map(|source| source.trust_score)
                    })
                    .collect::<Vec<_>>();
                let max_severity = theme_watchlists
                    .iter()
                    .map(|watchlist| watchlist.severity)
                    .max_by_key(|severity| severity.weight());
                let (corroborated_claim_count, rejected_claim_count, max_claim_confidence_bps) =
                    claim_review_metrics(&theme_claims);
                let top_entities = theme_watchlists
                    .iter()
                    .flat_map(|watchlist| watchlist.entities.iter().cloned())
                    .chain(
                        theme_cases
                            .iter()
                            .filter_map(|case| case.primary_entity.clone()),
                    )
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .take(4)
                    .collect::<Vec<_>>();
                let priority = score_market_theme(
                    &MarketThemePriorityInput {
                        max_severity,
                        active_case_count: theme_cases.len(),
                        escalated_case_count: theme_cases
                            .iter()
                            .filter(|case| case.status == CaseStatus::Escalated)
                            .count(),
                        watchlist_count: theme_watchlists.len(),
                        evidence_count: theme_evidence.len(),
                        claim_count: theme_claims.len(),
                        corroborated_claim_count,
                        rejected_claim_count,
                        max_claim_confidence_bps,
                        source_trust_scores,
                        latest_signal_at: latest_signal_at.map(str::to_string),
                    },
                    &signal_window,
                );

                MarketIntelThemeCard {
                    theme_id: (*theme_id).to_string(),
                    name: (*name).to_string(),
                    summary: (*summary).to_string(),
                    priority,
                    watchlist_count: theme_watchlists.len(),
                    evidence_count: theme_evidence_ids.len(),
                    active_case_count: theme_cases.len(),
                    escalated_case_count: theme_cases
                        .iter()
                        .filter(|case| case.status == CaseStatus::Escalated)
                        .count(),
                    top_entities,
                }
            })
            .collect::<Vec<_>>();
        theme_cards.sort_by(|left, right| {
            right
                .priority
                .total
                .cmp(&left.priority.total)
                .then(left.theme_id.cmp(&right.theme_id))
        });

        let tracked_companies = market_watchlists
            .iter()
            .flat_map(|watchlist| watchlist.entities.iter().cloned())
            .chain(
                self.evidence
                    .values()
                    .filter(|evidence| market_source_ids.contains(&evidence.source_id))
                    .flat_map(|evidence| evidence.entity_labels.iter().cloned()),
            )
            .collect::<BTreeSet<_>>();

        let mut company_cards = tracked_companies
            .iter()
            .map(|company| {
                let company_evidence = self
                    .evidence
                    .values()
                    .filter(|evidence| market_source_ids.contains(&evidence.source_id))
                    .filter(|evidence| evidence.entity_labels.iter().any(|label| label == company))
                    .cloned()
                    .collect::<Vec<_>>();
                let mention_count = company_evidence.len();
                let company_claims = self
                    .claims
                    .values()
                    .filter(|claim| {
                        company_evidence
                            .iter()
                            .any(|evidence| evidence.id == claim.evidence_id)
                            && (claim.subject == *company || claim.object == *company)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let company_cases = active_market_cases
                    .iter()
                    .filter(|case| case.primary_entity.as_deref() == Some(company.as_str()))
                    .collect::<Vec<_>>();
                let latest_signal_at = company_evidence
                    .iter()
                    .map(|evidence| evidence.observed_at.clone())
                    .max();
                let themes = company_cases
                    .iter()
                    .filter_map(|case| {
                        self.watchlists
                            .get(&case.watchlist_id)
                            .and_then(market_theme_id_for_watchlist)
                    })
                    .map(market_theme_name)
                    .chain(
                        market_watchlists
                            .iter()
                            .filter(|watchlist| {
                                watchlist.entities.iter().any(|entity| entity == company)
                            })
                            .filter_map(|watchlist| market_theme_id_for_watchlist(watchlist))
                            .map(market_theme_name),
                    )
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                let source_trust_scores = company_evidence
                    .iter()
                    .filter_map(|evidence| {
                        self.sources
                            .get(&evidence.source_id)
                            .map(|source| source.trust_score)
                    })
                    .collect::<Vec<_>>();
                let max_severity = company_cases
                    .iter()
                    .filter_map(|case| {
                        self.watchlists
                            .get(&case.watchlist_id)
                            .map(|watchlist| watchlist.severity)
                    })
                    .chain(
                        market_watchlists
                            .iter()
                            .filter(|watchlist| {
                                watchlist.entities.iter().any(|entity| entity == company)
                            })
                            .map(|watchlist| watchlist.severity),
                    )
                    .max_by_key(|severity| severity.weight());
                let (corroborated_claim_count, rejected_claim_count, max_claim_confidence_bps) =
                    claim_review_metrics(&company_claims);
                let priority = score_market_company(
                    &MarketCompanyPriorityInput {
                        max_severity,
                        active_case_count: company_cases.len(),
                        escalated_case_count: company_cases
                            .iter()
                            .filter(|case| case.status == CaseStatus::Escalated)
                            .count(),
                        mention_count,
                        claim_count: company_claims.len(),
                        corroborated_claim_count,
                        rejected_claim_count,
                        max_claim_confidence_bps,
                        source_trust_scores,
                        latest_signal_at: latest_signal_at.clone(),
                    },
                    &signal_window,
                );

                MarketIntelCompanyCard {
                    company: company.clone(),
                    priority,
                    mention_count,
                    claim_count: company_claims.len(),
                    active_case_count: company_cases.len(),
                    themes,
                    latest_signal_at,
                }
            })
            .filter(|card| !card.themes.is_empty() || card.mention_count > 0)
            .collect::<Vec<_>>();
        company_cards.sort_by(|left, right| {
            right
                .priority
                .total
                .cmp(&left.priority.total)
                .then(right.latest_signal_at.cmp(&left.latest_signal_at))
                .then(left.company.cmp(&right.company))
        });
        company_cards.truncate(6);

        let mut case_briefs = active_market_cases
            .iter()
            .filter_map(|case| self.market_case_brief_with_window(case, &signal_window))
            .collect::<Vec<_>>();
        case_briefs.sort_by(|left, right| {
            right
                .priority
                .total
                .cmp(&left.priority.total)
                .then(right.latest_signal_at.cmp(&left.latest_signal_at))
                .then(left.case_id.cmp(&right.case_id))
        });

        MarketIntelOverviewResponse {
            market_source_count: market_sources.len(),
            market_watchlist_count: market_watchlists.len(),
            tracked_company_count: tracked_companies.len(),
            active_case_count: active_market_cases.len(),
            theme_cards,
            company_cards,
            case_briefs,
            playbooks: market_intelligence_playbooks(),
        }
    }

    fn market_case_brief(&self, case: &CaseFile) -> Option<MarketIntelCaseBrief> {
        let signal_window = self.market_signal_window(
            &self
                .sources
                .values()
                .filter(|source| is_market_source(source))
                .map(|source| source.id.clone())
                .collect::<BTreeSet<_>>(),
        );
        self.market_case_brief_with_window(case, &signal_window)
    }

    fn market_case_brief_with_window(
        &self,
        case: &CaseFile,
        signal_window: &MarketSignalWindow,
    ) -> Option<MarketIntelCaseBrief> {
        let watchlist = self.watchlists.get(&case.watchlist_id)?;
        let theme_id = market_theme_id_for_watchlist(watchlist)?;
        let theme_name = market_theme_name(theme_id).to_string();
        let priority = self.case_priority(case, signal_window)?;
        let evidence = self.case_evidence(case);
        let claims = self.case_claims(case);
        let latest_signal_at = latest_signal_at(&evidence);
        let latest_titles = evidence
            .iter()
            .map(|item| item.title.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .take(2)
            .collect::<Vec<_>>();
        let key_claims = summarize_key_claims(&claims);
        let summary = build_market_case_summary(
            case,
            &theme_name,
            latest_signal_at.as_deref(),
            &latest_titles,
            &key_claims,
        );

        Some(MarketIntelCaseBrief {
            case_id: case.id.clone(),
            title: case.title.clone(),
            company: case.primary_entity.clone(),
            theme_id: theme_id.to_string(),
            theme_name,
            priority,
            status: case.status,
            latest_signal_at,
            evidence_count: evidence.len(),
            claim_count: claims.len(),
            attached_to_case: case.briefing_summary.is_some(),
            summary,
            key_claims,
            recommended_actions: market_brief_actions(theme_id),
        })
    }

    fn generate_market_brief(
        &mut self,
        case_id: &str,
        attach_to_case: bool,
    ) -> Result<GenerateMarketIntelBriefResponse, HelixError> {
        let case = self
            .cases
            .get(case_id)
            .cloned()
            .ok_or_else(|| HelixError::not_found(format!("case {}", case_id)))?;
        let briefing = self.market_case_brief(&case).ok_or_else(|| {
            HelixError::validation_error("case", "case is not a market intelligence case")
        })?;

        let transition = if attach_to_case {
            let attached = self.transition_case(
                case_id,
                CaseCommand::AttachBrief {
                    summary: briefing_text(&briefing),
                },
            )?;
            Some(attached)
        } else {
            None
        };

        let briefing = if let Some(updated) = transition.as_ref() {
            self.market_case_brief(&updated.case)
                .ok_or_else(|| HelixError::internal_error("market briefing missing after attach"))?
        } else {
            briefing
        };

        Ok(GenerateMarketIntelBriefResponse {
            briefing,
            transition,
        })
    }

    fn market_signal_window(&self, market_source_ids: &BTreeSet<String>) -> MarketSignalWindow {
        MarketSignalWindow::from_observed_at_values(
            self.evidence
                .values()
                .filter(|evidence| market_source_ids.contains(&evidence.source_id))
                .map(|evidence| evidence.observed_at.as_str()),
        )
    }

    fn case_signal_window(&self) -> IntelSignalWindow {
        IntelSignalWindow::from_observed_at_values(
            self.evidence
                .values()
                .map(|evidence| evidence.observed_at.as_str()),
        )
    }

    fn case_queue(
        &self,
        filters: &CaseQueueFilterQuery,
    ) -> Result<Vec<CaseQueueEntry>, HelixError> {
        let limit = normalized_limit(filters.limit, "case")?;
        let watchlist_id =
            normalized_optional_filter(filters.watchlist_id.as_deref(), "watchlist_id")?;
        let primary_entity =
            normalized_optional_filter(filters.primary_entity.as_deref(), "primary_entity")?
                .map(|value| value.to_lowercase());
        let signal_window = self.case_signal_window();
        let mut cases = self
            .cases
            .values()
            .filter(|case| {
                filters
                    .status
                    .map(|status| case.status == status)
                    .unwrap_or(true)
            })
            .filter(|case| {
                filters
                    .severity
                    .map(|severity| {
                        self.watchlists
                            .get(&case.watchlist_id)
                            .map(|watchlist| watchlist.severity == severity)
                            .unwrap_or(false)
                    })
                    .unwrap_or(true)
            })
            .filter(|case| {
                watchlist_id
                    .as_deref()
                    .map(|target| case.watchlist_id == target)
                    .unwrap_or(true)
            })
            .filter(|case| {
                primary_entity
                    .as_deref()
                    .map(|target| {
                        case.primary_entity
                            .as_deref()
                            .map(|entity| entity == target)
                            .unwrap_or(false)
                    })
                    .unwrap_or(true)
            })
            .map(|case| self.case_queue_entry(case, &signal_window))
            .collect::<Result<Vec<_>, _>>()?;
        cases.sort_by(|left, right| {
            right
                .priority
                .total
                .cmp(&left.priority.total)
                .then(right.latest_signal_at.cmp(&left.latest_signal_at))
                .then(left.case.id.cmp(&right.case.id))
        });
        if let Some(limit) = limit {
            cases.truncate(limit);
        }
        Ok(cases)
    }

    fn case_queue_entry(
        &self,
        case: &CaseFile,
        signal_window: &IntelSignalWindow,
    ) -> Result<CaseQueueEntry, HelixError> {
        let watchlist = self
            .watchlists
            .get(&case.watchlist_id)
            .ok_or_else(|| HelixError::internal_error("case references unknown watchlist"))?;
        let priority = self
            .case_priority(case, signal_window)
            .ok_or_else(|| HelixError::internal_error("case priority could not be computed"))?;
        Ok(CaseQueueEntry {
            case: case.clone(),
            watchlist_name: watchlist.name.clone(),
            severity: watchlist.severity,
            priority,
            latest_signal_at: latest_signal_at(&self.case_evidence(case)),
        })
    }

    fn case_priority(
        &self,
        case: &CaseFile,
        signal_window: &IntelSignalWindow,
    ) -> Option<IntelPriorityBreakdown> {
        let watchlist = self.watchlists.get(&case.watchlist_id)?;
        let evidence = self.case_evidence(case);
        let claims = self.case_claims(case);
        let source_trust_scores = evidence
            .iter()
            .filter_map(|item| {
                self.sources
                    .get(&item.source_id)
                    .map(|source| source.trust_score)
            })
            .collect::<Vec<_>>();
        let latest_signal_at = latest_signal_at(&evidence);
        let (corroborated_claim_count, rejected_claim_count, max_claim_confidence_bps) =
            claim_review_metrics(&claims);

        Some(score_case(
            &CasePriorityInput {
                status: case.status,
                severity: watchlist.severity,
                source_trust_scores,
                evidence_count: evidence.len(),
                claim_count: claims.len(),
                corroborated_claim_count,
                rejected_claim_count,
                max_claim_confidence_bps,
                latest_signal_at,
                attached_to_case: case.briefing_summary.is_some(),
            },
            signal_window,
        ))
    }

    fn case_evidence(&self, case: &CaseFile) -> Vec<EvidenceItem> {
        case.evidence_ids
            .iter()
            .filter_map(|evidence_id| self.evidence.get(evidence_id).cloned())
            .collect()
    }

    fn case_claims(&self, case: &CaseFile) -> Vec<ClaimRecord> {
        case.claim_ids
            .iter()
            .filter_map(|claim_id| self.claims.get(claim_id).cloned())
            .collect()
    }

    fn evidence_cases(&self, evidence_id: &str) -> Vec<CaseFile> {
        self.cases
            .values()
            .filter(|case| case.evidence_ids.iter().any(|id| id == evidence_id))
            .cloned()
            .collect()
    }

    fn claim_cases(&self, claim_id: &str) -> Vec<CaseFile> {
        self.cases
            .values()
            .filter(|case| case.claim_ids.iter().any(|id| id == claim_id))
            .cloned()
            .collect()
    }

    fn max_linked_severity(&self, cases: &[CaseFile]) -> Option<WatchlistSeverity> {
        cases
            .iter()
            .filter_map(|case| {
                self.watchlists
                    .get(&case.watchlist_id)
                    .map(|watchlist| watchlist.severity)
            })
            .max_by_key(|severity| severity.weight())
    }

    fn evidence_queue(
        &self,
        filters: &EvidenceQueueFilterQuery,
    ) -> Result<Vec<EvidenceQueueEntry>, HelixError> {
        let limit = normalized_limit(filters.limit, "evidence")?;
        let source_id = normalized_optional_filter(filters.source_id.as_deref(), "source_id")?;
        let tag = normalized_optional_filter(filters.tag.as_deref(), "tag")?
            .map(|value| value.to_lowercase());
        let entity = normalized_optional_filter(filters.entity.as_deref(), "entity")?
            .map(|value| value.to_lowercase());
        let min_trust = normalized_trust_score(filters.min_trust)?;
        let semantic_ranker = normalized_semantic_query(filters.q.as_deref(), "q")?
            .map(|query| SemanticRanker::new(&query))
            .transpose()?;
        let signal_window = self.case_signal_window();
        let mut evidence = self
            .evidence
            .values()
            .filter(|item| {
                source_id
                    .as_deref()
                    .map(|value| item.source_id == value)
                    .unwrap_or(true)
            })
            .filter(|item| {
                tag.as_deref()
                    .map(|value| item.tags.iter().any(|tag| tag == value))
                    .unwrap_or(true)
            })
            .filter(|item| {
                entity
                    .as_deref()
                    .map(|value| item.entity_labels.iter().any(|entity| entity == value))
                    .unwrap_or(true)
            })
            .filter(|item| {
                min_trust
                    .map(|min_trust| {
                        self.sources
                            .get(&item.source_id)
                            .map(|source| source.trust_score >= min_trust)
                            .unwrap_or(false)
                    })
                    .unwrap_or(true)
            })
            .filter(|item| {
                filters
                    .linked_status
                    .map(|status| {
                        self.evidence_cases(&item.id)
                            .iter()
                            .any(|case| case.status == status)
                    })
                    .unwrap_or(true)
            })
            .map(|item| self.evidence_queue_entry(item, &signal_window))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(ranker) = &semantic_ranker {
            for entry in &mut evidence {
                entry.semantic_score_bps =
                    Some(ranker.score_bps(&evidence_semantic_document(entry))?);
            }
            evidence.sort_by(|left, right| {
                right
                    .semantic_score_bps
                    .unwrap_or(i32::MIN)
                    .cmp(&left.semantic_score_bps.unwrap_or(i32::MIN))
                    .then(right.priority.total.cmp(&left.priority.total))
                    .then(right.evidence.observed_at.cmp(&left.evidence.observed_at))
                    .then(left.evidence.id.cmp(&right.evidence.id))
            });
        } else {
            evidence.sort_by(|left, right| {
                right
                    .priority
                    .total
                    .cmp(&left.priority.total)
                    .then(right.evidence.observed_at.cmp(&left.evidence.observed_at))
                    .then(left.evidence.id.cmp(&right.evidence.id))
            });
        }
        if let Some(limit) = limit {
            evidence.truncate(limit);
        }
        Ok(evidence)
    }

    fn evidence_queue_entry(
        &self,
        evidence: &EvidenceItem,
        signal_window: &IntelSignalWindow,
    ) -> Result<EvidenceQueueEntry, HelixError> {
        let source = self
            .sources
            .get(&evidence.source_id)
            .ok_or_else(|| HelixError::internal_error("evidence references unknown source"))?;
        let linked_cases = self.evidence_cases(&evidence.id);
        let linked_claims = self
            .claims
            .values()
            .filter(|claim| claim.evidence_id == evidence.id)
            .cloned()
            .collect::<Vec<_>>();
        let (corroborated_claim_count, rejected_claim_count, max_claim_confidence_bps) =
            claim_review_metrics(&linked_claims);
        let priority = score_evidence(
            &EvidencePriorityInput {
                linked_case_statuses: linked_cases.iter().map(|case| case.status).collect(),
                max_linked_severity: self.max_linked_severity(&linked_cases),
                source_trust_scores: vec![source.trust_score],
                claim_count: linked_claims.len(),
                corroborated_claim_count,
                rejected_claim_count,
                max_claim_confidence_bps,
                observed_at: Some(evidence.observed_at.clone()),
                linked_case_count: linked_cases.len(),
            },
            signal_window,
        );

        Ok(EvidenceQueueEntry {
            evidence: evidence.clone(),
            source_name: source.name.clone(),
            source_trust_score: source.trust_score,
            priority,
            linked_case_count: linked_cases.len(),
            linked_claim_count: linked_claims.len(),
            max_linked_severity: self.max_linked_severity(&linked_cases),
            semantic_score_bps: None,
        })
    }

    fn claim_queue(
        &self,
        filters: &ClaimQueueFilterQuery,
    ) -> Result<Vec<ClaimQueueEntry>, HelixError> {
        let limit = normalized_limit(filters.limit, "claim")?;
        let predicate = normalized_optional_filter(filters.predicate.as_deref(), "predicate")?
            .map(|value| value.to_lowercase());
        let subject = normalized_optional_filter(filters.subject.as_deref(), "subject")?
            .map(|value| value.to_lowercase());
        let min_confidence_bps = normalized_confidence_bps(filters.min_confidence_bps)?;
        let semantic_ranker = normalized_semantic_query(filters.q.as_deref(), "q")?
            .map(|query| SemanticRanker::new(&query))
            .transpose()?;
        let signal_window = self.case_signal_window();
        let mut claims = self
            .claims
            .values()
            .filter(|claim| {
                filters
                    .review_status
                    .map(|status| claim.review_status == status)
                    .unwrap_or(true)
            })
            .filter(|claim| {
                predicate
                    .as_deref()
                    .map(|value| claim.predicate == value)
                    .unwrap_or(true)
            })
            .filter(|claim| {
                subject
                    .as_deref()
                    .map(|value| claim.subject == value)
                    .unwrap_or(true)
            })
            .filter(|claim| {
                min_confidence_bps
                    .map(|minimum| claim.confidence_bps >= minimum)
                    .unwrap_or(true)
            })
            .filter(|claim| {
                filters
                    .linked_status
                    .map(|status| {
                        self.claim_cases(&claim.id)
                            .iter()
                            .any(|case| case.status == status)
                    })
                    .unwrap_or(true)
            })
            .map(|claim| self.claim_queue_entry(claim, &signal_window))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(ranker) = &semantic_ranker {
            for entry in &mut claims {
                entry.semantic_score_bps = Some(ranker.score_bps(&claim_semantic_document(entry))?);
            }
            claims.sort_by(|left, right| {
                right
                    .semantic_score_bps
                    .unwrap_or(i32::MIN)
                    .cmp(&left.semantic_score_bps.unwrap_or(i32::MIN))
                    .then(right.priority.total.cmp(&left.priority.total))
                    .then(right.claim.confidence_bps.cmp(&left.claim.confidence_bps))
                    .then(left.claim.id.cmp(&right.claim.id))
            });
        } else {
            claims.sort_by(|left, right| {
                right
                    .priority
                    .total
                    .cmp(&left.priority.total)
                    .then(right.claim.confidence_bps.cmp(&left.claim.confidence_bps))
                    .then(left.claim.id.cmp(&right.claim.id))
            });
        }
        if let Some(limit) = limit {
            claims.truncate(limit);
        }
        Ok(claims)
    }

    fn claim_queue_entry(
        &self,
        claim: &ClaimRecord,
        signal_window: &IntelSignalWindow,
    ) -> Result<ClaimQueueEntry, HelixError> {
        let evidence = self
            .evidence
            .get(&claim.evidence_id)
            .ok_or_else(|| HelixError::internal_error("claim references unknown evidence"))?;
        let source = self.sources.get(&evidence.source_id).ok_or_else(|| {
            HelixError::internal_error("claim evidence references unknown source")
        })?;
        let linked_cases = self.claim_cases(&claim.id);
        let sibling_claims = self
            .claims
            .values()
            .filter(|item| item.evidence_id == claim.evidence_id)
            .cloned()
            .collect::<Vec<_>>();
        let corroborated_sibling_count = sibling_claims
            .iter()
            .filter(|item| {
                item.id != claim.id && item.review_status == ClaimReviewStatus::Corroborated
            })
            .count();
        let rejected_sibling_count = sibling_claims
            .iter()
            .filter(|item| item.id != claim.id && item.review_status == ClaimReviewStatus::Rejected)
            .count();
        let priority = score_claim(
            &ClaimPriorityInput {
                review_status: claim.review_status,
                confidence_bps: claim.confidence_bps,
                linked_case_statuses: linked_cases.iter().map(|case| case.status).collect(),
                max_linked_severity: self.max_linked_severity(&linked_cases),
                source_trust_scores: vec![source.trust_score],
                evidence_observed_at: Some(evidence.observed_at.clone()),
                sibling_claim_count: sibling_claims.len(),
                corroborated_sibling_count,
                rejected_sibling_count,
            },
            signal_window,
        );

        Ok(ClaimQueueEntry {
            claim: claim.clone(),
            evidence_title: evidence.title.clone(),
            evidence_observed_at: evidence.observed_at.clone(),
            source_name: source.name.clone(),
            source_trust_score: source.trust_score,
            priority,
            linked_case_count: linked_cases.len(),
            max_linked_severity: self.max_linked_severity(&linked_cases),
            semantic_score_bps: None,
        })
    }

    pub(crate) fn autopilot_review_queue(
        &self,
        filters: &AutopilotReviewQueueQuery,
    ) -> Result<Vec<AutopilotReviewQueueEntry>, HelixError> {
        let limit = normalized_limit(filters.limit, "review queue")?;
        let mut items = Vec::new();

        if filters.kind.is_none() || filters.kind == Some(AutopilotReviewKind::Case) {
            for entry in self
                .case_queue(&CaseQueueFilterQuery::default())?
                .into_iter()
                .filter(|entry| entry.case.status != CaseStatus::Closed)
            {
                items.push(AutopilotReviewQueueEntry {
                    kind: AutopilotReviewKind::Case,
                    item_id: entry.case.id.clone(),
                    title: entry.case.title.clone(),
                    summary: entry.case.latest_reason.clone(),
                    context_label: entry.watchlist_name.clone(),
                    route: "/cases".to_string(),
                    goal_hint: case_goal_hint(&entry.case, &entry.watchlist_name),
                    priority: entry.priority.clone(),
                    latest_signal_at: entry.latest_signal_at.clone(),
                    severity: Some(entry.severity),
                    case_status: Some(entry.case.status),
                    claim_review_status: None,
                });
            }
        }

        if filters.kind.is_none() || filters.kind == Some(AutopilotReviewKind::Claim) {
            for entry in self
                .claim_queue(&ClaimQueueFilterQuery::default())?
                .into_iter()
                .filter(|entry| entry.claim.review_status != ClaimReviewStatus::Rejected)
            {
                items.push(AutopilotReviewQueueEntry {
                    kind: AutopilotReviewKind::Claim,
                    item_id: entry.claim.id.clone(),
                    title: format!(
                        "{} {} {}",
                        entry.claim.subject, entry.claim.predicate, entry.claim.object
                    ),
                    summary: entry.claim.rationale.clone(),
                    context_label: entry.evidence_title.clone(),
                    route: "/evidence".to_string(),
                    goal_hint: claim_goal_hint(&entry.claim, &entry.evidence_title),
                    priority: entry.priority.clone(),
                    latest_signal_at: Some(entry.evidence_observed_at.clone()),
                    severity: entry.max_linked_severity,
                    case_status: None,
                    claim_review_status: Some(entry.claim.review_status),
                });
            }
        }

        if filters.kind.is_none() || filters.kind == Some(AutopilotReviewKind::Evidence) {
            for entry in self.evidence_queue(&EvidenceQueueFilterQuery::default())? {
                items.push(AutopilotReviewQueueEntry {
                    kind: AutopilotReviewKind::Evidence,
                    item_id: entry.evidence.id.clone(),
                    title: entry.evidence.title.clone(),
                    summary: if entry.evidence.summary.is_empty() {
                        entry.evidence.content.chars().take(160).collect()
                    } else {
                        entry.evidence.summary.clone()
                    },
                    context_label: entry.source_name.clone(),
                    route: "/evidence".to_string(),
                    goal_hint: evidence_goal_hint(&entry.evidence, entry.linked_case_count),
                    priority: entry.priority.clone(),
                    latest_signal_at: Some(entry.evidence.observed_at.clone()),
                    severity: entry.max_linked_severity,
                    case_status: None,
                    claim_review_status: None,
                });
            }
        }

        items.sort_by(|left, right| {
            right
                .priority
                .total
                .cmp(&left.priority.total)
                .then(right.latest_signal_at.cmp(&left.latest_signal_at))
                .then(
                    autopilot_review_kind_rank(left.kind)
                        .cmp(&autopilot_review_kind_rank(right.kind)),
                )
                .then(left.item_id.cmp(&right.item_id))
        });
        if let Some(limit) = limit {
            items.truncate(limit);
        }
        Ok(items)
    }

    pub(crate) fn autopilot_review_item(
        &self,
        kind: AutopilotReviewKind,
        item_id: &str,
    ) -> Result<AutopilotReviewQueueEntry, HelixError> {
        self.autopilot_review_queue(&AutopilotReviewQueueQuery {
            kind: Some(kind),
            limit: None,
        })?
        .into_iter()
        .find(|item| item.item_id == item_id)
        .ok_or_else(|| {
            HelixError::not_found(format!(
                "review item {}:{}",
                match kind {
                    AutopilotReviewKind::Case => "case",
                    AutopilotReviewKind::Claim => "claim",
                    AutopilotReviewKind::Evidence => "evidence",
                },
                item_id
            ))
        })
    }

    pub(crate) fn build_review_export_packet(
        &self,
        kind: AutopilotReviewKind,
        item_id: &str,
    ) -> Result<AutopilotReviewExportPacketResponse, HelixError> {
        let signal_window = self.case_signal_window();
        let item = self.autopilot_review_item(kind, item_id)?;

        let (mut supporting_cases, mut supporting_claims, mut supporting_evidence) = match kind {
            AutopilotReviewKind::Case => {
                let case = self
                    .cases
                    .get(item_id)
                    .ok_or_else(|| HelixError::internal_error("review case missing"))?;
                let cases = vec![self.case_queue_entry(case, &signal_window)?];
                let claims = self
                    .case_claims(case)
                    .iter()
                    .map(|claim| self.claim_queue_entry(claim, &signal_window))
                    .collect::<Result<Vec<_>, _>>()?;
                let evidence = self
                    .case_evidence(case)
                    .iter()
                    .map(|evidence| self.evidence_queue_entry(evidence, &signal_window))
                    .collect::<Result<Vec<_>, _>>()?;
                (cases, claims, evidence)
            }
            AutopilotReviewKind::Claim => {
                let claim = self
                    .claims
                    .get(item_id)
                    .ok_or_else(|| HelixError::internal_error("review claim missing"))?;
                let cases = self
                    .claim_cases(&claim.id)
                    .iter()
                    .map(|case| self.case_queue_entry(case, &signal_window))
                    .collect::<Result<Vec<_>, _>>()?;
                let claims = vec![self.claim_queue_entry(claim, &signal_window)?];
                let evidence = self
                    .evidence
                    .get(&claim.evidence_id)
                    .map(|evidence| self.evidence_queue_entry(evidence, &signal_window))
                    .transpose()?
                    .into_iter()
                    .collect::<Vec<_>>();
                (cases, claims, evidence)
            }
            AutopilotReviewKind::Evidence => {
                let evidence = self
                    .evidence
                    .get(item_id)
                    .ok_or_else(|| HelixError::internal_error("review evidence missing"))?;
                let cases = self
                    .evidence_cases(&evidence.id)
                    .iter()
                    .map(|case| self.case_queue_entry(case, &signal_window))
                    .collect::<Result<Vec<_>, _>>()?;
                let claims = self
                    .claims_for_evidence(&evidence.id)
                    .iter()
                    .map(|claim| self.claim_queue_entry(claim, &signal_window))
                    .collect::<Result<Vec<_>, _>>()?;
                let evidence_entries = vec![self.evidence_queue_entry(evidence, &signal_window)?];
                (cases, claims, evidence_entries)
            }
        };

        supporting_cases.sort_by(|left, right| {
            right
                .priority
                .total
                .cmp(&left.priority.total)
                .then(right.latest_signal_at.cmp(&left.latest_signal_at))
                .then(left.case.id.cmp(&right.case.id))
        });
        supporting_claims.sort_by(|left, right| {
            right
                .priority
                .total
                .cmp(&left.priority.total)
                .then(right.claim.confidence_bps.cmp(&left.claim.confidence_bps))
                .then(left.claim.id.cmp(&right.claim.id))
        });
        supporting_evidence.sort_by(|left, right| {
            right
                .priority
                .total
                .cmp(&left.priority.total)
                .then(right.evidence.observed_at.cmp(&left.evidence.observed_at))
                .then(left.evidence.id.cmp(&right.evidence.id))
        });

        Ok(AutopilotReviewExportPacketResponse {
            packet_id: stable_id("review_export", &[review_kind_label(kind), item_id]),
            kind,
            narrative: build_review_export_narrative(
                &item,
                &supporting_cases,
                &supporting_claims,
                &supporting_evidence,
            ),
            item,
            supporting_cases,
            supporting_claims,
            supporting_evidence,
        })
    }

    pub(crate) fn build_market_brief_export_packet(
        &self,
        case_id: &str,
    ) -> Result<MarketIntelBriefExportPacketResponse, HelixError> {
        let case = self
            .cases
            .get(case_id)
            .cloned()
            .ok_or_else(|| HelixError::not_found(format!("case {}", case_id)))?;
        let watchlist = self
            .watchlists
            .get(&case.watchlist_id)
            .cloned()
            .ok_or_else(|| {
                HelixError::internal_error("market export references unknown watchlist")
            })?;
        if !is_market_watchlist(&watchlist) {
            return Err(HelixError::validation_error(
                "case",
                "case is not a market intelligence case",
            ));
        }

        let briefing = self.market_case_brief(&case).ok_or_else(|| {
            HelixError::validation_error("case", "case is not a market intelligence case")
        })?;
        let signal_window = self.case_signal_window();
        let mut evidence = self
            .case_evidence(&case)
            .iter()
            .map(|item| self.evidence_queue_entry(item, &signal_window))
            .collect::<Result<Vec<_>, _>>()?;
        let mut claims = self
            .case_claims(&case)
            .iter()
            .map(|claim| self.claim_queue_entry(claim, &signal_window))
            .collect::<Result<Vec<_>, _>>()?;

        evidence.sort_by(|left, right| {
            right
                .priority
                .total
                .cmp(&left.priority.total)
                .then(right.evidence.observed_at.cmp(&left.evidence.observed_at))
                .then(left.evidence.id.cmp(&right.evidence.id))
        });
        claims.sort_by(|left, right| {
            right
                .priority
                .total
                .cmp(&left.priority.total)
                .then(right.claim.confidence_bps.cmp(&left.claim.confidence_bps))
                .then(left.claim.id.cmp(&right.claim.id))
        });

        Ok(MarketIntelBriefExportPacketResponse {
            packet_id: stable_id("market_brief_export", &[case_id]),
            narrative: briefing_text(&briefing),
            briefing,
            case_file: case,
            watchlist,
            evidence,
            claims,
        })
    }

    fn create_source(
        &mut self,
        request: CreateSourceRequest,
    ) -> Result<SourceDefinition, HelixError> {
        let source = canonicalize_source(SourceDefinition {
            id: slugify(&request.name),
            profile_id: request
                .profile_id
                .unwrap_or_else(|| "50000000-0000-0000-0000-000000000010".to_string()),
            name: request.name,
            description: request.description,
            kind: request.kind,
            endpoint_url: request.endpoint_url,
            credential_id: request.credential_id,
            credential_header_name: request
                .credential_header_name
                .unwrap_or_else(|| "Authorization".to_string()),
            credential_header_prefix: request
                .credential_header_prefix
                .or_else(|| Some("Bearer".to_string())),
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
        self.watchlists
            .insert(watchlist.id.clone(), watchlist.clone());
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
            return Err(HelixError::validation_error("source", "source is disabled"));
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
                &[
                    &evidence.id,
                    &proposed.subject,
                    &proposed.predicate,
                    &proposed.object,
                ],
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
            let claim_ids = claims
                .iter()
                .map(|claim| claim.id.clone())
                .collect::<Vec<_>>();

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

fn build_case_title(
    hit: &WatchlistHit,
    evidence: &EvidenceItem,
    primary_entity: Option<&str>,
) -> String {
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

async fn fetch_source_body(
    endpoint_url: &str,
    auth: Option<&SourceFetchAuth>,
) -> Result<String, HelixError> {
    let client = reqwest::Client::new();
    let mut request = client.get(endpoint_url);
    if let Some(auth) = auth {
        request = request.header(auth.header_name.clone(), auth.header_value.clone());
    }

    let response = request
        .send()
        .await
        .map_err(|error| source_fetch_error(error.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        return Err(source_fetch_error(format!(
            "source endpoint returned HTTP {status}"
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|error| source_fetch_error(error.to_string()))?;
    if bytes.len() > MAX_SOURCE_FETCH_BYTES {
        return Err(HelixError::validation_error(
            "source.payload",
            "source payload exceeds 1 MiB",
        ));
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| {
        HelixError::validation_error("source.payload", "source payload must be UTF-8 text")
    })
}

fn source_fetch_error(details: String) -> HelixError {
    HelixError::external_service_error("source_fetch".to_string(), details)
}

async fn source_fetch_auth(
    state: &AppState,
    source: &SourceDefinition,
) -> Result<Option<SourceFetchAuth>, HelixError> {
    let Some(credential_id) = source.credential_id.as_deref() else {
        return Ok(None);
    };
    let profile_id: ProfileId = source
        .profile_id
        .parse()
        .map_err(|_| HelixError::validation_error("source.profile_id", "must be a valid UUID"))?;
    let credential_id_uuid: CredentialId = credential_id.parse().map_err(|_| {
        HelixError::validation_error("source.credential_id", "must be a valid UUID")
    })?;
    let Some(persistence) = state.state_persistence.as_ref() else {
        return Err(HelixError::config_error(
            "source credential collection requires DATABASE_URL",
        ));
    };
    let Some(encrypted_data) = persistence
        .encrypted_credential_data(&profile_id, &credential_id_uuid)
        .await?
    else {
        return Err(HelixError::not_found(format!(
            "credential {credential_id} for source {}",
            source.id
        )));
    };
    let encrypter = credential_encrypter_from_env()?;
    let secret = encrypter
        .decrypt(&encrypted_data)
        .await
        .map_err(|error| HelixError::encryption_error(error.to_string()))?;
    let header_name =
        HeaderName::from_bytes(source.credential_header_name.as_bytes()).map_err(|_| {
            HelixError::validation_error(
                "source.credential_header_name",
                "must be a valid HTTP header name",
            )
        })?;
    let header_value =
        source_credential_header_value(source.credential_header_prefix.as_deref(), &secret)?;

    Ok(Some(SourceFetchAuth {
        credential_id: credential_id.to_string(),
        header_name,
        header_value,
    }))
}

fn source_credential_header_value(
    prefix: Option<&str>,
    secret: &str,
) -> Result<HeaderValue, HelixError> {
    if secret.trim().is_empty() {
        return Err(HelixError::validation_error(
            "credential.secret",
            "decrypted credential must not be empty",
        ));
    }
    let value = match prefix.map(str::trim).filter(|value| !value.is_empty()) {
        Some(prefix) => format!("{prefix} {secret}"),
        None => secret.to_string(),
    };
    HeaderValue::from_str(&value).map_err(|_| {
        HelixError::validation_error(
            "credential.secret",
            "decrypted credential cannot be represented as an HTTP header value",
        )
    })
}

fn collect_requests_from_payload(
    source: &SourceDefinition,
    payload: &str,
    fallback_observed_at: &str,
    max_items: usize,
) -> Result<Vec<IngestEvidenceRequest>, HelixError> {
    if fallback_observed_at.trim().is_empty() {
        return Err(HelixError::validation_error(
            "observed_at",
            "fallback observed_at is required",
        ));
    }
    let endpoint_url = source.endpoint_url.as_deref().ok_or_else(|| {
        HelixError::validation_error("source.endpoint_url", "source has no endpoint_url")
    })?;

    let mut requests = match source.kind {
        SourceKind::JsonApi => json_collection_requests(source, payload, fallback_observed_at)?,
        SourceKind::RssFeed => rss_collection_requests(source, payload, fallback_observed_at),
        SourceKind::WebsiteDiff => {
            website_collection_requests(source, payload, endpoint_url, fallback_observed_at)
        }
        SourceKind::WebhookIngest | SourceKind::EmailDigest | SourceKind::FileImport => {
            return Err(HelixError::validation_error(
                "source.kind",
                "source kind does not support pull collection",
            ));
        }
    };
    requests.truncate(max_items);
    Ok(requests)
}

fn normalize_collect_limit(limit: Option<usize>) -> Result<usize, HelixError> {
    let limit = limit.unwrap_or(10);
    if limit == 0 || limit > MAX_COLLECT_ITEMS {
        return Err(HelixError::validation_error(
            "max_items",
            &format!("must be between 1 and {MAX_COLLECT_ITEMS}"),
        ));
    }
    Ok(limit)
}

fn json_collection_requests(
    source: &SourceDefinition,
    payload: &str,
    fallback_observed_at: &str,
) -> Result<Vec<IngestEvidenceRequest>, HelixError> {
    let payload: JsonCollectionPayload = serde_json::from_str(payload).map_err(HelixError::from)?;
    let items = match payload {
        JsonCollectionPayload::Envelope { items } | JsonCollectionPayload::Array(items) => items,
        JsonCollectionPayload::Single(item) => vec![item],
    };
    items
        .into_iter()
        .map(|item| collected_item_to_request(source, item, fallback_observed_at))
        .collect()
}

fn collected_item_to_request(
    source: &SourceDefinition,
    item: CollectedEvidencePayload,
    fallback_observed_at: &str,
) -> Result<IngestEvidenceRequest, HelixError> {
    let title = item.title.trim();
    if title.is_empty() {
        return Err(HelixError::validation_error(
            "source.item.title",
            "title is required",
        ));
    }

    let content = first_non_empty([
        item.content.as_deref(),
        item.summary.as_deref(),
        Some(title),
    ])
    .unwrap_or(title);
    let summary = first_non_empty([item.summary.as_deref(), Some(content)]).unwrap_or(content);
    let observed_at = first_non_empty([item.observed_at.as_deref(), Some(fallback_observed_at)])
        .unwrap_or(fallback_observed_at);
    let url = item
        .url
        .or_else(|| source.endpoint_url.as_ref().map(ToString::to_string));

    Ok(IngestEvidenceRequest {
        source_id: source.id.clone(),
        title: truncate_text(title, 240),
        summary: truncate_text(summary, 1_024),
        content: truncate_text(content, MAX_COLLECT_CONTENT_LEN),
        url,
        observed_at: observed_at.trim().to_string(),
        tags: merge_source_tags(source, item.tags),
        entity_labels: item.entity_labels,
        proposed_claims: item.proposed_claims,
    })
}

fn rss_collection_requests(
    source: &SourceDefinition,
    payload: &str,
    fallback_observed_at: &str,
) -> Vec<IngestEvidenceRequest> {
    let endpoint_url = source.endpoint_url.clone();
    extract_blocks(payload, "item")
        .into_iter()
        .filter_map(|item| {
            let title = xml_tag_text(&item, "title")?;
            let description = xml_tag_text(&item, "description")
                .or_else(|| xml_tag_text(&item, "content:encoded"))
                .unwrap_or_else(|| title.clone());
            let content = strip_markup(&description);
            let link = xml_tag_text(&item, "link").or_else(|| endpoint_url.clone());
            let observed_at =
                xml_tag_text(&item, "pubDate").unwrap_or_else(|| fallback_observed_at.to_string());
            Some(IngestEvidenceRequest {
                source_id: source.id.clone(),
                title: truncate_text(&title, 240),
                summary: summarize_text(&content),
                content: truncate_text(&content, MAX_COLLECT_CONTENT_LEN),
                url: link,
                observed_at,
                tags: merge_source_tags(source, vec!["rss".to_string()]),
                entity_labels: Vec::new(),
                proposed_claims: Vec::new(),
            })
        })
        .collect()
}

fn website_collection_requests(
    source: &SourceDefinition,
    payload: &str,
    endpoint_url: &str,
    fallback_observed_at: &str,
) -> Vec<IngestEvidenceRequest> {
    let title = html_title(payload).unwrap_or_else(|| source.name.clone());
    let content = strip_markup(payload);
    vec![IngestEvidenceRequest {
        source_id: source.id.clone(),
        title: truncate_text(&title, 240),
        summary: summarize_text(&content),
        content: truncate_text(&content, MAX_COLLECT_CONTENT_LEN),
        url: Some(endpoint_url.to_string()),
        observed_at: fallback_observed_at.trim().to_string(),
        tags: merge_source_tags(source, vec!["website-diff".to_string()]),
        entity_labels: Vec::new(),
        proposed_claims: Vec::new(),
    }]
}

fn merge_source_tags(source: &SourceDefinition, tags: Vec<String>) -> Vec<String> {
    let mut merged = source.tags.clone();
    for tag in tags {
        if !merged.iter().any(|existing| existing == &tag) {
            merged.push(tag);
        }
    }
    merged
}

fn first_non_empty<const N: usize>(values: [Option<&str>; N]) -> Option<&str> {
    values
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty())
}

fn extract_blocks(input: &str, tag: &str) -> Vec<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut blocks = Vec::new();
    let mut rest = input;
    while let Some(open_start) = rest.find(&open) {
        let after_open = &rest[open_start..];
        let Some(open_end) = after_open.find('>') else {
            break;
        };
        let body_start = open_start + open_end + 1;
        let after_body_start = &rest[body_start..];
        let Some(close_start) = after_body_start.find(&close) else {
            break;
        };
        blocks.push(after_body_start[..close_start].to_string());
        rest = &after_body_start[close_start + close.len()..];
    }
    blocks
}

fn xml_tag_text(input: &str, tag: &str) -> Option<String> {
    extract_blocks(input, tag)
        .into_iter()
        .next()
        .map(|value| decode_xml_entities(strip_cdata(&value).trim()))
        .filter(|value| !value.trim().is_empty())
}

fn html_title(input: &str) -> Option<String> {
    extract_blocks(input, "title")
        .into_iter()
        .next()
        .map(|value| decode_xml_entities(strip_cdata(&value).trim()))
        .filter(|value| !value.trim().is_empty())
}

fn strip_cdata(value: &str) -> &str {
    value
        .strip_prefix("<![CDATA[")
        .and_then(|value| value.strip_suffix("]]>"))
        .unwrap_or(value)
}

fn strip_markup(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_tag = false;
    let mut previous_space = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                if !previous_space {
                    output.push(' ');
                    previous_space = true;
                }
            }
            _ if in_tag => {}
            _ if ch.is_whitespace() => {
                if !previous_space {
                    output.push(' ');
                    previous_space = true;
                }
            }
            _ => {
                output.push(ch);
                previous_space = false;
            }
        }
    }
    decode_xml_entities(output.trim())
}

fn decode_xml_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

fn summarize_text(input: &str) -> String {
    truncate_text(input, 280)
}

fn truncate_text(input: &str, max_len: usize) -> String {
    let trimmed = input.trim();
    if trimmed.len() <= max_len {
        return trimmed.to_string();
    }
    let mut end = 0;
    for (idx, _) in trimmed.char_indices() {
        if idx > max_len {
            break;
        }
        end = idx;
    }
    trimmed[..end].trim().to_string()
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

fn is_market_source(source: &SourceDefinition) -> bool {
    source
        .tags
        .iter()
        .any(|tag| tag == "market-intel" || tag == "competitor")
}

fn is_market_watchlist(watchlist: &Watchlist) -> bool {
    market_theme_id_for_watchlist(watchlist).is_some()
}

fn market_theme_descriptors() -> &'static [(&'static str, &'static str, &'static str)] {
    &[
        (
            "pricing",
            "Pricing Intelligence",
            "Detect discounting, packaging changes, and monetization pressure across the market.",
        ),
        (
            "product",
            "Product Launch Radar",
            "Track launches, betas, new SKUs, and roadmap shifts that change competitive positioning.",
        ),
        (
            "partnerships",
            "Ecosystem Mapping",
            "Monitor partner, reseller, and integration announcements to see channel momentum.",
        ),
        (
            "hiring",
            "Expansion Signals",
            "Use hiring bursts and role mix to infer GTM push, product investment, or geographic expansion.",
        ),
    ]
}

fn market_theme_id_for_watchlist(watchlist: &Watchlist) -> Option<&'static str> {
    let keywords = watchlist.keywords.join(" ");
    let id = watchlist.id.as_str();
    if id.contains("pricing")
        || ["pricing", "discount", "bundle", "seat"]
            .iter()
            .any(|needle| keywords.contains(needle))
    {
        Some("pricing")
    } else if id.contains("launch")
        || ["launch", "release", "beta", "ga"]
            .iter()
            .any(|needle| keywords.contains(needle))
    {
        Some("product")
    } else if id.contains("partnership")
        || ["partnership", "integration", "reseller", "ecosystem"]
            .iter()
            .any(|needle| keywords.contains(needle))
    {
        Some("partnerships")
    } else if id.contains("hiring")
        || ["hiring", "headcount", "recruiting", "open role"]
            .iter()
            .any(|needle| keywords.contains(needle))
    {
        Some("hiring")
    } else {
        None
    }
}

fn market_theme_name(theme_id: &'static str) -> &'static str {
    market_theme_descriptors()
        .iter()
        .find_map(|(id, name, _)| (*id == theme_id).then_some(*name))
        .unwrap_or("Unclassified")
}

fn market_intelligence_playbooks() -> Vec<MarketIntelPlaybook> {
    vec![
        MarketIntelPlaybook {
            id: "competitor_pricing_watch".to_string(),
            name: "Competitor Pricing Watch".to_string(),
            objective: "Track packaging, discounting, and seat-level monetization shifts before renewal pressure lands."
                .to_string(),
            signals: vec![
                "pricing page diffs".to_string(),
                "discount language".to_string(),
                "seat or bundle changes".to_string(),
            ],
        },
        MarketIntelPlaybook {
            id: "launch_radar".to_string(),
            name: "Launch Radar".to_string(),
            objective: "Capture launch, beta, and release signals that change category positioning or feature parity."
                .to_string(),
            signals: vec![
                "release notes".to_string(),
                "landing page diffs".to_string(),
                "SKU announcements".to_string(),
            ],
        },
        MarketIntelPlaybook {
            id: "partner_ecosystem_map".to_string(),
            name: "Partner Ecosystem Map".to_string(),
            objective: "Measure channel strength through reseller, integration, and alliance announcements."
                .to_string(),
            signals: vec![
                "integration posts".to_string(),
                "reseller pages".to_string(),
                "ecosystem announcements".to_string(),
            ],
        },
        MarketIntelPlaybook {
            id: "gtm_expansion_signals".to_string(),
            name: "GTM Expansion Signals".to_string(),
            objective: "Infer territory, segment, and product-line investment from hiring velocity and role mix."
                .to_string(),
            signals: vec![
                "sales hiring".to_string(),
                "channel roles".to_string(),
                "regional expansion postings".to_string(),
            ],
        },
    ]
}

fn summarize_key_claims(claims: &[ClaimRecord]) -> Vec<String> {
    let mut ranked = claims.to_vec();
    ranked.sort_by(|left, right| {
        right
            .confidence_bps
            .cmp(&left.confidence_bps)
            .then(left.subject.cmp(&right.subject))
            .then(left.predicate.cmp(&right.predicate))
            .then(left.object.cmp(&right.object))
    });
    ranked
        .into_iter()
        .take(3)
        .map(|claim| format!("{} {} {}", claim.subject, claim.predicate, claim.object))
        .collect()
}

fn claim_review_metrics(claims: &[ClaimRecord]) -> (usize, usize, u16) {
    let corroborated = claims
        .iter()
        .filter(|claim| claim.review_status == ClaimReviewStatus::Corroborated)
        .count();
    let rejected = claims
        .iter()
        .filter(|claim| claim.review_status == ClaimReviewStatus::Rejected)
        .count();
    let max_confidence_bps = claims
        .iter()
        .map(|claim| claim.confidence_bps)
        .max()
        .unwrap_or(0);
    (corroborated, rejected, max_confidence_bps)
}

fn review_kind_label(kind: AutopilotReviewKind) -> &'static str {
    match kind {
        AutopilotReviewKind::Case => "case",
        AutopilotReviewKind::Claim => "claim",
        AutopilotReviewKind::Evidence => "evidence",
    }
}

fn autopilot_review_kind_rank(kind: AutopilotReviewKind) -> u8 {
    match kind {
        AutopilotReviewKind::Case => 0,
        AutopilotReviewKind::Claim => 1,
        AutopilotReviewKind::Evidence => 2,
    }
}

fn case_goal_hint(case: &CaseFile, watchlist_name: &str) -> String {
    let entity = case
        .primary_entity
        .as_deref()
        .unwrap_or("unassigned entity");
    let status = match case.status {
        CaseStatus::Open => "open",
        CaseStatus::Monitoring => "monitoring",
        CaseStatus::BriefReady => "brief_ready",
        CaseStatus::Escalated => "escalated",
        CaseStatus::Closed => "closed",
    };
    format!(
        "Review the {} case '{}' for {} on watchlist '{}'. Validate the linked evidence, decide whether to keep monitoring, escalate, or attach a brief, and keep the proposal bounded to deterministic follow-up steps.",
        status, case.title, entity, watchlist_name
    )
}

fn claim_goal_hint(claim: &ClaimRecord, evidence_title: &str) -> String {
    format!(
        "Review the claim '{} {} {}' from evidence '{}'. Propose only deterministic next steps for corroboration, rejection, or case linkage with cited evidence.",
        claim.subject, claim.predicate, claim.object, evidence_title
    )
}

fn evidence_goal_hint(evidence: &EvidenceItem, linked_case_count: usize) -> String {
    let entity = evidence
        .entity_labels
        .first()
        .map(String::as_str)
        .unwrap_or("tracked entity");
    format!(
        "Triage evidence '{}' observed at {} for {}. Linked cases: {}. Propose a bounded deterministic follow-up such as claim review, case escalation, watchlist refinement, or briefing preparation.",
        evidence.title, evidence.observed_at, entity, linked_case_count
    )
}

fn build_review_export_narrative(
    item: &AutopilotReviewQueueEntry,
    cases: &[CaseQueueEntry],
    claims: &[ClaimQueueEntry],
    evidence: &[EvidenceQueueEntry],
) -> String {
    format!(
        "{} export packet for '{}'. Supporting cases: {}. Supporting claims: {}. Supporting evidence: {}.",
        review_kind_label(item.kind),
        item.title,
        cases.len(),
        claims.len(),
        evidence.len()
    )
}

struct SemanticRanker {
    generator: EmbeddingGenerator,
    query_vector: Vec<f32>,
}

impl SemanticRanker {
    fn new(query: &str) -> Result<Self, HelixError> {
        let generator = EmbeddingGenerator;
        let query_vector = generator
            .generate_text_embedding(query)
            .map_err(semantic_embedding_error)?;
        Ok(Self {
            generator,
            query_vector,
        })
    }

    fn score_bps(&self, document: &str) -> Result<i32, HelixError> {
        let document_vector = self
            .generator
            .generate_text_embedding(document)
            .map_err(semantic_embedding_error)?;
        let score = cosine_similarity(&self.query_vector, &document_vector)
            .map_err(semantic_embedding_error)?;
        Ok((score.clamp(-1.0, 1.0) * 10_000.0).round() as i32)
    }
}

fn evidence_semantic_document(entry: &EvidenceQueueEntry) -> String {
    format!(
        "{} {} {} {} {} {} trust:{} cases:{} claims:{}",
        entry.evidence.title,
        entry.evidence.summary,
        entry.evidence.content,
        entry.evidence.tags.join(" "),
        entry.evidence.entity_labels.join(" "),
        entry.source_name,
        entry.source_trust_score,
        entry.linked_case_count,
        entry.linked_claim_count
    )
}

fn claim_semantic_document(entry: &ClaimQueueEntry) -> String {
    format!(
        "{} {} {} {} {} {} {} confidence:{} status:{} cases:{}",
        entry.claim.subject,
        entry.claim.predicate,
        entry.claim.object,
        entry.claim.rationale,
        entry.evidence_title,
        entry.evidence_observed_at,
        entry.source_name,
        entry.claim.confidence_bps,
        json_string(&entry.claim.review_status),
        entry.linked_case_count
    )
}

fn semantic_embedding_error(error: helix_embeddings::EmbeddingError) -> HelixError {
    HelixError::InternalError(format!("semantic retrieval error: {error}"))
}

fn normalized_limit(limit: Option<usize>, subject: &str) -> Result<Option<usize>, HelixError> {
    match limit {
        None => Ok(None),
        Some(0) => Err(HelixError::validation_error(
            "limit".to_string(),
            format!("{subject} limit must be between 1 and 100"),
        )),
        Some(limit) if limit > 100 => Err(HelixError::validation_error(
            "limit".to_string(),
            format!("{subject} limit must be between 1 and 100"),
        )),
        Some(limit) => Ok(Some(limit)),
    }
}

fn normalized_semantic_query(
    value: Option<&str>,
    field: &str,
) -> Result<Option<String>, HelixError> {
    match value.map(str::trim) {
        None | Some("") => Ok(None),
        Some(value) if value.len() > MAX_SEMANTIC_QUERY_LEN => Err(HelixError::validation_error(
            field.to_string(),
            format!("semantic query must be at most {MAX_SEMANTIC_QUERY_LEN} bytes"),
        )),
        Some(value) if !value.chars().any(char::is_alphanumeric) => {
            Err(HelixError::validation_error(
                field.to_string(),
                "semantic query must contain at least one alphanumeric character".to_string(),
            ))
        }
        Some(value) => Ok(Some(value.to_string())),
    }
}

fn normalized_trust_score(value: Option<u8>) -> Result<Option<u8>, HelixError> {
    match value {
        None => Ok(None),
        Some(score) if score > 100 => Err(HelixError::validation_error(
            "min_trust",
            "min_trust must be between 0 and 100",
        )),
        Some(score) => Ok(Some(score)),
    }
}

fn normalized_confidence_bps(value: Option<u16>) -> Result<Option<u16>, HelixError> {
    match value {
        None => Ok(None),
        Some(score) if score > 10_000 => Err(HelixError::validation_error(
            "min_confidence_bps",
            "min_confidence_bps must be between 0 and 10000",
        )),
        Some(score) => Ok(Some(score)),
    }
}

fn normalized_optional_filter(
    value: Option<&str>,
    _field: &str,
) -> Result<Option<String>, HelixError> {
    match value.map(str::trim) {
        None | Some("") => Ok(None),
        Some(value) => Ok(Some(value.to_string())),
    }
}

fn latest_signal_at(evidence: &[EvidenceItem]) -> Option<String> {
    evidence.iter().map(|item| item.observed_at.clone()).max()
}

fn build_market_case_summary(
    case: &CaseFile,
    theme_name: &str,
    latest_signal_at: Option<&str>,
    latest_titles: &[String],
    key_claims: &[String],
) -> String {
    let company = case.primary_entity.as_deref().unwrap_or("tracked company");
    let timing = latest_signal_at.unwrap_or("unknown_time");
    let title_context = if latest_titles.is_empty() {
        "no evidence titles captured".to_string()
    } else {
        latest_titles.join("; ")
    };
    let claims = if key_claims.is_empty() {
        "no high-confidence claims yet".to_string()
    } else {
        key_claims.join("; ")
    };

    format!(
        "{} signal for {} at {}. Recent evidence: {}. Key claims: {}.",
        theme_name, company, timing, title_context, claims
    )
}

fn market_brief_actions(theme_id: &str) -> Vec<String> {
    match theme_id {
        "pricing" => vec![
            "compare current packaging against previous snapshot".to_string(),
            "brief account team on renewal pressure".to_string(),
            "record monetization deltas in competitor dossier".to_string(),
        ],
        "product" => vec![
            "map launch claims against current product parity".to_string(),
            "brief product and field teams on positioning impact".to_string(),
            "capture supporting release-note evidence".to_string(),
        ],
        "partnerships" => vec![
            "update ecosystem map and partner overlap notes".to_string(),
            "assess channel displacement or expansion risk".to_string(),
            "brief alliance team with cited evidence".to_string(),
        ],
        "hiring" => vec![
            "track role clusters for territory or segment expansion".to_string(),
            "update GTM expansion hypothesis with cited evidence".to_string(),
            "brief field leadership on hiring velocity changes".to_string(),
        ],
        _ => vec![
            "review case evidence and claims".to_string(),
            "decide whether to escalate or attach a brief".to_string(),
        ],
    }
}

fn briefing_text(briefing: &MarketIntelCaseBrief) -> String {
    format!(
        "{} | key_claims: {} | actions: {}",
        briefing.summary,
        if briefing.key_claims.is_empty() {
            "none".to_string()
        } else {
            briefing.key_claims.join("; ")
        },
        briefing.recommended_actions.join("; ")
    )
}

async fn load_records<T>(pool: &PgPool, table: &str) -> Result<BTreeMap<String, T>, HelixError>
where
    T: DeserializeOwned + HasIntelRecordId,
{
    let sql = format!("SELECT record FROM {table} ORDER BY id");
    let rows = sqlx::query(&sql).fetch_all(pool).await.map_err(db_error)?;
    let mut records = BTreeMap::new();
    for row in rows {
        let record_value: serde_json::Value = row.try_get("record").map_err(db_error)?;
        let record: T = serde_json::from_value(record_value).map_err(serde_error)?;
        records.insert(record.record_id().to_string(), record);
    }
    Ok(records)
}

trait HasIntelRecordId {
    fn record_id(&self) -> &str;
}

impl HasIntelRecordId for SourceDefinition {
    fn record_id(&self) -> &str {
        &self.id
    }
}

impl HasIntelRecordId for Watchlist {
    fn record_id(&self) -> &str {
        &self.id
    }
}

impl HasIntelRecordId for EvidenceItem {
    fn record_id(&self) -> &str {
        &self.id
    }
}

impl HasIntelRecordId for ClaimRecord {
    fn record_id(&self) -> &str {
        &self.id
    }
}

impl HasIntelRecordId for CaseFile {
    fn record_id(&self) -> &str {
        &self.id
    }
}

fn json_string<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

fn db_error(error: sqlx::Error) -> HelixError {
    HelixError::InternalError(format!("intel desk persistence error: {error}"))
}

fn serde_error(error: serde_json::Error) -> HelixError {
    HelixError::InternalError(format!("intel desk serialization error: {error}"))
}

async fn mutate_intel_desk<T>(
    state: &AppState,
    mutation: impl FnOnce(&mut IntelDeskStore) -> Result<T, HelixError>,
) -> Result<T, HelixError> {
    let persistence = state.intel_persistence.clone();
    let mut store = state.intel_desk.write().await;
    let rollback = persistence.as_ref().map(|_| store.clone());
    let result = mutation(&mut store)?;

    if let Some(persistence) = persistence {
        if let Err(error) = persistence.save(&store).await {
            if let Some(rollback) = rollback {
                *store = rollback;
            }
            return Err(error);
        }
    }

    Ok(result)
}

pub(crate) async fn get_intel_overview(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.intel_desk.read().await;
    (StatusCode::OK, Json(store.overview()))
}

pub(crate) async fn get_market_intel_overview(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.intel_desk.read().await;
    (StatusCode::OK, Json(store.market_intelligence_overview()))
}

pub(crate) async fn generate_market_intel_brief_handler(
    State(state): State<AppState>,
    Path(case_id): Path<String>,
    Json(request): Json<GenerateMarketIntelBriefRequest>,
) -> Response {
    let result = mutate_intel_desk(&state, |store| {
        store.generate_market_brief(&case_id, request.attach_to_case)
    })
    .await;
    match result {
        Ok(response) => {
            if let Err(error) = record_audit_event(
                &state,
                AuditEvent::allow(
                    "intel.market_brief.generate",
                    format!("cases/{case_id}/brief"),
                    serde_json::json!({
                        "case_id": case_id,
                        "attach_to_case": request.attach_to_case,
                        "transition": response.transition,
                    }),
                ),
            )
            .await
            {
                return api_error_response(error);
            }
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(error) => api_error_response(error),
    }
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
    let result = mutate_intel_desk(&state, |store| store.create_source(request)).await;
    match result {
        Ok(source) => {
            if let Err(error) = record_audit_event(
                &state,
                AuditEvent::allow(
                    "intel.source.create",
                    format!("sources/{}", source.id),
                    serde_json::json!({
                        "source_id": source.id,
                        "profile_id": source.profile_id,
                        "kind": source.kind,
                        "endpoint_url": source.endpoint_url,
                        "credential_id": source.credential_id,
                        "credential_header_name": source.credential_header_name,
                        "enabled": source.enabled,
                    }),
                ),
            )
            .await
            {
                return api_error_response(error);
            }
            (StatusCode::CREATED, Json(SourceResponse { source })).into_response()
        }
        Err(error) => api_error_response(error),
    }
}

pub(crate) async fn collect_source_handler(
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    Json(request): Json<CollectSourceRequest>,
) -> Response {
    let limit = match normalize_collect_limit(request.max_items) {
        Ok(limit) => limit,
        Err(error) => return api_error_response(error),
    };
    let source = {
        let store = state.intel_desk.read().await;
        match store.sources.get(&source_id).cloned() {
            Some(source) => source,
            None => {
                return api_error_response(HelixError::not_found(format!("source {source_id}")))
            }
        }
    };
    if !source.enabled {
        return api_error_response(HelixError::validation_error("source", "source is disabled"));
    }
    let endpoint_url = match source.endpoint_url.clone() {
        Some(endpoint_url) => endpoint_url,
        None => {
            return api_error_response(HelixError::validation_error(
                "source.endpoint_url",
                "source has no endpoint_url",
            ));
        }
    };
    let fetch_auth = match source_fetch_auth(&state, &source).await {
        Ok(fetch_auth) => fetch_auth,
        Err(error) => return api_error_response(error),
    };
    let payload = match fetch_source_body(&endpoint_url, fetch_auth.as_ref()).await {
        Ok(payload) => payload,
        Err(error) => return api_error_response(error),
    };
    let requests =
        match collect_requests_from_payload(&source, &payload, &request.observed_at, limit) {
            Ok(requests) => requests,
            Err(error) => return api_error_response(error),
        };
    if requests.is_empty() {
        return api_error_response(HelixError::validation_error(
            "source.payload",
            "source payload produced no evidence",
        ));
    }

    let result = mutate_intel_desk(&state, |store| {
        requests
            .into_iter()
            .map(|request| store.ingest_evidence(request))
            .collect::<Result<Vec<_>, _>>()
    })
    .await;

    match result {
        Ok(results) => {
            let duplicate_count = results.iter().filter(|result| result.duplicate).count();
            let collected_count = results.len();
            if let Err(error) = record_audit_event(
                &state,
                AuditEvent::allow(
                    "intel.source.collect",
                    format!("sources/{}/collect", source.id),
                    serde_json::json!({
                        "source_id": source.id,
                        "fetched_url": endpoint_url,
                        "credential_id": fetch_auth.as_ref().map(|auth| auth.credential_id.as_str()),
                        "collected_count": collected_count,
                        "duplicate_count": duplicate_count,
                    }),
                ),
            )
            .await
            {
                return api_error_response(error);
            }
            (
                StatusCode::CREATED,
                Json(CollectSourceResponse {
                    source,
                    fetched_url: endpoint_url,
                    collected_count,
                    duplicate_count,
                    results,
                }),
            )
                .into_response()
        }
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
    let result = mutate_intel_desk(&state, |store| store.create_watchlist(request)).await;
    match result {
        Ok(watchlist) => {
            if let Err(error) = record_audit_event(
                &state,
                AuditEvent::allow(
                    "intel.watchlist.create",
                    format!("watchlists/{}", watchlist.id),
                    serde_json::json!({
                        "watchlist_id": watchlist.id,
                        "severity": watchlist.severity,
                        "min_source_trust": watchlist.min_source_trust,
                        "enabled": watchlist.enabled,
                    }),
                ),
            )
            .await
            {
                return api_error_response(error);
            }
            (StatusCode::CREATED, Json(WatchlistResponse { watchlist })).into_response()
        }
        Err(error) => api_error_response(error),
    }
}

pub(crate) async fn list_evidence(
    State(state): State<AppState>,
    Query(filters): Query<EvidenceQueueFilterQuery>,
) -> Response {
    let store = state.intel_desk.read().await;
    match store.evidence_queue(&filters) {
        Ok(evidence) => {
            (StatusCode::OK, Json(EvidenceCatalogResponse { evidence })).into_response()
        }
        Err(error) => api_error_response(error),
    }
}

pub(crate) async fn list_claims(
    State(state): State<AppState>,
    Query(filters): Query<ClaimQueueFilterQuery>,
) -> Response {
    let store = state.intel_desk.read().await;
    match store.claim_queue(&filters) {
        Ok(claims) => (StatusCode::OK, Json(ClaimCatalogResponse { claims })).into_response(),
        Err(error) => api_error_response(error),
    }
}

pub(crate) async fn review_claim_handler(
    State(state): State<AppState>,
    Path(claim_id): Path<String>,
    Json(request): Json<ClaimReviewRequest>,
) -> Response {
    let result = mutate_intel_desk(&state, |store| {
        store.review_claim(&claim_id, request.status)
    })
    .await;
    match result {
        Ok(claim) => {
            if let Err(error) = record_audit_event(
                &state,
                AuditEvent::allow(
                    "intel.claim.review",
                    format!("claims/{claim_id}/review"),
                    serde_json::json!({
                        "claim_id": claim.id,
                        "review_status": claim.review_status,
                        "evidence_id": claim.evidence_id,
                    }),
                ),
            )
            .await
            {
                return api_error_response(error);
            }
            (StatusCode::OK, Json(ClaimResponse { claim })).into_response()
        }
        Err(error) => api_error_response(error),
    }
}

pub(crate) async fn ingest_evidence(
    State(state): State<AppState>,
    Json(request): Json<IngestEvidenceRequest>,
) -> Response {
    let result = mutate_intel_desk(&state, |store| store.ingest_evidence(request)).await;
    match result {
        Ok(response) => {
            if let Err(error) = record_audit_event(
                &state,
                AuditEvent::allow(
                    "intel.evidence.ingest",
                    format!("evidence/{}", response.evidence.id),
                    serde_json::json!({
                        "evidence_id": response.evidence.id,
                        "source_id": response.evidence.source_id,
                        "duplicate": response.duplicate,
                        "claim_count": response.claims.len(),
                        "hit_count": response.hits.len(),
                        "case_update_count": response.case_updates.len(),
                    }),
                ),
            )
            .await
            {
                return api_error_response(error);
            }
            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(error) => api_error_response(error),
    }
}

pub(crate) async fn list_cases(
    State(state): State<AppState>,
    Query(filters): Query<CaseQueueFilterQuery>,
) -> Response {
    let store = state.intel_desk.read().await;
    match store.case_queue(&filters) {
        Ok(cases) => (StatusCode::OK, Json(CaseCatalogResponse { cases })).into_response(),
        Err(error) => api_error_response(error),
    }
}

pub(crate) async fn get_autopilot_review_queue(
    State(state): State<AppState>,
    Query(filters): Query<AutopilotReviewQueueQuery>,
) -> Response {
    let store = state.intel_desk.read().await;
    match store.autopilot_review_queue(&filters) {
        Ok(items) => (StatusCode::OK, Json(AutopilotReviewQueueResponse { items })).into_response(),
        Err(error) => api_error_response(error),
    }
}

pub(crate) async fn export_autopilot_review_packet(
    State(state): State<AppState>,
    Query(query): Query<AutopilotReviewExportQuery>,
) -> Response {
    if query.item_id.trim().is_empty() {
        return api_error_response(HelixError::validation_error(
            "item_id",
            "item_id is required",
        ));
    }

    let store = state.intel_desk.read().await;
    match store.build_review_export_packet(query.review_kind, query.item_id.trim()) {
        Ok(packet) => (StatusCode::OK, Json(packet)).into_response(),
        Err(error) => api_error_response(error),
    }
}

pub(crate) async fn transition_case_handler(
    State(state): State<AppState>,
    Path(case_id): Path<String>,
    Json(request): Json<CaseTransitionRequest>,
) -> Response {
    let result = mutate_intel_desk(&state, |store| {
        store.transition_case(&case_id, request.command)
    })
    .await;
    match result {
        Ok(transition) => {
            if let Err(error) = record_audit_event(
                &state,
                AuditEvent::allow(
                    "intel.case.transition",
                    format!("cases/{case_id}/transition"),
                    serde_json::json!({
                        "case_id": transition.case.id,
                        "status": transition.case.status,
                        "decision": transition.decision,
                    }),
                ),
            )
            .await
            {
                return api_error_response(error);
            }
            (StatusCode::OK, Json(CaseTransitionResponse { transition })).into_response()
        }
        Err(error) => api_error_response(error),
    }
}

pub(crate) async fn export_market_brief_packet_handler(
    State(state): State<AppState>,
    Path(case_id): Path<String>,
) -> Response {
    let store = state.intel_desk.read().await;
    match store.build_market_brief_export_packet(&case_id) {
        Ok(packet) => (StatusCode::OK, Json(packet)).into_response(),
        Err(error) => api_error_response(error),
    }
}
