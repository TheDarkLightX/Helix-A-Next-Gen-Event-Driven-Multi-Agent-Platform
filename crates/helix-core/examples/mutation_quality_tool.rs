//! Practical mutation testing tool for improving Helix platform code quality
//! 
//! Run with: cargo run --example mutation_quality_tool --features mutation-testing -- [module_path]

#![cfg(feature = "mutation-testing")]

use helix_core::mutation_testing::{
    practical_analyzer::{PracticalMutationAnalyzer, AnalyzerConfig, Severity},
    MutationError,
};
use std::path::PathBuf;
use clap::{Parser, Subcommand};
use colored::*;

#[derive(Parser)]
#[command(name = "helix-quality")]
#[command(about = "Mutation testing tool for improving code quality", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze a specific module or file
    Analyze {
        /// Path to the module or file to analyze
        path: PathBuf,
        
        /// Custom test command (default: cargo test)
        #[arg(short, long)]
        test_cmd: Option<String>,
        
        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    
    /// Scan entire crate for quality issues
    Scan {
        /// Target directory (default: current directory)
        #[arg(short, long)]
        target: Option<PathBuf>,
        
        /// Minimum TES score threshold (0.0-1.0)
        #[arg(short, long, default_value = "0.7")]
        min_score: f64,
    },
    
    /// Watch for changes and run continuous analysis
    Watch {
        /// Path to watch
        path: PathBuf,
        
        /// Check interval in seconds
        #[arg(short, long, default_value = "5")]
        interval: u64,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Analyze { path, test_cmd, format } => {
            analyze_module(path, test_cmd, format).await?;
        }
        Commands::Scan { target, min_score } => {
            scan_crate(target, min_score).await?;
        }
        Commands::Watch { path, interval } => {
            watch_continuous(path, interval).await?;
        }
    }
    
    Ok(())
}

async fn analyze_module(
    path: PathBuf,
    test_cmd: Option<String>,
    format: String,
) -> Result<(), MutationError> {
    println!("{}", "🧬 Helix Mutation Quality Analyzer".bold().cyan());
    println!("{}", "=================================".cyan());
    
    // Configure analyzer
    let mut config = AnalyzerConfig::default();
    if let Some(cmd) = test_cmd {
        config.test_command = cmd;
    }
    
    let analyzer = PracticalMutationAnalyzer::new(config);
    
    // Run analysis
    println!("\n{} {}", "Analyzing:".bold(), path.display());
    let report = analyzer.analyze_module(&path).await?;
    
    // Display results
    match format.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        _ => {
            display_text_report(&report);
        }
    }
    
    Ok(())
}

async fn scan_crate(
    target: Option<PathBuf>,
    min_score: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let target_dir = target.unwrap_or_else(|| PathBuf::from("."));
    
    println!("{}", "🔍 Scanning crate for quality issues...".bold().cyan());
    println!("{}", "=====================================".cyan());
    
    let config = AnalyzerConfig {
        target_dir: target_dir.clone(),
        ..Default::default()
    };
    
    let analyzer = PracticalMutationAnalyzer::new(config);
    let mut total_files = 0;
    let mut below_threshold = 0;
    
    // Find all Rust source files
    let entries = walkdir::WalkDir::new(&target_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension()
                .map(|ext| ext == "rs")
                .unwrap_or(false)
        });
    
    for entry in entries {
        if entry.path().to_string_lossy().contains("target/") {
            continue; // Skip build artifacts
        }
        
        total_files += 1;
        
        match analyzer.analyze_module(entry.path()).await {
            Ok(report) => {
                let score = report.tes_score.calculate();
                if score < min_score {
                    below_threshold += 1;
                    println!(
                        "\n{} {} (TES: {:.2})",
                        "⚠️ ".yellow(),
                        entry.path().display(),
                        score
                    );
                    
                    // Show critical issues
                    for weak_spot in report.weak_spots.iter()
                        .filter(|w| w.severity == Severity::Critical)
                        .take(3)
                    {
                        println!(
                            "   {} Line {}: {}",
                            "└─".dimmed(),
                            weak_spot.line,
                            weak_spot.reason.dimmed()
                        );
                    }
                }
            }
            Err(e) => {
                eprintln!("Error analyzing {}: {}", entry.path().display(), e);
            }
        }
    }
    
    // Summary
    println!("\n{}", "Summary".bold().green());
    println!("{}", "-------".green());
    println!("Total files scanned: {}", total_files);
    println!("Files below threshold ({:.1}): {}", min_score, below_threshold);
    
    if below_threshold > 0 {
        println!(
            "\n{} Run 'helix-quality analyze <file>' for detailed recommendations",
            "💡".yellow()
        );
    } else {
        println!("\n{} All files meet quality standards!", "✅".green());
    }
    
    Ok(())
}

