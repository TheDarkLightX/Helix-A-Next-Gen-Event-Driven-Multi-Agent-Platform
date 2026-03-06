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

//! Deterministic neuro-symbolic reasoning backends.
//!
//! This module provides a stable facade over compiled reasoning engines:
//! - `krr_symbolic`: indexed symbolic closure over facts/rules/triples
//! - `expert_system`: compiled threshold-rule matching with deterministic voting
//! - `neuro`: deterministic linear model inference
//! - `neuro_symbolic`: fail-closed symbolic gate + neural confidence fusion

mod expert;
mod neuro;
mod symbolic;

use crate::HelixError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use expert::evaluate_expert_system;
use neuro::{evaluate_neuro, score_neuro_probability};
use symbolic::{
    evaluate_compiled_symbolic, SymbolicClosureStatus, SymbolicEvaluation, SymbolicEvaluationScope,
};
pub use symbolic::{fingerprint_symbolic_program, CompiledSymbolicProgram};

/// Supported reasoning backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningBackend {
    /// KRR + symbolic forward chaining.
    KrrSymbolic,
    /// Weighted expert-rule voting system.
    ExpertSystem,
    /// Deterministic neural model scoring.
    Neuro,
    /// Symbolic guard fused with neural confidence.
    NeuroSymbolic,
}

/// Final verdict produced by reasoning backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningVerdict {
    /// Permit execution.
    Allow,
    /// Require additional validation/human review.
    Review,
    /// Deny execution (fail closed).
    Deny,
}

/// Contradiction scope used for symbolic consistency checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningConsistencyScope {
    /// Any contradiction in the symbolic closure blocks the decision.
    #[default]
    Global,
    /// Only contradictions touching the query support slice block the decision.
    QuerySupport,
}

/// Symbolic implication rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolicRule {
    /// Stable rule identifier.
    pub id: String,
    /// Antecedent facts that must all hold.
    pub antecedents: Vec<String>,
    /// Consequent fact inferred when antecedents hold.
    pub consequent: String,
}

/// Knowledge graph relation triple.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KrrTriple {
    /// Subject node.
    pub subject: String,
    /// Predicate/relation name.
    pub predicate: String,
    /// Object node.
    pub object: String,
}

/// Expert-system rule with feature thresholds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpertRule {
    /// Stable rule identifier.
    pub id: String,
    /// Per-feature minimum values required for match.
    pub min_features: BTreeMap<String, i64>,
    /// Outcome asserted by this rule.
    pub verdict: ReasoningVerdict,
    /// Integer confidence weight for deterministic voting.
    pub weight: u16,
}

/// Deterministic linear model configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinearModel {
    /// Bias term.
    pub bias: f64,
    /// Feature weights.
    pub weights: BTreeMap<String, f64>,
    /// Probability threshold for allow.
    pub allow_threshold: f64,
    /// Probability threshold for review.
    pub review_threshold: f64,
}

/// Input request for reasoning evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "backend", rename_all = "snake_case")]
pub enum ReasoningEvaluationRequest {
    /// KRR + symbolic query check.
    KrrSymbolic {
        /// Query fact to prove.
        query: String,
        /// Seed facts.
        facts: Vec<String>,
        /// Implication rules.
        rules: Vec<SymbolicRule>,
        /// Knowledge graph triples converted to symbolic facts.
        triples: Vec<KrrTriple>,
        /// Contradiction scope used for fail-closed consistency checks.
        consistency_scope: Option<ReasoningConsistencyScope>,
        /// Maximum fixpoint rounds.
        max_rounds: Option<u8>,
    },
    /// Expert-system weighted voting.
    ExpertSystem {
        /// Integer features.
        features: BTreeMap<String, i64>,
        /// Expert rules.
        rules: Vec<ExpertRule>,
    },
    /// Pure neural inference.
    Neuro {
        /// Floating-point features.
        features: BTreeMap<String, f64>,
        /// Deterministic linear model.
        model: LinearModel,
    },
    /// Symbolic gate fused with neural confidence.
    NeuroSymbolic {
        /// Query fact that must be symbolically entailed.
        query: String,
        /// Seed facts.
        facts: Vec<String>,
        /// Implication rules.
        rules: Vec<SymbolicRule>,
        /// Knowledge graph triples.
        triples: Vec<KrrTriple>,
        /// Floating-point features for neural backend.
        features: BTreeMap<String, f64>,
        /// Deterministic linear model.
        model: LinearModel,
        /// Minimum neural probability required when symbolic gate passes.
        min_probability: Option<f64>,
        /// Contradiction scope used for fail-closed consistency checks.
        consistency_scope: Option<ReasoningConsistencyScope>,
        /// Maximum symbolic closure rounds.
        max_rounds: Option<u8>,
    },
}

/// Internal trace for deterministic replay/auditing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasoningSupportNode {
    /// Canonical fact proven or asserted in the symbolic closure.
    pub fact: String,
    /// Whether the fact was directly asserted or derived by a rule.
    pub kind: ReasoningSupportKind,
    /// Rule that produced the fact, when derived.
    pub rule_id: Option<String>,
    /// Antecedent facts that justified the derivation.
    pub supports: Vec<String>,
}

/// Symbolic support node type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningSupportKind {
    /// Fact originated from input facts or triples.
    Seed,
    /// Fact was produced by a rule firing.
    Derived,
}

/// Contradictory literal pair discovered in a symbolic closure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasoningContradiction {
    /// Positive literal in the contradiction pair.
    pub positive: String,
    /// Negative literal in the contradiction pair.
    pub negative: String,
}

/// Symbolic closure completeness status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningSymbolicStatus {
    /// Closure reached a fixed point within the configured round bound.
    Saturated,
    /// Closure hit the configured round bound before fixed point.
    Truncated,
}

