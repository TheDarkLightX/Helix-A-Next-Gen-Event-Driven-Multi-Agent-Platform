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
//! This module provides a formal functional core for four reasoning modes:
//! - `krr_symbolic`: finite forward-chaining over facts/rules/triples
//! - `expert_system`: deterministic weighted rule voting
//! - `neuro`: deterministic linear model inference
//! - `neuro_symbolic`: fail-closed symbolic gate + neural confidence fusion

use crate::HelixError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

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
        /// Maximum symbolic closure rounds.
        max_rounds: Option<u8>,
    },
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
            max_rounds,
        } => {
            let query = normalize_non_empty(&query, "reasoning.query", "query")?;
            let max_rounds = resolve_max_rounds(max_rounds)?;
            let (closure, matched_rules) =
                infer_symbolic_closure(facts, rules, triples, max_rounds)?;
            let entailed = closure.contains(query.as_str());
            let verdict = if entailed {
                ReasoningVerdict::Allow
            } else {
                ReasoningVerdict::Deny
            };
            Ok(ReasoningDecision {
                backend: ReasoningBackend::KrrSymbolic,
                verdict,
                confidence: if entailed { 1.0 } else { 0.0 },
                rationale: if entailed {
                    format!("symbolic query entailed: {query}")
                } else {
                    format!("symbolic query not entailed: {query}")
                },
                trace: ReasoningTrace {
                    derived_facts: closure.into_iter().collect(),
                    matched_rules,
                    symbolic_entailed: Some(entailed),
                    neural_probability: None,
                },
            })
        }
        ReasoningEvaluationRequest::ExpertSystem { features, rules } => {
            let mut allow_score = 0_u32;
            let mut review_score = 0_u32;
            let mut deny_score = 0_u32;
            let mut matched_rules = Vec::new();

            for rule in &rules {
                let rule_id = normalize_non_empty(&rule.id, "reasoning.rules", "rule id")?;
                let matches = rule.min_features.iter().all(|(feature, min)| {
                    if feature.trim().is_empty() {
                        return false;
                    }
                    features.get(feature).copied().unwrap_or(i64::MIN) >= *min
                });
                if matches {
                    matched_rules.push(rule_id);
                    match rule.verdict {
                        ReasoningVerdict::Allow => {
                            allow_score = allow_score.saturating_add(u32::from(rule.weight))
                        }
                        ReasoningVerdict::Review => {
                            review_score = review_score.saturating_add(u32::from(rule.weight))
                        }
                        ReasoningVerdict::Deny => {
                            deny_score = deny_score.saturating_add(u32::from(rule.weight))
                        }
                    }
                }
            }

            if matched_rules.is_empty() {
                return Ok(ReasoningDecision {
                    backend: ReasoningBackend::ExpertSystem,
                    verdict: ReasoningVerdict::Deny,
                    confidence: 1.0,
                    rationale: "no expert rules matched; fail-closed deny".to_string(),
                    trace: ReasoningTrace {
                        derived_facts: Vec::new(),
                        matched_rules,
                        symbolic_entailed: None,
                        neural_probability: None,
                    },
                });
            }

            let total = allow_score
                .saturating_add(review_score)
                .saturating_add(deny_score)
                .max(1);

            let verdict = if deny_score >= review_score && deny_score >= allow_score {
                ReasoningVerdict::Deny
            } else if review_score >= allow_score {
                ReasoningVerdict::Review
            } else {
                ReasoningVerdict::Allow
            };

            let selected_score = match verdict {
                ReasoningVerdict::Allow => allow_score,
                ReasoningVerdict::Review => review_score,
                ReasoningVerdict::Deny => deny_score,
            };

            Ok(ReasoningDecision {
                backend: ReasoningBackend::ExpertSystem,
                verdict,
                confidence: (selected_score as f64 / total as f64).clamp(0.0, 1.0),
                rationale: "expert-system weighted rule vote".to_string(),
                trace: ReasoningTrace {
                    derived_facts: Vec::new(),
                    matched_rules,
                    symbolic_entailed: None,
                    neural_probability: None,
                },
            })
        }
        ReasoningEvaluationRequest::Neuro { features, model } => {
            let probability = run_linear_model(&features, &model)?;
            let verdict = ml_verdict(probability, &model)?;
            Ok(ReasoningDecision {
                backend: ReasoningBackend::Neuro,
                verdict,
                confidence: confidence_from_probability(probability, verdict),
                rationale: "deterministic linear-model inference".to_string(),
                trace: ReasoningTrace {
                    derived_facts: Vec::new(),
                    matched_rules: Vec::new(),
                    symbolic_entailed: None,
                    neural_probability: Some(probability),
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
            max_rounds,
        } => {
            let query = normalize_non_empty(&query, "reasoning.query", "query")?;
            let max_rounds = resolve_max_rounds(max_rounds)?;
            let min_probability =
                validate_probability(min_probability.unwrap_or(0.8), "reasoning.min_probability")?;
            let (closure, matched_rules) =
                infer_symbolic_closure(facts, rules, triples, max_rounds)?;
            let entailed = closure.contains(query.as_str());
            let probability = run_linear_model(&features, &model)?;

            let (verdict, rationale, confidence) = if !entailed {
                (
                    ReasoningVerdict::Deny,
                    "symbolic gate failed; denying regardless of neural score".to_string(),
                    1.0,
                )
            } else if probability >= min_probability {
                (
                    ReasoningVerdict::Allow,
                    "symbolic gate passed and neural confidence exceeded threshold".to_string(),
                    confidence_from_probability(probability, ReasoningVerdict::Allow),
                )
            } else {
                (
                    ReasoningVerdict::Review,
                    "symbolic gate passed but neural confidence below allow threshold".to_string(),
                    confidence_from_probability(probability, ReasoningVerdict::Review),
                )
            };

            Ok(ReasoningDecision {
                backend: ReasoningBackend::NeuroSymbolic,
                verdict,
                confidence,
                rationale,
                trace: ReasoningTrace {
                    derived_facts: closure.into_iter().collect(),
                    matched_rules,
                    symbolic_entailed: Some(entailed),
                    neural_probability: Some(probability),
                },
            })
        }
    }
}

fn infer_symbolic_closure(
    facts: Vec<String>,
    rules: Vec<SymbolicRule>,
    triples: Vec<KrrTriple>,
    max_rounds: usize,
) -> Result<(BTreeSet<String>, Vec<String>), HelixError> {
    if max_rounds == 0 {
        return Err(HelixError::validation_error(
            "reasoning.max_rounds",
            "max_rounds must be greater than zero",
        ));
    }

    let mut closure: BTreeSet<String> = BTreeSet::new();
    for fact in facts {
        closure.insert(normalize_non_empty(
            &fact,
            "reasoning.facts",
            "fact entries",
        )?);
    }

    for triple in triples {
        let predicate =
            normalize_non_empty(&triple.predicate, "reasoning.triples", "triple predicate")?;
        let subject = normalize_non_empty(&triple.subject, "reasoning.triples", "triple subject")?;
        let object = normalize_non_empty(&triple.object, "reasoning.triples", "triple object")?;
        closure.insert(format!("{}({},{})", predicate, subject, object));
    }

    let normalized_rules: Vec<(String, Vec<String>, String)> = rules
        .into_iter()
        .map(|rule| {
            let id = normalize_non_empty(&rule.id, "reasoning.rules", "rule id")?;
            let consequent =
                normalize_non_empty(&rule.consequent, "reasoning.rules", "rule consequent")?;
            let antecedents = rule
                .antecedents
                .into_iter()
                .map(|ant| normalize_non_empty(&ant, "reasoning.rules", "rule antecedent"))
                .collect::<Result<Vec<_>, _>>()?;
            Ok::<_, HelixError>((id, antecedents, consequent))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut matched_rules = Vec::new();
    for _ in 0..max_rounds {
        let mut changed = false;
        for (rule_id, antecedents, consequent) in &normalized_rules {
            if antecedents.iter().all(|ant| closure.contains(ant))
                && !closure.contains(consequent.as_str())
            {
                closure.insert(consequent.clone());
                matched_rules.push(rule_id.clone());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    Ok((closure, matched_rules))
}

fn run_linear_model(
    features: &BTreeMap<String, f64>,
    model: &LinearModel,
) -> Result<f64, HelixError> {
    ensure_finite(model.bias, "reasoning.model", "model bias")?;
    let review_threshold =
        validate_probability(model.review_threshold, "reasoning.model.review_threshold")?;
    let allow_threshold =
        validate_probability(model.allow_threshold, "reasoning.model.allow_threshold")?;

    if review_threshold > allow_threshold {
        return Err(HelixError::validation_error(
            "reasoning.model",
            "review_threshold must be <= allow_threshold",
        ));
    }

    for (feature, value) in features {
        if feature.trim().is_empty() {
            return Err(HelixError::validation_error(
                "reasoning.features",
                "feature names must be non-empty",
            ));
        }
        ensure_finite(*value, "reasoning.features", "feature value")?;
    }

    let mut score = model.bias;
    for (feature, weight) in &model.weights {
        if feature.trim().is_empty() {
            return Err(HelixError::validation_error(
                "reasoning.model",
                "model feature names must be non-empty",
            ));
        }
        ensure_finite(*weight, "reasoning.model", "model weight")?;
        let value = features.get(feature).ok_or_else(|| {
            HelixError::validation_error(
                "reasoning.features".to_string(),
                format!("missing required model feature: {feature}"),
            )
        })?;
        score += weight * value;
    }

    let probability = 1.0 / (1.0 + (-score).exp());
    validate_probability(probability, "reasoning.probability")
}

fn ml_verdict(probability: f64, model: &LinearModel) -> Result<ReasoningVerdict, HelixError> {
    if !(0.0..=1.0).contains(&probability) {
        return Err(HelixError::validation_error(
            "reasoning.probability",
            "model probability must be in [0, 1]",
        ));
    }

    let verdict = if probability >= model.allow_threshold {
        ReasoningVerdict::Allow
    } else if probability >= model.review_threshold {
        ReasoningVerdict::Review
    } else {
        ReasoningVerdict::Deny
    };

    Ok(verdict)
}

fn confidence_from_probability(probability: f64, verdict: ReasoningVerdict) -> f64 {
    match verdict {
        ReasoningVerdict::Allow => probability,
        ReasoningVerdict::Review => (0.5 + (probability - 0.5).abs()).clamp(0.0, 1.0),
        ReasoningVerdict::Deny => (1.0 - probability).clamp(0.0, 1.0),
    }
}

fn normalize_non_empty(value: &str, context: &str, field: &str) -> Result<String, HelixError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(HelixError::validation_error(
            context.to_string(),
            format!("{field} must be non-empty"),
        ));
    }
    Ok(normalized.to_string())
}

fn ensure_finite(value: f64, context: &str, field: &str) -> Result<(), HelixError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(HelixError::validation_error(
            context.to_string(),
            format!("{field} must be finite"),
        ))
    }
}

fn validate_probability(value: f64, context: &str) -> Result<f64, HelixError> {
    ensure_finite(value, context, "probability")?;
    if !(0.0..=1.0).contains(&value) {
        return Err(HelixError::validation_error(
            context.to_string(),
            "probability must be in [0, 1]".to_string(),
        ));
    }
    Ok(value)
}

fn resolve_max_rounds(max_rounds: Option<u8>) -> Result<usize, HelixError> {
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
            max_rounds: Some(8),
        })
        .unwrap();

        assert_eq!(decision.backend, ReasoningBackend::KrrSymbolic);
        assert_eq!(decision.verdict, ReasoningVerdict::Allow);
        assert_eq!(decision.trace.symbolic_entailed, Some(true));
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
            max_rounds: Some(8),
        })
        .unwrap();

        assert_eq!(decision.verdict, ReasoningVerdict::Deny);
        assert_eq!(decision.trace.symbolic_entailed, Some(false));
        assert!(decision.trace.neural_probability.unwrap() > 0.9);
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
            max_rounds: Some(0),
        });
        assert!(matches!(
            neuro_symbolic,
            Err(HelixError::ValidationError { .. })
        ));
    }
}
