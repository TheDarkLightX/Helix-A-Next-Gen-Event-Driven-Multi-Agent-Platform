//! Quality Assessment Demo
//! 
//! This example demonstrates the Helix Mutation Testing Framework's quality assessment capabilities.

use helix_core::mutation_testing::{quick_assessment, generate_quality_report};

fn main() {
    println!("🎯 Helix Core Quality Assessment Demo");
    println!("=====================================\n");

    // Quick assessment
    println!("📊 QUICK ASSESSMENT:");
    println!("{}\n", quick_assessment());

    // Full quality report
    println!("📈 COMPREHENSIVE REPORT:");
    println!("{}", generate_quality_report());
}
