//! Document-aware tools for collaborative editing with the agent.
//!
//! These tools allow the agent to interact with wiki documents through
//! the CRDT layer, enabling selection-based editing, comments, and
//! cross-document navigation.

use crate::tools::{Tool, ToolError, ToolResult};
use crate::types::ToolDefinition;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;

/// A text range within a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRange {
    /// Block ID containing the range.
    pub block_id: String,
    /// Start offset within the block.
    pub start: usize,
    /// End offset within the block.
    pub end: usize,
}

/// A comment/annotation on a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    /// Unique comment ID.
    pub id: String,
    /// The range this comment is anchored to.
    pub range: TextRange,
    /// Comment content.
    pub content: String,
    /// Author (agent or user).
    pub author: String,
    /// Timestamp.
    pub created_at: String,
    /// Whether this comment is resolved.
    pub resolved: bool,
}

/// Current document context for the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentContext {
    /// Document slug.
    pub slug: String,
    /// Full document content (markdown).
    pub content: String,
    /// Currently selected text (if any).
    pub selection: Option<Selection>,
    /// Block structure for reference.
    pub blocks: Vec<BlockInfo>,
}

/// A user's current selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Selection {
    /// The selected text.
    pub text: String,
    /// Block ID containing the selection.
    pub block_id: String,
    /// Start offset.
    pub start: usize,
    /// End offset.
    pub end: usize,
}

/// Basic block information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockInfo {
    pub id: String,
    pub kind: String,
    pub text: String,
}

/// Trait for document operations that tools can use.
/// This will be implemented by the collab daemon's document store.
#[async_trait]
pub trait DocumentOperations: Send + Sync {
    /// Get the current document context.
    async fn get_context(&self, slug: &str) -> Result<DocumentContext, ToolError>;

    /// Get the current selection (if any).
    async fn get_selection(&self, slug: &str) -> Result<Option<Selection>, ToolError>;

    /// Replace text in a range.
    async fn replace_range(
        &self,
        slug: &str,
        block_id: &str,
        start: usize,
        end: usize,
        new_text: &str,
    ) -> Result<(), ToolError>;

    /// Insert text at a position.
    async fn insert_text(
        &self,
        slug: &str,
        block_id: &str,
        offset: usize,
        text: &str,
    ) -> Result<(), ToolError>;

    /// Add a comment anchored to a range.
    async fn add_comment(
        &self,
        slug: &str,
        block_id: &str,
        start: usize,
        end: usize,
        content: &str,
        author: &str,
    ) -> Result<String, ToolError>;

    /// Get comments on a document.
    async fn get_comments(&self, slug: &str) -> Result<Vec<Comment>, ToolError>;

    /// Resolve a comment.
    async fn resolve_comment(&self, slug: &str, comment_id: &str) -> Result<(), ToolError>;

    /// Read another document in the vault.
    async fn read_document(&self, slug: &str) -> Result<String, ToolError>;

    /// Search the vault for documents matching a query.
    async fn search_vault(&self, query: &str) -> Result<Vec<SearchResult>, ToolError>;

    /// Get linked documents (outlinks and backlinks).
    async fn get_links(&self, slug: &str) -> Result<LinkInfo, ToolError>;
}

/// Search result from vault search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub slug: String,
    pub title: String,
    pub snippet: String,
    pub score: f32,
}

/// Link information for a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkInfo {
    /// Documents this document links to.
    pub outlinks: Vec<String>,
    /// Documents that link to this document.
    pub backlinks: Vec<String>,
}

// =============================================================================
// Document Tools
// =============================================================================

/// Tool for reading the current document context.
pub struct GetContextTool {
    ops: Arc<dyn DocumentOperations>,
    current_slug: Arc<RwLock<Option<String>>>,
}

impl GetContextTool {
    pub fn new(ops: Arc<dyn DocumentOperations>, current_slug: Arc<RwLock<Option<String>>>) -> Self {
        Self { ops, current_slug }
    }
}

