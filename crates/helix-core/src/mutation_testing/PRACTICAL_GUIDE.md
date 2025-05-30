# Practical Mutation Testing Guide for Helix Platform

## Overview

The Helix mutation testing framework provides a practical, production-ready tool for improving code quality through Test Effectiveness Score (TES) analysis. This guide shows how to use it effectively in your development workflow.

## Quick Start

### 1. Analyze a Single Module

```bash
cargo run --example mutation_quality_tool --features mutation-testing -- analyze src/agent.rs
```

### 2. Scan Entire Crate

```bash
cargo run --example mutation_quality_tool --features mutation-testing -- scan --min-score 0.7
```

### 3. Continuous Monitoring

```bash
cargo run --example mutation_quality_tool --features mutation-testing -- watch src/agent.rs --interval 5
```

## Understanding TES (Test Effectiveness Score)

TES = Mutation Score × Assertion Density × Behavior Coverage × Speed Factor

- **Grade A+ (≥0.9)**: Exceptional test quality
- **Grade A (≥0.8)**: High quality tests
- **Grade B (≥0.7)**: Good tests with room for improvement
- **Grade C (≥0.6)**: Adequate but needs work
- **Grade F (<0.6)**: Poor test quality

## Key Features

### 1. Actionable Recommendations

The tool provides specific, prioritized recommendations:

- **🚨 Immediate**: Critical issues requiring immediate attention
- **⚡ High**: Important improvements for code quality
- **📌 Medium**: Beneficial enhancements
- **💭 Low**: Nice-to-have optimizations

### 2. Weak Spot Detection

Identifies code areas where mutations survive:

- **🔴 Critical**: Multiple mutation types survive (5+)
- **🟠 High**: Several mutations survive (3-4)
- **🟡 Medium**: Some mutations survive (2)
- **🟢 Low**: Few mutations survive (1)

### 3. Code Quality Metrics

- **Cyclomatic Complexity**: Code complexity measure
- **Test Ratio**: Proportion of test code to production code
- **Assertion Density**: Average assertions per test
- **Duplication Ratio**: Amount of duplicated code

## Integration with CI/CD

### GitHub Actions Example

```yaml
name: Mutation Testing
on: [push, pull_request]

jobs:
  mutation-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      
      - name: Run Mutation Analysis
        run: |
          cargo run --example mutation_quality_tool --features mutation-testing -- \
            scan --min-score 0.7 --format json > mutation-report.json
      
      - name: Check Quality Gate
        run: |
          score=$(jq '.tes_score.calculate()' mutation-report.json)
          if (( $(echo "$score < 0.7" | bc -l) )); then
            echo "Quality gate failed: TES score $score < 0.7"
            exit 1
          fi
```

### Pre-commit Hook

```bash
#!/bin/bash
# .git/hooks/pre-commit

# Run mutation testing on changed files
for file in $(git diff --cached --name-only | grep '\.rs$'); do
    cargo run --example mutation_quality_tool --features mutation-testing -- \
        analyze "$file" --format json | jq -e '.tes_score.calculate() >= 0.7'
    
    if [ $? -ne 0 ]; then
        echo "❌ $file does not meet quality standards"
        exit 1
    fi
done
```

## Best Practices

### 1. Start with Critical Weak Spots

Focus on files with critical severity weak spots first:

```bash
cargo run --example mutation_quality_tool --features mutation-testing -- scan | grep "Critical"
```

### 2. Improve Assertion Density

Add meaningful assertions to existing tests:

```rust
// Before
#[test]
fn test_add() {
    let result = add(2, 3);
    assert_eq!(result, 5);
}

// After - Higher assertion density
#[test]
fn test_add_comprehensive() {
    // Happy path
    assert_eq!(add(2, 3), 5);
    assert_eq!(add(0, 0), 0);
    
    // Edge cases
    assert_eq!(add(i32::MAX, 0), i32::MAX);
    assert_eq!(add(i32::MIN, 0), i32::MIN);
    
    // Properties
    assert_eq!(add(a, b), add(b, a)); // Commutative
}
```

### 3. Add Edge Case Tests

Target surviving mutations with specific edge cases:

```rust
// If boolean mutations survive
#[test]
fn test_edge_case_boundaries() {
    assert!(!is_valid(-1));  // Below minimum
    assert!(is_valid(0));    // Boundary
    assert!(is_valid(100));  // Within range
    assert!(!is_valid(101)); // Above maximum
}
```

### 4. Reduce Complexity

Refactor complex functions identified by the tool:

```rust
// Before - High complexity
fn process(data: &Data) -> Result<Output> {
    if data.is_valid() {
        if data.type == Type::A {
            // ... complex logic
        } else if data.type == Type::B {
            // ... more logic
        }
    }
}

// After - Lower complexity
fn process(data: &Data) -> Result<Output> {
    validate_data(data)?;
    match data.type {
        Type::A => process_type_a(data),
        Type::B => process_type_b(data),
    }
}
```

## Performance Optimization

### 1. Parallel Testing

Configure parallel test execution:

```toml
# .cargo/config.toml
[build]
jobs = 8

[test]
threads = 4
```

### 2. Incremental Analysis

Use watch mode during development:

```bash
cargo run --example mutation_quality_tool --features mutation-testing -- \
    watch src/my_module.rs --interval 3
```

### 3. Targeted Mutation Testing

Focus on changed files only:

```bash
git diff --name-only | grep '\.rs$' | xargs -I {} \
    cargo run --example mutation_quality_tool --features mutation-testing -- analyze {}
```

## Troubleshooting

### Common Issues

1. **Low Mutation Score**
   - Add more specific assertions
   - Test edge cases and error conditions
   - Verify all code paths are tested

2. **Low Speed Factor**
   - Optimize slow tests
   - Use test fixtures
   - Mock external dependencies

3. **High Complexity**
   - Extract methods
   - Use pattern matching
   - Apply SOLID principles

## Advanced Usage

### Custom Configuration

Create a `.helix-mutation.toml`:

```toml
[analyzer]
complexity_threshold = 8.0
min_assertion_density = 4.0
max_test_duration_ms = 50

[mutations]
max_mutations_per_file = 20
timeout_seconds = 30
```

### Programmatic API

```rust
use helix_core::mutation_testing::practical_analyzer::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AnalyzerConfig {
        target_dir: PathBuf::from("src"),
        test_command: "cargo test --lib".to_string(),
        complexity_threshold: 10.0,
        min_assertion_density: 3.0,
        max_test_duration_ms: 100,
    };
    
    let analyzer = PracticalMutationAnalyzer::new(config);
    let report = analyzer.analyze_module(Path::new("src/lib.rs")).await?;
    
    println!("TES Score: {:.2} ({})", 
        report.tes_score.calculate(), 
        report.tes_score.grade()
    );
    
    Ok(())
}
```

## Conclusion

The Helix mutation testing framework helps maintain high code quality through continuous analysis and actionable feedback. By focusing on TES scores and following the recommendations, teams can build more reliable and maintainable software.

Remember: **Coverage without quality = false confidence**. Focus on meaningful tests that catch real bugs.