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
