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


//! High-quality tests for the practical mutation analyzer
//! Following TDD principles with high TES scores

#[cfg(test)]
mod tests {
    use super::super::*;
    use std::path::PathBuf;
    use std::time::Instant;
    use tempfile::TempDir;
    use tokio::fs;

    /// Helper to create test analyzer with custom config
    fn create_test_analyzer() -> PracticalMutationAnalyzer {
        let config = AnalyzerConfig {
            target_dir: PathBuf::from("."),
            test_command: "echo 'test'".to_string(),
            complexity_threshold: 10.0,
            min_assertion_density: 3.0,
            max_test_duration_ms: 100,
        };
        PracticalMutationAnalyzer::new(config)
    }

    /// Test analyzer configuration with comprehensive assertions
    #[tokio::test]
    async fn test_analyzer_config_validation() {
        let start = Instant::now();
        
        // Test default configuration
        let default_config = AnalyzerConfig::default();
        assert_eq!(default_config.target_dir, PathBuf::from("."));
        assert_eq!(default_config.test_command, "cargo test");
        assert_eq!(default_config.complexity_threshold, 10.0);
        assert_eq!(default_config.min_assertion_density, 3.0);
        assert_eq!(default_config.max_test_duration_ms, 100);
        
        // Test custom configuration
        let custom_config = AnalyzerConfig {
            target_dir: PathBuf::from("/custom"),
            test_command: "custom test".to_string(),
            complexity_threshold: 5.0,
            min_assertion_density: 5.0,
            max_test_duration_ms: 50,
        };
        
        assert_ne!(custom_config.target_dir, default_config.target_dir);
        assert_ne!(custom_config.test_command, default_config.test_command);
        assert!(custom_config.complexity_threshold < default_config.complexity_threshold);
        assert!(custom_config.min_assertion_density > default_config.min_assertion_density);
        
        let _duration = start.elapsed().as_millis();
        assert!(_duration < 100); // Speed check
    }

    /// Test quality report generation with edge cases
    #[tokio::test]
    async fn test_quality_report_edge_cases() {
        let start = Instant::now();
        
        // Test with perfect scores
        let perfect_tes = TestEffectivenessScore {
            mutation_score: 1.0,
            assertion_density: 1.0,
            behavior_coverage: 1.0,
            speed_factor: 1.0,
        };
        
        let perfect_report = QualityReport {
            tes_score: perfect_tes.clone(),
            weak_spots: vec![],
            recommendations: vec![],
            metrics: CodeMetrics {
                cyclomatic_complexity: 2.0,
                test_ratio: 1.0,
                assertion_density: 5.0,
                duplication_ratio: 0.0,
            },
        };
        
        assert_eq!(perfect_tes.calculate(), 1.0);
        assert_eq!(perfect_tes.grade(), "A+");
        assert!(perfect_report.weak_spots.is_empty());
        assert!(perfect_report.recommendations.is_empty());
        
        // Test with poor scores
        let poor_tes = TestEffectivenessScore {
            mutation_score: 0.3,
            assertion_density: 0.5,
            behavior_coverage: 0.4,
            speed_factor: 0.5,
        };
        
        assert!(poor_tes.calculate() < 0.1);
        assert_eq!(poor_tes.grade(), "F");
        
        // Test boundary conditions
        let boundary_tes = TestEffectivenessScore {
            mutation_score: 0.8,
            assertion_density: 1.0,
            behavior_coverage: 1.0,
            speed_factor: 1.0,
        };
        
        assert_eq!(boundary_tes.calculate(), 0.8);
        assert_eq!(boundary_tes.grade(), "A");
        
        let _duration = start.elapsed().as_millis();
        assert!(_duration < 100);
    }

