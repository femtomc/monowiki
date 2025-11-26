//! Enforestation: resolve operator precedence in shrubbery to typed AST
//!
//! This module implements Rhombus-style enforestation with:
//! - Relative precedence (not numeric levels)
//! - Operator protocols: Automatic vs Macro
//! - Implicit operators for calls, juxtaposition, etc.
//! - Precedence climbing algorithm that returns (form, tail)
//! - Form recognizers for keyword-prefixed forms (def, if, for, etc.)
//!
//! Based on the paper "Honu: Syntactic Extension for Algebraic Notation through Enforestation"

use crate::content::*;
use crate::error::{MrlError, Result, Span};
use crate::shrubbery::{Literal, Param, ScopeSet, Shrubbery, Symbol};
use std::collections::HashMap;

// =============================================================================
// Operator Infrastructure
// =============================================================================

/// Relative precedence relationship between operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    Weaker,
    Stronger,
    Same,
}

/// Operator associativity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assoc {
    Left,
    Right,
    None,
}

/// Operator protocol determines how an operator is invoked
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// Automatic: operator function receives parsed forms
    /// The enforestation algorithm handles precedence automatically
    Automatic,
    /// Macro: operator transformer consumes the tail directly
    /// Used for complex syntax like `if`, `for`, etc.
    Macro,
}

/// Result of comparing relative precedence
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrecResult {
    Stronger,
    Weaker,
    Same,
    SameOnLeft,
    SameOnRight,
    Inconsistent,
    Unknown,
}

/// A prefix operator (appears before its operand)
#[derive(Debug, Clone)]
pub struct PrefixOperator {
    pub name: Symbol,
    pub protocol: Protocol,
    /// Relative precedences: (other_op, relation)
    /// e.g., [("-", Weaker)] means "I am weaker than -"
    pub precedences: Vec<(Symbol, Relation)>,
    /// Default precedence for operators not in the list
    pub default_prec: Option<Relation>,
}

/// An infix operator (appears between operands)
#[derive(Debug, Clone)]
pub struct InfixOperator {
    pub name: Symbol,
    pub protocol: Protocol,
    pub assoc: Assoc,
    /// Relative precedences: (other_op, relation)
    pub precedences: Vec<(Symbol, Relation)>,
    /// Default precedence for operators not in the list
    pub default_prec: Option<Relation>,
}

impl PrefixOperator {
    pub fn new(name: Symbol, protocol: Protocol) -> Self {
        Self {
            name,
            protocol,
            precedences: Vec::new(),
            default_prec: None,
        }
    }

    pub fn with_prec(mut self, other: Symbol, rel: Relation) -> Self {
        self.precedences.push((other, rel));
        self
    }

    pub fn with_default(mut self, rel: Relation) -> Self {
        self.default_prec = Some(rel);
        self
    }
}

impl InfixOperator {
    pub fn new(name: Symbol, protocol: Protocol, assoc: Assoc) -> Self {
        Self {
            name,
            protocol,
            assoc,
            precedences: Vec::new(),
            default_prec: None,
        }
    }

    pub fn with_prec(mut self, other: Symbol, rel: Relation) -> Self {
        self.precedences.push((other, rel));
        self
    }

    pub fn with_default(mut self, rel: Relation) -> Self {
        self.default_prec = Some(rel);
        self
    }

    /// Get precedence of self relative to another operator
    pub fn get_precedence(&self, other: Symbol) -> Option<Relation> {
        for (sym, rel) in &self.precedences {
            if *sym == other {
                return Some(*rel);
            }
        }
        self.default_prec
    }
}

/// Compare relative precedence between left operator and current operator
pub fn relative_precedence(
    left_op: &InfixOperator,
    right_op: &InfixOperator,
) -> PrecResult {
    // Check if right_op declares precedence relative to left_op
    if let Some(rel) = right_op.get_precedence(left_op.name) {
        match rel {
            Relation::Weaker => return PrecResult::Weaker,
            Relation::Stronger => return PrecResult::Stronger,
            Relation::Same => {
                // Check associativity
                match (left_op.assoc, right_op.assoc) {
                    (Assoc::Left, Assoc::Left) => return PrecResult::SameOnLeft,
                    (Assoc::Right, Assoc::Right) => return PrecResult::SameOnRight,
                    (Assoc::None, _) | (_, Assoc::None) => return PrecResult::Inconsistent,
                    _ => return PrecResult::Inconsistent,
                }
            }
        }
    }

    // Check if left_op declares precedence relative to right_op (inverted)
    if let Some(rel) = left_op.get_precedence(right_op.name) {
        match rel {
            Relation::Weaker => return PrecResult::Stronger, // inverted
            Relation::Stronger => return PrecResult::Weaker, // inverted
            Relation::Same => {
                match (left_op.assoc, right_op.assoc) {
                    (Assoc::Left, Assoc::Left) => return PrecResult::SameOnLeft,
                    (Assoc::Right, Assoc::Right) => return PrecResult::SameOnRight,
                    _ => return PrecResult::Inconsistent,
                }
            }
        }
    }

    PrecResult::Unknown
}

