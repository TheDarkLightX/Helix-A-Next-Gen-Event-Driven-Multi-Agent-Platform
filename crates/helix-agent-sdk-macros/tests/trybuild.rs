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