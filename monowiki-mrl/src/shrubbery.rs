use crate::error::Span;
use std::collections::BTreeSet;
use std::fmt;

/// A unique identifier for a scope
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Scope(u64);

impl Scope {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// A set of scopes for hygiene tracking
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScopeSet {
    scopes: BTreeSet<Scope>,
}

impl ScopeSet {
    pub fn new() -> Self {
        Self {
            scopes: BTreeSet::new(),
        }
    }

    pub fn with_scope(scope: Scope) -> Self {
        let mut set = Self::new();
        set.add(scope);
        set
    }

    pub fn add(&mut self, scope: Scope) {
        self.scopes.insert(scope);
    }

    pub fn remove(&mut self, scope: Scope) {
        self.scopes.remove(&scope);
    }

    pub fn contains(&self, scope: &Scope) -> bool {
        self.scopes.contains(scope)
    }

    pub fn is_subset(&self, other: &ScopeSet) -> bool {
        self.scopes.is_subset(&other.scopes)
    }

    pub fn union(&self, other: &ScopeSet) -> Self {
        Self {
            scopes: self.scopes.union(&other.scopes).copied().collect(),
        }
    }

    pub fn intersection(&self, other: &ScopeSet) -> Self {
        Self {
            scopes: self.scopes.intersection(&other.scopes).copied().collect(),
        }
    }

    /// Flip a scope: remove if present, add if absent
    pub fn flip(&mut self, scope: Scope) {
        if self.contains(&scope) {
            self.remove(scope);
        } else {
            self.add(scope);
        }
    }

    pub fn len(&self) -> usize {
        self.scopes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scopes.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Scope> {
        self.scopes.iter()
    }
}

impl FromIterator<Scope> for ScopeSet {
    fn from_iter<I: IntoIterator<Item = Scope>>(iter: I) -> Self {
        Self {
            scopes: iter.into_iter().collect(),
        }
    }
}

/// Symbol interning for identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol(u64);

impl Symbol {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn id(&self) -> u64 {
        self.0
    }
}

/// Literal values
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Symbol(String),
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Literal::None => write!(f, "none"),
            Literal::Bool(b) => write!(f, "{}", b),
            Literal::Int(i) => write!(f, "{}", i),
            Literal::Float(fl) => write!(f, "{}", fl),
            Literal::String(s) => write!(f, "\"{}\"", s),
            Literal::Symbol(s) => write!(f, "'{}", s),
        }
    }
}

/// Shrubbery: token tree representation with deferred precedence
#[derive(Debug, Clone, PartialEq)]
pub enum Shrubbery {
    /// Identifier with scope set for hygiene
    Identifier(Symbol, ScopeSet, Span),

    /// Literal value
    Literal(Literal, Span),

    /// Operator (e.g., +, -, *, precedence resolved later)
    Operator(Symbol, Span),

    /// Parenthesized group: (...)
    Parens(Vec<Shrubbery>, Span),

    /// Bracketed group: [...]
    Brackets(Vec<Shrubbery>, Span),

    /// Braced group: {...}
    Braces(Vec<Shrubbery>, Span),

    /// Prose text (from markdown)
    Prose(String, Span),

    /// Content block: mixed prose and code
    ContentBlock(Vec<Shrubbery>, Span),

    /// Sequence of elements (comma or newline separated)
    Sequence(Vec<Shrubbery>, Span),
}

impl Shrubbery {
    pub fn span(&self) -> Span {
        match self {
            Shrubbery::Identifier(_, _, span) => *span,
            Shrubbery::Literal(_, span) => *span,
            Shrubbery::Operator(_, span) => *span,
            Shrubbery::Parens(_, span) => *span,
            Shrubbery::Brackets(_, span) => *span,
            Shrubbery::Braces(_, span) => *span,
            Shrubbery::Prose(_, span) => *span,
            Shrubbery::ContentBlock(_, span) => *span,
            Shrubbery::Sequence(_, span) => *span,
        }
    }

