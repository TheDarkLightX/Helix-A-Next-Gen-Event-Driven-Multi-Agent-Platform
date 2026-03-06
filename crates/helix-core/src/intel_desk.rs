use crate::HelixError;
use serde::{Deserialize, Serialize};

const MAX_TAGS: usize = 16;
const MAX_ENTITIES: usize = 16;
const MAX_KEYWORDS: usize = 16;
const MAX_TEXT_LEN: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    RssFeed,
    WebsiteDiff,
    JsonApi,
    WebhookIngest,
    EmailDigest,
    FileImport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: SourceKind,
    pub cadence_minutes: u16,
    pub trust_score: u8,
    pub enabled: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceDraft {
    pub source_id: String,
    pub title: String,
    pub summary: String,
    pub content: String,
    pub url: Option<String>,
    pub observed_at: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub entity_labels: Vec<String>,
    #[serde(default)]
    pub proposed_claims: Vec<ProposedClaim>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceItem {
    pub id: String,
    pub source_id: String,
    pub title: String,
    pub summary: String,
    pub content: String,
    pub url: Option<String>,
    pub observed_at: String,
    pub tags: Vec<String>,
    pub entity_labels: Vec<String>,
    pub provenance_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedClaim {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence_bps: u16,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimReviewStatus {
    NeedsReview,
    Corroborated,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimRecord {
    pub id: String,
    pub evidence_id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence_bps: u16,
    pub review_status: ClaimReviewStatus,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchlistSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl WatchlistSeverity {
    pub fn weight(self) -> u8 {
        match self {
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
            Self::Critical => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Watchlist {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub entities: Vec<String>,
    pub min_source_trust: u8,
    pub severity: WatchlistSeverity,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchlistHit {
    pub watchlist_id: String,
    pub watchlist_name: String,
    pub evidence_id: String,
    pub severity: WatchlistSeverity,
    pub matched_keywords: Vec<String>,
    pub matched_entities: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseStatus {
    Open,
    Monitoring,
    BriefReady,
    Escalated,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseFile {
    pub id: String,
    pub title: String,
    pub watchlist_id: String,
    pub status: CaseStatus,
    pub primary_entity: Option<String>,
    pub evidence_ids: Vec<String>,
    pub claim_ids: Vec<String>,
    pub latest_reason: String,
    pub briefing_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CaseCommand {
    Open {
        case_id: String,
        title: String,
        watchlist_id: String,
        primary_entity: Option<String>,
        evidence_id: String,
        claim_ids: Vec<String>,
        reason: String,
    },
    AppendEvidence {
        evidence_id: String,
        claim_ids: Vec<String>,
        reason: String,
    },
    MarkMonitoring,
    AttachBrief {
        summary: String,
    },
    Escalate {
        reason: String,
    },
    Close,
    Reopen {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CaseDecision {
    Opened,
    Updated,
    StatusChanged { status: CaseStatus },
    Denied { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseTransition {
    pub case: CaseFile,
    pub decision: CaseDecision,
}

pub fn canonicalize_source(source: SourceDefinition) -> Result<SourceDefinition, HelixError> {
    validate_identifier("source.id", &source.id)?;
    validate_non_empty("source.name", &source.name)?;
    let description = clamp_trimmed_text("source.description", &source.description, 512)?;
    let tags = normalize_list(source.tags, MAX_TAGS, true, "source.tags")?;
    if source.cadence_minutes == 0 {
        return Err(HelixError::validation_error(
            "source.cadence_minutes",
            "must be greater than zero",
        ));
    }
    if source.trust_score > 100 {
        return Err(HelixError::validation_error(
            "source.trust_score",
            "must be between 0 and 100",
        ));
    }

    Ok(SourceDefinition {
        id: source.id.trim().to_string(),
        name: source.name.trim().to_string(),
        description,
        kind: source.kind,
        cadence_minutes: source.cadence_minutes,
        trust_score: source.trust_score,
        enabled: source.enabled,
        tags,
    })
}

pub fn canonicalize_watchlist(watchlist: Watchlist) -> Result<Watchlist, HelixError> {
    validate_identifier("watchlist.id", &watchlist.id)?;
    validate_non_empty("watchlist.name", &watchlist.name)?;
    let description = clamp_trimmed_text("watchlist.description", &watchlist.description, 512)?;
    let keywords = normalize_list(watchlist.keywords, MAX_KEYWORDS, true, "watchlist.keywords")?;
    let entities = normalize_list(watchlist.entities, MAX_ENTITIES, true, "watchlist.entities")?;

    if keywords.is_empty() && entities.is_empty() {
        return Err(HelixError::validation_error(
            "watchlist",
            "keywords or entities must be provided",
        ));
    }
    if watchlist.min_source_trust > 100 {
        return Err(HelixError::validation_error(
            "watchlist.min_source_trust",
            "must be between 0 and 100",
        ));
    }

    Ok(Watchlist {
        id: watchlist.id.trim().to_string(),
        name: watchlist.name.trim().to_string(),
        description,
        keywords,
        entities,
        min_source_trust: watchlist.min_source_trust,
        severity: watchlist.severity,
        enabled: watchlist.enabled,
    })
}

pub fn canonicalize_evidence(
    evidence_id: String,
    provenance_hash: String,
    draft: EvidenceDraft,
) -> Result<EvidenceItem, HelixError> {
    validate_identifier("evidence.id", &evidence_id)?;
    validate_identifier("evidence.source_id", &draft.source_id)?;
    validate_non_empty("evidence.title", &draft.title)?;
    validate_non_empty("evidence.observed_at", &draft.observed_at)?;
    let title = clamp_trimmed_text("evidence.title", &draft.title, 240)?;
    let summary = clamp_trimmed_text("evidence.summary", &draft.summary, 1_024)?;
    let content = clamp_trimmed_text("evidence.content", &draft.content, MAX_TEXT_LEN)?;
    let tags = normalize_list(draft.tags, MAX_TAGS, true, "evidence.tags")?;
    let entity_labels = normalize_list(
        draft.entity_labels,
        MAX_ENTITIES,
        true,
        "evidence.entity_labels",
    )?;

    if summary.is_empty() && content.is_empty() {
        return Err(HelixError::validation_error(
            "evidence",
            "summary or content must be present",
        ));
    }

    Ok(EvidenceItem {
        id: evidence_id.trim().to_string(),
        source_id: draft.source_id.trim().to_string(),
        title,
        summary,
        content,
        url: normalize_optional_text(draft.url, 512, "evidence.url")?,
        observed_at: draft.observed_at.trim().to_string(),
        tags,
        entity_labels,
        provenance_hash,
    })
}

pub fn canonicalize_claims(
    evidence: &EvidenceItem,
    proposed_claims: Vec<ProposedClaim>,
) -> Result<Vec<ProposedClaim>, HelixError> {
    let claims = if proposed_claims.is_empty() {
        derive_claims_from_entities(evidence)
    } else {
        proposed_claims
    };

    let mut canonical = Vec::new();
    for claim in claims {
        validate_non_empty("claim.subject", &claim.subject)?;
        validate_non_empty("claim.predicate", &claim.predicate)?;
        validate_non_empty("claim.object", &claim.object)?;
        if claim.confidence_bps > 10_000 {
            return Err(HelixError::validation_error(
                "claim.confidence_bps",
                "must be <= 10000",
            ));
        }
        let rationale = claim
            .rationale
            .as_ref()
            .map(|value| clamp_trimmed_text("claim.rationale", value, 512))
            .transpose()?;
        canonical.push(ProposedClaim {
            subject: claim.subject.trim().to_lowercase(),
            predicate: claim.predicate.trim().to_lowercase(),
            object: claim.object.trim().to_lowercase(),
            confidence_bps: claim.confidence_bps,
            rationale,
        });
    }

    canonical.sort_by(|left, right| {
        (&left.subject, &left.predicate, &left.object).cmp(&(&right.subject, &right.predicate, &right.object))
    });
    canonical.dedup_by(|left, right| {
        left.subject == right.subject
            && left.predicate == right.predicate
            && left.object == right.object
    });

    Ok(canonical)
}

pub fn evaluate_watchlists(
    source: &SourceDefinition,
    evidence: &EvidenceItem,
    claims: &[ClaimRecord],
    watchlists: &[Watchlist],
) -> Vec<WatchlistHit> {
    let searchable = format!(
        "{}\n{}\n{}\n{}",
        evidence.title, evidence.summary, evidence.content, evidence.tags.join(" ")
    )
    .to_lowercase();

    let mut hits: Vec<WatchlistHit> = watchlists
        .iter()
        .filter(|watchlist| watchlist.enabled)
        .filter(|_| source.enabled)
        .filter(|watchlist| source.trust_score >= watchlist.min_source_trust)
        .filter_map(|watchlist| {
            let mut matched_keywords: Vec<String> = watchlist
                .keywords
                .iter()
                .filter(|keyword| searchable.contains(keyword.as_str()))
                .cloned()
                .collect();
            matched_keywords.sort();
            matched_keywords.dedup();

            let mut matched_entities: Vec<String> = watchlist
                .entities
                .iter()
                .filter(|entity| entity_matches(entity, evidence, claims))
                .cloned()
                .collect();
            matched_entities.sort();
            matched_entities.dedup();

            if matched_keywords.is_empty() && matched_entities.is_empty() {
                return None;
            }

            let reason = match (!matched_keywords.is_empty(), !matched_entities.is_empty()) {
                (true, true) => "keyword_and_entity_match",
                (true, false) => "keyword_match",
                (false, true) => "entity_match",
                (false, false) => unreachable!(),
            };

            Some(WatchlistHit {
                watchlist_id: watchlist.id.clone(),
                watchlist_name: watchlist.name.clone(),
                evidence_id: evidence.id.clone(),
                severity: watchlist.severity,
                matched_keywords,
                matched_entities,
                reason: reason.to_string(),
            })
        })
        .collect();

    hits.sort_by(|left, right| {
        right
            .severity
            .weight()
            .cmp(&left.severity.weight())
            .then(left.watchlist_id.cmp(&right.watchlist_id))
    });
    hits
}

pub fn new_case(command: CaseCommand) -> Result<CaseTransition, HelixError> {
    match command {
        CaseCommand::Open {
            case_id,
            title,
            watchlist_id,
            primary_entity,
            evidence_id,
            claim_ids,
            reason,
        } => {
            validate_identifier("case.id", &case_id)?;
            validate_identifier("case.watchlist_id", &watchlist_id)?;
            validate_non_empty("case.title", &title)?;
            validate_non_empty("case.evidence_id", &evidence_id)?;
            validate_non_empty("case.reason", &reason)?;

            let case = CaseFile {
                id: case_id.trim().to_string(),
                title: title.trim().to_string(),
                watchlist_id: watchlist_id.trim().to_string(),
                status: CaseStatus::Open,
                primary_entity: primary_entity.map(|value| value.trim().to_lowercase()),
                evidence_ids: vec![evidence_id.trim().to_string()],
                claim_ids: normalize_ids(claim_ids),
                latest_reason: reason.trim().to_string(),
                briefing_summary: None,
            };
            Ok(CaseTransition {
                case,
                decision: CaseDecision::Opened,
            })
        }
        _ => Err(HelixError::validation_error(
            "case",
            "new_case requires an open command",
        )),
    }
}

pub fn transition_case(case: &CaseFile, command: CaseCommand) -> Result<CaseTransition, HelixError> {
    let mut next = case.clone();
    let decision = match command {
        CaseCommand::Open { .. } => {
            return Err(HelixError::validation_error(
                "case",
                "open command is only valid for new cases",
            ))
        }
        CaseCommand::AppendEvidence {
            evidence_id,
            claim_ids,
            reason,
        } => {
            if next.status == CaseStatus::Closed {
                CaseDecision::Denied {
                    reason: "closed_case".to_string(),
                }
            } else {
                validate_non_empty("case.evidence_id", &evidence_id)?;
                validate_non_empty("case.reason", &reason)?;
                push_unique(&mut next.evidence_ids, evidence_id.trim().to_string());
                for claim_id in normalize_ids(claim_ids) {
                    push_unique(&mut next.claim_ids, claim_id);
                }
                next.latest_reason = reason.trim().to_string();
                CaseDecision::Updated
            }
        }
        CaseCommand::MarkMonitoring => {
            if next.status == CaseStatus::Closed {
                CaseDecision::Denied {
                    reason: "closed_case".to_string(),
                }
            } else {
                next.status = CaseStatus::Monitoring;
                CaseDecision::StatusChanged {
                    status: next.status,
                }
            }
        }
        CaseCommand::AttachBrief { summary } => {
            if next.status == CaseStatus::Closed {
                CaseDecision::Denied {
                    reason: "closed_case".to_string(),
                }
            } else {
                let summary = clamp_trimmed_text("case.summary", &summary, 2_048)?;
                validate_non_empty("case.summary", &summary)?;
                next.briefing_summary = Some(summary);
                next.status = CaseStatus::BriefReady;
                CaseDecision::StatusChanged {
                    status: next.status,
                }
            }
        }
        CaseCommand::Escalate { reason } => {
            if next.status == CaseStatus::Closed {
                CaseDecision::Denied {
                    reason: "closed_case".to_string(),
                }
            } else {
                validate_non_empty("case.reason", &reason)?;
                next.latest_reason = reason.trim().to_string();
                next.status = CaseStatus::Escalated;
                CaseDecision::StatusChanged {
                    status: next.status,
                }
            }
        }
        CaseCommand::Close => {
            next.status = CaseStatus::Closed;
            CaseDecision::StatusChanged {
                status: next.status,
            }
        }
        CaseCommand::Reopen { reason } => {
            if next.status != CaseStatus::Closed {
                CaseDecision::Denied {
                    reason: "case_not_closed".to_string(),
                }
            } else {
                validate_non_empty("case.reason", &reason)?;
                next.latest_reason = reason.trim().to_string();
                next.status = CaseStatus::Open;
                CaseDecision::StatusChanged {
                    status: next.status,
                }
            }
        }
    };

    Ok(CaseTransition { case: next, decision })
}

fn derive_claims_from_entities(evidence: &EvidenceItem) -> Vec<ProposedClaim> {
    evidence
        .entity_labels
        .iter()
        .map(|entity| ProposedClaim {
            subject: entity.clone(),
            predicate: "mentioned_in_source".to_string(),
            object: evidence.title.to_lowercase(),
            confidence_bps: 5_500,
            rationale: Some("derived_from_entity_label".to_string()),
        })
        .collect()
}

fn entity_matches(entity: &str, evidence: &EvidenceItem, claims: &[ClaimRecord]) -> bool {
    evidence.entity_labels.iter().any(|value| value == entity)
        || claims.iter().any(|claim| claim.subject == entity || claim.object == entity)
}

fn normalize_list(
    values: Vec<String>,
    max_items: usize,
    lowercase: bool,
    context: &str,
) -> Result<Vec<String>, HelixError> {
    if values.len() > max_items {
        return Err(HelixError::validation_error(
            context,
            &format!("too many items; max is {max_items}"),
        ));
    }

    let mut normalized: Vec<String> = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(|value| if lowercase { value.to_lowercase() } else { value })
        .collect();
    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}

fn normalize_optional_text(
    value: Option<String>,
    max_len: usize,
    context: &str,
) -> Result<Option<String>, HelixError> {
    value
        .map(|value| clamp_trimmed_text(context, &value, max_len))
        .transpose()
        .map(|value| value.filter(|item| !item.is_empty()))
}

fn clamp_trimmed_text(context: &str, value: &str, max_len: usize) -> Result<String, HelixError> {
    let trimmed = value.trim();
    if trimmed.len() > max_len {
        return Err(HelixError::validation_error(
            context,
            &format!("must be <= {max_len} characters"),
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_non_empty(context: &str, value: &str) -> Result<(), HelixError> {
    if value.trim().is_empty() {
        return Err(HelixError::validation_error(context, "must not be empty"));
    }
    Ok(())
}

fn validate_identifier(context: &str, value: &str) -> Result<(), HelixError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(HelixError::validation_error(context, "must not be empty"));
    }
    if !trimmed
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_'))
    {
        return Err(HelixError::validation_error(
            context,
            "must use lowercase ascii, digits, '-' or '_'",
        ));
    }
    Ok(())
}

fn normalize_ids(ids: Vec<String>) -> Vec<String> {
    let mut normalized: Vec<String> = ids
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn push_unique(values: &mut Vec<String>, candidate: String) {
    if !values.iter().any(|value| value == &candidate) {
        values.push(candidate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_source() -> SourceDefinition {
        SourceDefinition {
            id: "rss_national_security".to_string(),
            name: "National Security RSS".to_string(),
            description: "Analyst feed".to_string(),
            kind: SourceKind::RssFeed,
            cadence_minutes: 30,
            trust_score: 88,
            enabled: true,
            tags: vec!["OSINT".to_string(), "Signals".to_string(), "signals".to_string()],
        }
    }

    fn test_watchlist() -> Watchlist {
        Watchlist {
            id: "watch_exec_moves".to_string(),
            name: "Executive Movements".to_string(),
            description: "Track executives and sensitive locations".to_string(),
            keywords: vec!["resigned".to_string(), "detained".to_string()],
            entities: vec!["alice north".to_string(), "orion dynamics".to_string()],
            min_source_trust: 50,
            severity: WatchlistSeverity::High,
            enabled: true,
        }
    }

    #[test]
    fn canonicalize_source_normalizes_tags() {
        let source = canonicalize_source(test_source()).unwrap();
        assert_eq!(source.tags, vec!["osint".to_string(), "signals".to_string()]);
    }

    #[test]
    fn canonicalize_source_rejects_zero_cadence() {
        let mut source = test_source();
        source.cadence_minutes = 0;
        assert!(matches!(
            canonicalize_source(source),
            Err(HelixError::ValidationError { .. })
        ));
    }

    #[test]
    fn canonicalize_source_rejects_trust_above_hundred() {
        let mut source = test_source();
        source.trust_score = 101;
        assert!(matches!(
            canonicalize_source(source),
            Err(HelixError::ValidationError { .. })
        ));
    }

    #[test]
    fn canonicalize_watchlist_requires_terms() {
        let mut watchlist = test_watchlist();
        watchlist.keywords.clear();
        watchlist.entities.clear();
        assert!(matches!(
            canonicalize_watchlist(watchlist),
            Err(HelixError::ValidationError { .. })
        ));
    }

    #[test]
    fn canonicalize_claims_derives_mentions_when_empty() {
        let evidence = canonicalize_evidence(
            "evidence_alpha".to_string(),
            "abc123".to_string(),
            EvidenceDraft {
                source_id: "rss_national_security".to_string(),
                title: "Alice North resigned".to_string(),
                summary: "summary".to_string(),
                content: "content".to_string(),
                url: None,
                observed_at: "2026-03-06T12:00:00Z".to_string(),
                tags: vec!["leadership".to_string()],
                entity_labels: vec!["alice north".to_string()],
                proposed_claims: Vec::new(),
            },
        )
        .unwrap();

        let claims = canonicalize_claims(&evidence, Vec::new()).unwrap();
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].predicate, "mentioned_in_source");
        assert_eq!(claims[0].confidence_bps, 5_500);
    }

    #[test]
    fn canonicalize_claims_rejects_confidence_over_max() {
        let evidence = canonicalize_evidence(
            "evidence_alpha".to_string(),
            "abc123".to_string(),
            EvidenceDraft {
                source_id: "rss_national_security".to_string(),
                title: "Alice North resigned".to_string(),
                summary: "summary".to_string(),
                content: "content".to_string(),
                url: None,
                observed_at: "2026-03-06T12:00:00Z".to_string(),
                tags: vec!["leadership".to_string()],
                entity_labels: vec!["alice north".to_string()],
                proposed_claims: Vec::new(),
            },
        )
        .unwrap();

        let result = canonicalize_claims(
            &evidence,
            vec![ProposedClaim {
                subject: "alice north".to_string(),
                predicate: "resigned_from".to_string(),
                object: "orion dynamics".to_string(),
                confidence_bps: 10_001,
                rationale: Some("invalid".to_string()),
            }],
        );
        assert!(matches!(result, Err(HelixError::ValidationError { .. })));
    }

    #[test]
    fn evaluate_watchlists_matches_keywords_and_entities() {
        let source = canonicalize_source(test_source()).unwrap();
        let watchlist = canonicalize_watchlist(test_watchlist()).unwrap();
        let evidence = canonicalize_evidence(
            "evidence_alpha".to_string(),
            "abc123".to_string(),
            EvidenceDraft {
                source_id: source.id.clone(),
                title: "Alice North resigned from Orion Dynamics".to_string(),
                summary: "Analyst note".to_string(),
                content: "Alice North was reported detained after resigning.".to_string(),
                url: None,
                observed_at: "2026-03-06T12:00:00Z".to_string(),
                tags: vec!["leadership".to_string()],
                entity_labels: vec!["alice north".to_string()],
                proposed_claims: Vec::new(),
            },
        )
        .unwrap();
        let claims = vec![ClaimRecord {
            id: "claim_alpha".to_string(),
            evidence_id: evidence.id.clone(),
            subject: "alice north".to_string(),
            predicate: "mentioned_in_source".to_string(),
            object: evidence.title.to_lowercase(),
            confidence_bps: 5_500,
            review_status: ClaimReviewStatus::NeedsReview,
            rationale: "derived".to_string(),
        }];

        let hits = evaluate_watchlists(&source, &evidence, &claims, &[watchlist]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].reason, "keyword_and_entity_match");
    }

    #[test]
    fn evaluate_watchlists_respects_source_trust_boundary() {
        let mut source = canonicalize_source(test_source()).unwrap();
        source.trust_score = 49;
        let mut watchlist = canonicalize_watchlist(test_watchlist()).unwrap();
        watchlist.min_source_trust = 50;
        let evidence = canonicalize_evidence(
            "evidence_alpha".to_string(),
            "abc123".to_string(),
            EvidenceDraft {
                source_id: source.id.clone(),
                title: "Alice North resigned".to_string(),
                summary: "summary".to_string(),
                content: "content".to_string(),
                url: None,
                observed_at: "2026-03-06T12:00:00Z".to_string(),
                tags: vec![],
                entity_labels: vec!["alice north".to_string()],
                proposed_claims: Vec::new(),
            },
        )
        .unwrap();

        let hits = evaluate_watchlists(&source, &evidence, &[], &[watchlist]);
        assert!(hits.is_empty());
    }

    #[test]
    fn case_lifecycle_rejects_append_to_closed_case() {
        let opened = new_case(CaseCommand::Open {
            case_id: "case_alpha".to_string(),
            title: "Executive movement".to_string(),
            watchlist_id: "watch_exec_moves".to_string(),
            primary_entity: Some("alice north".to_string()),
            evidence_id: "evidence_alpha".to_string(),
            claim_ids: vec!["claim_alpha".to_string()],
            reason: "keyword_match".to_string(),
        })
        .unwrap();
        let closed = transition_case(&opened.case, CaseCommand::Close).unwrap();
        let denied = transition_case(
            &closed.case,
            CaseCommand::AppendEvidence {
                evidence_id: "evidence_beta".to_string(),
                claim_ids: vec!["claim_beta".to_string()],
                reason: "keyword_match".to_string(),
            },
        )
        .unwrap();

        assert!(matches!(
            denied.decision,
            CaseDecision::Denied { ref reason } if reason == "closed_case"
        ));
    }

    #[test]
    fn case_lifecycle_supports_reopen_boundary() {
        let opened = new_case(CaseCommand::Open {
            case_id: "case_alpha".to_string(),
            title: "Executive movement".to_string(),
            watchlist_id: "watch_exec_moves".to_string(),
            primary_entity: Some("alice north".to_string()),
            evidence_id: "evidence_alpha".to_string(),
            claim_ids: vec!["claim_alpha".to_string()],
            reason: "keyword_match".to_string(),
        })
        .unwrap();
        let closed = transition_case(&opened.case, CaseCommand::Close).unwrap();
        let reopened = transition_case(
            &closed.case,
            CaseCommand::Reopen {
                reason: "new corroboration".to_string(),
            },
        )
        .unwrap();
        assert_eq!(reopened.case.status, CaseStatus::Open);
    }
}
