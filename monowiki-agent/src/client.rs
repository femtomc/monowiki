//! API client implementations for LLM providers.

use crate::types::{FunctionCall, Message, Role, ToolCall, ToolDefinition};
use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::pin::Pin;
use thiserror::Error;
use tokio_stream::StreamExt;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("API error: {status} - {message}")]
    ApiError { status: u16, message: String },
    #[error("Rate limited, retry after {retry_after:?}s")]
    RateLimited { retry_after: Option<u64> },
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Stream error: {0}")]
    StreamError(String),
}

/// Token usage information from an API response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

/// Response from a chat completion API.
#[derive(Debug, Clone)]
pub struct ChatResponse {
    /// The generated message.
    pub message: Message,
    /// Token usage information.
    pub usage: Option<Usage>,
    /// Stop reason (e.g., "stop", "tool_use", "end_turn").
    pub stop_reason: Option<String>,
}

/// A streaming chunk from the API.
#[derive(Debug, Clone)]
pub enum StreamChunk {
    /// A text delta.
    Text(String),
    /// A tool call being streamed.
    ToolCallStart { id: String, name: String },
    /// Arguments delta for a tool call.
    ToolCallDelta { id: String, arguments: String },
    /// Tool call complete.
    ToolCallEnd { id: String },
    /// Usage information (typically at the end).
    Usage(Usage),
    /// Stream finished.
    Done,
}

/// Trait for LLM API clients.
#[async_trait]
pub trait ApiClient: Send + Sync {
    /// Make a chat completion request.
    async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ChatResponse, ClientError>;

    /// Make a streaming chat completion request.
    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, ClientError>> + Send>>, ClientError>;
}

// ============================================================================
// OpenRouter Client (OpenAI-compatible API)
// ============================================================================

/// Client for the OpenRouter API.
pub struct OpenRouterClient {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenRouterClient {
    /// Create a new OpenRouter client.
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
            base_url: "https://openrouter.ai/api/v1".to_string(),
        }
    }

    /// Create a client with a custom base URL.
    pub fn with_base_url(api_key: String, model: String, base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
            base_url,
        }
    }

    fn convert_messages(&self, messages: &[Message]) -> Vec<Value> {
        messages
            .iter()
            .map(|msg| {
                let mut obj = json!({
                    "role": msg.role,
                });
                if let Some(ref content) = msg.content {
                    obj["content"] = json!(content);
                }
                if let Some(ref tool_calls) = msg.tool_calls {
                    obj["tool_calls"] = json!(tool_calls);
                }
                if let Some(ref tool_call_id) = msg.tool_call_id {
                    obj["tool_call_id"] = json!(tool_call_id);
                }
                obj
            })
            .collect()
    }

    fn convert_tools(&self, tools: &[ToolDefinition]) -> Vec<Value> {
        tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.parameters
                    }
                })
            })
            .collect()
    }
}

