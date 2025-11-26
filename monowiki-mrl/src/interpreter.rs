use crate::checker::TypeChecker;
use crate::content::Content;
use crate::enforest::enforest;
use crate::error::Result;
use crate::expander::{ExpandValue, Expander};
use crate::parser::{parse_with_symbols, SymbolTable};
use crate::shrubbery::Shrubbery;

/// Expand-time interpreter
///
/// This executes !staged blocks and produces Content trees
pub struct Interpreter {
    expander: Expander,
    checker: TypeChecker,
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            expander: Expander::new(),
            checker: TypeChecker::new(),
        }
    }

    /// Register symbols for reverse lookup during type checking
    pub fn register_symbols(&mut self, symbols: &SymbolTable) {
        self.checker.register_symbols(symbols.symbols());
    }

    /// Execute a staged block (legacy - shrubbery only)
    pub fn execute_staged(&mut self, shrub: &Shrubbery) -> Result<Content> {
        // First, type check shrubbery
        let _ty = self.checker.check(shrub)?;

        // Then expand
        let value = self.expander.expand(shrub)?;

        // Extract content
        match value {
            ExpandValue::Content(c) => Ok(c),
            _ => {
                // Try to convert to content
                Ok(Content::text(format!("{:?}", value)))
            }
        }
    }

    /// Execute a staged block with full Expr type checking
    pub fn execute_staged_checked(&mut self, shrub: &Shrubbery) -> Result<Content> {
        // Enforest to Expr
        let expr = enforest(shrub)?;

        // Type check the enforested expression
        let _ty = self.checker.check_expr(&expr)?;

        // Expand (still uses Shrubbery for now)
        let value = self.expander.expand(shrub)?;

        // Extract content
        match value {
            ExpandValue::Content(c) => Ok(c),
            _ => {
                Ok(Content::text(format!("{:?}", value)))
            }
        }
    }

    /// Execute an entire document
    pub fn execute_document(&mut self, source: &str) -> Result<Content> {
        use crate::lexer::tokenize;

        let tokens = tokenize(source)?;
        let (shrub, symbols) = parse_with_symbols(&tokens)?;

        // Register symbols for reverse lookup
        self.register_symbols(&symbols);

        // Use full Expr type checking
        self.execute_staged_checked(&shrub)
    }

    /// Get document reflection methods
    pub fn reflection(&self) -> DocumentReflection {
        DocumentReflection::new()
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

/// Document reflection API
///
/// Provides methods for introspecting document structure
pub struct DocumentReflection {
    // This would hold references to the document being processed
}

impl DocumentReflection {
    pub fn new() -> Self {
        Self {}
    }

    /// Get the document outline (headings)
    pub fn outline(&self) -> Vec<OutlineEntry> {
        // Placeholder implementation
        Vec::new()
    }

    /// Get all cross-references in the document
    pub fn refs(&self) -> Vec<ReferenceEntry> {
        Vec::new()
    }

    /// Find elements matching a selector
    pub fn find(&self, _selector: &str) -> Vec<Content> {
        Vec::new()
    }

    /// Get document metadata
    pub fn meta(&self, _key: &str) -> Option<String> {
        None
    }

    /// Get current section context
    pub fn here(&self) -> SectionContext {
        SectionContext::default()
    }
}

impl Default for DocumentReflection {
    fn default() -> Self {
        Self::new()
    }
}

/// An entry in the document outline
#[derive(Debug, Clone)]
pub struct OutlineEntry {
    pub level: u8,
    pub title: String,
    pub id: Option<String>,
}

/// A cross-reference entry
#[derive(Debug, Clone)]
pub struct ReferenceEntry {
    pub target: String,
    pub source_span: crate::error::Span,
}

/// Section context for reflection
#[derive(Debug, Clone, Default)]
pub struct SectionContext {
    pub section_id: Option<String>,
    pub heading: Option<String>,
    pub level: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpreter_literal() {
        let mut interp = Interpreter::new();
        let result = interp.execute_document("42");
        assert!(result.is_ok());
    }

    #[test]
    fn test_interpreter_string() {
        let mut interp = Interpreter::new();
        let result = interp.execute_document(r#""hello""#);
        assert!(result.is_ok());
    }

    #[test]
    fn test_reflection() {
        let reflection = DocumentReflection::new();
        let outline = reflection.outline();
        assert_eq!(outline.len(), 0);
    }
}
