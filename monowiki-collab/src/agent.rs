//! Agent integration for collaborative document editing.
//!
//! This module connects the monowiki-agent to the collab daemon,
//! implementing the DocumentOperations trait and managing agent sessions.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use monowiki_agent::{
    document::{
        BlockInfo, Comment as AgentComment, DocumentContext, DocumentOperations, LinkInfo,
        SearchResult, Selection,
    },
    tools::ToolError,
    Agent, AgentConfig, AnthropicClient, ApiClient, FetchUrlTool, OpenRouterClient, ToolRegistry,
    WebSearchTool,
};
use serde::{Deserialize, Serialize};
use serde_yaml;
use tokio::sync::RwLock;

use crate::crdt::DocStore;

/// Configuration for the agent.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AgentSettings {
    /// API provider: "openrouter" or "anthropic"
    #[serde(default = "default_provider")]
    pub provider: String,
    /// API key (or use env var OPENROUTER_API_KEY / ANTHROPIC_API_KEY)
    pub api_key: Option<String>,
    /// Model to use
    #[serde(default = "default_model")]
    pub model: String,
    /// Optional Tavily API key for web search
    pub tavily_api_key: Option<String>,
}

fn default_provider() -> String {
    "openrouter".to_string()
}

fn default_model() -> String {
    "anthropic/claude-sonnet-4".to_string()
}

/// An active agent session for a user/document.
pub struct AgentSession {
    /// The agent instance.
    pub agent: Agent,
    /// Current document slug.
    pub current_slug: Arc<RwLock<Option<String>>>,
    /// User's current selection (updated from frontend).
    pub selection: Arc<RwLock<Option<Selection>>>,
}

/// Manager for agent sessions.
pub struct AgentManager {
    /// Document store for CRDT access.
    doc_store: Arc<DocStore>,
    /// Site configuration.
    site_config: Arc<monowiki_core::Config>,
    /// Agent settings.
    settings: AgentSettings,
    /// Active sessions by session ID.
    sessions: RwLock<HashMap<String, Arc<RwLock<AgentSession>>>>,
}

