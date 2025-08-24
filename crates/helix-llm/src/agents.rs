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

//! LLM-powered agent implementations

use crate::{
    context::AgentContext, errors::LlmError, LlmAgent, LlmAgentConfig, LlmProvider, LlmRequest,
    LlmResponse,
};
use async_trait::async_trait;
use helix_core::{
    agent::{
        ActionAgent, ActionContext, Agent, AgentConfig, SourceAgent, SourceContext,
        TransformerAgent, TransformerContext,
    },
    event::Event,
    types::AgentId,
    HelixError,
};
use std::sync::Arc;

/// An LLM-powered source agent that can generate events based on natural language instructions
pub struct LlmSourceAgent {
    config: AgentConfig,
    llm_config: LlmAgentConfig,
    provider: Arc<dyn LlmProvider>,
}

impl LlmSourceAgent {
    /// Create a new LLM source agent
    pub fn new(
        config: AgentConfig,
        llm_config: LlmAgentConfig,
        provider: Arc<dyn LlmProvider>,
    ) -> Self {
        Self {
            config,
            llm_config,
            provider,
        }
    }
}

#[async_trait]
impl Agent for LlmSourceAgent {
    fn id(&self) -> AgentId {
        self.config.id
    }

    fn config(&self) -> &AgentConfig {
        &self.config
    }
}

#[async_trait]
impl SourceAgent for LlmSourceAgent {
    async fn run(&mut self, ctx: SourceContext) -> Result<(), HelixError> {
        // Use LLM to determine what events to generate
        let request = LlmRequest {
            system_prompt: Some(self.llm_config.system_prompt.clone()),
            messages: vec![],
            max_tokens: Some(self.llm_config.max_tokens),
            temperature: Some(self.llm_config.temperature),
            top_p: None,
            functions: None,
            parameters: self.llm_config.parameters.clone(),
        };

        let response = self
            .provider
            .complete(request)
            .await
            .map_err(|e| HelixError::InternalError(format!("LLM error: {}", e)))?;

        // Parse LLM response and generate events
        // This is a simplified implementation
        if !response.content.is_empty() {
            let event_data = serde_json::json!({
                "llm_response": response.content,
                "model": response.model,
                "usage": response.usage
            });

            ctx.emit(event_data, Some("llm.generated".to_string()))
                .await?;
        }

        Ok(())
    }
}

#[async_trait]
impl LlmAgent for LlmSourceAgent {
    async fn process_natural_language(
        &mut self,
        input: &str,
        _context: &AgentContext,
    ) -> Result<LlmResponse, LlmError> {
        let request = LlmRequest {
            system_prompt: Some(self.llm_config.system_prompt.clone()),
            messages: vec![crate::providers::Message {
                role: crate::providers::MessageRole::User,
                content: input.to_string(),
                function_call: None,
            }],
            max_tokens: Some(self.llm_config.max_tokens),
            temperature: Some(self.llm_config.temperature),
            top_p: None,
            functions: None,
            parameters: self.llm_config.parameters.clone(),
        };

        self.provider.complete(request).await
    }

    async fn synthesize_recipe(
        &mut self,
        _description: &str,
        _context: &AgentContext,
    ) -> Result<helix_core::recipe::Recipe, LlmError> {
        // Recipe synthesis is not yet supported for the source agent
        Err(LlmError::ModelNotSupported("recipe synthesis".into()))
    }

    async fn analyze_event(
        &mut self,
        event: &Event,
        context: &AgentContext,
    ) -> Result<Vec<String>, LlmError> {
        // Use LLM to analyze event and suggest actions
        let event_json = serde_json::to_string_pretty(event)
            .map_err(|e| LlmError::ParsingError(e.to_string()))?;

        let prompt = format!(
            "Analyze this event and suggest appropriate actions:\n\n{}",
            event_json
        );

        let response = self.process_natural_language(&prompt, context).await?;

        // Parse response to extract action suggestions
        // This is simplified - would need better parsing
        Ok(vec![response.content])
    }
}

/// An LLM-powered transformer agent that can modify events using natural language processing
pub struct LlmTransformerAgent {
    config: AgentConfig,
    llm_config: LlmAgentConfig,
    provider: Arc<dyn LlmProvider>,
}

impl LlmTransformerAgent {
    /// Create a new LLM transformer agent
    pub fn new(
        config: AgentConfig,
        llm_config: LlmAgentConfig,
        provider: Arc<dyn LlmProvider>,
    ) -> Self {
        Self {
            config,
            llm_config,
            provider,
        }
    }
}

#[async_trait]
impl Agent for LlmTransformerAgent {
    fn id(&self) -> AgentId {
        self.config.id
    }

    fn config(&self) -> &AgentConfig {
        &self.config
    }
}

