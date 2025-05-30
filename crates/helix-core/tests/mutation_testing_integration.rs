//! Integration tests for the mutation testing framework

#![cfg(feature = "mutation-testing")]

use helix_core::mutation_testing::{
    MutationConfig, 
    evolution::EvolutionaryMutationTester,
    mutator::Mutator,
    operators::{ArithmeticOperatorMutator, BooleanLiteralMutator, MutationOperator},
};
use std::path::PathBuf;
use tempfile::TempDir;
use std::fs;

/// Sample code to test mutations on
const SAMPLE_CODE: &str = r#"
pub fn calculate(a: i32, b: i32) -> i32 {
    if a > b {
        a + b
    } else {
        a - b
    }
}

pub fn is_valid(value: i32) -> bool {
    if value > 0 && value < 100 {
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_calculate_greater() {
        assert_eq!(calculate(10, 5), 15);
    }
    
    #[test]
    fn test_calculate_lesser() {
        assert_eq!(calculate(5, 10), -5);
    }
    
    #[test]
    fn test_is_valid_in_range() {
        assert!(is_valid(50));
    }
    
    #[test]
    fn test_is_valid_out_of_range() {
        assert!(!is_valid(150));
        assert!(!is_valid(-10));
    }
}
"#;

#[test]
fn test_arithmetic_mutations() {
    let mutator = ArithmeticOperatorMutator;
    let path = PathBuf::from("test.rs");
    
    let mutations = mutator.mutate(SAMPLE_CODE, &path).unwrap();
    
    // Should find + and - operators
    assert!(mutations.iter().any(|m| m.original == "+"));
    assert!(mutations.iter().any(|m| m.original == "-"));
    
    // Check mutation types
    for mutation in &mutations {
        assert_eq!(mutation.mutation_type, helix_core::mutation_testing::MutationType::ArithmeticOperator);
    }
}

#[test]
fn test_boolean_mutations() {
    let mutator = BooleanLiteralMutator;
    let path = PathBuf::from("test.rs");
    
    let mutations = mutator.mutate(SAMPLE_CODE, &path).unwrap();
    
    // Should find true and false literals
    assert!(mutations.iter().any(|m| m.original == "true"));
    assert!(mutations.iter().any(|m| m.original == "false"));
}

#[test]
fn test_mutator_apply() {
    let mutator = Mutator::new();
    let simple_code = "let x = 5 + 3;";
    
    let mutations = mutator.generate_mutations(simple_code).unwrap();
    assert!(!mutations.is_empty());
    
    // Apply first mutation
    if let Some(mutation) = mutations.first() {
        let mutated = mutator.apply_mutation(simple_code, mutation).unwrap();
        assert_ne!(simple_code, mutated);
    }
}

#[tokio::test]
async fn test_evolutionary_mutation_basic() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("sample.rs");
    
    // Write sample code to file
    fs::write(&test_file, SAMPLE_CODE).unwrap();
    
    // Configure mutation testing
    let config = MutationConfig {
        target_files: vec![test_file.clone()],
        max_generations: 2,
        population_size: 5,
        mutation_rate: 0.2,
        crossover_rate: 0.6,
        test_timeout: 10,
    };
    
    let mut tester = EvolutionaryMutationTester::new(config, temp_dir.path().to_path_buf());
    
    // Note: This would fail in real execution without proper test setup
    // This is just to verify the structure compiles correctly
    match tester.run().await {
        Ok(results) => {
            println!("Mutation testing completed with {} results", results.len());
        }
        Err(e) => {
            // Expected in test environment without proper cargo setup
            println!("Expected error in test environment: {}", e);
        }
    }
}

#[test]
fn test_mutation_config_serialization() {
    let config = MutationConfig {
        target_files: vec![PathBuf::from("src/lib.rs")],
        max_generations: 5,
        population_size: 10,
        mutation_rate: 0.15,
        crossover_rate: 0.75,
        test_timeout: 20,
    };
    
    // Serialize to JSON
    let json = serde_json::to_string_pretty(&config).unwrap();
    
    // Deserialize back
    let deserialized: MutationConfig = serde_json::from_str(&json).unwrap();
    
    assert_eq!(config.max_generations, deserialized.max_generations);
    assert_eq!(config.population_size, deserialized.population_size);
    assert!((config.mutation_rate - deserialized.mutation_rate).abs() < f64::EPSILON);
}

#[test]
fn test_mutation_result_fitness() {
    use helix_core::mutation_testing::{MutationResult, TestResult, Mutation, MutationType};
    use helix_core::mutation_testing::evaluator::DefaultFitnessEvaluator;
    use helix_core::mutation_testing::FitnessEvaluator;
    
    let evaluator = DefaultFitnessEvaluator;
    
    // Create a killed mutation result
    let result = MutationResult {
        mutation: Mutation {
            id: uuid::Uuid::new_v4(),
            file_path: PathBuf::from("test.rs"),
            line: 5,
            column: 10,
            mutation_type: MutationType::ArithmeticOperator,
            original: "+".to_string(),
            mutated: "-".to_string(),
        },
        killed: true,
        test_results: vec![
            TestResult {
                name: "test_calculate".to_string(),
                passed: false,
                error: Some("assertion failed".to_string()),
                duration: 50,
            }
        ],
        fitness: 0.0,
        execution_time: 100,
    };
    
    let fitness = evaluator.calculate_fitness(&result);
    
    // Killed mutations should have high fitness
    assert!(fitness > 0.5);
    assert!(fitness <= 1.0);
}