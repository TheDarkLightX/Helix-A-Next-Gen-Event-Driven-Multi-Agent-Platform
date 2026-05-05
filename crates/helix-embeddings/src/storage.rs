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

//! Deterministic in-memory vector storage.

use std::collections::BTreeMap;

use crate::{errors::EmbeddingError, search, search::SearchResult};

/// In-memory vector store with deterministic iteration order.
#[derive(Clone, Debug, Default)]
pub struct VectorStore {
    vectors: BTreeMap<String, Vec<f32>>,
    dimensions: Option<usize>,
}

impl VectorStore {
    /// Create an empty vector store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the number of stored vectors.
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    /// Return true when the store contains no vectors.
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }

    /// Return the vector width accepted by this store once initialized.
    pub fn dimensions(&self) -> Option<usize> {
        self.dimensions
    }

    /// Store a vector, panicking on invalid input for legacy callers.
    ///
    /// New code should prefer [`VectorStore::try_store`] so invalid vectors can be handled
    /// explicitly at the API boundary.
    pub fn store(&mut self, id: &str, vector: &[f32]) {
        self.try_store(id, vector)
            .expect("invalid vector passed to VectorStore::store");
    }

    /// Store a vector with explicit fail-closed validation.
    pub fn try_store(&mut self, id: &str, vector: &[f32]) -> Result<(), EmbeddingError> {
        let id = id.trim();
        if id.is_empty() {
            return Err(EmbeddingError::StorageError(
                "vector id must not be empty".to_string(),
            ));
        }
        validate_vector(vector, self.dimensions)?;
        self.dimensions = Some(vector.len());
        self.vectors.insert(id.to_string(), vector.to_vec());
        Ok(())
    }

    /// Retrieve a vector by ID.
    pub fn retrieve(&self, id: &str) -> Option<Vec<f32>> {
        self.vectors.get(id).cloned()
    }

    /// Remove a vector by ID.
    pub fn remove(&mut self, id: &str) -> Option<Vec<f32>> {
        let removed = self.vectors.remove(id);
        if self.vectors.is_empty() {
            self.dimensions = None;
        }
        removed
    }

    /// Iterate over stored vectors in deterministic identifier order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<f32>)> {
        self.vectors.iter()
    }

    /// Search stored vectors by cosine similarity.
    pub fn search(&self, query: &[f32], limit: usize) -> Result<Vec<SearchResult>, EmbeddingError> {
        search::rank_by_cosine(query, self.iter(), limit)
    }
}

fn validate_vector(
    vector: &[f32],
    expected_dimensions: Option<usize>,
) -> Result<(), EmbeddingError> {
    if vector.is_empty() {
        return Err(EmbeddingError::InvalidDimensions {
            expected: 1,
            actual: 0,
        });
    }
    if let Some(expected) = expected_dimensions {
        if vector.len() != expected {
            return Err(EmbeddingError::InvalidDimensions {
                expected,
                actual: vector.len(),
            });
        }
    }
    if !vector.iter().all(|value| value.is_finite()) {
        return Err(EmbeddingError::StorageError(
            "vector values must be finite".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_store_round_trips_known_vectors() {
        let mut store = VectorStore::new();

        store.try_store("a", &[1.0, 0.0, 0.0]).unwrap();
        store.try_store("b", &[0.0, 1.0, 0.0]).unwrap();

        assert_eq!(store.len(), 2);
        assert_eq!(store.dimensions(), Some(3));
        assert_eq!(store.retrieve("a"), Some(vec![1.0, 0.0, 0.0]));
        assert_eq!(store.retrieve("missing"), None);
    }

    #[test]
    fn vector_store_rejects_dimension_changes() {
        let mut store = VectorStore::new();
        store.try_store("a", &[1.0, 0.0]).unwrap();

        let result = store.try_store("b", &[1.0, 0.0, 0.0]);

        assert!(matches!(
            result,
            Err(EmbeddingError::InvalidDimensions {
                expected: 2,
                actual: 3
            })
        ));
    }

    #[test]
    fn vector_store_rejects_non_finite_values() {
        let mut store = VectorStore::new();

        let result = store.try_store("nan", &[f32::NAN]);

        assert!(matches!(result, Err(EmbeddingError::StorageError(_))));
    }
}