#[async_trait]
impl Tool for GetContextTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "get_document_context",
            "Get the current document's content, structure, and any active selection. Use this to understand what the user is working on.",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        )
    }

    async fn execute(&self, _args: Value) -> Result<ToolResult, ToolError> {
        let slug = self.current_slug.read().await;
        let slug = slug
            .as_ref()
            .ok_or_else(|| ToolError::ExecutionFailed("No document currently open".to_string()))?;

        let context = self.ops.get_context(slug).await?;
        let json = serde_json::to_string_pretty(&context)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(ToolResult::success(json))
    }
}

/// Tool for getting the current selection.
pub struct GetSelectionTool {
    ops: Arc<dyn DocumentOperations>,
    current_slug: Arc<RwLock<Option<String>>>,
}

impl GetSelectionTool {
    pub fn new(ops: Arc<dyn DocumentOperations>, current_slug: Arc<RwLock<Option<String>>>) -> Self {
        Self { ops, current_slug }
    }
}

#[async_trait]
impl Tool for GetSelectionTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "get_selection",
            "Get the text currently selected by the user in the editor.",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        )
    }

    async fn execute(&self, _args: Value) -> Result<ToolResult, ToolError> {
        let slug = self.current_slug.read().await;
        let slug = slug
            .as_ref()
            .ok_or_else(|| ToolError::ExecutionFailed("No document currently open".to_string()))?;

        match self.ops.get_selection(slug).await? {
            Some(selection) => {
                let json = serde_json::to_string_pretty(&selection)
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                Ok(ToolResult::success(json))
            }
            None => Ok(ToolResult::success("No text currently selected.")),
        }
    }
}

/// Tool for replacing text in a range.
pub struct ReplaceRangeTool {
    ops: Arc<dyn DocumentOperations>,
    current_slug: Arc<RwLock<Option<String>>>,
}

impl ReplaceRangeTool {
    pub fn new(ops: Arc<dyn DocumentOperations>, current_slug: Arc<RwLock<Option<String>>>) -> Self {
        Self { ops, current_slug }
    }
}

#[async_trait]
impl Tool for ReplaceRangeTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "replace_range",
            "Replace text in a specific range of the document. Use this to make edits to the user's document.",
            json!({
                "type": "object",
                "properties": {
                    "block_id": {
                        "type": "string",
                        "description": "The block ID to edit"
                    },
                    "start": {
                        "type": "integer",
                        "description": "Start offset within the block"
                    },
                    "end": {
                        "type": "integer",
                        "description": "End offset within the block"
                    },
                    "new_text": {
                        "type": "string",
                        "description": "The text to replace the range with"
                    }
                },
                "required": ["block_id", "start", "end", "new_text"]
            }),
        )
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let slug = self.current_slug.read().await;
        let slug = slug
            .as_ref()
            .ok_or_else(|| ToolError::ExecutionFailed("No document currently open".to_string()))?;

        let block_id = args
            .get("block_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing block_id".to_string()))?;
        let start = args
            .get("start")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::InvalidArguments("Missing start".to_string()))? as usize;
        let end = args
            .get("end")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::InvalidArguments("Missing end".to_string()))? as usize;
        let new_text = args
            .get("new_text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing new_text".to_string()))?;

        self.ops
            .replace_range(slug, block_id, start, end, new_text)
            .await?;

        Ok(ToolResult::success(format!(
            "Replaced text in block {} ({}:{}) with {} chars",
            block_id,
            start,
            end,
            new_text.len()
        )))
    }
}

/// Tool for adding comments.
pub struct AddCommentTool {
    ops: Arc<dyn DocumentOperations>,
    current_slug: Arc<RwLock<Option<String>>>,
}

impl AddCommentTool {
    pub fn new(ops: Arc<dyn DocumentOperations>, current_slug: Arc<RwLock<Option<String>>>) -> Self {
        Self { ops, current_slug }
    }
}

