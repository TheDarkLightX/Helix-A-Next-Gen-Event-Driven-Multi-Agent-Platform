//! Quality Analysis Tool for Helix Platform
//! 
//! This module provides comprehensive quality analysis using mutation testing
//! to identify weak spots and improve code quality systematically.

use crate::mutation_testing::{
    practical_analyzer::{PracticalMutationAnalyzer, AnalyzerConfig, QualityReport},
};
use crate::HelixError;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Comprehensive quality analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityAnalysisReport {
    /// Overall quality score (0.0 to 1.0)
    pub overall_score: f64,
    /// Module-specific reports
    pub module_reports: HashMap<String, QualityReport>,
    /// Critical issues requiring immediate attention
    pub critical_issues: Vec<CriticalIssue>,
    /// Recommended improvements prioritized by impact
    pub improvement_plan: Vec<ImprovementAction>,
    /// Quality trends over time
    pub quality_metrics: QualityMetrics,
}

/// Critical issue that needs immediate attention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalIssue {
    /// Module where the issue was found
    pub module: String,
    /// Severity level
    pub severity: IssueSeverity,
    /// Description of the issue
    pub description: String,
    /// Specific location in code
    pub location: CodeLocation,
    /// Recommended fix
    pub recommended_fix: String,
    /// Estimated effort to fix (hours)
    pub effort_estimate: u32,
}

/// Severity levels for issues
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum IssueSeverity {
    /// Security vulnerability or critical bug
    Critical,
    /// Major functionality issue
    High,
    /// Performance or maintainability issue
    Medium,
    /// Minor improvement opportunity
    Low,
}

/// Code location information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeLocation {
    /// File path
    pub file: PathBuf,
    /// Line number
    pub line: usize,
    /// Column number (optional)
    pub column: Option<usize>,
    /// Function or method name
    pub function: Option<String>,
}

/// Improvement action with priority and impact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementAction {
    /// Priority level
    pub priority: ActionPriority,
    /// Category of improvement
    pub category: ImprovementCategory,
    /// Description of the action
    pub description: String,
    /// Expected impact on quality score
    pub expected_impact: f64,
    /// Estimated effort (hours)
    pub effort_hours: u32,
    /// ROI (impact/effort ratio)
    pub roi: f64,
}

/// Priority levels for improvement actions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ActionPriority {
    /// Must be done immediately
    Immediate,
    /// Should be done this sprint
    High,
    /// Should be done this quarter
    Medium,
    /// Nice to have
    Low,
}

/// Categories of improvements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImprovementCategory {
    /// Add missing tests
    TestCoverage,
    /// Improve assertion quality
    TestQuality,
    /// Reduce code complexity
    Complexity,
    /// Fix security issues
    Security,
    /// Improve performance
    Performance,
    /// Enhance maintainability
    Maintainability,
    /// Remove code duplication
    Duplication,
}

/// Quality metrics tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    /// Test effectiveness score
    pub tes_score: f64,
    /// Mutation score (percentage of mutations killed)
    pub mutation_score: f64,
    /// Code coverage percentage
    pub code_coverage: f64,
    /// Average cyclomatic complexity
    pub avg_complexity: f64,
    /// Code duplication percentage
    pub duplication_ratio: f64,
    /// Number of security issues
    pub security_issues: u32,
    /// Performance score (0.0 to 1.0)
    pub performance_score: f64,
}

/// Main quality analyzer
pub struct QualityAnalyzer {
    /// Mutation analyzer
    analyzer: PracticalMutationAnalyzer,
    /// Configuration
    config: QualityAnalysisConfig,
}

/// Configuration for quality analysis
#[derive(Debug, Clone)]
pub struct QualityAnalysisConfig {
    /// Target directories to analyze
    pub target_dirs: Vec<PathBuf>,
    /// File patterns to include
    pub include_patterns: Vec<String>,
    /// File patterns to exclude
    pub exclude_patterns: Vec<String>,
    /// Minimum quality score threshold
    pub quality_threshold: f64,
    /// Maximum acceptable complexity
    pub max_complexity: f64,
    /// Minimum test coverage required
    pub min_coverage: f64,
}

impl Default for QualityAnalysisConfig {
    fn default() -> Self {
        Self {
            target_dirs: vec![PathBuf::from("src")],
            include_patterns: vec!["*.rs".to_string()],
            exclude_patterns: vec!["*/tests/*".to_string(), "*/target/*".to_string()],
            quality_threshold: 0.8,
            max_complexity: 10.0,
            min_coverage: 0.85,
        }
    }
}

impl QualityAnalyzer {
    /// Create a new quality analyzer
    pub fn new(config: QualityAnalysisConfig) -> Result<Self, HelixError> {
        let analyzer_config = AnalyzerConfig {
            target_dir: config.target_dirs.first()
                .unwrap_or(&PathBuf::from("."))
                .clone(),
            test_command: "cargo test".to_string(),
            complexity_threshold: config.max_complexity,
            min_assertion_density: 3.0,
            max_test_duration_ms: 5000,
        };
        
        let analyzer = PracticalMutationAnalyzer::new(analyzer_config);
        
        Ok(Self {
            analyzer,
            config,
        })
    }
    
