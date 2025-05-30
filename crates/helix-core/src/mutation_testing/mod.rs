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


//! # Helix Mutation Testing Framework
//!
//! A comprehensive evolutionary mutation testing system for continuous code quality improvement.
//!
//! ## Overview
//!
//! This module provides a complete mutation testing framework that uses evolutionary algorithms
//! to continuously improve test quality and code coverage. The system is designed to identify
//! weak spots in test suites and guide developers toward more robust testing practices.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                 Mutation Testing Framework                  │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
//! │  │   Quality   │  │ Evolutionary│  │    TES Calculator   │  │
//! │  │ Assessment  │  │  Algorithm  │  │                     │  │
//! │  └─────────────┘  └─────────────┘  └─────────────────────┘  │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
//! │  │  Mutation   │  │   Fitness   │  │     Reporting       │  │
//! │  │  Generator  │  │ Evaluation  │  │                     │  │
//! │  └─────────────┘  └─────────────┘  └─────────────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Core Components
//!
//! ### 1. Quality Assessment (`quality_assessment.rs`)
//! - **Purpose**: Comprehensive codebase quality analysis
//! - **Features**: Test coverage, complexity analysis, documentation coverage
//! - **Output**: Detailed quality metrics and TES scores
//!
//! ### 2. Evolutionary Algorithm (`evolution.rs`)
//! - **Purpose**: Genetic algorithm for test optimization
//! - **Features**: Population-based evolution, crossover, mutation, selection
//! - **Output**: Evolved test suites with improved effectiveness
//!
//! ### 3. TES Calculator (`test_effectiveness.rs`)
//! - **Purpose**: Test Effectiveness Score calculation
//! - **Features**: Multi-component scoring with weighted metrics
//! - **Output**: A-F grading system for test quality
//!
//! ### 4. Mutation Generator (`mutator.rs`, `operators.rs`)
//! - **Purpose**: Code mutation generation for testing
//! - **Features**: Various mutation operators (arithmetic, logical, boundary)
//! - **Output**: Mutated code variants for test validation
//!
//! ### 5. Fitness Evaluation (`evaluator.rs`)
//! - **Purpose**: Test suite fitness assessment
//! - **Features**: Mutation kill rate, coverage analysis, edge case detection
//! - **Output**: Fitness scores for evolutionary selection
//!
//! ### 6. Practical Analysis (`practical_analyzer.rs`)
//! - **Purpose**: Real-world mutation testing analysis
//! - **Features**: Comprehensive quality reporting and insights
//! - **Output**: Actionable recommendations for improvement
//!
//! ## Key Features
//!
//! ### ✅ Pros
//!
//! - **Evolutionary Optimization**: Continuously improves test quality over time
//! - **Comprehensive Metrics**: Multi-dimensional quality assessment
//! - **Automated Analysis**: Minimal manual intervention required
//! - **Actionable Insights**: Clear guidance on improvement areas
//! - **Industry Standards**: Based on established mutation testing principles
//! - **Scalable Architecture**: Modular design for easy extension
//! - **Real-time Feedback**: Immediate quality assessment
//! - **Security Focus**: Identifies security-critical code paths
//!
//! ### ⚠️ Limitations
//!
//! - **Computational Cost**: Mutation testing can be resource-intensive
//! - **False Positives**: Some mutations may not represent real bugs
//! - **Test Dependency**: Quality depends on existing test foundation
//! - **Language Specific**: Currently optimized for Rust codebases
//! - **Learning Curve**: Requires understanding of mutation testing concepts
//! - **Setup Complexity**: Initial configuration may be involved

// Core mutation testing modules
pub mod mutator;
pub mod evaluator;
pub mod evolution;
pub mod operators;
pub mod test_effectiveness;
pub mod practical_analyzer;
pub mod demo;
pub mod evolutionary_demo;

// Quality assessment and reporting
pub mod quality_assessment;
pub mod reporting;

use crate::HelixError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during mutation testing
#[derive(Error, Debug)]
pub enum MutationError {
    #[error("IO error: {0}")]
    IoError(String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Test execution error: {0}")]
    TestExecutionError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

impl From<MutationError> for HelixError {
    fn from(err: MutationError) -> Self {
        HelixError::InternalError(err.to_string())
    }
}

impl From<HelixError> for MutationError {
    fn from(err: HelixError) -> Self {
        MutationError::ConfigError(err.to_string())
    }
}

/// Configuration for mutation testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationConfig {
    /// Target source files to mutate
    pub target_files: Vec<PathBuf>,
    /// Maximum number of generations
    pub max_generations: usize,
    /// Population size per generation
    pub population_size: usize,
    /// Mutation rate (0.0 to 1.0)
    pub mutation_rate: f64,
    /// Crossover rate (0.0 to 1.0)
    pub crossover_rate: f64,
    /// Test timeout in seconds
    pub test_timeout: u64,
}

impl Default for MutationConfig {
    fn default() -> Self {
        Self {
            target_files: vec![],
            max_generations: 10,
            population_size: 20,
            mutation_rate: 0.1,
            crossover_rate: 0.7,
            test_timeout: 30,
        }
    }
}

/// Represents a mutation in the code
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Mutation {
    /// Unique identifier for this mutation
    pub id: uuid::Uuid,
    /// File path where mutation is applied
    pub file_path: PathBuf,
    /// Line number of the mutation
    pub line: usize,
    /// Column position
    pub column: usize,
    /// Type of mutation applied
    pub mutation_type: MutationType,
    /// Original code snippet
    pub original: String,
    /// Mutated code snippet
    pub mutated: String,
}

/// Types of mutations that can be applied
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MutationType {
    /// Replace arithmetic operators (+, -, *, /, %)
    ArithmeticOperator,
    /// Replace comparison operators (==, !=, <, >, <=, >=)
    ComparisonOperator,
    /// Replace logical operators (&&, ||, !)
    LogicalOperator,
    /// Replace boolean literals (true <-> false)
    BooleanLiteral,
    /// Modify numeric constants
    NumericConstant,
    /// Remove or modify conditional statements
    ConditionalStatement,
    /// Modify return values
    ReturnValue,
    /// Remove function calls
    FunctionCall,
}

/// Result of evaluating a mutation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationResult {
    /// The mutation that was tested
    pub mutation: Mutation,
    /// Whether the mutation was killed by tests
    pub killed: bool,
    /// Test results
    pub test_results: Vec<TestResult>,
    /// Fitness score (0.0 to 1.0)
    pub fitness: f64,
    /// Execution time in milliseconds
    pub execution_time: u64,
}

