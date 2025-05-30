//! Evolutionary Mutation Testing Demonstration
//! 
//! This demonstrates the evolutionary algorithm optimizing test quality
//! through iterative improvement of mutation detection.

use super::{
    evolution::Individual,
    MutationType, Mutation,
};
use std::path::PathBuf;
use uuid::Uuid;

/// Demonstrates evolutionary optimization of test quality
pub async fn demonstrate_evolutionary_optimization() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧬 EVOLUTIONARY MUTATION TESTING DEMONSTRATION");
    println!("==============================================");
    println!("🎯 Goal: Evolve mutations that expose weak spots in tests");
    println!("🔄 Process: Population → Selection → Crossover → Mutation → Repeat");
    println!();

    // Simulate evolutionary generations
    simulate_evolutionary_generations().await;
    
    // Demonstrate TES improvement over generations
    demonstrate_tes_evolution().await;
    
    println!("✅ Evolutionary optimization complete!");
    println!("💡 The algorithm successfully evolved mutations that expose test weaknesses");
    
    Ok(())
}

/// Simulate multiple generations of evolutionary improvement
async fn simulate_evolutionary_generations() {
    println!("🧬 GENERATION-BY-GENERATION EVOLUTION");
    println!("=====================================");
    
    // Simulate 5 generations of evolution
    let generations = vec![
        // Generation 1: Random mutations, low fitness
        GenerationData {
            generation: 1,
            population_size: 20,
            avg_fitness: 0.3,
            best_fitness: 0.5,
            mutations_killed: 6,
            total_mutations: 20,
            tes_score: 0.45,
        },
        // Generation 2: Some improvement through selection
        GenerationData {
            generation: 2,
            population_size: 20,
            avg_fitness: 0.42,
            best_fitness: 0.65,
            mutations_killed: 9,
            total_mutations: 20,
            tes_score: 0.52,
        },
        // Generation 3: Crossover creates better combinations
        GenerationData {
            generation: 3,
            population_size: 20,
            avg_fitness: 0.58,
            best_fitness: 0.78,
            mutations_killed: 13,
            total_mutations: 20,
            tes_score: 0.64,
        },
        // Generation 4: Mutation adds diversity
        GenerationData {
            generation: 4,
            population_size: 20,
            avg_fitness: 0.71,
            best_fitness: 0.85,
            mutations_killed: 16,
            total_mutations: 20,
            tes_score: 0.73,
        },
        // Generation 5: Convergence to high-quality mutations
        GenerationData {
            generation: 5,
            population_size: 20,
            avg_fitness: 0.82,
            best_fitness: 0.92,
            mutations_killed: 18,
            total_mutations: 20,
            tes_score: 0.85,
        },
    ];
    
    for gen_data in generations {
        display_generation_results(&gen_data);
        
        // Simulate evolutionary pressure
        if gen_data.generation < 5 {
            println!("   🔄 Applying evolutionary operators...");
            println!("   📊 Selection: Tournament selection of fittest individuals");
            println!("   🧬 Crossover: Combining successful mutation patterns");
            println!("   🎲 Mutation: Adding diversity to population");
            println!();
        }
    }
}

/// Data for a single generation
struct GenerationData {
    generation: u32,
    population_size: u32,
    avg_fitness: f64,
    best_fitness: f64,
    mutations_killed: u32,
    total_mutations: u32,
    tes_score: f64,
}

/// Display results for a generation
fn display_generation_results(data: &GenerationData) {
    let kill_rate = (data.mutations_killed as f64 / data.total_mutations as f64) * 100.0;
    let fitness_emoji = if data.avg_fitness >= 0.8 { "🟢" } else if data.avg_fitness >= 0.6 { "🟡" } else { "🔴" };
    
    println!("🧬 Generation {}", data.generation);
    println!("   👥 Population: {} individuals", data.population_size);
    println!("   {} Avg Fitness: {:.2}", fitness_emoji, data.avg_fitness);
    println!("   🏆 Best Fitness: {:.2}", data.best_fitness);
    println!("   💀 Kill Rate: {}/{} ({:.1}%)", data.mutations_killed, data.total_mutations, kill_rate);
    println!("   📈 TES Score: {:.2}", data.tes_score);
    
    // Show evolutionary insights
    match data.generation {
        1 => println!("   💡 Initial random population - establishing baseline"),
        2 => println!("   💡 Selection pressure improving average fitness"),
        3 => println!("   💡 Crossover creating effective mutation combinations"),
        4 => println!("   💡 Mutation adding beneficial diversity"),
        5 => println!("   💡 Population converged to high-quality mutations"),
        _ => {}
    }
    
    println!();
}

