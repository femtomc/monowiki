//! # monowiki-core
//!
//! Core library for the monowiki static site generator.
//!
//! This crate provides the fundamental building blocks for parsing markdown,
//! managing site configuration, and building the content model.

pub mod bibliography;
pub mod builder;
pub mod config;
pub mod frontmatter;
pub mod markdown;
pub mod models;
pub mod search;
pub mod slug;
// pub mod artifacts;
// pub mod assets;
// pub mod cleanup;

pub use bibliography::{Bibliography, BibliographyStore};
pub use builder::SiteBuilder;
pub use config::Config;
pub use models::{
    Comment, CommentStatus, Diagnostic, DiagnosticSeverity, Frontmatter, LinkGraph, Note, NoteType,
    SiteIndex,
};
pub use search::SectionDigest;
pub use search::{build_search_index, SearchEntry};
pub use slug::slugify;