impl AgentManager {
    /// Create a new agent manager.
    pub fn new(
        doc_store: Arc<DocStore>,
        site_config: Arc<monowiki_core::Config>,
        settings: AgentSettings,
    ) -> Self {
        Self {
            doc_store,
            site_config,
            settings,
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create an agent session.
    pub async fn get_or_create_session(
        &self,
        session_id: &str,
    ) -> Result<Arc<RwLock<AgentSession>>> {
        // Check if session exists
        {
            let sessions = self.sessions.read().await;
            if let Some(session) = sessions.get(session_id) {
                return Ok(session.clone());
            }
        }

        // Create new session
        let session = self.create_session().await?;
        let session = Arc::new(RwLock::new(session));

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.to_string(), session.clone());

        Ok(session)
    }

    /// Create a new agent session.
    async fn create_session(&self) -> Result<AgentSession> {
        let api_key = self
            .settings
            .api_key
            .clone()
            .or_else(|| {
                if self.settings.provider == "anthropic" {
                    std::env::var("ANTHROPIC_API_KEY").ok()
                } else {
                    std::env::var("OPENROUTER_API_KEY").ok()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("No API key configured for agent"))?;

        let client: Arc<dyn ApiClient> = if self.settings.provider == "anthropic" {
            Arc::new(AnthropicClient::new(api_key, self.settings.model.clone()))
        } else {
            Arc::new(OpenRouterClient::new(api_key, self.settings.model.clone()))
        };

        let current_slug = Arc::new(RwLock::new(None));
        let selection = Arc::new(RwLock::new(None));

        // Create document operations implementation
        let doc_ops = Arc::new(CollabDocumentOps {
            doc_store: self.doc_store.clone(),
            site_config: self.site_config.clone(),
            current_slug: current_slug.clone(),
            selection: selection.clone(),
        });

        // Create tool registry with document tools
        let mut tools = monowiki_agent::ToolRegistry::with_file_tools(self.site_config.vault_dir());

        // Add document-specific tools
        for tool in monowiki_agent::document::create_document_tools(doc_ops, current_slug.clone()) {
            tools.register(tool);
        }

        // Add web search if configured
        let tavily_key = self
            .settings
            .tavily_api_key
            .clone()
            .or_else(|| std::env::var("TAVILY_API_KEY").ok());
        if let Some(key) = tavily_key {
            tools.register(Box::new(monowiki_agent::tools::WebSearchTool::new(key)));
        }
        tools.register(Box::new(monowiki_agent::tools::FetchUrlTool::new()));

        let config = AgentConfig {
            api_key: String::new(), // Already in client
            model: self.settings.model.clone(),
            base_url: None,
            working_dir: self.site_config.vault_dir(),
            max_context_tokens: 100_000,
            checkpoint_interval: 10,
            tavily_api_key: None,
        };

        let agent = Agent::with_tools(client, config, tools);

        Ok(AgentSession {
            agent,
            current_slug,
            selection,
        })
    }

    /// Remove a session.
    pub async fn remove_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
    }

    /// Update the current document for a session.
    pub async fn set_current_document(&self, session_id: &str, slug: &str) -> Result<()> {
        let session = self.get_or_create_session(session_id).await?;
        let session = session.read().await;
        *session.current_slug.write().await = Some(slug.to_string());
        Ok(())
    }

    /// Update the selection for a session.
    pub async fn set_selection(
        &self,
        session_id: &str,
        selection: Option<Selection>,
    ) -> Result<()> {
        let session = self.get_or_create_session(session_id).await?;
        let session = session.read().await;
        *session.selection.write().await = selection;
        Ok(())
    }
}

/// Implementation of DocumentOperations backed by the CRDT DocStore.
struct CollabDocumentOps {
    doc_store: Arc<DocStore>,
    site_config: Arc<monowiki_core::Config>,
    current_slug: Arc<RwLock<Option<String>>>,
    selection: Arc<RwLock<Option<Selection>>>,
}

#[async_trait::async_trait]
impl DocumentOperations for CollabDocumentOps {
    async fn get_context(&self, slug: &str) -> Result<DocumentContext, ToolError> {
        let doc = self
            .doc_store
            .get_or_load(slug, &self.site_config)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let content = doc.to_markdown();
        let blocks: Vec<BlockInfo> = doc
            .get_blocks()
            .into_iter()
            .map(|b| BlockInfo {
                id: b.id,
                kind: b.kind,
                text: b.text,
            })
            .collect();

        let selection = self.selection.read().await.clone();

        Ok(DocumentContext {
            slug: slug.to_string(),
            content,
            selection,
            blocks,
        })
    }

    async fn get_selection(&self, _slug: &str) -> Result<Option<Selection>, ToolError> {
        Ok(self.selection.read().await.clone())
    }

    async fn replace_range(
        &self,
        slug: &str,
        block_id: &str,
        start: usize,
        end: usize,
        new_text: &str,
    ) -> Result<(), ToolError> {
        let doc = self
            .doc_store
            .get_or_load(slug, &self.site_config)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        // Delete the old range
        let len = end.saturating_sub(start);
        if len > 0 {
            doc.delete_block_text(block_id, start, len)
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        }

        // Insert the new text
        if !new_text.is_empty() {
            doc.insert_block_text(block_id, start, new_text)
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        }

        Ok(())
    }

    async fn insert_text(
        &self,
        slug: &str,
        block_id: &str,
        offset: usize,
        text: &str,
    ) -> Result<(), ToolError> {
        let doc = self
            .doc_store
            .get_or_load(slug, &self.site_config)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        doc.insert_block_text(block_id, offset, text)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(())
    }

    async fn add_comment(
        &self,
        slug: &str,
        block_id: &str,
        start: usize,
        end: usize,
        content: &str,
        author: &str,
    ) -> Result<String, ToolError> {
        let doc = self
            .doc_store
            .get_or_load(slug, &self.site_config)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let comment_id = doc
            .add_comment(block_id, start, end, content, author)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(comment_id)
    }

    async fn get_comments(&self, slug: &str) -> Result<Vec<AgentComment>, ToolError> {
        let doc = self
            .doc_store
            .get_or_load(slug, &self.site_config)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let crdt_comments = doc.get_all_comments();

        // Convert from CRDT Comment to agent Comment
        let comments = crdt_comments
            .into_iter()
            .map(|c| AgentComment {
                id: c.id,
                range: monowiki_agent::document::TextRange {
                    block_id: c.block_id,
                    start: c.start,
                    end: c.end,
                },
                content: c.content,
                author: c.author,
                created_at: c.created_at,
                resolved: c.resolved,
            })
            .collect();

        Ok(comments)
    }

    async fn resolve_comment(&self, slug: &str, comment_id: &str) -> Result<(), ToolError> {
        let doc = self
            .doc_store
            .get_or_load(slug, &self.site_config)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        doc.resolve_comment(comment_id)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(())
    }

    async fn read_document(&self, slug: &str) -> Result<String, ToolError> {
        let doc = self
            .doc_store
            .get_or_load(slug, &self.site_config)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(doc.to_markdown())
    }

    async fn search_vault(&self, query: &str) -> Result<Vec<SearchResult>, ToolError> {
        let vault_dir = self.site_config.vault_dir();
        let mut results = Vec::new();
        let query_lower = query.to_lowercase();

        // First search loaded docs (live CRDT state)
        {
            let docs = self.doc_store.loaded_docs().await;
            for (slug, doc) in docs {
                if let Ok((fm, body)) = doc.snapshot() {
                    let content = format_frontmatter_body(&fm, &body);
                    accumulate_match(&slug, &content, &query_lower, &mut results);
                }
            }
        }

        // Then scan disk for any other markdown files
        fn search_dir(
            dir: &std::path::Path,
            base: &std::path::Path,
            query: &str,
            results: &mut Vec<SearchResult>,
        ) -> std::io::Result<()> {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !name.starts_with('.') {
                        search_dir(&path, base, query, results)?;
                    }
                } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let rel_path = path.strip_prefix(base).unwrap_or(&path);
                        let slug = rel_path.with_extension("").to_string_lossy().to_string();
                        accumulate_match(&slug, &content, query, results);
                    }
                }
            }
            Ok(())
        }

        search_dir(&vault_dir, &vault_dir, &query_lower, &mut results)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        // Sort by relevance (for now just alphabetically)
        results.sort_by(|a, b| a.slug.cmp(&b.slug));

        // Limit results
        results.truncate(20);

        Ok(results)
    }

    async fn get_links(&self, slug: &str) -> Result<LinkInfo, ToolError> {
        let doc = self
            .doc_store
            .get_or_load(slug, &self.site_config)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let content = doc.to_markdown();

        // Extract wiki links from content
        let outlinks = extract_wiki_links(&content);

        // Backlinks: scan both live docs and disk to find links to this slug
        let target = slug.trim_matches('/');
        let mut backlinks = Vec::new();

        // Live docs
        {
            let docs = self.doc_store.loaded_docs().await;
            for (s, d) in docs {
                if s == slug {
                    continue;
                }
                if let Ok((_fm, body)) = d.snapshot() {
                    if extract_wiki_links(&body).iter().any(|l| l == target) {
                        backlinks.push(s);
                    }
                }
            }
        }

        // Disk docs
        fn scan_for_backlinks(
            dir: &std::path::Path,
            base: &std::path::Path,
            target: &str,
            acc: &mut Vec<String>,
        ) -> std::io::Result<()> {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !name.starts_with('.') {
                        scan_for_backlinks(&path, base, target, acc)?;
                    }
                } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if extract_wiki_links(&content).iter().any(|l| l == target) {
                            let rel = path.strip_prefix(base).unwrap_or(&path);
                            let slug = rel.with_extension("").to_string_lossy().to_string();
                            acc.push(slug);
                        }
                    }
                }
            }
            Ok(())
        }

        scan_for_backlinks(
            &self.site_config.vault_dir(),
            &self.site_config.vault_dir(),
            target,
            &mut backlinks,
        )
        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(LinkInfo {
            outlinks,
            backlinks,
        })
    }
}

