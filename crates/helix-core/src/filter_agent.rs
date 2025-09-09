use crate::agent::{Agent, AgentConfig, TransformerAgent, TransformerContext};
use crate::event::Event;
use crate::types::AgentId;
use crate::HelixError;
use async_trait::async_trait;

/// A deterministic transformer agent that forwards only events whose type matches a configured allowlist.
///
/// This demonstrates how event-processing agents can be composed in the Helix runtime.
pub struct TypeFilterAgent {
    config: AgentConfig,
    allowed_types: Vec<String>,
}

impl TypeFilterAgent {
    /// Create a new type-filtering agent with an allowlist of event types.
    pub fn new(config: AgentConfig, allowed_types: Vec<String>) -> Self {
        Self {
            config,
            allowed_types,
        }
    }
}

#[async_trait]
impl Agent for TypeFilterAgent {
    fn id(&self) -> AgentId {
        self.config.id
    }

    fn config(&self) -> &AgentConfig {
        &self.config
    }
}

#[async_trait]
impl TransformerAgent for TypeFilterAgent {
    async fn transform(
        &mut self,
        _ctx: TransformerContext,
        event: Event,
    ) -> Result<Vec<Event>, HelixError> {
        if self.allowed_types.iter().any(|t| t == &event.r#type) {
            Ok(vec![event])
        } else {
            Ok(vec![])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_transformer_context;
    use proptest::prelude::*;
    use serde_json::json;
    use tokio::runtime::Builder;
    use uuid::Uuid;

    #[tokio::test]
    async fn filter_agent_allows_only_matching_events() {
        // Given a filter agent configured to allow "foo.event"
        let id = Uuid::new_v4();
        let profile_id = Uuid::new_v4();
        let config = AgentConfig::new(id, profile_id, None, "filter".to_string(), json!({}));
        let mut agent = TypeFilterAgent::new(config, vec!["foo.event".to_string()]);

        // When events of different types are processed
        let ctx = || test_transformer_context(id, profile_id);
        let allowed_event = Event::new(id.to_string(), "foo.event".to_string(), None);
        let disallowed_event = Event::new(id.to_string(), "bar.event".to_string(), None);
        let allowed_output = agent.transform(ctx(), allowed_event).await.unwrap();
        let disallowed_output = agent.transform(ctx(), disallowed_event).await.unwrap();

        // Then only the allowed event is forwarded
        assert_eq!(allowed_output.len(), 1);
        assert_eq!(allowed_output[0].r#type, "foo.event");
        assert!(disallowed_output.is_empty());
    }

    proptest! {
        #[test]
        fn prop_filter_only_outputs_allowed(flags in proptest::collection::vec(proptest::bool::ANY, 0..20)) {
            let rt = Builder::new_current_thread()
                .enable_time()
                .build()
                .expect("runtime");
            let result = rt.block_on(async move {
                let id = Uuid::new_v4();
                let profile_id = Uuid::new_v4();
                let config = AgentConfig::new(id, profile_id, None, "filter".to_string(), json!({}));
                let mut agent = TypeFilterAgent::new(config, vec!["foo.event".to_string()]);
                let ctx = || test_transformer_context(id, profile_id);

                let mut outputs = Vec::new();
                for flag in &flags {
                    let etype = if *flag { "foo.event" } else { "other" };
                    let event = Event::new(id.to_string(), etype.to_string(), None);
                    outputs.extend(agent.transform(ctx(), event).await.unwrap());
                }

                let expected = flags.iter().filter(|b| **b).count();
                prop_assert_eq!(outputs.len(), expected);
                prop_assert!(outputs.iter().all(|e| e.r#type == "foo.event"));
                Ok(())
            });
            result.unwrap();
        }
    }
}