async fn watch_continuous(
    path: PathBuf,
    interval: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "👁️  Watching for changes...".bold().cyan());
    println!("Press Ctrl+C to stop\n");
    
    let config = AnalyzerConfig::default();
    let analyzer = PracticalMutationAnalyzer::new(config);
    
    loop {
        // Check if file has been modified
        let metadata = tokio::fs::metadata(&path).await?;
        let modified = metadata.modified()?;
        
        // Run analysis
        match analyzer.analyze_module(&path).await {
            Ok(report) => {
                // Clear screen
                print!("\x1B[2J\x1B[1;1H");
                
                println!("{} {}", "Analyzing:".bold(), path.display());
                println!("Last modified: {:?}", modified);
                println!();
                
                display_text_report(&report);
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
        
        // Wait for next check
        tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
    }
}

fn display_text_report(report: &helix_core::mutation_testing::practical_analyzer::QualityReport) {
    // TES Score with color coding
    let score = report.tes_score.calculate();
    let grade = report.tes_score.grade();
    let score_color = match grade {
        "A+" | "A" => "green",
        "B" => "yellow",
        _ => "red",
    };
    
    println!("\n{}", "📊 Test Effectiveness Score".bold());
    println!("{}", "-------------------------".dimmed());
    
    let score_display = format!("{:.2} ({})", score, grade);
    match score_color {
        "green" => println!("Overall: {}", score_display.green().bold()),
        "yellow" => println!("Overall: {}", score_display.yellow().bold()),
        _ => println!("Overall: {}", score_display.red().bold()),
    }
    
    // Component scores
    println!("├─ Mutation Score: {:.1}%", report.tes_score.mutation_score * 100.0);
    println!("├─ Assertion Density: {:.1}", report.tes_score.assertion_density * 3.0);
    println!("├─ Behavior Coverage: {:.1}%", report.tes_score.behavior_coverage * 100.0);
    println!("└─ Speed Factor: {:.2}", report.tes_score.speed_factor);
    
    // Code metrics
    println!("\n{}", "📈 Code Metrics".bold());
    println!("{}", "-------------".dimmed());
    println!("├─ Complexity: {:.1}", report.metrics.cyclomatic_complexity);
    println!("├─ Test Ratio: {:.1}%", report.metrics.test_ratio * 100.0);
    println!("├─ Assertions/Test: {:.1}", report.metrics.assertion_density);
    println!("└─ Duplication: {:.1}%", report.metrics.duplication_ratio * 100.0);
    
    // Weak spots
    if !report.weak_spots.is_empty() {
        println!("\n{}", "⚠️  Weak Spots".bold().yellow());
        println!("{}", "------------".dimmed());
        
        for (i, weak_spot) in report.weak_spots.iter().take(5).enumerate() {
            let severity_icon = match weak_spot.severity {
                Severity::Critical => "🔴",
                Severity::High => "🟠",
                Severity::Medium => "🟡",
                Severity::Low => "🟢",
            };
            
            println!(
                "{} Line {}: {} ({})",
                severity_icon,
                weak_spot.line,
                weak_spot.reason,
                format!("{:?}", weak_spot.severity).dimmed()
            );
            
            if i == 0 {
                // Show mutation types for the first weak spot
                let mutations = weak_spot.surviving_mutations.iter()
                    .map(|m| format!("{:?}", m))
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("   {} Mutations: {}", "└─".dimmed(), mutations.dimmed());
            }
        }
        
        if report.weak_spots.len() > 5 {
            println!("   ... and {} more", report.weak_spots.len() - 5);
        }
    }
    
    // Recommendations
    if !report.recommendations.is_empty() {
        println!("\n{}", "💡 Recommendations".bold().green());
        println!("{}", "----------------".dimmed());
        
        for (i, rec) in report.recommendations.iter().take(3).enumerate() {
            let priority_icon = match rec.priority {
                helix_core::mutation_testing::practical_analyzer::Priority::Immediate => "🚨",
                helix_core::mutation_testing::practical_analyzer::Priority::High => "⚡",
                helix_core::mutation_testing::practical_analyzer::Priority::Medium => "📌",
                helix_core::mutation_testing::practical_analyzer::Priority::Low => "💭",
            };
            
            println!("\n{} {}", priority_icon, rec.description.bold());
            
            if let Some(example) = &rec.example {
                println!("   {}", "Example:".dimmed());
                println!("   {}", example.cyan());
            }
        }
    }
    
    // Action items
    println!("\n{}", "🎯 Next Steps".bold());
    println!("{}", "-----------".dimmed());
    
    if score < 0.6 {
        println!("1. {} - Your tests need significant improvement", "Critical".red().bold());
        println!("2. Focus on adding assertions to existing tests");
        println!("3. Add edge case tests for critical weak spots");
    } else if score < 0.8 {
        println!("1. {} - Room for improvement", "Important".yellow().bold());
        println!("2. Increase assertion density in tests");
        println!("3. Cover more behavior scenarios");
    } else {
        println!("1. {} - Good test quality!", "Great".green().bold());
        println!("2. Consider performance optimizations");
        println!("3. Maintain current standards");
    }
}