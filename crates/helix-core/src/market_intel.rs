use crate::intel_desk::WatchlistSeverity;
use crate::intel_priority::{
    aggregate_attention_tier, bucket_usize, credibility_tier, fused_credibility_bps, score_case,
    severity_tier, trust_tier, IntelPriorityBreakdown,
};
use serde::{Deserialize, Serialize};

pub use crate::intel_priority::{
    CasePriorityInput as MarketCasePriorityInput,
    IntelPriorityBreakdown as MarketPriorityBreakdown, IntelSignalWindow as MarketSignalWindow,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketThemePriorityInput {
    pub max_severity: Option<WatchlistSeverity>,
    pub active_case_count: usize,
    pub escalated_case_count: usize,
    pub watchlist_count: usize,
    pub evidence_count: usize,
    pub claim_count: usize,
    pub corroborated_claim_count: usize,
    pub rejected_claim_count: usize,
    pub max_claim_confidence_bps: u16,
    pub source_trust_scores: Vec<u8>,
    pub latest_signal_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketCompanyPriorityInput {
    pub max_severity: Option<WatchlistSeverity>,
    pub active_case_count: usize,
    pub escalated_case_count: usize,
    pub mention_count: usize,
    pub claim_count: usize,
    pub corroborated_claim_count: usize,
    pub rejected_claim_count: usize,
    pub max_claim_confidence_bps: u16,
    pub source_trust_scores: Vec<u8>,
    pub latest_signal_at: Option<String>,
}

pub fn score_market_case(
    input: &MarketCasePriorityInput,
    window: &MarketSignalWindow,
) -> MarketPriorityBreakdown {
    score_case(input, window)
}

pub fn score_market_theme(
    input: &MarketThemePriorityInput,
    window: &MarketSignalWindow,
) -> MarketPriorityBreakdown {
    let attention_tier =
        aggregate_attention_tier(input.active_case_count, input.escalated_case_count);
    let severity_tier = severity_tier(input.max_severity);
    let credibility_bps = fused_credibility_bps(
        input.claim_count,
        input.corroborated_claim_count,
        input.rejected_claim_count,
        input.max_claim_confidence_bps,
    );
    let corroboration_tier = credibility_tier(credibility_bps);
    let freshness_tier = window.freshness_tier(input.latest_signal_at.as_deref());
    let trust_tier = trust_tier(&input.source_trust_scores);
    let density_tier = bucket_usize(
        input
            .evidence_count
            .saturating_add(input.watchlist_count)
            .saturating_add(input.active_case_count.saturating_mul(2)),
        &[1, 3, 5, 8, 12],
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

pub fn score_market_company(
    input: &MarketCompanyPriorityInput,
    window: &MarketSignalWindow,
) -> MarketPriorityBreakdown {
    let attention_tier =
        aggregate_attention_tier(input.active_case_count, input.escalated_case_count);
    let severity_tier = severity_tier(input.max_severity);
    let credibility_bps = fused_credibility_bps(
        input.claim_count,
        input.corroborated_claim_count,
        input.rejected_claim_count,
        input.max_claim_confidence_bps,
    );
    let corroboration_tier = credibility_tier(credibility_bps);
    let freshness_tier = window.freshness_tier(input.latest_signal_at.as_deref());
    let trust_tier = trust_tier(&input.source_trust_scores);
    let density_tier = bucket_usize(
        input
            .mention_count
            .saturating_add(input.claim_count)
            .saturating_add(input.active_case_count.saturating_mul(2)),
        &[1, 3, 5, 8, 12],
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intel_desk::{CaseStatus, WatchlistSeverity};
    use crate::intel_priority::CasePriorityInput;

    fn window() -> MarketSignalWindow {
        MarketSignalWindow::from_observed_at_values([
            "2026-03-10T12:00:00Z",
            "2026-03-09T12:00:00Z",
        ])
    }

    #[test]
    fn corroborated_case_beats_unreviewed_case_given_same_attention_and_severity() {
        let window = window();
        let corroborated = score_market_case(
            &CasePriorityInput {
                status: CaseStatus::Open,
                severity: WatchlistSeverity::High,
                source_trust_scores: vec![86],
                evidence_count: 2,
                claim_count: 2,
                corroborated_claim_count: 1,
                rejected_claim_count: 0,
                max_claim_confidence_bps: 8_900,
                latest_signal_at: Some("2026-03-10T08:00:00Z".to_string()),
                attached_to_case: false,
            },
            &window,
        );
        let unreviewed = score_market_case(
            &CasePriorityInput {
                status: CaseStatus::Open,
                severity: WatchlistSeverity::High,
                source_trust_scores: vec![86],
                evidence_count: 2,
                claim_count: 2,
                corroborated_claim_count: 0,
                rejected_claim_count: 0,
                max_claim_confidence_bps: 8_900,
                latest_signal_at: Some("2026-03-10T08:00:00Z".to_string()),
                attached_to_case: false,
            },
            &window,
        );

        assert!(corroborated.total > unreviewed.total);
        assert!(corroborated.corroboration_tier > unreviewed.corroboration_tier);
    }

    #[test]
    fn theme_with_escalation_outranks_background_theme() {
        let window = window();
        let urgent = score_market_theme(
            &MarketThemePriorityInput {
                max_severity: Some(WatchlistSeverity::Critical),
                active_case_count: 2,
                escalated_case_count: 1,
                watchlist_count: 2,
                evidence_count: 3,
                claim_count: 3,
                corroborated_claim_count: 1,
                rejected_claim_count: 0,
                max_claim_confidence_bps: 9_100,
                source_trust_scores: vec![88, 90, 92],
                latest_signal_at: Some("2026-03-10T11:00:00Z".to_string()),
            },
            &window,
        );
        let passive = score_market_theme(
            &MarketThemePriorityInput {
                max_severity: Some(WatchlistSeverity::Low),
                active_case_count: 0,
                escalated_case_count: 0,
                watchlist_count: 1,
                evidence_count: 1,
                claim_count: 0,
                corroborated_claim_count: 0,
                rejected_claim_count: 0,
                max_claim_confidence_bps: 0,
                source_trust_scores: vec![65],
                latest_signal_at: Some("2026-03-07T11:00:00Z".to_string()),
            },
            &window,
        );

        assert!(urgent.total > passive.total);
        assert_eq!(urgent.attention_tier, 5);
    }
}