/// Internal trace for deterministic replay/auditing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningTrace {
    /// Derived symbolic facts (sorted).
    pub derived_facts: Vec<String>,
    /// Rules that fired during symbolic/expert reasoning.
    pub matched_rules: Vec<String>,
    /// Whether symbolic query was entailed.
    pub symbolic_entailed: Option<bool>,
    /// Neural backend probability output.
    pub neural_probability: Option<f64>,
    /// Deterministic explanation graph for symbolic closures.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub support_graph: Vec<ReasoningSupportNode>,
    /// Contradictions discovered in the symbolic closure.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contradictions: Vec<ReasoningContradiction>,
    /// Whether symbolic evaluation saturated or hit the round bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbolic_status: Option<ReasoningSymbolicStatus>,
    /// Number of symbolic rounds executed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbolic_rounds: Option<usize>,
    /// Ready rules left unprocessed when a symbolic run was truncated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_rule_count: Option<usize>,
    /// Stable fingerprint for the compiled symbolic program used in evaluation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program_fingerprint: Option<String>,
    /// Contradiction scope used for the symbolic decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consistency_scope: Option<ReasoningConsistencyScope>,
    /// Minimal support slice for the requested symbolic query.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub query_support: Vec<String>,
    /// Contradictions that actually blocked the verdict under the selected scope.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocking_contradictions: Vec<ReasoningContradiction>,
}

/// Deterministic reasoning decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningDecision {
    /// Backend used for evaluation.
    pub backend: ReasoningBackend,
    /// Final verdict.
    pub verdict: ReasoningVerdict,
    /// Normalized confidence score in [0, 1].
    pub confidence: f64,
    /// Human-readable rationale.
    pub rationale: String,
    /// Replay trace.
    pub trace: ReasoningTrace,
}

/// Deterministic symbolic reasoning kernel.
pub struct SymbolicReasoningKernel;

impl SymbolicReasoningKernel {
    /// Evaluates a symbolic query with KRR triples and implication rules.
    pub fn decide(
        query: String,
        facts: Vec<String>,
        rules: Vec<SymbolicRule>,
        triples: Vec<KrrTriple>,
    ) -> Result<ReasoningDecision, HelixError> {
        evaluate_reasoning(ReasoningEvaluationRequest::KrrSymbolic {
            query,
            facts,
            rules,
            triples,
            consistency_scope: None,
            max_rounds: Some(16),
        })
    }
}

/// Deterministic expert-system kernel.
pub struct ExpertSystemKernel;

impl ExpertSystemKernel {
    /// Evaluates deterministic expert rules.
    pub fn decide(
        features: BTreeMap<String, i64>,
        rules: Vec<ExpertRule>,
    ) -> Result<ReasoningDecision, HelixError> {
        evaluate_reasoning(ReasoningEvaluationRequest::ExpertSystem { features, rules })
    }
}

/// Deterministic neural scoring kernel.
pub struct NeuroRiskKernel;

impl NeuroRiskKernel {
    /// Evaluates deterministic linear model inference.
    pub fn decide(
        features: BTreeMap<String, f64>,
        model: LinearModel,
    ) -> Result<ReasoningDecision, HelixError> {
        evaluate_reasoning(ReasoningEvaluationRequest::Neuro { features, model })
    }
}

/// Deterministic neuro-symbolic fusion kernel.
pub struct NeuroSymbolicFusionKernel;

impl NeuroSymbolicFusionKernel {
    /// Evaluates symbolic gate + neural confidence fusion.
    #[allow(clippy::too_many_arguments)]
    pub fn decide(
        query: String,
        facts: Vec<String>,
        rules: Vec<SymbolicRule>,
        triples: Vec<KrrTriple>,
        features: BTreeMap<String, f64>,
        model: LinearModel,
        min_probability: Option<f64>,
    ) -> Result<ReasoningDecision, HelixError> {
        evaluate_reasoning(ReasoningEvaluationRequest::NeuroSymbolic {
            query,
            facts,
            rules,
            triples,
            features,
            model,
            min_probability,
            consistency_scope: None,
            max_rounds: Some(16),
        })
    }
}

/// Evaluates one deterministic reasoning request.
pub fn evaluate_reasoning(
    request: ReasoningEvaluationRequest,
) -> Result<ReasoningDecision, HelixError> {
    match request {
        ReasoningEvaluationRequest::KrrSymbolic {
            query,
            facts,
            rules,
            triples,
            consistency_scope,
            max_rounds,
        } => {
            let program = compile_symbolic_program(rules, triples)?;
            evaluate_compiled_symbolic_reasoning(
                query,
                facts,
                &program,
                consistency_scope,
                max_rounds,
            )
        }
        ReasoningEvaluationRequest::ExpertSystem { features, rules } => {
            let expert = evaluate_expert_system(features, rules)?;
            Ok(ReasoningDecision {
                backend: ReasoningBackend::ExpertSystem,
                verdict: expert.verdict,
                confidence: expert.confidence,
                rationale: expert.rationale,
                trace: ReasoningTrace {
                    derived_facts: Vec::new(),
                    matched_rules: expert.matched_rules,
                    symbolic_entailed: None,
                    neural_probability: None,
                    support_graph: Vec::new(),
                    contradictions: Vec::new(),
                    symbolic_status: None,
                    symbolic_rounds: None,
                    pending_rule_count: None,
                    program_fingerprint: None,
                    consistency_scope: None,
                    query_support: Vec::new(),
                    blocking_contradictions: Vec::new(),
                },
            })
        }
        ReasoningEvaluationRequest::Neuro { features, model } => {
            let neuro = evaluate_neuro(features, model)?;
            Ok(ReasoningDecision {
                backend: ReasoningBackend::Neuro,
                verdict: neuro.verdict,
                confidence: neuro.confidence,
                rationale: "deterministic linear-model inference".to_string(),
                trace: ReasoningTrace {
                    derived_facts: Vec::new(),
                    matched_rules: Vec::new(),
                    symbolic_entailed: None,
                    neural_probability: Some(neuro.probability),
                    support_graph: Vec::new(),
                    contradictions: Vec::new(),
                    symbolic_status: None,
                    symbolic_rounds: None,
                    pending_rule_count: None,
                    program_fingerprint: None,
                    consistency_scope: None,
                    query_support: Vec::new(),
                    blocking_contradictions: Vec::new(),
                },
            })
        }
        ReasoningEvaluationRequest::NeuroSymbolic {
            query,
            facts,
            rules,
            triples,
            features,
            model,
            min_probability,
            consistency_scope,
            max_rounds,
        } => {
            let program = compile_symbolic_program(rules, triples)?;
            evaluate_compiled_neuro_symbolic_reasoning(
                query,
                facts,
                &program,
                features,
                model,
                min_probability,
                consistency_scope,
                max_rounds,
            )
        }
    }
}

