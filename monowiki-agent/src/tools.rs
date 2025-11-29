//! Tool definitions and execution.

use crate::types::ToolDefinition;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ToolError {
    #[error("Tool not found: {0}")]
    NotFound(String),
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Path outside sandbox: {0}")]
    PathOutsideSandbox(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
}

/// Result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Whether the tool execution was successful.
    pub success: bool,
    /// The output of the tool (or error message).
    pub output: String,
}

impl ToolResult {
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            output: message.into(),
        }
    }
}

/// Trait for implementing tools.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool definition.
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool with the given arguments.
    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError>;
}

/// Registry of available tools.
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Create a registry with default file tools.
    pub fn with_file_tools(working_dir: PathBuf) -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(ListFilesTool::new(working_dir.clone())));
        registry.register(Box::new(ReadFileTool::new(working_dir.clone())));
        registry.register(Box::new(WriteFileTool::new(working_dir.clone())));
        registry.register(Box::new(EditFileTool::new(working_dir)));
        registry
    }

    /// Create a registry with all tools including web search.
    pub fn with_all_tools(working_dir: PathBuf, tavily_api_key: Option<String>) -> Self {
        let mut registry = Self::with_file_tools(working_dir);
        if let Some(key) = tavily_api_key {
            registry.register(Box::new(WebSearchTool::new(key)));
        }
        registry.register(Box::new(FetchUrlTool::new()));
        registry
    }

    /// Register a tool.
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let def = tool.definition();
        self.tools.insert(def.name, tool);
    }

    /// Get all tool definitions.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// Execute a tool by name.
    pub async fn execute(&self, name: &str, args: Value) -> Result<ToolResult, ToolError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;
        tool.execute(args).await
    }

    /// Check if a tool exists.
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// File Tools
// ============================================================================

/// Resolve a path relative to the working directory, ensuring it stays within the sandbox.
fn resolve_sandboxed_path(working_dir: &Path, relative_path: &str) -> Result<PathBuf, ToolError> {
    let path = working_dir.join(relative_path);
    let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
    let working_canonical = working_dir
        .canonicalize()
        .unwrap_or_else(|_| working_dir.to_path_buf());

    if !canonical.starts_with(&working_canonical) {
        return Err(ToolError::PathOutsideSandbox(relative_path.to_string()));
    }
    Ok(path)
}

/// Tool for listing files in a directory.
pub struct ListFilesTool {
    working_dir: PathBuf,
}

impl ListFilesTool {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[async_trait]
impl Tool for ListFilesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "list_files",
            "List files and directories at a given path. Use glob patterns to filter results.",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to list (default: current directory)"
                    },
                    "pattern": {
                        "type": "string",
                        "description": "Optional glob pattern to filter results (e.g., '*.rs', '**/*.md')"
                    }
                },
                "required": []
            }),
        )
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");
        let pattern = args.get("pattern").and_then(|v| v.as_str());

        let full_path = resolve_sandboxed_path(&self.working_dir, path)?;

        if !full_path.exists() {
            return Ok(ToolResult::error(format!("Path does not exist: {}", path)));
        }

        let mut entries = Vec::new();

        if let Some(glob_pattern) = pattern {
            let full_pattern = full_path.join(glob_pattern);
            let pattern_str = full_pattern.to_string_lossy();

            match glob::glob(&pattern_str) {
                Ok(paths) => {
                    for entry in paths.filter_map(|e| e.ok()) {
                        if let Ok(rel) = entry.strip_prefix(&self.working_dir) {
                            let prefix = if entry.is_dir() { "[D] " } else { "[F] " };
                            entries.push(format!("{}{}", prefix, rel.display()));
                        }
                    }
                }
                Err(e) => return Ok(ToolResult::error(format!("Invalid glob pattern: {}", e))),
            }
        } else if full_path.is_dir() {
            let mut dir_entries: Vec<_> = std::fs::read_dir(&full_path)?
                .filter_map(|e| e.ok())
                .collect();
            dir_entries.sort_by_key(|e| e.file_name());

            for entry in dir_entries {
                let file_type = entry.file_type()?;
                let name = entry.file_name().to_string_lossy().to_string();
                let prefix = if file_type.is_dir() { "[D] " } else { "[F] " };
                entries.push(format!("{}{}", prefix, name));
            }
        } else {
            entries.push(format!("[F] {}", path));
        }

        if entries.is_empty() {
            Ok(ToolResult::success("No files found."))
        } else {
            Ok(ToolResult::success(entries.join("\n")))
        }
    }
}

