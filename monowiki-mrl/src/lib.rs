//! # Monowiki Reflective Language (MRL)
//!
//! MRL is a typed, staged document language for monowiki with:
//! - Single `!` escape from prose to code
//! - Hygienic macro system using scope sets
//! - Three-phase execution: read-time, expand-time, render-time
//! - Type-safe staged code generation with Code<K> types
//! - Document reflection and introspection
//!
//! ## Example
//!
//! ```ignore
//! # My Document
//!
//! This is prose with inline code: !today().
//!
//! !def greet(name: String):
//!   [Hello, *!name*!]
//!
//! !staged[
//!   for sec in doc.outline():
//!     paragraph([Section: !sec.title])
//! ]
//! ```

pub mod checker;
pub mod content;
pub mod document;
pub mod enforest;
pub mod error;
pub mod expander;
pub mod hygiene;
pub mod interpreter;
pub mod lexer;
pub mod parser;
pub mod rules;
pub mod shrubbery;
pub mod types;

#[cfg(test)]
mod tests;

// Re-export key types
pub use checker::TypeChecker;
pub use content::{Attributes, Block, Content, Inline, ListItem};
pub use document::{parse_document, DocumentElement, DocumentParser};
pub use enforest::{enforest, Assoc, BinOp, Expr, UnOp};
pub use error::{ErrorContext, MrlError, Result};
pub use expander::{ExpandFunction, ExpandValue, Expander};
pub use hygiene::{Binding, HygieneChecker, HygieneEnv, MacroContext};
pub use interpreter::{DocumentReflection, Interpreter, OutlineEntry, ReferenceEntry, SectionContext};
pub use lexer::{tokenize, Lexer, SpannedToken, Token};
pub use parser::{parse, Parser, SymbolTable};
pub use rules::{Predicate, PredicateValue, RuleSet, Selector, SelectorBase, SetRule, SetValue, ShowRule};
pub use shrubbery::{Literal, Param, Scope, ScopeSet, Shrubbery, Symbol};
pub use types::{MrlType, TypeScheme};

// Re-export shared types from monowiki-types
pub use monowiki_types::{BlockId, ContentKind, DocId, Span};

/// Parse MRL source into shrubbery
pub fn parse_source(source: &str) -> Result<Shrubbery> {
    let tokens = tokenize(source)?;
    parse(&tokens)
}

/// Execute MRL source to produce content
pub fn execute(source: &str) -> Result<Content> {
    let mut interpreter = Interpreter::new();
    interpreter.execute_document(source)
}

/// Type check MRL source
pub fn typecheck(source: &str) -> Result<MrlType> {
    let tokens = tokenize(source)?;
    let shrub = parse(&tokens)?;
    let mut checker = TypeChecker::new();
    checker.check(&shrub)
}
