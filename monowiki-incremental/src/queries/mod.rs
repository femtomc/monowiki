//! Standard queries for the document pipeline
//!
//! This module provides the standard query implementations for transforming
//! documents through the read → parse → expand → layout pipeline.

pub mod expand;
pub mod layout;
pub mod parse;
pub mod source;

pub use expand::{ActiveMacrosQuery, ExpandResult, ExpandToContentQuery, MacroConfig};
pub use layout::{ActiveStylesQuery, Layout, LayoutBox, LayoutDocumentQuery, LayoutKind, StyleConfig, Viewport};
pub use parse::{ParseResult, ParseShrubberyQuery};
pub use source::{BlockSourceQuery, DocumentSourceQuery, SourceStorage};
