//! # monowiki-adapters
//!
//! Code documentation adapters for extracting docs from source code.

use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AdapterError {
    #[error("Failed to read source file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse source: {0}")]
    ParseError(String),
}

/// Trait for documentation adapters
pub trait DocAdapter {
    /// Name of this adapter (e.g., "python", "typescript")
    fn name(&self) -> &str;

    /// Extract documentation from source files
    ///
    /// Returns a list of (output_path, markdown_content) tuples
    fn extract(
        &self,
        source_path: &Path,
        repo_url: Option<&str>,
    ) -> Result<Vec<(PathBuf, String)>, AdapterError>;
}

// Placeholder for future adapters
// pub mod python;
// pub mod typescript;
