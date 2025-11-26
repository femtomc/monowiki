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

// ===== Block construct tests =====

#[test]
fn test_parse_def_simple() {
    let source = r#"!def greet(name: String):
  text("Hello")
"#;
    let tokens = tokenize(source).unwrap();
    let shrub = parse(&tokens).unwrap();

    match &shrub {
        Shrubbery::DefBlock { params, body, .. } => {
            assert_eq!(params.len(), 1);
            assert!(!body.is_empty());
        }
        _ => panic!("Expected DefBlock, got {:?}", shrub),
    }
}

#[test]
fn test_parse_def_no_params() {
    let source = r#"!def today:
  text("2024")
"#;
    let tokens = tokenize(source).unwrap();
    let shrub = parse(&tokens).unwrap();

    match &shrub {
        Shrubbery::DefBlock { params, body, .. } => {
            assert_eq!(params.len(), 0);
            assert!(!body.is_empty());
        }
        _ => panic!("Expected DefBlock, got {:?}", shrub),
    }
}

#[test]
fn test_parse_def_single_line() {
    let source = r#"!def today = "2024-01-01""#;
    let tokens = tokenize(source).unwrap();
    let shrub = parse(&tokens).unwrap();

    match &shrub {
        Shrubbery::DefBlock { body, .. } => {
            assert_eq!(body.len(), 1);
        }
        _ => panic!("Expected DefBlock, got {:?}", shrub),
    }
}

#[test]
fn test_parse_staged_bracketed() {
    let source = r#"!staged[for x in items: paragraph(x)]"#;
    let tokens = tokenize(source).unwrap();
    let shrub = parse(&tokens).unwrap();

    match &shrub {
        Shrubbery::StagedBlock { body, .. } => {
            assert!(!body.is_empty());
        }
        _ => panic!("Expected StagedBlock, got {:?}", shrub),
    }
}

#[test]
fn test_parse_show_rule_simple() {
    let source = r#"!show heading: it"#;
    let tokens = tokenize(source).unwrap();
    let shrub = parse(&tokens).unwrap();

    match &shrub {
        Shrubbery::ShowRule { selector, .. } => {
            assert!(matches!(**selector, Shrubbery::Selector { .. }));
        }
        _ => panic!("Expected ShowRule, got {:?}", shrub),
    }
}

#[test]
fn test_parse_show_rule_with_where() {
    let source = r#"!show heading.where(level == 1): it"#;
    let tokens = tokenize(source).unwrap();
    let shrub = parse(&tokens).unwrap();

    match &shrub {
        Shrubbery::ShowRule { selector, .. } => {
            match &**selector {
                Shrubbery::Selector { predicate, .. } => {
                    assert!(predicate.is_some());
                }
                _ => panic!("Expected Selector"),
            }
        }
        _ => panic!("Expected ShowRule, got {:?}", shrub),
    }
}

#[test]
fn test_parse_set_rule() {
    let source = r#"!set heading {
  numbering: "1.",
  font: "Georgia"
}"#;
    let tokens = tokenize(source).unwrap();
    let shrub = parse(&tokens).unwrap();

    match &shrub {
        Shrubbery::SetRule { properties, .. } => {
            assert_eq!(properties.len(), 2);
        }
        _ => panic!("Expected SetRule, got {:?}", shrub),
    }
}

#[test]
fn test_parse_live_simple() {
    let source = r#"!live:
  x = 42
"#;
    let tokens = tokenize(source).unwrap();
    let shrub = parse(&tokens).unwrap();

    match &shrub {
        Shrubbery::LiveBlock { deps, body, .. } => {
            assert!(deps.is_none());
            assert!(!body.is_empty());
        }
        _ => panic!("Expected LiveBlock, got {:?}", shrub),
    }
}

#[test]
fn test_parse_quote_bracketed() {
    let source = r#"quote[text("Hello")]"#;
    let tokens = tokenize(source).unwrap();
    let shrub = parse(&tokens).unwrap();

    match &shrub {
        Shrubbery::Quote { .. } => {}
        _ => panic!("Expected Quote, got {:?}", shrub),
    }
}

#[test]
fn test_parse_splice_dollar() {
    let source = r#"$x"#;
    let tokens = tokenize(source).unwrap();
    let shrub = parse(&tokens).unwrap();

    match &shrub {
        Shrubbery::Splice { expr, .. } => {
            assert!(matches!(**expr, Shrubbery::Identifier(_, _, _)));
        }
        _ => panic!("Expected Splice, got {:?}", shrub),
    }
}

#[test]
fn test_parse_if_simple() {
    let source = r#"if x == 1: text("one")"#;
    let tokens = tokenize(source).unwrap();
    let shrub = parse(&tokens).unwrap();

    match &shrub {
        Shrubbery::If { else_branch, .. } => {
            assert!(else_branch.is_none());
        }
        _ => panic!("Expected If, got {:?}", shrub),
    }
}

#[test]
fn test_parse_for_loop() {
    let source = r#"for item in items: paragraph(item)"#;
    let tokens = tokenize(source).unwrap();
    let shrub = parse(&tokens).unwrap();

    assert!(matches!(shrub, Shrubbery::For { .. }));
}
