//! Standard queries for the document pipeline
//!
//! This module provides the standard query implementations for transforming
//! documents through the read → parse → expand → layout pipeline.

pub mod source;
pub mod parse;
pub mod expand;
pub mod layout;

// Re-export common types
pub use source::SourceTextQuery;
pub use parse::ParseShrubberyQuery;
pub use expand::{ActiveMacrosQuery, ExpandToContentQuery};
pub use layout::LayoutSectionQuery;
