use crate::intel_desk::{CaseStatus, ClaimReviewStatus, WatchlistSeverity};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const PRIORITY_RADIX: u64 = 6;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntelSignalWindow {
    newest_signal_at: Option<DateTime<Utc>>,
}

impl IntelSignalWindow {
    pub fn from_observed_at_values<'a, I>(values: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        let newest_signal_at = values.into_iter().filter_map(parse_signal_time).max();
        Self { newest_signal_at }
    }

    pub fn newest_signal_at(&self) -> Option<DateTime<Utc>> {
        self.newest_signal_at
    }

    pub fn freshness_tier(&self, observed_at: Option<&str>) -> u8 {
        let Some(reference) = self.newest_signal_at else {
            return 0;
        };
        let Some(observed) = observed_at.and_then(parse_signal_time) else {
            return 0;
        };
        let lag_hours = reference.signed_duration_since(observed).num_hours().max(0);
        match lag_hours {
            0..=6 => 5,
            7..=24 => 4,
            25..=72 => 3,
            73..=168 => 2,
            169..=336 => 1,
            _ => 0,
        }
    }
}

impl Default for IntelSignalWindow {
    fn default() -> Self {
        Self {
            newest_signal_at: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntelPriorityBreakdown {
    pub total: u64,
    pub attention_tier: u8,
    pub severity_tier: u8,
    pub corroboration_tier: u8,
    pub credibility_bps: u16,
    pub freshness_tier: u8,
    pub trust_tier: u8,
    pub density_tier: u8,
}

impl IntelPriorityBreakdown {
    pub fn new(
        attention_tier: u8,
        severity_tier: u8,
        corroboration_tier: u8,
        credibility_bps: u16,
        freshness_tier: u8,
        trust_tier: u8,
        density_tier: u8,
    ) -> Self {
        let tiers = [
            clamp_tier(attention_tier),
            clamp_tier(severity_tier),
            clamp_tier(corroboration_tier),
            clamp_tier(freshness_tier),
            clamp_tier(trust_tier),
            clamp_tier(density_tier),
        ];
        let total = tiers
            .iter()
            .fold(0_u64, |acc, tier| acc * PRIORITY_RADIX + u64::from(*tier));
        Self {
            total,
            attention_tier: tiers[0],
            severity_tier: tiers[1],
            corroboration_tier: tiers[2],
            credibility_bps: credibility_bps.min(10_000),
            freshness_tier: tiers[3],
            trust_tier: tiers[4],
            density_tier: tiers[5],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CasePriorityInput {
    pub status: CaseStatus,
    pub severity: WatchlistSeverity,
    pub source_trust_scores: Vec<u8>,
    pub evidence_count: usize,
    pub claim_count: usize,
    pub corroborated_claim_count: usize,
    pub rejected_claim_count: usize,
    pub max_claim_confidence_bps: u16,
    pub latest_signal_at: Option<String>,
    pub attached_to_case: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidencePriorityInput {
    pub linked_case_statuses: Vec<CaseStatus>,
    pub max_linked_severity: Option<WatchlistSeverity>,
    pub source_trust_scores: Vec<u8>,
    pub claim_count: usize,
    pub corroborated_claim_count: usize,
    pub rejected_claim_count: usize,
    pub max_claim_confidence_bps: u16,
    pub observed_at: Option<String>,
    pub linked_case_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimPriorityInput {
    pub review_status: ClaimReviewStatus,
    pub confidence_bps: u16,
    pub linked_case_statuses: Vec<CaseStatus>,
    pub max_linked_severity: Option<WatchlistSeverity>,
    pub source_trust_scores: Vec<u8>,
    pub evidence_observed_at: Option<String>,
    pub sibling_claim_count: usize,
    pub corroborated_sibling_count: usize,
    pub rejected_sibling_count: usize,
}

pub fn score_case(input: &CasePriorityInput, window: &IntelSignalWindow) -> IntelPriorityBreakdown {
    let attention_tier = case_attention_tier(input.status, input.attached_to_case);
    let severity_tier = severity_tier(Some(input.severity));
    let credibility_bps = fused_credibility_bps(
        input.claim_count,
        input.corroborated_claim_count,
        input.rejected_claim_count,
        input.max_claim_confidence_bps,
    );
    let corroboration_tier = corroboration_tier(
        input.claim_count,
        input.corroborated_claim_count,
        input.rejected_claim_count,
        input.max_claim_confidence_bps,
    );
    let freshness_tier = window.freshness_tier(input.latest_signal_at.as_deref());
    let trust_tier = trust_tier(&input.source_trust_scores);
    let density_tier = density_tier(
        input.evidence_count,
        input.claim_count,
        input.corroborated_claim_count,
    );

    IntelPriorityBreakdown::new(
        attention_tier,
        severity_tier,
        corroboration_tier,
        credibility_bps,
        freshness_tier,
        trust_tier,
        density_tier,
    )
}

pub fn score_evidence(
    input: &EvidencePriorityInput,
    window: &IntelSignalWindow,
) -> IntelPriorityBreakdown {
    let attention_tier = evidence_attention_tier(&input.linked_case_statuses, input.claim_count);
    let severity_tier = severity_tier(input.max_linked_severity);
    let credibility_bps = fused_credibility_bps(
        input.claim_count,
        input.corroborated_claim_count,
        input.rejected_claim_count,
        input.max_claim_confidence_bps,
    );
    let corroboration_tier = corroboration_tier(
        input.claim_count,
        input.corroborated_claim_count,
        input.rejected_claim_count,
        input.max_claim_confidence_bps,
    );
    let freshness_tier = window.freshness_tier(input.observed_at.as_deref());
    let trust_tier = trust_tier(&input.source_trust_scores);
    let density_tier = bucket_usize(
        input
            .claim_count
            .saturating_add(input.linked_case_count.saturating_mul(2))
            .saturating_add(input.corroborated_claim_count.saturating_mul(2)),
        &[1, 2, 3, 5, 8],
    );

    IntelPriorityBreakdown::new(
        attention_tier,
        severity_tier,
        corroboration_tier,
        credibility_bps,
        freshness_tier,
        trust_tier,
        density_tier,
    )
}

pub fn score_claim(
    input: &ClaimPriorityInput,
    window: &IntelSignalWindow,
) -> IntelPriorityBreakdown {
    let attention_tier = claim_attention_tier(input.review_status, &input.linked_case_statuses);
    let severity_tier = severity_tier(input.max_linked_severity);
    let credibility_bps = claim_credibility_bps(
        input.review_status,
        input.confidence_bps,
        input.corroborated_sibling_count,
        input.rejected_sibling_count,
    );
    let corroboration_tier = claim_corroboration_tier(
        input.review_status,
        input.confidence_bps,
        input.corroborated_sibling_count,
        input.rejected_sibling_count,
    );
    let freshness_tier = window.freshness_tier(input.evidence_observed_at.as_deref());
    let trust_tier = trust_tier(&input.source_trust_scores);
    let density_tier = bucket_usize(
        input
            .sibling_claim_count
            .saturating_add(input.corroborated_sibling_count.saturating_mul(2)),
        &[1, 2, 3, 5, 8],
    );

    IntelPriorityBreakdown::new(
        attention_tier,
        severity_tier,
        corroboration_tier,
        credibility_bps,
        freshness_tier,
        trust_tier,
        density_tier,
    )
}

pub(crate) fn aggregate_attention_tier(
    active_case_count: usize,
    escalated_case_count: usize,
) -> u8 {
    if escalated_case_count > 0 {
        5
    } else {
        bucket_usize(active_case_count, &[1, 2, 3, 5, 8])
    }
}

pub(crate) fn severity_tier(severity: Option<WatchlistSeverity>) -> u8 {
    severity.map(WatchlistSeverity::weight).unwrap_or(0)
}

pub(crate) fn corroboration_tier(
    claim_count: usize,
    corroborated_claim_count: usize,
    rejected_claim_count: usize,
    max_claim_confidence_bps: u16,
) -> u8 {
    credibility_tier(fused_credibility_bps(
        claim_count,
        corroborated_claim_count,
        rejected_claim_count,
        max_claim_confidence_bps,
    ))
}

pub(crate) fn trust_tier(scores: &[u8]) -> u8 {
    if scores.is_empty() {
        return 0;
    }
    let total: usize = scores.iter().map(|score| usize::from(*score)).sum();
    let average = total / scores.len();
    bucket_usize(average, &[45, 60, 70, 80, 90])
}

pub(crate) fn bucket_usize(value: usize, thresholds: &[usize]) -> u8 {
    thresholds
        .iter()
        .filter(|threshold| value >= **threshold)
        .count()
        .try_into()
        .unwrap_or(5)
}

fn parse_signal_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

fn case_attention_tier(status: CaseStatus, attached_to_case: bool) -> u8 {
    let base = case_status_attention_tier(status);
    if attached_to_case && base < 4 {
        base + 1
    } else {
        base
    }
}

fn case_status_attention_tier(status: CaseStatus) -> u8 {
    match status {
        CaseStatus::Closed => 0,
        CaseStatus::Monitoring => 1,
        CaseStatus::Open => 2,
        CaseStatus::BriefReady => 3,
        CaseStatus::Escalated => 5,
    }
}

fn max_case_attention_tier(statuses: &[CaseStatus]) -> u8 {
    statuses
        .iter()
        .copied()
        .map(case_status_attention_tier)
        .max()
        .unwrap_or(0)
}

fn evidence_attention_tier(linked_case_statuses: &[CaseStatus], claim_count: usize) -> u8 {
    let linked_tier = max_case_attention_tier(linked_case_statuses);
    if linked_tier > 0 {
        linked_tier
    } else if claim_count > 0 {
        1
    } else {
        0
    }
}

fn claim_attention_tier(
    review_status: ClaimReviewStatus,
    linked_case_statuses: &[CaseStatus],
) -> u8 {
    let linked_tier = max_case_attention_tier(linked_case_statuses);
    match review_status {
        ClaimReviewStatus::NeedsReview => linked_tier.max(3),
        ClaimReviewStatus::Corroborated => linked_tier.min(4).max(1),
        ClaimReviewStatus::Rejected => linked_tier.min(1),
    }
}

fn claim_corroboration_tier(
    review_status: ClaimReviewStatus,
    confidence_bps: u16,
    corroborated_sibling_count: usize,
    rejected_sibling_count: usize,
) -> u8 {
    credibility_tier(claim_credibility_bps(
        review_status,
        confidence_bps,
        corroborated_sibling_count,
        rejected_sibling_count,
    ))
}

fn density_tier(evidence_count: usize, claim_count: usize, corroborated_claim_count: usize) -> u8 {
    let signal_units = evidence_count
        .saturating_mul(2)
        .saturating_add(claim_count)
        .saturating_add(corroborated_claim_count.saturating_mul(2));
    bucket_usize(signal_units, &[1, 3, 5, 8, 12])
}

fn clamp_tier(value: u8) -> u8 {
    value.min(5)
}

pub(crate) fn credibility_tier(credibility_bps: u16) -> u8 {
    bucket_u16(credibility_bps, &[1_000, 3_000, 5_500, 8_000, 9_300])
}

pub(crate) fn fused_credibility_bps(
    claim_count: usize,
    corroborated_claim_count: usize,
    rejected_claim_count: usize,
    max_claim_confidence_bps: u16,
) -> u16 {
    let corroborated_support = corroborated_support_bps(max_claim_confidence_bps);
    let proposal_support = proposal_support_bps(max_claim_confidence_bps);
    let rejection_support = rejection_support_bps(max_claim_confidence_bps);
    let unresolved_claim_count = claim_count
        .saturating_sub(corroborated_claim_count)
        .saturating_sub(rejected_claim_count);

    let support = accumulate_noisy_or(0, corroborated_support, corroborated_claim_count);
    let support = accumulate_noisy_or(support, proposal_support, unresolved_claim_count);
    let rejection = accumulate_noisy_or(0, rejection_support, rejected_claim_count);
    attenuate_support(support, rejection)
}

fn claim_credibility_bps(
    review_status: ClaimReviewStatus,
    confidence_bps: u16,
    corroborated_sibling_count: usize,
    rejected_sibling_count: usize,
) -> u16 {
    if review_status == ClaimReviewStatus::Rejected {
        return 0;
    }

    let corroborated_support = corroborated_support_bps(confidence_bps);
    let proposal_support = proposal_support_bps(confidence_bps);
    let rejection_support = rejection_support_bps(confidence_bps);

    let support = match review_status {
        ClaimReviewStatus::Corroborated => {
            accumulate_noisy_or(0, corroborated_support, 1 + corroborated_sibling_count)
        }
        ClaimReviewStatus::NeedsReview => {
            let support = accumulate_noisy_or(0, corroborated_support, corroborated_sibling_count);
            accumulate_noisy_or(support, proposal_support, 1)
        }
        ClaimReviewStatus::Rejected => 0,
    };
    let rejection = accumulate_noisy_or(0, rejection_support, rejected_sibling_count);
    attenuate_support(support, rejection)
}

fn proposal_support_bps(confidence_bps: u16) -> u16 {
    ((u32::from(confidence_bps.min(10_000)) * 7) / 10)
        .try_into()
        .unwrap_or(7_000)
}

fn corroborated_support_bps(confidence_bps: u16) -> u16 {
    confidence_bps.clamp(8_500, 10_000)
}

fn rejection_support_bps(confidence_bps: u16) -> u16 {
    proposal_support_bps(confidence_bps).max(6_500)
}

fn accumulate_noisy_or(mut aggregate_bps: u16, signal_bps: u16, count: usize) -> u16 {
    let signal_bps = signal_bps.min(10_000);
    for _ in 0..count {
        let remaining = 10_000_u32.saturating_sub(u32::from(aggregate_bps));
        let increment = (remaining * u32::from(signal_bps)) / 10_000;
        aggregate_bps = (u32::from(aggregate_bps) + increment)
            .min(10_000)
            .try_into()
            .unwrap_or(10_000);
    }
    aggregate_bps
}

fn attenuate_support(support_bps: u16, rejection_bps: u16) -> u16 {
    ((u32::from(support_bps.min(10_000))
        * u32::from(10_000_u16.saturating_sub(rejection_bps.min(10_000))))
        / 10_000)
        .try_into()
        .unwrap_or(0)
}

fn bucket_u16(value: u16, thresholds: &[u16]) -> u8 {
    thresholds
        .iter()
        .filter(|threshold| value >= **threshold)
        .count()
        .try_into()
        .unwrap_or(5)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window() -> IntelSignalWindow {
        IntelSignalWindow::from_observed_at_values(["2026-03-10T12:00:00Z", "2026-03-09T12:00:00Z"])
    }

    #[test]
    fn freshness_is_relative_to_newest_signal() {
        let window = window();
        assert_eq!(window.freshness_tier(Some("2026-03-10T10:00:00Z")), 5);
        assert_eq!(window.freshness_tier(Some("2026-03-10T00:00:00Z")), 4);
        assert_eq!(window.freshness_tier(Some("2026-03-08T12:00:00Z")), 3);
        assert_eq!(window.freshness_tier(Some("invalid")), 0);
    }

    #[test]
    fn escalated_critical_case_outranks_noisy_low_case() {
        let window = window();
        let high = score_case(
            &CasePriorityInput {
                status: CaseStatus::Escalated,
                severity: WatchlistSeverity::Critical,
                source_trust_scores: vec![92, 88],
                evidence_count: 2,
                claim_count: 2,
                corroborated_claim_count: 1,
                rejected_claim_count: 0,
                max_claim_confidence_bps: 9_200,
                latest_signal_at: Some("2026-03-10T11:30:00Z".to_string()),
                attached_to_case: true,
            },
            &window,
        );
        let low = score_case(
            &CasePriorityInput {
                status: CaseStatus::Monitoring,
                severity: WatchlistSeverity::Low,
                source_trust_scores: vec![55, 58, 60],
                evidence_count: 6,
                claim_count: 6,
                corroborated_claim_count: 0,
                rejected_claim_count: 2,
                max_claim_confidence_bps: 8_300,
                latest_signal_at: Some("2026-03-10T11:45:00Z".to_string()),
                attached_to_case: false,
            },
            &window,
        );

        assert!(high.total > low.total);
        assert_eq!(high.attention_tier, 5);
        assert_eq!(high.severity_tier, 4);
    }

    #[test]
    fn linked_evidence_outranks_unlinked_background_evidence() {
        let window = window();
        let urgent = score_evidence(
            &EvidencePriorityInput {
                linked_case_statuses: vec![CaseStatus::Escalated],
                max_linked_severity: Some(WatchlistSeverity::High),
                source_trust_scores: vec![90],
                claim_count: 2,
                corroborated_claim_count: 1,
                rejected_claim_count: 0,
                max_claim_confidence_bps: 9_100,
                observed_at: Some("2026-03-10T11:30:00Z".to_string()),
                linked_case_count: 1,
            },
            &window,
        );
        let passive = score_evidence(
            &EvidencePriorityInput {
                linked_case_statuses: Vec::new(),
                max_linked_severity: None,
                source_trust_scores: vec![55],
                claim_count: 0,
                corroborated_claim_count: 0,
                rejected_claim_count: 0,
                max_claim_confidence_bps: 0,
                observed_at: Some("2026-03-08T11:30:00Z".to_string()),
                linked_case_count: 0,
            },
            &window,
        );

        assert!(urgent.total > passive.total);
        assert_eq!(urgent.attention_tier, 5);
    }

    #[test]
    fn needs_review_claim_outranks_rejected_claim() {
        let window = window();
        let review = score_claim(
            &ClaimPriorityInput {
                review_status: ClaimReviewStatus::NeedsReview,
                confidence_bps: 9_000,
                linked_case_statuses: vec![CaseStatus::Open],
                max_linked_severity: Some(WatchlistSeverity::Medium),
                source_trust_scores: vec![82],
                evidence_observed_at: Some("2026-03-10T09:00:00Z".to_string()),
                sibling_claim_count: 2,
                corroborated_sibling_count: 0,
                rejected_sibling_count: 0,
            },
            &window,
        );
        let rejected = score_claim(
            &ClaimPriorityInput {
                review_status: ClaimReviewStatus::Rejected,
                confidence_bps: 9_500,
                linked_case_statuses: vec![CaseStatus::Open],
                max_linked_severity: Some(WatchlistSeverity::Medium),
                source_trust_scores: vec![82],
                evidence_observed_at: Some("2026-03-10T09:00:00Z".to_string()),
                sibling_claim_count: 2,
                corroborated_sibling_count: 0,
                rejected_sibling_count: 1,
            },
            &window,
        );

        assert!(review.total > rejected.total);
        assert!(review.attention_tier > rejected.attention_tier);
    }

    #[test]
    fn fused_credibility_grows_with_corroboration() {
        let weak = fused_credibility_bps(1, 0, 0, 9_000);
        let corroborated = fused_credibility_bps(1, 1, 0, 9_000);
        let multi_source = fused_credibility_bps(3, 2, 0, 9_000);

        assert!(weak < corroborated);
        assert!(corroborated < multi_source);
        assert!(credibility_tier(weak) < credibility_tier(corroborated));
    }

    #[test]
    fn fused_credibility_penalizes_rejections() {
        let clean = fused_credibility_bps(3, 1, 0, 8_800);
        let contested = fused_credibility_bps(3, 1, 1, 8_800);

        assert!(clean > contested);
        assert!(credibility_tier(clean) >= credibility_tier(contested));
    }

    #[test]
    fn fused_credibility_is_antitone_in_rejections() {
        let none = fused_credibility_bps(4, 1, 0, 9_100);
        let one = fused_credibility_bps(4, 1, 1, 9_100);
        let two = fused_credibility_bps(4, 1, 2, 9_100);

        assert!(none >= one);
        assert!(one >= two);
    }

    #[test]
    fn attenuation_is_antitone_in_rejection_signal() {
        let support = 8_200;
        let mild = attenuate_support(support, 1_000);
        let medium = attenuate_support(support, 4_000);
        let severe = attenuate_support(support, 9_000);

        assert_eq!(attenuate_support(support, 0), support);
        assert_eq!(attenuate_support(support, 10_000), 0);
        assert!(mild >= medium);
        assert!(medium >= severe);
    }

    #[test]
    fn noisy_or_accumulation_is_monotone_and_bounded() {
        let once = accumulate_noisy_or(0, 7_000, 1);
        let twice = accumulate_noisy_or(0, 7_000, 2);
        let many = accumulate_noisy_or(0, 7_000, 8);

        assert!(once <= twice);
        assert!(twice <= many);
        assert!(many <= 10_000);
    }

    #[test]
    fn fixed_claim_budget_prefers_full_corroboration_over_open_proposals() {
        for confidence_bps in [0_u16, 1, 8_499, 8_500, 10_000] {
            let empty = fused_credibility_bps(0, 0, 0, confidence_bps);
            assert_eq!(empty, 0);

            for claim_count in 1..=5 {
                let unresolved = fused_credibility_bps(claim_count, 0, 0, confidence_bps);
                let corroborated =
                    fused_credibility_bps(claim_count, claim_count, 0, confidence_bps);

                assert!(
                    corroborated > unresolved,
                    "expected full corroboration to outrank unresolved proposals for claim_count={claim_count}, confidence_bps={confidence_bps}, unresolved={unresolved}, corroborated={corroborated}"
                );
            }
        }
    }

    #[test]
    fn replacing_one_open_proposal_with_corroboration_never_decreases_score() {
        for confidence_bps in [0_u16, 1, 1_000, 5_000, 8_499, 8_500, 9_100, 10_000] {
            for claim_count in 1..=7 {
                for rejected_claim_count in 0..claim_count {
                    for corroborated_claim_count in 0..(claim_count - rejected_claim_count) {
                        if corroborated_claim_count + rejected_claim_count >= claim_count {
                            continue;
                        }

                        let before = fused_credibility_bps(
                            claim_count,
                            corroborated_claim_count,
                            rejected_claim_count,
                            confidence_bps,
                        );
                        let after = fused_credibility_bps(
                            claim_count,
                            corroborated_claim_count + 1,
                            rejected_claim_count,
                            confidence_bps,
                        );

                        assert!(
                            before <= after,
                            "expected one-step corroboration replacement to be monotone for claim_count={claim_count}, corroborated_claim_count={corroborated_claim_count}, rejected_claim_count={rejected_claim_count}, confidence_bps={confidence_bps}, before={before}, after={after}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn last_open_claim_replacement_is_strict_away_from_saturation() {
        let before = fused_credibility_bps(1, 0, 0, 0);
        let after = fused_credibility_bps(1, 1, 0, 0);

        assert_eq!(before, 0);
        assert_eq!(after, 8_500);
        assert!(before < after);
    }

    #[test]
    fn last_open_claim_replacement_can_flatten_at_saturation_boundary() {
        let before = fused_credibility_bps(6, 5, 0, 0);
        let after = fused_credibility_bps(6, 6, 0, 0);

        assert_eq!(before, 9_999);
        assert_eq!(after, 9_999);
        assert_eq!(before, after);
    }

    #[test]
    fn last_open_claim_with_fixed_rejection_can_still_increase_strictly() {
        let before = fused_credibility_bps(2, 0, 1, 0);
        let after = fused_credibility_bps(2, 1, 1, 0);

        assert_eq!(before, 0);
        assert_eq!(after, 2_975);
        assert!(before < after);
    }

    #[test]
    fn last_open_claim_with_fixed_rejection_can_also_flatten() {
        let before = fused_credibility_bps(7, 5, 1, 0);
        let after = fused_credibility_bps(7, 6, 1, 0);

        assert_eq!(before, 3_499);
        assert_eq!(after, 3_499);
        assert_eq!(before, after);
    }

    #[test]
    fn last_open_fixed_rejection_scaled_gap_guarantees_strict_lift() {
        for confidence_bps in [0_u16, 1, 1_000, 5_000, 8_499, 8_500, 9_100, 10_000] {
            for corroborated_claim_count in 0..=6 {
                for rejected_claim_count in 0..=4 {
                    let claim_count = corroborated_claim_count + rejected_claim_count + 1;
                    let corroborated_support = corroborated_support_bps(confidence_bps);
                    let proposal_support = proposal_support_bps(confidence_bps);
                    let rejection_support = rejection_support_bps(confidence_bps);
                    let base_support =
                        accumulate_noisy_or(0, corroborated_support, corroborated_claim_count);
                    let before_support = accumulate_noisy_or(base_support, proposal_support, 1);
                    let after_support = accumulate_noisy_or(base_support, corroborated_support, 1);
                    let rejection_bps =
                        accumulate_noisy_or(0, rejection_support, rejected_claim_count);
                    let remaining_bps = 10_000_u32.saturating_sub(u32::from(rejection_bps));
                    let scaled_gap_floor = u32::from(before_support) * remaining_bps + 10_000;
                    let scaled_gap_ceiling = u32::from(after_support) * remaining_bps;
                    let before = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count,
                        rejected_claim_count,
                        confidence_bps,
                    );
                    let after = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count + 1,
                        rejected_claim_count,
                        confidence_bps,
                    );

                    if scaled_gap_floor <= scaled_gap_ceiling {
                        assert!(
                            before < after,
                            "expected scaled-gap condition to force a strict lift for claim_count={claim_count}, corroborated_claim_count={corroborated_claim_count}, rejected_claim_count={rejected_claim_count}, confidence_bps={confidence_bps}, before_support={before_support}, after_support={after_support}, rejection_bps={rejection_bps}, before={before}, after={after}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn last_open_fixed_rejection_gap_threshold_guarantees_strict_lift() {
        for confidence_bps in [0_u16, 1, 1_000, 5_000, 8_499, 8_500, 9_100, 10_000] {
            for corroborated_claim_count in 0..=6 {
                for rejected_claim_count in 0..=4 {
                    let claim_count = corroborated_claim_count + rejected_claim_count + 1;
                    let corroborated_support = corroborated_support_bps(confidence_bps);
                    let proposal_support = proposal_support_bps(confidence_bps);
                    let rejection_support = rejection_support_bps(confidence_bps);
                    let base_support =
                        accumulate_noisy_or(0, corroborated_support, corroborated_claim_count);
                    let before_support = accumulate_noisy_or(base_support, proposal_support, 1);
                    let after_support = accumulate_noisy_or(base_support, corroborated_support, 1);
                    let rejection_bps =
                        accumulate_noisy_or(0, rejection_support, rejected_claim_count);
                    let before = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count,
                        rejected_claim_count,
                        confidence_bps,
                    );
                    let after = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count + 1,
                        rejected_claim_count,
                        confidence_bps,
                    );

                    if rejection_bps < 10_000 {
                        let remaining_bps = 10_000_u32 - u32::from(rejection_bps);
                        let strict_gap_threshold = ((10_000_u32 - 1) / remaining_bps) + 1;

                        if u32::from(before_support) + strict_gap_threshold
                            <= u32::from(after_support)
                        {
                            assert!(
                                before < after,
                                "expected support-gap threshold to force a strict lift for claim_count={claim_count}, corroborated_claim_count={corroborated_claim_count}, rejected_claim_count={rejected_claim_count}, confidence_bps={confidence_bps}, before_support={before_support}, after_support={after_support}, rejection_bps={rejection_bps}, threshold={strict_gap_threshold}, before={before}, after={after}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn last_open_fixed_rejection_two_gap_under_half_headroom_is_strict() {
        for confidence_bps in [0_u16, 1, 1_000, 5_000, 8_499, 8_500, 9_100, 10_000] {
            for corroborated_claim_count in 0..=6 {
                for rejected_claim_count in 0..=4 {
                    let claim_count = corroborated_claim_count + rejected_claim_count + 1;
                    let corroborated_support = corroborated_support_bps(confidence_bps);
                    let proposal_support = proposal_support_bps(confidence_bps);
                    let rejection_support = rejection_support_bps(confidence_bps);
                    let base_support =
                        accumulate_noisy_or(0, corroborated_support, corroborated_claim_count);
                    let before_support = accumulate_noisy_or(base_support, proposal_support, 1);
                    let after_support = accumulate_noisy_or(base_support, corroborated_support, 1);
                    let rejection_bps =
                        accumulate_noisy_or(0, rejection_support, rejected_claim_count);
                    let before = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count,
                        rejected_claim_count,
                        confidence_bps,
                    );
                    let after = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count + 1,
                        rejected_claim_count,
                        confidence_bps,
                    );

                    if rejection_bps <= 5_000
                        && u32::from(before_support) + 2 <= u32::from(after_support)
                    {
                        assert!(
                            before < after,
                            "expected half-headroom two-gap condition to force a strict lift for claim_count={claim_count}, corroborated_claim_count={corroborated_claim_count}, rejected_claim_count={rejected_claim_count}, confidence_bps={confidence_bps}, before_support={before_support}, after_support={after_support}, rejection_bps={rejection_bps}, before={before}, after={after}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn last_open_fixed_rejection_three_gap_under_two_thirds_headroom_is_strict() {
        for confidence_bps in [0_u16, 1, 1_000, 5_000, 8_499, 8_500, 9_100, 10_000] {
            for corroborated_claim_count in 0..=6 {
                for rejected_claim_count in 0..=4 {
                    let claim_count = corroborated_claim_count + rejected_claim_count + 1;
                    let corroborated_support = corroborated_support_bps(confidence_bps);
                    let proposal_support = proposal_support_bps(confidence_bps);
                    let rejection_support = rejection_support_bps(confidence_bps);
                    let base_support =
                        accumulate_noisy_or(0, corroborated_support, corroborated_claim_count);
                    let before_support = accumulate_noisy_or(base_support, proposal_support, 1);
                    let after_support = accumulate_noisy_or(base_support, corroborated_support, 1);
                    let rejection_bps =
                        accumulate_noisy_or(0, rejection_support, rejected_claim_count);
                    let before = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count,
                        rejected_claim_count,
                        confidence_bps,
                    );
                    let after = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count + 1,
                        rejected_claim_count,
                        confidence_bps,
                    );

                    if rejection_bps <= 6_666
                        && u32::from(before_support) + 3 <= u32::from(after_support)
                    {
                        assert!(
                            before < after,
                            "expected two-thirds-headroom three-gap condition to force a strict lift for claim_count={claim_count}, corroborated_claim_count={corroborated_claim_count}, rejected_claim_count={rejected_claim_count}, confidence_bps={confidence_bps}, before_support={before_support}, after_support={after_support}, rejection_bps={rejection_bps}, before={before}, after={after}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn last_open_fixed_rejection_four_gap_under_quarter_headroom_is_strict() {
        for confidence_bps in [0_u16, 1, 1_000, 5_000, 8_499, 8_500, 9_100, 10_000] {
            for corroborated_claim_count in 0..=6 {
                for rejected_claim_count in 0..=4 {
                    let claim_count = corroborated_claim_count + rejected_claim_count + 1;
                    let corroborated_support = corroborated_support_bps(confidence_bps);
                    let proposal_support = proposal_support_bps(confidence_bps);
                    let rejection_support = rejection_support_bps(confidence_bps);
                    let base_support =
                        accumulate_noisy_or(0, corroborated_support, corroborated_claim_count);
                    let before_support = accumulate_noisy_or(base_support, proposal_support, 1);
                    let after_support = accumulate_noisy_or(base_support, corroborated_support, 1);
                    let rejection_bps =
                        accumulate_noisy_or(0, rejection_support, rejected_claim_count);
                    let before = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count,
                        rejected_claim_count,
                        confidence_bps,
                    );
                    let after = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count + 1,
                        rejected_claim_count,
                        confidence_bps,
                    );

                    if rejection_bps <= 7_500
                        && u32::from(before_support) + 4 <= u32::from(after_support)
                    {
                        assert!(
                            before < after,
                            "expected quarter-headroom four-gap condition to force a strict lift for claim_count={claim_count}, corroborated_claim_count={corroborated_claim_count}, rejected_claim_count={rejected_claim_count}, confidence_bps={confidence_bps}, before_support={before_support}, after_support={after_support}, rejection_bps={rejection_bps}, before={before}, after={after}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn last_open_fixed_rejection_five_gap_under_fifth_headroom_is_strict() {
        for confidence_bps in [0_u16, 1, 1_000, 5_000, 8_499, 8_500, 9_100, 10_000] {
            for corroborated_claim_count in 0..=6 {
                for rejected_claim_count in 0..=4 {
                    let claim_count = corroborated_claim_count + rejected_claim_count + 1;
                    let corroborated_support = corroborated_support_bps(confidence_bps);
                    let proposal_support = proposal_support_bps(confidence_bps);
                    let rejection_support = rejection_support_bps(confidence_bps);
                    let base_support =
                        accumulate_noisy_or(0, corroborated_support, corroborated_claim_count);
                    let before_support = accumulate_noisy_or(base_support, proposal_support, 1);
                    let after_support = accumulate_noisy_or(base_support, corroborated_support, 1);
                    let rejection_bps =
                        accumulate_noisy_or(0, rejection_support, rejected_claim_count);
                    let before = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count,
                        rejected_claim_count,
                        confidence_bps,
                    );
                    let after = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count + 1,
                        rejected_claim_count,
                        confidence_bps,
                    );

                    if rejection_bps <= 8_000
                        && u32::from(before_support) + 5 <= u32::from(after_support)
                    {
                        assert!(
                            before < after,
                            "expected fifth-headroom five-gap condition to force a strict lift for claim_count={claim_count}, corroborated_claim_count={corroborated_claim_count}, rejected_claim_count={rejected_claim_count}, confidence_bps={confidence_bps}, before_support={before_support}, after_support={after_support}, rejection_bps={rejection_bps}, before={before}, after={after}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn last_open_fixed_rejection_six_gap_under_sixth_headroom_is_strict() {
        for confidence_bps in [0_u16, 1, 1_000, 5_000, 8_499, 8_500, 9_100, 10_000] {
            for corroborated_claim_count in 0..=6 {
                for rejected_claim_count in 0..=4 {
                    let claim_count = corroborated_claim_count + rejected_claim_count + 1;
                    let corroborated_support = corroborated_support_bps(confidence_bps);
                    let proposal_support = proposal_support_bps(confidence_bps);
                    let rejection_support = rejection_support_bps(confidence_bps);
                    let base_support =
                        accumulate_noisy_or(0, corroborated_support, corroborated_claim_count);
                    let before_support = accumulate_noisy_or(base_support, proposal_support, 1);
                    let after_support = accumulate_noisy_or(base_support, corroborated_support, 1);
                    let rejection_bps =
                        accumulate_noisy_or(0, rejection_support, rejected_claim_count);
                    let before = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count,
                        rejected_claim_count,
                        confidence_bps,
                    );
                    let after = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count + 1,
                        rejected_claim_count,
                        confidence_bps,
                    );

                    if rejection_bps <= 8_333
                        && u32::from(before_support) + 6 <= u32::from(after_support)
                    {
                        assert!(
                            before < after,
                            "expected sixth-headroom six-gap condition to force a strict lift for claim_count={claim_count}, corroborated_claim_count={corroborated_claim_count}, rejected_claim_count={rejected_claim_count}, confidence_bps={confidence_bps}, before_support={before_support}, after_support={after_support}, rejection_bps={rejection_bps}, before={before}, after={after}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn last_open_fixed_rejection_seven_gap_under_seventh_headroom_is_strict() {
        for confidence_bps in [0_u16, 1, 1_000, 5_000, 8_499, 8_500, 9_100, 10_000] {
            for corroborated_claim_count in 0..=6 {
                for rejected_claim_count in 0..=4 {
                    let claim_count = corroborated_claim_count + rejected_claim_count + 1;
                    let corroborated_support = corroborated_support_bps(confidence_bps);
                    let proposal_support = proposal_support_bps(confidence_bps);
                    let rejection_support = rejection_support_bps(confidence_bps);
                    let base_support =
                        accumulate_noisy_or(0, corroborated_support, corroborated_claim_count);
                    let before_support = accumulate_noisy_or(base_support, proposal_support, 1);
                    let after_support = accumulate_noisy_or(base_support, corroborated_support, 1);
                    let rejection_bps =
                        accumulate_noisy_or(0, rejection_support, rejected_claim_count);
                    let before = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count,
                        rejected_claim_count,
                        confidence_bps,
                    );
                    let after = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count + 1,
                        rejected_claim_count,
                        confidence_bps,
                    );

                    if rejection_bps <= 8_571
                        && u32::from(before_support) + 7 <= u32::from(after_support)
                    {
                        assert!(
                            before < after,
                            "expected seventh-headroom seven-gap condition to force a strict lift for claim_count={claim_count}, corroborated_claim_count={corroborated_claim_count}, rejected_claim_count={rejected_claim_count}, confidence_bps={confidence_bps}, before_support={before_support}, after_support={after_support}, rejection_bps={rejection_bps}, before={before}, after={after}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn last_open_fixed_rejection_eight_gap_under_eighth_headroom_is_strict() {
        for confidence_bps in [0_u16, 1, 1_000, 5_000, 8_499, 8_500, 9_100, 10_000] {
            for corroborated_claim_count in 0..=6 {
                for rejected_claim_count in 0..=4 {
                    let claim_count = corroborated_claim_count + rejected_claim_count + 1;
                    let corroborated_support = corroborated_support_bps(confidence_bps);
                    let proposal_support = proposal_support_bps(confidence_bps);
                    let rejection_support = rejection_support_bps(confidence_bps);
                    let base_support =
                        accumulate_noisy_or(0, corroborated_support, corroborated_claim_count);
                    let before_support = accumulate_noisy_or(base_support, proposal_support, 1);
                    let after_support = accumulate_noisy_or(base_support, corroborated_support, 1);
                    let rejection_bps =
                        accumulate_noisy_or(0, rejection_support, rejected_claim_count);
                    let before = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count,
                        rejected_claim_count,
                        confidence_bps,
                    );
                    let after = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count + 1,
                        rejected_claim_count,
                        confidence_bps,
                    );

                    if rejection_bps <= 8_750
                        && u32::from(before_support) + 8 <= u32::from(after_support)
                    {
                        assert!(
                            before < after,
                            "expected eighth-headroom eight-gap condition to force a strict lift for claim_count={claim_count}, corroborated_claim_count={corroborated_claim_count}, rejected_claim_count={rejected_claim_count}, confidence_bps={confidence_bps}, before_support={before_support}, after_support={after_support}, rejection_bps={rejection_bps}, before={before}, after={after}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn last_open_fixed_rejection_nine_gap_under_ninth_headroom_is_strict() {
        for confidence_bps in [0_u16, 1, 1_000, 5_000, 8_499, 8_500, 9_100, 10_000] {
            for corroborated_claim_count in 0..=6 {
                for rejected_claim_count in 0..=4 {
                    let claim_count = corroborated_claim_count + rejected_claim_count + 1;
                    let corroborated_support = corroborated_support_bps(confidence_bps);
                    let proposal_support = proposal_support_bps(confidence_bps);
                    let rejection_support = rejection_support_bps(confidence_bps);
                    let base_support =
                        accumulate_noisy_or(0, corroborated_support, corroborated_claim_count);
                    let before_support = accumulate_noisy_or(base_support, proposal_support, 1);
                    let after_support = accumulate_noisy_or(base_support, corroborated_support, 1);
                    let rejection_bps =
                        accumulate_noisy_or(0, rejection_support, rejected_claim_count);
                    let before = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count,
                        rejected_claim_count,
                        confidence_bps,
                    );
                    let after = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count + 1,
                        rejected_claim_count,
                        confidence_bps,
                    );

                    if rejection_bps <= 8_888
                        && u32::from(before_support) + 9 <= u32::from(after_support)
                    {
                        assert!(
                            before < after,
                            "expected ninth-headroom nine-gap condition to force a strict lift for claim_count={claim_count}, corroborated_claim_count={corroborated_claim_count}, rejected_claim_count={rejected_claim_count}, confidence_bps={confidence_bps}, before_support={before_support}, after_support={after_support}, rejection_bps={rejection_bps}, before={before}, after={after}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn last_open_fixed_rejection_ten_gap_under_tenth_headroom_is_strict() {
        for confidence_bps in [0_u16, 1, 1_000, 5_000, 8_499, 8_500, 9_100, 10_000] {
            for corroborated_claim_count in 0..=6 {
                for rejected_claim_count in 0..=4 {
                    let claim_count = corroborated_claim_count + rejected_claim_count + 1;
                    let corroborated_support = corroborated_support_bps(confidence_bps);
                    let proposal_support = proposal_support_bps(confidence_bps);
                    let rejection_support = rejection_support_bps(confidence_bps);
                    let base_support =
                        accumulate_noisy_or(0, corroborated_support, corroborated_claim_count);
                    let before_support = accumulate_noisy_or(base_support, proposal_support, 1);
                    let after_support = accumulate_noisy_or(base_support, corroborated_support, 1);
                    let rejection_bps =
                        accumulate_noisy_or(0, rejection_support, rejected_claim_count);
                    let before = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count,
                        rejected_claim_count,
                        confidence_bps,
                    );
                    let after = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count + 1,
                        rejected_claim_count,
                        confidence_bps,
                    );

                    if rejection_bps <= 9_000
                        && u32::from(before_support) + 10 <= u32::from(after_support)
                    {
                        assert!(
                            before < after,
                            "expected tenth-headroom ten-gap condition to force a strict lift for claim_count={claim_count}, corroborated_claim_count={corroborated_claim_count}, rejected_claim_count={rejected_claim_count}, confidence_bps={confidence_bps}, before_support={before_support}, after_support={after_support}, rejection_bps={rejection_bps}, before={before}, after={after}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn last_open_fixed_rejection_eleven_gap_under_eleventh_headroom_is_strict() {
        for confidence_bps in [0_u16, 1, 1_000, 5_000, 8_499, 8_500, 9_100, 10_000] {
            for corroborated_claim_count in 0..=6 {
                for rejected_claim_count in 0..=4 {
                    let claim_count = corroborated_claim_count + rejected_claim_count + 1;
                    let corroborated_support = corroborated_support_bps(confidence_bps);
                    let proposal_support = proposal_support_bps(confidence_bps);
                    let rejection_support = rejection_support_bps(confidence_bps);
                    let base_support =
                        accumulate_noisy_or(0, corroborated_support, corroborated_claim_count);
                    let before_support = accumulate_noisy_or(base_support, proposal_support, 1);
                    let after_support = accumulate_noisy_or(base_support, corroborated_support, 1);
                    let rejection_bps =
                        accumulate_noisy_or(0, rejection_support, rejected_claim_count);
                    let before = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count,
                        rejected_claim_count,
                        confidence_bps,
                    );
                    let after = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count + 1,
                        rejected_claim_count,
                        confidence_bps,
                    );

                    if rejection_bps <= 9_090
                        && u32::from(before_support) + 11 <= u32::from(after_support)
                    {
                        assert!(
                            before < after,
                            "expected eleventh-headroom eleven-gap condition to force a strict lift for claim_count={claim_count}, corroborated_claim_count={corroborated_claim_count}, rejected_claim_count={rejected_claim_count}, confidence_bps={confidence_bps}, before_support={before_support}, after_support={after_support}, rejection_bps={rejection_bps}, before={before}, after={after}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn last_open_fixed_rejection_twelve_gap_under_twelfth_headroom_is_strict() {
        for confidence_bps in [0_u16, 1, 1_000, 5_000, 8_499, 8_500, 9_100, 10_000] {
            for corroborated_claim_count in 0..=6 {
                for rejected_claim_count in 0..=4 {
                    let claim_count = corroborated_claim_count + rejected_claim_count + 1;
                    let corroborated_support = corroborated_support_bps(confidence_bps);
                    let proposal_support = proposal_support_bps(confidence_bps);
                    let rejection_support = rejection_support_bps(confidence_bps);
                    let base_support =
                        accumulate_noisy_or(0, corroborated_support, corroborated_claim_count);
                    let before_support = accumulate_noisy_or(base_support, proposal_support, 1);
                    let after_support = accumulate_noisy_or(base_support, corroborated_support, 1);
                    let rejection_bps =
                        accumulate_noisy_or(0, rejection_support, rejected_claim_count);
                    let before = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count,
                        rejected_claim_count,
                        confidence_bps,
                    );
                    let after = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count + 1,
                        rejected_claim_count,
                        confidence_bps,
                    );

                    if rejection_bps <= 9_166
                        && u32::from(before_support) + 12 <= u32::from(after_support)
                    {
                        assert!(
                            before < after,
                            "expected twelfth-headroom twelve-gap condition to force a strict lift for claim_count={claim_count}, corroborated_claim_count={corroborated_claim_count}, rejected_claim_count={rejected_claim_count}, confidence_bps={confidence_bps}, before_support={before_support}, after_support={after_support}, rejection_bps={rejection_bps}, before={before}, after={after}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn last_open_fixed_rejection_thirteen_gap_under_thirteenth_headroom_is_strict() {
        for confidence_bps in [0_u16, 1, 1_000, 5_000, 8_499, 8_500, 9_100, 10_000] {
            for corroborated_claim_count in 0..=6 {
                for rejected_claim_count in 0..=4 {
                    let claim_count = corroborated_claim_count + rejected_claim_count + 1;
                    let corroborated_support = corroborated_support_bps(confidence_bps);
                    let proposal_support = proposal_support_bps(confidence_bps);
                    let rejection_support = rejection_support_bps(confidence_bps);
                    let base_support =
                        accumulate_noisy_or(0, corroborated_support, corroborated_claim_count);
                    let before_support = accumulate_noisy_or(base_support, proposal_support, 1);
                    let after_support = accumulate_noisy_or(base_support, corroborated_support, 1);
                    let rejection_bps =
                        accumulate_noisy_or(0, rejection_support, rejected_claim_count);
                    let before = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count,
                        rejected_claim_count,
                        confidence_bps,
                    );
                    let after = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count + 1,
                        rejected_claim_count,
                        confidence_bps,
                    );

                    if rejection_bps <= 9_230
                        && u32::from(before_support) + 13 <= u32::from(after_support)
                    {
                        assert!(
                            before < after,
                            "expected thirteenth-headroom thirteen-gap condition to force a strict lift for claim_count={claim_count}, corroborated_claim_count={corroborated_claim_count}, rejected_claim_count={rejected_claim_count}, confidence_bps={confidence_bps}, before_support={before_support}, after_support={after_support}, rejection_bps={rejection_bps}, before={before}, after={after}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn last_open_fixed_rejection_fourteen_gap_under_fourteenth_headroom_is_strict() {
        for confidence_bps in [0_u16, 1, 1_000, 5_000, 8_499, 8_500, 9_100, 10_000] {
            for corroborated_claim_count in 0..=6 {
                for rejected_claim_count in 0..=4 {
                    let claim_count = corroborated_claim_count + rejected_claim_count + 1;
                    let corroborated_support = corroborated_support_bps(confidence_bps);
                    let proposal_support = proposal_support_bps(confidence_bps);
                    let rejection_support = rejection_support_bps(confidence_bps);
                    let base_support =
                        accumulate_noisy_or(0, corroborated_support, corroborated_claim_count);
                    let before_support = accumulate_noisy_or(base_support, proposal_support, 1);
                    let after_support = accumulate_noisy_or(base_support, corroborated_support, 1);
                    let rejection_bps =
                        accumulate_noisy_or(0, rejection_support, rejected_claim_count);
                    let before = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count,
                        rejected_claim_count,
                        confidence_bps,
                    );
                    let after = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count + 1,
                        rejected_claim_count,
                        confidence_bps,
                    );

                    if rejection_bps <= 9_285
                        && u32::from(before_support) + 14 <= u32::from(after_support)
                    {
                        assert!(
                            before < after,
                            "expected fourteenth-headroom fourteen-gap condition to force a strict lift for claim_count={claim_count}, corroborated_claim_count={corroborated_claim_count}, rejected_claim_count={rejected_claim_count}, confidence_bps={confidence_bps}, before_support={before_support}, after_support={after_support}, rejection_bps={rejection_bps}, before={before}, after={after}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn last_open_fixed_rejection_fifteen_gap_under_fifteenth_headroom_is_strict() {
        for confidence_bps in [0_u16, 1, 1_000, 5_000, 8_499, 8_500, 9_100, 10_000] {
            for corroborated_claim_count in 0..=6 {
                for rejected_claim_count in 0..=4 {
                    let claim_count = corroborated_claim_count + rejected_claim_count + 1;
                    let corroborated_support = corroborated_support_bps(confidence_bps);
                    let proposal_support = proposal_support_bps(confidence_bps);
                    let rejection_support = rejection_support_bps(confidence_bps);
                    let base_support =
                        accumulate_noisy_or(0, corroborated_support, corroborated_claim_count);
                    let before_support = accumulate_noisy_or(base_support, proposal_support, 1);
                    let after_support = accumulate_noisy_or(base_support, corroborated_support, 1);
                    let rejection_bps =
                        accumulate_noisy_or(0, rejection_support, rejected_claim_count);
                    let before = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count,
                        rejected_claim_count,
                        confidence_bps,
                    );
                    let after = fused_credibility_bps(
                        claim_count,
                        corroborated_claim_count + 1,
                        rejected_claim_count,
                        confidence_bps,
                    );

                    if rejection_bps <= 9_333
                        && u32::from(before_support) + 15 <= u32::from(after_support)
                    {
                        assert!(
                            before < after,
                            "expected fifteenth-headroom fifteen-gap condition to force a strict lift for claim_count={claim_count}, corroborated_claim_count={corroborated_claim_count}, rejected_claim_count={rejected_claim_count}, confidence_bps={confidence_bps}, before_support={before_support}, after_support={after_support}, rejection_bps={rejection_bps}, before={before}, after={after}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn single_proposal_does_not_max_out_corroboration() {
        let proposal = fused_credibility_bps(1, 0, 0, 9_400);
        let corroborated = fused_credibility_bps(1, 1, 0, 9_400);
        let double_corroborated = fused_credibility_bps(2, 2, 0, 9_400);

        assert!(proposal < 8_000);
        assert!(credibility_tier(proposal) < credibility_tier(corroborated));
        assert_eq!(credibility_tier(double_corroborated), 5);
    }
}
