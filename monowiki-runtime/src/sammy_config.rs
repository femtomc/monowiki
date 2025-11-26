//! Monowiki runtime configuration for sammy
//!
//! This module provides the RuntimeConfig implementation that creates
//! dataspaces according to monowiki's document model:
//!
//! - `doc-content/<doc-id>`: Document state views (backed by CRDT)
//! - `doc-view/<doc-id>`: Live cell decorations, diagnostics, eval requests/results
//! - `system`: Plugin registration, capability grants

use sammy::assertion::OrSetStore;
use sammy::dataspace::{LocalDataspace, Permissions};
use sammy::runtime::{Runtime, RuntimeConfig};
use sammy::PatternBuilder;

/// Dataspace naming conventions
pub mod names {
    /// Format a doc-content dataspace name
    pub fn doc_content(doc_id: &str) -> String {
        format!("doc-content/{}", doc_id)
    }

    /// Format a doc-view dataspace name
    pub fn doc_view(doc_id: &str) -> String {
        format!("doc-view/{}", doc_id)
    }

    /// The system dataspace name
    pub const SYSTEM: &str = "system";

    /// Check if a dataspace name is a doc-content space
    pub fn is_doc_content(name: &str) -> bool {
        name.starts_with("doc-content/")
    }

    /// Check if a dataspace name is a doc-view space
    pub fn is_doc_view(name: &str) -> bool {
        name.starts_with("doc-view/")
    }

    /// Extract doc_id from a doc-content or doc-view name
    pub fn extract_doc_id(name: &str) -> Option<&str> {
        if let Some(rest) = name.strip_prefix("doc-content/") {
            Some(rest)
        } else if let Some(rest) = name.strip_prefix("doc-view/") {
            Some(rest)
        } else {
            None
        }
    }
}

/// Runtime configuration for monowiki
///
/// Creates LocalDataspace instances with appropriate permissions
/// based on the dataspace name.
#[derive(Debug, Clone, Default)]
pub struct MonowikiConfig;

impl RuntimeConfig for MonowikiConfig {
    type Dataspace = LocalDataspace<OrSetStore>;

    fn create_dataspace(&self, name: &str) -> Self::Dataspace {
        LocalDataspace::new(name)
    }
}

/// Monowiki-specific runtime wrapper
///
/// Provides convenience methods for document-centric operations.
pub struct MonowikiRuntime {
    inner: Runtime<MonowikiConfig>,
}

impl MonowikiRuntime {
    /// Create a new monowiki runtime
    pub fn new() -> Self {
        let mut runtime = Runtime::new(MonowikiConfig);
        // Pre-create the system dataspace
        runtime.dataspace(names::SYSTEM);
        Self { inner: runtime }
    }

    /// Get or create dataspaces for a document
    ///
    /// Returns references to (doc-content, doc-view) dataspaces.
    pub fn document_dataspaces(
        &mut self,
        doc_id: &str,
    ) -> (
        std::sync::Arc<parking_lot::RwLock<LocalDataspace<OrSetStore>>>,
        std::sync::Arc<parking_lot::RwLock<LocalDataspace<OrSetStore>>>,
    ) {
        let content = self.inner.dataspace(&names::doc_content(doc_id));
        let view = self.inner.dataspace(&names::doc_view(doc_id));
        (content, view)
    }

    /// Get the system dataspace
    pub fn system_dataspace(
        &mut self,
    ) -> std::sync::Arc<parking_lot::RwLock<LocalDataspace<OrSetStore>>> {
        self.inner.dataspace(names::SYSTEM)
    }

    /// Spawn an actor for a live cell
    ///
    /// The actor is granted capabilities to:
    /// - Read/write doc-view/<doc-id>
    /// - Read doc-content/<doc-id>
    pub fn spawn_live_cell(
        &mut self,
        cell_id: &str,
        doc_id: &str,
    ) -> sammy::ActorId {
        let actor_id = self.inner.spawn_actor(format!("cell:{}/{}", doc_id, cell_id));

        // Grant full access to doc-view for this document
        self.inner.grant_capability(
            &actor_id,
            &names::doc_view(doc_id),
            Permissions::full(),
        );

        // Grant read-only access to doc-content
        self.inner.grant_capability(
            &actor_id,
            &names::doc_content(doc_id),
            Permissions::read_only(),
        );

        actor_id
    }

