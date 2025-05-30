//! Example demonstrating evolutionary mutation testing
//! 
//! Run with: cargo run --example mutation_testing_demo --features mutation-testing

#![cfg(feature = "mutation-testing")]

use helix_core::mutation_testing::{
    MutationConfig,
    evolution::EvolutionaryMutationTester,
    mutator::{Mutator, MutationFilter},
    MutationStrategy,
};
use std::path::PathBuf;
use std::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Helix Evolutionary Mutation Testing Demo");
    println!("========================================\n");
    
    // Create a sample file to test
    let sample_code = r#"
/// Calculator module for demonstration
pub mod calculator {
    /// Add two numbers
    pub fn add(a: i32, b: i32) -> i32 {
        a + b
    }
    
    /// Subtract two numbers
    pub fn subtract(a: i32, b: i32) -> i32 {
        a - b
    }
    
    /// Check if a number is positive
    pub fn is_positive(n: i32) -> bool {
        n > 0
    }
    
    /// Calculate factorial
    pub fn factorial(n: u32) -> u32 {
        if n <= 1 {
            1
        } else {
            n * factorial(n - 1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::calculator::*;
    
    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
        assert_eq!(add(-1, 1), 0);
    }
    
    #[test]
    fn test_subtract() {
        assert_eq!(subtract(5, 3), 2);
        assert_eq!(subtract(0, 5), -5);
    }
    
    #[test]
    fn test_is_positive() {
        assert!(is_positive(5));
        assert!(!is_positive(-5));
        assert!(!is_positive(0));
    }
    
    #[test]
    fn test_factorial() {
        assert_eq!(factorial(0), 1);
        assert_eq!(factorial(1), 1);
        assert_eq!(factorial(5), 120);
    }
}
"#;
    
    // Save to temporary file
    let test_file = PathBuf::from("demo_calculator.rs");
    fs::write(&test_file, sample_code)?;
    
    println!("Step 1: Generating mutations");
    println!("----------------------------");
    
    // Generate mutations
    let mutator = Mutator::new();
    let mutations = mutator.generate_mutations(sample_code)?;
    
    println!("Found {} possible mutations", mutations.len());
    
    // Apply filtering
    let filter = MutationFilter;
    let filtered = filter.filter_equivalent(mutations);
    let prioritized = filter.prioritize(filtered);
    
    println!("After filtering: {} mutations", prioritized.len());
    
    // Show some example mutations
    println!("\nExample mutations:");
    for (i, mutation) in prioritized.iter().take(5).enumerate() {
        println!("  {}. Line {}: {} -> {} ({})", 
            i + 1,
            mutation.line,
            mutation.original,
            mutation.mutated,
            format!("{:?}", mutation.mutation_type)
        );
    }
    
    println!("\nStep 2: Evolutionary Testing Configuration");
    println!("------------------------------------------");
    
    // Configure evolutionary testing
    let config = MutationConfig {
        target_files: vec![test_file.clone()],
        max_generations: 3,
        population_size: 10,
        mutation_rate: 0.15,
        crossover_rate: 0.70,
        test_timeout: 30,
    };
    
    println!("Configuration:");
    println!("  Generations: {}", config.max_generations);
    println!("  Population size: {}", config.population_size);
    println!("  Mutation rate: {:.0}%", config.mutation_rate * 100.0);
    println!("  Crossover rate: {:.0}%", config.crossover_rate * 100.0);
    
    println!("\nStep 3: Running Evolutionary Algorithm");
    println!("--------------------------------------");
    
    // Note: In a real scenario, this would run actual tests
    // For demo purposes, we'll just show the structure
    let work_dir = std::env::current_dir()?;
    let mut tester = EvolutionaryMutationTester::new(config, work_dir);
    
    println!("Evolutionary mutation testing would:");
    println!("  1. Create initial population of mutation combinations");
    println!("  2. Evaluate each individual by running tests");
    println!("  3. Select fittest individuals (mutations that are killed)");
    println!("  4. Apply crossover and mutation to create new generation");
    println!("  5. Repeat for {} generations", 3);
    
    println!("\nMutation Testing Benefits:");
    println!("-------------------------");
    println!("✓ Identifies weak test coverage");
    println!("✓ Finds equivalent mutations");
    println!("✓ Improves test suite quality");
    println!("✓ Evolutionary approach finds optimal mutation sets");
    
    // Clean up
    fs::remove_file(&test_file)?;
    
    println!("\nDemo completed successfully!");
    
    Ok(())
}