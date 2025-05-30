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


//! Circuit definitions and utilities for zkVM

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::errors::ZkVmError;

/// A circuit definition for zkVM execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Circuit {
    /// Circuit identifier
    pub id: String,
    /// Circuit name
    pub name: String,
    /// Circuit description
    pub description: String,
    /// Input schema
    pub inputs: Vec<CircuitInput>,
    /// Output schema
    pub outputs: Vec<CircuitOutput>,
    /// Circuit constraints
    pub constraints: Vec<Constraint>,
    /// Circuit metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Input definition for a circuit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitInput {
    /// Input name
    pub name: String,
    /// Input type
    pub input_type: InputType,
    /// Whether this input is public
    pub is_public: bool,
    /// Input description
    pub description: String,
}

/// Output definition for a circuit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitOutput {
    /// Output name
    pub name: String,
    /// Output type
    pub output_type: OutputType,
    /// Output description
    pub description: String,
}

/// Types of circuit inputs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    /// Field element
    Field,
    /// Boolean value
    Boolean,
    /// Integer value
    Integer,
    /// Array of values
    Array(Box<InputType>),
    /// Bytes
    Bytes,
}

/// Types of circuit outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputType {
    /// Field element
    Field,
    /// Boolean value
    Boolean,
    /// Integer value
    Integer,
    /// Array of values
    Array(Box<OutputType>),
    /// Bytes
    Bytes,
}

/// A constraint in the circuit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    /// Constraint type
    pub constraint_type: ConstraintType,
    /// Variables involved in the constraint
    pub variables: Vec<String>,
    /// Constraint parameters
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Types of constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintType {
    /// Equality constraint (a == b)
    Equality,
    /// Addition constraint (a + b == c)
    Addition,
    /// Multiplication constraint (a * b == c)
    Multiplication,
    /// Boolean constraint (a is 0 or 1)
    Boolean,
    /// Range constraint (a is in range [min, max])
    Range,
    /// Hash constraint (hash(a) == b)
    Hash,
    /// Custom constraint
    Custom(String),
}

