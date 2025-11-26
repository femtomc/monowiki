use crate::types::*;

#[test]
fn test_content_kind_subtyping() {
    assert!(ContentKind::Block.is_subkind_of(&ContentKind::Content));
    assert!(ContentKind::Inline.is_subkind_of(&ContentKind::Content));
    assert!(ContentKind::Block.is_subkind_of(&ContentKind::Block));
    assert!(!ContentKind::Content.is_subkind_of(&ContentKind::Block));
}

#[test]
fn test_primitive_subtyping() {
    assert!(MrlType::Int.is_subtype_of(&MrlType::Int));
    assert!(!MrlType::Int.is_subtype_of(&MrlType::String));
}

#[test]
fn test_content_subtyping() {
    assert!(MrlType::Block.is_subtype_of(&MrlType::Content));
    assert!(MrlType::Inline.is_subtype_of(&MrlType::Content));
    assert!(!MrlType::Content.is_subtype_of(&MrlType::Block));
    assert!(!MrlType::Content.is_subtype_of(&MrlType::Inline));
}

#[test]
fn test_array_covariance() {
    let int_array = MrlType::Array(Box::new(MrlType::Int));
    let dyn_array = MrlType::Array(Box::new(MrlType::Dyn));

    assert!(int_array.is_subtype_of(&dyn_array));
    assert!(!dyn_array.is_subtype_of(&int_array));
}

#[test]
fn test_function_subtyping() {
    // Test basic function subtyping - contravariance in params doesn't apply to MRL
    // since we use structural subtyping, not behavioral subtyping
    let f1 = MrlType::Function {
        params: vec![MrlType::Int],
        ret: Box::new(MrlType::Block),
    };
    let f2 = MrlType::Function {
        params: vec![MrlType::Int],
        ret: Box::new(MrlType::Content),
    };

    assert!(f1.is_subtype_of(&f2));
}

#[test]
fn test_code_type_subtyping() {
    let code_block = MrlType::Code(ContentKind::Block);
    let code_content = MrlType::Code(ContentKind::Content);

    assert!(code_block.is_subtype_of(&code_content));
    assert!(!code_content.is_subtype_of(&code_block));
}

#[test]
fn test_signal_covariance() {
    let int_signal = MrlType::Signal(Box::new(MrlType::Int));
    let dyn_signal = MrlType::Signal(Box::new(MrlType::Dyn));

    assert!(int_signal.is_subtype_of(&dyn_signal));
}

#[test]
fn test_dyn_is_top() {
    assert!(MrlType::Int.is_subtype_of(&MrlType::Dyn));
    assert!(MrlType::String.is_subtype_of(&MrlType::Dyn));
    assert!(MrlType::Block.is_subtype_of(&MrlType::Dyn));
    assert!(MrlType::Content.is_subtype_of(&MrlType::Dyn));
}

#[test]
fn test_content_nesting_inline_cannot_contain_block() {
    assert!(!MrlType::Inline.can_contain(&MrlType::Block));
    assert!(!MrlType::Inline.can_contain(&MrlType::Content));
}

#[test]
fn test_content_nesting_block_can_contain_inline() {
    assert!(MrlType::Block.can_contain(&MrlType::Inline));
    assert!(MrlType::Block.can_contain(&MrlType::Content));
}

#[test]
fn test_content_nesting_content_can_contain_all() {
    assert!(MrlType::Content.can_contain(&MrlType::Block));
    assert!(MrlType::Content.can_contain(&MrlType::Inline));
    assert!(MrlType::Content.can_contain(&MrlType::Content));
}

#[test]
fn test_content_kind_detection() {
    assert_eq!(MrlType::Block.as_content_kind(), Some(ContentKind::Block));
    assert_eq!(MrlType::Inline.as_content_kind(), Some(ContentKind::Inline));
    assert_eq!(MrlType::Content.as_content_kind(), Some(ContentKind::Content));
    assert_eq!(MrlType::Int.as_content_kind(), None);
}

#[test]
fn test_is_content() {
    assert!(MrlType::Block.is_content());
    assert!(MrlType::Inline.is_content());
    assert!(MrlType::Content.is_content());
    assert!(!MrlType::Int.is_content());
    assert!(!MrlType::String.is_content());
}

#[test]
fn test_type_display() {
    assert_eq!(MrlType::Int.to_string(), "Int");
    assert_eq!(MrlType::String.to_string(), "String");
    assert_eq!(
        MrlType::Array(Box::new(MrlType::Int)).to_string(),
        "Array<Int>"
    );
    assert_eq!(
        MrlType::Code(ContentKind::Block).to_string(),
        "Code<Block>"
    );

    let func = MrlType::Function {
        params: vec![MrlType::String, MrlType::Int],
        ret: Box::new(MrlType::Content),
    };
    assert_eq!(func.to_string(), "(String, Int) -> Content");
}

#[test]
fn test_type_scheme() {
    let scheme = TypeScheme::mono(MrlType::Int);
    assert_eq!(scheme.vars.len(), 0);
    assert_eq!(scheme.ty, MrlType::Int);

    let poly_scheme = TypeScheme::poly(vec![0, 1], MrlType::Var(0));
    assert_eq!(poly_scheme.vars.len(), 2);
}
