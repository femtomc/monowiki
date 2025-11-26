//! Shrubbery parsing queries
//!
//! These queries parse source text into shrubbery (token trees).

use crate::durability::Durability;
use crate::invalidation::SectionId;
use crate::queries::source::SourceTextQuery;
use crate::query::{Query, QueryDatabase};

/// Query for parsing source text into shrubbery
///
/// This query depends on SourceTextQuery and produces a token tree
/// representation of the document.
pub struct ParseShrubberyQuery;

impl Query for ParseShrubberyQuery {
    type Key = SectionId;
    type Value = Shrubbery;

    fn execute<DB: QueryDatabase>(db: &DB, key: &Self::Key) -> Self::Value {
        // Get source text (creates dependency)
        let source = db.query::<SourceTextQuery>(*key);

        // Parse the source into shrubbery
        // In a real implementation, this would call monowiki_mrl::parser::parse
        parse_shrubbery(&source)
    }

    fn durability() -> Durability {
        // Parsing results change when source changes (volatile)
        Durability::Volatile
    }

    fn name() -> &'static str {
        "ParseShrubberyQuery"
    }
}

/// Simplified shrubbery representation
///
/// In the full implementation, this would be a rich token tree structure
/// with grouping, precedence information, and source locations.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Shrubbery {
    /// Token tree nodes
    pub nodes: Vec<ShrubNode>,
}

impl Shrubbery {
    /// Create empty shrubbery
    pub fn new() -> Self {
        Shrubbery { nodes: Vec::new() }
    }

    /// Create from nodes
    pub fn from_nodes(nodes: Vec<ShrubNode>) -> Self {
        Shrubbery { nodes }
    }
}

impl Default for Shrubbery {
    fn default() -> Self {
        Self::new()
    }
}

/// A node in the token tree
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ShrubNode {
    /// A text token
    Text(String),

    /// An identifier
    Identifier(String),

    /// A number literal
    Number(String),

    /// A grouped sequence (e.g., parentheses, brackets)
    Group {
        delimiter: Delimiter,
        children: Vec<ShrubNode>,
    },

    /// A macro invocation (!name)
    MacroInvoke {
        name: String,
        args: Vec<ShrubNode>,
        body: Option<Box<ShrubNode>>,
    },

    /// A heading
    Heading { level: usize, content: String },

    /// A paragraph
    Paragraph(String),

    /// A code block
    CodeBlock { lang: Option<String>, code: String },
}

/// Delimiter types for grouped sequences
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Delimiter {
    Parenthesis, // ()
    Bracket,     // []
    Brace,       // {}
}

/// Parse source text into shrubbery
///
/// This is a simplified parser for demonstration. The real implementation
/// would handle the full grammar from design.md.
fn parse_shrubbery(source: &str) -> Shrubbery {
    let mut nodes = Vec::new();

    // Very simple parsing: split by double newlines into paragraphs
    for line in source.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        // Check for heading
        if let Some(rest) = trimmed.strip_prefix('#') {
            let level = 1 + rest.chars().take_while(|c| *c == '#').count();
            let content = rest.trim_start_matches('#').trim().to_string();
            nodes.push(ShrubNode::Heading { level, content });
        }
        // Check for code block start
        else if trimmed.starts_with("```") {
            let lang = trimmed.strip_prefix("```").map(|s| s.trim().to_string());
            // In a real parser, we'd collect lines until closing ```
            nodes.push(ShrubNode::CodeBlock {
                lang: lang.filter(|s| !s.is_empty()),
                code: String::new(),
            });
        }
        // Check for macro invocation
        else if trimmed.starts_with('!') {
            if let Some(name_end) = trimmed[1..].find(|c: char| !c.is_alphanumeric() && c != '_')
            {
                let name = trimmed[1..=name_end].to_string();
                nodes.push(ShrubNode::MacroInvoke {
                    name,
                    args: Vec::new(),
                    body: None,
                });
            } else {
                let name = trimmed[1..].to_string();
                nodes.push(ShrubNode::MacroInvoke {
                    name,
                    args: Vec::new(),
                    body: None,
                });
            }
        }
        // Otherwise, treat as paragraph
        else {
            nodes.push(ShrubNode::Paragraph(trimmed.to_string()));
        }
    }

    Shrubbery::from_nodes(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::invalidation::BlockId;

    #[test]
    fn test_parse_heading() {
        let source = "# Hello World";
        let shrubbery = parse_shrubbery(source);

        assert_eq!(shrubbery.nodes.len(), 1);
        match &shrubbery.nodes[0] {
            ShrubNode::Heading { level, content } => {
                assert_eq!(*level, 1);
                assert_eq!(content, "Hello World");
            }
            _ => panic!("Expected heading"),
        }
    }

    #[test]
    fn test_parse_paragraph() {
        let source = "This is a paragraph.";
        let shrubbery = parse_shrubbery(source);

        assert_eq!(shrubbery.nodes.len(), 1);
        match &shrubbery.nodes[0] {
            ShrubNode::Paragraph(text) => {
                assert_eq!(text, "This is a paragraph.");
            }
            _ => panic!("Expected paragraph"),
        }
    }

    #[test]
    fn test_parse_query_integration() {
        let db = Db::new();

        // Set source text
        let section_id = SectionId(BlockId(1).0);
        SourceTextQuery::set(&db, section_id, "# Test\n\nParagraph.".to_string());

        // Parse it
        let shrubbery = db.query::<ParseShrubberyQuery>(section_id);

        assert_eq!(shrubbery.nodes.len(), 2);
    }

    #[test]
    fn test_parse_macro_invocation() {
        let source = "!callout";
        let shrubbery = parse_shrubbery(source);

        assert_eq!(shrubbery.nodes.len(), 1);
        match &shrubbery.nodes[0] {
            ShrubNode::MacroInvoke { name, .. } => {
                assert_eq!(name, "callout");
            }
            _ => panic!("Expected macro invocation"),
        }
    }
}