// =============================================================================
// Implicit Operators
// =============================================================================

/// Implicit operators handle cases where no explicit operator is present
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImplicitOp {
    /// #%call - function call: identifier followed by parens
    Call,
    /// #%juxtapose - two forms adjacent without operator
    Juxtapose,
    /// #%parens - bare parentheses (grouping)
    Parens,
    /// #%brackets - bare brackets (content/array)
    Brackets,
    /// #%braces - bare braces (block/object)
    Braces,
    /// #%literal - literal values
    Literal,
    /// #%identifier - bare identifier
    Identifier,
}

// =============================================================================
// Enforestation Result Types
// =============================================================================

/// Binary operator for AST
#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Concat,
    Assign,
}

impl BinOp {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "+" => Some(BinOp::Add),
            "-" => Some(BinOp::Sub),
            "*" => Some(BinOp::Mul),
            "/" => Some(BinOp::Div),
            "%" => Some(BinOp::Mod),
            "**" => Some(BinOp::Pow),
            "==" => Some(BinOp::Eq),
            "!=" => Some(BinOp::Ne),
            "<" => Some(BinOp::Lt),
            "<=" => Some(BinOp::Le),
            ">" => Some(BinOp::Gt),
            ">=" => Some(BinOp::Ge),
            "&&" => Some(BinOp::And),
            "||" => Some(BinOp::Or),
            "++" => Some(BinOp::Concat),
            "=" => Some(BinOp::Assign),
            _ => None,
        }
    }
}

/// Unary operator
#[derive(Debug, Clone, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
}

/// Enforestation result: typed expression AST
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal, Span),
    Var(Symbol, ScopeSet, Span),
    BinOp(Box<Expr>, BinOp, Box<Expr>, Span),
    UnOp(UnOp, Box<Expr>, Span),
    Call(Box<Expr>, Vec<Expr>, Span),
    FieldAccess(Box<Expr>, Symbol, Span),
    Subscript(Box<Expr>, Box<Expr>, Span),
    If(Box<Expr>, Box<Expr>, Option<Box<Expr>>, Span),
    For(Symbol, Box<Expr>, Box<Expr>, Span),
    Quote(Box<Expr>, Span),
    Splice(Box<Expr>, Span),
    Content(Content, Span),
    Block(Vec<Expr>, Span),
    /// A sequence of unevaluated shrubbery (for macro arguments, etc.)
    Sequence(Vec<Shrubbery>, Span),

    // Definition forms (created by form recognizers)
    /// Function/macro definition: def name(params): body
    Def {
        name: Symbol,
        params: Vec<Param>,
        return_type: Option<Box<Expr>>,
        body: Box<Expr>,
        span: Span,
    },
    /// Staged code block: staged[body]
    Staged(Box<Expr>, Span),
    /// Show rule: show selector: transform
    ShowRule {
        selector: Box<Expr>,
        transform: Box<Expr>,
        span: Span,
    },
    /// Set rule: set selector { props }
    SetRule {
        selector: Box<Expr>,
        properties: Vec<(Symbol, Expr)>,
        span: Span,
    },
    /// Live reactive block
    Live {
        deps: Option<Vec<Symbol>>,
        body: Box<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal(_, s) => *s,
            Expr::Var(_, _, s) => *s,
            Expr::BinOp(_, _, _, s) => *s,
            Expr::UnOp(_, _, s) => *s,
            Expr::Call(_, _, s) => *s,
            Expr::FieldAccess(_, _, s) => *s,
            Expr::Subscript(_, _, s) => *s,
            Expr::If(_, _, _, s) => *s,
            Expr::For(_, _, _, s) => *s,
            Expr::Quote(_, s) => *s,
            Expr::Splice(_, s) => *s,
            Expr::Content(_, s) => *s,
            Expr::Block(_, s) => *s,
            Expr::Sequence(_, s) => *s,
            Expr::Def { span, .. } => *span,
            Expr::Staged(_, s) => *s,
            Expr::ShowRule { span, .. } => *span,
            Expr::SetRule { span, .. } => *span,
            Expr::Live { span, .. } => *span,
        }
    }
}

// =============================================================================
// Operator Environment
// =============================================================================

