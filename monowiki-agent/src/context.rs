//! Context window management for long sessions.

use crate::types::Message;

/// Manager for context window compression.
pub struct ContextManager {
    /// Maximum tokens before triggering compression.
    max_tokens: usize,
    /// Target utilization (0.0-1.0) before compressing.
    compression_threshold: f64,
    /// Number of recent messages to always keep.
    keep_recent: usize,
}

impl ContextManager {
    /// Create a new context manager.
    pub fn new(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            compression_threshold: 0.7,
            keep_recent: 10,
        }
    }

    /// Create with custom settings.
    pub fn with_settings(
        max_tokens: usize,
        compression_threshold: f64,
        keep_recent: usize,
    ) -> Self {
        Self {
            max_tokens,
            compression_threshold,
            keep_recent,
        }
    }

    /// Estimate total tokens in the message history.
    pub fn estimate_tokens(&self, messages: &[Message]) -> usize {
        messages.iter().map(|m| m.estimate_tokens()).sum()
    }

    /// Check if compression is needed.
    pub fn needs_compression(&self, messages: &[Message]) -> bool {
        let tokens = self.estimate_tokens(messages);
        let threshold = (self.max_tokens as f64 * self.compression_threshold) as usize;
        tokens > threshold
    }

    /// Compress the message history by summarizing older messages.
    ///
    /// Returns a new message history with older messages replaced by a summary.
    /// The system message (if any) and recent messages are preserved.
    pub fn compress(&self, messages: &[Message]) -> (Vec<Message>, Option<String>) {
        if messages.len() <= self.keep_recent + 1 {
            return (messages.to_vec(), None);
        }

        let mut result = Vec::new();
        let mut summary_parts = Vec::new();

        // Find and preserve system message
        let mut start_idx = 0;
        if let Some(first) = messages.first() {
            if matches!(first.role, crate::types::Role::System) {
                result.push(first.clone());
                start_idx = 1;
            }
        }

        // Calculate how many messages to summarize
        let messages_to_keep = self.keep_recent;
        let total_after_system = messages.len() - start_idx;
        let messages_to_summarize = total_after_system.saturating_sub(messages_to_keep);

        if messages_to_summarize == 0 {
            return (messages.to_vec(), None);
        }

        // Build summary of older messages
        for msg in &messages[start_idx..start_idx + messages_to_summarize] {
            match msg.role {
                crate::types::Role::User => {
                    if let Some(text) = msg.text() {
                        let truncated = truncate_str(text, 200);
                        summary_parts.push(format!("User: {}", truncated));
                    }
                }
                crate::types::Role::Assistant => {
                    if let Some(ref tool_calls) = msg.tool_calls {
                        let names: Vec<_> = tool_calls
                            .iter()
                            .map(|tc| tc.function.name.as_str())
                            .collect();
                        summary_parts.push(format!("Assistant called tools: {}", names.join(", ")));
                    } else if let Some(text) = msg.text() {
                        let truncated = truncate_str(text, 200);
                        summary_parts.push(format!("Assistant: {}", truncated));
                    }
                }
                crate::types::Role::Tool => {
                    // Summarize tool results very briefly
                    if let Some(text) = msg.text() {
                        let truncated = truncate_str(text, 100);
                        summary_parts.push(format!("Tool result: {}", truncated));
                    }
                }
                _ => {}
            }
        }

        // Create summary message
        let summary = format!(
            "[Context Summary - {} earlier messages]\n\n{}",
            messages_to_summarize,
            summary_parts.join("\n")
        );

        result.push(Message::user(format!(
            "[The following is a summary of earlier conversation]\n{}",
            summary
        )));

        // Add recent messages
        for msg in &messages[start_idx + messages_to_summarize..] {
            result.push(msg.clone());
        }

        (result, Some(summary))
    }

    /// Truncate tool results that are too long.
    pub fn truncate_tool_result(content: &str, max_chars: usize) -> String {
        if content.len() <= max_chars {
            content.to_string()
        } else {
            format!(
                "{}\n\n[Truncated: {} chars total, showing first {}]",
                &content[..max_chars],
                content.len(),
                max_chars
            )
        }
    }
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new(100_000)
    }
}

/// Truncate a string to a maximum length, adding ellipsis if needed.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        let manager = ContextManager::new(1000);
        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Hello, world!"),
        ];

        let tokens = manager.estimate_tokens(&messages);
        assert!(tokens > 0);
    }

    #[test]
    fn test_needs_compression() {
        let manager = ContextManager::new(100); // Very small for testing
        let messages = vec![
            Message::system("x".repeat(200)), // 50 tokens
            Message::user("y".repeat(200)),   // 50 tokens
        ];

        assert!(manager.needs_compression(&messages));
    }

    #[test]
    fn test_compress_preserves_recent() {
        let manager = ContextManager::with_settings(1000, 0.5, 2);
        let messages = vec![
            Message::system("System prompt"),
            Message::user("Old message 1"),
            Message::assistant("Old response 1"),
            Message::user("Recent message 1"),
            Message::assistant("Recent response 1"),
        ];

        let (compressed, _summary) = manager.compress(&messages);

        // Should have: system + summary + 2 recent pairs
        assert!(compressed.len() <= 5);
        // Last message should be preserved
        assert_eq!(compressed.last().unwrap().text(), Some("Recent response 1"));
    }

    #[test]
    fn test_truncate_tool_result() {
        let long_content = "x".repeat(1000);
        let truncated = ContextManager::truncate_tool_result(&long_content, 100);

        assert!(truncated.len() < 200); // Should be truncated with message
        assert!(truncated.contains("[Truncated:"));
    }
}
