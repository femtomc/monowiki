use crate::lexer::tokenize;
use crate::parser::parse;
use crate::shrubbery::{Literal, Shrubbery};

#[test]
fn test_parse_int_literal() {
    let tokens = tokenize("42").unwrap();
    let shrub = parse(&tokens).unwrap();
    match shrub {
        Shrubbery::Literal(Literal::Int(i), _) => assert_eq!(i, 42),
        _ => panic!("Expected int literal"),
    }
}

#[test]
fn test_parse_float_literal() {
    let tokens = tokenize("3.14").unwrap();
    let shrub = parse(&tokens).unwrap();
    match shrub {
        Shrubbery::Literal(Literal::Float(f), _) => assert!((f - 3.14).abs() < 0.001),
        _ => panic!("Expected float literal"),
    }
}

#[test]
fn test_parse_string_literal() {
    let tokens = tokenize(r#""hello world""#).unwrap();
    let shrub = parse(&tokens).unwrap();
    match shrub {
        Shrubbery::Literal(Literal::String(s), _) => assert_eq!(s, "hello world"),
        _ => panic!("Expected string literal"),
    }
}

#[test]
fn test_parse_symbol_literal() {
    let tokens = tokenize("'foo").unwrap();
    let shrub = parse(&tokens).unwrap();
    match shrub {
        Shrubbery::Literal(Literal::Symbol(s), _) => assert_eq!(s, "foo"),
        _ => panic!("Expected symbol literal"),
    }
}

#[test]
fn test_parse_bool_true() {
    let tokens = tokenize("true").unwrap();
    let shrub = parse(&tokens).unwrap();
    match shrub {
        Shrubbery::Literal(Literal::Bool(b), _) => assert!(b),
        _ => panic!("Expected bool literal"),
    }
}

#[test]
fn test_parse_bool_false() {
    let tokens = tokenize("false").unwrap();
    let shrub = parse(&tokens).unwrap();
    match shrub {
        Shrubbery::Literal(Literal::Bool(b), _) => assert!(!b),
        _ => panic!("Expected bool literal"),
    }
}

#[test]
fn test_parse_none() {
    let tokens = tokenize("none").unwrap();
    let shrub = parse(&tokens).unwrap();
    match shrub {
        Shrubbery::Literal(Literal::None, _) => {}
        _ => panic!("Expected none literal"),
    }
}

#[test]
fn test_parse_identifier() {
    let tokens = tokenize("foo").unwrap();
    let shrub = parse(&tokens).unwrap();
    assert!(matches!(shrub, Shrubbery::Identifier(_, _, _)));
}

#[test]
fn test_parse_parens() {
    let tokens = tokenize("(foo bar)").unwrap();
    let shrub = parse(&tokens).unwrap();
    match shrub {
        Shrubbery::Parens(items, _) => {
            assert_eq!(items.len(), 2);
            assert!(matches!(items[0], Shrubbery::Identifier(_, _, _)));
            assert!(matches!(items[1], Shrubbery::Identifier(_, _, _)));
        }
        _ => panic!("Expected parens"),
    }
}

#[test]
fn test_parse_brackets() {
    let tokens = tokenize("[1, 2, 3]").unwrap();
    let shrub = parse(&tokens).unwrap();
    match shrub {
        Shrubbery::Brackets(items, _) => {
            assert_eq!(items.len(), 3);
            for item in items {
                assert!(matches!(item, Shrubbery::Literal(Literal::Int(_), _)));
            }
        }
        _ => panic!("Expected brackets"),
    }
}

#[test]
fn test_parse_braces() {
    let tokens = tokenize("{a: 1, b: 2}").unwrap();
    let shrub = parse(&tokens).unwrap();
    match shrub {
        Shrubbery::Braces(items, _) => {
            assert!(items.len() > 0);
        }
        _ => panic!("Expected braces"),
    }
}

#[test]
fn test_parse_nested_parens() {
    let tokens = tokenize("(a (b c))").unwrap();
    let shrub = parse(&tokens).unwrap();
    match shrub {
        Shrubbery::Parens(items, _) => {
            assert_eq!(items.len(), 2);
            assert!(matches!(items[0], Shrubbery::Identifier(_, _, _)));
            assert!(matches!(items[1], Shrubbery::Parens(_, _)));
        }
        _ => panic!("Expected parens"),
    }
}

#[test]
fn test_parse_mixed_delimiters() {
    let tokens = tokenize("foo([1, 2], {a: b})").unwrap();
    let shrub = parse(&tokens).unwrap();
    // Should parse as a sequence
    assert!(matches!(shrub, Shrubbery::Sequence(_, _)));
}

#[test]
fn test_parse_empty_parens() {
    let tokens = tokenize("()").unwrap();
    let shrub = parse(&tokens).unwrap();
    match shrub {
        Shrubbery::Parens(items, _) => assert_eq!(items.len(), 0),
        _ => panic!("Expected empty parens"),
    }
}

#[test]
fn test_parse_empty_brackets() {
    let tokens = tokenize("[]").unwrap();
    let shrub = parse(&tokens).unwrap();
    match shrub {
        Shrubbery::Brackets(items, _) => assert_eq!(items.len(), 0),
        _ => panic!("Expected empty brackets"),
    }
}

#[test]
fn test_parse_sequence() {
    let tokens = tokenize("a b c").unwrap();
    let shrub = parse(&tokens).unwrap();
    match shrub {
        Shrubbery::Sequence(items, _) => {
            assert_eq!(items.len(), 3);
        }
        _ => panic!("Expected sequence"),
    }
}
