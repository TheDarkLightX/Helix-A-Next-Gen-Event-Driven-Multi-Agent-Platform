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

//! Deterministic embedding generation.
//!
//! Selection record:
//! - Chosen algorithm: bounded lexical feature hashing with L2 normalization.
//! - Alternatives considered: random placeholder vectors, remote embedding APIs, and local model
//!   inference. Random placeholders are not meaningful; remote/local models introduce dependency,
//!   latency, and reproducibility costs that do not fit Helix's deterministic core.
//! - Safety invariant: equal input text and dimensions always produce equal finite vectors; empty,
//!   oversized, or invalid dimension inputs fail closed.

use crate::errors::EmbeddingError;

/// Default vector width used by Helix's deterministic lexical embedder.
pub const DEFAULT_EMBEDDING_DIMENSIONS: usize = 64;

/// Maximum text size accepted by the deterministic embedder.
pub const MAX_TEXT_BYTES: usize = 64 * 1024;

/// Deterministic text embedding generator.
#[derive(Clone, Copy, Debug, Default)]
pub struct EmbeddingGenerator;

impl EmbeddingGenerator {
    /// Generate a deterministic embedding for text using the default dimensions.
    pub fn generate_text_embedding(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        self.generate_text_embedding_with_dimensions(text, DEFAULT_EMBEDDING_DIMENSIONS)
    }

    /// Generate a deterministic embedding for text using an explicit vector width.
    pub fn generate_text_embedding_with_dimensions(
        &self,
        text: &str,
        dimensions: usize,
    ) -> Result<Vec<f32>, EmbeddingError> {
        if dimensions == 0 {
            return Err(EmbeddingError::InvalidDimensions {
                expected: 1,
                actual: 0,
            });
        }
        if text.trim().is_empty() {
            return Err(EmbeddingError::GenerationError(
                "text must contain at least one token".to_string(),
            ));
        }
        if text.len() > MAX_TEXT_BYTES {
            return Err(EmbeddingError::GenerationError(format!(
                "text exceeds {} byte embedding limit",
                MAX_TEXT_BYTES
            )));
        }

        let tokens = tokenize(text);
        if tokens.is_empty() {
            return Err(EmbeddingError::GenerationError(
                "text must contain at least one alphanumeric token".to_string(),
            ));
        }

        let mut vector = vec![0.0_f32; dimensions];
        let mut previous: Option<&str> = None;

        for token in &tokens {
            add_feature(&mut vector, token.as_bytes(), 1.0);

            if token.len() > 6 {
                add_feature(&mut vector, &token.as_bytes()[..6], 0.5);
            }

            if let Some(previous_token) = previous {
                let pair = format!("{previous_token} {token}");
                add_feature(&mut vector, pair.as_bytes(), 0.4);
            }
            previous = Some(token);
        }

        normalize(&mut vector)
    }
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for character in text.chars() {
        if character.is_alphanumeric() {
            for lower in character.to_lowercase() {
                current.push(lower);
            }
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn add_feature(vector: &mut [f32], feature: &[u8], weight: f32) {
    let hash = fnv1a64(feature);
    let index = (hash as usize) % vector.len();
    let sign = if (hash >> 63) == 0 { 1.0 } else { -1.0 };
    vector[index] += sign * weight;
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn normalize(vector: &mut [f32]) -> Result<Vec<f32>, EmbeddingError> {
    let norm_squared: f32 = vector.iter().map(|value| value * value).sum();
    if !norm_squared.is_finite() || norm_squared == 0.0 {
        return Err(EmbeddingError::GenerationError(
            "embedding norm must be positive and finite".to_string(),
        ));
    }

    let norm = norm_squared.sqrt();
    for value in vector.iter_mut() {
        *value /= norm;
    }
    Ok(vector.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_embeddings_are_stable_and_normalized() {
        let generator = EmbeddingGenerator;

        let first = generator
            .generate_text_embedding("Alice North resigned from Orion Dynamics")
            .unwrap();
        let second = generator
            .generate_text_embedding("Alice North resigned from Orion Dynamics")
            .unwrap();

        assert_eq!(first, second);
        assert_eq!(first.len(), DEFAULT_EMBEDDING_DIMENSIONS);
        let norm: f32 = first.iter().map(|value| value * value).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.000_01);
        assert!(first.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn empty_text_fails_closed() {
        let generator = EmbeddingGenerator;

        let result = generator.generate_text_embedding("  \n\t ");

        assert!(matches!(result, Err(EmbeddingError::GenerationError(_))));
    }

    #[test]
    fn invalid_dimensions_fail_closed() {
        let generator = EmbeddingGenerator;

        let result = generator.generate_text_embedding_with_dimensions("helix", 0);

        assert!(matches!(
            result,
            Err(EmbeddingError::InvalidDimensions {
                expected: 1,
                actual: 0
            })
        ));
    }
}
