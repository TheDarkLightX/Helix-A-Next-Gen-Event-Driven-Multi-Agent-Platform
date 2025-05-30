//! Mutation operators for different code patterns

use super::{Mutation, MutationType};
use crate::HelixError;
use regex::Regex;
use std::path::PathBuf;
use uuid::Uuid;

/// Base trait for mutation operators
pub trait MutationOperator: Send + Sync {
    /// Apply this operator to generate mutations
    fn mutate(&self, code: &str, file_path: &Path) -> Result<Vec<Mutation>, HelixError>;
}

/// Arithmetic operator mutations (+, -, *, /, %)
pub struct ArithmeticOperatorMutator;

impl MutationOperator for ArithmeticOperatorMutator {
    fn mutate(&self, code: &str, file_path: &Path) -> Result<Vec<Mutation>, HelixError> {
        let mut mutations = Vec::new();
        let operators = vec![
            ("+", vec!["-", "*", "/", "%"]),
            ("-", vec!["+", "*", "/", "%"]),
            ("*", vec!["+", "-", "/", "%"]),
            ("/", vec!["+", "-", "*", "%"]),
            ("%", vec!["+", "-", "*", "/"]),
        ];
        
        for (line_num, line) in code.lines().enumerate() {
            for (op, replacements) in &operators {
                if let Some(col) = line.find(op) {
                    for replacement in replacements {
                        mutations.push(Mutation {
                            id: Uuid::new_v4(),
                            file_path: file_path.clone(),
                            line: line_num + 1,
                            column: col + 1,
                            mutation_type: MutationType::ArithmeticOperator,
                            original: op.to_string(),
                            mutated: replacement.to_string(),
                        });
                    }
                }
            }
        }
        
        Ok(mutations)
    }
}

/// Comparison operator mutations
pub struct ComparisonOperatorMutator;

impl MutationOperator for ComparisonOperatorMutator {
    fn mutate(&self, code: &str, file_path: &Path) -> Result<Vec<Mutation>, HelixError> {
        let mut mutations = Vec::new();
        let operators = vec![
            ("==", vec!["!=", "<", ">", "<=", ">="]),
            ("!=", vec!["==", "<", ">", "<=", ">="]),
            ("<=", vec!["<", ">=", ">", "==", "!="]),
            (">=", vec![">", "<=", "<", "==", "!="]),
            ("<", vec!["<=", ">", ">=", "==", "!="]),
            (">", vec![">=", "<", "<=", "==", "!="]),
        ];
        
        for (line_num, line) in code.lines().enumerate() {
            for (op, replacements) in &operators {
                if let Some(col) = line.find(op) {
                    for replacement in replacements {
                        mutations.push(Mutation {
                            id: Uuid::new_v4(),
                            file_path: file_path.clone(),
                            line: line_num + 1,
                            column: col + 1,
                            mutation_type: MutationType::ComparisonOperator,
                            original: op.to_string(),
                            mutated: replacement.to_string(),
                        });
                    }
                }
            }
        }
        
        Ok(mutations)
    }
}

/// Boolean literal mutations
pub struct BooleanLiteralMutator;

impl MutationOperator for BooleanLiteralMutator {
    fn mutate(&self, code: &str, file_path: &Path) -> Result<Vec<Mutation>, HelixError> {
        let mut mutations = Vec::new();
        let true_regex = Regex::new(r"\btrue\b").unwrap();
        let false_regex = Regex::new(r"\bfalse\b").unwrap();
        
        for (line_num, line) in code.lines().enumerate() {
            // Find true literals
            for mat in true_regex.find_iter(line) {
                mutations.push(Mutation {
                    id: Uuid::new_v4(),
                    file_path: file_path.clone(),
                    line: line_num + 1,
                    column: mat.start() + 1,
                    mutation_type: MutationType::BooleanLiteral,
                    original: "true".to_string(),
                    mutated: "false".to_string(),
                });
            }
            
            // Find false literals
            for mat in false_regex.find_iter(line) {
                mutations.push(Mutation {
                    id: Uuid::new_v4(),
                    file_path: file_path.clone(),
                    line: line_num + 1,
                    column: mat.start() + 1,
                    mutation_type: MutationType::BooleanLiteral,
                    original: "false".to_string(),
                    mutated: "true".to_string(),
                });
            }
        }
        
        Ok(mutations)
    }
}

