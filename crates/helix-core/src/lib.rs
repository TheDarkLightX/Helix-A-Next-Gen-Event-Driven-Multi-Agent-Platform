// Copyright 2024 Helix Platform
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

#![deny(unsafe_code)]
#![allow(missing_docs)] // Documentation is incomplete; re-enable once ready

//! Core Helix types, traits, and utilities shared across the platform.

// Core modules
pub mod agent;
/// Defines the Credential struct for secure storage.
pub mod credential;
pub mod errors;
pub mod event;
pub mod policy;
pub mod profile;
pub mod recipe;
pub mod state;
pub mod types;

/// Evolutionary mutation testing framework
#[cfg(feature = "mutation-testing")]
pub mod mutation_testing;

/// Quality analysis using mutation testing
#[cfg(feature = "mutation-testing")]
pub mod quality_analysis;

/// Test utilities for TES-driven testing
pub mod test_utils;

// Temporary function to calculate cyclomatic complexity
#[cfg(feature = "mutation-testing")]
pub fn calculate_codebase_complexity() -> f64 {
    use crate::mutation_testing::quality_assessment::analyze_helix_core_quality;
    let metrics = analyze_helix_core_quality();
    metrics.complexity_score
}
// Quality assessment moved to mutation_testing module
// pub mod quality_assessment; // Deprecated - use mutation_testing::quality_assessment

pub use errors::HelixError;

/// Placeholder function to demonstrate module linkage.
pub fn hello_helix() -> String {
    "Hello from Helix Core!".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(hello_helix(), "Hello from Helix Core!");
    }
}
