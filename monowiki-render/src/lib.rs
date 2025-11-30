//! # monowiki-render
//!
//! Template rendering library for monowiki.
//!
//! This crate handles HTML template rendering using Askama.

pub mod templates;

pub use templates::{
    Author, BacklinkEntry, CommentRender, DirectoryNode, FileNode, IndexTemplate, NotFoundTemplate,
    NoteEntry, Paper, PostTemplate,
};
