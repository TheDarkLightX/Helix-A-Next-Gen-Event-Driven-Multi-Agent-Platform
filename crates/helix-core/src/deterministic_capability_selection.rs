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

//! Pure, deterministic capability selection for bounded model prompts.
//!
//! The selector reduces the capability catalog presented to an untrusted model.
//! It does not authorize execution: every selected capability remains subject to
//! Helix policy, guard, and executor checks.

use crate::{
    deterministic_agent_catalog::high_roi_agent_catalog,
    deterministic_agents_expanded::EXPANDED_AGENT_DESCRIPTORS,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const MAX_QUERY_BYTES: usize = 8 * 1024;
const MAX_SELECTED_CAPABILITIES: usize = 64;
const CATALOG_DIGEST_DOMAIN_V1: &[u8] = b"helix:capability-catalog:v1";

/// Coarse risk class used to bound which capabilities may enter a model prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityRisk {
    Low,
    Moderate,
    High,
    Critical,
}

impl CapabilityRisk {
    fn tag(self) -> u8 {
        match self {
            Self::Low => 0,
            Self::Moderate => 1,
            Self::High => 2,
            Self::Critical => 3,
        }
    }
}

/// Stable metadata for one model-visible capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityDescriptor {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub risk: CapabilityRisk,
}

/// Explicit inputs to the pure selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySelectionRequest {
    pub query: String,
    pub max_selected: usize,
    pub max_risk: CapabilityRisk,
    #[serde(default)]
    pub required_ids: Vec<String>,
}

/// One capability selected for prompt construction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedCapability {
    pub id: String,
    pub score: u32,
    pub matched_terms: Vec<String>,
    pub required: bool,
}

