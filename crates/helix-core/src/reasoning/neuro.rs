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

use super::{ensure_finite, validate_probability, LinearModel, ReasoningVerdict};
use crate::HelixError;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NeuroEvaluation {
    pub(crate) verdict: ReasoningVerdict,
    pub(crate) confidence: f64,
    pub(crate) probability: f64,
}

pub(crate) fn evaluate_neuro(
    features: BTreeMap<String, f64>,
    model: LinearModel,
) -> Result<NeuroEvaluation, HelixError> {
    let probability = run_linear_model(&features, &model)?;
    let verdict = ml_verdict(probability, &model)?;
    Ok(NeuroEvaluation {
        verdict,
        confidence: score_neuro_probability(probability, verdict),
        probability,
    })
}

pub(crate) fn score_neuro_probability(probability: f64, verdict: ReasoningVerdict) -> f64 {
    match verdict {
        ReasoningVerdict::Allow => probability,
        ReasoningVerdict::Review => (0.5 + (probability - 0.5).abs()).clamp(0.0, 1.0),
        ReasoningVerdict::Deny => (1.0 - probability).clamp(0.0, 1.0),
    }
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

    Ok(if probability >= model.allow_threshold {
        ReasoningVerdict::Allow
    } else if probability >= model.review_threshold {
        ReasoningVerdict::Review
    } else {
        ReasoningVerdict::Deny
    })
}
