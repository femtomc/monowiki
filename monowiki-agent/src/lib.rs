//! # monowiki-agent
//!
//! AI agent for codebase investigation and documentation.
//!
//! This crate provides an agentic loop that uses LLMs (via OpenRouter or Anthropic)
//! to investigate codebases, perform web research, and produce documentation.
//!
//! It also supports collaborative document editing, allowing agents to work
//! alongside users in the wiki editor.

pub mod agent;
pub mod client;
pub mod context;
pub mod document;
pub mod prompts;
pub mod state;
pub mod tools;
pub mod types;

pub use agent::Agent;
pub use client::{AnthropicClient, ApiClient, OpenRouterClient};
pub use document::{Comment, DocumentContext, DocumentOperations, Selection};
pub use state::{Checkpoint, SessionStats};
pub use tools::{FetchUrlTool, Tool, ToolRegistry, ToolResult, WebSearchTool};
pub use types::{AgentConfig, Message, Role, ToolCall, ToolDefinition};
