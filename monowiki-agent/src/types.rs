//! Core types for the agent system.

use serde::{Deserialize, Serialize};

/// Role of a message in the conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// A tool call requested by the assistant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for this tool call.
    pub id: String,
    /// Type of tool call (always "function" for now).
    #[serde(rename = "type")]
    pub call_type: String,
    /// The function being called.
    pub function: FunctionCall,
}

/// A function call within a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// Name of the function to call.
    pub name: String,
    /// JSON-encoded arguments.
    pub arguments: String,
}

/// Content block in an Anthropic-style message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

/// A message in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender.
    pub role: Role,
    /// Content of the message (for simple text messages).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Content blocks (for Anthropic-style messages).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_blocks: Option<Vec<ContentBlock>>,
    /// Tool calls made by the assistant (OpenAI-style).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Tool call ID this message is responding to (for tool role).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    /// Create a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: Some(content.into()),
            content_blocks: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: Some(content.into()),
            content_blocks: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: Some(content.into()),
            content_blocks: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create an assistant message with tool calls.
    pub fn assistant_with_tools(content: Option<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            content,
            content_blocks: None,
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }

    /// Create a tool result message.
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: Some(content.into()),
            content_blocks: None,
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }

    /// Get the text content of this message.
    pub fn text(&self) -> Option<&str> {
        if let Some(ref content) = self.content {
            return Some(content);
        }
        if let Some(ref blocks) = self.content_blocks {
            for block in blocks {
                if let ContentBlock::Text { text } = block {
                    return Some(text);
                }
            }
        }
        None
    }

    /// Estimate token count (rough: ~4 chars per token).
    pub fn estimate_tokens(&self) -> usize {
        let char_count = self.content.as_ref().map(|s| s.len()).unwrap_or(0)
            + self
                .content_blocks
                .as_ref()
                .map(|blocks| {
                    blocks
                        .iter()
                        .map(|b| match b {
                            ContentBlock::Text { text } => text.len(),
                            ContentBlock::ToolUse { input, .. } => input.to_string().len(),
                            ContentBlock::ToolResult { content, .. } => content.len(),
                        })
                        .sum()
                })
                .unwrap_or(0)
            + self
                .tool_calls
                .as_ref()
                .map(|calls| calls.iter().map(|c| c.function.arguments.len()).sum())
                .unwrap_or(0);
        char_count / 4
    }
}

/// Definition of a tool that can be called by the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Name of the tool.
    pub name: String,
    /// Description of what the tool does.
    pub description: String,
    /// JSON Schema for the tool's parameters.
    pub parameters: serde_json::Value,
}

impl ToolDefinition {
    /// Create a new tool definition.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

/// Configuration for the agent.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// API key for the LLM provider.
    pub api_key: String,
    /// Model to use (e.g., "anthropic/claude-sonnet-4").
    pub model: String,
    /// Base URL for the API (for OpenRouter).
    pub base_url: Option<String>,
    /// Working directory for file operations.
    pub working_dir: std::path::PathBuf,
    /// Maximum context window size in tokens.
    pub max_context_tokens: usize,
    /// Checkpoint interval (number of tool calls between checkpoints).
    pub checkpoint_interval: usize,
    /// Optional Tavily API key for web search.
    pub tavily_api_key: Option<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "anthropic/claude-sonnet-4".to_string(),
            base_url: Some("https://openrouter.ai/api/v1".to_string()),
            working_dir: std::env::current_dir().unwrap_or_default(),
            max_context_tokens: 100_000,
            checkpoint_interval: 5,
            tavily_api_key: None,
        }
    }
}
