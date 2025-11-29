//! # monowiki-agent
//!
//! AI agent for codebase investigation and documentation.
//!
//! This crate provides an agentic loop that uses LLMs (via OpenRouter or Anthropic)
//! to investigate codebases, perform web research, and produce documentation.
//!
//! It also supports collaborative document editing, allowing agents to work
//! alongside users in the wiki editor.

pub mod types;
pub mod tools;
pub mod client;
pub mod agent;
pub mod context;
pub mod state;
pub mod prompts;
pub mod document;

pub use agent::Agent;
pub use client::{ApiClient, OpenRouterClient, AnthropicClient};
pub use document::{DocumentContext, DocumentOperations, Selection, Comment};
pub use state::{SessionStats, Checkpoint};
pub use tools::{Tool, ToolRegistry, ToolResult, WebSearchTool, FetchUrlTool};
pub use types::{AgentConfig, Message, Role, ToolCall, ToolDefinition};
