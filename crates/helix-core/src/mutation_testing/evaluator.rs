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


//! Mutation evaluation engine that runs tests against mutated code

use super::{FitnessEvaluator, Mutation, MutationResult, TestResult};
use crate::HelixError;
use std::time::{Duration, Instant};
use std::fs;
use std::path::PathBuf;
use tokio::time::timeout;

/// Evaluates mutations by running tests
pub struct MutationEvaluator {
    /// Working directory for test execution
    work_dir: PathBuf,
    /// Test timeout duration
    timeout_duration: Duration,
}

impl MutationEvaluator {
    /// Create a new mutation evaluator
    pub fn new(work_dir: PathBuf, timeout_secs: u64) -> Self {
        Self {
            work_dir,
            timeout_duration: Duration::from_secs(timeout_secs),
        }
    }
    
    /// Evaluate a mutation by running tests
    pub async fn evaluate_mutation(
        &self,
        mutation: &Mutation,
        mutated_code: &str,
    ) -> Result<MutationResult, HelixError> {
        let start_time = Instant::now();
        
        // Write mutated code to file
        self.write_mutated_file(&mutation.file_path, mutated_code)?;
        
        // Run tests
        let test_results = self.run_tests().await?;
        
        // Restore original file
        self.restore_original_file(&mutation.file_path)?;
        
        // Calculate results
        let killed = test_results.iter().any(|t| !t.passed);
        let fitness = self.calculate_mutation_fitness(&test_results, killed);
        
        Ok(MutationResult {
            mutation: mutation.clone(),
            killed,
            test_results,
            fitness,
            execution_time: start_time.elapsed().as_millis() as u64,
        })
    }
    
    /// Write mutated code to file
    fn write_mutated_file(&self, file_path: &PathBuf, content: &str) -> Result<(), HelixError> {
        // Backup original file
        let backup_path = file_path.with_extension("bak");
        fs::copy(file_path, &backup_path)
            .map_err(HelixError::IoError)?;

        // Write mutated content
        fs::write(file_path, content)
            .map_err(HelixError::IoError)?;
        
        Ok(())
    }
    
    /// Restore original file from backup
    fn restore_original_file(&self, file_path: &PathBuf) -> Result<(), HelixError> {
        let backup_path = file_path.with_extension("bak");
        
        // Restore from backup
        fs::copy(&backup_path, file_path)
            .map_err(HelixError::IoError)?;

        // Remove backup
        fs::remove_file(&backup_path)
            .map_err(HelixError::IoError)?;
        
        Ok(())
    }
    
    /// Run test suite
    async fn run_tests(&self) -> Result<Vec<TestResult>, HelixError> {
        let output = timeout(
            self.timeout_duration,
            tokio::process::Command::new("cargo")
                .arg("test")
                .arg("--quiet")
                .current_dir(&self.work_dir)
                .output()
        ).await
        .map_err(|_| HelixError::InternalError("Test execution timed out".to_string()))?
        .map_err(HelixError::IoError)?;
        
        // Parse test output
        self.parse_test_output(&output.stdout, &output.stderr)
    }
    
    /// Parse test output to extract individual test results
    fn parse_test_output(&self, stdout: &[u8], stderr: &[u8]) -> Result<Vec<TestResult>, HelixError> {
        let stdout_str = String::from_utf8_lossy(stdout);
        let stderr_str = String::from_utf8_lossy(stderr);
        
        let mut results = Vec::new();
        
        // Simple parsing - in production, use a proper test output parser
        for line in stdout_str.lines() {
            if line.contains("test") && line.contains("...") {
                let parts: Vec<&str> = line.split("...").collect();
                if parts.len() >= 2 {
                    let test_name = parts[0].trim().replace("test ", "");
                    let passed = parts[1].trim() == "ok";
                    let error = if !passed {
                        Some(stderr_str.clone().into_owned())
                    } else {
                        None
                    };
                    
                    results.push(TestResult {
                        name: test_name,
                        passed,
                        error,
                        duration: 0, // Would need proper parsing for actual duration
                    });
                }
            }
        }
        
        // If no test results parsed, assume all tests passed (or failed)
        if results.is_empty() {
            results.push(TestResult {
                name: "all_tests".to_string(),
                passed: stderr_str.is_empty(),
                error: if stderr_str.is_empty() { None } else { Some(stderr_str.into_owned()) },
                duration: 0,
            });
        }
        
        Ok(results)
    }
    