/// Environment holding operator definitions
#[derive(Debug, Clone)]
pub struct OperatorEnv {
    /// Prefix operators indexed by symbol
    prefix_ops: HashMap<Symbol, PrefixOperator>,
    /// Infix operators indexed by symbol
    infix_ops: HashMap<Symbol, InfixOperator>,
    /// Symbol table for looking up operator names
    symbol_names: HashMap<Symbol, String>,
}

impl OperatorEnv {
    pub fn new() -> Self {
        Self {
            prefix_ops: HashMap::new(),
            infix_ops: HashMap::new(),
            symbol_names: HashMap::new(),
        }
    }

    /// Create default operator environment with standard operators
    pub fn with_defaults(symbol_table: &HashMap<String, Symbol>) -> Self {
        let mut env = Self::new();

        // Helper to get or create symbol
        let get_sym = |name: &str| -> Option<Symbol> {
            symbol_table.get(name).copied()
        };

        // Build reverse lookup
        for (name, sym) in symbol_table {
            env.symbol_names.insert(*sym, name.clone());
        }

        // Define arithmetic operators with relative precedences
        // Using the standard precedence hierarchy:
        // || < && < == != < < <= > >= < + - ++ < * / % < **

        if let (Some(or_sym), Some(and_sym)) = (get_sym("||"), get_sym("&&")) {
            env.add_infix(InfixOperator::new(or_sym, Protocol::Automatic, Assoc::Left)
                .with_default(Relation::Weaker));

            env.add_infix(InfixOperator::new(and_sym, Protocol::Automatic, Assoc::Left)
                .with_prec(or_sym, Relation::Stronger)
                .with_default(Relation::Weaker));
        }

        if let Some(eq_sym) = get_sym("==") {
            let mut op = InfixOperator::new(eq_sym, Protocol::Automatic, Assoc::Left);
            if let Some(and_sym) = get_sym("&&") {
                op = op.with_prec(and_sym, Relation::Stronger);
            }
            if let Some(or_sym) = get_sym("||") {
                op = op.with_prec(or_sym, Relation::Stronger);
            }
            env.add_infix(op.with_default(Relation::Weaker));
        }

        if let Some(ne_sym) = get_sym("!=") {
            let mut op = InfixOperator::new(ne_sym, Protocol::Automatic, Assoc::Left);
            if let Some(eq_sym) = get_sym("==") {
                op = op.with_prec(eq_sym, Relation::Same);
            }
            env.add_infix(op.with_default(Relation::Weaker));
        }

        // Comparison operators
        for name in &["<", "<=", ">", ">="] {
            if let Some(sym) = get_sym(name) {
                let mut op = InfixOperator::new(sym, Protocol::Automatic, Assoc::Left);
                if let Some(eq_sym) = get_sym("==") {
                    op = op.with_prec(eq_sym, Relation::Stronger);
                }
                env.add_infix(op.with_default(Relation::Weaker));
            }
        }

        // Additive operators: +, -, ++
        for name in &["+", "-", "++"] {
            if let Some(sym) = get_sym(name) {
                let mut op = InfixOperator::new(sym, Protocol::Automatic, Assoc::Left);
                if let Some(lt_sym) = get_sym("<") {
                    op = op.with_prec(lt_sym, Relation::Stronger);
                }
                if let Some(eq_sym) = get_sym("==") {
                    op = op.with_prec(eq_sym, Relation::Stronger);
                }
                env.add_infix(op.with_default(Relation::Weaker));
            }
        }

        // Multiplicative operators: *, /, %
        for name in &["*", "/", "%"] {
            if let Some(sym) = get_sym(name) {
                let mut op = InfixOperator::new(sym, Protocol::Automatic, Assoc::Left);
                if let Some(plus_sym) = get_sym("+") {
                    op = op.with_prec(plus_sym, Relation::Stronger);
                }
                if let Some(minus_sym) = get_sym("-") {
                    op = op.with_prec(minus_sym, Relation::Stronger);
                }
                env.add_infix(op.with_default(Relation::Weaker));
            }
        }

        // Exponentiation: ** (right-associative)
        if let Some(pow_sym) = get_sym("**") {
            let mut op = InfixOperator::new(pow_sym, Protocol::Automatic, Assoc::Right);
            if let Some(mul_sym) = get_sym("*") {
                op = op.with_prec(mul_sym, Relation::Stronger);
            }
            env.add_infix(op);
        }

        // Assignment: = (right-associative, lowest precedence)
        if let Some(eq_sym) = get_sym("=") {
            env.add_infix(InfixOperator::new(eq_sym, Protocol::Automatic, Assoc::Right)
                .with_default(Relation::Weaker));
        }

        // Prefix operators
        if let Some(minus_sym) = get_sym("-") {
            env.add_prefix(PrefixOperator::new(minus_sym, Protocol::Automatic));
        }
        if let Some(bang_sym) = get_sym("!") {
            env.add_prefix(PrefixOperator::new(bang_sym, Protocol::Automatic));
        }

        env
    }