#[async_trait]
impl ApiClient for OpenRouterClient {
    async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ChatResponse, ClientError> {
        let mut body = json!({
            "model": self.model,
            "messages": self.convert_messages(messages),
        });

        if !tools.is_empty() {
            body["tools"] = json!(self.convert_tools(tools));
        }

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok());
            return Err(ClientError::RateLimited { retry_after });
        }

        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(ClientError::ApiError {
                status: status.as_u16(),
                message,
            });
        }

        let data: Value = response.json().await?;

        let choice = data
            .get("choices")
            .and_then(|c| c.get(0))
            .ok_or_else(|| ClientError::ParseError("No choices in response".to_string()))?;

        let msg = choice
            .get("message")
            .ok_or_else(|| ClientError::ParseError("No message in choice".to_string()))?;

        let role = msg
            .get("role")
            .and_then(|r| r.as_str())
            .unwrap_or("assistant");
        let content = msg
            .get("content")
            .and_then(|c| c.as_str())
            .map(String::from);
        let tool_calls: Option<Vec<ToolCall>> = msg
            .get("tool_calls")
            .and_then(|tc| serde_json::from_value(tc.clone()).ok());

        let usage = data.get("usage").and_then(|u| {
            Some(Usage {
                prompt_tokens: u.get("prompt_tokens")?.as_u64()? as usize,
                completion_tokens: u.get("completion_tokens")?.as_u64()? as usize,
                total_tokens: u.get("total_tokens")?.as_u64()? as usize,
            })
        });

        let stop_reason = choice
            .get("finish_reason")
            .and_then(|r| r.as_str())
            .map(String::from);

        let message = Message {
            role: if role == "assistant" {
                Role::Assistant
            } else {
                Role::User
            },
            content,
            content_blocks: None,
            tool_calls,
            tool_call_id: None,
        };

        Ok(ChatResponse {
            message,
            usage,
            stop_reason,
        })
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, ClientError>> + Send>>, ClientError>
    {
        let mut body = json!({
            "model": self.model,
            "messages": self.convert_messages(messages),
            "stream": true,
        });

        if !tools.is_empty() {
            body["tools"] = json!(self.convert_tools(tools));
        }

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(ClientError::ApiError {
                status: status.as_u16(),
                message,
            });
        }

        let stream = async_stream::stream! {
            let mut bytes_stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk) = bytes_stream.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        yield Err(ClientError::HttpError(e));
                        continue;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&chunk));

                // Process complete SSE lines
                while let Some(line_end) = buffer.find('\n') {
                    let line = buffer[..line_end].trim().to_string();
                    buffer = buffer[line_end + 1..].to_string();

                    if line.is_empty() || line.starts_with(':') {
                        continue;
                    }

                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            yield Ok(StreamChunk::Done);
                            continue;
                        }

                        if let Ok(json) = serde_json::from_str::<Value>(data) {
                            if let Some(choice) = json.get("choices").and_then(|c| c.get(0)) {
                                if let Some(delta) = choice.get("delta") {
                                    // Text content
                                    if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                        if !content.is_empty() {
                                            yield Ok(StreamChunk::Text(content.to_string()));
                                        }
                                    }

                                    // Tool calls
                                    if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                                        for tc in tool_calls {
                                            let id = tc.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                                            if let Some(func) = tc.get("function") {
                                                if let Some(name) = func.get("name").and_then(|n| n.as_str()) {
                                                    yield Ok(StreamChunk::ToolCallStart {
                                                        id: id.clone(),
                                                        name: name.to_string(),
                                                    });
                                                }
                                                if let Some(args) = func.get("arguments").and_then(|a| a.as_str()) {
                                                    if !args.is_empty() {
                                                        yield Ok(StreamChunk::ToolCallDelta {
                                                            id: id.clone(),
                                                            arguments: args.to_string(),
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Usage (may come at end)
                            if let Some(usage) = json.get("usage") {
                                if let (Some(prompt), Some(completion)) = (
                                    usage.get("prompt_tokens").and_then(|p| p.as_u64()),
                                    usage.get("completion_tokens").and_then(|c| c.as_u64()),
                                ) {
                                    yield Ok(StreamChunk::Usage(Usage {
                                        prompt_tokens: prompt as usize,
                                        completion_tokens: completion as usize,
                                        total_tokens: (prompt + completion) as usize,
                                    }));
                                }
                            }
                        }
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}

// ============================================================================
// Anthropic Client (native API)
// ============================================================================

/// Client for the Anthropic API.
pub struct AnthropicClient {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl AnthropicClient {
    /// Create a new Anthropic client.
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
        }
    }

    fn convert_messages(&self, messages: &[Message]) -> (Option<String>, Vec<Value>) {
        let mut system = None;
        let mut converted = Vec::new();

        for msg in messages {
            match msg.role {
                Role::System => {
                    system = msg.content.clone();
                }
                Role::User => {
                    converted.push(json!({
                        "role": "user",
                        "content": msg.content.as_deref().unwrap_or("")
                    }));
                }
                Role::Assistant => {
                    if let Some(ref tool_calls) = msg.tool_calls {
                        // Convert OpenAI-style tool calls to Anthropic content blocks
                        let mut content = Vec::new();
                        if let Some(ref text) = msg.content {
                            content.push(json!({"type": "text", "text": text}));
                        }
                        for tc in tool_calls {
                            let input: Value =
                                serde_json::from_str(&tc.function.arguments).unwrap_or(json!({}));
                            content.push(json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.function.name,
                                "input": input
                            }));
                        }
                        converted.push(json!({
                            "role": "assistant",
                            "content": content
                        }));
                    } else {
                        converted.push(json!({
                            "role": "assistant",
                            "content": msg.content.as_deref().unwrap_or("")
                        }));
                    }
                }
                Role::Tool => {
                    // Tool results go in user messages for Anthropic
                    converted.push(json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": msg.tool_call_id,
                            "content": msg.content.as_deref().unwrap_or("")
                        }]
                    }));
                }
            }
        }

        (system, converted)
    }

    fn convert_tools(&self, tools: &[ToolDefinition]) -> Vec<Value> {
        tools
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.parameters
                })
            })
            .collect()
    }
}

