//! Core mutation engine that applies mutations to source code

use super::{Mutation, MutationStrategy, MutationType};
use super::operators::{CompositeOperator, MutationOperator};
use crate::HelixError;
use std::fs;
use std::path::PathBuf;

/// Main mutator that coordinates mutation generation and application
pub struct Mutator {
    operator: CompositeOperator,
}

impl Default for Mutator {
    fn default() -> Self {
        Self::new()
    }
}

impl Mutator {
    /// Create a new mutator with default operators
    pub fn new() -> Self {
        Self {
            operator: CompositeOperator::new(),
        }
    }
    
    /// Generate all possible mutations for a file
    pub fn generate_file_mutations(&self, file_path: &PathBuf) -> Result<Vec<Mutation>, HelixError> {
        let code = fs::read_to_string(file_path)
            .map_err(HelixError::IoError)?;

        self.operator.mutate(&code, file_path.as_path())
    }
    
    /// Apply a specific mutation to a file's content
    pub fn apply_mutation_to_code(&self, code: &str, mutation: &Mutation) -> Result<String, HelixError> {
        let lines: Vec<&str> = code.lines().collect();
        
        if mutation.line == 0 || mutation.line > lines.len() {
            return Err(HelixError::ValidationError {
                context: "mutation".to_string(),
                message: format!("Invalid line number: {}", mutation.line),
            });
        }
        
        let mut result = String::new();
        
        for (idx, line) in lines.iter().enumerate() {
            if idx + 1 == mutation.line {
                // Apply mutation to this line
                let mutated_line = self.apply_mutation_to_line(line, mutation)?;
                result.push_str(&mutated_line);
            } else {
                result.push_str(line);
            }
            
            if idx < lines.len() - 1 {
                result.push('\n');
            }
        }
        
        Ok(result)
    }
    
    /// Apply mutation to a specific line
    fn apply_mutation_to_line(&self, line: &str, mutation: &Mutation) -> Result<String, HelixError> {
        // Simple string replacement for now
        // In a real implementation, we'd use proper AST manipulation
        Ok(line.replacen(&mutation.original, &mutation.mutated, 1))
    }
}

impl MutationStrategy for Mutator {
    fn generate_mutations(&self, code: &str) -> Result<Vec<Mutation>, HelixError> {
        let temp_path = PathBuf::from("temp.rs");
        self.operator.mutate(code, &temp_path)
    }
    
    fn apply_mutation(&self, code: &str, mutation: &Mutation) -> Result<String, HelixError> {
        self.apply_mutation_to_code(code, mutation)
    }
}

/// Filters mutations based on various criteria
pub struct MutationFilter;

impl MutationFilter {
    /// Filter mutations to avoid equivalent mutations
    pub fn filter_equivalent(&self, mutations: Vec<Mutation>) -> Vec<Mutation> {
        mutations.into_iter()
            .filter(|m| !self.is_likely_equivalent(m))
            .collect()
    }
    
    /// Check if a mutation is likely to be equivalent (no behavioral change)
    fn is_likely_equivalent(&self, mutation: &Mutation) -> bool {
        match mutation.mutation_type {
            MutationType::ArithmeticOperator => {
                // x * 0 -> x / 0 would cause runtime error, not equivalent
                mutation.original == "*" && mutation.mutated == "/" && 
                    mutation.original.contains('0')
            }
            MutationType::ComparisonOperator => {
                // Some comparisons might be equivalent in specific contexts
                false
            }
            _ => false,
        }
    }
    
    /// Prioritize mutations based on likelihood of being caught
    pub fn prioritize(&self, mutations: Vec<Mutation>) -> Vec<Mutation> {
        let mut sorted = mutations;
        sorted.sort_by_key(|m| self.mutation_priority(m));
        sorted
    }
    
    /// Assign priority score to mutation types (lower is higher priority)
    fn mutation_priority(&self, mutation: &Mutation) -> u32 {
        match mutation.mutation_type {
            MutationType::BooleanLiteral => 1,      // Most likely to be caught
            MutationType::ComparisonOperator => 2,
            MutationType::LogicalOperator => 3,
            MutationType::ArithmeticOperator => 4,
            MutationType::ReturnValue => 5,
            MutationType::ConditionalStatement => 6,
            MutationType::NumericConstant => 7,
            MutationType::FunctionCall => 8,        // Least likely to be caught
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    
    #[test]
    fn test_mutator_apply_mutation() {
        let mutator = Mutator::new();
        let code = "let a = 5 + 3;\nlet b = 10 - 2;";
        
        let mutation = Mutation {
            id: Uuid::new_v4(),
            file_path: PathBuf::from("test.rs"),
            line: 1,
            column: 11,
            mutation_type: MutationType::ArithmeticOperator,
            original: "+".to_string(),
            mutated: "-".to_string(),
        };
        
        let result = mutator.apply_mutation_to_code(code, &mutation).unwrap();
        assert!(result.contains("5 - 3"));
        assert!(result.contains("10 - 2")); // Second line unchanged
    }
    
    #[test]
    fn test_mutation_filter_prioritize() {
        let filter = MutationFilter;
        
        let mutations = vec![
            Mutation {
                id: Uuid::new_v4(),
                file_path: PathBuf::from("test.rs"),
                line: 1,
                column: 1,
                mutation_type: MutationType::FunctionCall,
                original: "call()".to_string(),
                mutated: "".to_string(),
            },
            Mutation {
                id: Uuid::new_v4(),
                file_path: PathBuf::from("test.rs"),
                line: 2,
                column: 1,
                mutation_type: MutationType::BooleanLiteral,
                original: "true".to_string(),
                mutated: "false".to_string(),
            },
        ];
        
        let prioritized = filter.prioritize(mutations);
        assert_eq!(prioritized[0].mutation_type, MutationType::BooleanLiteral);
        assert_eq!(prioritized[1].mutation_type, MutationType::FunctionCall);
    }
}