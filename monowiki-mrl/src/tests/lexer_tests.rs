use crate::lexer::*;

#[test]
fn test_tokenize_keywords() {
    let tokens = tokenize("def staged show set live quote").unwrap();
    assert!(matches!(tokens[0].token, Token::Def));
    assert!(matches!(tokens[1].token, Token::Staged));
    assert!(matches!(tokens[2].token, Token::Show));
    assert!(matches!(tokens[3].token, Token::Set));
    assert!(matches!(tokens[4].token, Token::Live));
    assert!(matches!(tokens[5].token, Token::Quote));
}

#[test]
fn test_tokenize_identifiers() {
    let tokens = tokenize("foo bar_baz test-123").unwrap();
    assert!(matches!(&tokens[0].token, Token::Identifier(s) if s == "foo"));
    assert!(matches!(&tokens[1].token, Token::Identifier(s) if s == "bar_baz"));
    assert!(matches!(&tokens[2].token, Token::Identifier(s) if s == "test-123"));
}

#[test]
fn test_tokenize_integers() {
    let tokens = tokenize("0 42 -100").unwrap();
    assert!(matches!(tokens[0].token, Token::IntLiteral(Some(0))));
    assert!(matches!(tokens[1].token, Token::IntLiteral(Some(42))));
    assert!(matches!(tokens[2].token, Token::IntLiteral(Some(-100))));
}

#[test]
fn test_tokenize_floats() {
    let tokens = tokenize("3.14 -2.5 0.0").unwrap();
    assert!(matches!(tokens[0].token, Token::FloatLiteral(Some(_))));
    assert!(matches!(tokens[1].token, Token::FloatLiteral(Some(_))));
    assert!(matches!(tokens[2].token, Token::FloatLiteral(Some(_))));
}

#[test]
fn test_tokenize_strings() {
    let tokens = tokenize(r#""hello" "world with spaces""#).unwrap();
    assert!(matches!(&tokens[0].token, Token::StringLiteral(s) if s == "hello"));
    assert!(matches!(&tokens[1].token, Token::StringLiteral(s) if s == "world with spaces"));
}

#[test]
fn test_tokenize_symbols() {
    let tokens = tokenize("'foo 'bar-baz").unwrap();
    assert!(matches!(&tokens[0].token, Token::SymbolLiteral(s) if s == "foo"));
    assert!(matches!(&tokens[1].token, Token::SymbolLiteral(s) if s == "bar-baz"));
}

#[test]
fn test_tokenize_operators() {
    let tokens = tokenize("+ - * / == != < <= > >=").unwrap();
    assert!(matches!(tokens[0].token, Token::Plus));
    assert!(matches!(tokens[1].token, Token::Minus));
    assert!(matches!(tokens[2].token, Token::Star));
    assert!(matches!(tokens[3].token, Token::Slash));
    assert!(matches!(tokens[4].token, Token::EqEq));
    assert!(matches!(tokens[5].token, Token::NotEq));
    assert!(matches!(tokens[6].token, Token::Lt));
    assert!(matches!(tokens[7].token, Token::Le));
    assert!(matches!(tokens[8].token, Token::Gt));
    assert!(matches!(tokens[9].token, Token::Ge));
}

#[test]
fn test_tokenize_delimiters() {
    let tokens = tokenize("( ) [ ] { }").unwrap();
    assert!(matches!(tokens[0].token, Token::LParen));
    assert!(matches!(tokens[1].token, Token::RParen));
    assert!(matches!(tokens[2].token, Token::LBracket));
    assert!(matches!(tokens[3].token, Token::RBracket));
    assert!(matches!(tokens[4].token, Token::LBrace));
    assert!(matches!(tokens[5].token, Token::RBrace));
}

#[test]
fn test_tokenize_complex_expression() {
    let tokens = tokenize(r#"foo(bar, "test", 42)"#).unwrap();
    assert!(matches!(&tokens[0].token, Token::Identifier(_)));
    assert!(matches!(tokens[1].token, Token::LParen));
    assert!(matches!(&tokens[2].token, Token::Identifier(_)));
    assert!(matches!(tokens[3].token, Token::Comma));
    assert!(matches!(&tokens[4].token, Token::StringLiteral(_)));
    assert!(matches!(tokens[5].token, Token::Comma));
    assert!(matches!(tokens[6].token, Token::IntLiteral(_)));
    assert!(matches!(tokens[7].token, Token::RParen));
}

#[test]
fn test_token_spans() {
    let source = "foo bar";
    let tokens = tokenize(source).unwrap();

    // First token should be "foo"
    assert_eq!(tokens[0].span.start, 0);
    assert_eq!(tokens[0].span.end, 3);
    assert_eq!(&source[tokens[0].span.start..tokens[0].span.end], "foo");

    // Second token should be "bar"
    assert_eq!(tokens[1].span.start, 4);
    assert_eq!(tokens[1].span.end, 7);
    assert_eq!(&source[tokens[1].span.start..tokens[1].span.end], "bar");
}

#[test]
fn test_boolean_literals() {
    let tokens = tokenize("true false").unwrap();
    assert!(matches!(tokens[0].token, Token::True));
    assert!(matches!(tokens[1].token, Token::False));
}

#[test]
fn test_none_literal() {
    let tokens = tokenize("none").unwrap();
    assert!(matches!(tokens[0].token, Token::None));
}
