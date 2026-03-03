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
            let max_rounds = usize::from(max_rounds.unwrap_or(16).max(1));
            let (closure, matched_rules) =
                infer_symbolic_closure(facts, rules, triples, max_rounds)?;
            let entailed = closure.contains(&query);
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
                let matches = rule.min_features.iter().all(|(feature, min)| {
                    features.get(feature).copied().unwrap_or(i64::MIN) >= *min
                });
                if matches {
                    matched_rules.push(rule.id.clone());
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
            let max_rounds = usize::from(max_rounds.unwrap_or(16).max(1));
            let min_probability = min_probability.unwrap_or(0.8).clamp(0.0, 1.0);
            let (closure, matched_rules) =
                infer_symbolic_closure(facts, rules, triples, max_rounds)?;
            let entailed = closure.contains(&query);
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
        if fact.trim().is_empty() {
            return Err(HelixError::validation_error(
                "reasoning.facts",
                "fact entries must be non-empty",
            ));
        }
        closure.insert(fact);
    }

    for triple in triples {
        closure.insert(format!(
            "{}({},{})",
            triple.predicate.trim(),
            triple.subject.trim(),
            triple.object.trim()
        ));
    }

    let mut matched_rules = Vec::new();
    for _ in 0..max_rounds {
        let mut changed = false;
        for rule in &rules {
            if rule.consequent.trim().is_empty() {
                return Err(HelixError::validation_error(
                    "reasoning.rules",
                    "rule consequent must be non-empty",
                ));
            }

            if rule
                .antecedents
                .iter()
                .all(|ant| closure.contains(ant.as_str()))
                && !closure.contains(rule.consequent.as_str())
            {
                closure.insert(rule.consequent.clone());
                matched_rules.push(rule.id.clone());
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
    if model.review_threshold > model.allow_threshold {
        return Err(HelixError::validation_error(
            "reasoning.model",
            "review_threshold must be <= allow_threshold",
        ));
    }

    if !(0.0..=1.0).contains(&model.review_threshold)
        || !(0.0..=1.0).contains(&model.allow_threshold)
    {
        return Err(HelixError::validation_error(
            "reasoning.model",
            "thresholds must be in [0, 1]",
        ));
    }

    let mut score = model.bias;
    for (feature, weight) in &model.weights {
        let value = features.get(feature).ok_or_else(|| {
            HelixError::validation_error("reasoning.features", "missing required model feature")
        })?;
        score += weight * value;
    }

    let probability = 1.0 / (1.0 + (-score).exp());
    Ok(probability.clamp(0.0, 1.0))
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
}
