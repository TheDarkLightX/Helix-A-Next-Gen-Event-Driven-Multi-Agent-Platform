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
use std::collections::{BTreeMap, BTreeSet};

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
    pub(crate) watchlist_count: usize,
    pub(crate) evidence_count: usize,
    pub(crate) active_case_count: usize,
    pub(crate) escalated_case_count: usize,
    pub(crate) top_entities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MarketIntelCompanyCard {
    pub(crate) company: String,
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
    pub(crate) playbooks: Vec<MarketIntelPlaybook>,
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
            SourceDefinition {
                id: "json_api_cloud_pricing".to_string(),
                name: "Cloud Pricing API".to_string(),
                description: "Normalized pricing snapshots for competitor packaging, discounting, and seat changes."
                    .to_string(),
                kind: SourceKind::JsonApi,
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
                name: "Vector Works Release Diff".to_string(),
                description: "Website diff feed for product launches, beta announcements, and packaging changes."
                    .to_string(),
                kind: SourceKind::WebsiteDiff,
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
                name: "Partner Ecosystem Feed".to_string(),
                description: "Partnership, channel, and ecosystem signal feed for market mapping."
                    .to_string(),
                kind: SourceKind::RssFeed,
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
                name: "GTM Hiring Tracker".to_string(),
                description: "Hiring and expansion signal feed for sales, success, and channel roles."
                    .to_string(),
                kind: SourceKind::RssFeed,
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

        let theme_cards = market_theme_descriptors()
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

                MarketIntelThemeCard {
                    theme_id: (*theme_id).to_string(),
                    name: (*name).to_string(),
                    summary: (*summary).to_string(),
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

        let company_cards = tracked_companies
            .iter()
            .map(|company| {
                let mention_count = self
                    .evidence
                    .values()
                    .filter(|evidence| market_source_ids.contains(&evidence.source_id))
                    .filter(|evidence| evidence.entity_labels.iter().any(|label| label == company))
                    .count();
                let claim_count = self
                    .claims
                    .values()
                    .filter(|claim| {
                        self.evidence
                            .get(&claim.evidence_id)
                            .map(|evidence| market_source_ids.contains(&evidence.source_id))
                            .unwrap_or(false)
                            && (claim.subject == *company || claim.object == *company)
                    })
                    .count();
                let company_cases = active_market_cases
                    .iter()
                    .filter(|case| case.primary_entity.as_deref() == Some(company.as_str()))
                    .collect::<Vec<_>>();
                let latest_signal_at = self
                    .evidence
                    .values()
                    .filter(|evidence| market_source_ids.contains(&evidence.source_id))
                    .filter(|evidence| evidence.entity_labels.iter().any(|label| label == company))
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
                            .filter(|watchlist| watchlist.entities.iter().any(|entity| entity == company))
                            .filter_map(|watchlist| market_theme_id_for_watchlist(watchlist))
                            .map(market_theme_name),
                    )
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .map(str::to_string)
                    .collect::<Vec<_>>();

                MarketIntelCompanyCard {
                    company: company.clone(),
                    mention_count,
                    claim_count,
                    active_case_count: company_cases.len(),
                    themes,
                    latest_signal_at,
                }
            })
            .filter(|card| !card.themes.is_empty() || card.mention_count > 0)
            .take(6)
            .collect::<Vec<_>>();

        MarketIntelOverviewResponse {
            market_source_count: market_sources.len(),
            market_watchlist_count: market_watchlists.len(),
            tracked_company_count: tracked_companies.len(),
            active_case_count: active_market_cases.len(),
            theme_cards,
            company_cards,
            playbooks: market_intelligence_playbooks(),
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

pub(crate) async fn get_intel_overview(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.intel_desk.read().await;
    (StatusCode::OK, Json(store.overview()))
}

pub(crate) async fn get_market_intel_overview(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.intel_desk.read().await;
    (StatusCode::OK, Json(store.market_intelligence_overview()))
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
