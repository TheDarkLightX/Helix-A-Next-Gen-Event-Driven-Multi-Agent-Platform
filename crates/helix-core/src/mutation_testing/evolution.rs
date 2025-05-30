//! Evolutionary algorithm for mutation testing

use super::{Mutation, MutationConfig, MutationResult, MutationStrategy, FitnessEvaluator};
use super::evaluator::{MutationEvaluator, DefaultFitnessEvaluator};
use super::mutator::Mutator;
use crate::HelixError;
use rand::prelude::*;
use std::path::PathBuf;
use std::fs;

/// Individual in the evolutionary population
#[derive(Clone)]
pub struct Individual {
    /// Set of mutations applied
    pub mutations: Vec<Mutation>,
    /// Fitness score
    pub fitness: f64,
    /// Test results for this individual
    pub results: Vec<MutationResult>,
}

/// Evolutionary mutation testing engine
pub struct EvolutionaryMutationTester {
    config: MutationConfig,
    mutator: Mutator,
    evaluator: MutationEvaluator,
    fitness_evaluator: Box<dyn FitnessEvaluator>,
}

impl EvolutionaryMutationTester {
    /// Create a new evolutionary mutation tester
    pub fn new(config: MutationConfig, work_dir: PathBuf) -> Self {
        Self {
            evaluator: MutationEvaluator::new(work_dir, config.test_timeout),
            config,
            mutator: Mutator::new(),
            fitness_evaluator: Box::new(DefaultFitnessEvaluator),
        }
    }
    
    /// Run evolutionary mutation testing
    pub async fn run(&mut self) -> Result<Vec<MutationResult>, HelixError> {
        let mut all_results = Vec::new();
        
        // Generate initial population
        let mut population = self.generate_initial_population()?;
        
        // Evolution loop
        for generation in 0..self.config.max_generations {
            println!("Generation {}/{}", generation + 1, self.config.max_generations);
            
            // Evaluate population
            self.evaluate_population(&mut population).await?;
            
            // Collect results
            for individual in &population {
                all_results.extend(individual.results.clone());
            }
            
            // Select and evolve
            population = self.evolve_population(population)?;
        }
        
        Ok(all_results)
    }
    
    /// Generate initial population of mutation sets
    fn generate_initial_population(&self) -> Result<Vec<Individual>, HelixError> {
        let mut population = Vec::new();
        let mut all_mutations = Vec::new();
        
        // Collect all possible mutations
        for file_path in &self.config.target_files {
            let mutations = self.mutator.generate_file_mutations(file_path)?;
            all_mutations.extend(mutations);
        }
        
        // Create individuals with random mutation subsets
        let mut rng = thread_rng();
        for _ in 0..self.config.population_size {
            let num_mutations = rng.gen_range(1..=5.min(all_mutations.len()));
            let mutations: Vec<Mutation> = all_mutations
                .choose_multiple(&mut rng, num_mutations)
                .cloned()
                .collect();
            
            population.push(Individual {
                mutations,
                fitness: 0.0,
                results: Vec::new(),
            });
        }
        
        Ok(population)
    }
    
    /// Evaluate fitness of all individuals in population
    async fn evaluate_population(&self, population: &mut Vec<Individual>) -> Result<(), HelixError> {
        for individual in population.iter_mut() {
            let mut total_fitness = 0.0;
            let mut results = Vec::new();
            
            for mutation in &individual.mutations {
                // Read original file
                let original_code = fs::read_to_string(&mutation.file_path)
                    .map_err(HelixError::IoError)?;
                
                // Apply mutation
                let mutated_code = self.mutator.apply_mutation(&original_code, mutation)?;
                
                // Evaluate
                let result = self.evaluator.evaluate_mutation(mutation, &mutated_code).await?;
                total_fitness += self.fitness_evaluator.calculate_fitness(&result);
                results.push(result);
            }
            
            individual.fitness = total_fitness / individual.mutations.len() as f64;
            individual.results = results;
        }
        
        Ok(())
    }
    
    /// Evolve population using genetic operators
    fn evolve_population(&self, mut population: Vec<Individual>) -> Result<Vec<Individual>, HelixError> {
        // Sort by fitness (descending)
        population.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap());
        
        let mut new_population = Vec::new();
        let mut rng = thread_rng();
        
        // Elitism: Keep top 20%
        let elite_count = self.config.population_size / 5;
        new_population.extend(population.iter().take(elite_count).cloned());
        
