// Copyright 2026 DarkLightX
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

//! Quality Analysis CLI Tool
//!
//! This tool runs comprehensive quality analysis on the Helix platform
//! using mutation testing to identify weak spots and improvement opportunities.

use helix_core::quality_analysis::{QualityAnalysisConfig, QualityAnalyzer};
use std::env;
use std::path::PathBuf;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    let target_dir = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("src")
    };

    println!("🔍 Helix Quality Analyzer");
    println!("========================");
    println!("Analyzing: {}", target_dir.display());
    println!();

    // Configure quality analysis
    let config = QualityAnalysisConfig {
        target_dirs: vec![target_dir],
        include_patterns: vec!["*.rs".to_string()],
        exclude_patterns: vec![
            "*/tests/*".to_string(),
            "*/target/*".to_string(),
            "*/test_*".to_string(),
            "*_test.rs".to_string(),
        ],
        quality_threshold: 0.8,
        max_complexity: 10.0,
        min_coverage: 0.85,
    };

    // Create analyzer
    let analyzer = QualityAnalyzer::new(config)?;

    // Run analysis
    println!("🧪 Running mutation testing analysis...");
    let report = analyzer.analyze_quality().await?;

    // Display results
    display_quality_report(&report);

    Ok(())
}

fn display_quality_report(report: &helix_core::quality_analysis::QualityAnalysisReport) {
    println!("\n📊 QUALITY ANALYSIS RESULTS");
    println!("============================");

    // Overall score
    let score_emoji = if report.overall_score >= 0.9 {
        "🟢"
    } else if report.overall_score >= 0.7 {
        "🟡"
    } else {
        "🔴"
    };

    println!(
        "{} Overall Quality Score: {:.1}%",
        score_emoji,
        report.overall_score * 100.0
    );
    println!();

    // Quality metrics
    println!("📈 QUALITY METRICS");
    println!("------------------");
    println!(
        "🎯 Test Effectiveness Score: {:.1}%",
        report.quality_metrics.tes_score * 100.0
    );
    println!(
        "🧬 Mutation Score: {:.1}%",
        report.quality_metrics.mutation_score * 100.0
    );
    println!(
        "📋 Code Coverage: {:.1}%",
        report.quality_metrics.code_coverage * 100.0
    );
    println!(
        "🔄 Avg Complexity: {:.1}",
        report.quality_metrics.avg_complexity
    );
    println!(
        "📄 Duplication: {:.1}%",
        report.quality_metrics.duplication_ratio * 100.0
    );
    println!(
        "🔒 Security Issues: {}",
        report.quality_metrics.security_issues
    );
    println!(
        "⚡ Performance Score: {:.1}%",
        report.quality_metrics.performance_score * 100.0
    );
    println!();

    // Critical issues
    if !report.critical_issues.is_empty() {
        println!(
            "🚨 CRITICAL ISSUES ({} found)",
            report.critical_issues.len()
        );
        println!("------------------");

        for (i, issue) in report.critical_issues.iter().take(5).enumerate() {
            let severity_emoji = match issue.severity {
                helix_core::quality_analysis::IssueSeverity::Critical => "🔴",
                helix_core::quality_analysis::IssueSeverity::High => "🟠",
                helix_core::quality_analysis::IssueSeverity::Medium => "🟡",
                helix_core::quality_analysis::IssueSeverity::Low => "🟢",
            };

            println!(
                "{}. {} [{}] {}",
                i + 1,
                severity_emoji,
                issue.module,
                issue.description
            );
            println!("   💡 Fix: {}", issue.recommended_fix);
            println!("   ⏱️  Effort: {} hours", issue.effort_estimate);
            println!();
        }

        if report.critical_issues.len() > 5 {
            println!(
                "   ... and {} more issues",
                report.critical_issues.len() - 5
            );
            println!();
        }
    }

    // Improvement plan
    if !report.improvement_plan.is_empty() {
        println!("🎯 IMPROVEMENT PLAN (Top 5 by ROI)");
        println!("-----------------------------------");

        for (i, action) in report.improvement_plan.iter().take(5).enumerate() {
            let priority_emoji = match action.priority {
                helix_core::quality_analysis::ActionPriority::Immediate => "🔥",
                helix_core::quality_analysis::ActionPriority::High => "⚡",
                helix_core::quality_analysis::ActionPriority::Medium => "📋",
                helix_core::quality_analysis::ActionPriority::Low => "💡",
            };

            let category_emoji = match action.category {
                helix_core::quality_analysis::ImprovementCategory::TestCoverage => "🧪",
                helix_core::quality_analysis::ImprovementCategory::TestQuality => "✅",
                helix_core::quality_analysis::ImprovementCategory::Complexity => "🔄",
                helix_core::quality_analysis::ImprovementCategory::Security => "🔒",
                helix_core::quality_analysis::ImprovementCategory::Performance => "⚡",
                helix_core::quality_analysis::ImprovementCategory::Maintainability => "🛠️",
                helix_core::quality_analysis::ImprovementCategory::Duplication => "📄",
            };

            println!(
                "{}. {} {} {}",
                i + 1,
                priority_emoji,
                category_emoji,
                action.description
            );
            println!(
                "   📈 Impact: +{:.1}% quality",
                action.expected_impact * 100.0
            );
            println!("   ⏱️  Effort: {} hours", action.effort_hours);
            println!("   💰 ROI: {:.2}", action.roi);
            println!();
        }
    }

    // Module breakdown
    if !report.module_reports.is_empty() {
        println!("📁 MODULE BREAKDOWN");
        println!("-------------------");

        let mut modules: Vec<_> = report.module_reports.iter().collect();
        modules.sort_by(|a, b| {
            let score_a = a.1.tes_score.calculate();
            let score_b = b.1.tes_score.calculate();
            score_a
                .partial_cmp(&score_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for (module_name, module_report) in modules.iter().take(10) {
            let tes_score = module_report.tes_score.calculate();
            let score_emoji = if tes_score >= 0.8 {
                "🟢"
            } else if tes_score >= 0.6 {
                "🟡"
            } else {
                "🔴"
            };

            println!(
                "{} {} - TES: {:.1}%, Complexity: {:.1}, Weak Spots: {}",
                score_emoji,
                module_name,
                tes_score * 100.0,
                module_report.metrics.cyclomatic_complexity,
                module_report.weak_spots.len()
            );
        }

        if report.module_reports.len() > 10 {
            println!(
                "   ... and {} more modules",
                report.module_reports.len() - 10
            );
        }
    }

    println!();
    println!("✨ Analysis complete! Use the improvement plan to enhance code quality.");
}
