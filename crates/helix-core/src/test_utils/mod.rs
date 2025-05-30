//! Test Effectiveness Score (TES) utilities for high-quality testing
//! 
//! TES = Mutation Score × Assertion Density × Behavior Coverage × Speed Factor

use std::time::Instant;

/// Test Effectiveness Score calculator and tracker
#[derive(Debug, Clone)]
pub struct TesTracker {
    test_name: String,
    assertions: Vec<Assertion>,
    behaviors: Vec<BehaviorTest>,
    start_time: Instant,
    mutations_caught: usize,
    total_mutations: usize,
}

/// Represents a single assertion with context
#[derive(Debug, Clone)]
pub struct Assertion {
    pub description: String,
    pub passed: bool,
    pub actual: String,
    pub expected: String,
}

/// Represents a behavior test scenario
#[derive(Debug, Clone)]
pub struct BehaviorTest {
    pub scenario: String,
    pub given: String,
    pub when: String,
    pub then: String,
    pub passed: bool,
}

impl TesTracker {
    /// Create a new TES tracker for a test
    pub fn new(test_name: impl Into<String>) -> Self {
        Self {
            test_name: test_name.into(),
            assertions: Vec::new(),
            behaviors: Vec::new(),
            start_time: Instant::now(),
            mutations_caught: 0,
            total_mutations: 0,
        }
    }
    
    /// Record an assertion
    pub fn assert_eq<T: std::fmt::Debug>(&mut self, actual: T, expected: T, description: &str) -> bool {
        let passed = format!("{:?}", actual) == format!("{:?}", expected);
        self.assertions.push(Assertion {
            description: description.to_string(),
            passed,
            actual: format!("{:?}", actual),
            expected: format!("{:?}", expected),
        });
        passed
    }
    
    /// Record a behavior test
    pub fn test_behavior(&mut self, behavior: BehaviorTest) {
        self.behaviors.push(behavior);
    }
    
    /// Record mutation testing results
    pub fn record_mutations(&mut self, caught: usize, total: usize) {
        self.mutations_caught = caught;
        self.total_mutations = total;
    }
    
    /// Calculate the Test Effectiveness Score
    pub fn calculate_tes(&self) -> TesScore {
        let duration = self.start_time.elapsed();
        
        // Mutation Score
        let mutation_score = if self.total_mutations > 0 {
            (self.mutations_caught as f64 / self.total_mutations as f64).min(1.0)
        } else {
            0.85 // Default if no mutations tested
        };
        
        // Assertion Density (normalized by target of 3)
        let assertion_density = (self.assertions.len() as f64 / 3.0).min(1.0);
        
        // Behavior Coverage
        let behavior_patterns = ["happy_path", "error", "edge_case", "boundary", "concurrent"];
        let covered = behavior_patterns.iter()
            .filter(|p| self.behaviors.iter().any(|b| b.scenario.contains(*p)))
            .count();
        let behavior_coverage = (covered as f64 / behavior_patterns.len() as f64).min(1.0);
        
        // Speed Factor
        let avg_ms = duration.as_millis() as f64;
        let speed_factor = 1.0 / (1.0 + avg_ms / 100.0);
        
        TesScore {
            mutation_score,
            assertion_density,
            behavior_coverage,
            speed_factor,
            overall: mutation_score * assertion_density * behavior_coverage * speed_factor,
        }
    }
    
    /// Generate a test report
    pub fn report(&self) -> String {
        let score = self.calculate_tes();
        format!(
            "Test: {}\n\
             TES Score: {:.3} ({})\n\
             - Mutation Score: {:.2}\n\
             - Assertion Density: {:.2} ({} assertions)\n\
             - Behavior Coverage: {:.2}\n\
             - Speed Factor: {:.2} ({:.0}ms)\n\
             Failed Assertions: {}\n",
            self.test_name,
            score.overall,
            score.grade(),
            score.mutation_score,
            score.assertion_density,
            self.assertions.len(),
            score.behavior_coverage,
            score.speed_factor,
            self.start_time.elapsed().as_millis(),
            self.assertions.iter().filter(|a| !a.passed).count()
        )
    }
}

/// Test Effectiveness Score components
#[derive(Debug, Clone)]
pub struct TesScore {
    pub mutation_score: f64,
    pub assertion_density: f64,
    pub behavior_coverage: f64,
    pub speed_factor: f64,
    pub overall: f64,
}

impl TesScore {
    /// Get letter grade
    pub fn grade(&self) -> &'static str {
        match self.overall {
            s if s >= 0.9 => "A+",
            s if s >= 0.8 => "A",
            s if s >= 0.7 => "B",
            s if s >= 0.6 => "C",
            _ => "F",
        }
    }
}

/// Macro for TES-aware assertions
#[macro_export]
macro_rules! tes_assert_eq {
    ($tracker:expr, $actual:expr, $expected:expr, $desc:expr) => {
        assert!($tracker.assert_eq($actual, $expected, $desc), 
                "Assertion failed: {}", $desc);
    };
}

/// Macro for behavior-driven tests
#[macro_export]
macro_rules! tes_behavior {
    ($tracker:expr, $scenario:expr, given: $given:expr, when: $when:expr, then: $then:expr, $test:expr) => {{
        let passed = $test;
        $tracker.test_behavior($crate::test_utils::BehaviorTest {
            scenario: $scenario.to_string(),
            given: $given.to_string(),
            when: $when.to_string(),
            then: $then.to_string(),
            passed,
        });
        assert!(passed, "Behavior test failed: {}", $scenario);
    }};
}

/// Test data builder for reducing boilerplate
pub struct TestDataBuilder<T> {
    data: T,
}

impl<T> TestDataBuilder<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
    
    pub fn with<F>(mut self, f: F) -> Self 
    where 
        F: FnOnce(&mut T)
    {
        f(&mut self.data);
        self
    }
    
    pub fn build(self) -> T {
        self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tes_tracker_comprehensive() {
        let mut tracker = TesTracker::new("test_tes_tracker");
        
        // Multiple assertions
        tes_assert_eq!(tracker, 1 + 1, 2, "Basic addition");
        tes_assert_eq!(tracker, "hello".len(), 5, "String length");
        tes_assert_eq!(tracker, vec![1, 2, 3].len(), 3, "Vector length");
        tes_assert_eq!(tracker, true, true, "Boolean equality");
        
        // Behavior tests
        tes_behavior!(tracker, "happy_path",
            given: "A valid input",
            when: "Processing occurs",
            then: "Expected output is produced",
            true
        );
        
        tes_behavior!(tracker, "edge_case",
            given: "Empty input",
            when: "Processing occurs",
            then: "Handles gracefully",
            true
        );
        
        // Mutation testing simulation
        tracker.record_mutations(9, 10);
        
        // Check TES score
        let score = tracker.calculate_tes();
        assert!(score.mutation_score >= 0.85);
        assert!(score.assertion_density >= 1.0);
        assert!(score.behavior_coverage >= 0.4);
        assert!(score.speed_factor >= 0.8);
        
        println!("{}", tracker.report());
    }
}