    pub fn add_prefix(&mut self, op: PrefixOperator) {
        self.prefix_ops.insert(op.name, op);
    }

    pub fn add_infix(&mut self, op: InfixOperator) {
        self.infix_ops.insert(op.name, op);
    }

    pub fn get_prefix(&self, sym: Symbol) -> Option<&PrefixOperator> {
        self.prefix_ops.get(&sym)
    }

    pub fn get_infix(&self, sym: Symbol) -> Option<&InfixOperator> {
        self.infix_ops.get(&sym)
    }

    pub fn get_symbol_name(&self, sym: Symbol) -> Option<&str> {
        self.symbol_names.get(&sym).map(|s| s.as_str())
    }
}

impl Default for OperatorEnv {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Enforestation Engine
// =============================================================================

/// The main enforestation engine
pub struct Enforester<'a> {
    env: &'a OperatorEnv,
}

impl<'a> Enforester<'a> {
    pub fn new(env: &'a OperatorEnv) -> Self {
        Self { env }
    }

    /// Main entry point: enforest a shrubbery into an expression
    pub fn enforest(&self, shrub: &Shrubbery) -> Result<Expr> {
        match shrub {
            Shrubbery::Literal(lit, span) => Ok(Expr::Literal(lit.clone(), *span)),

            Shrubbery::Identifier(sym, scopes, span) => {
                Ok(Expr::Var(*sym, scopes.clone(), *span))
            }

            Shrubbery::Sequence(items, span) => {
                if items.is_empty() {
                    return Ok(Expr::Block(vec![], *span));
                }
                self.enforest_sequence(items, *span)
            }

            Shrubbery::Parens(items, span) => {
                // Parentheses: grouping or tuple
                if items.is_empty() {
                    Ok(Expr::Block(vec![], *span))
                } else if items.len() == 1 {
                    self.enforest(&items[0])
                } else {
                    // Multiple items - could be tuple or comma-separated
                    let exprs = items.iter().map(|s| self.enforest(s)).collect::<Result<Vec<_>>>()?;
                    Ok(Expr::Block(exprs, *span))
                }
            }

            Shrubbery::Brackets(items, span) => {
                // Brackets: content block or array
                if items.is_empty() {
                    Ok(Expr::Content(Content::Sequence(vec![]), *span))
                } else {
                    let exprs = items.iter().map(|s| self.enforest(s)).collect::<Result<Vec<_>>>()?;
                    Ok(Expr::Block(exprs, *span))
                }
            }

            Shrubbery::Braces(items, span) => {
                // Braces: block
                let exprs = items.iter().map(|s| self.enforest(s)).collect::<Result<Vec<_>>>()?;
                Ok(Expr::Block(exprs, *span))
            }

            Shrubbery::Quote { body, span } => {
                let inner = self.enforest(body)?;
                Ok(Expr::Quote(Box::new(inner), *span))
            }

            Shrubbery::Splice { expr, span } => {
                let inner = self.enforest(expr)?;
                Ok(Expr::Splice(Box::new(inner), *span))
            }

            Shrubbery::If { condition, then_branch, else_branch, span } => {
                let cond = self.enforest(condition)?;
                let then_expr = self.enforest(then_branch)?;
                let else_expr = if let Some(else_br) = else_branch {
                    Some(Box::new(self.enforest(else_br)?))
                } else {
                    None
                };
                Ok(Expr::If(Box::new(cond), Box::new(then_expr), else_expr, *span))
            }

            Shrubbery::For { pattern, iterable, body, span } => {
                let iter = self.enforest(iterable)?;
                let body_expr = self.enforest(body)?;
                Ok(Expr::For(*pattern, Box::new(iter), Box::new(body_expr), *span))
            }

            Shrubbery::Operator(sym, span) => {
                // Bare operator - could be prefix
                Err(MrlError::ParserError {
                    span: *span,
                    message: format!("Unexpected operator: {:?}", sym),
                })
            }

            // Block-level constructs: convert Shrubbery forms to Expr forms
            Shrubbery::DefBlock { name, params, return_type, body, span } => {
                // Enforest the body
                let body_exprs: Vec<Expr> = body.iter()
                    .map(|s| self.enforest(s))
                    .collect::<Result<Vec<_>>>()?;
                let body_expr = if body_exprs.len() == 1 {
                    body_exprs.into_iter().next().unwrap()
                } else {
                    Expr::Block(body_exprs, *span)
                };

                // Enforest return type if present
                let return_type_expr = if let Some(rt) = return_type {
                    Some(Box::new(self.enforest(rt)?))
                } else {
                    None
                };

                Ok(Expr::Def {
                    name: *name,
                    params: params.clone(),
                    return_type: return_type_expr,
                    body: Box::new(body_expr),
                    span: *span,
                })
            }

            Shrubbery::StagedBlock { body, span } => {
                let body_exprs: Vec<Expr> = body.iter()
                    .map(|s| self.enforest(s))
                    .collect::<Result<Vec<_>>>()?;
                let body_expr = if body_exprs.len() == 1 {
                    body_exprs.into_iter().next().unwrap()
                } else {
                    Expr::Block(body_exprs, *span)
                };
                Ok(Expr::Staged(Box::new(body_expr), *span))
            }

            Shrubbery::ShowRule { selector, transform, span } => {
                let selector_expr = self.enforest(selector)?;
                let transform_expr = self.enforest(transform)?;
                Ok(Expr::ShowRule {
                    selector: Box::new(selector_expr),
                    transform: Box::new(transform_expr),
                    span: *span,
                })
            }

            Shrubbery::SetRule { selector, properties, span } => {
                let selector_expr = self.enforest(selector)?;
                let props: Vec<(Symbol, Expr)> = properties.iter()
                    .map(|(sym, shrub)| {
                        self.enforest(shrub).map(|e| (*sym, e))
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(Expr::SetRule {
                    selector: Box::new(selector_expr),
                    properties: props,
                    span: *span,
                })
            }

            Shrubbery::LiveBlock { deps, body, span } => {
                let body_exprs: Vec<Expr> = body.iter()
                    .map(|s| self.enforest(s))
                    .collect::<Result<Vec<_>>>()?;
                let body_expr = if body_exprs.len() == 1 {
                    body_exprs.into_iter().next().unwrap()
                } else {
                    Expr::Block(body_exprs, *span)
                };
                Ok(Expr::Live {
                    deps: deps.clone(),
                    body: Box::new(body_expr),
                    span: *span,
                })
            }

            Shrubbery::Selector { base, predicate, span } => {
                // Selectors become variable references with optional predicate call
                if let Some(pred) = predicate {
                    let pred_expr = self.enforest(pred)?;
                    // Represent as: base.where(pred)
                    let base_expr = Expr::Var(*base, ScopeSet::new(), *span);
                    // For now, just return the base - full selector handling in interpreter
                    Ok(Expr::Call(
                        Box::new(base_expr),
                        vec![pred_expr],
                        *span,
                    ))
                } else {
                    Ok(Expr::Var(*base, ScopeSet::new(), *span))
                }
            }

            Shrubbery::Prose(text, span) => {
                // Prose becomes a content literal
                Ok(Expr::Content(
                    Content::Inline(Inline::Text(text.clone())),
                    *span,
                ))
            }

            Shrubbery::ContentBlock(items, span) => {
                let exprs: Vec<Expr> = items.iter()
                    .map(|s| self.enforest(s))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Expr::Block(exprs, *span))
            }
        }
    }

    /// Enforest a sequence using precedence climbing
    fn enforest_sequence(&self, items: &[Shrubbery], span: Span) -> Result<Expr> {
        if items.is_empty() {
            return Ok(Expr::Block(vec![], span));
        }

        // Convert to a working list
        let mut stxes: Vec<&Shrubbery> = items.iter().collect();

        // Enforce the sequence
        let (expr, remaining) = self.enforest_step(&mut stxes, None)?;

        if remaining.is_empty() {
            Ok(expr)
        } else {
            // Continue enforesting remaining items
            let mut exprs = vec![expr];
            let mut tail = remaining;
            while !tail.is_empty() {
                let (next_expr, new_tail) = self.enforest_step(&mut tail.iter().copied().collect(), None)?;
                exprs.push(next_expr);
                tail = new_tail;
            }

            if exprs.len() == 1 {
                Ok(exprs.into_iter().next().unwrap())
            } else {
                Ok(Expr::Block(exprs, span))
            }
        }
    }

    /// Single enforest step: parse one form and return (form, remaining_tail)
    fn enforest_step<'b>(
        &self,
        stxes: &mut Vec<&'b Shrubbery>,
        current_op: Option<&InfixOperator>,
    ) -> Result<(Expr, Vec<&'b Shrubbery>)> {
        if stxes.is_empty() {
            return Err(MrlError::ParserError {
                span: Span::default(),
                message: "Unexpected end of expression".to_string(),
            });
        }

        let head = stxes.remove(0);
        let start_span = head.span();

        // Dispatch based on head type
        let form = match head {
            Shrubbery::Literal(lit, span) => Expr::Literal(lit.clone(), *span),

            Shrubbery::Identifier(sym, scopes, span) => {
                // Check if this is a prefix operator
                if let Some(_prefix_op) = self.env.get_prefix(*sym) {
                    // Parse the operand
                    let (operand, tail) = self.enforest_step(stxes, None)?;
                    *stxes = tail;

                    // Determine unary operator
                    let unop = match self.env.get_symbol_name(*sym) {
                        Some("-") => UnOp::Neg,
                        Some("!") => UnOp::Not,
                        _ => {
                            return Err(MrlError::ParserError {
                                span: *span,
                                message: format!("Unknown prefix operator: {:?}", sym),
                            });
                        }
                    };

                    let combined_span = Span::new(span.start, operand.span().end);
                    Expr::UnOp(unop, Box::new(operand), combined_span)
                } else {
                    Expr::Var(*sym, scopes.clone(), *span)
                }
            }

            Shrubbery::Operator(sym, span) => {
                // Prefix operator
                if let Some(_prefix_op) = self.env.get_prefix(*sym) {
                    let (operand, tail) = self.enforest_step(stxes, None)?;
                    *stxes = tail;

                    let unop = match self.env.get_symbol_name(*sym) {
                        Some("-") => UnOp::Neg,
                        Some("!") => UnOp::Not,
                        _ => {
                            return Err(MrlError::ParserError {
                                span: *span,
                                message: format!("Unknown prefix operator: {:?}", sym),
                            });
                        }
                    };

                    let combined_span = Span::new(span.start, operand.span().end);
                    Expr::UnOp(unop, Box::new(operand), combined_span)
                } else {
                    return Err(MrlError::ParserError {
                        span: *span,
                        message: format!("Unexpected operator: {:?}", sym),
                    });
                }
            }

            Shrubbery::Parens(items, span) => {
                // Check if this is a call (previous form was identifier)
                if items.is_empty() {
                    Expr::Block(vec![], *span)
                } else if items.len() == 1 {
                    self.enforest(&items[0])?
                } else {
                    let exprs = items.iter().map(|s| self.enforest(s)).collect::<Result<Vec<_>>>()?;
                    Expr::Block(exprs, *span)
                }
            }

            Shrubbery::Brackets(items, span) => {
                let exprs = items.iter().map(|s| self.enforest(s)).collect::<Result<Vec<_>>>()?;
                Expr::Block(exprs, *span)
            }

            Shrubbery::Quote { body, span } => {
                let inner = self.enforest(body)?;
                Expr::Quote(Box::new(inner), *span)
            }

            Shrubbery::Splice { expr, span } => {
                let inner = self.enforest(expr)?;
                Expr::Splice(Box::new(inner), *span)
            }

            other => {
                // Recursively enforest other types
                self.enforest(other)?
            }
        };

        // Now check for infix operators or implicit call
        self.enforest_infix(form, stxes, current_op, start_span)
    }

