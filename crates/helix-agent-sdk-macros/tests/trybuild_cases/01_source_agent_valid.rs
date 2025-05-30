use helix_agent_sdk::{AgentContext, SdkError, SourceSdkAgent}; // SdkAgent is brought in by SourceSdkAgent
use helix_agent_sdk_macros::source_agent;
use helix_core::agent::{Agent, AgentConfig};
use helix_core::types::{AgentId, RecipeId};
use serde_json::json;
use std::sync::Arc;

#[source_agent]
pub struct MyValidSourceAgent {
    pub agent_config: Arc<AgentConfig>,
    // Add other fields specific to your agent
}

impl MyValidSourceAgent {
    // Constructor or other helper methods
    pub fn new(id_str: &str) -> Self {
        Self {
            agent_config: Arc::new(AgentConfig {
                id: AgentId::new(id_str),
                name: "My Valid Source Agent".to_string(),
                class_name: "MyValidSourceAgent".to_string(),
                config: json!({}),
                recipe_id: RecipeId::new("test-recipe"),
                credentials: None,
            }),
        }
    }

    // The macro expects this inherent method
    pub async fn run(&mut self, _context: &AgentContext) -> Result<(), SdkError> {
        println!("MyValidSourceAgent running!");
        // In a real agent, you'd use context.emit_event(...)
        Ok(())
    }

    // Optional: User can still define their own init, start, stop if needed,
    // but the macro provides defaults. If defined, they would be called if
    // the macro was more advanced to detect and prefer user-defined ones.
    // For now, the macro's default SdkAgent impls are simple.
    // async fn init(&mut self, _context: &AgentContext) -> Result<(), SdkError> {
    //     println!("Custom init for MyValidSourceAgent");
    //     Ok(())
    // }
}

// Main function to satisfy the compiler for a binary crate,
// or this could be part of a lib crate test.
// For trybuild, it just needs to compile.
fn main() {
    let _agent = MyValidSourceAgent::new("test-source");
    // In a real test, you might try to call agent methods,
    // but for trybuild, successful compilation is the main goal.
    println!("MyValidSourceAgent compiles!");
}