/// Tool for reading file contents.
pub struct ReadFileTool {
    working_dir: PathBuf,
}

impl ReadFileTool {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "read_file",
            "Read the contents of a file. Returns the file content as text.",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file to read"
                    }
                },
                "required": ["path"]
            }),
        )
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'path' argument".to_string()))?;

        let full_path = resolve_sandboxed_path(&self.working_dir, path)?;

        if !full_path.exists() {
            return Ok(ToolResult::error(format!("File does not exist: {}", path)));
        }

        if !full_path.is_file() {
            return Ok(ToolResult::error(format!("Path is not a file: {}", path)));
        }

        match std::fs::read_to_string(&full_path) {
            Ok(content) => {
                // Truncate very large files
                const MAX_SIZE: usize = 100_000;
                if content.len() > MAX_SIZE {
                    let truncated = &content[..MAX_SIZE];
                    Ok(ToolResult::success(format!(
                        "{}\n\n[Truncated: file is {} bytes, showing first {} bytes]",
                        truncated,
                        content.len(),
                        MAX_SIZE
                    )))
                } else {
                    Ok(ToolResult::success(content))
                }
            }
            Err(e) => Ok(ToolResult::error(format!("Failed to read file: {}", e))),
        }
    }
}

/// Tool for writing file contents.
pub struct WriteFileTool {
    working_dir: PathBuf,
}

impl WriteFileTool {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[async_trait]
impl Tool for WriteFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "write_file",
            "Write content to a file. Creates the file if it doesn't exist, overwrites if it does.",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    }
                },
                "required": ["path", "content"]
            }),
        )
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'path' argument".to_string()))?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'content' argument".to_string()))?;

        let full_path = self.working_dir.join(path);

        // Ensure path is within sandbox (check parent for new files)
        let parent = full_path.parent().unwrap_or(&self.working_dir);
        if parent.exists() {
            let canonical_parent = parent.canonicalize()?;
            let canonical_working = self.working_dir.canonicalize()?;
            if !canonical_parent.starts_with(&canonical_working) {
                return Err(ToolError::PathOutsideSandbox(path.to_string()));
            }
        }

        // Create parent directories if needed
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&full_path, content)?;
        Ok(ToolResult::success(format!("Successfully wrote {} bytes to {}", content.len(), path)))
    }
}

/// Tool for editing files with string replacement.
pub struct EditFileTool {
    working_dir: PathBuf,
}

impl EditFileTool {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[async_trait]
impl Tool for EditFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "edit_file",
            "Edit a file by replacing a specific string. The old_string must appear exactly once in the file.",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file to edit"
                    },
                    "old_string": {
                        "type": "string",
                        "description": "The exact string to find and replace"
                    },
                    "new_string": {
                        "type": "string",
                        "description": "The string to replace it with"
                    }
                },
                "required": ["path", "old_string", "new_string"]
            }),
        )
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'path' argument".to_string()))?;
        let old_string = args
            .get("old_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'old_string' argument".to_string()))?;
        let new_string = args
            .get("new_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'new_string' argument".to_string()))?;

        let full_path = resolve_sandboxed_path(&self.working_dir, path)?;

        if !full_path.exists() {
            return Ok(ToolResult::error(format!("File does not exist: {}", path)));
        }

        let content = std::fs::read_to_string(&full_path)?;
        let count = content.matches(old_string).count();

        if count == 0 {
            return Ok(ToolResult::error("old_string not found in file"));
        }
        if count > 1 {
            return Ok(ToolResult::error(format!(
                "old_string appears {} times in file; must be unique",
                count
            )));
        }

        let new_content = content.replace(old_string, new_string);
        std::fs::write(&full_path, new_content)?;
        Ok(ToolResult::success(format!("Successfully edited {}", path)))
    }
}

