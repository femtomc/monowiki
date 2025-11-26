use crate::error::{MrlError, Result, Span};
use crate::lexer::{SpannedToken, Token};
use crate::shrubbery::{Literal, Param, Scope, ScopeSet, Shrubbery, Symbol};
use std::collections::HashMap;

/// Symbol table for interning identifiers
pub struct SymbolTable {
    symbols: HashMap<String, Symbol>,
    next_id: u64,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn intern(&mut self, name: &str) -> Symbol {
        if let Some(&sym) = self.symbols.get(name) {
            sym
        } else {
            let sym = Symbol::new(self.next_id);
            self.next_id += 1;
            self.symbols.insert(name.to_string(), sym);
            sym
        }
    }

    pub fn get(&self, name: &str) -> Option<Symbol> {
        self.symbols.get(name).copied()
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Parser for MRL source
pub struct Parser<'a> {
    tokens: &'a [SpannedToken],
    pos: usize,
    symbols: SymbolTable,
    next_scope_id: u64,
    /// Track indentation levels for Python-style blocks
    indent_stack: Vec<usize>,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [SpannedToken]) -> Self {
        Self {
            tokens,
            pos: 0,
            symbols: SymbolTable::new(),
            next_scope_id: 0,
            indent_stack: vec![0],
        }
    }

    /// Create a fresh scope
    pub fn fresh_scope(&mut self) -> Scope {
        let scope = Scope::new(self.next_scope_id);
        self.next_scope_id += 1;
        scope
    }

    /// Peek at current token
    fn peek(&self) -> Option<&SpannedToken> {
        self.tokens.get(self.pos)
    }

