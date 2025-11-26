//! Monowiki Runtime - Render-time WASM execution for live cells
//!
//! This crate provides the runtime infrastructure for executing live cells in monowiki documents.
//! It includes:
//!
//! - **Reactive signals**: Primitive for reactive computation with automatic dependency tracking
//! - **UI widgets**: Host-side widget management (sliders, text inputs, buttons)
//! - **Diagnostics**: Error/warning reporting and document decorations
//! - **Dataspace integration**: Actor communication via publish/subscribe patterns
//! - **WASM emitter**: Minimal bytecode emitter for simple expressions
//! - **Interpreter**: Fallback interpreter for trivial expressions
//!
//! ## Architecture
//!
//! The runtime follows monowiki's three-phase execution model:
//!
//! 1. **Read-time**: Parse source into shrubbery (handled by parser crate)
//! 2. **Expand-time**: Macro expansion, type checking (handled by MRL crate)
//! 3. **Render-time**: Live cell execution (THIS CRATE)
//!
//! ## Live Cells
//!
//! Live cells are reactive code blocks that execute in the browser. They can:
//!
//! - Create and manipulate reactive signals
//! - Render UI widgets
//! - Emit diagnostics and decorations
//! - Communicate via dataspaces
//! - Make HTTP requests (if capability granted)
//!
//! ## Example
//!
//! ```rust
//! use monowiki_runtime::{RuntimeHost, Capabilities};
//!
//! // Create a runtime host with UI capabilities
//! let mut host = RuntimeHost::new(
//!     Capabilities::new().with_ui().with_diagnostics()
//! );
//!
//! // Create a signal
//! let signal_id = host.signal_create(b"42").unwrap();
//!
//! // Create a slider widget
//! let slider_id = host.ui_slider(0.0, 100.0, 50.0).unwrap();
//!
//! // Process pending signal updates
//! let updates = host.process_signals();
//! ```
//!
//! ## Capabilities
//!
//! The runtime uses a capability-based security model. Live cells must explicitly
//! request capabilities for:
//!
//! - `read`: Document content access (always granted)
//! - `write`: Document modification
//! - `network`: HTTP fetch
//! - `ui`: Widget creation
//! - `diagnostics`: Diagnostic/decoration publishing
//! - `dataspace`: Dataspace pub/sub
//!
//! ## WASM Integration
//!
//! Live cells compile to WASM modules that import host functions via WIT interfaces.
//! The runtime provides two execution strategies:
//!
//! 1. **WASM emitter**: For simple expressions, directly emit WASM bytecode
//! 2. **Interpreter**: For complex expressions, use a fallback interpreter
//!
//! ## Safety
//!
//! All WASM modules run in a sandbox with:
//!
//! - Memory limits (default: 16MB)
//! - Execution timeout (default: 5 seconds)
//! - Capability checks for privileged operations
//! - No direct host memory access

pub mod abi;
pub mod dataspace;
pub mod diagnostics;
pub mod emitter;
pub mod engine;
pub mod host;
pub mod interpreter;
pub mod signals;
pub mod ui;

// Re-export main types
pub use abi::{
    Capabilities, Capability, HttpRequest, HttpResponse, Position, RuntimeError, RuntimeLimits,
    RuntimeResult, Severity, Span,
};

pub use dataspace::{Assertion, DataspaceClient, Subscription};

pub use diagnostics::{Decoration, Diagnostic, DiagnosticCollector};

pub use emitter::{
    ExportKind, FuncBody, FuncType, Instruction, ValType, WasmEmitter,
};

pub use engine::{LiveCellEngine, LiveCellInstance};

pub use host::RuntimeHost;

pub use interpreter::{BinOp, Interpreter, SimpleExpr, Stmt, UnOp, Value};

pub use signals::{Signal, SignalStore};

pub use ui::{Widget, WidgetStore};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_host_creation() {
        let host = RuntimeHost::with_default_capabilities();
        assert!(host.capabilities.has(Capability::Ui));
        assert!(host.capabilities.has(Capability::Diagnostics));
    }

    #[test]
    fn test_signal_flow() {
        let mut host = RuntimeHost::with_default_capabilities();

        // Create a signal
        let id = host.signal_create(b"initial").unwrap();

        // Get the value
        let value = host.signal_get(id).unwrap();
        assert_eq!(value, b"initial");

        // Update the value
        host.signal_set(id, b"updated").unwrap();

        // Subscribe a callback
        host.signal_subscribe(id, 100).unwrap();

        // Process pending updates
        let updates = host.process_signals();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].0, id);
        assert_eq!(updates[0].1, b"updated");
        assert_eq!(updates[0].2, vec![100]);
    }

    #[test]
    fn test_ui_widget_creation() {
        let mut host = RuntimeHost::new(Capabilities::new().with_ui());

        let slider = host.ui_slider(0.0, 100.0, 50.0).unwrap();
        assert!(slider > 0);

        let text = host.ui_text_input("placeholder", "initial").unwrap();
        assert!(text > 0);

        let button = host.ui_button("Click me").unwrap();
        assert!(button > 0);
    }

    #[test]
    fn test_diagnostic_emission() {
        let mut host = RuntimeHost::new(Capabilities::new().with_diagnostics());

        let span = Span::new(1, 0, 1, 10);
        host.emit_diagnostic(Severity::Error, span, "Test error")
            .unwrap();

        assert_eq!(host.diagnostics.diagnostic_count(), 1);
        assert!(host.diagnostics.has_errors());
    }

    #[test]
    fn test_interpreter_basic() {
        let host = RuntimeHost::with_default_capabilities();
        let mut interp = Interpreter::new(host);

        let expr = SimpleExpr::BinOp(
            Box::new(SimpleExpr::Const(Value::Int(40))),
            BinOp::Add,
            Box::new(SimpleExpr::Const(Value::Int(2))),
        );

        let result = interp.eval(&expr).unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_wasm_emitter_simple_module() {
        let mut emitter = WasmEmitter::new();

        // Create a function that returns 42
        let func_idx = emitter.add_function(
            &[],
            &[ValType::I32],
            vec![],
            vec![Instruction::I32Const(42), Instruction::End],
        );

        emitter.add_export("main", func_idx);

        let module = emitter.emit();

        // Verify it's a valid WASM module (has magic number)
        assert_eq!(&module[0..4], &[0x00, 0x61, 0x73, 0x6D]);
    }

    #[test]
    fn test_capability_enforcement() {
        let mut host = RuntimeHost::new(Capabilities::new());

        // Should fail without UI capability
        let result = host.ui_slider(0.0, 100.0, 50.0);
        assert!(result.is_err());

        // Should fail without network capability
        let request = HttpRequest {
            method: "GET".to_string(),
            url: "https://example.com".to_string(),
            headers: vec![],
            body: None,
        };
        let result = host.fetch(request);
        assert!(result.is_err());
    }
}
