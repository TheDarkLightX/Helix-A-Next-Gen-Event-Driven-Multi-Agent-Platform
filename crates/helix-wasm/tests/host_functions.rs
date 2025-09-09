use std::sync::{Arc, Mutex};

use helix_agent_sdk::EventPublisher;
use helix_core::agent::AgentConfig;
use helix_core::state::InMemoryStateStore;
use helix_core::test_utils::NoopCredentialProvider;
use helix_core::types::AgentId;
use helix_wasm::{WasmRuntime, WasmRuntimeConfig};
use serde_json::json;
use uuid::Uuid;

#[derive(Default)]
struct RecordingPublisher {
    events: Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl EventPublisher for RecordingPublisher {
    async fn publish_event(
        &self,
        _agent_id: &AgentId,
        _payload: serde_json::Value,
        event_type_override: Option<String>,
    ) -> Result<(), helix_agent_sdk::SdkError> {
        self.events
            .lock()
            .unwrap()
            .push(event_type_override.unwrap_or_default());
        Ok(())
    }
}

#[tokio::test]
async fn helix_get_time_returns_timestamp() {
    let runtime = WasmRuntime::new(WasmRuntimeConfig::default()).unwrap();

    let wat = r#"(module
        (import "env" "helix_get_time" (func $ht (result i64)))
        (func (export "call") (result i64)
            call $ht))"#;
    let bytes = wat::parse_str(wat).unwrap();
    let module = runtime.load_module_from_bytes(&bytes).await.unwrap();

    let agent_config = Arc::new(AgentConfig::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        None,
        "test".into(),
        json!({}),
    ));

    let publisher = Arc::new(RecordingPublisher::default());
    let credential_provider = Arc::new(NoopCredentialProvider);
    let state_store = Arc::new(InMemoryStateStore::default());

    let instance_id = runtime
        .instantiate_module(
            &module,
            agent_config,
            publisher,
            credential_provider,
            state_store,
        )
        .await
        .unwrap();

    let result = runtime
        .call_function_on_instance(instance_id, "call", &[])
        .await
        .unwrap();

    assert!(result.result.as_u64().unwrap() > 0);
}