    /// Advance to next token
    fn advance(&mut self) -> Option<&SpannedToken> {
        let token = self.tokens.get(self.pos);
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    /// Check if current token matches expected
    fn check(&self, expected: &Token) -> bool {
        self.peek()
            .map(|t| std::mem::discriminant(&t.token) == std::mem::discriminant(expected))
            .unwrap_or(false)
    }

    /// Consume token if it matches
    fn consume(&mut self, expected: &Token) -> Option<&SpannedToken> {
        if self.check(expected) {
            self.advance()
        } else {
            None
        }
    }

    /// Expect a token, error if not found
    fn expect(&mut self, expected: Token) -> Result<&SpannedToken> {
        if self.check(&expected) {
            Ok(self.advance().unwrap())
        } else {
            let span = self.peek().map(|t| t.span).unwrap_or(Span::default());
            Err(MrlError::ParserError {
                span,
                message: format!("Expected {:?}", expected),
            })
        }
    }

    /// Check if at end of input
    fn is_eof(&self) -> bool {
        self.peek()
            .map(|t| matches!(t.token, Token::Eof))
            .unwrap_or(true)
    }

    /// Parse the entire input as a shrubbery
    pub fn parse(&mut self) -> Result<Shrubbery> {
        let mut elements = Vec::new();
        let start = self.peek().map(|t| t.span.start).unwrap_or(0);

        while !self.is_eof() {
            if let Some(element) = self.parse_element()? {
                elements.push(element);
            } else {
                break;
            }
        }

        let end = self.peek().map(|t| t.span.start).unwrap_or(start);
        let span = Span::new(start, end);

        if elements.len() == 1 {
            Ok(elements.into_iter().next().unwrap())
        } else {
            Ok(Shrubbery::Sequence(elements, span))
        }
    }

    /// Check if current token is an operator (including assignment for expressions)
    fn is_operator(&self) -> bool {
        matches!(
            self.peek().map(|t| &t.token),
            Some(Token::Plus)
                | Some(Token::Minus)
                | Some(Token::Star)
                | Some(Token::Slash)
                | Some(Token::Percent)
                | Some(Token::EqEq)
                | Some(Token::NotEq)
                | Some(Token::Lt)
                | Some(Token::Le)
                | Some(Token::Gt)
                | Some(Token::Ge)
                | Some(Token::AndAnd)
                | Some(Token::OrOr)
                | Some(Token::PlusPlus)
                | Some(Token::Eq) // assignment
        )
    }

    /// Parse a single element
    fn parse_element(&mut self) -> Result<Option<Shrubbery>> {
        // Skip newlines
        while self.check(&Token::Newline) {
            self.advance();
        }

        if self.is_eof() {
            return Ok(None);
        }

        let token = self.peek().unwrap();
        let result = match &token.token {
            // Check for block constructs first
            Token::Bang => {
                // Peek ahead to see if this is a block keyword
                if self.pos + 1 < self.tokens.len() {
                    match &self.tokens[self.pos + 1].token {
                        Token::Def => return Ok(Some(self.parse_def()?)),
                        Token::Staged => return Ok(Some(self.parse_staged()?)),
                        Token::Show => return Ok(Some(self.parse_show()?)),
                        Token::Set => return Ok(Some(self.parse_set()?)),
                        Token::Live => return Ok(Some(self.parse_live()?)),
                        _ => {}
                    }
                }
                Some(self.parse_operator()?)
            }
            Token::Quote => Some(self.parse_quote()?),
            Token::Splice => Some(self.parse_splice_keyword()?),
            Token::Dollar => Some(self.parse_splice_dollar()?),
            Token::If => Some(self.parse_if()?),
            Token::For => Some(self.parse_for()?),
            Token::Identifier(_) => Some(self.parse_identifier()?),
            Token::IntLiteral(_) | Token::FloatLiteral(_) | Token::StringLiteral(_)
            | Token::SymbolLiteral(_) | Token::True | Token::False | Token::None => {
                Some(self.parse_literal()?)
            }
            Token::LParen => Some(self.parse_parens()?),
            Token::LBracket => Some(self.parse_brackets()?),
            Token::LBrace => Some(self.parse_braces()?),
            Token::Plus | Token::Minus | Token::Star | Token::Slash | Token::Percent
            | Token::EqEq | Token::NotEq | Token::Lt | Token::Le | Token::Gt | Token::Ge
            | Token::AndAnd | Token::OrOr | Token::PlusPlus => Some(self.parse_operator()?),
            _ => {
                // Unknown token, skip it
                self.advance();
                None
            }
        };

        Ok(result)
    }

    /// Parse an expression (primary element followed by optional operators and operands)
    /// Also handles function calls: identifier(args)
    fn parse_expression(&mut self) -> Result<Option<Shrubbery>> {
        let first = self.parse_element()?;
        if first.is_none() {
            return Ok(None);
        }

        let mut elements = vec![first.unwrap()];
        let start = elements[0].span().start;

        // Check for function call syntax: identifier(...)
        // This handles `text("one")`, `paragraph(item)`, etc.
        if self.check(&Token::LParen) {
            let args = self.parse_parens()?;
            elements.push(args);
        }

        // Continue parsing while we see operators
        while self.is_operator() {
            elements.push(self.parse_operator()?);
            if let Some(operand) = self.parse_element()? {
                elements.push(operand);
                // Check for function call after operand too
                if self.check(&Token::LParen) {
                    let args = self.parse_parens()?;
                    elements.push(args);
                }
            } else {
                break;
            }
        }

        if elements.len() == 1 {
            Ok(Some(elements.into_iter().next().unwrap()))
        } else {
            let end = elements.last().map(|e| e.span().end).unwrap_or(start);
            Ok(Some(Shrubbery::Sequence(elements, Span::new(start, end))))
        }
    }

    /// Parse an identifier
    /// Note: Function calls like `foo(...)` are handled at a higher level - here we just
    /// return the identifier. The shrubbery model keeps things flat for later enforestation.
    fn parse_identifier(&mut self) -> Result<Shrubbery> {
        let token = self.expect(Token::Identifier(String::new()))?;
        let (name_clone, span) = if let Token::Identifier(name) = &token.token {
            (name.clone(), token.span)
        } else {
            unreachable!()
        };

        let sym = self.symbols.intern(&name_clone);
        Ok(Shrubbery::Identifier(sym, ScopeSet::new(), span))
    }

    /// Parse a literal
    fn parse_literal(&mut self) -> Result<Shrubbery> {
        let token = self.advance().unwrap();
        let lit = match &token.token {
            Token::IntLiteral(Some(i)) => Literal::Int(*i),
            Token::IntLiteral(None) => {
                return Err(MrlError::ParserError {
                    span: token.span,
                    message: "Invalid integer literal".to_string(),
                });
            }
            Token::FloatLiteral(Some(f)) => Literal::Float(*f),
            Token::FloatLiteral(None) => {
                return Err(MrlError::ParserError {
                    span: token.span,
                    message: "Invalid float literal".to_string(),
                });
            }
            Token::StringLiteral(s) => Literal::String(s.clone()),
            Token::SymbolLiteral(s) => Literal::Symbol(s.clone()),
            Token::True => Literal::Bool(true),
            Token::False => Literal::Bool(false),
            Token::None => Literal::None,
            _ => unreachable!(),
        };
        Ok(Shrubbery::Literal(lit, token.span))
    }

    /// Parse parenthesized expression
    fn parse_parens(&mut self) -> Result<Shrubbery> {
        let start_token = self.expect(Token::LParen)?;
        let start = start_token.span.start;
        let mut elements = Vec::new();

        while !self.check(&Token::RParen) && !self.is_eof() {
            if let Some(elem) = self.parse_element()? {
                elements.push(elem);
            }

            // Handle comma separation
            if self.check(&Token::Comma) {
                self.advance();
            }
        }

        let end_token = self.expect(Token::RParen)?;
        let end = end_token.span.end;
        let span = Span::new(start, end);

        Ok(Shrubbery::Parens(elements, span))
    }

    /// Parse bracketed expression
    fn parse_brackets(&mut self) -> Result<Shrubbery> {
        let start_token = self.expect(Token::LBracket)?;
        let start = start_token.span.start;
        let mut elements = Vec::new();

        while !self.check(&Token::RBracket) && !self.is_eof() {
            if let Some(elem) = self.parse_element()? {
                elements.push(elem);
            }

            // Handle comma separation
            if self.check(&Token::Comma) {
                self.advance();
            }
        }

        let end_token = self.expect(Token::RBracket)?;
        let end = end_token.span.end;
        let span = Span::new(start, end);

        Ok(Shrubbery::Brackets(elements, span))
    }

    /// Parse braced expression
    fn parse_braces(&mut self) -> Result<Shrubbery> {
        let start_token = self.expect(Token::LBrace)?;
        let start = start_token.span.start;
        let mut elements = Vec::new();

        while !self.check(&Token::RBrace) && !self.is_eof() {
            if let Some(elem) = self.parse_element()? {
                elements.push(elem);
            }

            // Handle comma separation
            if self.check(&Token::Comma) {
                self.advance();
            }
        }

        let end_token = self.expect(Token::RBrace)?;
        let end = end_token.span.end;
        let span = Span::new(start, end);

        Ok(Shrubbery::Braces(elements, span))
    }

    /// Parse an operator
    fn parse_operator(&mut self) -> Result<Shrubbery> {
        let token = self.advance().unwrap();
        let span = token.span;
        let op_name = match &token.token {
            Token::Plus => "+",
            Token::Minus => "-",
            Token::Star => "*",
            Token::Slash => "/",
            Token::Percent => "%",
            Token::EqEq => "==",
            Token::NotEq => "!=",
            Token::Lt => "<",
            Token::Le => "<=",
            Token::Gt => ">",
            Token::Ge => ">=",
            Token::AndAnd => "&&",
            Token::OrOr => "||",
            Token::Bang => "!",
            Token::PlusPlus => "++",
            Token::Eq => "=",
            _ => unreachable!(),
        };
        let sym = self.symbols.intern(op_name);
        Ok(Shrubbery::Operator(sym, span))
    }

    // ===== Block-level parsing =====

    /// Parse !def name(params): body
    fn parse_def(&mut self) -> Result<Shrubbery> {
        let start_token = self.expect(Token::Bang)?;
        let start = start_token.span.start;
        self.expect(Token::Def)?;

        // Parse name
        let name_token = self.expect(Token::Identifier(String::new()))?;
        let name_str = if let Token::Identifier(n) = &name_token.token {
            n.clone()
        } else {
            unreachable!()
        };
        let name = self.symbols.intern(&name_str);

        // Parse optional parameters
        let params = if self.check(&Token::LParen) {
            self.parse_params()?
        } else {
            Vec::new()
        };

        // Parse optional return type
        let return_type = if self.check(&Token::Colon) {
            self.advance();
            // Check if next is a type identifier or newline
            if !self.check(&Token::Newline) && !self.check(&Token::Eq) {
                Some(Box::new(self.parse_element()?.unwrap_or_else(|| {
                    Shrubbery::Identifier(self.symbols.intern("Block"), ScopeSet::new(), Span::default())
                })))
            } else {
                None
            }
        } else {
            None
        };

        // Check for = (single-line definition)
        let body = if self.check(&Token::Eq) {
            self.advance();
            vec![self.parse_element()?.unwrap_or_else(|| {
                Shrubbery::Literal(Literal::None, Span::default())
            })]
        } else {
            // Expect : for block body (may have been consumed for return type)
            if !self.check(&Token::Newline) && !self.check(&Token::Colon) {
                return Err(MrlError::ParserError {
                    span: self.peek().map(|t| t.span).unwrap_or(Span::default()),
                    message: "Expected ':' or '=' after def signature".to_string(),
                });
            }
            if self.check(&Token::Colon) {
                self.advance();
            }
            self.expect(Token::Newline)?;
            self.parse_indented_block()?
        };

        let end = self.peek().map(|t| t.span.start).unwrap_or(start);
        let span = Span::new(start, end);

        Ok(Shrubbery::DefBlock {
            name,
            params,
            return_type,
            body,
            span,
        })
    }

    /// Parse parameter list: (param1: Type = default, ...)
    fn parse_params(&mut self) -> Result<Vec<Param>> {
        self.expect(Token::LParen)?;
        let mut params = Vec::new();

        while !self.check(&Token::RParen) && !self.is_eof() {
            // Skip newlines in param list
            while self.check(&Token::Newline) {
                self.advance();
            }

            if self.check(&Token::RParen) {
                break;
            }

            let name_token = self.expect(Token::Identifier(String::new()))?;
            let (name_str, name_span) = if let Token::Identifier(n) = &name_token.token {
                (n.clone(), name_token.span)
            } else {
                unreachable!()
            };
            let name = self.symbols.intern(&name_str);

            let mut param = Param::new(name, name_span);

            // Optional type annotation
            if self.check(&Token::Colon) {
                self.advance();
                if let Some(type_expr) = self.parse_element()? {
                    param = param.with_type(type_expr);
                }
            }

            // Optional default value
            if self.check(&Token::Eq) {
                self.advance();
                if let Some(default_expr) = self.parse_element()? {
                    param = param.with_default(default_expr);
                }
            }

            params.push(param);

            // Handle comma separation
            if self.check(&Token::Comma) {
                self.advance();
            } else if !self.check(&Token::RParen) {
                break;
            }
        }

        self.expect(Token::RParen)?;
        Ok(params)
    }

    /// Parse indented block (Python-style)
    /// Note: This is a simplified version - full implementation would track column positions
    fn parse_indented_block(&mut self) -> Result<Vec<Shrubbery>> {
        let mut body = Vec::new();

        // For now, just parse expressions until we hit a dedent or EOF
        // In a real implementation, we'd track indentation levels
        while !self.is_eof() {
            // Check if we're at a dedent by looking for a keyword at the same level
            if self.check(&Token::Bang) || self.check(&Token::Eof) {
                break;
            }

            // Use parse_expression to handle things like `x = 42`
            if let Some(elem) = self.parse_expression()? {
                body.push(elem);
            } else {
                break;
            }
        }

        Ok(body)
    }

    /// Parse !staged[...] or !staged: indented_body
    fn parse_staged(&mut self) -> Result<Shrubbery> {
        let start_token = self.expect(Token::Bang)?;
        let start = start_token.span.start;
        self.expect(Token::Staged)?;

        let body = if self.check(&Token::LBracket) {
            // Bracketed form: !staged[...]
            self.advance();
            let mut elements = Vec::new();
            while !self.check(&Token::RBracket) && !self.is_eof() {
                if let Some(elem) = self.parse_element()? {
                    elements.push(elem);
                }
            }
            self.expect(Token::RBracket)?;
            elements
        } else {
            // Indented form: !staged: ... (newline + indent)
            if self.check(&Token::Colon) {
                self.advance();
            }
            self.expect(Token::Newline)?;
            self.parse_indented_block()?
        };

        let end = self.peek().map(|t| t.span.start).unwrap_or(start);
        let span = Span::new(start, end);

        Ok(Shrubbery::StagedBlock { body, span })
    }

    /// Parse !show selector: transform
    fn parse_show(&mut self) -> Result<Shrubbery> {
        let start_token = self.expect(Token::Bang)?;
        let start = start_token.span.start;
        self.expect(Token::Show)?;

        // Parse selector
        let selector = Box::new(self.parse_selector()?);

        // Expect colon
        self.expect(Token::Colon)?;

        // Parse transform (can be on same line or indented block)
        let transform = if self.check(&Token::Newline) {
            self.advance();
            let body = self.parse_indented_block()?;
            Box::new(Shrubbery::Sequence(body, Span::default()))
        } else {
            Box::new(self.parse_element()?.unwrap_or_else(|| {
                Shrubbery::Literal(Literal::None, Span::default())
            }))
        };

        let end = self.peek().map(|t| t.span.start).unwrap_or(start);
        let span = Span::new(start, end);

        Ok(Shrubbery::ShowRule {
            selector,
            transform,
            span,
        })
    }

    /// Parse !set selector {...}
    fn parse_set(&mut self) -> Result<Shrubbery> {
        let start_token = self.expect(Token::Bang)?;
        let start = start_token.span.start;
        self.expect(Token::Set)?;

        // Parse selector
        let selector = Box::new(self.parse_selector()?);

        // Expect braces
        self.expect(Token::LBrace)?;

        // Parse properties: key: value, ...
        let mut properties = Vec::new();
        while !self.check(&Token::RBrace) && !self.is_eof() {
            // Skip newlines
            while self.check(&Token::Newline) {
                self.advance();
            }

            if self.check(&Token::RBrace) {
                break;
            }

            // Parse property name
            let key_token = self.expect(Token::Identifier(String::new()))?;
            let key_str = if let Token::Identifier(k) = &key_token.token {
                k.clone()
            } else {
                unreachable!()
            };
            let key = self.symbols.intern(&key_str);

            self.expect(Token::Colon)?;

            // Parse property value
            let value = self.parse_element()?.unwrap_or_else(|| {
                Shrubbery::Literal(Literal::None, Span::default())
            });

            properties.push((key, value));

            // Handle comma separation
            if self.check(&Token::Comma) {
                self.advance();
            }
        }

        self.expect(Token::RBrace)?;

        let end = self.peek().map(|t| t.span.start).unwrap_or(start);
        let span = Span::new(start, end);

        Ok(Shrubbery::SetRule {
            selector,
            properties,
            span,
        })
    }

    /// Parse !live[...] or !live: indented_body
    fn parse_live(&mut self) -> Result<Shrubbery> {
        let start_token = self.expect(Token::Bang)?;
        let start = start_token.span.start;
        self.expect(Token::Live)?;

        // Optional dependencies: !live(deps: [...])
        let deps = if self.check(&Token::LParen) {
            self.advance();
            // Look for deps: [...]
            if let Some(Token::Identifier(name)) = self.peek().map(|t| &t.token) {
                if name == "deps" {
                    self.advance();
                    self.expect(Token::Colon)?;
                    self.expect(Token::LBracket)?;
                    let mut dep_list = Vec::new();
                    while !self.check(&Token::RBracket) && !self.is_eof() {
                        let id_token = self.expect(Token::Identifier(String::new()))?;
                        let id_str = if let Token::Identifier(id) = &id_token.token {
                            id.clone()
                        } else {
                            continue;
                        };
                        dep_list.push(self.symbols.intern(&id_str));
                        if self.check(&Token::Comma) {
                            self.advance();
                        }
                    }
                    self.expect(Token::RBracket)?;
                    self.expect(Token::RParen)?;
                    Some(dep_list)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let body = if self.check(&Token::LBracket) {
            // Bracketed form
            self.advance();
            let mut elements = Vec::new();
            while !self.check(&Token::RBracket) && !self.is_eof() {
                if let Some(elem) = self.parse_element()? {
                    elements.push(elem);
                }
            }
            self.expect(Token::RBracket)?;
            elements
        } else {
            // Indented form
            if self.check(&Token::Colon) {
                self.advance();
            }
            self.expect(Token::Newline)?;
            self.parse_indented_block()?
        };

        let end = self.peek().map(|t| t.span.start).unwrap_or(start);
        let span = Span::new(start, end);

        Ok(Shrubbery::LiveBlock { deps, body, span })
    }

    /// Parse selector for show/set: heading, paragraph, link.where(...), etc.
    fn parse_selector(&mut self) -> Result<Shrubbery> {
        let start_token = self.expect(Token::Identifier(String::new()))?;
        let (base_name, start) = if let Token::Identifier(name) = &start_token.token {
            (name.clone(), start_token.span.start)
        } else {
            unreachable!()
        };
        let base = self.symbols.intern(&base_name);

        // Check for .where(predicate)
        let predicate = if self.check(&Token::Dot) {
            self.advance();
            // Check for Token::Where keyword or identifier "where"
            let is_where = self.check(&Token::Where)
                || matches!(self.peek().map(|t| &t.token), Some(Token::Identifier(name)) if name == "where");

            if is_where {
                self.advance(); // consume "where"
                self.expect(Token::LParen)?;
                // Parse predicate as an expression (to handle level == 1, etc.)
                let pred = self.parse_expression()?.unwrap_or_else(|| {
                    Shrubbery::Literal(Literal::Bool(true), Span::default())
                });
                self.expect(Token::RParen)?;
                Some(Box::new(pred))
            } else {
                None
            }
        } else {
            None
        };

        let end = self.peek().map(|t| t.span.start).unwrap_or(start);
        let span = Span::new(start, end);

        Ok(Shrubbery::Selector {
            base,
            predicate,
            span,
        })
    }

    /// Parse quote expression: quote[...] or quote: body
    fn parse_quote(&mut self) -> Result<Shrubbery> {
        let start_token = self.expect(Token::Quote)?;
        let start = start_token.span.start;

        let body = if self.check(&Token::LBracket) {
            self.advance();
            let mut elements = Vec::new();
            while !self.check(&Token::RBracket) && !self.is_eof() {
                if let Some(elem) = self.parse_element()? {
                    elements.push(elem);
                }
            }
            self.expect(Token::RBracket)?;
            Box::new(Shrubbery::Sequence(elements, Span::default()))
        } else if self.check(&Token::Colon) {
            self.advance();
            self.expect(Token::Newline)?;
            let body_elems = self.parse_indented_block()?;
            Box::new(Shrubbery::Sequence(body_elems, Span::default()))
        } else {
            // Single expression
            Box::new(self.parse_element()?.unwrap_or_else(|| {
                Shrubbery::Literal(Literal::None, Span::default())
            }))
        };

        let end = self.peek().map(|t| t.span.start).unwrap_or(start);
        let span = Span::new(start, end);

        Ok(Shrubbery::Quote { body, span })
    }

    /// Parse splice with keyword: splice(expr)
    fn parse_splice_keyword(&mut self) -> Result<Shrubbery> {
        let start_token = self.expect(Token::Splice)?;
        let start = start_token.span.start;

        self.expect(Token::LParen)?;
        let expr = Box::new(self.parse_element()?.unwrap_or_else(|| {
            Shrubbery::Literal(Literal::None, Span::default())
        }));
        self.expect(Token::RParen)?;

        let end = self.peek().map(|t| t.span.start).unwrap_or(start);
        let span = Span::new(start, end);

        Ok(Shrubbery::Splice { expr, span })
    }

    /// Parse splice with dollar: $identifier
    fn parse_splice_dollar(&mut self) -> Result<Shrubbery> {
        let start_token = self.expect(Token::Dollar)?;
        let start = start_token.span.start;

        let expr = Box::new(self.parse_identifier()?);

        let end = self.peek().map(|t| t.span.start).unwrap_or(start);
        let span = Span::new(start, end);

        Ok(Shrubbery::Splice { expr, span })
    }

    /// Parse if expression: if cond: then else: otherwise
    fn parse_if(&mut self) -> Result<Shrubbery> {
        let start_token = self.expect(Token::If)?;
        let start = start_token.span.start;

        // Parse condition as an expression (to handle x == 1, etc.)
        let condition = Box::new(self.parse_expression()?.unwrap_or_else(|| {
            Shrubbery::Literal(Literal::Bool(true), Span::default())
        }));

        self.expect(Token::Colon)?;

        let then_branch = if self.check(&Token::Newline) {
            self.advance();
            let body = self.parse_indented_block()?;
            Box::new(Shrubbery::Sequence(body, Span::default()))
        } else {
            // Use parse_expression to handle `text("one")` as a single expression
            Box::new(self.parse_expression()?.unwrap_or_else(|| {
                Shrubbery::Literal(Literal::None, Span::default())
            }))
        };

        let else_branch = if self.check(&Token::Else) {
            self.advance();
            self.expect(Token::Colon)?;
            if self.check(&Token::Newline) {
                self.advance();
                let body = self.parse_indented_block()?;
                Some(Box::new(Shrubbery::Sequence(body, Span::default())))
            } else {
                Some(Box::new(self.parse_expression()?.unwrap_or_else(|| {
                    Shrubbery::Literal(Literal::None, Span::default())
                })))
            }
        } else {
            None
        };

        let end = self.peek().map(|t| t.span.start).unwrap_or(start);
        let span = Span::new(start, end);

        Ok(Shrubbery::If {
            condition,
            then_branch,
            else_branch,
            span,
        })
    }

    /// Parse for expression: for pattern in iterable: body
    fn parse_for(&mut self) -> Result<Shrubbery> {
        let start_token = self.expect(Token::For)?;
        let start = start_token.span.start;

        // Parse pattern (for now just an identifier)
        let pattern_token = self.expect(Token::Identifier(String::new()))?;
        let pattern_str = if let Token::Identifier(p) = &pattern_token.token {
            p.clone()
        } else {
            unreachable!()
        };
        let pattern = self.symbols.intern(&pattern_str);

        self.expect(Token::In)?;

        let iterable = Box::new(self.parse_element()?.unwrap_or_else(|| {
            Shrubbery::Literal(Literal::None, Span::default())
        }));

        self.expect(Token::Colon)?;

        let body = if self.check(&Token::Newline) {
            self.advance();
            let body_elems = self.parse_indented_block()?;
            Box::new(Shrubbery::Sequence(body_elems, Span::default()))
        } else {
            // Use parse_expression to handle `paragraph(item)` as a single expression
            Box::new(self.parse_expression()?.unwrap_or_else(|| {
                Shrubbery::Literal(Literal::None, Span::default())
            }))
        };

        let end = self.peek().map(|t| t.span.start).unwrap_or(start);
        let span = Span::new(start, end);

        Ok(Shrubbery::For {
            pattern,
            iterable,
            body,
            span,
        })
    }
}

/// Parse tokens into shrubbery
pub fn parse(tokens: &[SpannedToken]) -> Result<Shrubbery> {
    let mut parser = Parser::new(tokens);
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    #[test]
    fn test_parse_identifier() {
        let tokens = tokenize("foo").unwrap();
        let shrub = parse(&tokens).unwrap();
        assert!(matches!(shrub, Shrubbery::Identifier(_, _, _)));
    }

    #[test]
    fn test_parse_literal() {
        let tokens = tokenize("42").unwrap();
        let shrub = parse(&tokens).unwrap();
        if let Shrubbery::Literal(Literal::Int(i), _) = shrub {
            assert_eq!(i, 42);
        } else {
            panic!("Expected int literal");
        }
    }

    #[test]
    fn test_parse_string_literal() {
        let tokens = tokenize(r#""hello""#).unwrap();
        let shrub = parse(&tokens).unwrap();
        if let Shrubbery::Literal(Literal::String(s), _) = shrub {
            assert_eq!(s, "hello");
        } else {
            panic!("Expected string literal");
        }
    }

    #[test]
    fn test_parse_parens() {
        let tokens = tokenize("(foo bar)").unwrap();
        let shrub = parse(&tokens).unwrap();
        if let Shrubbery::Parens(items, _) = shrub {
            assert_eq!(items.len(), 2);
        } else {
            panic!("Expected parens");
        }
    }

    #[test]
    fn test_parse_brackets() {
        let tokens = tokenize("[1, 2, 3]").unwrap();
        let shrub = parse(&tokens).unwrap();
        if let Shrubbery::Brackets(items, _) = shrub {
            assert_eq!(items.len(), 3);
        } else {
            panic!("Expected brackets");
        }
    }

    #[test]
    fn test_parse_complex() {
        let tokens = tokenize("foo(bar, [1, 2])").unwrap();
        let shrub = parse(&tokens).unwrap();
        assert!(matches!(shrub, Shrubbery::Sequence(_, _)));
    }
}