pub fn compile_symbolic_program(
    rules: Vec<SymbolicRule>,
    triples: Vec<KrrTriple>,
) -> Result<CompiledSymbolicProgram, HelixError> {
    CompiledSymbolicProgram::compile(rules, triples)
}

pub fn evaluate_compiled_symbolic_reasoning(
    query: String,
    facts: Vec<String>,
    program: &CompiledSymbolicProgram,
    consistency_scope: Option<ReasoningConsistencyScope>,
    max_rounds: Option<u8>,
) -> Result<ReasoningDecision, HelixError> {
    let query = normalize_non_empty(&query, "reasoning.query", "query")?;
    let max_rounds = resolve_max_rounds(max_rounds)?;
    let consistency_scope = consistency_scope.unwrap_or_default();
    let relevant = evaluate_compiled_symbolic(
        program,
        query.clone(),
        facts.clone(),
        max_rounds,
        SymbolicEvaluationScope::QueryDirected,
    )?;
    if relevant.closure_status == SymbolicClosureStatus::Truncated {
        return Ok(build_symbolic_decision(
            query,
            relevant,
            program.fingerprint(),
            consistency_scope,
            Some(
                "symbolic query evaluation truncated before saturation; fail-closed deny"
                    .to_string(),
            ),
        ));
    }
    if !relevant.entailed || has_blocking_contradictions(&relevant, consistency_scope) {
        return Ok(build_symbolic_decision(
            query,
            relevant,
            program.fingerprint(),
            consistency_scope,
            None,
        ));
    }

    let symbolic = evaluate_compiled_symbolic(
        program,
        query.clone(),
        facts,
        max_rounds,
        SymbolicEvaluationScope::Full,
    )?;
    let rationale = (symbolic.closure_status == SymbolicClosureStatus::Truncated).then(|| {
        "symbolic consistency evaluation truncated before saturation; fail-closed deny".to_string()
    });
    Ok(build_symbolic_decision(
        query,
        symbolic,
        program.fingerprint(),
        consistency_scope,
        rationale,
    ))
}

pub fn evaluate_compiled_neuro_symbolic_reasoning(
    query: String,
    facts: Vec<String>,
    program: &CompiledSymbolicProgram,
    features: BTreeMap<String, f64>,
    model: LinearModel,
    min_probability: Option<f64>,
    consistency_scope: Option<ReasoningConsistencyScope>,
    max_rounds: Option<u8>,
) -> Result<ReasoningDecision, HelixError> {
    let query = normalize_non_empty(&query, "reasoning.query", "query")?;
    let max_rounds = resolve_max_rounds(max_rounds)?;
    let min_probability =
        validate_probability(min_probability.unwrap_or(0.8), "reasoning.min_probability")?;
    let consistency_scope = consistency_scope.unwrap_or_default();
    let neuro = evaluate_neuro(features, model)?;
    let relevant = evaluate_compiled_symbolic(
        program,
        query.clone(),
        facts.clone(),
        max_rounds,
        SymbolicEvaluationScope::QueryDirected,
    )?;

    if relevant.closure_status == SymbolicClosureStatus::Truncated {
        return Ok(build_neuro_symbolic_decision(
            relevant,
            neuro.probability,
            program.fingerprint(),
            consistency_scope,
            ReasoningVerdict::Deny,
            1.0,
            "symbolic query evaluation truncated before saturation; denying regardless of neural score"
                .to_string(),
        ));
    }
    if has_blocking_contradictions(&relevant, consistency_scope) {
        return Ok(build_neuro_symbolic_decision(
            relevant,
            neuro.probability,
            program.fingerprint(),
            consistency_scope,
            ReasoningVerdict::Deny,
            1.0,
            "symbolic contradiction detected; denying regardless of neural score".to_string(),
        ));
    }
    if !relevant.entailed {
        return Ok(build_neuro_symbolic_decision(
            relevant,
            neuro.probability,
            program.fingerprint(),
            consistency_scope,
            ReasoningVerdict::Deny,
            1.0,
            "symbolic gate failed; denying regardless of neural score".to_string(),
        ));
    }

    let symbolic = evaluate_compiled_symbolic(
        program,
        query,
        facts,
        max_rounds,
        SymbolicEvaluationScope::Full,
    )?;

    let (verdict, rationale, confidence) = if symbolic.closure_status
        == SymbolicClosureStatus::Truncated
    {
        (
            ReasoningVerdict::Deny,
            "symbolic consistency evaluation truncated before saturation; denying regardless of neural score".to_string(),
            1.0,
        )
    } else if has_blocking_contradictions(&symbolic, consistency_scope) {
        (
            ReasoningVerdict::Deny,
            "symbolic contradiction detected; denying regardless of neural score".to_string(),
            1.0,
        )
    } else if !symbolic.entailed {
        (
            ReasoningVerdict::Deny,
            "symbolic gate failed; denying regardless of neural score".to_string(),
            1.0,
        )
    } else if neuro.probability >= min_probability {
        (
            ReasoningVerdict::Allow,
            "symbolic gate passed and neural confidence exceeded threshold".to_string(),
            score_neuro_probability(neuro.probability, ReasoningVerdict::Allow),
        )
    } else {
        (
            ReasoningVerdict::Review,
            "symbolic gate passed but neural confidence below allow threshold".to_string(),
            score_neuro_probability(neuro.probability, ReasoningVerdict::Review),
        )
    };

    Ok(build_neuro_symbolic_decision(
        symbolic,
        neuro.probability,
        program.fingerprint(),
        consistency_scope,
        verdict,
        confidence,
        rationale,
    ))
}

