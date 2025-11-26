use std::fmt;

/// Content kind for staged types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentKind {
    Block,
    Inline,
    Content,
}

impl ContentKind {
    /// Check if this kind is a subkind of another
    pub fn is_subkind_of(&self, other: &ContentKind) -> bool {
        match (self, other) {
            (ContentKind::Block, ContentKind::Content) => true,
            (ContentKind::Inline, ContentKind::Content) => true,
            (a, b) => a == b,
        }
    }
}

impl fmt::Display for ContentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentKind::Block => write!(f, "Block"),
            ContentKind::Inline => write!(f, "Inline"),
            ContentKind::Content => write!(f, "Content"),
        }
    }
}

/// MRL type system
#[derive(Debug, Clone, PartialEq)]
pub enum MrlType {
    // Primitives
    None,
    Bool,
    Int,
    Float,
    String,
    Symbol,

    // Content types
    Content,
    Block,
    Inline,

    // Composite types
    Array(Box<MrlType>),
    Map(Box<MrlType>, Box<MrlType>),
    Tuple(Vec<MrlType>),
    Record(Vec<(String, MrlType)>),

    // Function types
    Function {
        params: Vec<MrlType>,
        ret: Box<MrlType>,
    },

    // Staged types
    Code(ContentKind),
    Shrubbery,

    // Selector types (for show/set)
    Selector(ContentKind),

    // Reactive types (render-time)
    Signal(Box<MrlType>),
    Effect,

    // Dynamic typing escape hatch
    Dyn,

    // Type variable for inference
    Var(u64),
}

impl MrlType {
    /// Check if this type is a subtype of another
    pub fn is_subtype_of(&self, other: &MrlType) -> bool {
        match (self, other) {
            // Reflexivity
            (a, b) if a == b => true,

            // Content subtyping
            (MrlType::Block, MrlType::Content) => true,
            (MrlType::Inline, MrlType::Content) => true,

            // Array covariance
            (MrlType::Array(t1), MrlType::Array(t2)) => t1.is_subtype_of(t2),

            // Function contravariance in params, covariance in return
            // f1 <: f2 if f2.params <: f1.params (contravariant) and f1.ret <: f2.ret (covariant)
            (
                MrlType::Function {
                    params: p1,
                    ret: r1,
                },
                MrlType::Function {
                    params: p2,
                    ret: r2,
                },
            ) => {
                p1.len() == p2.len()
                    && p2.iter().zip(p1.iter()).all(|(a, b)| a.is_subtype_of(b))
                    && r1.is_subtype_of(r2)
            }

            // Code subtyping follows ContentKind subtyping
            (MrlType::Code(k1), MrlType::Code(k2)) => k1.is_subkind_of(k2),

            // Selector subtyping
            (MrlType::Selector(k1), MrlType::Selector(k2)) => k1 == k2,

            // Signal covariance
            (MrlType::Signal(t1), MrlType::Signal(t2)) => t1.is_subtype_of(t2),

            // Dyn is top type
            (_, MrlType::Dyn) => true,

            _ => false,
        }
    }

    /// Get the content kind if this is a content type
    pub fn as_content_kind(&self) -> Option<ContentKind> {
        match self {
            MrlType::Block => Some(ContentKind::Block),
            MrlType::Inline => Some(ContentKind::Inline),
            MrlType::Content => Some(ContentKind::Content),
            _ => None,
        }
    }

    /// Check if this is a content type
    pub fn is_content(&self) -> bool {
        matches!(self, MrlType::Block | MrlType::Inline | MrlType::Content)
    }

    /// Check if this type can contain another type (for nesting validation)
    pub fn can_contain(&self, other: &MrlType) -> bool {
        match (self, other) {
            // Inline cannot contain Block
            (MrlType::Inline, MrlType::Block) => false,
            (MrlType::Inline, MrlType::Content) => false, // Could be Block

            // Content can contain anything
            (MrlType::Content, _) => true,

            // Block can contain Inline and Content
            (MrlType::Block, MrlType::Inline) => true,
            (MrlType::Block, MrlType::Content) => true,

            // Inline can contain Inline
            (MrlType::Inline, MrlType::Inline) => true,

            _ => true,
        }
    }
}

