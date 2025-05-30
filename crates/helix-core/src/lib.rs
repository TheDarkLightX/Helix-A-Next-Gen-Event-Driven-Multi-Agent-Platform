#![deny(unsafe_code)]
#![warn(missing_docs)] // TODO: Enforce this later

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
