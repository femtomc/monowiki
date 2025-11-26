//! Document processing pipeline connecting all crates
//!
//! Flow: Source → Parse (MRL) → Expand → Render
//!       ↑                          ↓
//!       CRDT ←─── Query ───→ Cache
//!
//! This module wires together:
//! - monowiki-mrl: Parse MRL source into shrubbery and expand to content
//! - monowiki-incremental: Memoize and cache results with dependency tracking
//! - monowiki-collab (optional): CRDT document storage for collaboration
//! - monowiki-runtime: Execute WASM cells in live documents

use monowiki_types::{BlockId, DocChange, DocId};
use std::sync::Arc;

/// The main document processing pipeline
///
/// This connects all the monowiki crates into a unified end-to-end
/// document transformation system.
pub struct DocumentPipeline {
    /// Incremental query database
    db: Arc<monowiki_incremental::Db>,
    /// Optional: CRDT document store
    #[cfg(feature = "collab")]
    doc_store: Option<Arc<monowiki_collab::DocStore>>,
}

impl DocumentPipeline {
    /// Create a new document pipeline
    pub fn new() -> Self {
        Self {
            db: Arc::new(monowiki_incremental::Db::new()),
            #[cfg(feature = "collab")]
            doc_store: None,
        }
    }

    /// Create a pipeline with access to the incremental database
    pub fn with_db(db: Arc<monowiki_incremental::Db>) -> Self {
        Self {
            db,
            #[cfg(feature = "collab")]
            doc_store: None,
        }
    }

    /// Enable collaborative editing (requires 'collab' feature)
    #[cfg(feature = "collab")]
    pub fn with_collab(mut self, store: Arc<monowiki_collab::DocStore>) -> Self {
        self.doc_store = Some(store);
        self
    }

    /// Get a reference to the incremental database
    pub fn db(&self) -> &Arc<monowiki_incremental::Db> {
        &self.db
    }

    /// Process a document from source string
    ///
    /// This is the basic pipeline without caching:
    /// 1. Tokenize MRL source
    /// 2. Parse to shrubbery AST
    /// 3. Type check
    /// 4. Expand to content
    pub fn process_source(&self, source: &str) -> Result<monowiki_mrl::Content, PipelineError> {
        // 1. Tokenize
        let tokens = monowiki_mrl::tokenize(source)
            .map_err(PipelineError::Parse)?;

        // 2. Parse to shrubbery
        let shrubbery = monowiki_mrl::parse(&tokens)
            .map_err(PipelineError::Parse)?;

        // 3. Type check
        let mut checker = monowiki_mrl::TypeChecker::new();
        checker.check(&shrubbery)
            .map_err(PipelineError::TypeCheck)?;

        // 4. Expand to content
        let mut expander = monowiki_mrl::Expander::new();
        let content = expander.expand(&shrubbery)
            .map_err(PipelineError::Expand)?;

        Ok(content)
    }

    /// Process a document with incremental caching
    ///
    /// Uses the incremental query system to memoize results.
    /// Only recomputes when the source changes.
    pub fn process_cached(&self, doc_id: &DocId, source: &str) -> Result<monowiki_mrl::Content, PipelineError> {
        use monowiki_incremental::prelude::*;
        use monowiki_incremental::queries::source::{DocumentSourceQuery, SourceStorage};

        // Set source text (input query)
        let storage = Arc::new(SourceStorage::new());
        storage.set_document(doc_id.clone(), source.to_string());
        self.db.set_any("source_storage".to_string(), Box::new(storage));

        // Query for expanded content (automatically memoized)
        let content = self.db.query::<monowiki_incremental::queries::ExpandToContentQuery>(doc_id.clone());

        Ok(content)
    }

    /// Handle a CRDT change event
    ///
    /// This invalidates affected queries in the incremental system,
    /// ensuring that subsequent queries recompute as needed.
    pub fn on_change(&self, doc_id: &DocId, change: DocChange) {
        use monowiki_incremental::InvalidationBridge;

        let bridge = InvalidationBridge::new(self.db.clone());
        bridge.on_change(doc_id, change);
    }

    /// Handle multiple CRDT changes efficiently
    pub fn on_changes(&self, doc_id: &DocId, changes: Vec<DocChange>) {
        use monowiki_incremental::InvalidationBridge;

        let bridge = InvalidationBridge::new(self.db.clone());
        bridge.on_changes(doc_id, changes);
    }

    /// Execute a document with the interpreter
    ///
    /// This runs the full MRL execution pipeline, including
    /// render-time evaluation and staged computation.
    pub fn execute(&self, source: &str) -> Result<monowiki_mrl::Content, PipelineError> {
        let mut interpreter = monowiki_mrl::Interpreter::new();
        interpreter.execute_document(source)
            .map_err(PipelineError::Execution)
    }
}

impl Default for DocumentPipeline {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur in the document pipeline
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    /// Error during MRL parsing
    #[error("Parse error: {0}")]
    Parse(#[from] monowiki_mrl::MrlError),

    /// Error during type checking
    #[error("Type check error: {0}")]
    TypeCheck(monowiki_mrl::MrlError),

    /// Error during expansion
    #[error("Expansion error: {0}")]
    Expand(monowiki_mrl::MrlError),

    /// Error during execution
    #[error("Execution error: {0}")]
    Execution(monowiki_mrl::MrlError),

    /// Error from CRDT layer
    #[error("CRDT error: {0}")]
    Crdt(#[from] anyhow::Error),

    /// Generic pipeline error
    #[error("Pipeline error: {0}")]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_document() {
        let source = r#"
# Hello World

This is a *test* document.
"#;

        let pipeline = DocumentPipeline::new();
        let content = pipeline.process_source(source).unwrap();

        // Verify we got content back
        assert!(matches!(content, monowiki_mrl::Content::Sequence(_)));
    }

    #[test]
    fn test_mrl_code() {
        let source = "!text(\"Hello, MRL!\")";

        let pipeline = DocumentPipeline::new();
        let result = pipeline.process_source(source);

        // Should parse successfully
        assert!(result.is_ok());
    }

    #[test]
    fn test_cached_pipeline() {
        let source = "# Test Document\n\nSome content.";
        let doc_id = DocId::new("test-doc");

        let pipeline = DocumentPipeline::new();

        // First query - will be computed
        let content1 = pipeline.process_cached(&doc_id, source).unwrap();

        // Second query - should be cached
        let content2 = pipeline.process_cached(&doc_id, source).unwrap();

        // Both should be the same
        assert_eq!(format!("{:?}", content1), format!("{:?}", content2));
    }

    #[test]
    fn test_invalidation() {
        let doc_id = DocId::new("test-doc");
        let pipeline = DocumentPipeline::new();

        // Make a change
        let change = DocChange::TextChanged {
            block_id: BlockId::new(1),
            start: 0,
            end: 5,
            new_text: "hello".to_string(),
        };

        // Should not panic
        pipeline.on_change(&doc_id, change);
    }

    #[test]
    fn test_execute_document() {
        let source = "!text(\"Generated content\")";

        let pipeline = DocumentPipeline::new();
        let result = pipeline.execute(source);

        // Should execute successfully
        assert!(result.is_ok());
    }
}
