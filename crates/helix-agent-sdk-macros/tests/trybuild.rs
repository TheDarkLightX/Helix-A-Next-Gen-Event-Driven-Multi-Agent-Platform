#[test]
fn trybuild_tests() {
    let t = trybuild::TestCases::new();
    // Test cases for successful compilation
    t.pass("tests/trybuild_cases/01_source_agent_valid.rs");
    t.pass("tests/trybuild_cases/02_action_agent_valid.rs");
    
    // Test cases for expected compilation failures (if any were designed)
    // e.g., t.compile_fail("tests/trybuild_cases/03_source_agent_missing_run.rs");
    // For now, we'll focus on pass cases as the macros are straightforward.
    // The compiler will naturally fail if the required inherent methods are missing.
}