fn format_frontmatter_body(fm: &serde_json::Value, body: &str) -> String {
    let yaml = serde_yaml::to_string(fm).unwrap_or_default();
    format!("---\n{}---\n{}", yaml, body)
}

fn accumulate_match(slug: &str, content: &str, query_lower: &str, results: &mut Vec<SearchResult>) {
    let content_lower = content.to_lowercase();
    if content_lower.contains(query_lower) {
        // Extract title from frontmatter or first heading
        let title = extract_title(content).unwrap_or_else(|| slug.to_string());

        // Find snippet around match
        let snippet = if let Some(pos) = content_lower.find(query_lower) {
            let start = pos.saturating_sub(50);
            let end = (pos + query_lower.len() + 50).min(content.len());
            format!("...{}...", &content[start..end])
        } else {
            content.chars().take(100).collect()
        };

        results.push(SearchResult {
            slug: slug.to_string(),
            title,
            snippet,
            score: 1.0,
        });
    }
}

/// Extract title from document content.
fn extract_title(content: &str) -> Option<String> {
    // Try frontmatter title
    if content.starts_with("---") {
        if let Some(end) = content[3..].find("---") {
            let yaml = &content[3..3 + end];
            for line in yaml.lines() {
                if let Some(title) = line.strip_prefix("title:") {
                    return Some(
                        title
                            .trim()
                            .trim_matches('"')
                            .trim_matches('\'')
                            .to_string(),
                    );
                }
            }
        }
    }

    // Try first heading
    for line in content.lines() {
        if let Some(heading) = line.strip_prefix("# ") {
            return Some(heading.trim().to_string());
        }
    }

    None
}