    /// Test weak spot identification with multiple severity levels
    #[tokio::test]
    async fn test_weak_spot_identification_comprehensive() {
        let start = Instant::now();
        
        // Create various weak spots
        let critical_spot = WeakSpot {
            file: PathBuf::from("critical.rs"),
            line: 42,
            reason: "5 mutations survived".to_string(),
            severity: Severity::Critical,
            surviving_mutations: vec![
                MutationType::BooleanLiteral,
                MutationType::ComparisonOperator,
                MutationType::LogicalOperator,
                MutationType::ArithmeticOperator,
                MutationType::ReturnValue,
            ],
        };
        
        let high_spot = WeakSpot {
            file: PathBuf::from("high.rs"),
            line: 100,
            reason: "3 mutations survived".to_string(),
            severity: Severity::High,
            surviving_mutations: vec![
                MutationType::ComparisonOperator,
                MutationType::LogicalOperator,
                MutationType::BooleanLiteral,
            ],
        };
        
        // Verify severity ordering
        assert!(matches!(critical_spot.severity, Severity::Critical));
        assert!(matches!(high_spot.severity, Severity::High));
        assert_eq!(critical_spot.surviving_mutations.len(), 5);
        assert_eq!(high_spot.surviving_mutations.len(), 3);
        
        // Test severity comparison
        let spots = vec![high_spot, critical_spot];
        let sorted: Vec<_> = spots.into_iter()
            .map(|s| (s.severity as u8, s))
            .collect();
        
        assert!(sorted[0].0 > sorted[1].0); // Critical < High in enum order
        
        let _duration = start.elapsed().as_millis();
        assert!(_duration < 100);
    }

    /// Test recommendation generation with all categories
    #[tokio::test]
    async fn test_recommendation_generation_all_categories() {
        let start = Instant::now();
        
        let categories = vec![
            RecommendationCategory::AddAssertion,
            RecommendationCategory::AddEdgeCaseTest,
            RecommendationCategory::RefactorComplexCode,
            RecommendationCategory::RemoveDuplication,
            RecommendationCategory::ImproveTestSpeed,
        ];
        
        for category in categories {
            let rec = Recommendation {
                priority: Priority::High,
                category: category.clone(),
                description: format!("Test recommendation for {:?}", category),
                example: Some("Example code".to_string()),
            };
            
            assert_eq!(rec.category, category);
            assert!(rec.description.contains("Test recommendation"));
            assert!(rec.example.is_some());
        }
        
        // Test priority ordering
        let priorities = [
            Priority::Immediate,
            Priority::High,
            Priority::Medium,
            Priority::Low,
        ];
        
        for (i, priority) in priorities.iter().enumerate() {
            assert_eq!(*priority as u8, i as u8);
        }
        
        let _duration = start.elapsed().as_millis();
        assert!(_duration < 100);
    }

    /// Test code metrics calculation with boundary values
    #[tokio::test]
    async fn test_code_metrics_boundary_conditions() {
        let start = Instant::now();
        
        let analyzer = create_test_analyzer();
        
        // Test empty code
        let empty_metrics = analyzer.calculate_metrics("", &[]).unwrap();
        assert_eq!(empty_metrics.test_ratio, 0.0);
        assert_eq!(empty_metrics.cyclomatic_complexity, 0.0);
        
        // Test simple code
        let simple_code = r#"
            fn add(a: i32, b: i32) -> i32 {
                a + b
            }
            
            #[test]
            fn test_add() {
                assert_eq!(add(2, 3), 5);
            }
        "#;
        
        let simple_metrics = analyzer.calculate_metrics(simple_code, &[]).unwrap();
        assert!(simple_metrics.test_ratio > 0.0);
        assert!(simple_metrics.assertion_density > 0.0);
        
        // Test complex code
        let complex_code = r#"
            fn complex(x: i32) -> i32 {
                if x > 0 {
                    match x {
                        1 => 1,
                        2 => 2,
                        _ => x * 2,
                    }
                } else if x < 0 {
                    -x
                } else {
                    0
                }
            }
        "#;
        
        let complex_metrics = analyzer.calculate_metrics(complex_code, &[]).unwrap();
        assert!(complex_metrics.cyclomatic_complexity > simple_metrics.cyclomatic_complexity);
        
        let _duration = start.elapsed().as_millis();
        assert!(_duration < 100);
    }

