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

#![warn(missing_docs)]

//! Vector embeddings and similarity search for Helix.
//!
//! This crate provides:
//! - Text and data embedding generation
//! - Vector similarity search
//! - Embedding storage and retrieval
//! - Semantic search capabilities

pub mod embeddings;
pub mod errors;
pub mod search;
pub mod storage;

pub use embeddings::{EmbeddingGenerator, DEFAULT_EMBEDDING_DIMENSIONS, MAX_TEXT_BYTES};
pub use errors::EmbeddingError;
pub use search::{cosine_similarity, SearchEngine, SearchResult, SemanticIndex};
pub use storage::VectorStore;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_exposes_deterministic_semantic_index() {
        let mut index = SemanticIndex::new();

        index
            .add_document(
                "case",
                "Orion Dynamics leadership changed after Alice North resigned.",
            )
            .unwrap();
        let results = index.query("Alice North Orion leadership", 1).unwrap();

        assert_eq!(results[0].id, "case");
    }
}