fn build_symbolic_decision(
    query: String,
    symbolic: SymbolicEvaluation,
    program_fingerprint: &str,
    consistency_scope: ReasoningConsistencyScope,
    rationale_override: Option<String>,
) -> ReasoningDecision {
    let entailed = symbolic.entailed;
    let contradiction_detected = has_blocking_contradictions(&symbolic, consistency_scope);
    let truncated = symbolic.closure_status == SymbolicClosureStatus::Truncated;
    ReasoningDecision {
        backend: ReasoningBackend::KrrSymbolic,
        verdict: if truncated || contradiction_detected {
            ReasoningVerdict::Deny
        } else if entailed {
            ReasoningVerdict::Allow
        } else {
            ReasoningVerdict::Deny
        },
        confidence: if truncated || contradiction_detected || entailed {
            1.0
        } else {
            0.0
        },
        rationale: rationale_override.unwrap_or_else(|| {
            if truncated {
                "symbolic evaluation truncated before saturation; fail-closed deny".to_string()
            } else if contradiction_detected {
                "symbolic contradiction detected; fail-closed deny".to_string()
            } else if entailed {
                format!(
                    "symbolic query entailed: {query} via {} compiled rules",
                    symbolic.matched_rules.len()
                )
            } else {
                format!("symbolic query not entailed: {query}")
            }
        }),
        trace: build_symbolic_trace(symbolic, None, program_fingerprint, consistency_scope),
    }
}

fn build_neuro_symbolic_decision(
    symbolic: SymbolicEvaluation,
    neural_probability: f64,
    program_fingerprint: &str,
    consistency_scope: ReasoningConsistencyScope,
    verdict: ReasoningVerdict,
    confidence: f64,
    rationale: String,
) -> ReasoningDecision {
    ReasoningDecision {
        backend: ReasoningBackend::NeuroSymbolic,
        verdict,
        confidence,
        rationale,
        trace: build_symbolic_trace(
            symbolic,
            Some(neural_probability),
            program_fingerprint,
            consistency_scope,
        ),
    }
}

fn build_symbolic_trace(
    symbolic: SymbolicEvaluation,
    neural_probability: Option<f64>,
    program_fingerprint: &str,
    consistency_scope: ReasoningConsistencyScope,
) -> ReasoningTrace {
    let blocking_contradictions = filter_blocking_contradictions(
        &symbolic.contradictions,
        &symbolic.query_support,
        consistency_scope,
    );
    ReasoningTrace {
        derived_facts: symbolic.derived_facts,
        matched_rules: symbolic.matched_rules,
        symbolic_entailed: Some(symbolic.entailed),
        neural_probability,
        support_graph: symbolic.support_graph,
        contradictions: symbolic.contradictions,
        symbolic_status: Some(map_symbolic_status(symbolic.closure_status)),
        symbolic_rounds: Some(symbolic.rounds_executed),
        pending_rule_count: Some(symbolic.pending_rule_count),
        program_fingerprint: Some(program_fingerprint.to_string()),
        consistency_scope: Some(consistency_scope),
        query_support: symbolic.query_support,
        blocking_contradictions,
    }
}

fn map_symbolic_status(status: SymbolicClosureStatus) -> ReasoningSymbolicStatus {
    match status {
        SymbolicClosureStatus::Saturated => ReasoningSymbolicStatus::Saturated,
        SymbolicClosureStatus::Truncated => ReasoningSymbolicStatus::Truncated,
    }
}

fn has_blocking_contradictions(
    symbolic: &SymbolicEvaluation,
    consistency_scope: ReasoningConsistencyScope,
) -> bool {
    !filter_blocking_contradictions(
        &symbolic.contradictions,
        &symbolic.query_support,
        consistency_scope,
    )
    .is_empty()
}

fn filter_blocking_contradictions(
    contradictions: &[ReasoningContradiction],
    query_support: &[String],
    consistency_scope: ReasoningConsistencyScope,
) -> Vec<ReasoningContradiction> {
    match consistency_scope {
        ReasoningConsistencyScope::Global => contradictions.to_vec(),
        ReasoningConsistencyScope::QuerySupport => {
            let relevant_atoms: std::collections::BTreeSet<String> = query_support
                .iter()
                .map(|fact| contradiction_atom_key(fact))
                .collect();
            contradictions
                .iter()
                .filter(|contradiction| {
                    relevant_atoms.contains(&contradiction_atom_key(&contradiction.positive))
                })
                .cloned()
                .collect()
        }
    }
}

fn contradiction_atom_key(value: &str) -> String {
    value
        .trim()
        .strip_prefix("not ")
        .unwrap_or(value.trim())
        .to_string()
}

pub(crate) fn normalize_non_empty(
    value: &str,
    context: &str,
    field: &str,
) -> Result<String, HelixError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(HelixError::validation_error(
            context.to_string(),
            format!("{field} must be non-empty"),
        ));
    }
    Ok(normalized.to_string())
}

pub(crate) fn ensure_finite(value: f64, context: &str, field: &str) -> Result<(), HelixError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(HelixError::validation_error(
            context.to_string(),
            format!("{field} must be finite"),
        ))
    }
}

pub(crate) fn validate_probability(value: f64, context: &str) -> Result<f64, HelixError> {
    ensure_finite(value, context, "probability")?;
    if !(0.0..=1.0).contains(&value) {
        return Err(HelixError::validation_error(
            context.to_string(),
            "probability must be in [0, 1]".to_string(),
        ));
    }
    Ok(value)
}

