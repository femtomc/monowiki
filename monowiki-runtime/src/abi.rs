//! Render-time ABI type definitions
//!
//! This module defines the types used in the WASM runtime ABI for live cells.
//! These types match the WIT interface definitions and provide safe Rust bindings.

use serde::{Deserialize, Serialize};

/// Position in source document (line, column)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

impl Position {
    pub fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }
}

/// Span in source document
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl Span {
    pub fn new(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        Self {
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    pub fn from_positions(start: Position, end: Position) -> Self {
        Self {
            start_line: start.line,
            start_col: start.column,
            end_line: end.line,
            end_col: end.column,
        }
    }
}

/// Diagnostic severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

/// HTTP request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

/// HTTP response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Capability types for resource access
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    /// Read document content
    Read,
    /// Modify document content
    Write,
    /// Network access (HTTP fetch)
    Network,
    /// UI widget creation
    Ui,
    /// Diagnostic/decoration publishing
    Diagnostics,
    /// Dataspace access
    Dataspace,
}

/// Capability set for a live cell
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Capabilities {
    pub read: bool,
    pub write: bool,
    pub network: bool,
    pub ui: bool,
    pub diagnostics: bool,
    pub dataspace: bool,
}

impl Capabilities {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_read(mut self) -> Self {
        self.read = true;
        self
    }

    pub fn with_write(mut self) -> Self {
        self.write = true;
        self
    }

    pub fn with_network(mut self) -> Self {
        self.network = true;
        self
    }

    pub fn with_ui(mut self) -> Self {
        self.ui = true;
        self
    }

    pub fn with_diagnostics(mut self) -> Self {
        self.diagnostics = true;
        self
    }

    pub fn with_dataspace(mut self) -> Self {
        self.dataspace = true;
        self
    }

    pub fn has(&self, cap: Capability) -> bool {
        match cap {
            Capability::Read => self.read,
            Capability::Write => self.write,
            Capability::Network => self.network,
            Capability::Ui => self.ui,
            Capability::Diagnostics => self.diagnostics,
            Capability::Dataspace => self.dataspace,
        }
    }
}

/// Runtime limits for WASM execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeLimits {
    /// Maximum memory in bytes (default: 16MB)
    pub max_memory: usize,
    /// Maximum execution time in milliseconds (default: 5000ms)
    pub max_execution_time_ms: u64,
    /// Maximum number of signals (default: 1000)
    pub max_signals: usize,
    /// Maximum number of UI widgets (default: 100)
    pub max_widgets: usize,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            max_memory: 16 * 1024 * 1024, // 16MB
            max_execution_time_ms: 5000,   // 5 seconds
            max_signals: 1000,
            max_widgets: 100,
        }
    }
}

/// Error types for runtime operations
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("Signal not found: {0}")]
    SignalNotFound(u64),

    #[error("Widget not found: {0}")]
    WidgetNotFound(u64),

    #[error("Capability denied: {0:?}")]
    CapabilityDenied(Capability),

    #[error("Memory limit exceeded: {current} > {limit}")]
    MemoryLimitExceeded { current: usize, limit: usize },

    #[error("Execution timeout: {0}ms")]
    ExecutionTimeout(u64),

    #[error("Too many signals: {0}")]
    TooManySignals(usize),

    #[error("Too many widgets: {0}")]
    TooManyWidgets(usize),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("WASM error: {0}")]
    WasmError(String),

    #[error("HTTP error: {0}")]
    HttpError(String),

    #[error("Dataspace error: {0}")]
    DataspaceError(String),
}

pub type RuntimeResult<T> = Result<T, RuntimeError>;
