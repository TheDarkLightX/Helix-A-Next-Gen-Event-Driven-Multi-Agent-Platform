//! Example demonstrating Test Effectiveness Score (TES) driven development
//! 
//! This example shows how to write high-quality tests that focus on:
//! - High mutation score (catching bugs)
//! - High assertion density (meaningful checks)
//! - Comprehensive behavior coverage
//! - Fast execution

use std::time::Instant;

/// Example calculator to demonstrate TES-driven testing
#[derive(Debug, Clone)]
pub struct Calculator {
    history: Vec<String>,
}

impl Calculator {
    pub fn new() -> Self {
        Self { history: Vec::new() }
    }
    
    pub fn add(&mut self, a: i32, b: i32) -> Result<i32, String> {
        match a.checked_add(b) {
            Some(result) => {
                self.history.push(format!("add({}, {}) = {}", a, b, result));
                Ok(result)
            }
            None => Err("Overflow occurred".to_string())
        }
    }
    
    pub fn divide(&mut self, a: i32, b: i32) -> Result<i32, String> {
        if b == 0 {
            return Err("Division by zero".to_string());
        }
        let result = a / b;
        self.history.push(format!("divide({}, {}) = {}", a, b, result));
        Ok(result)
    }
    
    pub fn history(&self) -> &[String] {
        &self.history
    }
    
    pub fn clear_history(&mut self) {
        self.history.clear();
    }
}

/// TES-driven test example
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_calculator_add_comprehensive() {
        let start = Instant::now();
        let mut assertions = 0;
        let mut calc = Calculator::new();
        
        // Happy path - multiple assertions
        assert_eq!(calc.add(2, 3).unwrap(), 5); assertions += 1;
        assert_eq!(calc.add(0, 0).unwrap(), 0); assertions += 1;
        assert_eq!(calc.add(-5, 5).unwrap(), 0); assertions += 1;
        assert_eq!(calc.add(-10, -20).unwrap(), -30); assertions += 1;
        
        // Edge cases
        assert_eq!(calc.add(i32::MAX, 0).unwrap(), i32::MAX); assertions += 1;
        assert_eq!(calc.add(i32::MIN, 0).unwrap(), i32::MIN); assertions += 1;
        
        // Error conditions
        assert!(calc.add(i32::MAX, 1).is_err()); assertions += 1;
        assert!(calc.add(i32::MIN, -1).is_err()); assertions += 1;
        assert_eq!(calc.add(i32::MAX, 1).unwrap_err(), "Overflow occurred"); assertions += 1;
        
        // Behavior verification
        assert_eq!(calc.history().len(), 6); assertions += 1;
        assert!(calc.history()[0].contains("add(2, 3) = 5")); assertions += 1;
        
        let duration = start.elapsed();
        println!("Test: test_calculator_add_comprehensive");
        println!("Assertions: {} (density: {:.1})", assertions, assertions as f64 / 3.0);
        println!("Duration: {:?}", duration);
        println!("Behaviors tested: happy_path, edge_case, error, state_tracking");
    }
    
    #[test]
    fn test_calculator_divide_comprehensive() {
        let start = Instant::now();
        let mut assertions = 0;
        let mut calc = Calculator::new();
        
        // Happy path
        assert_eq!(calc.divide(10, 2).unwrap(), 5); assertions += 1;
        assert_eq!(calc.divide(15, 3).unwrap(), 5); assertions += 1;
        assert_eq!(calc.divide(-20, 4).unwrap(), -5); assertions += 1;
        assert_eq!(calc.divide(7, 2).unwrap(), 3); assertions += 1; // Integer division
        
        // Edge cases
        assert_eq!(calc.divide(0, 5).unwrap(), 0); assertions += 1;
        assert_eq!(calc.divide(1, 1).unwrap(), 1); assertions += 1;
        assert_eq!(calc.divide(-1, -1).unwrap(), 1); assertions += 1;
        
        // Error conditions
        assert!(calc.divide(5, 0).is_err()); assertions += 1;
        assert_eq!(calc.divide(5, 0).unwrap_err(), "Division by zero"); assertions += 1;
        
        // History verification
        assert_eq!(calc.history().len(), 6); assertions += 1;
        assert!(calc.history().iter().all(|h| h.contains("divide"))); assertions += 1;
        
        let duration = start.elapsed();
        println!("\nTest: test_calculator_divide_comprehensive");
        println!("Assertions: {} (density: {:.1})", assertions, assertions as f64 / 3.0);
        println!("Duration: {:?}", duration);
        println!("Behaviors tested: happy_path, edge_case, error, integer_division");
    }
    
    #[test]
    fn test_calculator_history_management() {
        let start = Instant::now();
        let mut assertions = 0;
        let mut calc = Calculator::new();
        
        // Initial state
        assert!(calc.history().is_empty()); assertions += 1;
        
        // Operations add to history
        calc.add(1, 1).unwrap();
        calc.divide(4, 2).unwrap();
        assert_eq!(calc.history().len(), 2); assertions += 1;
        assert!(calc.history()[0].starts_with("add")); assertions += 1;
        assert!(calc.history()[1].starts_with("divide")); assertions += 1;
        
        // Failed operations don't add to history
        let _ = calc.divide(1, 0);
        let _ = calc.add(i32::MAX, 1);
        assert_eq!(calc.history().len(), 2); assertions += 1;
        
        // Clear history
        calc.clear_history();
        assert!(calc.history().is_empty()); assertions += 1;
        
        let duration = start.elapsed();
        println!("\nTest: test_calculator_history_management");
        println!("Assertions: {} (density: {:.1})", assertions, assertions as f64 / 3.0);
        println!("Duration: {:?}", duration);
        println!("Behaviors tested: state_management, error_handling, clearing");
    }
}

fn main() {
    println!("TES-Driven Development Example");
    println!("==============================\n");
    
    println!("Test Effectiveness Score (TES) Formula:");
    println!("TES = Mutation Score × Assertion Density × Behavior Coverage × Speed Factor\n");
    
    println!("Target Metrics:");
    println!("- Mutation Score: >0.85 (tests catch 85%+ of mutations)");
    println!("- Assertion Density: >3 (at least 3 meaningful assertions per test)");
    println!("- Behavior Coverage: >0.90 (90%+ of user stories tested)");
    println!("- Speed Factor: >0.80 (tests complete quickly)\n");
    
    println!("Best Practices Demonstrated:");
    println!("1. Multiple assertions per test (high density)");
    println!("2. Testing happy path, edge cases, and errors");
    println!("3. Verifying state changes and side effects");
    println!("4. Fast execution (no unnecessary delays)");
    println!("5. Clear test names describing behavior\n");
    
    println!("Example Test Structure:");
    println!("- Arrange: Set up test data");
    println!("- Act: Execute the behavior");
    println!("- Assert: Multiple meaningful checks");
    println!("- Verify: State and side effects\n");
    
    // Demonstrate the calculator
    let mut calc = Calculator::new();
    
    println!("Calculator Demo:");
    match calc.add(10, 20) {
        Ok(result) => println!("10 + 20 = {}", result),
        Err(e) => println!("Error: {}", e),
    }
    
    match calc.divide(100, 5) {
        Ok(result) => println!("100 / 5 = {}", result),
        Err(e) => println!("Error: {}", e),
    }
    
    println!("\nHistory:");
    for entry in calc.history() {
        println!("  {}", entry);
    }
    
    println!("\nRun tests with: cargo test -- --nocapture");
}