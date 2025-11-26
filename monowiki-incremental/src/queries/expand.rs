//! Content expansion queries
//!
//! These queries expand shrubbery into typed Content structures.

use crate::durability::Durability;
use crate::invalidation::SectionId;
use crate::queries::parse::{ParseShrubberyQuery, Shrubbery, ShrubNode};
use crate::query::{Query, QueryDatabase};
use std::collections::HashMap;

/// Query for getting active macros
///
/// This provides the macro environment for expansion.
pub struct ActiveMacrosQuery;

impl Query for ActiveMacrosQuery {
    type Key = ();
    type Value = MacroEnv;

    fn execute<DB: QueryDatabase>(_db: &DB, _key: &Self::Key) -> Self::Value {
        // In a real implementation, this would load macros from:
        // - Built-in standard library
        // - User-defined macros
        // - Plugin-provided macros
        MacroEnv::default()
    }

    fn durability() -> Durability {
        // Macros change less frequently (durable tier)
        Durability::Durable
    }

    fn name() -> &'static str {
        "ActiveMacrosQuery"
    }
}

/// Macro environment for expansion
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MacroEnv {
    /// Registered macros by name
    pub macros: HashMap<String, MacroDef>,
}

// Manual Hash implementation for MacroEnv
impl std::hash::Hash for MacroEnv {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash HashMap as a sorted vec of entries
        let mut entries: Vec<_> = self.macros.iter().collect();
        entries.sort_by_key(|(k, _)| k.as_str());
        for (k, v) in entries {
            k.hash(state);
            v.hash(state);
        }
    }
}

impl MacroEnv {
    /// Create a new empty macro environment
    pub fn new() -> Self {
        MacroEnv {
            macros: HashMap::new(),
        }
    }

    /// Register a macro
    pub fn register(&mut self, name: String, def: MacroDef) {
        self.macros.insert(name, def);
    }

    /// Get a macro definition
    pub fn get(&self, name: &str) -> Option<&MacroDef> {
        self.macros.get(name)
    }
}

/// A macro definition
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MacroDef {
    /// Name of the macro
    pub name: String,

    /// Expected content kind
    pub kind: ContentKind,
}

/// Kind of content produced by a macro
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentKind {
    Block,
    Inline,
    Any,
}

/// Query for expanding shrubbery to Content
pub struct ExpandToContentQuery;

impl Query for ExpandToContentQuery {
    type Key = SectionId;
    type Value = Content;

    fn execute<DB: QueryDatabase>(db: &DB, key: &Self::Key) -> Self::Value {
        // Get parsed shrubbery (creates dependency)
        let shrubbery = db.query::<ParseShrubberyQuery>(*key);

        // Get active macros (creates dependency)
        let macros = db.query::<ActiveMacrosQuery>(());

        // Expand shrubbery to Content
        expand_content(&shrubbery, &macros)
    }

    fn durability() -> Durability {
        // Content changes when source or macros change (volatile)
        Durability::Volatile
    }

    fn name() -> &'static str {
        "ExpandToContentQuery"
    }
}

/// Content representation (typed document tree)
///
/// This is the semantic representation from design.md.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Content {
    /// Content elements
    pub elements: Vec<ContentElement>,
}

impl Content {
    /// Create empty content
    pub fn new() -> Self {
        Content {
            elements: Vec::new(),
        }
    }

    /// Create from elements
    pub fn from_elements(elements: Vec<ContentElement>) -> Self {
        Content { elements }
    }

    /// Add an element
    pub fn push(&mut self, element: ContentElement) {
        self.elements.push(element);
    }

    /// Compose two content values
    pub fn compose(mut self, other: Content) -> Self {
        self.elements.extend(other.elements);
        self
    }
}

impl Default for Content {
    fn default() -> Self {
        Self::new()
    }
}

/// A content element (block or inline)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ContentElement {
    /// Block-level elements
    Block(BlockElement),

    /// Inline-level elements
    Inline(InlineElement),
}

/// Block-level content elements
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BlockElement {
    Heading {
        level: usize,
        body: Vec<InlineElement>,
    },
    Paragraph {
        body: Vec<InlineElement>,
    },
    CodeBlock {
        lang: Option<String>,
        code: String,
        attrs: Vec<(String, String)>, // Changed from HashMap to Vec for Hash compatibility
    },
    List {
        items: Vec<ListItem>,
    },
    Blockquote {
        body: Box<Content>,
    },
    ThematicBreak,
}

/// Inline-level content elements
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InlineElement {
    Text(String),
    Emphasis { body: Vec<InlineElement> },
    Strong { body: Vec<InlineElement> },
    Code(String),
    Link { body: Vec<InlineElement>, url: String },
    Math(String),
}

/// A list item
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ListItem {
    pub body: Vec<InlineElement>,
    pub children: Vec<BlockElement>,
}

/// Expand shrubbery to Content
fn expand_content(shrubbery: &Shrubbery, _macros: &MacroEnv) -> Content {
    let mut elements = Vec::new();

    for node in &shrubbery.nodes {
        match node {
            ShrubNode::Heading { level, content } => {
                elements.push(ContentElement::Block(BlockElement::Heading {
                    level: *level,
                    body: vec![InlineElement::Text(content.clone())],
                }));
            }

            ShrubNode::Paragraph(text) => {
                elements.push(ContentElement::Block(BlockElement::Paragraph {
                    body: vec![InlineElement::Text(text.clone())],
                }));
            }

            ShrubNode::CodeBlock { lang, code } => {
                elements.push(ContentElement::Block(BlockElement::CodeBlock {
                    lang: lang.clone(),
                    code: code.clone(),
                    attrs: Vec::new(),
                }));
            }

            ShrubNode::MacroInvoke { name, .. } => {
                // In a real implementation, we'd expand the macro
                // For now, just create a paragraph mentioning it
                elements.push(ContentElement::Block(BlockElement::Paragraph {
                    body: vec![InlineElement::Text(format!("[Macro: {}]", name))],
                }));
            }

            _ => {
                // Handle other node types
            }
        }
    }

    Content::from_elements(elements)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::invalidation::BlockId;
    use crate::queries::source::SourceTextQuery;

    #[test]
    fn test_expand_heading() {
        let db = Db::new();

        // Set up a simple document
        let section_id = SectionId(BlockId(1).0);
        SourceTextQuery::set(&db, section_id, "# Test Heading".to_string());

        // Expand to content
        let content = db.query::<ExpandToContentQuery>(section_id);

        assert_eq!(content.elements.len(), 1);
        match &content.elements[0] {
            ContentElement::Block(BlockElement::Heading { level, body }) => {
                assert_eq!(*level, 1);
                assert_eq!(body.len(), 1);
            }
            _ => panic!("Expected heading"),
        }
    }

    #[test]
    fn test_expand_paragraph() {
        let db = Db::new();

        let section_id = SectionId(BlockId(1).0);
        SourceTextQuery::set(&db, section_id, "This is a test.".to_string());

        let content = db.query::<ExpandToContentQuery>(section_id);

        assert_eq!(content.elements.len(), 1);
        match &content.elements[0] {
            ContentElement::Block(BlockElement::Paragraph { body }) => {
                assert_eq!(body.len(), 1);
            }
            _ => panic!("Expected paragraph"),
        }
    }

    #[test]
    fn test_macro_env() {
        let mut env = MacroEnv::new();

        let def = MacroDef {
            name: "test".to_string(),
            kind: ContentKind::Block,
        };

        env.register("test".to_string(), def.clone());

        assert_eq!(env.get("test"), Some(&def));
        assert_eq!(env.get("nonexistent"), None);
    }
}