/// Logical operator mutations
pub struct LogicalOperatorMutator;

impl MutationOperator for LogicalOperatorMutator {
    fn mutate(&self, code: &str, file_path: &Path) -> Result<Vec<Mutation>, HelixError> {
        let mut mutations = Vec::new();
        let operators = vec![
            ("&&", vec!["||"]),
            ("||", vec!["&&"]),
        ];
        
        for (line_num, line) in code.lines().enumerate() {
            for (op, replacements) in &operators {
                let mut start = 0;
                while let Some(col) = line[start..].find(op) {
                    let actual_col = start + col;
                    for replacement in replacements {
                        mutations.push(Mutation {
                            id: Uuid::new_v4(),
                            file_path: file_path.clone(),
                            line: line_num + 1,
                            column: actual_col + 1,
                            mutation_type: MutationType::LogicalOperator,
                            original: op.to_string(),
                            mutated: replacement.to_string(),
                        });
                    }
                    start = actual_col + op.len();
                }
            }
        }
        
        Ok(mutations)
    }
}

/// Composite operator that combines multiple mutation operators
pub struct CompositeOperator {
    operators: Vec<Box<dyn MutationOperator>>,
}

impl Default for CompositeOperator {
    fn default() -> Self {
        Self::new()
    }
}

impl CompositeOperator {
    /// Create a new composite operator with default mutation operators
    pub fn new() -> Self {
        Self {
            operators: vec![
                Box::new(ArithmeticOperatorMutator),
                Box::new(ComparisonOperatorMutator),
                Box::new(BooleanLiteralMutator),
                Box::new(LogicalOperatorMutator),
            ],
        }
    }
}

impl MutationOperator for CompositeOperator {
    fn mutate(&self, code: &str, file_path: &Path) -> Result<Vec<Mutation>, HelixError> {
        let mut all_mutations = Vec::new();
        
        for operator in &self.operators {
            match operator.mutate(code, file_path) {
                Ok(mutations) => all_mutations.extend(mutations),
                Err(e) => return Err(e),
            }
        }
        
        Ok(all_mutations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_arithmetic_operator_mutator() {
        let mutator = ArithmeticOperatorMutator;
        let code = "let result = a + b - c * d / e % f;";
        let path = PathBuf::from("test.rs");
        
        let mutations = mutator.mutate(code, &path).unwrap();
        assert!(!mutations.is_empty());
        
        // Check that we found the + operator
        let plus_mutations: Vec<_> = mutations.iter()
            .filter(|m| m.original == "+")
            .collect();
        assert!(!plus_mutations.is_empty());
    }
    
    #[test]
    fn test_comparison_operator_mutator() {
        let mutator = ComparisonOperatorMutator;
        let code = "if a == b && c != d && e < f && g > h && i <= j && k >= l {}";
        let path = PathBuf::from("test.rs");
        
        let mutations = mutator.mutate(code, &path).unwrap();
        assert!(!mutations.is_empty());
        
        // Check various operators were found
        assert!(mutations.iter().any(|m| m.original == "=="));
        assert!(mutations.iter().any(|m| m.original == "!="));
        assert!(mutations.iter().any(|m| m.original == "<"));
    }
    
    #[test]
    fn test_boolean_literal_mutator() {
        let mutator = BooleanLiteralMutator;
        let code = "let a = true; let b = false; if true { return false; }";
        let path = PathBuf::from("test.rs");
        
        let mutations = mutator.mutate(code, &path).unwrap();
        assert_eq!(mutations.len(), 4); // 2 true and 2 false literals
    }
    
    #[test]
    fn test_logical_operator_mutator() {
        let mutator = LogicalOperatorMutator;
        let code = "if a && b || c && d {}";
        let path = PathBuf::from("test.rs");
        
        let mutations = mutator.mutate(code, &path).unwrap();
        assert_eq!(mutations.len(), 3); // 2 && and 1 ||
    }
}