        // Generate rest through crossover and mutation
        while new_population.len() < self.config.population_size {
            if rng.gen::<f64>() < self.config.crossover_rate {
                // Crossover
                let parent1 = self.tournament_selection(&population, &mut rng);
                let parent2 = self.tournament_selection(&population, &mut rng);
                let child = self.crossover(parent1, parent2, &mut rng);
                new_population.push(child);
            } else {
                // Mutation
                let parent = self.tournament_selection(&population, &mut rng);
                let child = self.mutate_individual(parent, &mut rng)?;
                new_population.push(child);
            }
        }
        
        Ok(new_population)
    }
    
    /// Tournament selection
    fn tournament_selection<'a>(&self, population: &'a [Individual], rng: &mut ThreadRng) -> &'a Individual {
        let tournament_size = 3;
        let mut best = &population[rng.gen_range(0..population.len())];
        
        for _ in 1..tournament_size {
            let candidate = &population[rng.gen_range(0..population.len())];
            if candidate.fitness > best.fitness {
                best = candidate;
            }
        }
        
        best
    }
    
    /// Crossover two individuals
    fn crossover(&self, parent1: &Individual, parent2: &Individual, rng: &mut ThreadRng) -> Individual {
        let mut child_mutations = Vec::new();
        
        // Uniform crossover
        for mutation in &parent1.mutations {
            if rng.gen::<bool>() {
                child_mutations.push(mutation.clone());
            }
        }
        
        for mutation in &parent2.mutations {
            if rng.gen::<bool>() && !child_mutations.iter().any(|m| m.id == mutation.id) {
                child_mutations.push(mutation.clone());
            }
        }
        
        // Ensure at least one mutation
        if child_mutations.is_empty() {
            child_mutations.push(parent1.mutations[0].clone());
        }
        
        Individual {
            mutations: child_mutations,
            fitness: 0.0,
            results: Vec::new(),
        }
    }
    
    /// Mutate an individual
    fn mutate_individual(&self, parent: &Individual, rng: &mut ThreadRng) -> Result<Individual, HelixError> {
        let mut child = parent.clone();
        
        // Add or remove mutations based on mutation rate
        if rng.gen::<f64>() < self.config.mutation_rate {
            // Try to add a new mutation
            if let Some(file_path) = self.config.target_files.choose(rng) {
                let available_mutations = self.mutator.generate_file_mutations(file_path)?;
                if let Some(new_mutation) = available_mutations.choose(rng) {
                    child.mutations.push(new_mutation.clone());
                }
            }
        }
        
        // Remove a random mutation
        if child.mutations.len() > 1 && rng.gen::<f64>() < self.config.mutation_rate {
            let idx = rng.gen_range(0..child.mutations.len());
            child.mutations.remove(idx);
        }
        
        child.fitness = 0.0;
        child.results = Vec::new();
        
        Ok(child)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_evolutionary_tester_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = MutationConfig {
            target_files: vec![PathBuf::from("test.rs")],
            max_generations: 2,
            population_size: 5,
            mutation_rate: 0.1,
            crossover_rate: 0.7,
            test_timeout: 10,
        };
        
        let tester = EvolutionaryMutationTester::new(config, temp_dir.path().to_path_buf());
        assert_eq!(tester.config.max_generations, 2);
        assert_eq!(tester.config.population_size, 5);
    }
    
    #[test]
    fn test_crossover() {
        let temp_dir = TempDir::new().unwrap();
        let config = MutationConfig::default();
        let tester = EvolutionaryMutationTester::new(config, temp_dir.path().to_path_buf());
        
        let parent1 = Individual {
            mutations: vec![
                Mutation {
                    id: uuid::Uuid::new_v4(),
                    file_path: PathBuf::from("test.rs"),
                    line: 1,
                    column: 1,
                    mutation_type: super::super::MutationType::BooleanLiteral,
                    original: "true".to_string(),
                    mutated: "false".to_string(),
                },
            ],
            fitness: 0.8,
            results: Vec::new(),
        };
        
        let parent2 = Individual {
            mutations: vec![
                Mutation {
                    id: uuid::Uuid::new_v4(),
                    file_path: PathBuf::from("test.rs"),
                    line: 2,
                    column: 1,
                    mutation_type: super::super::MutationType::ArithmeticOperator,
                    original: "+".to_string(),
                    mutated: "-".to_string(),
                },
            ],
            fitness: 0.6,
            results: Vec::new(),
        };
        
        let mut rng = thread_rng();
        let child = tester.crossover(&parent1, &parent2, &mut rng);
        
        assert!(!child.mutations.is_empty());
        assert!(child.mutations.len() <= 2);
    }
}