    /// Handle infix operators using precedence climbing
    fn enforest_infix<'b>(
        &self,
        mut left: Expr,
        stxes: &mut Vec<&'b Shrubbery>,
        current_op: Option<&InfixOperator>,
        start_span: Span,
    ) -> Result<(Expr, Vec<&'b Shrubbery>)> {
        loop {
            if stxes.is_empty() {
                return Ok((left, vec![]));
            }

            let next = stxes[0];

            // Check for infix operator
            let (op_sym, op_span) = match next {
                Shrubbery::Operator(sym, span) => (*sym, *span),
                Shrubbery::Identifier(sym, _, span) => {
                    // Check if identifier is an infix operator
                    if self.env.get_infix(*sym).is_some() {
                        (*sym, *span)
                    } else {
                        // Implicit call or juxtaposition?
                        // Check if next is parens (function call)
                        if let Shrubbery::Parens(args, pspan) = next {
                            stxes.remove(0);
                            let arg_exprs = args.iter()
                                .map(|s| self.enforest(s))
                                .collect::<Result<Vec<_>>>()?;
                            let combined_span = Span::new(start_span.start, pspan.end);
                            left = Expr::Call(Box::new(left), arg_exprs, combined_span);
                            continue;
                        }
                        // Not an operator - return what we have
                        return Ok((left, stxes.clone()));
                    }
                }
                // Check for implicit call: expr followed by parens
                Shrubbery::Parens(args, pspan) => {
                    stxes.remove(0);
                    let arg_exprs = args.iter()
                        .map(|s| self.enforest(s))
                        .collect::<Result<Vec<_>>>()?;
                    let combined_span = Span::new(start_span.start, pspan.end);
                    left = Expr::Call(Box::new(left), arg_exprs, combined_span);
                    continue;
                }
                // Check for subscript: expr followed by brackets
                Shrubbery::Brackets(items, bspan) => {
                    if items.len() == 1 {
                        stxes.remove(0);
                        let index = self.enforest(&items[0])?;
                        let combined_span = Span::new(start_span.start, bspan.end);
                        left = Expr::Subscript(Box::new(left), Box::new(index), combined_span);
                        continue;
                    }
                    return Ok((left, stxes.clone()));
                }
                _ => {
                    // Not an operator - return what we have
                    return Ok((left, stxes.clone()));
                }
            };

            // Get the infix operator
            let infix_op = match self.env.get_infix(op_sym) {
                Some(op) => op,
                None => return Ok((left, stxes.clone())),
            };

            // Check precedence against current operator
            if let Some(curr_op) = current_op {
                let prec = relative_precedence(curr_op, infix_op);
                match prec {
                    PrecResult::Weaker | PrecResult::SameOnLeft => {
                        // Right op is weaker - return and let caller handle it
                        return Ok((left, stxes.clone()));
                    }
                    PrecResult::Inconsistent => {
                        return Err(MrlError::ParserError {
                            span: op_span,
                            message: format!(
                                "Inconsistent precedence between {:?} and {:?}",
                                curr_op.name, infix_op.name
                            ),
                        });
                    }
                    _ => {
                        // Continue parsing with this operator
                    }
                }
            }

            // Consume the operator
            stxes.remove(0);

            // Parse the right operand
            let (right, tail) = self.enforest_step(stxes, Some(infix_op))?;
            *stxes = tail;

            // Build the binary expression
            let op_name = self.env.get_symbol_name(op_sym).unwrap_or("");
            let binop = BinOp::from_str(op_name).ok_or_else(|| MrlError::ParserError {
                span: op_span,
                message: format!("Unknown binary operator: {}", op_name),
            })?;

            let combined_span = Span::new(left.span().start, right.span().end);
            left = Expr::BinOp(Box::new(left), binop, Box::new(right), combined_span);
        }
    }
}

