//! Source text queries
//!
//! These queries provide access to the raw text content from the CRDT layer.

use crate::durability::Durability;
use crate::invalidation::SectionId;
use crate::query::{InputQuery, Query, QueryDatabase};

/// Query for getting source text from a section
///
/// This is an input query that reads from the CRDT operational layer.
/// It's the root of the document pipeline dependency graph.
pub struct SourceTextQuery;

impl Query for SourceTextQuery {
    type Key = SectionId;
    type Value = String;

    fn execute<DB: QueryDatabase>(db: &DB, key: &Self::Key) -> Self::Value {
        // In a real implementation, this would read from the CRDT layer
        // For now, return a placeholder that can be set via set_input
        //
        // Example:
        // db.get_operational_doc()
        //    .get_block_text(*key)
        //    .unwrap_or_default()

        // For now, try to get from dynamic storage
        let key_str = format!("source_text_{}", key.0);
        if let Some(value) = db.get_any(&key_str) {
            if let Some(text) = value.downcast_ref::<String>() {
                return text.clone();
            }
        }

        String::new()
    }

    fn durability() -> Durability {
        // Source text changes on every edit
        Durability::Volatile
    }

    fn name() -> &'static str {
        "SourceTextQuery"
    }
}

impl InputQuery for SourceTextQuery {
    fn set<DB: QueryDatabase>(db: &DB, key: Self::Key, value: Self::Value) {
        // Store in dynamic storage for retrieval
        let key_str = format!("source_text_{}", key.0);
        db.set_any(key_str, Box::new(value));
    }
}

/// Query for getting the full document structure
///
/// This provides a view of the entire document tree from the CRDT layer.
pub struct DocTreeQuery;

impl Query for DocTreeQuery {
    type Key = ();
    type Value = DocTree;

    fn execute<DB: QueryDatabase>(_db: &DB, _key: &Self::Key) -> Self::Value {
        // In a real implementation, this would construct a tree from CRDT state
        // For now, return an empty tree
        DocTree::default()
    }

    fn durability() -> Durability {
        Durability::Volatile
    }

    fn name() -> &'static str {
        "DocTreeQuery"
    }
}

/// Represents the document tree structure
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocTree {
    /// Root sections in the document
    pub sections: Vec<SectionId>,

    /// Map of section ID to its children
    pub children: std::collections::HashMap<SectionId, Vec<SectionId>>,
}

// Manual Hash implementation for DocTree
impl std::hash::Hash for DocTree {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.sections.hash(state);
        // Hash HashMap as a sorted vec of entries
        let mut entries: Vec<_> = self.children.iter().collect();
        entries.sort_by_key(|(k, _)| *k);
        for (k, v) in entries {
            k.hash(state);
            v.hash(state);
        }
    }
}

impl DocTree {
    /// Create a new empty document tree
    pub fn new() -> Self {
        DocTree {
            sections: Vec::new(),
            children: std::collections::HashMap::new(),
        }
    }

    /// Add a root section
    pub fn add_section(&mut self, section_id: SectionId) {
        self.sections.push(section_id);
    }

    /// Add a child section
    pub fn add_child(&mut self, parent: SectionId, child: SectionId) {
        self.children.entry(parent).or_insert_with(Vec::new).push(child);
    }

    /// Get all sections in depth-first order
    pub fn sections_dfs(&self) -> Vec<SectionId> {
        let mut result = Vec::new();
        for section in &self.sections {
            self.visit_section(*section, &mut result);
        }
        result
    }

    fn visit_section(&self, section: SectionId, result: &mut Vec<SectionId>) {
        result.push(section);
        if let Some(children) = self.children.get(&section) {
            for child in children {
                self.visit_section(*child, result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::invalidation::BlockId;

    #[test]
    fn test_source_text_query() {
        let db = Db::new();

        // Set some source text
        let section_id = SectionId(BlockId(1).0);
        SourceTextQuery::set(&db, section_id, "# Hello World".to_string());

        // Query it back
        let text = db.query::<SourceTextQuery>(section_id);
        assert_eq!(text, "# Hello World");
    }

    #[test]
    fn test_doc_tree() {
        let mut tree = DocTree::new();

        let sec1 = SectionId(1);
        let sec2 = SectionId(2);
        let sec3 = SectionId(3);

        tree.add_section(sec1);
        tree.add_child(sec1, sec2);
        tree.add_child(sec1, sec3);

        let sections = tree.sections_dfs();
        assert_eq!(sections, vec![sec1, sec2, sec3]);
    }
}
