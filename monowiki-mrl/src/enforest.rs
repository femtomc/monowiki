//! Enforestation: resolve operator precedence in shrubbery to typed AST
//!
//! This module converts flat shrubbery with operators into a properly structured AST
//! using precedence rules. This is the "enforestation" step described in the Racket
//! literature - growing a shrubbery (flat token tree) into a proper tree.

use crate::error::{MrlError, Result, Span};
use crate::shrubbery::{Literal, Shrubbery, Symbol};
use crate::content::*;

/// Operator associativity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assoc {
    Left,
    Right,
}

/// Operator precedence table (higher number = tighter binding)
/// Precedence levels from lowest to highest:
/// 1. || (logical OR)
/// 2. && (logical AND)
/// 3. ==, != (equality)
/// 4. <, <=, >, >= (comparison)
/// 5. +, -, ++ (additive, string concat)
/// 6. *, /, % (multiplicative)
/// 7. ** (exponentiation, right-associative)
pub const PRECEDENCE: &[(&[&str], u8, Assoc)] = &[
    (&["||"], 1, Assoc::Left),
    (&["&&"], 2, Assoc::Left),
    (&["==", "!="], 3, Assoc::Left),
    (&["<", "<=", ">", ">="], 4, Assoc::Left),
    (&["+", "-", "++"], 5, Assoc::Left),
    (&["*", "/", "%"], 6, Assoc::Left),
    (&["**"], 7, Assoc::Right),
];

/// Binary operator
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
}

impl BinOp {
    fn from_symbol(sym: &str) -> Option<Self> {
        match sym {
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
    Var(Symbol, Span),
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
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal(_, s) => *s,
            Expr::Var(_, s) => *s,
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
        }
    }
}

/// Get operator precedence and associativity
fn get_precedence(op: &str) -> Option<(u8, Assoc)> {
    for (ops, prec, assoc) in PRECEDENCE {
        if ops.contains(&op) {
            return Some((*prec, *assoc));
        }
    }
    None
}

/// Enforest a shrubbery into an expression
/// This is a simplified Pratt parser-style implementation
pub fn enforest(shrub: &Shrubbery) -> Result<Expr> {
    match shrub {
        Shrubbery::Literal(lit, span) => Ok(Expr::Literal(lit.clone(), *span)),

        Shrubbery::Identifier(sym, _, span) => Ok(Expr::Var(*sym, *span)),

        Shrubbery::Sequence(items, span) => {
            if items.is_empty() {
                return Ok(Expr::Block(vec![], *span));
            }

            // Try to parse as binary expression with operators
            if items.len() > 1 {
                if let Some(expr) = try_parse_binop(items)? {
                    return Ok(expr);
                }
            }

            // Otherwise, treat as a block of statements
            let exprs = items.iter().map(enforest).collect::<Result<Vec<_>>>()?;
            if exprs.len() == 1 {
                Ok(exprs.into_iter().next().unwrap())
            } else {
                Ok(Expr::Block(exprs, *span))
            }
        }

        Shrubbery::Parens(items, span) => {
            // Check if this is a function call or just grouping
            if items.is_empty() {
                Ok(Expr::Block(vec![], *span))
            } else {
                let exprs = items.iter().map(enforest).collect::<Result<Vec<_>>>()?;
                if exprs.len() == 1 {
                    Ok(exprs.into_iter().next().unwrap())
                } else {
                    Ok(Expr::Block(exprs, *span))
                }
            }
        }

        Shrubbery::Brackets(items, span) => {
            // Content block
            Ok(Expr::Content(Content::Block(Block::Paragraph {
                body: Box::new(Inline::Text("".to_string())),
                attrs: Attributes::default(),
            }), *span))
        }

        Shrubbery::Quote { body, span } => {
            let inner = enforest(body)?;
            Ok(Expr::Quote(Box::new(inner), *span))
        }

        Shrubbery::Splice { expr, span } => {
            let inner = enforest(expr)?;
            Ok(Expr::Splice(Box::new(inner), *span))
        }

        Shrubbery::If { condition, then_branch, else_branch, span } => {
            let cond = enforest(condition)?;
            let then_expr = enforest(then_branch)?;
            let else_expr = if let Some(else_br) = else_branch {
                Some(Box::new(enforest(else_br)?))
            } else {
                None
            };
            Ok(Expr::If(Box::new(cond), Box::new(then_expr), else_expr, *span))
        }

        Shrubbery::For { pattern, iterable, body, span } => {
            let iter = enforest(iterable)?;
            let body_expr = enforest(body)?;
            Ok(Expr::For(*pattern, Box::new(iter), Box::new(body_expr), *span))
        }

        // Block constructs don't enforest - they stay as shrubbery
        _ => Err(MrlError::ParserError {
            span: shrub.span(),
            message: format!("Cannot enforest this shrubbery type: {:?}", shrub),
        }),
    }
}

/// Try to parse a sequence of items as a binary operation
fn try_parse_binop(items: &[Shrubbery]) -> Result<Option<Expr>> {
    // Look for operators in the sequence
    let mut has_operator = false;
    for item in items {
        if matches!(item, Shrubbery::Operator(_, _)) {
            has_operator = true;
            break;
        }
    }

    if !has_operator {
        return Ok(None);
    }

    // Use a simple precedence climbing algorithm
    // For now, just find the lowest precedence operator and split there
    let mut lowest_prec = u8::MAX;
    let mut split_idx = None;

    for (i, item) in items.iter().enumerate() {
        if let Shrubbery::Operator(sym, _) = item {
            // Get operator name from symbol table (we don't have access here, so use a placeholder)
            // In a real implementation, we'd need to pass the symbol table
            let op_str = "+"; // Placeholder
            if let Some((prec, _)) = get_precedence(op_str) {
                if prec <= lowest_prec {
                    lowest_prec = prec;
                    split_idx = Some(i);
                }
            }
        }
    }

    if let Some(idx) = split_idx {
        if idx == 0 || idx == items.len() - 1 {
            // Unary operator or malformed
            return Ok(None);
        }

        // Split into left, operator, right
        let left_shrub = if items[0..idx].len() == 1 {
            &items[0]
        } else {
            return Ok(None); // For now, simplified
        };

        let right_shrub = if items[idx+1..].len() == 1 {
            &items[idx + 1]
        } else {
            return Ok(None); // For now, simplified
        };

        let left = enforest(left_shrub)?;
        let right = enforest(right_shrub)?;

        if let Shrubbery::Operator(_, span) = &items[idx] {
            // Placeholder: assume addition
            let op = BinOp::Add;
            let combined_span = Span::new(left.span().start, right.span().end);
            return Ok(Some(Expr::BinOp(Box::new(left), op, Box::new(right), combined_span)));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shrubbery::{ScopeSet, Symbol};

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
        assert!(matches!(expr, Expr::Var(Symbol(1), _)));
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
}