// ============================================================================
// Web Tools
// ============================================================================

/// Tool for web search via Tavily.
pub struct WebSearchTool {
    api_key: String,
    client: reqwest::Client,
}

impl WebSearchTool {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "web_search",
            "Search the web for information. Returns titles, URLs, and snippets from top results.",
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
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'query' argument".to_string()))?;

        let response = self
            .client
            .post("https://api.tavily.com/search")
            .json(&json!({
                "api_key": self.api_key,
                "query": query,
                "search_depth": "advanced",
                "max_results": 5
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Ok(ToolResult::error(format!(
                "Search API error: {}",
                response.status()
            )));
        }

        let data: Value = response.json().await?;
        let results = data
            .get("results")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut output = Vec::new();
        for result in results {
            let title = result.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let url = result.get("url").and_then(|v| v.as_str()).unwrap_or("");
            let snippet = result.get("content").and_then(|v| v.as_str()).unwrap_or("");
            output.push(format!("**{}**\n{}\n{}\n", title, url, snippet));
        }

        if output.is_empty() {
            Ok(ToolResult::success("No results found."))
        } else {
            Ok(ToolResult::success(output.join("\n---\n")))
        }
    }
}

/// Tool for fetching web pages.
pub struct FetchUrlTool {
    client: reqwest::Client,
}

impl FetchUrlTool {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::limited(10))
                .build()
                .unwrap_or_default(),
        }
    }
}

impl Default for FetchUrlTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for FetchUrlTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "fetch_url",
            "Fetch the contents of a web page. Returns the text content.",
            json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch"
                    }
                },
                "required": ["url"]
            }),
        )
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'url' argument".to_string()))?;

        let response = self
            .client
            .get(url)
            .header("User-Agent", "monowiki-agent/0.1")
            .send()
            .await?;

        if !response.status().is_success() {
            return Ok(ToolResult::error(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();

        let text = response.text().await?;

        // Truncate large responses
        const MAX_SIZE: usize = 50_000;
        let output = if text.len() > MAX_SIZE {
            format!(
                "{}\n\n[Truncated: content is {} bytes, showing first {} bytes]",
                &text[..MAX_SIZE],
                text.len(),
                MAX_SIZE
            )
        } else {
            text
        };

        // For HTML, we'd ideally strip tags, but for now return raw
        if content_type.contains("text/html") {
            Ok(ToolResult::success(format!("[HTML content from {}]\n\n{}", url, output)))
        } else {
            Ok(ToolResult::success(output))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_list_files() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("test.txt"), "hello").unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();

        let tool = ListFilesTool::new(dir.path().to_path_buf());
        let result = tool.execute(json!({"path": "."})).await.unwrap();

        assert!(result.success);
        assert!(result.output.contains("test.txt"));
        assert!(result.output.contains("subdir"));
    }

    #[tokio::test]
    async fn test_read_file() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("test.txt"), "hello world").unwrap();

        let tool = ReadFileTool::new(dir.path().to_path_buf());
        let result = tool.execute(json!({"path": "test.txt"})).await.unwrap();

        assert!(result.success);
        assert_eq!(result.output, "hello world");
    }

    #[tokio::test]
    async fn test_write_file() {
        let dir = tempdir().unwrap();

        let tool = WriteFileTool::new(dir.path().to_path_buf());
        let result = tool
            .execute(json!({"path": "new.txt", "content": "new content"}))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("new.txt")).unwrap(),
            "new content"
        );
    }

    #[tokio::test]
    async fn test_edit_file() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("test.txt"), "hello world").unwrap();

        let tool = EditFileTool::new(dir.path().to_path_buf());
        let result = tool
            .execute(json!({
                "path": "test.txt",
                "old_string": "world",
                "new_string": "rust"
            }))
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("test.txt")).unwrap(),
            "hello rust"
        );
    }

    #[tokio::test]
    async fn test_sandbox_escape_prevented() {
        let dir = tempdir().unwrap();

        let tool = ReadFileTool::new(dir.path().to_path_buf());
        let result = tool.execute(json!({"path": "../../../etc/passwd"})).await;

        assert!(result.is_err() || !result.unwrap().success);
    }
}