    /// Spawn a kernel actor
    ///
    /// Kernels get:
    /// - Read on system (to see capability grants)
    /// - Full access to all doc-view spaces (to publish results)
    pub fn spawn_kernel(&mut self, kernel_id: &str) -> sammy::ActorId {
        let actor_id = self.inner.spawn_actor(format!("kernel:{}", kernel_id));

        // Grant read access to system dataspace
        self.inner.grant_capability(
            &actor_id,
            names::SYSTEM,
            Permissions::read_only(),
        );

        actor_id
    }

    /// Grant a kernel access to a specific document's view space
    pub fn grant_kernel_doc_access(&mut self, kernel_actor: &sammy::ActorId, doc_id: &str) {
        self.inner.grant_capability(
            kernel_actor,
            &names::doc_view(doc_id),
            Permissions::full(),
        );
    }

    /// Create a capability for publishing EvalRequest
    ///
    /// Returns permissions that allow asserting EvalRequest records.
    pub fn eval_request_permissions() -> Permissions {
        Permissions {
            assert_filter: Some(PatternBuilder::record(
                "EvalRequest",
                vec![
                    PatternBuilder::wildcard(),
                    PatternBuilder::wildcard(),
                    PatternBuilder::wildcard(),
                    PatternBuilder::wildcard(),
                    PatternBuilder::wildcard(),
                ],
            )),
            observe_filter: None, // No observation
            can_retract_own: true,
            can_retract_any: false,
        }
    }

    /// Create a capability for observing EvalResult
    ///
    /// Returns permissions that allow subscribing to EvalResult for a specific cell.
    pub fn eval_result_observe_permissions(cell_id: &str, doc_id: &str) -> Permissions {
        use preserves::IOValue;

        Permissions {
            assert_filter: None, // No assertion
            observe_filter: Some(PatternBuilder::record(
                "EvalResult",
                vec![
                    PatternBuilder::literal(IOValue::new(cell_id.to_string())),
                    PatternBuilder::literal(IOValue::new(doc_id.to_string())),
                    PatternBuilder::wildcard(),
                    PatternBuilder::wildcard(),
                ],
            )),
            can_retract_own: false,
            can_retract_any: false,
        }
    }

    /// Get the inner sammy runtime
    pub fn inner(&self) -> &Runtime<MonowikiConfig> {
        &self.inner
    }

    /// Get mutable access to the inner sammy runtime
    pub fn inner_mut(&mut self) -> &mut Runtime<MonowikiConfig> {
        &mut self.inner
    }

    /// Get runtime statistics
    pub fn stats(&self) -> sammy::RuntimeStats {
        self.inner.stats()
    }
}

impl Default for MonowikiRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sammy::Dataspace;

    #[test]
    fn test_dataspace_naming() {
        assert_eq!(names::doc_content("abc"), "doc-content/abc");
        assert_eq!(names::doc_view("abc"), "doc-view/abc");
        assert!(names::is_doc_content("doc-content/foo"));
        assert!(!names::is_doc_content("doc-view/foo"));
        assert_eq!(names::extract_doc_id("doc-content/foo"), Some("foo"));
        assert_eq!(names::extract_doc_id("doc-view/bar"), Some("bar"));
        assert_eq!(names::extract_doc_id("system"), None);
    }

    #[test]
    fn test_runtime_creation() {
        let runtime = MonowikiRuntime::new();
        let stats = runtime.stats();
        // System dataspace is pre-created
        assert_eq!(stats.dataspace_count, 1);
    }

    #[test]
    fn test_document_dataspaces() {
        let mut runtime = MonowikiRuntime::new();

        let (content, view) = runtime.document_dataspaces("doc1");

        assert_eq!(content.read().name(), "doc-content/doc1");
        assert_eq!(view.read().name(), "doc-view/doc1");

        // Should have 3 dataspaces now: system, doc-content/doc1, doc-view/doc1
        assert_eq!(runtime.stats().dataspace_count, 3);
    }

    #[test]
    fn test_spawn_live_cell() {
        let mut runtime = MonowikiRuntime::new();

        // Ensure dataspaces exist first
        let _ = runtime.document_dataspaces("doc1");

        let actor_id = runtime.spawn_live_cell("cell1", "doc1");

        // Actor should exist
        let actor = runtime.inner().actor(&actor_id).unwrap();

        // Should have capabilities for doc-view and doc-content
        assert!(actor.capability("doc-view/doc1").is_some());
        assert!(actor.capability("doc-content/doc1").is_some());
    }

    #[test]
    fn test_spawn_kernel() {
        let mut runtime = MonowikiRuntime::new();

        let kernel_id = runtime.spawn_kernel("js");

        // Kernel should have system access
        let actor = runtime.inner().actor(&kernel_id).unwrap();
        assert!(actor.capability("system").is_some());
    }
}