impl Circuit {
    /// Create a new circuit
    pub fn new(id: String, name: String, description: String) -> Self {
        Self {
            id,
            name,
            description,
            inputs: Vec::new(),
            outputs: Vec::new(),
            constraints: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add an input to the circuit
    pub fn add_input(&mut self, input: CircuitInput) {
        self.inputs.push(input);
    }

    /// Add an output to the circuit
    pub fn add_output(&mut self, output: CircuitOutput) {
        self.outputs.push(output);
    }

    /// Add a constraint to the circuit
    pub fn add_constraint(&mut self, constraint: Constraint) {
        self.constraints.push(constraint);
    }

    /// Validate the circuit definition
    pub fn validate(&self) -> Result<(), ZkVmError> {
        if self.id.is_empty() {
            return Err(ZkVmError::InvalidProgram("Circuit ID cannot be empty".to_string()));
        }

        if self.inputs.is_empty() {
            return Err(ZkVmError::InvalidProgram("Circuit must have at least one input".to_string()));
        }

        if self.outputs.is_empty() {
            return Err(ZkVmError::InvalidProgram("Circuit must have at least one output".to_string()));
        }

        // Validate that all constraint variables reference valid inputs/outputs
        let mut all_variables = std::collections::HashSet::new();
        for input in &self.inputs {
            all_variables.insert(input.name.clone());
        }
        for output in &self.outputs {
            all_variables.insert(output.name.clone());
        }

        for constraint in &self.constraints {
            for var in &constraint.variables {
                if !all_variables.contains(var) {
                    return Err(ZkVmError::InvalidProgram(
                        format!("Constraint references unknown variable: {}", var)
                    ));
                }
            }
        }

        Ok(())
    }

    /// Get the number of constraints
    pub fn constraint_count(&self) -> usize {
        self.constraints.len()
    }

    /// Get public inputs
    pub fn public_inputs(&self) -> Vec<&CircuitInput> {
        self.inputs.iter().filter(|input| input.is_public).collect()
    }

    /// Get private inputs
    pub fn private_inputs(&self) -> Vec<&CircuitInput> {
        self.inputs.iter().filter(|input| !input.is_public).collect()
    }
}

/// Circuit builder for easier circuit construction
pub struct CircuitBuilder {
    circuit: Circuit,
}

impl CircuitBuilder {
    /// Create a new circuit builder
    pub fn new(id: String, name: String, description: String) -> Self {
        Self {
            circuit: Circuit::new(id, name, description),
        }
    }

    /// Add a public input
    pub fn public_input(mut self, name: String, input_type: InputType, description: String) -> Self {
        self.circuit.add_input(CircuitInput {
            name,
            input_type,
            is_public: true,
            description,
        });
        self
    }

    /// Add a private input
    pub fn private_input(mut self, name: String, input_type: InputType, description: String) -> Self {
        self.circuit.add_input(CircuitInput {
            name,
            input_type,
            is_public: false,
            description,
        });
        self
    }

    /// Add an output
    pub fn output(mut self, name: String, output_type: OutputType, description: String) -> Self {
        self.circuit.add_output(CircuitOutput {
            name,
            output_type,
            description,
        });
        self
    }

    /// Add an equality constraint
    pub fn equality(mut self, var1: String, var2: String) -> Self {
        self.circuit.add_constraint(Constraint {
            constraint_type: ConstraintType::Equality,
            variables: vec![var1, var2],
            parameters: HashMap::new(),
        });
        self
    }

    /// Add an addition constraint (a + b == c)
    pub fn addition(mut self, a: String, b: String, c: String) -> Self {
        self.circuit.add_constraint(Constraint {
            constraint_type: ConstraintType::Addition,
            variables: vec![a, b, c],
            parameters: HashMap::new(),
        });
        self
    }

    /// Add a multiplication constraint (a * b == c)
    pub fn multiplication(mut self, a: String, b: String, c: String) -> Self {
        self.circuit.add_constraint(Constraint {
            constraint_type: ConstraintType::Multiplication,
            variables: vec![a, b, c],
            parameters: HashMap::new(),
        });
        self
    }

    /// Build the circuit
    pub fn build(self) -> Result<Circuit, ZkVmError> {
        self.circuit.validate()?;
        Ok(self.circuit)
    }
}

/// Common circuit templates
pub struct CircuitTemplates;

impl CircuitTemplates {
    /// Create a simple hash verification circuit
    pub fn hash_verification() -> Result<Circuit, ZkVmError> {
        CircuitBuilder::new(
            "hash_verification".to_string(),
            "Hash Verification".to_string(),
            "Verifies that a hash matches the expected value".to_string(),
        )
        .private_input("preimage".to_string(), InputType::Bytes, "The preimage to hash".to_string())
        .public_input("expected_hash".to_string(), InputType::Bytes, "Expected hash value".to_string())
        .output("is_valid".to_string(), OutputType::Boolean, "Whether the hash is valid".to_string())
        .build()
    }

    /// Create a simple arithmetic circuit
    pub fn arithmetic() -> Result<Circuit, ZkVmError> {
        CircuitBuilder::new(
            "arithmetic".to_string(),
            "Arithmetic Circuit".to_string(),
            "Performs basic arithmetic operations".to_string(),
        )
        .private_input("a".to_string(), InputType::Integer, "First operand".to_string())
        .private_input("b".to_string(), InputType::Integer, "Second operand".to_string())
        .public_input("operation".to_string(), InputType::Integer, "Operation type (0=add, 1=mul)".to_string())
        .output("result".to_string(), OutputType::Integer, "Operation result".to_string())
        .build()
    }

    /// Create a range proof circuit
    pub fn range_proof(min: i64, max: i64) -> Result<Circuit, ZkVmError> {
        let mut circuit = CircuitBuilder::new(
            "range_proof".to_string(),
            "Range Proof".to_string(),
            format!("Proves that a value is in range [{}, {}]", min, max),
        )
        .private_input("value".to_string(), InputType::Integer, "Value to prove".to_string())
        .output("is_in_range".to_string(), OutputType::Boolean, "Whether value is in range".to_string())
        .build()?;

        // Add range constraint
        let mut params = HashMap::new();
        params.insert("min".to_string(), serde_json::Value::Number(min.into()));
        params.insert("max".to_string(), serde_json::Value::Number(max.into()));

        circuit.add_constraint(Constraint {
            constraint_type: ConstraintType::Range,
            variables: vec!["value".to_string()],
            parameters: params,
        });

        Ok(circuit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_creation() {
        let circuit = Circuit::new(
            "test".to_string(),
            "Test Circuit".to_string(),
            "A test circuit".to_string(),
        );

        assert_eq!(circuit.id, "test");
        assert_eq!(circuit.name, "Test Circuit");
        assert!(circuit.inputs.is_empty());
        assert!(circuit.outputs.is_empty());
    }

    #[test]
    fn test_circuit_builder() {
        let circuit = CircuitBuilder::new(
            "builder_test".to_string(),
            "Builder Test".to_string(),
            "Testing the builder".to_string(),
        )
        .public_input("x".to_string(), InputType::Integer, "Public input".to_string())
        .private_input("y".to_string(), InputType::Integer, "Private input".to_string())
        .output("z".to_string(), OutputType::Integer, "Output".to_string())
        .addition("x".to_string(), "y".to_string(), "z".to_string())
        .build()
        .unwrap();

        assert_eq!(circuit.inputs.len(), 2);
        assert_eq!(circuit.outputs.len(), 1);
        assert_eq!(circuit.constraints.len(), 1);
        assert_eq!(circuit.public_inputs().len(), 1);
        assert_eq!(circuit.private_inputs().len(), 1);
    }

    #[test]
    fn test_circuit_validation() {
        // Valid circuit
        let valid_circuit = CircuitBuilder::new(
            "valid".to_string(),
            "Valid".to_string(),
            "Valid circuit".to_string(),
        )
        .public_input("a".to_string(), InputType::Integer, "Input A".to_string())
        .output("b".to_string(), OutputType::Integer, "Output B".to_string())
        .build();

        assert!(valid_circuit.is_ok());

        // Invalid circuit (no inputs)
        let invalid_circuit = Circuit::new(
            "invalid".to_string(),
            "Invalid".to_string(),
            "Invalid circuit".to_string(),
        );

        assert!(invalid_circuit.validate().is_err());
    }

    #[test]
    fn test_circuit_templates() {
        let hash_circuit = CircuitTemplates::hash_verification().unwrap();
        assert_eq!(hash_circuit.id, "hash_verification");
        assert!(hash_circuit.validate().is_ok());

        let arithmetic_circuit = CircuitTemplates::arithmetic().unwrap();
        assert_eq!(arithmetic_circuit.id, "arithmetic");
        assert!(arithmetic_circuit.validate().is_ok());

        let range_circuit = CircuitTemplates::range_proof(0, 100).unwrap();
        assert_eq!(range_circuit.id, "range_proof");
        assert!(range_circuit.validate().is_ok());
    }
}
