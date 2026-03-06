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

use super::{normalize_non_empty, ExpertRule, ReasoningVerdict};
use crate::HelixError;
use std::cmp::Ordering;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
struct CompiledExpertRule {
    id: String,
    verdict: ReasoningVerdict,
    weight: u32,
    required_constraints: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ThresholdEntry {
    min_value: i64,
    rule_index: usize,
}

#[derive(Debug, Clone)]
struct CompiledExpertProgram {
    rules: Vec<CompiledExpertRule>,
    feature_index: BTreeMap<String, Vec<ThresholdEntry>>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExpertEvaluation {
    pub(crate) verdict: ReasoningVerdict,
    pub(crate) confidence: f64,
    pub(crate) rationale: String,
    pub(crate) matched_rules: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default)]
struct VerdictScores {
    allow: u32,
    review: u32,
    deny: u32,
}

pub(crate) fn evaluate_expert_system(
    features: BTreeMap<String, i64>,
    rules: Vec<ExpertRule>,
) -> Result<ExpertEvaluation, HelixError> {
    let program = CompiledExpertProgram::compile(rules)?;
    program.evaluate(features)
}

impl CompiledExpertProgram {
    fn compile(rules: Vec<ExpertRule>) -> Result<Self, HelixError> {
        let mut compiled_rules = Vec::with_capacity(rules.len());
        let mut feature_index: BTreeMap<String, Vec<ThresholdEntry>> = BTreeMap::new();

        for rule in rules {
            let id = normalize_non_empty(&rule.id, "reasoning.rules", "rule id")?;
            let rule_index = compiled_rules.len();
            for (feature, min_value) in &rule.min_features {
                let normalized_feature =
                    normalize_non_empty(feature, "reasoning.rules", "expert rule feature name")?;
                feature_index
                    .entry(normalized_feature)
                    .or_default()
                    .push(ThresholdEntry {
                        min_value: *min_value,
                        rule_index,
                    });
            }
            compiled_rules.push(CompiledExpertRule {
                id,
                verdict: rule.verdict,
                weight: u32::from(rule.weight),
                required_constraints: rule.min_features.len(),
            });
        }

        for entries in feature_index.values_mut() {
            entries.sort();
        }

        Ok(Self {
            rules: compiled_rules,
            feature_index,
        })
    }

    fn evaluate(&self, features: BTreeMap<String, i64>) -> Result<ExpertEvaluation, HelixError> {
        let mut satisfied_constraints = vec![0usize; self.rules.len()];
        for (feature, value) in &features {
            let normalized_feature =
                normalize_non_empty(feature, "reasoning.features", "feature names")?;
            if let Some(entries) = self.feature_index.get(normalized_feature.as_str()) {
                let upper_bound = entries.partition_point(|entry| entry.min_value <= *value);
                for entry in &entries[..upper_bound] {
                    satisfied_constraints[entry.rule_index] += 1;
                }
            }
        }

        let mut scores = VerdictScores::default();
        let mut matched_rules = Vec::new();
        for (rule_index, rule) in self.rules.iter().enumerate() {
            let matched = rule.required_constraints == 0
                || satisfied_constraints[rule_index] == rule.required_constraints;
            if !matched {
                continue;
            }

            matched_rules.push(rule.id.clone());
            match rule.verdict {
                ReasoningVerdict::Allow => scores.allow = scores.allow.saturating_add(rule.weight),
                ReasoningVerdict::Review => {
                    scores.review = scores.review.saturating_add(rule.weight)
                }
                ReasoningVerdict::Deny => scores.deny = scores.deny.saturating_add(rule.weight),
            }
        }

        if matched_rules.is_empty() {
            return Ok(ExpertEvaluation {
                verdict: ReasoningVerdict::Deny,
                confidence: 1.0,
                rationale: "no expert rules matched; fail-closed deny".to_string(),
                matched_rules,
            });
        }

        let verdict = select_verdict(scores);
        let selected_score = match verdict {
            ReasoningVerdict::Allow => scores.allow,
            ReasoningVerdict::Review => scores.review,
            ReasoningVerdict::Deny => scores.deny,
        };
        let total = scores
            .allow
            .saturating_add(scores.review)
            .saturating_add(scores.deny)
            .max(1);

        Ok(ExpertEvaluation {
            verdict,
            confidence: (selected_score as f64 / total as f64).clamp(0.0, 1.0),
            rationale: format!(
                "expert-system compiled vote selected {:?} (allow={}, review={}, deny={})",
                verdict, scores.allow, scores.review, scores.deny
            )
            .to_lowercase(),
            matched_rules,
        })
    }
}

fn select_verdict(scores: VerdictScores) -> ReasoningVerdict {
    let mut ranked = [
        (ReasoningVerdict::Allow, scores.allow),
        (ReasoningVerdict::Review, scores.review),
        (ReasoningVerdict::Deny, scores.deny),
    ];
    ranked.sort_by(compare_verdict_scores);
    ranked[0].0
}

fn compare_verdict_scores(
    left: &(ReasoningVerdict, u32),
    right: &(ReasoningVerdict, u32),
) -> Ordering {
    right
        .1
        .cmp(&left.1)
        .then_with(|| verdict_precedence(right.0).cmp(&verdict_precedence(left.0)))
}

fn verdict_precedence(verdict: ReasoningVerdict) -> u8 {
    match verdict {
        ReasoningVerdict::Deny => 3,
        ReasoningVerdict::Review => 2,
        ReasoningVerdict::Allow => 1,
    }
}