    /// Calculate fitness score for a mutation
    fn calculate_mutation_fitness(&self, test_results: &[TestResult], killed: bool) -> f64 {
        if killed {
            // Higher fitness for mutations that are killed quickly
            let avg_duration = test_results.iter()
                .map(|t| t.duration as f64)
                .sum::<f64>() / test_results.len() as f64;
            
            // Normalize duration to 0-1 range (assuming max 1000ms)
            let duration_score = 1.0 - (avg_duration / 1000.0).min(1.0);
            
            // Bonus for failing multiple tests
            let failure_rate = test_results.iter()
                .filter(|t| !t.passed)
                .count() as f64 / test_results.len() as f64;
            
            0.5 + (0.3 * duration_score) + (0.2 * failure_rate)
        } else {
            // Lower fitness for surviving mutations
            // But give some credit based on test coverage
            let coverage_score = test_results.len() as f64 / 100.0; // Assume 100 tests max
            0.3 * coverage_score.min(1.0)
        }
    }
}

/// Default fitness evaluator implementation
pub struct DefaultFitnessEvaluator;

impl FitnessEvaluator for DefaultFitnessEvaluator {
    fn calculate_fitness(&self, result: &MutationResult) -> f64 {
        if result.killed {
            // Killed mutations have higher fitness
            let base_fitness = 0.7;
            
            // Bonus for quick detection
            let time_bonus = if result.execution_time < 1000 {
                0.2
            } else if result.execution_time < 5000 {
                0.1
            } else {
                0.0
            };
            
            // Bonus for multiple test failures
            let failure_bonus = (result.test_results.iter()
                .filter(|t| !t.passed)
                .count() as f64 / result.test_results.len() as f64) * 0.1;
            
            (base_fitness + time_bonus + failure_bonus).min(1.0)
        } else {
            // Surviving mutations have lower fitness
            // But we still want to evolve them
            0.2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    
    #[test]
    fn test_fitness_calculation() {
        let evaluator = DefaultFitnessEvaluator;
        
        // Killed mutation with quick detection
        let killed_result = MutationResult {
            mutation: Mutation {
                id: Uuid::new_v4(),
                file_path: PathBuf::from("test.rs"),
                line: 1,
                column: 1,
                mutation_type: super::super::MutationType::BooleanLiteral,
                original: "true".to_string(),
                mutated: "false".to_string(),
            },
            killed: true,
            test_results: vec![
                TestResult {
                    name: "test1".to_string(),
                    passed: false,
                    error: Some("assertion failed".to_string()),
                    duration: 100,
                },
            ],
            fitness: 0.0, // Will be calculated
            execution_time: 500,
        };
        
        let fitness = evaluator.calculate_fitness(&killed_result);
        assert!(fitness > 0.7);
        assert!(fitness <= 1.0);
        
        // Surviving mutation
        let surviving_result = MutationResult {
            mutation: killed_result.mutation.clone(),
            killed: false,
            test_results: vec![
                TestResult {
                    name: "test1".to_string(),
                    passed: true,
                    error: None,
                    duration: 100,
                },
            ],
            fitness: 0.0,
            execution_time: 500,
        };
        
        let fitness = evaluator.calculate_fitness(&surviving_result);
        assert!(fitness < 0.5);
        assert!(fitness >= 0.0);
    }
}