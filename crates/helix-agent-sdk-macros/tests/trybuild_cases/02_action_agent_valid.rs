use helix_agent_sdk::{AgentContext, SdkError};
use helix_agent_sdk_macros::action_agent;
use helix_core::agent::AgentConfig;
use helix_core::event::Event as HelixEvent;
use std::sync::Arc;

#[action_agent]
pub struct MyValidActionAgent {
    pub agent_config: Arc<AgentConfig>,
}

impl MyValidActionAgent {
    pub async fn execute(
        &mut self,
        _context: &AgentContext,
        _event: HelixEvent,
    ) -> Result<(), SdkError> {
        Ok(())
    }
}

fn main() {}