    /// Run comprehensive quality analysis
    pub async fn analyze_quality(&self) -> Result<QualityAnalysisReport, HelixError> {
        let mut module_reports = HashMap::new();
        let mut critical_issues = Vec::new();
        
        // Analyze each target directory
        for target_dir in &self.config.target_dirs {
            let modules = self.discover_modules(target_dir)?;
            
            for module_path in modules {
                let report = self.analyzer.analyze_module(&module_path).await
                    .map_err(|e| HelixError::InternalError(format!("Analysis failed: {}", e)))?;
                
                // Extract critical issues
                critical_issues.extend(self.extract_critical_issues(&module_path, &report));
                
                let module_name = module_path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                
                module_reports.insert(module_name, report);
            }
        }
        
        // Calculate overall metrics
        let quality_metrics = self.calculate_overall_metrics(&module_reports);
        let overall_score = self.calculate_overall_score(&quality_metrics);
        
        // Generate improvement plan
        let improvement_plan = self.generate_improvement_plan(&module_reports, &critical_issues);
        
        Ok(QualityAnalysisReport {
            overall_score,
            module_reports,
            critical_issues,
            improvement_plan,
            quality_metrics,
        })
    }
    
    /// Discover Rust modules in a directory
    fn discover_modules(&self, dir: &Path) -> Result<Vec<PathBuf>, HelixError> {
        let mut modules = Vec::new();
        
        if !dir.exists() {
            return Ok(modules);
        }
        
        for entry in std::fs::read_dir(dir)
            .map_err(|e| HelixError::InternalError(format!("Failed to read directory: {}", e)))? 
        {
            let entry = entry
                .map_err(|e| HelixError::InternalError(format!("Failed to read entry: {}", e)))?;
            let path = entry.path();
            
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
                // Skip test files and generated files
                if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                    if !file_name.contains("test") && !file_name.starts_with("mod.rs") {
                        modules.push(path);
                    }
                }
            } else if path.is_dir() && !self.should_exclude_dir(&path) {
                modules.extend(self.discover_modules(&path)?);
            }
        }
        
        Ok(modules)
    }
    
    /// Check if directory should be excluded
    fn should_exclude_dir(&self, path: &Path) -> bool {
        if let Some(dir_name) = path.file_name().and_then(|s| s.to_str()) {
            matches!(dir_name, "target" | "tests" | ".git" | "node_modules")
        } else {
            false
        }
    }

    /// Extract critical issues from a quality report
    fn extract_critical_issues(&self, module_path: &Path, report: &QualityReport) -> Vec<CriticalIssue> {
        let mut issues = Vec::new();

        // Check for critical weak spots
        for weak_spot in &report.weak_spots {
            if matches!(weak_spot.severity, crate::mutation_testing::practical_analyzer::Severity::Critical) {
                issues.push(CriticalIssue {
                    module: module_path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    severity: IssueSeverity::Critical,
                    description: format!("Critical weak spot: {}", weak_spot.reason),
                    location: CodeLocation {
                        file: weak_spot.file.clone(),
                        line: weak_spot.line,
                        column: None,
                        function: None,
                    },
                    recommended_fix: "Add comprehensive tests for this code path".to_string(),
                    effort_estimate: 4,
                });
            }
        }

        // Check for low TES score
        let tes_score = report.tes_score.calculate();
        if tes_score < 0.5 {
            issues.push(CriticalIssue {
                module: module_path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
                severity: IssueSeverity::High,
                description: format!("Low Test Effectiveness Score: {:.2}", tes_score),
                location: CodeLocation {
                    file: module_path.to_path_buf(),
                    line: 1,
                    column: None,
                    function: None,
                },
                recommended_fix: "Improve test quality and coverage".to_string(),
                effort_estimate: 8,
            });
        }

        // Check for high complexity
        if report.metrics.cyclomatic_complexity > self.config.max_complexity {
            issues.push(CriticalIssue {
                module: module_path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
                severity: IssueSeverity::Medium,
                description: format!("High cyclomatic complexity: {:.1}", report.metrics.cyclomatic_complexity),
                location: CodeLocation {
                    file: module_path.to_path_buf(),
                    line: 1,
                    column: None,
                    function: None,
                },
                recommended_fix: "Refactor complex functions into smaller, focused methods".to_string(),
                effort_estimate: 6,
            });
        }

        issues
    }

    /// Calculate overall quality metrics
    fn calculate_overall_metrics(&self, reports: &HashMap<String, QualityReport>) -> QualityMetrics {
        if reports.is_empty() {
            return QualityMetrics {
                tes_score: 0.0,
                mutation_score: 0.0,
                code_coverage: 0.0,
                avg_complexity: 0.0,
                duplication_ratio: 0.0,
                security_issues: 0,
                performance_score: 0.0,
            };
        }

        let count = reports.len() as f64;

        let tes_score = reports.values()
            .map(|r| r.tes_score.calculate())
            .sum::<f64>() / count;

        let mutation_score = reports.values()
            .map(|r| r.tes_score.mutation_score)
            .sum::<f64>() / count;

        let avg_complexity = reports.values()
            .map(|r| r.metrics.cyclomatic_complexity)
            .sum::<f64>() / count;

        let duplication_ratio = reports.values()
            .map(|r| r.metrics.duplication_ratio)
            .sum::<f64>() / count;

        let security_issues = reports.values()
            .flat_map(|r| &r.weak_spots)
            .filter(|w| matches!(w.severity, crate::mutation_testing::practical_analyzer::Severity::Critical))
            .count() as u32;

        // Estimate code coverage from assertion density
        let code_coverage = reports.values()
            .map(|r| (r.metrics.assertion_density / 5.0).min(1.0))
            .sum::<f64>() / count;

        // Calculate performance score based on speed factor
        let performance_score = reports.values()
            .map(|r| r.tes_score.speed_factor)
            .sum::<f64>() / count;

        QualityMetrics {
            tes_score,
            mutation_score,
            code_coverage,
            avg_complexity,
            duplication_ratio,
            security_issues,
            performance_score,
        }
    }

    /// Calculate overall quality score
    fn calculate_overall_score(&self, metrics: &QualityMetrics) -> f64 {
        // Weighted average of different quality factors
        let weights = [
            (metrics.tes_score, 0.25),           // Test effectiveness
            (metrics.mutation_score, 0.20),      // Mutation testing
            (metrics.code_coverage, 0.15),       // Code coverage
            (1.0 - (metrics.avg_complexity / 20.0).min(1.0), 0.15), // Complexity (inverted)
            (1.0 - metrics.duplication_ratio, 0.10), // Duplication (inverted)
            (if metrics.security_issues == 0 { 1.0 } else { 0.5 }, 0.10), // Security
            (metrics.performance_score, 0.05),   // Performance
        ];

        weights.iter()
            .map(|(score, weight)| score * weight)
            .sum()
    }

    /// Generate prioritized improvement plan
    fn generate_improvement_plan(
        &self,
        reports: &HashMap<String, QualityReport>,
        critical_issues: &[CriticalIssue]
    ) -> Vec<ImprovementAction> {
        let mut actions = Vec::new();

        // Add actions for critical issues
        for issue in critical_issues {
            let priority = match issue.severity {
                IssueSeverity::Critical => ActionPriority::Immediate,
                IssueSeverity::High => ActionPriority::High,
                IssueSeverity::Medium => ActionPriority::Medium,
                IssueSeverity::Low => ActionPriority::Low,
            };

            let expected_impact = match issue.severity {
                IssueSeverity::Critical => 0.15,
                IssueSeverity::High => 0.10,
                IssueSeverity::Medium => 0.05,
                IssueSeverity::Low => 0.02,
            };

            actions.push(ImprovementAction {
                priority,
                category: self.categorize_issue(issue),
                description: issue.recommended_fix.clone(),
                expected_impact,
                effort_hours: issue.effort_estimate,
                roi: expected_impact / (issue.effort_estimate as f64 / 8.0), // Assuming 8-hour workday
            });
        }

        // Add general improvement actions based on reports
        for (module_name, report) in reports {
            // Low mutation score
            if report.tes_score.mutation_score < 0.8 {
                actions.push(ImprovementAction {
                    priority: ActionPriority::High,
                    category: ImprovementCategory::TestCoverage,
                    description: format!("Improve test coverage in {}", module_name),
                    expected_impact: 0.08,
                    effort_hours: 6,
                    roi: 0.08 / 0.75,
                });
            }

            // Low assertion density
            if report.metrics.assertion_density < 3.0 {
                actions.push(ImprovementAction {
                    priority: ActionPriority::Medium,
                    category: ImprovementCategory::TestQuality,
                    description: format!("Add more assertions to tests in {}", module_name),
                    expected_impact: 0.05,
                    effort_hours: 4,
                    roi: 0.05 / 0.5,
                });
            }
        }

        // Sort by ROI (highest first)
        actions.sort_by(|a, b| b.roi.partial_cmp(&a.roi).unwrap_or(std::cmp::Ordering::Equal));

        actions
    }

    /// Categorize an issue into improvement category
    fn categorize_issue(&self, issue: &CriticalIssue) -> ImprovementCategory {
        if issue.description.contains("weak spot") {
            ImprovementCategory::TestCoverage
        } else if issue.description.contains("TES") || issue.description.contains("Test Effectiveness") {
            ImprovementCategory::TestQuality
        } else if issue.description.contains("complexity") {
            ImprovementCategory::Complexity
        } else if issue.description.contains("security") {
            ImprovementCategory::Security
        } else {
            ImprovementCategory::Maintainability
        }
    }
}
