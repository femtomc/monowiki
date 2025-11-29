//! The main agent implementation.

use crate::client::{ApiClient, ChatResponse, ClientError};
use crate::context::ContextManager;
use crate::prompts;
use crate::state::{Checkpoint, SessionStats};
use crate::tools::{ToolRegistry, ToolResult};
use crate::types::{AgentConfig, Message, ToolCall};
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{debug, error, warn};

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Client error: {0}")]
    ClientError(#[from] ClientError),
    #[error("Tool error: {0}")]
    ToolError(#[from] crate::tools::ToolError),
    #[error("State error: {0}")]
    StateError(#[from] crate::state::StateError),
    #[error("Max retries exceeded")]
    MaxRetriesExceeded,
    #[error("Agent stopped")]
    Stopped,
}

/// Event emitted during agent execution.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Agent is thinking/processing.
    Thinking,
    /// Text chunk from the assistant.
    Text(String),
    /// Tool is being called.
    ToolCall { name: String, id: String },
    /// Tool execution completed.
    ToolResult { name: String, success: bool },
    /// Agent completed successfully.
    Done,
    /// Error occurred.
    Error(String),
    /// Status message.
    Status(String),
}

/// The main agent that orchestrates the agentic loop.
pub struct Agent {
    /// API client for LLM calls.
    client: Arc<dyn ApiClient>,
    /// Tool registry.
    tools: ToolRegistry,
    /// Context manager for compression.
    context_manager: ContextManager,
    /// Configuration.
    config: AgentConfig,
    /// Current message history.
    messages: Vec<Message>,
    /// Session statistics.
    stats: SessionStats,
    /// Whether the agent is currently running.
    running: Arc<Mutex<bool>>,
}

impl Agent {
    /// Create a new agent with the given client and config.
    pub fn new(client: Arc<dyn ApiClient>, config: AgentConfig) -> Self {
        let tools =
            ToolRegistry::with_all_tools(config.working_dir.clone(), config.tavily_api_key.clone());
        let context_manager = ContextManager::new(config.max_context_tokens);

        Self {
            client,
            tools,
            context_manager,
            config,
            messages: Vec::new(),
            stats: SessionStats::new(),
            running: Arc::new(Mutex::new(false)),
        }
    }

    /// Create an agent with a custom tool registry.
    pub fn with_tools(
        client: Arc<dyn ApiClient>,
        config: AgentConfig,
        tools: ToolRegistry,
    ) -> Self {
        let context_manager = ContextManager::new(config.max_context_tokens);

        Self {
            client,
            tools,
            context_manager,
            config,
            messages: Vec::new(),
            stats: SessionStats::new(),
            running: Arc::new(Mutex::new(false)),
        }
    }

    /// Resume from a checkpoint.
    pub fn from_checkpoint(
        client: Arc<dyn ApiClient>,
        checkpoint: Checkpoint,
        config: AgentConfig,
    ) -> Self {
        let tools =
            ToolRegistry::with_all_tools(config.working_dir.clone(), config.tavily_api_key.clone());
        let context_manager = ContextManager::new(config.max_context_tokens);

        Self {
            client,
            tools,
            context_manager,
            config,
            messages: checkpoint.messages,
            stats: checkpoint.stats,
            running: Arc::new(Mutex::new(false)),
        }
    }

    /// Get current session statistics.
    pub fn stats(&self) -> &SessionStats {
        &self.stats
    }

    /// Get current message history.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Stop the agent.
    pub async fn stop(&self) {
        let mut running = self.running.lock().await;
        *running = false;
    }

    /// Check if the agent is running.
    pub async fn is_running(&self) -> bool {
        *self.running.lock().await
    }

    /// Save current state to a checkpoint.
    pub fn save_checkpoint(&self, query: &str) -> Result<(), crate::state::StateError> {
        let checkpoint = Checkpoint::new(
            query.to_string(),
            self.messages.clone(),
            self.stats.clone(),
            self.config.working_dir.to_string_lossy().to_string(),
            self.config.model.clone(),
        );
        let path = Checkpoint::default_path(&self.config.working_dir);
        checkpoint.save(path)
    }

    /// Run the agent with a query, returning a stream of events.
    pub async fn run<'a>(
        &'a mut self,
        query: &'a str,
    ) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send + 'a>>, AgentError> {
        // Set running flag
        {
            let mut running = self.running.lock().await;
            *running = true;
        }

        // Initialize with system prompt if empty
        if self.messages.is_empty() {
            self.messages
                .push(Message::system(prompts::investigation_prompt()));
        }

        // Add user query
        self.messages.push(Message::user(query));

        let stream = async_stream::stream! {
            let max_iterations = 50;
            let mut iteration = 0;

            loop {
                // Check if stopped
                if !*self.running.lock().await {
                    yield AgentEvent::Status("Agent stopped".to_string());
                    break;
                }

                iteration += 1;
                if iteration > max_iterations {
                    yield AgentEvent::Error("Max iterations reached".to_string());
                    break;
                }

                // Check for context compression
                if self.context_manager.needs_compression(&self.messages) {
                    yield AgentEvent::Status("Compressing context...".to_string());
                    let (compressed, summary) = self.context_manager.compress(&self.messages);
                    self.messages = compressed;
                    if let Some(s) = summary {
                        debug!("Context compressed: {}", s);
                    }
                }

                // Make API call
                yield AgentEvent::Thinking;

                let response = match self.call_api_with_retry().await {
                    Ok(r) => r,
                    Err(e) => {
                        yield AgentEvent::Error(e.to_string());
                        break;
                    }
                };

                // Update stats
                if let Some(usage) = &response.usage {
                    self.stats.record_usage(usage.prompt_tokens, usage.completion_tokens);
                }

                // Check for tool calls
                if let Some(ref tool_calls) = response.message.tool_calls {
                    // Add assistant message with tool calls
                    self.messages.push(response.message.clone());

                    // Execute each tool
                    for tool_call in tool_calls {
                        yield AgentEvent::ToolCall {
                            name: tool_call.function.name.clone(),
                            id: tool_call.id.clone(),
                        };

                        let result = self.execute_tool(tool_call).await;
                        self.stats.record_tool_call();

                        yield AgentEvent::ToolResult {
                            name: tool_call.function.name.clone(),
                            success: result.success,
                        };

                        // Add tool result to messages
                        let content = ContextManager::truncate_tool_result(&result.output, 30_000);
                        self.messages.push(Message::tool_result(&tool_call.id, content));
                    }

                    // Checkpoint periodically
                    if self.stats.tool_calls % self.config.checkpoint_interval == 0 {
                        if let Err(e) = self.save_checkpoint(query) {
                            warn!("Failed to save checkpoint: {}", e);
                        }
                    }
                } else {
                    // No tool calls - stream the final response
                    if let Some(text) = response.message.text() {
                        // For non-streaming, emit the whole text
                        yield AgentEvent::Text(text.to_string());
                    }

                    // Add assistant message
                    self.messages.push(response.message);

                    yield AgentEvent::Done;
                    break;
                }
            }

            // Final checkpoint
            let _ = self.save_checkpoint(query);

            // Clear running flag
            *self.running.lock().await = false;
        };

        Ok(Box::pin(stream))
    }

    /// Run the agent synchronously, collecting all output.
    pub async fn run_to_completion(&mut self, query: &str) -> Result<String, AgentError> {
        use futures::StreamExt;

        let mut output = String::new();
        let mut stream = self.run(query).await?;

        while let Some(event) = stream.next().await {
            match event {
                AgentEvent::Text(text) => output.push_str(&text),
                AgentEvent::Error(e) => {
                    return Err(AgentError::ClientError(ClientError::ApiError {
                        status: 0,
                        message: e,
                    }))
                }
                AgentEvent::Done => break,
                _ => {}
            }
        }

        Ok(output)
    }

    /// Make an API call with retry logic.
    async fn call_api_with_retry(&mut self) -> Result<ChatResponse, AgentError> {
        let max_retries = 5;
        let mut attempt = 0;
        let mut last_error = None;

        while attempt < max_retries {
            attempt += 1;

            match self
                .client
                .chat(&self.messages, &self.tools.definitions())
                .await
            {
                Ok(response) => return Ok(response),
                Err(ClientError::RateLimited { retry_after }) => {
                    let wait = retry_after.unwrap_or(5).min(60);
                    warn!(
                        "Rate limited, waiting {}s (attempt {}/{})",
                        wait, attempt, max_retries
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(wait)).await;
                    self.stats.record_error();
                    last_error = Some(ClientError::RateLimited { retry_after });
                }
                Err(e @ ClientError::HttpError(_)) => {
                    let wait = (1 << attempt).min(60);
                    warn!(
                        "HTTP error, retrying in {}s (attempt {}/{}): {}",
                        wait, attempt, max_retries, e
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(wait)).await;
                    self.stats.record_error();
                    last_error = Some(e);
                }
                Err(e) => {
                    error!("Unrecoverable API error: {}", e);
                    return Err(e.into());
                }
            }
        }

        Err(last_error
            .map(AgentError::ClientError)
            .unwrap_or(AgentError::MaxRetriesExceeded))
    }

    /// Execute a single tool call.
    async fn execute_tool(&self, tool_call: &ToolCall) -> ToolResult {
        let args: serde_json::Value = match serde_json::from_str(&tool_call.function.arguments) {
            Ok(v) => v,
            Err(e) => {
                return ToolResult::error(format!("Invalid tool arguments: {}", e));
            }
        };

        debug!(
            "Executing tool: {} with args: {}",
            tool_call.function.name, args
        );

        match self.tools.execute(&tool_call.function.name, args).await {
            Ok(result) => result,
            Err(e) => ToolResult::error(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{ApiClient, ChatResponse, StreamChunk, Usage};
    use crate::types::Message;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockClient {
        responses: Vec<ChatResponse>,
        call_count: AtomicUsize,
    }

    impl MockClient {
        fn new(responses: Vec<ChatResponse>) -> Self {
            Self {
                responses,
                call_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl ApiClient for MockClient {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: &[crate::types::ToolDefinition],
        ) -> Result<ChatResponse, ClientError> {
            let idx = self.call_count.fetch_add(1, Ordering::SeqCst);
            if idx < self.responses.len() {
                Ok(self.responses[idx].clone())
            } else {
                // Return a simple completion
                Ok(ChatResponse {
                    message: Message::assistant("Done"),
                    usage: Some(Usage::default()),
                    stop_reason: Some("stop".to_string()),
                })
            }
        }

        async fn chat_stream(
            &self,
            messages: &[Message],
            tools: &[crate::types::ToolDefinition],
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, ClientError>> + Send>>, ClientError>
        {
            // For tests, just use non-streaming
            let response = self.chat(messages, tools).await?;
            let stream = async_stream::stream! {
                if let Some(text) = response.message.text() {
                    yield Ok(StreamChunk::Text(text.to_string()));
                }
                yield Ok(StreamChunk::Done);
            };
            Ok(Box::pin(stream))
        }
    }

    #[tokio::test]
    async fn test_agent_simple_query() {
        let client = Arc::new(MockClient::new(vec![ChatResponse {
            message: Message::assistant("Hello! I'm here to help."),
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            }),
            stop_reason: Some("stop".to_string()),
        }]));

        let config = AgentConfig {
            working_dir: std::env::temp_dir(),
            ..Default::default()
        };

        let mut agent = Agent::new(client, config);
        let result = agent.run_to_completion("Hello").await.unwrap();

        assert!(result.contains("Hello"));
        assert_eq!(agent.stats().api_calls, 1);
    }
}
