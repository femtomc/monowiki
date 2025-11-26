use crate::error::{MrlError, Result, Span};
use crate::lexer::{SpannedToken, Token};
use crate::shrubbery::{Literal, Scope, ScopeSet, Shrubbery, Symbol};
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
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [SpannedToken]) -> Self {
        Self {
            tokens,
            pos: 0,
            symbols: SymbolTable::new(),
            next_scope_id: 0,
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
            Token::Identifier(_) => Some(self.parse_identifier()?),
            Token::IntLiteral(_) | Token::FloatLiteral(_) | Token::StringLiteral(_)
            | Token::SymbolLiteral(_) | Token::True | Token::False | Token::None => {
                Some(self.parse_literal()?)
            }
            Token::LParen => Some(self.parse_parens()?),
            Token::LBracket => Some(self.parse_brackets()?),
            Token::LBrace => Some(self.parse_braces()?),
            Token::Bang => Some(self.parse_operator()?),
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

    /// Parse an identifier
    fn parse_identifier(&mut self) -> Result<Shrubbery> {
        let token = self.expect(Token::Identifier(String::new()))?;
        if let Token::Identifier(name) = &token.token {
            let name_clone = name.clone();
            let span = token.span;
            let sym = self.symbols.intern(&name_clone);
            Ok(Shrubbery::Identifier(sym, ScopeSet::new(), span))
        } else {
            unreachable!()
        }
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
            _ => unreachable!(),
        };
        let sym = self.symbols.intern(op_name);
        Ok(Shrubbery::Operator(sym, span))
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
