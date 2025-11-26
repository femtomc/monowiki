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

    /// Get the scope's unique ID
    pub fn id(&self) -> u64 {
        self.0
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

/// Parameter with optional type annotation and default value
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: Symbol,
    pub type_annotation: Option<Box<Shrubbery>>,
    pub default: Option<Box<Shrubbery>>,
    pub span: Span,
}

impl Param {
    pub fn new(name: Symbol, span: Span) -> Self {
        Self {
            name,
            type_annotation: None,
            default: None,
            span,
        }
    }

    pub fn with_type(mut self, type_annotation: Shrubbery) -> Self {
        self.type_annotation = Some(Box::new(type_annotation));
        self
    }

    pub fn with_default(mut self, default: Shrubbery) -> Self {
        self.default = Some(Box::new(default));
        self
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

    // Block-level constructs
    /// Macro definition: !def name(params): body
    DefBlock {
        name: Symbol,
        params: Vec<Param>,
        return_type: Option<Box<Shrubbery>>,
        body: Vec<Shrubbery>,
        span: Span,
    },

    /// Staged block: !staged[...] or !staged: indented_body
    StagedBlock {
        body: Vec<Shrubbery>,
        span: Span,
    },

    /// Show rule: !show selector: transform
    ShowRule {
        selector: Box<Shrubbery>,
        transform: Box<Shrubbery>,
        span: Span,
    },

    /// Set rule: !set selector {...}
    SetRule {
        selector: Box<Shrubbery>,
        properties: Vec<(Symbol, Shrubbery)>,
        span: Span,
    },

    /// Live code block: !live[...] or !live: indented_body
    LiveBlock {
        deps: Option<Vec<Symbol>>,
        body: Vec<Shrubbery>,
        span: Span,
    },

    /// Selector for show/set rules
    Selector {
        base: Symbol,
        predicate: Option<Box<Shrubbery>>,
        span: Span,
    },

    /// Quote expression: quote[...] or quote: body
    Quote {
        body: Box<Shrubbery>,
        span: Span,
    },

    /// Splice expression: $identifier or splice(expr)
    Splice {
        expr: Box<Shrubbery>,
        span: Span,
    },

    /// If expression: if cond: then else: otherwise
    If {
        condition: Box<Shrubbery>,
        then_branch: Box<Shrubbery>,
        else_branch: Option<Box<Shrubbery>>,
        span: Span,
    },

    /// For expression: for pattern in iterable: body
    For {
        pattern: Symbol,
        iterable: Box<Shrubbery>,
        body: Box<Shrubbery>,
        span: Span,
    },
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
            Shrubbery::DefBlock { span, .. } => *span,
            Shrubbery::StagedBlock { span, .. } => *span,
            Shrubbery::ShowRule { span, .. } => *span,
            Shrubbery::SetRule { span, .. } => *span,
            Shrubbery::LiveBlock { span, .. } => *span,
            Shrubbery::Selector { span, .. } => *span,
            Shrubbery::Quote { span, .. } => *span,
            Shrubbery::Splice { span, .. } => *span,
            Shrubbery::If { span, .. } => *span,
            Shrubbery::For { span, .. } => *span,
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
            Shrubbery::DefBlock { body, .. } | Shrubbery::StagedBlock { body, .. } | Shrubbery::LiveBlock { body, .. } => {
                for item in body {
                    item.add_scope(scope);
                }
            }
            Shrubbery::ShowRule { selector, transform, .. } => {
                selector.add_scope(scope);
                transform.add_scope(scope);
            }
            Shrubbery::SetRule { selector, properties, .. } => {
                selector.add_scope(scope);
                for (_, prop) in properties {
                    prop.add_scope(scope);
                }
            }
            Shrubbery::Quote { body, .. } | Shrubbery::Splice { expr: body, .. } => {
                body.add_scope(scope);
            }
            Shrubbery::If { condition, then_branch, else_branch, .. } => {
                condition.add_scope(scope);
                then_branch.add_scope(scope);
                if let Some(else_br) = else_branch {
                    else_br.add_scope(scope);
                }
            }
            Shrubbery::For { iterable, body, .. } => {
                iterable.add_scope(scope);
                body.add_scope(scope);
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
            Shrubbery::DefBlock { body, .. } | Shrubbery::StagedBlock { body, .. } | Shrubbery::LiveBlock { body, .. } => {
                for item in body {
                    item.flip_scope(scope);
                }
            }
            Shrubbery::ShowRule { selector, transform, .. } => {
                selector.flip_scope(scope);
                transform.flip_scope(scope);
            }
            Shrubbery::SetRule { selector, properties, .. } => {
                selector.flip_scope(scope);
                for (_, prop) in properties {
                    prop.flip_scope(scope);
                }
            }
            Shrubbery::Quote { body, .. } | Shrubbery::Splice { expr: body, .. } => {
                body.flip_scope(scope);
            }
            Shrubbery::If { condition, then_branch, else_branch, .. } => {
                condition.flip_scope(scope);
                then_branch.flip_scope(scope);
                if let Some(else_br) = else_branch {
                    else_br.flip_scope(scope);
                }
            }
            Shrubbery::For { iterable, body, .. } => {
                iterable.flip_scope(scope);
                body.flip_scope(scope);
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
            Shrubbery::DefBlock { name, params, body, .. } => {
                write!(f, "def:{}(", name.0)?;
                for (i, _param) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "param")?;
                }
                write!(f, "): body[{}]", body.len())
            }
            Shrubbery::StagedBlock { body, .. } => {
                write!(f, "staged[{}]", body.len())
            }
            Shrubbery::ShowRule { selector, transform, .. } => {
                write!(f, "show {} : {}", selector, transform)
            }
            Shrubbery::SetRule { selector, properties, .. } => {
                write!(f, "set {} {{ {} props }}", selector, properties.len())
            }
            Shrubbery::LiveBlock { body, .. } => {
                write!(f, "live[{}]", body.len())
            }
            Shrubbery::Selector { base, predicate, .. } => {
                write!(f, "selector:{}", base.0)?;
                if predicate.is_some() {
                    write!(f, ".where(...)")?;
                }
                Ok(())
            }
            Shrubbery::Quote { body, .. } => {
                write!(f, "quote[{}]", body)
            }
            Shrubbery::Splice { expr, .. } => {
                write!(f, "splice({})", expr)
            }
            Shrubbery::If { condition, then_branch, else_branch, .. } => {
                write!(f, "if {} then {} else {:?}", condition, then_branch, else_branch)
            }
            Shrubbery::For { pattern, iterable, body, .. } => {
                write!(f, "for {} in {} : {}", pattern.0, iterable, body)
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
