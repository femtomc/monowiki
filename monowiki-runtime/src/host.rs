//! Host function implementations
//!
//! This module provides the host-side implementation of all functions
//! exposed to WASM live cells through the WIT interface.

use crate::abi::{Capabilities, Capability, HttpRequest, HttpResponse, RuntimeError, RuntimeResult, Severity, Span};
use crate::dataspace::DataspaceClient;
use crate::diagnostics::DiagnosticCollector;
use crate::signals::SignalStore;
use crate::ui::WidgetStore;
use parking_lot::RwLock;
use sammy::assertion::OrSetStore;
use sammy::dataspace::LocalDataspace;
use std::sync::Arc;

/// Host functions for WASM runtime
///
/// This struct holds all the state needed to execute live cell WASM modules
/// and provides implementations of all host functions defined in the WIT interface.
pub struct RuntimeHost {
    pub signals: SignalStore,
    pub widgets: WidgetStore,
    pub diagnostics: DiagnosticCollector,
    pub dataspace_client: Option<DataspaceClient>,
    pub capabilities: Capabilities,
}

impl RuntimeHost {
    pub fn new(capabilities: Capabilities) -> Self {
        let dataspace_client = if capabilities.dataspace {
            // Create a default local dataspace for standalone use
            let ds = Arc::new(RwLock::new(LocalDataspace::<OrSetStore>::new("default")));
            Some(DataspaceClient::new(ds))
        } else {
            None
        };

        Self {
            signals: SignalStore::new(),
            widgets: WidgetStore::new(),
            diagnostics: DiagnosticCollector::new(),
            dataspace_client,
            capabilities,
        }
    }

    /// Create a runtime host with an existing dataspace
    pub fn with_dataspace(
        capabilities: Capabilities,
        dataspace: Arc<RwLock<LocalDataspace<OrSetStore>>>,
    ) -> Self {
        let dataspace_client = if capabilities.dataspace {
            Some(DataspaceClient::new(dataspace))
        } else {
            None
        };

        Self {
            signals: SignalStore::new(),
            widgets: WidgetStore::new(),
            diagnostics: DiagnosticCollector::new(),
            dataspace_client,
            capabilities,
        }
    }

    /// Create a new runtime host with default capabilities (UI only)
    pub fn with_default_capabilities() -> Self {
        Self::new(Capabilities::new().with_ui().with_diagnostics())
    }

    /// Check if a capability is granted
    fn check_capability(&self, cap: Capability) -> RuntimeResult<()> {
        if self.capabilities.has(cap) {
            Ok(())
        } else {
            Err(RuntimeError::CapabilityDenied(cap))
        }
    }

    // ===== Signal Functions =====

    /// Create a new signal with initial value
    pub fn signal_create(&mut self, initial: &[u8]) -> RuntimeResult<u64> {
        Ok(self.signals.create_raw(initial.to_vec()))
    }

    /// Get current signal value
    pub fn signal_get(&self, id: u64) -> RuntimeResult<Vec<u8>> {
        self.signals.get_raw(id)
    }

    /// Set signal value (triggers reactivity)
    pub fn signal_set(&mut self, id: u64, value: &[u8]) -> RuntimeResult<()> {
        self.signals.set_raw(id, value.to_vec())
    }

    /// Subscribe to signal changes
    pub fn signal_subscribe(&mut self, id: u64, callback_id: u64) -> RuntimeResult<()> {
        self.signals.subscribe(id, callback_id)
    }

    // ===== UI Functions =====

    /// Create a slider widget
    pub fn ui_slider(&mut self, min: f64, max: f64, initial: f64) -> RuntimeResult<u64> {
        self.check_capability(Capability::Ui)?;
        Ok(self.widgets.create_slider(min, max, initial))
    }

    /// Create a text input widget
    pub fn ui_text_input(&mut self, placeholder: &str, initial: &str) -> RuntimeResult<u64> {
        self.check_capability(Capability::Ui)?;
        Ok(self
            .widgets
            .create_text_input(placeholder.to_string(), initial.to_string()))
    }

    /// Create a button widget
    pub fn ui_button(&mut self, label: &str) -> RuntimeResult<u64> {
        self.check_capability(Capability::Ui)?;
        Ok(self.widgets.create_button(label.to_string()))
    }

    /// Show a value in the output area
    pub fn ui_show(&mut self, value: &[u8]) -> RuntimeResult<()> {
        self.check_capability(Capability::Ui)?;
        self.widgets.show(value.to_vec());
        Ok(())
    }

    // ===== Diagnostic Functions =====

    /// Emit a diagnostic message
    pub fn emit_diagnostic(&mut self, severity: Severity, span: Span, msg: &str) -> RuntimeResult<()> {
        self.check_capability(Capability::Diagnostics)?;
        self.diagnostics.emit(severity, span, msg.to_string());
        Ok(())
    }

    /// Add a decoration to the document
    pub fn add_decoration(&mut self, span: Span, class: &str) -> RuntimeResult<()> {
        self.check_capability(Capability::Diagnostics)?;
        self.diagnostics.decorate(span, class.to_string());
        Ok(())
    }

    // ===== Fetch Function =====

