//! Kernel actor for processing live cell evaluations
//!
//! This module provides kernel actors that subscribe to EvalRequest assertions
//! and publish EvalResult responses. Kernels can either:
//! - Execute WASM modules directly
//! - Interpret source code (for language-specific kernels)

use crate::abi::Capabilities;
use crate::engine::LiveCellEngine;
use crate::host::RuntimeHost;
use crate::sammy_config::names;
use crate::schemas::{EvalPayload, EvalRequest, EvalResult};
use preserves::IOValue;
use sammy::actor::{Entity, EntityContext};
use sammy::assertion::OrSetStore;
use sammy::dataspace::LocalDataspace;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Extract record fields from an IOValue
/// Returns (label, fields) if the value is a record, None otherwise
fn extract_record_fields(value: &IOValue) -> Option<(String, Vec<IOValue>)> {
    if !value.is_record() {
        return None;
    }
    let label = value
        .label()
        .as_symbol()
        .map(|sym| sym.as_ref().to_string())
        .unwrap_or_else(|| "<record>".to_string());
    let fields: Vec<IOValue> = value.iter().map(IOValue::from).collect();
    Some((label, fields))
}

/// A WASM kernel that executes compiled WebAssembly modules
///
/// This kernel receives EvalRequest assertions containing WASM bytecode,
/// executes them in a sandboxed environment, and publishes EvalResult
/// assertions with the output.
pub struct WasmKernel {
    /// Kernel identifier
    kernel_id: String,
    /// The WASM execution engine
    engine: Arc<LiveCellEngine>,
    /// Execution timeout
    timeout: Duration,
    /// Pending results to publish (handle -> result)
    pending_results: Vec<EvalResult>,
}

impl WasmKernel {
    /// Create a new WASM kernel
    pub fn new(kernel_id: impl Into<String>) -> Self {
        Self {
            kernel_id: kernel_id.into(),
            engine: Arc::new(LiveCellEngine::new().expect("Failed to create WASM engine")),
            timeout: Duration::from_secs(30),
            pending_results: Vec::new(),
        }
    }

    /// Create with a custom timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Create with a shared engine
    pub fn with_engine(mut self, engine: Arc<LiveCellEngine>) -> Self {
        self.engine = engine;
        self
    }

    /// Process an evaluation request
    fn process_request(&mut self, request: &EvalRequest) -> EvalResult {
        let start = Instant::now();

        match &request.payload {
            EvalPayload::Wasm(wasm_bytes) => {
                // Create a host with full capabilities for the kernel
                let host = RuntimeHost::new(Capabilities::all());

                match self.engine.instantiate(wasm_bytes, host) {
                    Ok(mut instance) => {
                        // Execute with timeout check
                        if start.elapsed() > self.timeout {
                            return EvalResult::timeout(
                                request.cell_id.clone(),
                                request.doc_id.clone(),
                                request.seq,
                            );
                        }

                        match instance.run() {
                            Ok(()) => {
                                // Collect output from the host
                                let outputs = instance.host_mut().take_output();
                                let output = if outputs.is_empty() {
                                    Vec::new()
                                } else {
                                    // Concatenate all outputs
                                    outputs.into_iter().flatten().collect()
                                };

                                EvalResult::success(
                                    request.cell_id.clone(),
                                    request.doc_id.clone(),
                                    request.seq,
                                    output,
                                )
                            }
                            Err(e) => EvalResult::error(
                                request.cell_id.clone(),
                                request.doc_id.clone(),
                                request.seq,
                                format!("Execution error: {}", e),
                            ),
                        }
                    }
                    Err(e) => EvalResult::error(
                        request.cell_id.clone(),
                        request.doc_id.clone(),
                        request.seq,
                        format!("Instantiation error: {}", e),
                    ),
                }
            }
            EvalPayload::Source(source) => {
                // WASM kernel doesn't handle source directly
                // Return an error suggesting the wrong kernel was used
                EvalResult::error(
                    request.cell_id.clone(),
                    request.doc_id.clone(),
                    request.seq,
                    format!(
                        "WASM kernel '{}' cannot evaluate source code directly. \
                         Use a language-specific kernel or compile to WASM first.",
                        self.kernel_id
                    ),
                )
            }
        }
    }

    /// Take pending results for publishing
    pub fn take_pending_results(&mut self) -> Vec<EvalResult> {
        std::mem::take(&mut self.pending_results)
    }

