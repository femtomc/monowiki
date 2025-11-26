use crate::error::{MrlError, Result, Span};
use logos::Logos;

/// Tokens for MRL language
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t]+")]
pub enum Token {
    // Keywords
    #[token("def")]
    Def,
    #[token("staged")]
    Staged,
    #[token("show")]
    Show,
    #[token("set")]
    Set,
    #[token("live")]
    Live,
    #[token("quote")]
    Quote,
    #[token("splice")]
    Splice,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("where")]
    Where,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("none")]
    None,

    // Identifiers and literals
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_-]*", |lex| lex.slice().to_string())]
    Identifier(String),

    #[regex(r"-?[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    IntLiteral(Option<i64>),

    #[regex(r"-?[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    FloatLiteral(Option<f64>),

    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        s[1..s.len()-1].to_string()
    })]
    StringLiteral(String),

    #[regex(r"'[a-zA-Z_][a-zA-Z0-9_-]*", |lex| {
        lex.slice()[1..].to_string()
    })]
    SymbolLiteral(String),

    // Operators
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token("<")]
    Lt,
    #[token("<=")]
    Le,
    #[token(">")]
    Gt,
    #[token(">=")]
    Ge,
    #[token("&&")]
    AndAnd,
    #[token("||")]
    OrOr,
    #[token("!")]
    Bang,
    #[token("++")]
    PlusPlus,

    // Delimiters
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,

    // Punctuation
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token("=")]
    Eq,
    #[token(".")]
    Dot,
    #[token("$")]
    Dollar,
    #[token("\n")]
    Newline,

    // End of file
    Eof,
}

impl Token {
    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            Token::Def
                | Token::Staged
                | Token::Show
                | Token::Set
                | Token::Live
                | Token::Quote
                | Token::Splice
                | Token::If
                | Token::Else
                | Token::For
                | Token::In
                | Token::Where
                | Token::True
                | Token::False
                | Token::None
        )
    }
}

/// A token with its source span
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

impl SpannedToken {
    pub fn new(token: Token, span: Span) -> Self {
        Self { token, span }
    }
}

/// Lexer for MRL source code
pub struct Lexer<'a> {
    source: &'a str,
    tokens: Vec<SpannedToken>,
    current: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            tokens: Vec::new(),
            current: 0,
        }
    }

    /// Tokenize the entire source
    pub fn tokenize(&mut self) -> Result<Vec<SpannedToken>> {
        let mut lex = Token::lexer(self.source);
        let mut tokens = Vec::new();

        while let Some(token_result) = lex.next() {
            let span = Span::new(lex.span().start, lex.span().end);

            match token_result {
                Ok(token) => {
                    tokens.push(SpannedToken::new(token, span));
                }
                Err(_) => {
                    return Err(MrlError::LexerError {
                        span,
                        message: format!("Invalid token: {}", &self.source[span.start..span.end]),
                    });
                }
            }
        }

        tokens.push(SpannedToken::new(Token::Eof, Span::new(self.source.len(), self.source.len())));
        self.tokens = tokens.clone();
        Ok(tokens)
    }

    /// Peek at the current token
    pub fn peek(&self) -> Option<&SpannedToken> {
        self.tokens.get(self.current)
    }

    /// Advance to the next token
    pub fn next(&mut self) -> Option<&SpannedToken> {
        let token = self.tokens.get(self.current);
        if token.is_some() {
            self.current += 1;
        }
        token
    }

    /// Check if we're at the end
    pub fn is_eof(&self) -> bool {
        matches!(self.peek().map(|t| &t.token), Some(Token::Eof) | None)
    }
}

/// Helper to tokenize a string slice
pub fn tokenize(source: &str) -> Result<Vec<SpannedToken>> {
    let mut lexer = Lexer::new(source);
    lexer.tokenize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keywords() {
        let source = "def staged show set live";
        let tokens = tokenize(source).unwrap();
        assert_eq!(tokens[0].token, Token::Def);
        assert_eq!(tokens[1].token, Token::Staged);
        assert_eq!(tokens[2].token, Token::Show);
        assert_eq!(tokens[3].token, Token::Set);
        assert_eq!(tokens[4].token, Token::Live);
    }

    #[test]
    fn test_identifiers() {
        let source = "foo bar_baz test-name";
        let tokens = tokenize(source).unwrap();
        assert!(matches!(&tokens[0].token, Token::Identifier(s) if s == "foo"));
        assert!(matches!(&tokens[1].token, Token::Identifier(s) if s == "bar_baz"));
        assert!(matches!(&tokens[2].token, Token::Identifier(s) if s == "test-name"));
    }

    #[test]
    fn test_literals() {
        let source = r#"42 3.14 "hello" 'symbol true false none"#;
        let tokens = tokenize(source).unwrap();
        assert!(matches!(&tokens[0].token, Token::IntLiteral(Some(42))));
        assert!(matches!(&tokens[1].token, Token::FloatLiteral(_)));
        assert!(matches!(&tokens[2].token, Token::StringLiteral(s) if s == "hello"));
        assert!(matches!(&tokens[3].token, Token::SymbolLiteral(s) if s == "symbol"));
        assert_eq!(tokens[4].token, Token::True);
        assert_eq!(tokens[5].token, Token::False);
        assert_eq!(tokens[6].token, Token::None);
    }

    #[test]
    fn test_operators() {
        let source = "+ - * / % == != < <= > >= && || ! ++";
        let tokens = tokenize(source).unwrap();
        assert_eq!(tokens[0].token, Token::Plus);
        assert_eq!(tokens[1].token, Token::Minus);
        assert_eq!(tokens[2].token, Token::Star);
        assert_eq!(tokens[3].token, Token::Slash);
        assert_eq!(tokens[4].token, Token::Percent);
        assert_eq!(tokens[5].token, Token::EqEq);
        assert_eq!(tokens[6].token, Token::NotEq);
        assert_eq!(tokens[7].token, Token::Lt);
        assert_eq!(tokens[8].token, Token::Le);
        assert_eq!(tokens[9].token, Token::Gt);
        assert_eq!(tokens[10].token, Token::Ge);
        assert_eq!(tokens[11].token, Token::AndAnd);
        assert_eq!(tokens[12].token, Token::OrOr);
        assert_eq!(tokens[13].token, Token::Bang);
        assert_eq!(tokens[14].token, Token::PlusPlus);
    }

    #[test]
    fn test_delimiters() {
        let source = "( ) [ ] { }";
        let tokens = tokenize(source).unwrap();
        assert_eq!(tokens[0].token, Token::LParen);
        assert_eq!(tokens[1].token, Token::RParen);
        assert_eq!(tokens[2].token, Token::LBracket);
        assert_eq!(tokens[3].token, Token::RBracket);
        assert_eq!(tokens[4].token, Token::LBrace);
        assert_eq!(tokens[5].token, Token::RBrace);
    }

    #[test]
    fn test_punctuation() {
        let source = ", : = . $";
        let tokens = tokenize(source).unwrap();
        assert_eq!(tokens[0].token, Token::Comma);
        assert_eq!(tokens[1].token, Token::Colon);
        assert_eq!(tokens[2].token, Token::Eq);
        assert_eq!(tokens[3].token, Token::Dot);
        assert_eq!(tokens[4].token, Token::Dollar);
    }
}