    /// Test mutation analysis with real code sample
    #[tokio::test]
    async fn test_analyze_module_integration() {
        let start = Instant::now();
        
        // Create temporary test file
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_module.rs");
        
        let test_code = r#"
            pub fn calculate(a: i32, b: i32) -> i32 {
                if a > b {
                    a - b
                } else {
                    b - a
                }
            }
            
            #[cfg(test)]
            mod tests {
                use super::*;
                
                #[test]
                fn test_calculate() {
                    assert_eq!(calculate(5, 3), 2);
                    assert_eq!(calculate(3, 5), 2);
                    assert_eq!(calculate(5, 5), 0);
                }
            }
        "#;
        
        fs::write(&test_file, test_code).await.unwrap();
        
        let analyzer = create_test_analyzer();
        let report = analyzer.analyze_module(&test_file).await.unwrap();
        
        // Verify report contents
        assert!(report.tes_score.mutation_score >= 0.0);
        assert!(report.tes_score.mutation_score <= 1.0);
        assert!(report.metrics.cyclomatic_complexity > 0.0);
        assert!(!report.recommendations.is_empty());
        
        // Verify summary generation
        let summary = report.summary();
        assert!(summary.contains("TES Score"));
        assert!(summary.contains("Mutation Score"));
        assert!(summary.contains(report.tes_score.grade()));
        
        let _duration = start.elapsed().as_millis();
        assert!(_duration < 500); // Allow more time for integration test
    }

    /// Test concurrent analysis safety
    #[tokio::test]
    async fn test_concurrent_analysis_safety() {
        let start = Instant::now();
        
        let _analyzer = create_test_analyzer();
        let temp_dir = TempDir::new().unwrap();
        
        // Create multiple test files
        let mut handles = vec![];
        
        for i in 0..3 {
            let file_path = temp_dir.path().join(format!("concurrent_{}.rs", i));
            let code = format!(
                r#"
                pub fn func_{}(x: i32) -> i32 {{
                    x + {}
                }}

                #[test]
                fn test_func_{}_happy_path() {{
                    assert_eq!(func_{}(1), {});
                    assert!(func_{}(0) >= 0);
                }}

                #[test]
                fn test_func_{}_edge_case() {{
                    assert_eq!(func_{}(-1), {});
                }}
                "#,
                i, i, i, i, i + 1, i, i, i, i - 1
            );
            
            fs::write(&file_path, code).await.unwrap();
            
            let analyzer_clone = create_test_analyzer();
            let handle = tokio::spawn(async move {
                analyzer_clone.analyze_module(&file_path).await
            });
            
            handles.push(handle);
        }
        
        // Wait for all analyses to complete
        let results: Vec<_> = futures::future::join_all(handles).await;
        
        // Verify all completed successfully
        assert_eq!(results.len(), 3);
        for (i, result) in results.iter().enumerate() {
            assert!(result.is_ok());
            let report = result.as_ref().unwrap().as_ref().unwrap();
            let tes_score = report.tes_score.calculate();
            assert!(tes_score > 0.0, "Report {} has zero TES score", i);
        }
        
        let _duration = start.elapsed().as_millis();
        assert!(_duration < 1000); // Allow time for concurrent execution
    }

    /// Test error handling for invalid inputs
    #[tokio::test]
    async fn test_error_handling_comprehensive() {
        let start = Instant::now();
        
        let analyzer = create_test_analyzer();
        
        // Test non-existent file
        let result = analyzer.analyze_module(&PathBuf::from("/non/existent/file.rs")).await;
        assert!(result.is_err());
        
        // Test invalid path
        let result = analyzer.analyze_module(&PathBuf::from("")).await;
        assert!(result.is_err());
        
        // Verify error types
        if let Err(e) = result {
            assert!(matches!(e, MutationError::IoError(_)));
        }
        
        let _duration = start.elapsed().as_millis();
        assert!(_duration < 100);
    }
}