    /// Get the kernel ID
    pub fn kernel_id(&self) -> &str {
        &self.kernel_id
    }
}

impl Entity<LocalDataspace<OrSetStore>> for WasmKernel {
    fn on_assert(
        &mut self,
        ctx: &mut EntityContext<LocalDataspace<OrSetStore>>,
        value: &IOValue,
    ) {
        // Try to parse the EvalRequest from the IOValue
        // The value should be a record <EvalRequest kernel_id json>
        if let Some((_label, fields)) = extract_record_fields(value) {
            if fields.len() >= 2 {
                // fields[0] is kernel_id, fields[1] is JSON
                if let Some(json_str) = fields[1].as_string() {
                    if let Some(request) = EvalRequest::from_json(&json_str) {
                        // Verify this request is for our kernel
                        if request.kernel_id == self.kernel_id {
                            tracing::debug!(
                                kernel = %self.kernel_id,
                                cell = %request.cell_id,
                                doc = %request.doc_id,
                                seq = request.seq,
                                "Processing eval request"
                            );

                            // Process the request
                            let result = self.process_request(&request);

                            // Publish result to doc-view dataspace
                            let doc_view = names::doc_view(&request.doc_id);
                            match ctx.assert(&doc_view, result.to_iovalue()) {
                                Ok(_handle) => {
                                    tracing::debug!(
                                        kernel = %self.kernel_id,
                                        cell = %request.cell_id,
                                        "Published eval result"
                                    );
                                }
                                Err(e) => {
                                    tracing::error!(
                                        kernel = %self.kernel_id,
                                        error = ?e,
                                        "Failed to publish eval result"
                                    );
                                    // Store for later retrieval
                                    self.pending_results.push(result);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn on_retract(
        &mut self,
        _ctx: &mut EntityContext<LocalDataspace<OrSetStore>>,
        _value: &IOValue,
    ) {
        // Retractions of EvalRequest don't require any action
        // The result has already been published
    }

    fn on_stop(&mut self, _ctx: &mut EntityContext<LocalDataspace<OrSetStore>>) {
        tracing::info!(kernel = %self.kernel_id, "Kernel stopping");
    }

    fn type_name(&self) -> &'static str {
        "WasmKernel"
    }
}

/// A source interpreter kernel base trait
///
/// Language-specific kernels can implement this to handle EvalPayload::Source
pub trait SourceKernel: Send + Sync {
    /// Evaluate source code and return bytes output
    fn evaluate(&self, source: &str) -> Result<Vec<u8>, String>;

    /// Get the kernel ID
    fn kernel_id(&self) -> &str;

    /// Get the language this kernel supports
    fn language(&self) -> &str;
}

/// Adapter to turn a SourceKernel into an Entity
pub struct SourceKernelEntity<K: SourceKernel> {
    inner: K,
}

impl<K: SourceKernel> SourceKernelEntity<K> {
    pub fn new(kernel: K) -> Self {
        Self { inner: kernel }
    }
}

impl<K: SourceKernel + 'static> Entity<LocalDataspace<OrSetStore>> for SourceKernelEntity<K> {
    fn on_assert(
        &mut self,
        ctx: &mut EntityContext<LocalDataspace<OrSetStore>>,
        value: &IOValue,
    ) {
        if let Some((_label, fields)) = extract_record_fields(value) {
            if fields.len() >= 2 {
                if let Some(json_str) = fields[1].as_string() {
                    if let Some(request) = EvalRequest::from_json(&json_str) {
                        if request.kernel_id == self.inner.kernel_id() {
                            let result = match &request.payload {
                                EvalPayload::Source(source) => {
                                    match self.inner.evaluate(source) {
                                        Ok(output) => EvalResult::success(
                                            request.cell_id.clone(),
                                            request.doc_id.clone(),
                                            request.seq,
                                            output,
                                        ),
                                        Err(e) => EvalResult::error(
                                            request.cell_id.clone(),
                                            request.doc_id.clone(),
                                            request.seq,
                                            e,
                                        ),
                                    }
                                }
                                EvalPayload::Wasm(_) => EvalResult::error(
                                    request.cell_id.clone(),
                                    request.doc_id.clone(),
                                    request.seq,
                                    format!(
                                        "Source kernel '{}' cannot execute WASM directly",
                                        self.inner.kernel_id()
                                    ),
                                ),
                            };

                            let doc_view = names::doc_view(&request.doc_id);
                            if let Err(e) = ctx.assert(&doc_view, result.to_iovalue()) {
                                tracing::error!(
                                    kernel = %self.inner.kernel_id(),
                                    error = ?e,
                                    "Failed to publish eval result"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    fn type_name(&self) -> &'static str {
        "SourceKernelEntity"
    }
}

/// A simple echo kernel for testing
///
/// Returns the input source/wasm as output.
pub struct EchoKernel {
    kernel_id: String,
}

impl EchoKernel {
    pub fn new(kernel_id: impl Into<String>) -> Self {
        Self {
            kernel_id: kernel_id.into(),
        }
    }
}

impl Entity<LocalDataspace<OrSetStore>> for EchoKernel {
    fn on_assert(
        &mut self,
        ctx: &mut EntityContext<LocalDataspace<OrSetStore>>,
        value: &IOValue,
    ) {
        if let Some((_label, fields)) = extract_record_fields(value) {
            if fields.len() >= 2 {
                if let Some(json_str) = fields[1].as_string() {
                    if let Some(request) = EvalRequest::from_json(&json_str) {
                        if request.kernel_id == self.kernel_id {
                            let output = match &request.payload {
                                EvalPayload::Wasm(bytes) => bytes.clone(),
                                EvalPayload::Source(s) => s.as_bytes().to_vec(),
                            };

                            let result = EvalResult::success(
                                request.cell_id.clone(),
                                request.doc_id.clone(),
                                request.seq,
                                output,
                            );

                            let doc_view = names::doc_view(&request.doc_id);
                            let _ = ctx.assert(&doc_view, result.to_iovalue());
                        }
                    }
                }
            }
        }
    }

    fn type_name(&self) -> &'static str {
        "EchoKernel"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schemas::{eval_request_pattern, EvalResultKind};
    use sammy::dataspace::Permissions;
    use std::collections::HashMap;

    fn create_test_dataspace() -> Arc<parking_lot::RwLock<LocalDataspace<OrSetStore>>> {
        Arc::new(parking_lot::RwLock::new(LocalDataspace::new("test")))
    }

    #[test]
    fn test_wasm_kernel_creation() {
        let kernel = WasmKernel::new("wasm");
        assert_eq!(kernel.kernel_id(), "wasm");
    }

    #[test]
    fn test_wasm_kernel_source_error() {
        let mut kernel = WasmKernel::new("wasm");

        let request = EvalRequest::source(
            "wasm".to_string(),
            "cell1".to_string(),
            "doc1".to_string(),
            "console.log('hi')".to_string(),
            1,
        );

        let result = kernel.process_request(&request);
        assert!(matches!(result.result, EvalResultKind::Error(_)));
    }

    #[test]
    fn test_echo_kernel() {
        let kernel = EchoKernel::new("echo");

        // Create a minimal test context
        let doc_view_ds = create_test_dataspace();
        let mut caps = HashMap::new();

        // Add doc-view capability
        let doc_view_ref = sammy::dataspace::DataspaceRef::new(
            "doc-view/doc1",
            doc_view_ds,
            Permissions::full(),
        );
        caps.insert("doc-view/doc1".to_string(), doc_view_ref);

        let facet_id = sammy::types::FacetId::root(sammy::types::ActorId::new("test"));
        let mut ctx = EntityContext::new(facet_id, &mut caps);

        // Create an EvalRequest
        let request = EvalRequest::source(
            "echo".to_string(),
            "cell1".to_string(),
            "doc1".to_string(),
            "hello world".to_string(),
            1,
        );

        // Wrap kernel in a mutable ref to test Entity impl
        let mut kernel = kernel;
        kernel.on_assert(&mut ctx, &request.to_iovalue());

        // The result should be in the doc-view dataspace
        let results = caps
            .get("doc-view/doc1")
            .unwrap()
            .query(&sammy::PatternBuilder::wildcard())
            .unwrap();

        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_eval_request_pattern_matching() {
        let pattern = eval_request_pattern("wasm");

        let request = EvalRequest::source(
            "wasm".to_string(),
            "cell1".to_string(),
            "doc1".to_string(),
            "test".to_string(),
            1,
        );

        let iovalue = request.to_iovalue();

        // The pattern should match
        assert!(pattern.matches_tagged(&iovalue));

        // Different kernel should not match
        let other_request = EvalRequest::source(
            "js".to_string(),
            "cell1".to_string(),
            "doc1".to_string(),
            "test".to_string(),
            1,
        );

        assert!(!pattern.matches_tagged(&other_request.to_iovalue()));
    }
}