    /// Perform HTTP fetch (capability-gated)
    ///
    /// This is a stub implementation. In production, this would make
    /// actual HTTP requests with proper timeout and size limits.
    pub fn fetch(&self, _request: HttpRequest) -> RuntimeResult<HttpResponse> {
        self.check_capability(Capability::Network)?;

        // Stub implementation - always returns a 501 Not Implemented
        // In production, this would use reqwest or similar to make the actual request
        Ok(HttpResponse {
            status: 501,
            headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
            body: b"HTTP fetch not yet implemented".to_vec(),
        })
    }

    // ===== Dataspace Functions =====

    /// Publish an assertion to the dataspace
    pub fn dataspace_publish(&mut self, pattern: &str, value: &[u8]) -> RuntimeResult<u64> {
        self.check_capability(Capability::Dataspace)?;

        let client = self.dataspace_client.as_mut().ok_or_else(|| {
            RuntimeError::DataspaceError("Dataspace client not initialized".to_string())
        })?;

        client.publish(pattern.to_string(), value.to_vec())
    }

    /// Retract an assertion from the dataspace
    pub fn dataspace_retract(&mut self, assertion_id: u64) -> RuntimeResult<()> {
        self.check_capability(Capability::Dataspace)?;

        let client = self.dataspace_client.as_mut().ok_or_else(|| {
            RuntimeError::DataspaceError("Dataspace client not initialized".to_string())
        })?;

        client.retract(assertion_id)
    }

    /// Subscribe to a dataspace pattern
    pub fn dataspace_subscribe(&mut self, pattern: &str, callback_id: u64) -> RuntimeResult<u64> {
        self.check_capability(Capability::Dataspace)?;

        let client = self.dataspace_client.as_mut().ok_or_else(|| {
            RuntimeError::DataspaceError("Dataspace client not initialized".to_string())
        })?;

        client.subscribe(pattern.to_string(), callback_id)
    }

    /// Unsubscribe from a dataspace pattern
    pub fn dataspace_unsubscribe(&mut self, subscription_id: u64) -> RuntimeResult<()> {
        self.check_capability(Capability::Dataspace)?;

        let client = self.dataspace_client.as_mut().ok_or_else(|| {
            RuntimeError::DataspaceError("Dataspace client not initialized".to_string())
        })?;

        client.unsubscribe(subscription_id)
    }

    // ===== Utility Functions =====

    /// Process pending signal updates
    ///
    /// Returns a list of (signal_id, value, callbacks) for signals that changed
    pub fn process_signals(&mut self) -> Vec<(u64, Vec<u8>, Vec<u64>)> {
        self.signals.process_pending()
    }

    /// Get all output values from UI
    pub fn take_output(&mut self) -> Vec<Vec<u8>> {
        self.widgets.take_output()
    }

    /// Clear all runtime state
    pub fn clear(&mut self) {
        self.signals.clear();
        self.widgets.clear();
        self.diagnostics.clear();
        if let Some(client) = &mut self.dataspace_client {
            client.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_operations() {
        let mut host = RuntimeHost::with_default_capabilities();

        let id = host.signal_create(b"hello").unwrap();
        let value = host.signal_get(id).unwrap();
        assert_eq!(value, b"hello");

        host.signal_set(id, b"world").unwrap();
        let value = host.signal_get(id).unwrap();
        assert_eq!(value, b"world");
    }

    #[test]
    fn test_ui_capability_check() {
        let mut host = RuntimeHost::new(Capabilities::new());

        let result = host.ui_slider(0.0, 100.0, 50.0);
        assert!(matches!(
            result,
            Err(RuntimeError::CapabilityDenied(Capability::Ui))
        ));
    }

    #[test]
    fn test_ui_slider_with_capability() {
        let mut host = RuntimeHost::new(Capabilities::new().with_ui());

        let id = host.ui_slider(0.0, 100.0, 50.0).unwrap();
        assert!(id > 0);
    }

    #[test]
    fn test_diagnostics_with_capability() {
        let mut host = RuntimeHost::new(Capabilities::new().with_diagnostics());

        let span = Span::new(1, 0, 1, 10);
        host.emit_diagnostic(Severity::Error, span, "Test error")
            .unwrap();

        assert_eq!(host.diagnostics.diagnostic_count(), 1);
    }

    #[test]
    fn test_dataspace_without_capability() {
        let mut host = RuntimeHost::new(Capabilities::new());

        let result = host.dataspace_publish("test.pattern", b"value");
        assert!(matches!(
            result,
            Err(RuntimeError::CapabilityDenied(Capability::Dataspace))
        ));
    }

    #[test]
    fn test_dataspace_with_capability() {
        let mut host = RuntimeHost::new(Capabilities::new().with_dataspace());

        let id = host.dataspace_publish("test.pattern", b"value").unwrap();
        assert!(id > 0);

        host.dataspace_retract(id).unwrap();
    }

    #[test]
    fn test_fetch_capability() {
        let host = RuntimeHost::new(Capabilities::new());

        let request = HttpRequest {
            method: "GET".to_string(),
            url: "https://example.com".to_string(),
            headers: vec![],
            body: None,
        };

        let result = host.fetch(request);
        assert!(matches!(
            result,
            Err(RuntimeError::CapabilityDenied(Capability::Network))
        ));
    }
}
