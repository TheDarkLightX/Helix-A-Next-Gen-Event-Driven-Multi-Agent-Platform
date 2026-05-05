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

//! Vector and semantic similarity search.

use crate::{embeddings::EmbeddingGenerator, errors::EmbeddingError, storage::VectorStore};

/// A ranked vector search result.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    /// Caller-supplied document or vector identifier.
    pub id: String,
    /// Cosine similarity score in descending ranking order.
    pub score: f32,
}

/// Search engine over an owned in-memory vector store.
#[derive(Clone, Debug, Default)]
pub struct SearchEngine {
    store: VectorStore,
}

impl SearchEngine {
    /// Create a search engine over an existing store.
    pub fn new(store: VectorStore) -> Self {
        Self { store }
    }

    /// Borrow the underlying vector store.
    pub fn store(&self) -> &VectorStore {
        &self.store
    }

    /// Mutably borrow the underlying vector store.
    pub fn store_mut(&mut self) -> &mut VectorStore {
        &mut self.store
    }

    /// Insert or replace a vector in the owned store.
    pub fn insert_vector(&mut self, id: &str, vector: &[f32]) -> Result<(), EmbeddingError> {
        self.store.try_store(id, vector)
    }

    /// Search for similar vectors.
    ///
    /// This compatibility method fails closed by returning an empty list when the query is invalid.
    /// Use [`SearchEngine::try_search`] when callers need explicit error handling.
    pub fn search(&self, query: &[f32], limit: usize) -> Vec<(String, f32)> {
        self.try_search(query, limit)
            .unwrap_or_default()
            .into_iter()
            .map(|result| (result.id, result.score))
            .collect()
    }

    /// Search for similar vectors with explicit validation errors.
    pub fn try_search(
        &self,
        query: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchResult>, EmbeddingError> {
        rank_by_cosine(query, self.store.iter(), limit)
    }
}

/// Deterministic text-to-vector semantic index.
#[derive(Clone, Debug, Default)]
pub struct SemanticIndex {
    generator: EmbeddingGenerator,
    search: SearchEngine,
}

impl SemanticIndex {
    /// Create an empty semantic index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a document by embedding and storing its text.
    pub fn add_document(&mut self, id: &str, text: &str) -> Result<(), EmbeddingError> {
        let vector = self.generator.generate_text_embedding(text)?;
        self.search.insert_vector(id, &vector)
    }

    /// Retrieve documents most similar to a text query.
    pub fn query(&self, text: &str, limit: usize) -> Result<Vec<SearchResult>, EmbeddingError> {
        let vector = self.generator.generate_text_embedding(text)?;
        self.search.try_search(&vector, limit)
    }

    /// Borrow the vector search engine backing this semantic index.
    pub fn search_engine(&self) -> &SearchEngine {
        &self.search
    }
}

/// Compute cosine similarity for two finite vectors with equal dimensions.
pub fn cosine_similarity(left: &[f32], right: &[f32]) -> Result<f32, EmbeddingError> {
    if left.len() != right.len() {
        return Err(EmbeddingError::InvalidDimensions {
            expected: left.len(),
            actual: right.len(),
        });
    }
    if left.is_empty() {
        return Err(EmbeddingError::InvalidDimensions {
            expected: 1,
            actual: 0,
        });
    }
    if !left
        .iter()
        .chain(right.iter())
        .all(|value| value.is_finite())
    {
        return Err(EmbeddingError::SearchError(
            "vectors must contain only finite values".to_string(),
        ));
    }

    let dot: f32 = left
        .iter()
        .zip(right.iter())
        .map(|(lhs, rhs)| lhs * rhs)
        .sum();
    let left_norm: f32 = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm: f32 = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        return Err(EmbeddingError::SearchError(
            "vectors must have positive norm".to_string(),
        ));
    }

    Ok(dot / (left_norm * right_norm))
}

/// Rank candidate vectors by cosine similarity, breaking score ties by identifier.
pub fn rank_by_cosine<'a, I>(
    query: &[f32],
    candidates: I,
    limit: usize,
) -> Result<Vec<SearchResult>, EmbeddingError>
where
    I: IntoIterator<Item = (&'a String, &'a Vec<f32>)>,
{
    if limit == 0 {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    for (id, vector) in candidates {
        results.push(SearchResult {
            id: id.clone(),
            score: cosine_similarity(query, vector)?,
        });
    }

    results.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.id.cmp(&right.id))
    });
    results.truncate(limit);
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similarity_rejects_dimension_mismatch() {
        let result = cosine_similarity(&[1.0, 0.0], &[1.0]);

        assert!(matches!(
            result,
            Err(EmbeddingError::InvalidDimensions {
                expected: 2,
                actual: 1
            })
        ));
    }

    #[test]
    fn search_engine_ranks_by_score_then_id() {
        let mut engine = SearchEngine::default();
        engine.insert_vector("b", &[1.0, 0.0]).unwrap();
        engine.insert_vector("a", &[1.0, 0.0]).unwrap();
        engine.insert_vector("c", &[0.0, 1.0]).unwrap();

        let results = engine.try_search(&[1.0, 0.0], 2).unwrap();

        assert_eq!(results[0].id, "a");
        assert_eq!(results[1].id, "b");
        assert_eq!(results[0].score, 1.0);
    }

    #[test]
    fn semantic_index_returns_relevant_document_first() {
        let mut index = SemanticIndex::new();
        index
            .add_document(
                "orion",
                "Alice North resigned from Orion Dynamics after a security review.",
            )
            .unwrap();
        index
            .add_document(
                "macro",
                "Copper futures moved lower after global inventory reports.",
            )
            .unwrap();

        let results = index
            .query("Orion Dynamics leadership resignation", 2)
            .unwrap();

        assert_eq!(results[0].id, "orion");
        assert!(results[0].score > results[1].score);
    }
}