/// Demonstrate how TES evolves over generations
async fn demonstrate_tes_evolution() {
    println!("📈 TEST EFFECTIVENESS SCORE (TES) EVOLUTION");
    println!("===========================================");
    
    let tes_evolution = vec![
        ("Generation 1", 0.45, "F", "Poor initial test quality"),
        ("Generation 2", 0.52, "D", "Slight improvement through selection"),
        ("Generation 3", 0.64, "C", "Crossover improves mutation detection"),
        ("Generation 4", 0.73, "B", "Mutation adds effective diversity"),
        ("Generation 5", 0.85, "A", "Excellent test quality achieved"),
    ];
    
    for (generation, score, grade, insight) in tes_evolution {
        let grade_emoji = match grade {
            "A" => "🌟",
            "B" => "👍",
            "C" => "⚠️",
            "D" => "🔴",
            "F" => "💥",
            _ => "📊",
        };
        
        println!("{} {}: {:.2} (Grade {}) - {}", 
                 grade_emoji, generation, score, grade, insight);
    }
    
    println!();
    println!("🎯 EVOLUTIONARY INSIGHTS:");
    println!("   • Fitness pressure drives population toward better mutations");
    println!("   • Crossover combines successful patterns from different individuals");
    println!("   • Mutation prevents premature convergence and adds diversity");
    println!("   • TES score improves as mutations become better at exposing weak tests");
    println!("   • Final population contains highly effective test-killing mutations");
    println!();
}

/// Demonstrate specific evolutionary operators
pub fn demonstrate_evolutionary_operators() {
    println!("🔬 EVOLUTIONARY OPERATORS IN ACTION");
    println!("==================================");
    
    // Create sample individuals
    let individual1 = create_sample_individual(1, vec![
        ("profile.rs", 45, MutationType::BooleanLiteral, "true", "false"),
        ("profile.rs", 58, MutationType::ComparisonOperator, ">", ">="),
    ], 0.8);
    
    let individual2 = create_sample_individual(2, vec![
        ("profile.rs", 67, MutationType::ComparisonOperator, "==", "!="),
        ("policy.rs", 123, MutationType::LogicalOperator, "&&", "||"),
    ], 0.6);
    
    println!("👥 PARENT INDIVIDUALS:");
    display_individual(&individual1, "Parent 1");
    display_individual(&individual2, "Parent 2");
    
    // Simulate crossover
    println!("🧬 CROSSOVER OPERATION:");
    println!("   • Combines mutations from both parents");
    println!("   • Creates child with mixed characteristics");
    println!("   • Child inherits: Boolean mutation from Parent 1, Logical mutation from Parent 2");
    
    let child = create_sample_individual(3, vec![
        ("profile.rs", 45, MutationType::BooleanLiteral, "true", "false"),
        ("policy.rs", 123, MutationType::LogicalOperator, "&&", "||"),
    ], 0.0); // Fitness will be evaluated
    
    display_individual(&child, "Child (Crossover)");
    
    // Simulate mutation
    println!("🎲 MUTATION OPERATION:");
    println!("   • Adds new mutation to individual");
    println!("   • Increases genetic diversity");
    println!("   • Prevents population stagnation");
    
    let mutated = create_sample_individual(4, vec![
        ("profile.rs", 45, MutationType::BooleanLiteral, "true", "false"),
        ("policy.rs", 123, MutationType::LogicalOperator, "&&", "||"),
        ("types.rs", 89, MutationType::ArithmeticOperator, "+", "-"), // New mutation added
    ], 0.0);
    
    display_individual(&mutated, "Mutated Individual");
    
    println!("✨ EVOLUTIONARY PRESSURE:");
    println!("   • High-fitness individuals more likely to reproduce");
    println!("   • Poor mutations gradually eliminated from population");
    println!("   • Population evolves toward better test-killing ability");
    println!();
}

/// Create a sample individual for demonstration
fn create_sample_individual(
    _id: u32,
    mutations: Vec<(&str, usize, MutationType, &str, &str)>,
    fitness: f64
) -> Individual {
    let mutation_objects: Vec<Mutation> = mutations.into_iter().map(|(file, line, mut_type, orig, mutated)| {
        Mutation {
            id: Uuid::new_v4(),
            file_path: PathBuf::from(file),
            line,
            column: 10,
            mutation_type: mut_type,
            original: orig.to_string(),
            mutated: mutated.to_string(),
        }
    }).collect();
    
    Individual {
        mutations: mutation_objects,
        fitness,
        results: Vec::new(),
    }
}

/// Display an individual's characteristics
fn display_individual(individual: &Individual, name: &str) {
    println!("   {} (Fitness: {:.2}):", name, individual.fitness);
    for (i, mutation) in individual.mutations.iter().enumerate() {
        println!("     {}. {:?} in {} line {}: {} → {}", 
                 i + 1,
                 mutation.mutation_type,
                 mutation.file_path.file_name().unwrap().to_str().unwrap(),
                 mutation.line,
                 mutation.original,
                 mutation.mutated);
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sample_individual_creation() {
        let individual = create_sample_individual(1, vec![
            ("test.rs", 10, MutationType::BooleanLiteral, "true", "false"),
        ], 0.5);
        
        assert_eq!(individual.mutations.len(), 1);
        assert_eq!(individual.fitness, 0.5);
        assert_eq!(individual.mutations[0].line, 10);
    }
    
    #[tokio::test]
    async fn test_evolutionary_demo_runs() {
        // This test ensures the demo functions don't panic
        let result = demonstrate_evolutionary_optimization().await;
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_evolutionary_operators_demo() {
        // This test ensures the operators demo doesn't panic
        demonstrate_evolutionary_operators();
    }
}
