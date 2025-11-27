//! Standard queries for the document pipeline
//!
//! This module provides the standard query implementations for transforming
//! documents through the read → parse → expand → layout pipeline.
//!
//! ## Query Granularity
//!
//! Each stage provides both document-level and block-level queries:
//!
//! - **Document-level**: Process entire documents (e.g., `ParseShrubberyQuery`)
//! - **Block-level**: Process individual blocks for fine-grained invalidation
//!   (e.g., `ParseBlockQuery`)
//!
//! When a block changes, only that block's queries need to re-run, rather than
//! reprocessing the entire document.

pub mod expand;
pub mod layout;
pub mod parse;
pub mod source;

// Document-level queries
pub use expand::{ActiveMacrosQuery, ExpandResult, ExpandToContentQuery, MacroConfig};
pub use layout::{ActiveStylesQuery, Layout, LayoutBox, LayoutDocumentQuery, LayoutKind, StyleConfig, Viewport};
pub use parse::{ParseResult, ParseShrubberyQuery};
pub use source::{BlockSourceQuery, DocumentSourceQuery, SourceStorage};

// Block-level queries (for fine-grained invalidation)
pub use expand::ExpandBlockQuery;
pub use layout::LayoutBlockQuery;
pub use parse::ParseBlockQuery;
