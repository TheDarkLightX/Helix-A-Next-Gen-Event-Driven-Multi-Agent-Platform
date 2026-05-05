use helix_agent_sdk::{AgentContext, SdkError};
use helix_agent_sdk_macros::source_agent;
use helix_core::agent::AgentConfig;
use helix_core::credential::EnvCredentialProvider;
use helix_core::state::InMemoryStateStore;
use helix_runtime::agent_registry::AgentRegistry;
use helix_runtime::agent_runner::AgentRunner;
use helix_runtime::messaging::InMemoryEventCollector;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[source_agent]
pub struct DummySourceAgent {
    pub agent_config: Arc<AgentConfig>,
}

impl DummySourceAgent {
    pub async fn run(&mut self, _context: &AgentContext) -> Result<(), SdkError> {
        Ok(())
    }
}

#[tokio::test]
async fn registry_discovers_macro_registered_agent() {
    let registry = AgentRegistry::new().expect("registry");
    assert!(!registry.is_empty());

    let publisher: Arc<dyn helix_agent_sdk::EventPublisher> =
        Arc::new(InMemoryEventCollector::new());
    let creds: Arc<dyn helix_core::credential::CredentialProvider> =
        Arc::new(EnvCredentialProvider::default());
    let state: Arc<dyn helix_core::state::StateStore> = Arc::new(InMemoryStateStore::new());

    let mut runner = AgentRunner::new_native(Arc::new(registry), publisher, creds, state);

    let cfg = AgentConfig::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        None,
        "DummySourceAgent".to_string(),
        json!({}),
    );

    let id = runner.start_agent(cfg).await.expect("start");
    assert_eq!(
        runner.agent_status(&id),
        Some(helix_runtime::AgentStatus::Running)
    );
    runner.stop_agent(&id).await.expect("stop");
}
