//! Loro CRDT implementation for operational documents.
//!
//! This module provides a Loro-based implementation of the OperationalDoc trait,
//! using MovableTree for document structure, Fugue for text sequences, and
//! Peritext-style marks for formatting.

#[cfg(feature = "loro")]
pub mod doc;

#[cfg(feature = "loro")]
pub use doc::LoroOperationalDoc;