#[async_trait]
impl ApiClient for AnthropicClient {
    async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ChatResponse, ClientError> {
        let (system, converted_messages) = self.convert_messages(messages);

        let mut body = json!({
            "model": self.model,
            "max_tokens": 8192,
            "messages": converted_messages,
        });

        if let Some(sys) = system {
            body["system"] = json!(sys);
        }

        if !tools.is_empty() {
            body["tools"] = json!(self.convert_tools(tools));
        }

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok());
            return Err(ClientError::RateLimited { retry_after });
        }

        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(ClientError::ApiError {
                status: status.as_u16(),
                message,
            });
        }

        let data: Value = response.json().await?;

        let content_blocks = data
            .get("content")
            .and_then(|c| c.as_array())
            .cloned()
            .unwrap_or_default();

        let mut text_content = String::new();
        let mut tool_calls = Vec::new();

        for block in content_blocks {
            match block.get("type").and_then(|t| t.as_str()) {
                Some("text") => {
                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                        text_content.push_str(text);
                    }
                }
                Some("tool_use") => {
                    let id = block
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = block
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string();
                    let input = block.get("input").cloned().unwrap_or(json!({}));

                    tool_calls.push(ToolCall {
                        id,
                        call_type: "function".to_string(),
                        function: FunctionCall {
                            name,
                            arguments: input.to_string(),
                        },
                    });
                }
                _ => {}
            }
        }

        let usage = data.get("usage").and_then(|u| {
            Some(Usage {
                prompt_tokens: u.get("input_tokens")?.as_u64()? as usize,
                completion_tokens: u.get("output_tokens")?.as_u64()? as usize,
                total_tokens: (u.get("input_tokens")?.as_u64()?
                    + u.get("output_tokens")?.as_u64()?) as usize,
            })
        });

        let stop_reason = data
            .get("stop_reason")
            .and_then(|r| r.as_str())
            .map(String::from);

        let message = Message {
            role: Role::Assistant,
            content: if text_content.is_empty() {
                None
            } else {
                Some(text_content)
            },
            content_blocks: None,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
            tool_call_id: None,
        };

        Ok(ChatResponse {
            message,
            usage,
            stop_reason,
        })
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, ClientError>> + Send>>, ClientError>
    {
        let (system, converted_messages) = self.convert_messages(messages);

        let mut body = json!({
            "model": self.model,
            "max_tokens": 8192,
            "messages": converted_messages,
            "stream": true,
        });

        if let Some(sys) = system {
            body["system"] = json!(sys);
        }

        if !tools.is_empty() {
            body["tools"] = json!(self.convert_tools(tools));
        }

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(ClientError::ApiError {
                status: status.as_u16(),
                message,
            });
        }

        let stream = async_stream::stream! {
            let mut bytes_stream = response.bytes_stream();
            let mut buffer = String::new();
            let mut current_tool_id = String::new();

            while let Some(chunk) = bytes_stream.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        yield Err(ClientError::HttpError(e));
                        continue;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(line_end) = buffer.find('\n') {
                    let line = buffer[..line_end].trim().to_string();
                    buffer = buffer[line_end + 1..].to_string();

                    if line.is_empty() || line.starts_with(':') {
                        continue;
                    }

                    if let Some(data) = line.strip_prefix("data: ") {
                        if let Ok(json) = serde_json::from_str::<Value>(data) {
                            match json.get("type").and_then(|t| t.as_str()) {
                                Some("content_block_start") => {
                                    if let Some(block) = json.get("content_block") {
                                        if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                            let id = block.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                                            let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                                            current_tool_id = id.clone();
                                            yield Ok(StreamChunk::ToolCallStart { id, name });
                                        }
                                    }
                                }
                                Some("content_block_delta") => {
                                    if let Some(delta) = json.get("delta") {
                                        match delta.get("type").and_then(|t| t.as_str()) {
                                            Some("text_delta") => {
                                                if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                                    yield Ok(StreamChunk::Text(text.to_string()));
                                                }
                                            }
                                            Some("input_json_delta") => {
                                                if let Some(partial) = delta.get("partial_json").and_then(|p| p.as_str()) {
                                                    yield Ok(StreamChunk::ToolCallDelta {
                                                        id: current_tool_id.clone(),
                                                        arguments: partial.to_string(),
                                                    });
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                Some("content_block_stop") => {
                                    if !current_tool_id.is_empty() {
                                        yield Ok(StreamChunk::ToolCallEnd { id: current_tool_id.clone() });
                                        current_tool_id.clear();
                                    }
                                }
                                Some("message_delta") => {
                                    if let Some(usage) = json.get("usage") {
                                        if let Some(output) = usage.get("output_tokens").and_then(|o| o.as_u64()) {
                                            yield Ok(StreamChunk::Usage(Usage {
                                                prompt_tokens: 0,
                                                completion_tokens: output as usize,
                                                total_tokens: output as usize,
                                            }));
                                        }
                                    }
                                }
                                Some("message_stop") => {
                                    yield Ok(StreamChunk::Done);
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}