/// Individual test result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Test name
    pub name: String,
    /// Whether the test passed
    pub passed: bool,
    /// Error message if test failed
    pub error: Option<String>,
    /// Execution time in milliseconds
    pub duration: u64,
}

/// Trait for mutation strategies
pub trait MutationStrategy: Send + Sync {
    /// Generate mutations for given code
    fn generate_mutations(&self, code: &str) -> Result<Vec<Mutation>, HelixError>;
    
    /// Apply a mutation to code
    fn apply_mutation(&self, code: &str, mutation: &Mutation) -> Result<String, HelixError>;
}

/// Trait for fitness evaluation
pub trait FitnessEvaluator: Send + Sync {
    /// Calculate fitness score for a mutation result
    fn calculate_fitness(&self, result: &MutationResult) -> f64;
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mutation_config_default() {
        let config = MutationConfig::default();
        assert_eq!(config.max_generations, 10);
        assert_eq!(config.population_size, 20);
        assert!((config.mutation_rate - 0.1).abs() < f64::EPSILON);
    }
    
    #[test]
    fn test_mutation_creation() {
        let mutation = Mutation {
            id: uuid::Uuid::new_v4(),
            file_path: PathBuf::from("test.rs"),
            line: 10,
            column: 5,
            mutation_type: MutationType::ArithmeticOperator,
            original: "a + b".to_string(),
            mutated: "a - b".to_string(),
        };
        
        assert_eq!(mutation.line, 10);
        assert_eq!(mutation.mutation_type, MutationType::ArithmeticOperator);
    }

    #[test]
    fn test_framework_version() {
        assert_eq!(VERSION, "1.0.0");
    }

    #[test]
    fn test_quick_assessment() {
        let assessment = quick_assessment();
        assert!(assessment.contains("TES Score"));
        assert!(assessment.contains("Grade"));
        assert!(assessment.contains("Total Tests"));
    }

    #[test]
    fn test_config_constants() {
        assert_eq!(config::DEFAULT_POPULATION_SIZE, 20);
        assert_eq!(config::DEFAULT_GENERATIONS, 10);
        assert_eq!(config::DEFAULT_CROSSOVER_RATE, 0.8);
        assert_eq!(config::DEFAULT_MUTATION_RATE, 0.1);
        assert_eq!(config::DEFAULT_TES_TARGET, 80.0);
        assert_eq!(config::DEFAULT_MUTATION_TIMEOUT, 30);
    }
}

// Re-export commonly used types for convenience
pub use quality_assessment::{QualityMetrics, TESScore, TESComponents, analyze_helix_core_quality};
pub use reporting::{generate_quality_report, QualityReporter, QualityReport};

/// Framework version for compatibility tracking
pub const VERSION: &str = "1.0.0";

/// Default configuration values
pub mod config {
    /// Default population size for evolutionary algorithm
    pub const DEFAULT_POPULATION_SIZE: usize = 20;

    /// Default number of generations to evolve
    pub const DEFAULT_GENERATIONS: usize = 10;

    /// Default crossover rate for genetic algorithm
    pub const DEFAULT_CROSSOVER_RATE: f64 = 0.8;

    /// Default mutation rate for genetic algorithm
    pub const DEFAULT_MUTATION_RATE: f64 = 0.1;

    /// Default target TES score for completion
    pub const DEFAULT_TES_TARGET: f64 = 80.0;

    /// Default timeout for individual mutation tests (seconds)
    pub const DEFAULT_MUTATION_TIMEOUT: u64 = 30;
}

/// Quick start function for basic quality assessment
pub fn quick_assessment() -> String {
    let metrics = quality_assessment::analyze_helix_core_quality();
    let tes = metrics.calculate_tes();

    format!(
        "Quick Quality Assessment:\n\
         TES Score: {:.1}% (Grade: {})\n\
         Total Tests: {}\n\
         Test Coverage: {:.1}%\n\
         Mutation Score: {:.1}%",
        tes.score,
        tes.grade,
        metrics.total_tests,
        metrics.test_coverage,
        metrics.mutation_score
    )
}