#[async_trait]
impl TransformerAgent for LlmTransformerAgent {
    async fn transform(
        &mut self,
        _ctx: TransformerContext,
        event: Event,
    ) -> Result<Vec<Event>, HelixError> {
        // Use LLM to transform the event
        let event_json = serde_json::to_string_pretty(&event)
            .map_err(|e| HelixError::InternalError(e.to_string()))?;

        let prompt = format!(
            "Transform this event according to the instructions: {}\n\nEvent: {}",
            self.llm_config.system_prompt, event_json
        );

        let request = LlmRequest {
            system_prompt: Some(
                "You are an event transformer. Return transformed events as JSON.".to_string(),
            ),
            messages: vec![crate::providers::Message {
                role: crate::providers::MessageRole::User,
                content: prompt,
                function_call: None,
            }],
            max_tokens: Some(self.llm_config.max_tokens),
            temperature: Some(self.llm_config.temperature),
            top_p: None,
            functions: None,
            parameters: self.llm_config.parameters.clone(),
        };

        let response = self
            .provider
            .complete(request)
            .await
            .map_err(|e| HelixError::InternalError(format!("LLM error: {}", e)))?;

        // Parse LLM response and create transformed events
        // This is simplified - would need better JSON parsing
        let mut transformed_event = event;
        if let Ok(new_data) = serde_json::from_str::<serde_json::Value>(&response.content) {
            transformed_event.data = Some(new_data);
        }

        Ok(vec![transformed_event])
    }
}

#[async_trait]
impl LlmAgent for LlmTransformerAgent {
    async fn process_natural_language(
        &mut self,
        input: &str,
        _context: &AgentContext,
    ) -> Result<LlmResponse, LlmError> {
        let request = LlmRequest {
            system_prompt: Some(self.llm_config.system_prompt.clone()),
            messages: vec![crate::providers::Message {
                role: crate::providers::MessageRole::User,
                content: input.to_string(),
                function_call: None,
            }],
            max_tokens: Some(self.llm_config.max_tokens),
            temperature: Some(self.llm_config.temperature),
            top_p: None,
            functions: None,
            parameters: self.llm_config.parameters.clone(),
        };

        self.provider.complete(request).await
    }

    async fn synthesize_recipe(
        &mut self,
        _description: &str,
        _context: &AgentContext,
    ) -> Result<helix_core::recipe::Recipe, LlmError> {
        Err(LlmError::ModelNotSupported("recipe synthesis".into()))
    }

    async fn analyze_event(
        &mut self,
        _event: &Event,
        _context: &AgentContext,
    ) -> Result<Vec<String>, LlmError> {
        Err(LlmError::ModelNotSupported("event analysis".into()))
    }
}

/// An LLM-powered action agent that can perform actions based on natural language instructions
pub struct LlmActionAgent {
    config: AgentConfig,
    llm_config: LlmAgentConfig,
    provider: Arc<dyn LlmProvider>,
}

impl LlmActionAgent {
    /// Create a new LLM action agent
    pub fn new(
        config: AgentConfig,
        llm_config: LlmAgentConfig,
        provider: Arc<dyn LlmProvider>,
    ) -> Self {
        Self {
            config,
            llm_config,
            provider,
        }
    }
}

#[async_trait]
impl Agent for LlmActionAgent {
    fn id(&self) -> AgentId {
        self.config.id
    }

    fn config(&self) -> &AgentConfig {
        &self.config
    }
}

#[async_trait]
impl ActionAgent for LlmActionAgent {
    async fn execute(&mut self, _ctx: ActionContext, event: Event) -> Result<(), HelixError> {
        // Use LLM to determine what action to take
        let event_json = serde_json::to_string_pretty(&event)
            .map_err(|e| HelixError::InternalError(e.to_string()))?;

        let prompt = format!(
            "Execute an action based on this event: {}\n\nEvent: {}",
            self.llm_config.system_prompt, event_json
        );

        let request = LlmRequest {
            system_prompt: Some(
                "You are an action executor. Determine and describe the action to take."
                    .to_string(),
            ),
            messages: vec![crate::providers::Message {
                role: crate::providers::MessageRole::User,
                content: prompt,
                function_call: None,
            }],
            max_tokens: Some(self.llm_config.max_tokens),
            temperature: Some(self.llm_config.temperature),
            top_p: None,
            functions: None,
            parameters: self.llm_config.parameters.clone(),
        };

        let response = self
            .provider
            .complete(request)
            .await
            .map_err(|e| HelixError::InternalError(format!("LLM error: {}", e)))?;

        // Log the action that would be taken
        tracing::info!(
            agent_id = %self.config.id,
            action = %response.content,
            "LLM action agent executed"
        );

        Ok(())
    }
}

#[async_trait]
impl LlmAgent for LlmActionAgent {
    async fn process_natural_language(
        &mut self,
        input: &str,
        _context: &AgentContext,
    ) -> Result<LlmResponse, LlmError> {
        let request = LlmRequest {
            system_prompt: Some(self.llm_config.system_prompt.clone()),
            messages: vec![crate::providers::Message {
                role: crate::providers::MessageRole::User,
                content: input.to_string(),
                function_call: None,
            }],
            max_tokens: Some(self.llm_config.max_tokens),
            temperature: Some(self.llm_config.temperature),
            top_p: None,
            functions: None,
            parameters: self.llm_config.parameters.clone(),
        };

        self.provider.complete(request).await
    }

    async fn synthesize_recipe(
        &mut self,
        _description: &str,
        _context: &AgentContext,
    ) -> Result<helix_core::recipe::Recipe, LlmError> {
        Err(LlmError::ModelNotSupported("recipe synthesis".into()))
    }

    async fn analyze_event(
        &mut self,
        _event: &Event,
        _context: &AgentContext,
    ) -> Result<Vec<String>, LlmError> {
        Err(LlmError::ModelNotSupported("event analysis".into()))
    }
}