    /// Add a scope to all identifiers in this shrubbery
    pub fn add_scope(&mut self, scope: Scope) {
        match self {
            Shrubbery::Identifier(_, scopes, _) => {
                scopes.add(scope);
            }
            Shrubbery::Parens(items, _)
            | Shrubbery::Brackets(items, _)
            | Shrubbery::Braces(items, _)
            | Shrubbery::ContentBlock(items, _)
            | Shrubbery::Sequence(items, _) => {
                for item in items {
                    item.add_scope(scope);
                }
            }
            _ => {}
        }
    }

    /// Flip a scope on all identifiers in this shrubbery
    pub fn flip_scope(&mut self, scope: Scope) {
        match self {
            Shrubbery::Identifier(_, scopes, _) => {
                scopes.flip(scope);
            }
            Shrubbery::Parens(items, _)
            | Shrubbery::Brackets(items, _)
            | Shrubbery::Braces(items, _)
            | Shrubbery::ContentBlock(items, _)
            | Shrubbery::Sequence(items, _) => {
                for item in items {
                    item.flip_scope(scope);
                }
            }
            _ => {}
        }
    }

    /// Get the identifier name if this is an identifier
    pub fn as_identifier(&self) -> Option<(Symbol, &ScopeSet)> {
        match self {
            Shrubbery::Identifier(sym, scopes, _) => Some((*sym, scopes)),
            _ => None,
        }
    }

    /// Get the literal if this is a literal
    pub fn as_literal(&self) -> Option<&Literal> {
        match self {
            Shrubbery::Literal(lit, _) => Some(lit),
            _ => None,
        }
    }
}

impl fmt::Display for Shrubbery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Shrubbery::Identifier(sym, _, _) => write!(f, "id:{}", sym.0),
            Shrubbery::Literal(lit, _) => write!(f, "{}", lit),
            Shrubbery::Operator(sym, _) => write!(f, "op:{}", sym.0),
            Shrubbery::Parens(items, _) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, ")")
            }
            Shrubbery::Brackets(items, _) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            Shrubbery::Braces(items, _) => {
                write!(f, "{{")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "}}")
            }
            Shrubbery::Prose(text, _) => write!(f, "prose:{:?}", text),
            Shrubbery::ContentBlock(items, _) => {
                write!(f, "content[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            Shrubbery::Sequence(items, _) => {
                write!(f, "seq[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_set() {
        let mut set = ScopeSet::new();
        let scope1 = Scope::new(1);
        let scope2 = Scope::new(2);

        assert!(!set.contains(&scope1));
        set.add(scope1);
        assert!(set.contains(&scope1));
        assert!(!set.contains(&scope2));

        set.add(scope2);
        assert_eq!(set.len(), 2);

        set.remove(scope1);
        assert!(!set.contains(&scope1));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_scope_set_flip() {
        let mut set = ScopeSet::new();
        let scope = Scope::new(1);

        set.flip(scope);
        assert!(set.contains(&scope));

        set.flip(scope);
        assert!(!set.contains(&scope));
    }

    #[test]
    fn test_scope_set_subset() {
        let scope1 = Scope::new(1);
        let scope2 = Scope::new(2);

        let set1 = ScopeSet::from_iter(vec![scope1]);
        let set2 = ScopeSet::from_iter(vec![scope1, scope2]);

        assert!(set1.is_subset(&set2));
        assert!(!set2.is_subset(&set1));
    }

    #[test]
    fn test_shrubbery_add_scope() {
        let scope = Scope::new(1);
        let mut shrub = Shrubbery::Identifier(
            Symbol::new(42),
            ScopeSet::new(),
            Span::new(0, 3),
        );

        shrub.add_scope(scope);

        if let Shrubbery::Identifier(_, scopes, _) = &shrub {
            assert!(scopes.contains(&scope));
        } else {
            panic!("Expected identifier");
        }
    }

    #[test]
    fn test_shrubbery_flip_scope() {
        let scope = Scope::new(1);
        let mut shrub = Shrubbery::Identifier(
            Symbol::new(42),
            ScopeSet::with_scope(scope),
            Span::new(0, 3),
        );

        shrub.flip_scope(scope);

        if let Shrubbery::Identifier(_, scopes, _) = &shrub {
            assert!(!scopes.contains(&scope));
        } else {
            panic!("Expected identifier");
        }
    }
}