impl fmt::Display for MrlType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MrlType::None => write!(f, "None"),
            MrlType::Bool => write!(f, "Bool"),
            MrlType::Int => write!(f, "Int"),
            MrlType::Float => write!(f, "Float"),
            MrlType::String => write!(f, "String"),
            MrlType::Symbol => write!(f, "Symbol"),
            MrlType::Content => write!(f, "Content"),
            MrlType::Block => write!(f, "Block"),
            MrlType::Inline => write!(f, "Inline"),
            MrlType::Array(t) => write!(f, "Array<{}>", t),
            MrlType::Map(k, v) => write!(f, "Map<{}, {}>", k, v),
            MrlType::Tuple(ts) => {
                write!(f, "(")?;
                for (i, t) in ts.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", t)?;
                }
                write!(f, ")")
            }
            MrlType::Record(fields) => {
                write!(f, "{{ ")?;
                for (i, (name, ty)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", name, ty)?;
                }
                write!(f, " }}")
            }
            MrlType::Function { params, ret } => {
                write!(f, "(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", p)?;
                }
                write!(f, ") -> {}", ret)
            }
            MrlType::Code(k) => write!(f, "Code<{}>", k),
            MrlType::Shrubbery => write!(f, "Shrubbery"),
            MrlType::Selector(k) => write!(f, "Selector<{}>", k),
            MrlType::Signal(t) => write!(f, "Signal<{}>", t),
            MrlType::Effect => write!(f, "Effect"),
            MrlType::Dyn => write!(f, "Dyn"),
            MrlType::Var(id) => write!(f, "?{}", id),
        }
    }
}

/// Type scheme for polymorphism
#[derive(Debug, Clone, PartialEq)]
pub struct TypeScheme {
    pub vars: Vec<u64>,
    pub ty: MrlType,
}

impl TypeScheme {
    pub fn mono(ty: MrlType) -> Self {
        Self {
            vars: Vec::new(),
            ty,
        }
    }

    pub fn poly(vars: Vec<u64>, ty: MrlType) -> Self {
        Self { vars, ty }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_kind_subtyping() {
        let block = ContentKind::Block;
        let inline = ContentKind::Inline;
        let content = ContentKind::Content;

        assert!(block.is_subkind_of(&content));
        assert!(inline.is_subkind_of(&content));
        assert!(!content.is_subkind_of(&block));
        assert!(!content.is_subkind_of(&inline));
        assert!(block.is_subkind_of(&block));
    }

    #[test]
    fn test_type_subtyping() {
        assert!(MrlType::Block.is_subtype_of(&MrlType::Content));
        assert!(MrlType::Inline.is_subtype_of(&MrlType::Content));
        assert!(!MrlType::Content.is_subtype_of(&MrlType::Block));

        let int_array = MrlType::Array(Box::new(MrlType::Int));
        let dyn_array = MrlType::Array(Box::new(MrlType::Dyn));
        assert!(int_array.is_subtype_of(&dyn_array));
    }

    #[test]
    fn test_content_nesting() {
        let inline = MrlType::Inline;
        let block = MrlType::Block;
        let content = MrlType::Content;

        // Inline cannot contain Block
        assert!(!inline.can_contain(&block));
        assert!(!inline.can_contain(&content));

        // Block can contain Inline
        assert!(block.can_contain(&inline));
        assert!(block.can_contain(&content));

        // Content can contain anything
        assert!(content.can_contain(&block));
        assert!(content.can_contain(&inline));
        assert!(content.can_contain(&content));
    }

    #[test]
    fn test_code_type_subtyping() {
        let code_block = MrlType::Code(ContentKind::Block);
        let code_content = MrlType::Code(ContentKind::Content);

        assert!(code_block.is_subtype_of(&code_content));
        assert!(!code_content.is_subtype_of(&code_block));
    }

    #[test]
    fn test_function_subtyping() {
        // Test basic function subtyping with same params
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
    fn test_display() {
        let ty = MrlType::Function {
            params: vec![MrlType::String, MrlType::Int],
            ret: Box::new(MrlType::Code(ContentKind::Block)),
        };
        assert_eq!(ty.to_string(), "(String, Int) -> Code<Block>");
    }
}