pub(crate) fn resolve_max_rounds(max_rounds: Option<u8>) -> Result<usize, HelixError> {
    match max_rounds {
        Some(0) => Err(HelixError::validation_error(
            "reasoning.max_rounds",
            "max_rounds must be greater than zero",
        )),
        Some(value) => Ok(usize::from(value)),
        None => Ok(16),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbolic_backend_entails_query() {
        let decision = evaluate_reasoning(ReasoningEvaluationRequest::KrrSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec!["trusted(user)".to_string(), "kyc_passed(user)".to_string()],
            rules: vec![SymbolicRule {
                id: "r1".to_string(),
                antecedents: vec!["trusted(user)".to_string(), "kyc_passed(user)".to_string()],
                consequent: "allow(tx)".to_string(),
            }],
            triples: vec![],
            consistency_scope: None,
            max_rounds: Some(8),
        })
        .unwrap();

        assert_eq!(decision.backend, ReasoningBackend::KrrSymbolic);
        assert_eq!(decision.verdict, ReasoningVerdict::Allow);
        assert_eq!(decision.trace.symbolic_entailed, Some(true));
        assert_eq!(
            decision.trace.symbolic_status,
            Some(ReasoningSymbolicStatus::Saturated)
        );
        assert_eq!(decision.trace.pending_rule_count, Some(0));
        assert!(decision.trace.program_fingerprint.is_some());
        assert_eq!(
            decision.trace.consistency_scope,
            Some(ReasoningConsistencyScope::Global)
        );
        assert_eq!(decision.trace.support_graph.len(), 3);
        assert!(decision.trace.contradictions.is_empty());
        assert!(decision.trace.blocking_contradictions.is_empty());
        assert_eq!(
            decision.trace.query_support,
            vec![
                "allow(tx)".to_string(),
                "kyc_passed(user)".to_string(),
                "trusted(user)".to_string(),
            ]
        );
        assert!(decision
            .trace
            .support_graph
            .iter()
            .any(|node| node.fact == "allow(tx)" && node.rule_id.as_deref() == Some("r1")));
    }

    #[test]
    fn symbolic_backend_preserves_rule_order_semantics_across_rounds() {
        let decision = evaluate_reasoning(ReasoningEvaluationRequest::KrrSymbolic {
            query: "c".to_string(),
            facts: vec!["a".to_string()],
            rules: vec![
                SymbolicRule {
                    id: "r2".to_string(),
                    antecedents: vec!["b".to_string()],
                    consequent: "c".to_string(),
                },
                SymbolicRule {
                    id: "r1".to_string(),
                    antecedents: vec!["a".to_string()],
                    consequent: "b".to_string(),
                },
            ],
            triples: vec![],
            consistency_scope: None,
            max_rounds: Some(2),
        })
        .unwrap();

        assert_eq!(decision.verdict, ReasoningVerdict::Allow);
        assert_eq!(decision.trace.matched_rules, vec!["r1", "r2"]);
    }

    #[test]
    fn symbolic_backend_supports_unconditional_rules() {
        let decision = evaluate_reasoning(ReasoningEvaluationRequest::KrrSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec![],
            rules: vec![SymbolicRule {
                id: "bootstrap".to_string(),
                antecedents: vec![],
                consequent: "allow(tx)".to_string(),
            }],
            triples: vec![],
            consistency_scope: None,
            max_rounds: Some(1),
        })
        .unwrap();

        assert_eq!(decision.verdict, ReasoningVerdict::Allow);
    }

    #[test]
    fn symbolic_backend_canonicalizes_predicates_in_support_graph() {
        let decision = evaluate_reasoning(ReasoningEvaluationRequest::KrrSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec![
                "trusted( user )".to_string(),
                "kyc_passed(user)".to_string(),
            ],
            rules: vec![SymbolicRule {
                id: "r1".to_string(),
                antecedents: vec!["trusted(user)".to_string(), "kyc_passed(user)".to_string()],
                consequent: "allow(tx)".to_string(),
            }],
            triples: vec![],
            consistency_scope: None,
            max_rounds: Some(8),
        })
        .unwrap();

        assert!(decision
            .trace
            .support_graph
            .iter()
            .any(|node| node.fact == "trusted(user)" && node.kind == ReasoningSupportKind::Seed));
    }

    #[test]
    fn symbolic_program_fingerprint_is_stable() {
        let rules = vec![SymbolicRule {
            id: "r1".to_string(),
            antecedents: vec!["trusted(user)".to_string()],
            consequent: "allow(tx)".to_string(),
        }];
        let triples = vec![KrrTriple {
            subject: "user".to_string(),
            predicate: "owns".to_string(),
            object: "tx".to_string(),
        }];
        let a = fingerprint_symbolic_program(&rules, &triples).unwrap();
        let b = fingerprint_symbolic_program(&rules, &triples).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn symbolic_backend_denies_when_contradiction_is_present() {
        let decision = evaluate_reasoning(ReasoningEvaluationRequest::KrrSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec!["allow(tx)".to_string(), "!allow(tx)".to_string()],
            rules: vec![],
            triples: vec![],
            consistency_scope: None,
            max_rounds: Some(4),
        })
        .unwrap();

        assert_eq!(decision.verdict, ReasoningVerdict::Deny);
        assert_eq!(decision.trace.contradictions.len(), 1);
        assert_eq!(decision.trace.contradictions[0].positive, "allow(tx)");
        assert_eq!(decision.trace.contradictions[0].negative, "not allow(tx)");
    }

    #[test]
    fn symbolic_backend_reports_truncation_when_round_budget_is_exhausted() {
        let decision = evaluate_reasoning(ReasoningEvaluationRequest::KrrSymbolic {
            query: "c".to_string(),
            facts: vec!["a".to_string()],
            rules: vec![
                SymbolicRule {
                    id: "r1".to_string(),
                    antecedents: vec!["a".to_string()],
                    consequent: "b".to_string(),
                },
                SymbolicRule {
                    id: "r2".to_string(),
                    antecedents: vec!["b".to_string()],
                    consequent: "c".to_string(),
                },
            ],
            triples: vec![],
            consistency_scope: None,
            max_rounds: Some(1),
        })
        .unwrap();

        assert_eq!(decision.verdict, ReasoningVerdict::Deny);
        assert_eq!(
            decision.trace.symbolic_status,
            Some(ReasoningSymbolicStatus::Truncated)
        );
        assert_eq!(decision.trace.pending_rule_count, Some(1));
        assert_eq!(decision.trace.symbolic_entailed, Some(false));
        assert!(decision.rationale.contains("truncated"));
    }

    #[test]
    fn symbolic_query_support_excludes_irrelevant_derivations() {
        let decision = evaluate_reasoning(ReasoningEvaluationRequest::KrrSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec!["trusted(user)".to_string(), "news(noise)".to_string()],
            rules: vec![
                SymbolicRule {
                    id: "permit".to_string(),
                    antecedents: vec!["trusted(user)".to_string()],
                    consequent: "allow(tx)".to_string(),
                },
                SymbolicRule {
                    id: "noise".to_string(),
                    antecedents: vec!["news(noise)".to_string()],
                    consequent: "alert(side_channel)".to_string(),
                },
            ],
            triples: vec![],
            consistency_scope: None,
            max_rounds: Some(4),
        })
        .unwrap();

        assert_eq!(decision.verdict, ReasoningVerdict::Allow);
        assert!(decision
            .trace
            .derived_facts
            .contains(&"alert(side_channel)".to_string()));
        assert_eq!(
            decision.trace.query_support,
            vec!["allow(tx)".to_string(), "trusted(user)".to_string()]
        );
    }

    #[test]
    fn symbolic_query_support_scope_allows_unrelated_contradictions() {
        let decision = evaluate_reasoning(ReasoningEvaluationRequest::KrrSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec![
                "trusted(user)".to_string(),
                "noise".to_string(),
                "!noise".to_string(),
            ],
            rules: vec![SymbolicRule {
                id: "permit".to_string(),
                antecedents: vec!["trusted(user)".to_string()],
                consequent: "allow(tx)".to_string(),
            }],
            triples: vec![],
            consistency_scope: Some(ReasoningConsistencyScope::QuerySupport),
            max_rounds: Some(4),
        })
        .unwrap();

        assert_eq!(decision.verdict, ReasoningVerdict::Allow);
        assert_eq!(
            decision.trace.consistency_scope,
            Some(ReasoningConsistencyScope::QuerySupport)
        );
        assert_eq!(decision.trace.contradictions.len(), 1);
        assert!(decision.trace.blocking_contradictions.is_empty());
    }

    #[test]
    fn symbolic_query_support_scope_blocks_support_contradictions() {
        let decision = evaluate_reasoning(ReasoningEvaluationRequest::KrrSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec!["trusted(user)".to_string(), "!trusted(user)".to_string()],
            rules: vec![SymbolicRule {
                id: "permit".to_string(),
                antecedents: vec!["trusted(user)".to_string()],
                consequent: "allow(tx)".to_string(),
            }],
            triples: vec![],
            consistency_scope: Some(ReasoningConsistencyScope::QuerySupport),
            max_rounds: Some(4),
        })
        .unwrap();

        assert_eq!(decision.verdict, ReasoningVerdict::Deny);
        assert_eq!(decision.trace.contradictions.len(), 1);
        assert_eq!(decision.trace.blocking_contradictions.len(), 1);
        assert_eq!(
            decision.trace.blocking_contradictions[0].positive,
            "trusted(user)"
        );
    }

    #[test]
    fn expert_backend_tie_breaks_fail_closed() {
        let decision = evaluate_reasoning(ReasoningEvaluationRequest::ExpertSystem {
            features: BTreeMap::from([("risk".to_string(), 5)]),
            rules: vec![
                ExpertRule {
                    id: "allow_rule".to_string(),
                    min_features: BTreeMap::from([("risk".to_string(), 5)]),
                    verdict: ReasoningVerdict::Allow,
                    weight: 10,
                },
                ExpertRule {
                    id: "deny_rule".to_string(),
                    min_features: BTreeMap::from([("risk".to_string(), 5)]),
                    verdict: ReasoningVerdict::Deny,
                    weight: 10,
                },
            ],
        })
        .unwrap();

        assert_eq!(decision.verdict, ReasoningVerdict::Deny);
    }

    #[test]
    fn expert_backend_denies_when_no_rule_matches() {
        let decision = evaluate_reasoning(ReasoningEvaluationRequest::ExpertSystem {
            features: BTreeMap::from([("risk".to_string(), 1)]),
            rules: vec![ExpertRule {
                id: "allow_if_high".to_string(),
                min_features: BTreeMap::from([("risk".to_string(), 5)]),
                verdict: ReasoningVerdict::Allow,
                weight: 10,
            }],
        })
        .unwrap();

        assert_eq!(decision.verdict, ReasoningVerdict::Deny);
        assert_eq!(decision.confidence, 1.0);
    }

    #[test]
    fn expert_backend_supports_unconditional_rules() {
        let decision = evaluate_reasoning(ReasoningEvaluationRequest::ExpertSystem {
            features: BTreeMap::new(),
            rules: vec![ExpertRule {
                id: "default_review".to_string(),
                min_features: BTreeMap::new(),
                verdict: ReasoningVerdict::Review,
                weight: 3,
            }],
        })
        .unwrap();

        assert_eq!(decision.verdict, ReasoningVerdict::Review);
        assert_eq!(decision.trace.matched_rules, vec!["default_review"]);
    }

    #[test]
    fn expert_backend_rejects_blank_feature_names() {
        let result = evaluate_reasoning(ReasoningEvaluationRequest::ExpertSystem {
            features: BTreeMap::from([(" ".to_string(), 5)]),
            rules: vec![ExpertRule {
                id: "allow_rule".to_string(),
                min_features: BTreeMap::from([("risk".to_string(), 5)]),
                verdict: ReasoningVerdict::Allow,
                weight: 1,
            }],
        });
        assert!(matches!(result, Err(HelixError::ValidationError { .. })));
    }

    #[test]
    fn neuro_backend_is_deterministic() {
        let request = ReasoningEvaluationRequest::Neuro {
            features: BTreeMap::from([("f1".to_string(), 1.0), ("f2".to_string(), 0.2)]),
            model: LinearModel {
                bias: 0.1,
                weights: BTreeMap::from([("f1".to_string(), 0.8), ("f2".to_string(), -0.1)]),
                allow_threshold: 0.8,
                review_threshold: 0.5,
            },
        };

        let a = evaluate_reasoning(request.clone()).unwrap();
        let b = evaluate_reasoning(request).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn neuro_symbolic_denies_when_symbolic_gate_fails() {
        let decision = evaluate_reasoning(ReasoningEvaluationRequest::NeuroSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec!["trusted(user)".to_string()],
            rules: vec![],
            triples: vec![],
            features: BTreeMap::from([("f1".to_string(), 10.0)]),
            model: LinearModel {
                bias: 0.0,
                weights: BTreeMap::from([("f1".to_string(), 1.0)]),
                allow_threshold: 0.8,
                review_threshold: 0.5,
            },
            min_probability: Some(0.8),
            consistency_scope: None,
            max_rounds: Some(8),
        })
        .unwrap();

        assert_eq!(decision.verdict, ReasoningVerdict::Deny);
        assert_eq!(decision.trace.symbolic_entailed, Some(false));
        assert!(decision.trace.neural_probability.unwrap() > 0.9);
    }

    #[test]
    fn neuro_symbolic_denies_when_symbolic_contradiction_exists() {
        let decision = evaluate_reasoning(ReasoningEvaluationRequest::NeuroSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec!["allow(tx)".to_string(), "!allow(tx)".to_string()],
            rules: vec![],
            triples: vec![],
            features: BTreeMap::from([("f1".to_string(), 10.0)]),
            model: LinearModel {
                bias: 0.0,
                weights: BTreeMap::from([("f1".to_string(), 1.0)]),
                allow_threshold: 0.8,
                review_threshold: 0.5,
            },
            min_probability: Some(0.8),
            consistency_scope: None,
            max_rounds: Some(8),
        })
        .unwrap();

        assert_eq!(decision.verdict, ReasoningVerdict::Deny);
        assert_eq!(decision.trace.contradictions.len(), 1);
    }

    #[test]
    fn neuro_symbolic_denies_when_consistency_sweep_truncates() {
        let decision = evaluate_reasoning(ReasoningEvaluationRequest::NeuroSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec!["trusted(user)".to_string(), "seed(x0)".to_string()],
            rules: vec![
                SymbolicRule {
                    id: "permit".to_string(),
                    antecedents: vec!["trusted(user)".to_string()],
                    consequent: "allow(tx)".to_string(),
                },
                SymbolicRule {
                    id: "chain1".to_string(),
                    antecedents: vec!["seed(x0)".to_string()],
                    consequent: "seed(x1)".to_string(),
                },
                SymbolicRule {
                    id: "chain2".to_string(),
                    antecedents: vec!["seed(x1)".to_string()],
                    consequent: "seed(x2)".to_string(),
                },
            ],
            triples: vec![],
            features: BTreeMap::from([("f1".to_string(), 10.0)]),
            model: LinearModel {
                bias: 0.0,
                weights: BTreeMap::from([("f1".to_string(), 1.0)]),
                allow_threshold: 0.8,
                review_threshold: 0.5,
            },
            min_probability: Some(0.8),
            consistency_scope: None,
            max_rounds: Some(1),
        })
        .unwrap();

        assert_eq!(decision.verdict, ReasoningVerdict::Deny);
        assert!(decision.trace.neural_probability.unwrap() > 0.99);
        assert_eq!(
            decision.trace.symbolic_status,
            Some(ReasoningSymbolicStatus::Truncated)
        );
        assert!(decision
            .rationale
            .contains("consistency evaluation truncated"));
    }

    #[test]
    fn neuro_symbolic_query_support_scope_allows_unrelated_contradictions() {
        let decision = evaluate_reasoning(ReasoningEvaluationRequest::NeuroSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec![
                "trusted(user)".to_string(),
                "noise".to_string(),
                "!noise".to_string(),
            ],
            rules: vec![SymbolicRule {
                id: "permit".to_string(),
                antecedents: vec!["trusted(user)".to_string()],
                consequent: "allow(tx)".to_string(),
            }],
            triples: vec![],
            features: BTreeMap::from([("f1".to_string(), 10.0)]),
            model: LinearModel {
                bias: 0.0,
                weights: BTreeMap::from([("f1".to_string(), 1.0)]),
                allow_threshold: 0.8,
                review_threshold: 0.5,
            },
            min_probability: Some(0.8),
            consistency_scope: Some(ReasoningConsistencyScope::QuerySupport),
            max_rounds: Some(4),
        })
        .unwrap();

        assert_eq!(decision.verdict, ReasoningVerdict::Allow);
        assert_eq!(decision.trace.contradictions.len(), 1);
        assert!(decision.trace.blocking_contradictions.is_empty());
    }

    #[test]
    fn neuro_backend_boundary_thresholds_are_stable() {
        let features = BTreeMap::new();
        let model = LinearModel {
            bias: 0.0,
            weights: BTreeMap::new(),
            allow_threshold: 0.5,
            review_threshold: 0.5,
        };
        let decision = evaluate_reasoning(ReasoningEvaluationRequest::Neuro {
            features: features.clone(),
            model: model.clone(),
        })
        .unwrap();
        assert_eq!(decision.verdict, ReasoningVerdict::Allow);

        let review_model = LinearModel {
            allow_threshold: 0.8,
            review_threshold: 0.5,
            ..model
        };
        let review_decision = evaluate_reasoning(ReasoningEvaluationRequest::Neuro {
            features: features.clone(),
            model: review_model,
        })
        .unwrap();
        assert_eq!(review_decision.verdict, ReasoningVerdict::Review);

        let deny_decision = evaluate_reasoning(ReasoningEvaluationRequest::Neuro {
            features,
            model: LinearModel {
                bias: 0.0,
                weights: BTreeMap::new(),
                allow_threshold: 0.8,
                review_threshold: 0.6,
            },
        })
        .unwrap();
        assert_eq!(deny_decision.verdict, ReasoningVerdict::Deny);
    }

    #[test]
    fn symbolic_backend_rejects_empty_query_and_triple_fields() {
        let empty_query = evaluate_reasoning(ReasoningEvaluationRequest::KrrSymbolic {
            query: "   ".to_string(),
            facts: vec!["trusted(user)".to_string()],
            rules: vec![],
            triples: vec![],
            consistency_scope: None,
            max_rounds: Some(4),
        });
        assert!(matches!(
            empty_query,
            Err(HelixError::ValidationError { .. })
        ));

        let empty_triple = evaluate_reasoning(ReasoningEvaluationRequest::KrrSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec!["trusted(user)".to_string()],
            rules: vec![],
            triples: vec![KrrTriple {
                subject: "user".to_string(),
                predicate: " ".to_string(),
                object: "tx".to_string(),
            }],
            consistency_scope: None,
            max_rounds: Some(4),
        });
        assert!(matches!(
            empty_triple,
            Err(HelixError::ValidationError { .. })
        ));
    }

    #[test]
    fn neuro_symbolic_rejects_invalid_probability_inputs() {
        let nan_probability = evaluate_reasoning(ReasoningEvaluationRequest::NeuroSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec!["allow(tx)".to_string()],
            rules: vec![],
            triples: vec![],
            features: BTreeMap::from([("f1".to_string(), f64::NAN)]),
            model: LinearModel {
                bias: 0.0,
                weights: BTreeMap::from([("f1".to_string(), 1.0)]),
                allow_threshold: 0.8,
                review_threshold: 0.5,
            },
            min_probability: Some(0.8),
            consistency_scope: None,
            max_rounds: Some(4),
        });
        assert!(matches!(
            nan_probability,
            Err(HelixError::ValidationError { .. })
        ));

        let nan_min_probability = evaluate_reasoning(ReasoningEvaluationRequest::NeuroSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec!["allow(tx)".to_string()],
            rules: vec![],
            triples: vec![],
            features: BTreeMap::from([("f1".to_string(), 1.0)]),
            model: LinearModel {
                bias: 0.0,
                weights: BTreeMap::from([("f1".to_string(), 1.0)]),
                allow_threshold: 0.8,
                review_threshold: 0.5,
            },
            min_probability: Some(f64::NAN),
            consistency_scope: None,
            max_rounds: Some(4),
        });
        assert!(matches!(
            nan_min_probability,
            Err(HelixError::ValidationError { .. })
        ));
    }

    #[test]
    fn neuro_symbolic_rejects_out_of_range_min_probability() {
        let above_one = evaluate_reasoning(ReasoningEvaluationRequest::NeuroSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec!["allow(tx)".to_string()],
            rules: vec![],
            triples: vec![],
            features: BTreeMap::from([("f1".to_string(), 1.0)]),
            model: LinearModel {
                bias: 0.0,
                weights: BTreeMap::from([("f1".to_string(), 1.0)]),
                allow_threshold: 0.8,
                review_threshold: 0.5,
            },
            min_probability: Some(1.1),
            consistency_scope: None,
            max_rounds: Some(4),
        });
        assert!(matches!(above_one, Err(HelixError::ValidationError { .. })));

        let below_zero = evaluate_reasoning(ReasoningEvaluationRequest::NeuroSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec!["allow(tx)".to_string()],
            rules: vec![],
            triples: vec![],
            features: BTreeMap::from([("f1".to_string(), 1.0)]),
            model: LinearModel {
                bias: 0.0,
                weights: BTreeMap::from([("f1".to_string(), 1.0)]),
                allow_threshold: 0.8,
                review_threshold: 0.5,
            },
            min_probability: Some(-0.1),
            consistency_scope: None,
            max_rounds: Some(4),
        });
        assert!(matches!(
            below_zero,
            Err(HelixError::ValidationError { .. })
        ));
    }

    #[test]
    fn neuro_backend_rejects_invalid_thresholds() {
        let invalid_order = evaluate_reasoning(ReasoningEvaluationRequest::Neuro {
            features: BTreeMap::new(),
            model: LinearModel {
                bias: 0.0,
                weights: BTreeMap::new(),
                allow_threshold: 0.4,
                review_threshold: 0.5,
            },
        });
        assert!(matches!(
            invalid_order,
            Err(HelixError::ValidationError { .. })
        ));

        let out_of_range = evaluate_reasoning(ReasoningEvaluationRequest::Neuro {
            features: BTreeMap::new(),
            model: LinearModel {
                bias: 0.0,
                weights: BTreeMap::new(),
                allow_threshold: 1.1,
                review_threshold: 0.5,
            },
        });
        assert!(matches!(
            out_of_range,
            Err(HelixError::ValidationError { .. })
        ));
    }

    #[test]
    fn symbolic_backends_reject_zero_max_rounds() {
        let symbolic = evaluate_reasoning(ReasoningEvaluationRequest::KrrSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec!["allow(tx)".to_string()],
            rules: vec![],
            triples: vec![],
            consistency_scope: None,
            max_rounds: Some(0),
        });
        assert!(matches!(symbolic, Err(HelixError::ValidationError { .. })));

        let neuro_symbolic = evaluate_reasoning(ReasoningEvaluationRequest::NeuroSymbolic {
            query: "allow(tx)".to_string(),
            facts: vec!["allow(tx)".to_string()],
            rules: vec![],
            triples: vec![],
            features: BTreeMap::from([("f1".to_string(), 1.0)]),
            model: LinearModel {
                bias: 0.0,
                weights: BTreeMap::from([("f1".to_string(), 1.0)]),
                allow_threshold: 0.8,
                review_threshold: 0.5,
            },
            min_probability: Some(0.8),
            consistency_scope: None,
            max_rounds: Some(0),
        });
        assert!(matches!(
            neuro_symbolic,
            Err(HelixError::ValidationError { .. })
        ));
    }
}
