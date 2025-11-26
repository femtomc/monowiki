use crate::*;

#[test]
fn test_end_to_end_literal() {
    let source = "42";
    let result = execute(source);
    assert!(result.is_ok());
}

#[test]
fn test_end_to_end_string() {
    let source = r#""hello world""#;
    let result = execute(source);
    assert!(result.is_ok());
}

#[test]
fn test_parse_and_typecheck() {
    let source = "42";
    let ty = typecheck(source).unwrap();
    assert_eq!(ty, MrlType::Int);
}

#[test]
fn test_content_creation() {
    use content::{Content, Inline};

    let text = Content::text("Hello");
    assert!(text.is_inline());

    let para = Content::paragraph(Inline::text("World"));
    assert!(para.is_block());
}

#[test]
fn test_content_combination() {
    use content::{Content, Inline};

    let i1 = Inline::text("Hello ");
    let i2 = Inline::text("world");
    let combined = i1.concat(i2);

    assert_eq!(combined, Inline::text("Hello world"));
}

#[test]
fn test_type_subtyping() {
    assert!(MrlType::Block.is_subtype_of(&MrlType::Content));
    assert!(MrlType::Inline.is_subtype_of(&MrlType::Content));
    assert!(!MrlType::Content.is_subtype_of(&MrlType::Block));
}

#[test]
fn test_content_nesting_validation() {
    let inline = MrlType::Inline;
    let block = MrlType::Block;

    // Inline cannot contain Block
    assert!(!inline.can_contain(&block));

    // Block can contain Inline
    assert!(block.can_contain(&inline));
}

#[test]
fn test_scope_operations() {
    use shrubbery::{Scope, ScopeSet};

    let scope1 = Scope::new(1);
    let scope2 = Scope::new(2);

    let mut set = ScopeSet::new();
    set.add(scope1);
    assert!(set.contains(&scope1));

    set.flip(scope1);
    assert!(!set.contains(&scope1));

    set.flip(scope1);
    assert!(set.contains(&scope1));

    set.add(scope2);
    assert_eq!(set.len(), 2);
}

#[test]
fn test_hygiene_binding_resolution() {
    use hygiene::{Binding, HygieneEnv};
    use shrubbery::{Scope, ScopeSet, Symbol};
    use error::Span;

    let mut env = HygieneEnv::new();

    let sym = Symbol::new(0);
    let scopes: ScopeSet = vec![1, 2].into_iter().map(Scope::new).collect();
    let binding = Binding::new(sym, scopes.clone(), Span::new(0, 3));

    env.add_binding(binding);

    let use_scopes: ScopeSet = vec![1, 2, 3].into_iter().map(Scope::new).collect();
    assert!(env.is_bound(sym, &use_scopes));
}

#[test]
fn test_error_context_display() {
    use error::{ErrorContext, MrlError, Span};

    let source = "foo bar baz";
    let error = MrlError::ParserError {
        span: Span::new(4, 7),
        message: "test error".to_string(),
    };

    let ctx = ErrorContext::new(source, &error);
    let (line, col) = ctx.line_col();
    assert_eq!(line, 1);
    assert_eq!(col, 5);
}
