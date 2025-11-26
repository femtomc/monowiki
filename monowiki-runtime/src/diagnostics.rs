//! Diagnostic and decoration publishing
//!
//! This module provides diagnostic message and decoration APIs for live cells.
//! Diagnostics report errors, warnings, and hints. Decorations add visual
//! styling to document ranges.

use crate::abi::{Severity, Span};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A diagnostic message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub span: Span,
    pub message: String,
    pub source: Option<String>,
}

impl Diagnostic {
    pub fn new(severity: Severity, span: Span, message: String) -> Self {
        Self {
            severity,
            span,
            message,
            source: None,
        }
    }

    pub fn with_source(mut self, source: String) -> Self {
        self.source = Some(source);
        self
    }

    pub fn error(span: Span, message: String) -> Self {
        Self::new(Severity::Error, span, message)
    }

    pub fn warning(span: Span, message: String) -> Self {
        Self::new(Severity::Warning, span, message)
    }

    pub fn info(span: Span, message: String) -> Self {
        Self::new(Severity::Info, span, message)
    }

    pub fn hint(span: Span, message: String) -> Self {
        Self::new(Severity::Hint, span, message)
    }
}

/// A decoration applied to a document range
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decoration {
    pub span: Span,
    pub class: String,
    pub attributes: HashMap<String, String>,
}

impl Decoration {
    pub fn new(span: Span, class: String) -> Self {
        Self {
            span,
            class,
            attributes: HashMap::new(),
        }
    }

    pub fn with_attribute(mut self, key: String, value: String) -> Self {
        self.attributes.insert(key, value);
        self
    }

    pub fn with_attributes(mut self, attributes: HashMap<String, String>) -> Self {
        self.attributes = attributes;
        self
    }
}

/// Collector for diagnostics and decorations
#[derive(Debug, Default)]
pub struct DiagnosticCollector {
    diagnostics: Vec<Diagnostic>,
    decorations: Vec<Decoration>,
}

