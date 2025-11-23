//! # monowiki-render
//!
//! Template rendering library for monowiki.
//!
//! This crate handles HTML template rendering using Askama.

pub mod templates;

pub use templates::{
    Author, BacklinkEntry, IndexTemplate, NotFoundTemplate, NoteEntry, Paper, PostTemplate,
};
