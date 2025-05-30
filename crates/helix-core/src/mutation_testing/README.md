# Evolutionary Mutation Testing Framework

A comprehensive mutation testing framework for Rust that uses evolutionary algorithms to generate and evaluate code mutations, helping identify weaknesses in test suites.

## Features

- **Multiple Mutation Operators**: Arithmetic, comparison, logical, and boolean literal mutations
- **Evolutionary Algorithm**: Uses genetic algorithms to evolve effective mutation sets
- **Fitness-Based Selection**: Prioritizes mutations based on test detection effectiveness
- **Configurable Parameters**: Control population size, mutation rates, and evolution parameters
- **Test Integration**: Seamlessly integrates with Rust's built-in test framework

## Architecture

The framework follows SOLID principles with clear separation of concerns:

```
mutation_testing/
├── mod.rs          # Core types and traits
├── operators.rs    # Mutation operators (Strategy pattern)
├── mutator.rs      # Mutation application engine
├── evaluator.rs    # Test execution and fitness evaluation
└── evolution.rs    # Evolutionary algorithm implementation
```

## Usage

### Basic Example

```rust
use helix_core::mutation_testing::{
    MutationConfig,
    evolution::EvolutionaryMutationTester,
};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure mutation testing
    let config = MutationConfig {
        target_files: vec![PathBuf::from("src/lib.rs")],
        max_generations: 10,
        population_size: 20,
        mutation_rate: 0.1,
        crossover_rate: 0.7,
        test_timeout: 30,
    };
    
    // Run evolutionary mutation testing
    let work_dir = std::env::current_dir()?;
    let mut tester = EvolutionaryMutationTester::new(config, work_dir);
    let results = tester.run().await?;
    
    // Analyze results
    let killed_count = results.iter().filter(|r| r.killed).count();
    let total_count = results.len();
    let mutation_score = killed_count as f64 / total_count as f64;
    
    println!("Mutation Score: {:.2}%", mutation_score * 100.0);
    
    Ok(())
}
```

### Custom Mutation Operators

```rust
use helix_core::mutation_testing::operators::MutationOperator;

struct CustomOperator;

impl MutationOperator for CustomOperator {
    fn mutate(&self, code: &str, file_path: &PathBuf) -> Result<Vec<Mutation>, HelixError> {
        // Implement custom mutation logic
        Ok(vec![])
    }
}
```

## Mutation Types

1. **Arithmetic Operators**: `+` → `-`, `*`, `/`, `%`
2. **Comparison Operators**: `==` → `!=`, `<`, `>`, `<=`, `>=`
3. **Logical Operators**: `&&` → `||`
4. **Boolean Literals**: `true` → `false`

## Evolutionary Algorithm

The framework uses a genetic algorithm with:

- **Selection**: Tournament selection with configurable tournament size
- **Crossover**: Uniform crossover between parent mutations
- **Mutation**: Add/remove mutations based on mutation rate
- **Elitism**: Top 20% of population preserved each generation

## Best Practices

1. **Start Small**: Begin with a small population and few generations
2. **Filter Equivalents**: Use `MutationFilter` to remove likely equivalent mutations
3. **Prioritize**: Focus on high-priority mutations first
4. **Monitor Performance**: Track execution time and adjust timeouts
5. **Incremental Testing**: Test individual modules before full codebase

## Performance Considerations

- Mutation testing is computationally expensive
- Use parallel test execution when possible
- Consider caching test results for identical mutations
- Limit scope to critical code paths initially

## Future Enhancements

- [ ] AST-based mutation for more precise modifications
- [ ] Parallel mutation evaluation
- [ ] Machine learning for mutation prediction
- [ ] Integration with CI/CD pipelines
- [ ] Visual mutation reports