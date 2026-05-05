use helix_agent_sdk::{AgentContext, SdkError};
use helix_agent_sdk_macros::source_agent;
use helix_core::agent::AgentConfig;
use std::sync::Arc;

#[source_agent]
pub struct MyValidSourceAgent {
    pub agent_config: Arc<AgentConfig>,
}

impl MyValidSourceAgent {
    pub async fn run(&mut self, _context: &AgentContext) -> Result<(), SdkError> {
        Ok(())
    }
}

fn main() {}
