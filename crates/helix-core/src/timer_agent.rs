use crate::agent::{Agent, AgentConfig, SourceAgent, SourceContext};
use crate::types::AgentId;
use crate::HelixError;
use async_trait::async_trait;
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

/// A simple deterministic source agent that emits an event after a fixed interval.
///
/// This demonstrates how non-LLM agents can participate in the Helix runtime.
pub struct TimerSourceAgent {
    config: AgentConfig,
    interval: Duration,
}

impl TimerSourceAgent {
    /// Create a new timer agent that waits `interval` before emitting an event.
    pub fn new(config: AgentConfig, interval: Duration) -> Self {
        Self { config, interval }
    }
}

#[async_trait]
impl Agent for TimerSourceAgent {
    fn id(&self) -> AgentId {
        self.config.id
    }

    fn config(&self) -> &AgentConfig {
        &self.config
    }
}

#[async_trait]
impl SourceAgent for TimerSourceAgent {
    async fn run(&mut self, ctx: SourceContext) -> Result<(), HelixError> {
        sleep(self.interval).await;
        let payload = json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        ctx.emit(payload, Some("timer.tick".to_string())).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{CredentialProvider, StateStore};
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    struct NoopCredentialProvider;

    #[async_trait]
    impl CredentialProvider for NoopCredentialProvider {
        async fn get_credential(&self, _id: &str) -> Result<Option<String>, HelixError> {
            Ok(None)
        }
    }

    #[derive(Default)]
    struct MemoryStateStore(std::sync::Mutex<HashMap<String, Vec<u8>>>);

    #[async_trait]
    impl StateStore for MemoryStateStore {
        async fn get_state(&self, key: &str) -> Result<Option<Vec<u8>>, HelixError> {
            Ok(self.0.lock().unwrap().get(key).cloned())
        }

        async fn set_state(&self, key: &str, value: &[u8]) -> Result<(), HelixError> {
            self.0
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_vec());
            Ok(())
        }
    }

    #[tokio::test]
    async fn timer_agent_emits_event() {
        let id = Uuid::new_v4();
        let profile_id = Uuid::new_v4();
        let config = AgentConfig::new(id, profile_id, None, "timer".to_string(), json!({}));
        let (tx, mut rx) = mpsc::channel(1);
        let ctx = SourceContext {
            agent_id: id,
            profile_id: profile_id,
            credential_provider: Arc::new(NoopCredentialProvider),
            state_store: Arc::new(MemoryStateStore::default()),
            event_tx: tx,
        };
        let mut agent = TimerSourceAgent::new(config, Duration::from_millis(10));
        agent.run(ctx).await.unwrap();
        let event = rx.recv().await.expect("event");
        assert_eq!(event.r#type, "timer.tick");
        assert!(event.data.is_some());
    }
}
