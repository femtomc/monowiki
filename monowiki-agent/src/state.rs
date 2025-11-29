//! Session state management and checkpointing.

use crate::types::Message;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StateError {
    #[error("Failed to read checkpoint: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("Failed to parse checkpoint: {0}")]
    ParseError(#[from] serde_json::Error),
}

/// Statistics for the current session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionStats {
    /// Number of prompt tokens used.
    pub prompt_tokens: usize,
    /// Number of completion tokens used.
    pub completion_tokens: usize,
    /// Total tokens used.
    pub total_tokens: usize,
    /// Number of tool calls made.
    pub tool_calls: usize,
    /// Number of API calls made.
    pub api_calls: usize,
    /// Number of errors recovered from.
    pub errors_recovered: usize,
    /// Estimated total cost in USD.
    pub total_cost: f64,
}

impl SessionStats {
    /// Create new empty stats.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record token usage from an API response.
    pub fn record_usage(&mut self, prompt_tokens: usize, completion_tokens: usize) {
        self.prompt_tokens += prompt_tokens;
        self.completion_tokens += completion_tokens;
        self.total_tokens += prompt_tokens + completion_tokens;
        self.api_calls += 1;
    }

    /// Record a tool call.
    pub fn record_tool_call(&mut self) {
        self.tool_calls += 1;
    }

    /// Record an error recovery.
    pub fn record_error(&mut self) {
        self.errors_recovered += 1;
    }

    /// Update cost estimate (can be called with actual cost from API).
    pub fn update_cost(&mut self, cost: f64) {
        self.total_cost = cost;
    }

    /// Estimate cost based on token counts (rough estimate).
    /// Uses approximate Claude Sonnet pricing: $3/1M input, $15/1M output.
    pub fn estimate_cost(&self) -> f64 {
        let input_cost = (self.prompt_tokens as f64 / 1_000_000.0) * 3.0;
        let output_cost = (self.completion_tokens as f64 / 1_000_000.0) * 15.0;
        input_cost + output_cost
    }
}

/// A checkpoint for resuming interrupted sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// The original query that started this session.
    pub query: String,
    /// Message history.
    pub messages: Vec<Message>,
    /// Session statistics.
    pub stats: SessionStats,
    /// Working directory.
    pub working_dir: String,
    /// Model being used.
    pub model: String,
    /// When the checkpoint was created.
    pub timestamp: DateTime<Utc>,
    /// Version of the checkpoint format.
    pub version: u32,
}

impl Checkpoint {
    /// Current checkpoint format version.
    pub const VERSION: u32 = 1;

    /// Create a new checkpoint.
    pub fn new(
        query: String,
        messages: Vec<Message>,
        stats: SessionStats,
        working_dir: String,
        model: String,
    ) -> Self {
        Self {
            query,
            messages,
            stats,
            working_dir,
            model,
            timestamp: Utc::now(),
            version: Self::VERSION,
        }
    }

    /// Save checkpoint to a file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), StateError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load checkpoint from a file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, StateError> {
        let json = std::fs::read_to_string(path)?;
        let checkpoint: Self = serde_json::from_str(&json)?;
        Ok(checkpoint)
    }

    /// Get the default checkpoint path for a working directory.
    pub fn default_path(working_dir: impl AsRef<Path>) -> std::path::PathBuf {
        working_dir.as_ref().join(".monowiki-agent/checkpoint.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_stats_recording() {
        let mut stats = SessionStats::new();
        stats.record_usage(100, 50);
        stats.record_tool_call();
        stats.record_error();

        assert_eq!(stats.prompt_tokens, 100);
        assert_eq!(stats.completion_tokens, 50);
        assert_eq!(stats.total_tokens, 150);
        assert_eq!(stats.tool_calls, 1);
        assert_eq!(stats.api_calls, 1);
        assert_eq!(stats.errors_recovered, 1);
    }

    #[test]
    fn test_checkpoint_save_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("checkpoint.json");

        let checkpoint = Checkpoint::new(
            "test query".to_string(),
            vec![Message::user("hello")],
            SessionStats::new(),
            "/tmp".to_string(),
            "test-model".to_string(),
        );

        checkpoint.save(&path).unwrap();
        let loaded = Checkpoint::load(&path).unwrap();

        assert_eq!(loaded.query, "test query");
        assert_eq!(loaded.model, "test-model");
    }
}