/// Extract wiki links from content.
fn extract_wiki_links(content: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut chars = content.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '[' && chars.peek() == Some(&'[') {
            chars.next(); // consume second '['
            let mut link = String::new();

            while let Some(c) = chars.next() {
                if c == ']' && chars.peek() == Some(&']') {
                    chars.next(); // consume second ']'
                                  // Handle aliases: [[target|alias]]
                    let target = link.split('|').next().unwrap_or(&link).to_string();
                    if !target.is_empty() {
                        links.push(target);
                    }
                    break;
                }
                link.push(c);
            }
        }
    }

    links
}

/// Request to ask the agent a question.
#[derive(Debug, Deserialize)]
pub struct AskRequest {
    /// The user's question or request.
    pub query: String,
    /// Current document slug.
    pub slug: String,
    /// Current selection (if any).
    pub selection: Option<Selection>,
}

/// Response from the agent.
#[derive(Debug, Serialize)]
pub struct AskResponse {
    /// The agent's response text.
    pub response: String,
    /// Whether the agent made any edits.
    pub made_edits: bool,
    /// Comments added by the agent.
    pub comments_added: Vec<String>,
}

/// Streaming event from the agent.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentStreamEvent {
    /// Agent is thinking.
    Thinking,
    /// Text chunk from agent.
    Text { content: String },
    /// Agent is calling a tool.
    ToolCall { name: String },
    /// Tool execution result.
    ToolResult { name: String, success: bool },
    /// Agent finished.
    Done { response: String },
    /// Error occurred.
    Error { message: String },
}