/// Replayable evidence for a capability-selection decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySelectionReceipt {
    pub catalog_digest: String,
    pub query_terms: Vec<String>,
    pub selected: Vec<SelectedCapability>,
    pub omitted_count: usize,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CapabilitySelectionError {
    #[error("capability catalog is empty")]
    EmptyCatalog,
    #[error("capability id must be non-empty and canonical: {0:?}")]
    InvalidCapabilityId(String),
    #[error("capability name must be non-empty for id {0}")]
    EmptyCapabilityName(String),
    #[error("duplicate capability id: {0}")]
    DuplicateCapabilityId(String),
    #[error("query must contain at least one ASCII alphanumeric term")]
    EmptyQuery,
    #[error("query exceeds the {MAX_QUERY_BYTES}-byte bound")]
    QueryTooLong,
    #[error("max_selected must be between 1 and {MAX_SELECTED_CAPABILITIES}, found {0}")]
    InvalidSelectionBudget(usize),
    #[error("required capability id must be non-empty")]
    InvalidRequiredCapabilityId,
    #[error("unknown required capability: {0}")]
    UnknownRequiredCapability(String),
    #[error("required capability {id} has risk {risk:?}, above request maximum {max_risk:?}")]
    RequiredCapabilityAboveRisk {
        id: String,
        risk: CapabilityRisk,
        max_risk: CapabilityRisk,
    },
    #[error("{required} required capabilities exceed max_selected={max_selected}")]
    RequiredCapabilitiesExceedBudget {
        required: usize,
        max_selected: usize,
    },
    #[error("no capability matched the bounded request")]
    NoMatchingCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScoredCapability {
    id: String,
    score: u32,
    matched_terms: Vec<String>,
    required: bool,
}

/// Convert Helix's deterministic agent catalog into selector descriptors.
///
/// Risk is conservative metadata for prompt exposure only. It is not a grant of
/// authority and cannot bypass the policy or execution boundary.
#[must_use]
pub fn deterministic_agent_capabilities() -> Vec<CapabilityDescriptor> {
    high_roi_agent_catalog()
        .into_iter()
        .map(|agent| CapabilityDescriptor {
            risk: prompt_exposure_risk(&agent.id),
            id: agent.id,
            name: agent.name,
            summary: agent.roi_rationale,
            keywords: vec![agent.kernel_module, agent.formal_model],
        })
        .collect()
}

fn prompt_exposure_risk(agent_id: &str) -> CapabilityRisk {
    match agent_id {
        "dedup_window" | "token_bucket" | "circuit_breaker" | "retry_budget" | "backpressure"
        | "sla_deadline" | "dlq_budget" => CapabilityRisk::Low,
        "approval_gate"
        | "finality_guard"
        | "allowlist_guard"
        | "symbolic_reasoning_gate"
        | "expert_system_gate"
        | "neuro_risk_gate"
        | "neuro_symbolic_fusion_gate" => CapabilityRisk::Moderate,
        "nonce_manager" | "fee_bidding" | "onchain_tx_intent" => CapabilityRisk::High,
        expanded
            if EXPANDED_AGENT_DESCRIPTORS
                .iter()
                .any(|descriptor| descriptor.id == expanded) =>
        {
            CapabilityRisk::Moderate
        }
        // New catalog entries remain unavailable below Critical until a reviewer
        // assigns an explicit prompt-exposure risk above.
        _ => CapabilityRisk::Critical,
    }
}

/// Select a bounded capability subset using deterministic lexical evidence.
///
/// The same canonical catalog and request always produce the same receipt. The
/// function fails closed rather than returning the full catalog on ambiguity or
/// selector failure.
pub fn select_capabilities(
    catalog: &[CapabilityDescriptor],
    request: &CapabilitySelectionRequest,
) -> Result<CapabilitySelectionReceipt, CapabilitySelectionError> {
    if request.query.len() > MAX_QUERY_BYTES {
        return Err(CapabilitySelectionError::QueryTooLong);
    }
    if request.max_selected == 0 || request.max_selected > MAX_SELECTED_CAPABILITIES {
        return Err(CapabilitySelectionError::InvalidSelectionBudget(
            request.max_selected,
        ));
    }

    let query_terms = normalized_terms(&request.query);
    if query_terms.is_empty() {
        return Err(CapabilitySelectionError::EmptyQuery);
    }

    let by_id = validate_catalog(catalog)?;
    let required_ids = normalize_required_ids(&request.required_ids)?;
    if required_ids.len() > request.max_selected {
        return Err(CapabilitySelectionError::RequiredCapabilitiesExceedBudget {
            required: required_ids.len(),
            max_selected: request.max_selected,
        });
    }

    for id in &required_ids {
        let capability = by_id
            .get(id)
            .ok_or_else(|| CapabilitySelectionError::UnknownRequiredCapability(id.clone()))?;
        if capability.risk > request.max_risk {
            return Err(CapabilitySelectionError::RequiredCapabilityAboveRisk {
                id: id.clone(),
                risk: capability.risk,
                max_risk: request.max_risk,
            });
        }
    }

    let mut scored = Vec::new();
    for (id, capability) in &by_id {
        if capability.risk > request.max_risk {
            continue;
        }
        let required = required_ids.contains(id);
        let (score, matched_terms) = score_capability(capability, &query_terms);
        if required || score > 0 {
            scored.push(ScoredCapability {
                id: id.clone(),
                score,
                matched_terms,
                required,
            });
        }
    }

    scored.sort_by(|left, right| {
        right
            .required
            .cmp(&left.required)
            .then_with(|| right.score.cmp(&left.score))
            .then_with(|| left.id.cmp(&right.id))
    });
    scored.truncate(request.max_selected);

    if scored.is_empty() {
        return Err(CapabilitySelectionError::NoMatchingCapabilities);
    }

    let selected = scored
        .into_iter()
        .map(|capability| SelectedCapability {
            id: capability.id,
            score: capability.score,
            matched_terms: capability.matched_terms,
            required: capability.required,
        })
        .collect::<Vec<_>>();

    Ok(CapabilitySelectionReceipt {
        catalog_digest: capability_catalog_digest(catalog)?,
        query_terms,
        omitted_count: catalog.len().saturating_sub(selected.len()),
        selected,
    })
}

/// Hash the semantic catalog in canonical id order with sorted, deduplicated keywords.
pub fn capability_catalog_digest(
    catalog: &[CapabilityDescriptor],
) -> Result<String, CapabilitySelectionError> {
    let by_id = validate_catalog(catalog)?;
    let mut hasher = Sha256::new();
    hasher.update(CATALOG_DIGEST_DOMAIN_V1);
    hasher.update((by_id.len() as u64).to_le_bytes());

    for (id, capability) in by_id {
        hash_field(&mut hasher, id.as_bytes());
        hash_field(&mut hasher, capability.name.as_bytes());
        hash_field(&mut hasher, capability.summary.as_bytes());
        hasher.update([capability.risk.tag()]);

        let keywords = capability
            .keywords
            .iter()
            .map(|keyword| keyword.trim())
            .filter(|keyword| !keyword.is_empty())
            .collect::<BTreeSet<_>>();
        hasher.update((keywords.len() as u64).to_le_bytes());
        for keyword in keywords {
            hash_field(&mut hasher, keyword.as_bytes());
        }
    }

    Ok(hex_lower(&hasher.finalize()))
}

fn validate_catalog(
    catalog: &[CapabilityDescriptor],
) -> Result<BTreeMap<String, &CapabilityDescriptor>, CapabilitySelectionError> {
    if catalog.is_empty() {
        return Err(CapabilitySelectionError::EmptyCatalog);
    }

    let mut by_id = BTreeMap::new();
    for capability in catalog {
        if capability.id.is_empty() || capability.id.trim() != capability.id {
            return Err(CapabilitySelectionError::InvalidCapabilityId(
                capability.id.clone(),
            ));
        }
        if capability.name.trim().is_empty() {
            return Err(CapabilitySelectionError::EmptyCapabilityName(
                capability.id.clone(),
            ));
        }
        if by_id.insert(capability.id.clone(), capability).is_some() {
            return Err(CapabilitySelectionError::DuplicateCapabilityId(
                capability.id.clone(),
            ));
        }
    }
    Ok(by_id)
}

fn normalize_required_ids(
    required_ids: &[String],
) -> Result<BTreeSet<String>, CapabilitySelectionError> {
    let mut normalized = BTreeSet::new();
    for id in required_ids {
        if id.is_empty() || id.trim() != id {
            return Err(CapabilitySelectionError::InvalidRequiredCapabilityId);
        }
        normalized.insert(id.clone());
    }
    Ok(normalized)
}

fn score_capability(
    capability: &CapabilityDescriptor,
    query_terms: &[String],
) -> (u32, Vec<String>) {
    let id_terms = normalized_terms(&capability.id)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let name_terms = normalized_terms(&capability.name)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let summary_terms = normalized_terms(&capability.summary)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let keyword_terms = capability
        .keywords
        .iter()
        .flat_map(|keyword| normalized_terms(keyword))
        .collect::<BTreeSet<_>>();

    let mut score = 0u32;
    let mut matched_terms = Vec::new();
    for term in query_terms {
        let mut term_score = 0u32;
        if id_terms.contains(term) {
            term_score = term_score.saturating_add(16);
        }
        if name_terms.contains(term) {
            term_score = term_score.saturating_add(8);
        }
        if keyword_terms.contains(term) {
            term_score = term_score.saturating_add(6);
        }
        if summary_terms.contains(term) {
            term_score = term_score.saturating_add(3);
        }
        if term_score > 0 {
            matched_terms.push(term.clone());
            score = score.saturating_add(term_score);
        }
    }
    (score, matched_terms)
}

fn normalized_terms(value: &str) -> Vec<String> {
    let mut terms = BTreeSet::new();
    let mut current = String::new();

    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            current.push(character.to_ascii_lowercase());
        } else if !current.is_empty() {
            terms.insert(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        terms.insert(current);
    }

    terms.into_iter().collect()
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(
        id: &str,
        name: &str,
        summary: &str,
        risk: CapabilityRisk,
    ) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: id.to_string(),
            name: name.to_string(),
            summary: summary.to_string(),
            keywords: Vec::new(),
            risk,
        }
    }

    fn request(query: &str, max_selected: usize) -> CapabilitySelectionRequest {
        CapabilitySelectionRequest {
            query: query.to_string(),
            max_selected,
            max_risk: CapabilityRisk::High,
            required_ids: Vec::new(),
        }
    }

    #[test]
    fn unknown_agent_ids_default_to_critical_risk() {
        assert_eq!(
            prompt_exposure_risk("new_unreviewed_side_effect_agent"),
            CapabilityRisk::Critical
        );
    }

    #[test]
    fn shipped_agent_catalog_has_reviewed_risk_assignments() {
        let unreviewed = deterministic_agent_capabilities()
            .into_iter()
            .filter(|capability| capability.risk == CapabilityRisk::Critical)
            .map(|capability| capability.id)
            .collect::<Vec<_>>();
        assert!(
            unreviewed.is_empty(),
            "shipped capabilities need explicit prompt-risk review: {unreviewed:?}"
        );
    }

    #[test]
    fn catalog_and_query_order_do_not_change_receipt() {
        let a = descriptor(
            "case_export",
            "Case Export",
            "Export a replayable case evidence packet",
            CapabilityRisk::Low,
        );
        let b = descriptor(
            "case_rank",
            "Case Rank",
            "Rank case evidence deterministically",
            CapabilityRisk::Low,
        );
        let left = select_capabilities(&[a.clone(), b.clone()], &request("case evidence", 2))
            .expect("selection");
        let right = select_capabilities(&[b, a], &request("evidence case", 2)).expect("selection");
        assert_eq!(left, right);
    }

    #[test]
    fn ties_break_by_stable_capability_id() {
        let catalog = vec![
            descriptor("zeta", "Export", "case packet", CapabilityRisk::Low),
            descriptor("alpha", "Export", "case packet", CapabilityRisk::Low),
        ];
        let receipt = select_capabilities(&catalog, &request("export", 1)).expect("selection");
        assert_eq!(receipt.selected[0].id, "alpha");
    }

    #[test]
    fn required_capabilities_are_selected_without_bypassing_risk() {
        let catalog = vec![
            descriptor("read", "Read", "read evidence", CapabilityRisk::Low),
            descriptor(
                "broadcast",
                "Broadcast",
                "send transaction",
                CapabilityRisk::Critical,
            ),
        ];
        let mut req = request("read", 2);
        req.max_risk = CapabilityRisk::High;
        req.required_ids = vec!["broadcast".to_string()];

        assert!(matches!(
            select_capabilities(&catalog, &req),
            Err(CapabilitySelectionError::RequiredCapabilityAboveRisk { id, .. }) if id == "broadcast"
        ));
    }

    #[test]
    fn required_set_must_fit_inside_budget() {
        let catalog = vec![
            descriptor("a", "A", "alpha", CapabilityRisk::Low),
            descriptor("b", "B", "beta", CapabilityRisk::Low),
        ];
        let mut req = request("alpha", 1);
        req.required_ids = vec!["a".to_string(), "b".to_string()];
        assert!(matches!(
            select_capabilities(&catalog, &req),
            Err(CapabilitySelectionError::RequiredCapabilitiesExceedBudget {
                required: 2,
                max_selected: 1
            })
        ));
    }

    #[test]
    fn over_risk_nonrequired_capabilities_are_omitted() {
        let catalog = vec![
            descriptor(
                "safe_search",
                "Search",
                "search evidence",
                CapabilityRisk::Low,
            ),
            descriptor(
                "unsafe_search",
                "Search",
                "search evidence",
                CapabilityRisk::Critical,
            ),
        ];
        let mut req = request("search", 2);
        req.max_risk = CapabilityRisk::Moderate;
        let receipt = select_capabilities(&catalog, &req).expect("selection");
        assert_eq!(receipt.selected.len(), 1);
        assert_eq!(receipt.selected[0].id, "safe_search");
    }

    #[test]
    fn duplicate_ids_fail_closed() {
        let duplicate = descriptor("same", "Same", "same", CapabilityRisk::Low);
        assert!(matches!(
            select_capabilities(&[duplicate.clone(), duplicate], &request("same", 1)),
            Err(CapabilitySelectionError::DuplicateCapabilityId(id)) if id == "same"
        ));
    }

    #[test]
    fn no_match_fails_closed_instead_of_returning_full_catalog() {
        let catalog = vec![descriptor(
            "case_export",
            "Case Export",
            "export evidence",
            CapabilityRisk::Low,
        )];
        assert_eq!(
            select_capabilities(&catalog, &request("weather", 1)),
            Err(CapabilitySelectionError::NoMatchingCapabilities)
        );
    }

    #[test]
    fn catalog_digest_changes_when_semantic_metadata_changes() {
        let original = vec![descriptor(
            "case_export",
            "Case Export",
            "export evidence",
            CapabilityRisk::Low,
        )];
        let changed = vec![descriptor(
            "case_export",
            "Case Export",
            "export a signed evidence packet",
            CapabilityRisk::Low,
        )];
        assert_ne!(
            capability_catalog_digest(&original).expect("digest"),
            capability_catalog_digest(&changed).expect("digest")
        );
    }
}
