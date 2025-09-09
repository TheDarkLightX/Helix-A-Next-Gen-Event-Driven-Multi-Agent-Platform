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
    use crate::test_utils::test_source_context;
    use proptest::prelude::*;
    use serde_json::json;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    #[tokio::test(start_paused = true)]
    async fn timer_agent_emits_event() {
        // Given a timer agent configured with a 10ms interval
        let id = Uuid::new_v4();
        let profile_id = Uuid::new_v4();
        let config = AgentConfig::new(id, profile_id, None, "timer".to_string(), json!({}));
        let (tx, mut rx) = mpsc::channel(1);
        let ctx = test_source_context(id, profile_id, tx);

        // When the agent is run and time advances past the interval
        let mut agent = TimerSourceAgent::new(config, Duration::from_millis(10));
        let handle = tokio::spawn(async move { agent.run(ctx).await.unwrap() });
        tokio::time::advance(Duration::from_millis(10)).await;

        // Then a timer.tick event is emitted with data
        let event = rx.recv().await.expect("event");
        assert_eq!(event.r#type, "timer.tick");
        assert!(event.data.is_some());
        handle.await.unwrap();
    }

    proptest! {
        # [test]
        fn prop_timer_emits_event(ms in 1u64..100) {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .expect("runtime");
            rt.block_on(async move {
                // Given a timer agent configured with a random interval
                tokio::time::pause();
                let id = Uuid::new_v4();
                let profile_id = Uuid::new_v4();
                let config = AgentConfig::new(id, profile_id, None, "timer".to_string(), json!({}));
                let (tx, mut rx) = mpsc::channel(1);
                let ctx = test_source_context(id, profile_id, tx);
                let mut agent = TimerSourceAgent::new(config, Duration::from_millis(ms));
                let handle = tokio::spawn(async move { agent.run(ctx).await.unwrap() });

                // When time advances to the interval
                tokio::time::advance(Duration::from_millis(ms)).await;

                // Then an event is produced
                let event = rx.recv().await.expect("event");
                assert_eq!(event.r#type, "timer.tick");
                assert!(event.data.is_some());
                handle.await.unwrap();
            });
        }
    }
}