#[async_trait]
impl Tool for AddCommentTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "add_comment",
            "Add a comment anchored to a specific range in the document. Use this to leave notes, suggestions, or questions for the user.",
            json!({
                "type": "object",
                "properties": {
                    "block_id": {
                        "type": "string",
                        "description": "The block ID to attach the comment to"
                    },
                    "start": {
                        "type": "integer",
                        "description": "Start offset within the block"
                    },
                    "end": {
                        "type": "integer",
                        "description": "End offset within the block"
                    },
                    "content": {
                        "type": "string",
                        "description": "The comment text"
                    }
                },
                "required": ["block_id", "start", "end", "content"]
            }),
        )
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let slug = self.current_slug.read().await;
        let slug = slug
            .as_ref()
            .ok_or_else(|| ToolError::ExecutionFailed("No document currently open".to_string()))?;

        let block_id = args
            .get("block_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing block_id".to_string()))?;
        let start = args
            .get("start")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::InvalidArguments("Missing start".to_string()))? as usize;
        let end = args
            .get("end")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::InvalidArguments("Missing end".to_string()))? as usize;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing content".to_string()))?;

        let comment_id = self
            .ops
            .add_comment(slug, block_id, start, end, content, "agent")
            .await?;

        Ok(ToolResult::success(format!(
            "Added comment {} on block {} ({}:{})",
            comment_id, block_id, start, end
        )))
    }
}

/// Tool for reading other documents in the vault.
pub struct ReadDocumentTool {
    ops: Arc<dyn DocumentOperations>,
}

impl ReadDocumentTool {
    pub fn new(ops: Arc<dyn DocumentOperations>) -> Self {
        Self { ops }
    }
}

#[async_trait]
impl Tool for ReadDocumentTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "read_document",
            "Read another document in the vault by its slug. Use this to understand related content or context.",
            json!({
                "type": "object",
                "properties": {
                    "slug": {
                        "type": "string",
                        "description": "The document slug (e.g., 'notes/meeting-notes')"
                    }
                },
                "required": ["slug"]
            }),
        )
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let slug = args
            .get("slug")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing slug".to_string()))?;

        let content = self.ops.read_document(slug).await?;
        Ok(ToolResult::success(content))
    }
}

/// Tool for searching the vault.
pub struct SearchVaultTool {
    ops: Arc<dyn DocumentOperations>,
}

impl SearchVaultTool {
    pub fn new(ops: Arc<dyn DocumentOperations>) -> Self {
        Self { ops }
    }
}

#[async_trait]
impl Tool for SearchVaultTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "search_vault",
            "Search the vault for documents matching a query. Returns titles, slugs, and snippets.",
            json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    }
                },
                "required": ["query"]
            }),
        )
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing query".to_string()))?;

        let results = self.ops.search_vault(query).await?;
        let json = serde_json::to_string_pretty(&results)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(ToolResult::success(json))
    }
}

/// Tool for getting document links.
pub struct GetLinksTool {
    ops: Arc<dyn DocumentOperations>,
    current_slug: Arc<RwLock<Option<String>>>,
}

impl GetLinksTool {
    pub fn new(ops: Arc<dyn DocumentOperations>, current_slug: Arc<RwLock<Option<String>>>) -> Self {
        Self { ops, current_slug }
    }
}

#[async_trait]
impl Tool for GetLinksTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "get_links",
            "Get the outlinks (documents this links to) and backlinks (documents linking here) for the current document.",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        )
    }

    async fn execute(&self, _args: Value) -> Result<ToolResult, ToolError> {
        let slug = self.current_slug.read().await;
        let slug = slug
            .as_ref()
            .ok_or_else(|| ToolError::ExecutionFailed("No document currently open".to_string()))?;

        let links = self.ops.get_links(slug).await?;
        let json = serde_json::to_string_pretty(&links)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(ToolResult::success(json))
    }
}

/// Create a tool registry with all document-aware tools.
pub fn create_document_tools(
    ops: Arc<dyn DocumentOperations>,
    current_slug: Arc<RwLock<Option<String>>>,
) -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(GetContextTool::new(ops.clone(), current_slug.clone())),
        Box::new(GetSelectionTool::new(ops.clone(), current_slug.clone())),
        Box::new(ReplaceRangeTool::new(ops.clone(), current_slug.clone())),
        Box::new(AddCommentTool::new(ops.clone(), current_slug.clone())),
        Box::new(ReadDocumentTool::new(ops.clone())),
        Box::new(SearchVaultTool::new(ops.clone())),
        Box::new(GetLinksTool::new(ops, current_slug)),
    ]
}
