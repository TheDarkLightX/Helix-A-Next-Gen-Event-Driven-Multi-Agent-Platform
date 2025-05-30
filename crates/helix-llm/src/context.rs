//! Context management for LLM interactions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use helix_core::{event::Event, types::{AgentId, ProfileId}};

/// Context for LLM agent operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContext {
    /// The agent ID
    pub agent_id: AgentId,
    /// The profile ID
    pub profile_id: ProfileId,
    /// Recent events for context
    pub recent_events: Vec<Event>,
    /// Agent state variables
    pub state: HashMap<String, serde_json::Value>,
    /// Available tools/functions
    pub available_tools: Vec<ToolDefinition>,
    /// Conversation history
    pub conversation: ConversationContext,
    /// Metadata about the current session
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Conversation context for maintaining dialogue state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationContext {
    /// Conversation ID
    pub id: String,
    /// Messages in the conversation
    pub messages: Vec<ConversationMessage>,
    /// Maximum number of messages to keep
    pub max_messages: usize,
    /// Total token count estimate
    pub token_count: usize,
    /// Conversation metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// A message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    /// Message ID
    pub id: String,
    /// Role of the sender
    pub role: MessageRole,
    /// Message content
    pub content: String,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Token count for this message
    pub token_count: usize,
    /// Message metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Role of a message sender
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System message
    System,
    /// User message
    User,
    /// Assistant message
    Assistant,
    /// Tool/function result
    Tool,
}

/// Definition of a tool/function available to the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// Input schema (JSON Schema)
    pub input_schema: serde_json::Value,
    /// Output schema (JSON Schema)
    pub output_schema: serde_json::Value,
    /// Whether the tool is enabled
    pub enabled: bool,
}

impl AgentContext {
    /// Create a new agent context
    pub fn new(agent_id: AgentId, profile_id: ProfileId) -> Self {
        Self {
            agent_id,
            profile_id,
            recent_events: Vec::new(),
            state: HashMap::new(),
            available_tools: Vec::new(),
            conversation: ConversationContext::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add an event to the context
    pub fn add_event(&mut self, event: Event) {
        self.recent_events.push(event);
        
        // Keep only the last 10 events to prevent context bloat
        if self.recent_events.len() > 10 {
            self.recent_events.remove(0);
        }
    }

    /// Update agent state
    pub fn update_state(&mut self, key: String, value: serde_json::Value) {
        self.state.insert(key, value);
    }

    /// Get agent state value
    pub fn get_state(&self, key: &str) -> Option<&serde_json::Value> {
        self.state.get(key)
    }

    /// Add a tool to the available tools
    pub fn add_tool(&mut self, tool: ToolDefinition) {
        self.available_tools.push(tool);
    }

    /// Get enabled tools
    pub fn get_enabled_tools(&self) -> Vec<&ToolDefinition> {
        self.available_tools.iter().filter(|t| t.enabled).collect()
    }
}

impl ConversationContext {
    /// Create a new conversation context
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            messages: Vec::new(),
            max_messages: 50,
            token_count: 0,
            metadata: HashMap::new(),
        }
    }

    /// Add a message to the conversation
    pub fn add_message(&mut self, role: MessageRole, content: String) -> String {
        let message_id = uuid::Uuid::new_v4().to_string();
        let token_count = estimate_token_count(&content);
        
        let message = ConversationMessage {
            id: message_id.clone(),
            role,
            content,
            timestamp: chrono::Utc::now(),
            token_count,
            metadata: HashMap::new(),
        };

        self.messages.push(message);
        self.token_count += token_count;

        // Trim old messages if we exceed the limit
        while self.messages.len() > self.max_messages {
            let removed = self.messages.remove(0);
            self.token_count = self.token_count.saturating_sub(removed.token_count);
        }

        message_id
    }

    /// Get the last N messages
    pub fn get_recent_messages(&self, count: usize) -> &[ConversationMessage] {
        let start = self.messages.len().saturating_sub(count);
        &self.messages[start..]
    }

    /// Clear the conversation
    pub fn clear(&mut self) {
        self.messages.clear();
        self.token_count = 0;
    }

    /// Get conversation summary for context
    pub fn get_summary(&self) -> String {
        if self.messages.is_empty() {
            return "No conversation history".to_string();
        }

        let recent = self.get_recent_messages(5);
        let mut summary = String::new();
        
        for msg in recent {
            summary.push_str(&format!("{:?}: {}\n", msg.role, 
                if msg.content.len() > 100 {
                    format!("{}...", &msg.content[..100])
                } else {
                    msg.content.clone()
                }
            ));
        }

        summary
    }
}

impl Default for ConversationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Estimate token count for a text string (rough approximation)
fn estimate_token_count(text: &str) -> usize {
    // Rough approximation: 1 token ≈ 4 characters for English text
    (text.len() + 3) / 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_context_creation() {
        let agent_id = uuid::Uuid::new_v4();
        let profile_id = uuid::Uuid::new_v4();
        let context = AgentContext::new(agent_id, profile_id);

        assert_eq!(context.agent_id, agent_id);
        assert_eq!(context.profile_id, profile_id);
        assert!(context.recent_events.is_empty());
        assert!(context.state.is_empty());
    }

    #[test]
    fn test_conversation_context() {
        let mut conv = ConversationContext::new();
        
        let msg_id = conv.add_message(MessageRole::User, "Hello".to_string());
        assert!(!msg_id.is_empty());
        assert_eq!(conv.messages.len(), 1);
        assert!(conv.token_count > 0);
    }

    #[test]
    fn test_token_estimation() {
        assert_eq!(estimate_token_count("hello"), 2); // 5 chars -> (5+3)/4 = 2
        assert_eq!(estimate_token_count("hello world"), 3); // 11 chars -> (11+3)/4 = 3
        assert_eq!(estimate_token_count(""), 0);
        assert_eq!(estimate_token_count("a"), 1); // 1 char -> (1+3)/4 = 1
    }

    #[test]
    fn test_conversation_trimming() {
        let mut conv = ConversationContext::new();
        conv.max_messages = 3;
        
        // Add more messages than the limit
        for i in 0..5 {
            conv.add_message(MessageRole::User, format!("Message {}", i));
        }
        
        assert_eq!(conv.messages.len(), 3);
        // Should keep the last 3 messages
        assert_eq!(conv.messages[0].content, "Message 2");
        assert_eq!(conv.messages[2].content, "Message 4");
    }
}