// =============================================================================
// Public API
// =============================================================================

/// Enforest a shrubbery into an expression using default operator environment
pub fn enforest(shrub: &Shrubbery) -> Result<Expr> {
    let env = OperatorEnv::new();
    let enforester = Enforester::new(&env);
    enforester.enforest(shrub)
}

/// Enforest with a custom operator environment
pub fn enforest_with_env(shrub: &Shrubbery, env: &OperatorEnv) -> Result<Expr> {
    let enforester = Enforester::new(env);
    enforester.enforest(shrub)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shrubbery::Symbol;

    #[test]
    fn test_enforest_literal() {
        let shrub = Shrubbery::Literal(Literal::Int(42), Span::new(0, 2));
        let expr = enforest(&shrub).unwrap();
        assert!(matches!(expr, Expr::Literal(Literal::Int(42), _)));
    }

    #[test]
    fn test_enforest_identifier() {
        let shrub = Shrubbery::Identifier(Symbol::new(1), ScopeSet::new(), Span::new(0, 3));
        let expr = enforest(&shrub).unwrap();
        if let Expr::Var(sym, _, _) = expr {
            assert_eq!(sym.id(), 1);
        } else {
            panic!("Expected Expr::Var");
        }
    }

    #[test]
    fn test_enforest_if() {
        let condition = Shrubbery::Literal(Literal::Bool(true), Span::new(0, 4));
        let then_branch = Shrubbery::Literal(Literal::Int(1), Span::new(5, 6));
        let shrub = Shrubbery::If {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch: None,
            span: Span::new(0, 6),
        };

        let expr = enforest(&shrub).unwrap();
        assert!(matches!(expr, Expr::If(_, _, None, _)));
    }

    #[test]
    fn test_relative_precedence() {
        let plus = InfixOperator::new(Symbol::new(1), Protocol::Automatic, Assoc::Left);
        let times = InfixOperator::new(Symbol::new(2), Protocol::Automatic, Assoc::Left)
            .with_prec(Symbol::new(1), Relation::Stronger);

        // times is stronger than plus
        let prec = relative_precedence(&plus, &times);
        assert_eq!(prec, PrecResult::Stronger);

        // plus is weaker than times
        let prec = relative_precedence(&times, &plus);
        assert_eq!(prec, PrecResult::Weaker);
    }

    #[test]
    fn test_operator_env_defaults() {
        let mut symbol_table = HashMap::new();
        symbol_table.insert("+".to_string(), Symbol::new(1));
        symbol_table.insert("*".to_string(), Symbol::new(2));
        symbol_table.insert("-".to_string(), Symbol::new(3));

        let env = OperatorEnv::with_defaults(&symbol_table);

        // Check that operators are defined
        assert!(env.get_infix(Symbol::new(1)).is_some());
        assert!(env.get_infix(Symbol::new(2)).is_some());
        assert!(env.get_prefix(Symbol::new(3)).is_some());
    }

    #[test]
    fn test_enforest_def_block() {
        let name = Symbol::new(10);
        let body = vec![Shrubbery::Literal(Literal::Int(42), Span::new(10, 12))];
        let shrub = Shrubbery::DefBlock {
            name,
            params: vec![],
            return_type: None,
            body,
            span: Span::new(0, 12),
        };

        let expr = enforest(&shrub).unwrap();
        match expr {
            Expr::Def { name: n, params, return_type, body, .. } => {
                assert_eq!(n.id(), 10);
                assert!(params.is_empty());
                assert!(return_type.is_none());
                assert!(matches!(*body, Expr::Literal(Literal::Int(42), _)));
            }
            _ => panic!("Expected Expr::Def"),
        }
    }

    #[test]
    fn test_enforest_staged_block() {
        let body = vec![
            Shrubbery::Literal(Literal::String("hello".to_string()), Span::new(0, 7)),
        ];
        let shrub = Shrubbery::StagedBlock {
            body,
            span: Span::new(0, 10),
        };

        let expr = enforest(&shrub).unwrap();
        assert!(matches!(expr, Expr::Staged(_, _)));
    }

    #[test]
    fn test_enforest_show_rule() {
        let selector = Box::new(Shrubbery::Identifier(Symbol::new(1), ScopeSet::new(), Span::new(5, 12)));
        let transform = Box::new(Shrubbery::Literal(Literal::String("bold".to_string()), Span::new(14, 20)));
        let shrub = Shrubbery::ShowRule {
            selector,
            transform,
            span: Span::new(0, 20),
        };

        let expr = enforest(&shrub).unwrap();
        match expr {
            Expr::ShowRule { selector, transform, .. } => {
                assert!(matches!(*selector, Expr::Var(_, _, _)));
                assert!(matches!(*transform, Expr::Literal(Literal::String(_), _)));
            }
            _ => panic!("Expected Expr::ShowRule"),
        }
    }

    #[test]
    fn test_enforest_set_rule() {
        let selector = Box::new(Shrubbery::Identifier(Symbol::new(1), ScopeSet::new(), Span::new(4, 11)));
        let properties = vec![
            (Symbol::new(2), Shrubbery::Literal(Literal::Int(14), Span::new(20, 22))),
        ];
        let shrub = Shrubbery::SetRule {
            selector,
            properties,
            span: Span::new(0, 25),
        };

        let expr = enforest(&shrub).unwrap();
        match expr {
            Expr::SetRule { selector, properties, .. } => {
                assert!(matches!(*selector, Expr::Var(_, _, _)));
                assert_eq!(properties.len(), 1);
                assert_eq!(properties[0].0.id(), 2);
            }
            _ => panic!("Expected Expr::SetRule"),
        }
    }

    #[test]
    fn test_enforest_live_block() {
        let body = vec![Shrubbery::Literal(Literal::Int(1), Span::new(5, 6))];
        let deps = Some(vec![Symbol::new(100), Symbol::new(101)]);
        let shrub = Shrubbery::LiveBlock {
            deps,
            body,
            span: Span::new(0, 10),
        };

        let expr = enforest(&shrub).unwrap();
        match expr {
            Expr::Live { deps, body, .. } => {
                assert!(deps.is_some());
                assert_eq!(deps.unwrap().len(), 2);
                assert!(matches!(*body, Expr::Literal(Literal::Int(1), _)));
            }
            _ => panic!("Expected Expr::Live"),
        }
    }

    #[test]
    fn test_enforest_prose() {
        let shrub = Shrubbery::Prose("Hello world".to_string(), Span::new(0, 11));
        let expr = enforest(&shrub).unwrap();
        assert!(matches!(expr, Expr::Content(_, _)));
    }

    #[test]
    fn test_enforest_selector_simple() {
        let shrub = Shrubbery::Selector {
            base: Symbol::new(5),
            predicate: None,
            span: Span::new(0, 8),
        };

        let expr = enforest(&shrub).unwrap();
        assert!(matches!(expr, Expr::Var(_, _, _)));
    }

    #[test]
    fn test_enforest_selector_with_predicate() {
        let pred = Box::new(Shrubbery::Literal(Literal::Int(1), Span::new(10, 11)));
        let shrub = Shrubbery::Selector {
            base: Symbol::new(5),
            predicate: Some(pred),
            span: Span::new(0, 15),
        };

        let expr = enforest(&shrub).unwrap();
        assert!(matches!(expr, Expr::Call(_, _, _)));
    }
}
