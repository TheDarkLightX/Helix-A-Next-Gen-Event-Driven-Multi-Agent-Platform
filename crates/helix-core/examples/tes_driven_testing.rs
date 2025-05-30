//! Example demonstrating Test Effectiveness Score (TES) driven testing
//! 
//! Run with: cargo run --example tes_driven_testing --features mutation-testing

#![cfg(feature = "mutation-testing")]

use helix_core::mutation_testing::{
    test_effectiveness::TestEffectivenessScore,
    TestResult,
};

fn main() {
    println!("Test Effectiveness Score (TES) Demonstration");
    println!("===========================================\n");
    
    // Simulate test results from a high-quality test suite
    let high_quality_results = vec![
        TestResult {
            name: "test_add_happy_path_comprehensive".to_string(),
            passed: true,
            error: None,
            duration: 45,
        },
        TestResult {
            name: "test_add_boundary_conditions".to_string(),
            passed: true,
            error: None,
            duration: 52,
        },
        TestResult {
            name: "test_add_error_overflow".to_string(),
            passed: false,
            error: Some("Mutation detected: overflow behavior changed".to_string()),
            duration: 38,
        },
        TestResult {
            name: "test_subtract_edge_cases".to_string(),
            passed: false,
            error: Some("Mutation detected: sign change".to_string()),
            duration: 41,
        },
        TestResult {
            name: "test_concurrent_operations".to_string(),
            passed: true,
            error: None,
            duration: 95,
        },
    ];
    
    // Calculate TES for high-quality suite
    let high_tes = TestEffectivenessScore::from_results(&high_quality_results, 92, 100);
    
    println!("High-Quality Test Suite:");
    println!("------------------------");
    println!("Mutation Score: {:.2} (Target: >0.85)", high_tes.mutation_score);
    println!("Assertion Density: {:.2} (Target: >3 per test)", high_tes.assertion_density);
    println!("Behavior Coverage: {:.2} (Target: >0.90)", high_tes.behavior_coverage);
    println!("Speed Factor: {:.2} (Target: >0.80)", high_tes.speed_factor);
    println!("Overall TES: {:.3}", high_tes.calculate());
    println!("Grade: {}\n", high_tes.grade());
    
    // Simulate results from a poor test suite
    let poor_results = vec![
        TestResult {
            name: "test_basic".to_string(),
            passed: true,
            error: None,
            duration: 250,
        },
        TestResult {
            name: "test_another".to_string(),
            passed: true,
            error: None,
            duration: 300,
        },
    ];
    
    let poor_tes = TestEffectivenessScore::from_results(&poor_results, 30, 100);
    
    println!("Poor Test Suite:");
    println!("----------------");
    println!("Mutation Score: {:.2} (Target: >0.85)", poor_tes.mutation_score);
    println!("Assertion Density: {:.2} (Target: >3 per test)", poor_tes.assertion_density);
    println!("Behavior Coverage: {:.2} (Target: >0.90)", poor_tes.behavior_coverage);
    println!("Speed Factor: {:.2} (Target: >0.80)", poor_tes.speed_factor);
    println!("Overall TES: {:.3}", poor_tes.calculate());
    println!("Grade: {}\n", poor_tes.grade());
    
    // Recommendations
    println!("TES-Driven Testing Best Practices:");
    println!("----------------------------------");
    println!("1. Write tests that kill mutations (high mutation score)");
    println!("2. Include multiple assertions per test (high assertion density)");
    println!("3. Cover all behavior patterns (high behavior coverage)");
    println!("4. Keep tests fast (high speed factor)");
    println!("5. Aim for grade A or higher (TES ≥ 0.8)");
    
    // Example of ideal test structure
    println!("\nIdeal Test Structure:");
    println!("--------------------");
    println!(r#"
#[test]
fn test_calculator_add_comprehensive() {
    // Arrange
    let calc = Calculator::new();
    
    // Act & Assert - Happy path
    assert_eq!(calc.add(2, 3), 5);
    assert_eq!(calc.add(0, 0), 0);
    assert_eq!(calc.add(-1, 1), 0);
    
    // Edge cases
    assert_eq!(calc.add(i32::MAX, 0), i32::MAX);
    assert_eq!(calc.add(i32::MIN, 0), i32::MIN);
    
    // Error conditions
    assert!(calc.add_checked(i32::MAX, 1).is_err());
    
    // Behavior verification
    assert!(calc.history().contains(&"add(2, 3) = 5"));
}
"#);
}