impl DiagnosticCollector {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            decorations: Vec::new(),
        }
    }

    /// Emit a diagnostic message
    pub fn emit_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Emit a diagnostic with explicit parameters
    pub fn emit(&mut self, severity: Severity, span: Span, message: String) {
        self.emit_diagnostic(Diagnostic::new(severity, span, message));
    }

    /// Emit an error diagnostic
    pub fn error(&mut self, span: Span, message: String) {
        self.emit(Severity::Error, span, message);
    }

    /// Emit a warning diagnostic
    pub fn warning(&mut self, span: Span, message: String) {
        self.emit(Severity::Warning, span, message);
    }

    /// Emit an info diagnostic
    pub fn info(&mut self, span: Span, message: String) {
        self.emit(Severity::Info, span, message);
    }

    /// Emit a hint diagnostic
    pub fn hint(&mut self, span: Span, message: String) {
        self.emit(Severity::Hint, span, message);
    }

    /// Add a decoration
    pub fn add_decoration(&mut self, decoration: Decoration) {
        self.decorations.push(decoration);
    }

    /// Add a decoration with explicit parameters
    pub fn decorate(&mut self, span: Span, class: String) {
        self.add_decoration(Decoration::new(span, class));
    }

    /// Get all diagnostics
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Get all decorations
    pub fn decorations(&self) -> &[Decoration] {
        &self.decorations
    }

    /// Get diagnostics by severity
    pub fn diagnostics_by_severity(&self, severity: Severity) -> Vec<&Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == severity)
            .collect()
    }

    /// Get errors
    pub fn errors(&self) -> Vec<&Diagnostic> {
        self.diagnostics_by_severity(Severity::Error)
    }

    /// Get warnings
    pub fn warnings(&self) -> Vec<&Diagnostic> {
        self.diagnostics_by_severity(Severity::Warning)
    }

    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    /// Check if there are any diagnostics
    pub fn has_diagnostics(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    /// Check if there are any decorations
    pub fn has_decorations(&self) -> bool {
        !self.decorations.is_empty()
    }

    /// Take all diagnostics, clearing the collector
    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
        std::mem::take(&mut self.diagnostics)
    }

    /// Take all decorations, clearing the collector
    pub fn take_decorations(&mut self) -> Vec<Decoration> {
        std::mem::take(&mut self.decorations)
    }

    /// Clear all diagnostics and decorations
    pub fn clear(&mut self) {
        self.diagnostics.clear();
        self.decorations.clear();
    }

    /// Get the number of diagnostics
    pub fn diagnostic_count(&self) -> usize {
        self.diagnostics.len()
    }

    /// Get the number of decorations
    pub fn decoration_count(&self) -> usize {
        self.decorations.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_creation() {
        let span = Span::new(1, 0, 1, 10);
        let diag = Diagnostic::error(span, "Test error".to_string());

        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "Test error");
        assert_eq!(diag.span, span);
    }

    #[test]
    fn test_diagnostic_with_source() {
        let span = Span::new(1, 0, 1, 10);
        let diag = Diagnostic::error(span, "Test error".to_string())
            .with_source("test-cell".to_string());

        assert_eq!(diag.source, Some("test-cell".to_string()));
    }

    #[test]
    fn test_decoration_creation() {
        let span = Span::new(1, 0, 1, 10);
        let decoration = Decoration::new(span, "highlight".to_string());

        assert_eq!(decoration.class, "highlight");
        assert_eq!(decoration.span, span);
        assert!(decoration.attributes.is_empty());
    }

    #[test]
    fn test_decoration_with_attributes() {
        let span = Span::new(1, 0, 1, 10);
        let decoration = Decoration::new(span, "highlight".to_string())
            .with_attribute("color".to_string(), "red".to_string());

        assert_eq!(decoration.attributes.get("color"), Some(&"red".to_string()));
    }

    #[test]
    fn test_collector_emit() {
        let mut collector = DiagnosticCollector::new();
        let span = Span::new(1, 0, 1, 10);

        collector.error(span, "Error 1".to_string());
        collector.warning(span, "Warning 1".to_string());

        assert_eq!(collector.diagnostic_count(), 2);
        assert!(collector.has_errors());
    }

    #[test]
    fn test_collector_by_severity() {
        let mut collector = DiagnosticCollector::new();
        let span = Span::new(1, 0, 1, 10);

        collector.error(span, "Error 1".to_string());
        collector.error(span, "Error 2".to_string());
        collector.warning(span, "Warning 1".to_string());

        let errors = collector.errors();
        assert_eq!(errors.len(), 2);

        let warnings = collector.warnings();
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn test_collector_decorations() {
        let mut collector = DiagnosticCollector::new();
        let span = Span::new(1, 0, 1, 10);

        collector.decorate(span, "highlight".to_string());
        collector.decorate(span, "underline".to_string());

        assert_eq!(collector.decoration_count(), 2);
        assert!(collector.has_decorations());
    }

    #[test]
    fn test_collector_take() {
        let mut collector = DiagnosticCollector::new();
        let span = Span::new(1, 0, 1, 10);

        collector.error(span, "Error 1".to_string());
        collector.decorate(span, "highlight".to_string());

        let diagnostics = collector.take_diagnostics();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(collector.diagnostic_count(), 0);

        let decorations = collector.take_decorations();
        assert_eq!(decorations.len(), 1);
        assert_eq!(collector.decoration_count(), 0);
    }

    #[test]
    fn test_collector_clear() {
        let mut collector = DiagnosticCollector::new();
        let span = Span::new(1, 0, 1, 10);

        collector.error(span, "Error 1".to_string());
        collector.decorate(span, "highlight".to_string());

        collector.clear();

        assert_eq!(collector.diagnostic_count(), 0);
        assert_eq!(collector.decoration_count(), 0);
    }
}
