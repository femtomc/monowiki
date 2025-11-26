//! Monowiki Incremental Computation Engine
//!
//! This crate provides a Salsa-inspired incremental computation system
//! for the monowiki document pipeline. It enables efficient, demand-driven
//! computation with memoization and early cutoff optimization.
//!
//! # Architecture
//!
//! The system is organized around **queries** - pure, memoized functions
//! that transform data. Queries track their dependencies automatically,
//! enabling precise invalidation when inputs change.
//!
//! ## Document Pipeline
//!
//! The standard document transformation pipeline:
//!
//! ```text
//! source_text → parse_shrubbery → expand_to_content → layout_section → render
//! ```
//!
//! Each stage is a query that depends on previous stages. When source
//! text changes, only affected downstream queries are recomputed.
//!
//! ## Key Features
//!
//! - **Dependency Tracking**: Queries automatically track what they depend on
//! - **Memoization**: Results are cached and reused when inputs haven't changed
//! - **Early Cutoff**: If a recomputed value is unchanged, downstream queries aren't invalidated
//! - **Durability Tiers**: Queries are organized by expected change frequency
//! - **Content-Addressable Caching**: Results can be cached across sessions
//! - **CRDT Integration**: Changes from the CRDT layer trigger precise invalidation
//!
//! # Example
//!
//! ```rust
//! use monowiki_incremental::{Db, Query, QueryDatabase};
//! use monowiki_incremental::queries::{SourceTextQuery, ParseShrubberyQuery};
//! use monowiki_incremental::invalidation::{SectionId, BlockId};
//!
//! // Create a database
//! let db = Db::new();
//!
//! // Set some source text
//! let section_id = SectionId(BlockId(1).0);
//! SourceTextQuery::set(&db, section_id, "# Hello World".to_string());
//!
//! // Parse it (automatically memoized)
//! let shrubbery = db.query::<ParseShrubberyQuery>(section_id);
//!
//! // Second query uses cached result
//! let shrubbery2 = db.query::<ParseShrubberyQuery>(section_id);
//! ```

#![warn(missing_docs)]
#![warn(missing_debug_implementations)]

// Core modules
pub mod db;
pub mod durability;
pub mod query;
pub mod memo;
pub mod cache;
pub mod invalidation;
pub mod metrics;

// Standard queries
pub mod queries;

// Re-export main types
pub use cache::{CacheError, CacheKey, CacheStats, ContentCache};
pub use db::Db;
pub use durability::Durability;
pub use invalidation::InvalidationBridge;
pub use memo::{MemoEntry, MemoStorage, MemoTable};
pub use metrics::{MetricsSnapshot, QueryMetrics};
pub use monowiki_types::{BlockId, DocChange, DocId};
pub use queries::SourceStorage;
pub use query::{hash_value, InputQuery, Query, QueryDatabase, QueryKey, Revision};

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::db::Db;
    pub use crate::durability::Durability;
    pub use crate::invalidation::InvalidationBridge;
    pub use crate::queries::{
        ActiveMacrosQuery, DocumentSourceQuery, ExpandToContentQuery, LayoutSectionQuery,
        ParseShrubberyQuery, SourceStorage,
    };
    pub use crate::query::{InputQuery, Query, QueryDatabase};
    pub use monowiki_types::{BlockId, DocChange, DocId};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_usage() {
        let db = Db::new();
        assert_eq!(db.revision(), Revision(1));
    }
}
