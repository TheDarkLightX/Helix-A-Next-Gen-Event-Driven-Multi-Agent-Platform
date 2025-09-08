use crate::agent::{Agent, AgentConfig, TransformerAgent, TransformerContext};
use crate::event::Event;
use crate::types::AgentId;
use crate::HelixError;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

/// Trait representing a generic language model capable of completing prompts.
#[async_trait]
pub trait LanguageModel: Send + Sync {
    /// Produce a completion for the given prompt.
    async fn complete(&self, prompt: &str) -> Result<String, HelixError>;
}

/// A transformer agent that converts `llm.prompt` events into `llm.completion` events
/// using an injected [`LanguageModel`].
pub struct LlmCompletionAgent {
    config: AgentConfig,
    model: Arc<dyn LanguageModel>,
}

impl LlmCompletionAgent {
    /// Create a new agent backed by the provided language model.
    pub fn new(config: AgentConfig, model: Arc<dyn LanguageModel>) -> Self {
        Self { config, model }
    }
}

#[async_trait]
impl Agent for LlmCompletionAgent {
    fn id(&self) -> AgentId {
        self.config.id
    }

    fn config(&self) -> &AgentConfig {
        &self.config
    }
}

#[async_trait]
impl TransformerAgent for LlmCompletionAgent {
    async fn transform(
        &mut self,
        ctx: TransformerContext,
        event: Event,
    ) -> Result<Vec<Event>, HelixError> {
        if event.r#type != "llm.prompt" {
            // Pass through events we don't handle
            return Ok(vec![event]);
        }

        let prompt = event
            .data
            .as_ref()
            .and_then(|d| d.get("prompt"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| HelixError::validation_error("data.prompt", "missing prompt"))?;

        let completion = self.model.complete(prompt).await?;
        let out = Event::new(
            ctx.agent_id.to_string(),
            "llm.completion".to_string(),
            Some(json!({
                "prompt": prompt,
                "completion": completion,
            })),
        );
        Ok(vec![out])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_transformer_context;
    use proptest::prelude::*;
    use std::sync::Arc;
    use tokio::runtime::Builder;
    use uuid::Uuid;

    struct EchoModel;

    #[async_trait]
    impl LanguageModel for EchoModel {
        async fn complete(&self, prompt: &str) -> Result<String, HelixError> {
            Ok(format!("{}!", prompt))
        }
    }

    #[tokio::test]
    async fn llm_agent_transforms_prompt_event() {
        // Given an LLM agent backed by an echo model
        let id = Uuid::new_v4();
        let profile_id = Uuid::new_v4();
        let config = AgentConfig::new(id, profile_id, None, "llm".to_string(), json!({}));
        let model = Arc::new(EchoModel);
        let mut agent = LlmCompletionAgent::new(config, model);
        let ctx = test_transformer_context(id, profile_id);
        let event = Event::new(
            id.to_string(),
            "llm.prompt".to_string(),
            Some(json!({"prompt":"hi"})),
        );

        // When the event is transformed
        let out = agent.transform(ctx, event).await.unwrap();

        // Then we receive a completion event with echoed text
        assert_eq!(out.len(), 1);
        let evt = &out[0];
        assert_eq!(evt.r#type, "llm.completion");
        let data = evt.data.as_ref().unwrap();
        assert_eq!(data.get("prompt").unwrap(), "hi");
        assert_eq!(data.get("completion").unwrap(), "hi!");
    }

    proptest! {
        #[test]
        fn prop_llm_agent_emits_completion(s in "[a-zA-Z0-9 ]{0,20}") {
            let rt = Builder::new_current_thread()
                .enable_time()
                .build()
                .expect("runtime");
            let result = rt.block_on(async move {
                let id = Uuid::new_v4();
                let profile_id = Uuid::new_v4();
                let config = AgentConfig::new(id, profile_id, None, "llm".to_string(), json!({}));
                let model = Arc::new(EchoModel);
                let mut agent = LlmCompletionAgent::new(config, model);
                let ctx = test_transformer_context(id, profile_id);
                let event = Event::new(id.to_string(), "llm.prompt".to_string(), Some(json!({"prompt": s.clone()})));

                let out = agent.transform(ctx, event).await.unwrap();
                prop_assert_eq!(out.len(), 1);
                let evt = &out[0];
                prop_assert_eq!(&evt.r#type, "llm.completion");
                let data = evt.data.as_ref().unwrap();
                prop_assert_eq!(data.get("prompt").unwrap().as_str().unwrap(), s.as_str());
                prop_assert_eq!(data.get("completion").unwrap().as_str().unwrap(), format!("{}!", s));
                Ok(())
            });
            result.unwrap();
        }
    }
}
