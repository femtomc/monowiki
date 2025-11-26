//! Assertion schemas for monowiki dataspaces
//!
//! Defines the typed assertion formats for doc-view and system dataspaces.
//! These schemas use serde for serialization, wrapped in preserves IOValue.

use preserves::IOValue;
use serde::{Deserialize, Serialize};

/// Document ID type
pub type DocId = String;

/// Kernel ID type
pub type KernelId = String;

/// Cell ID within a document
pub type CellId = String;

// =============================================================================
// doc-view/<doc-id> assertions
// =============================================================================

/// A decoration applied to a span of document content
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Decoration {
    /// Start offset in the document
    pub start: usize,
    /// End offset in the document
    pub end: usize,
    /// CSS class to apply
    pub class: String,
    /// Optional tooltip text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
}

impl Decoration {
    pub fn new(start: usize, end: usize, class: impl Into<String>) -> Self {
        Self {
            start,
            end,
            class: class.into(),
            tooltip: None,
        }
    }

    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Convert to preserves IOValue
    pub fn to_iovalue(&self) -> IOValue {
        // Serialize as a tagged record: <Decoration start end class tooltip>
        let json = serde_json::to_string(self).unwrap_or_default();
        IOValue::record(
            IOValue::symbol("Decoration"),
            vec![IOValue::new(json)],
        )
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

/// Severity level for diagnostics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

/// A diagnostic message for a document location
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiagnosticAssertion {
    /// Start offset
    pub start: usize,
    /// End offset
    pub end: usize,
    /// Severity level
    pub severity: DiagnosticSeverity,
    /// Message text
    pub message: String,
    /// Source of the diagnostic (e.g., "mrl", "kernel:js")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl DiagnosticAssertion {
    pub fn new(
        start: usize,
        end: usize,
        severity: DiagnosticSeverity,
        message: impl Into<String>,
    ) -> Self {
        Self {
            start,
            end,
            severity,
            message: message.into(),
            source: None,
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn to_iovalue(&self) -> IOValue {
        let json = serde_json::to_string(self).unwrap_or_default();
        IOValue::record(
            IOValue::symbol("Diagnostic"),
            vec![IOValue::new(json)],
        )
    }

    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

/// Request to evaluate a live cell
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalRequest {
    /// The kernel to evaluate with
    pub kernel_id: KernelId,
    /// Cell identifier
    pub cell_id: CellId,
    /// Document ID
    pub doc_id: DocId,
    /// The code/WASM to evaluate
    pub payload: EvalPayload,
    /// Request sequence number (for ordering)
    pub seq: u64,
}

/// Payload for evaluation request
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum EvalPayload {
    /// WASM bytecode to execute (base64 encoded in JSON)
    #[serde(rename = "wasm")]
    Wasm(Vec<u8>),
    /// Source code string (for interpreted kernels)
    #[serde(rename = "source")]
    Source(String),
}

impl EvalRequest {
    pub fn wasm(kernel_id: KernelId, cell_id: CellId, doc_id: DocId, wasm: Vec<u8>, seq: u64) -> Self {
        Self {
            kernel_id,
            cell_id,
            doc_id,
            payload: EvalPayload::Wasm(wasm),
            seq,
        }
    }

    pub fn source(kernel_id: KernelId, cell_id: CellId, doc_id: DocId, source: String, seq: u64) -> Self {
        Self {
            kernel_id,
            cell_id,
            doc_id,
            payload: EvalPayload::Source(source),
            seq,
        }
    }

    pub fn to_iovalue(&self) -> IOValue {
        let json = serde_json::to_string(self).unwrap_or_default();
        IOValue::record(
            IOValue::symbol("EvalRequest"),
            vec![
                IOValue::new(self.kernel_id.clone()), // For pattern matching
                IOValue::new(json),
            ],
        )
    }

    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

/// Result from evaluating a live cell
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalResult {
    /// Cell identifier (matches request)
    pub cell_id: CellId,
    /// Document ID (matches request)
    pub doc_id: DocId,
    /// Request sequence number (matches request)
    pub seq: u64,
    /// The evaluation result
    pub result: EvalResultKind,
}

/// Kind of evaluation result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum EvalResultKind {
    /// Successful evaluation with output value
    #[serde(rename = "success")]
    Success(Vec<u8>),
    /// Evaluation error
    #[serde(rename = "error")]
    Error(String),
    /// Evaluation timed out
    #[serde(rename = "timeout")]
    Timeout,
}

impl EvalResult {
    pub fn success(cell_id: CellId, doc_id: DocId, seq: u64, output: Vec<u8>) -> Self {
        Self {
            cell_id,
            doc_id,
            seq,
            result: EvalResultKind::Success(output),
        }
    }

    pub fn error(cell_id: CellId, doc_id: DocId, seq: u64, message: String) -> Self {
        Self {
            cell_id,
            doc_id,
            seq,
            result: EvalResultKind::Error(message),
        }
    }

    pub fn timeout(cell_id: CellId, doc_id: DocId, seq: u64) -> Self {
        Self {
            cell_id,
            doc_id,
            seq,
            result: EvalResultKind::Timeout,
        }
    }

    pub fn to_iovalue(&self) -> IOValue {
        let json = serde_json::to_string(self).unwrap_or_default();
        IOValue::record(
            IOValue::symbol("EvalResult"),
            vec![
                IOValue::new(self.cell_id.clone()), // For pattern matching
                IOValue::new(self.doc_id.clone()),  // For pattern matching
                IOValue::new(json),
            ],
        )
    }

    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

// =============================================================================
// system dataspace assertions
// =============================================================================

/// A registered plugin/kernel
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginRegistered {
    /// Kernel identifier
    pub kernel_id: KernelId,
    /// Human-readable name
    pub name: String,
    /// Supported language/format
    pub language: String,
    /// Capabilities this kernel provides
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl PluginRegistered {
    pub fn new(kernel_id: KernelId, name: String, language: String) -> Self {
        Self {
            kernel_id,
            name,
            language,
            capabilities: vec![],
        }
    }

    pub fn with_capabilities(mut self, caps: Vec<String>) -> Self {
        self.capabilities = caps;
        self
    }

    pub fn to_iovalue(&self) -> IOValue {
        let json = serde_json::to_string(self).unwrap_or_default();
        IOValue::record(
            IOValue::symbol("PluginRegistered"),
            vec![
                IOValue::new(self.kernel_id.clone()), // For pattern matching
                IOValue::new(json),
            ],
        )
    }

    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

/// A capability grant for an actor
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilityGrant {
    /// Target actor/cell ID
    pub target: String,
    /// Granted capability name
    pub capability: String,
    /// Optional restrictions/parameters
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restrictions: Option<serde_json::Value>,
}

impl CapabilityGrant {
    pub fn new(target: String, capability: String) -> Self {
        Self {
            target,
            capability,
            restrictions: None,
        }
    }

    pub fn with_restrictions(mut self, restrictions: serde_json::Value) -> Self {
        self.restrictions = Some(restrictions);
        self
    }

    pub fn to_iovalue(&self) -> IOValue {
        let json = serde_json::to_string(self).unwrap_or_default();
        IOValue::record(
            IOValue::symbol("CapabilityGrant"),
            vec![IOValue::new(json)],
        )
    }

    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

// =============================================================================
// Pattern helpers for subscription
// =============================================================================

/// Build a pattern for matching EvalRequest for a specific kernel
pub fn eval_request_pattern(kernel_id: &str) -> sammy::Pattern {
    sammy::PatternBuilder::record(
        "EvalRequest",
        vec![
            sammy::PatternBuilder::literal(IOValue::new(kernel_id.to_string())),
            sammy::PatternBuilder::wildcard(), // JSON payload
        ],
    )
}

/// Build a pattern for matching EvalResult for a specific cell
pub fn eval_result_pattern(cell_id: &str, doc_id: &str) -> sammy::Pattern {
    sammy::PatternBuilder::record(
        "EvalResult",
        vec![
            sammy::PatternBuilder::literal(IOValue::new(cell_id.to_string())),
            sammy::PatternBuilder::literal(IOValue::new(doc_id.to_string())),
            sammy::PatternBuilder::wildcard(), // JSON payload
        ],
    )
}

/// Build a pattern for matching any EvalResult
pub fn any_eval_result_pattern() -> sammy::Pattern {
    sammy::PatternBuilder::record(
        "EvalResult",
        vec![
            sammy::PatternBuilder::wildcard(),
            sammy::PatternBuilder::wildcard(),
            sammy::PatternBuilder::wildcard(),
        ],
    )
}

/// Build a pattern for matching PluginRegistered for a specific kernel
pub fn plugin_by_kernel_pattern(kernel_id: &str) -> sammy::Pattern {
    sammy::PatternBuilder::record(
        "PluginRegistered",
        vec![
            sammy::PatternBuilder::literal(IOValue::new(kernel_id.to_string())),
            sammy::PatternBuilder::wildcard(),
        ],
    )
}

/// Build a pattern for matching any PluginRegistered
pub fn any_plugin_pattern() -> sammy::Pattern {
    sammy::PatternBuilder::record(
        "PluginRegistered",
        vec![
            sammy::PatternBuilder::wildcard(),
            sammy::PatternBuilder::wildcard(),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoration_serde() {
        let dec = Decoration::new(10, 20, "highlight")
            .with_tooltip("Test tooltip");

        let json = serde_json::to_string(&dec).unwrap();
        let parsed: Decoration = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.start, 10);
        assert_eq!(parsed.end, 20);
        assert_eq!(parsed.class, "highlight");
        assert_eq!(parsed.tooltip, Some("Test tooltip".to_string()));
    }

    #[test]
    fn test_diagnostic_serde() {
        let diag = DiagnosticAssertion::new(0, 10, DiagnosticSeverity::Error, "Test error")
            .with_source("mrl");

        let json = serde_json::to_string(&diag).unwrap();
        let parsed: DiagnosticAssertion = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.start, 0);
        assert_eq!(parsed.end, 10);
        assert_eq!(parsed.severity, DiagnosticSeverity::Error);
        assert_eq!(parsed.message, "Test error");
        assert_eq!(parsed.source, Some("mrl".to_string()));
    }

    #[test]
    fn test_eval_request_serde() {
        let req = EvalRequest::source(
            "js".to_string(),
            "cell1".to_string(),
            "doc1".to_string(),
            "console.log('hello')".to_string(),
            42,
        );

        let json = serde_json::to_string(&req).unwrap();
        let parsed: EvalRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.kernel_id, "js");
        assert_eq!(parsed.cell_id, "cell1");
        assert_eq!(parsed.seq, 42);
    }

    #[test]
    fn test_eval_result_serde() {
        let result = EvalResult::success(
            "cell1".to_string(),
            "doc1".to_string(),
            42,
            b"output".to_vec(),
        );

        let json = serde_json::to_string(&result).unwrap();
        let parsed: EvalResult = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.cell_id, "cell1");
        assert_eq!(parsed.seq, 42);
    }

    #[test]
    fn test_plugin_registered_serde() {
        let plugin = PluginRegistered::new(
            "js-kernel".to_string(),
            "JavaScript".to_string(),
            "javascript".to_string(),
        )
        .with_capabilities(vec!["eval".to_string(), "async".to_string()]);

        let json = serde_json::to_string(&plugin).unwrap();
        let parsed: PluginRegistered = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.kernel_id, "js-kernel");
        assert_eq!(parsed.language, "javascript");
        assert_eq!(parsed.capabilities, vec!["eval", "async"]);
    }
}
