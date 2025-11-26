use crate::content::Content;
use crate::error::{MrlError, Result, Span};
use crate::expander::ExpandValue;
use crate::shrubbery::Shrubbery;
use crate::types::{ContentKind, MrlType};
use std::collections::HashMap;

/// Type environment for type checking
#[derive(Debug, Clone)]
pub struct TypeEnv {
    bindings: HashMap<String, MrlType>,
}

impl TypeEnv {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    pub fn bind(&mut self, name: String, ty: MrlType) {
        self.bindings.insert(name, ty);
    }

    pub fn lookup(&self, name: &str) -> Option<&MrlType> {
        self.bindings.get(name)
    }

    pub fn child(&self) -> Self {
        Self {
            bindings: self.bindings.clone(),
        }
    }
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self::new()
    }
}

/// Type checker for staged fragments
pub struct TypeChecker {
    env: TypeEnv,
    /// Current stage level (0 = expand-time, 1+ = quoted)
    stage_level: usize,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut checker = Self {
            env: TypeEnv::new(),
            stage_level: 0,
        };
        checker.register_builtins();
        checker
    }

    fn register_builtins(&mut self) {
        // Content constructors
        self.env.bind(
            "text".to_string(),
            MrlType::Function {
                params: vec![MrlType::String],
                ret: Box::new(MrlType::Inline),
            },
        );
        self.env.bind(
            "paragraph".to_string(),
            MrlType::Function {
                params: vec![MrlType::Inline],
                ret: Box::new(MrlType::Block),
            },
        );
        self.env.bind(
            "heading".to_string(),
            MrlType::Function {
                params: vec![MrlType::Int, MrlType::Inline],
                ret: Box::new(MrlType::Block),
            },
        );
        self.env.bind(
            "emphasis".to_string(),
            MrlType::Function {
                params: vec![MrlType::Inline],
                ret: Box::new(MrlType::Inline),
            },
        );
        self.env.bind(
            "strong".to_string(),
            MrlType::Function {
                params: vec![MrlType::Inline],
                ret: Box::new(MrlType::Inline),
            },
        );

        // Staging operations
        self.env.bind(
            "quote".to_string(),
            MrlType::Function {
                params: vec![MrlType::Content],
                ret: Box::new(MrlType::Code(ContentKind::Content)),
            },
        );
        self.env.bind(
            "splice".to_string(),
            MrlType::Function {
                params: vec![MrlType::Code(ContentKind::Content)],
                ret: Box::new(MrlType::Content),
            },
        );
    }

    /// Check a shrubbery expression
    pub fn check(&mut self, shrub: &Shrubbery) -> Result<MrlType> {
        match shrub {
            Shrubbery::Literal(lit, _) => {
                use crate::shrubbery::Literal;
                Ok(match lit {
                    Literal::None => MrlType::None,
                    Literal::Bool(_) => MrlType::Bool,
                    Literal::Int(_) => MrlType::Int,
                    Literal::Float(_) => MrlType::Float,
                    Literal::String(_) => MrlType::String,
                    Literal::Symbol(_) => MrlType::Symbol,
                })
            }

            Shrubbery::Identifier(sym, _, span) => {
                let name = format!("id:{}", sym.id());
                self.env.lookup(&name).cloned().ok_or_else(|| MrlError::UnboundIdentifier {
                    span: *span,
                    name,
                })
            }

            Shrubbery::Sequence(items, span) => {
                if items.is_empty() {
                    return Ok(MrlType::None);
                }

                // Check if this is a function call
                if let Some(Shrubbery::Identifier(_, _, _)) = items.first() {
                    self.check_call(items, *span)
                } else {
                    // Check all items, return type of last
                    let mut result_ty = MrlType::None;
                    for item in items {
                        result_ty = self.check(item)?;
                    }
                    Ok(result_ty)
                }
            }

            Shrubbery::Brackets(items, _) => {
                // Content literal - check all items are content
                let mut content_kind = ContentKind::Content;
                for item in items {
                    let ty = self.check(item)?;
                    if let Some(kind) = ty.as_content_kind() {
                        // Update content kind to most specific
                        match (content_kind, kind) {
                            (ContentKind::Content, k) => content_kind = k,
                            (k, ContentKind::Content) => content_kind = k,
                            (ContentKind::Block, ContentKind::Inline) => {
                                // Cannot mix Block and Inline
                                return Err(MrlError::TypeError {
                                    span: item.span(),
                                    message: "Cannot mix Block and Inline in content literal"
                                        .to_string(),
                                });
                            }
                            (ContentKind::Inline, ContentKind::Block) => {
                                return Err(MrlError::TypeError {
                                    span: item.span(),
                                    message: "Cannot mix Inline and Block in content literal"
                                        .to_string(),
                                });
                            }
                            _ => {}
                        }
                    }
                }

                Ok(match content_kind {
                    ContentKind::Block => MrlType::Block,
                    ContentKind::Inline => MrlType::Inline,
                    ContentKind::Content => MrlType::Content,
                })
            }

            Shrubbery::Prose(_, _) => Ok(MrlType::Inline),

            _ => Ok(MrlType::Dyn), // Fallback for unimplemented forms
        }
    }

    /// Check a function call
    fn check_call(&mut self, items: &[Shrubbery], span: Span) -> Result<MrlType> {
        if items.is_empty() {
            return Err(MrlError::TypeError {
                span,
                message: "Empty call".to_string(),
            });
        }

        // Get function type
        let func_ty = self.check(&items[0])?;

        // Check arguments
        let arg_types: Result<Vec<_>> = items[1..].iter().map(|arg| self.check(arg)).collect();
        let arg_types = arg_types?;

        // Validate function call
        match func_ty {
            MrlType::Function { params, ret } => {
                if arg_types.len() != params.len() {
                    return Err(MrlError::ArityMismatch {
                        span,
                        expected: params.len(),
                        got: arg_types.len(),
                    });
                }

                // Check each argument type
                for (arg_ty, param_ty) in arg_types.iter().zip(params.iter()) {
                    if !arg_ty.is_subtype_of(param_ty) {
                        return Err(MrlError::TypeError {
                            span,
                            message: format!(
                                "Argument type mismatch: expected {}, got {}",
                                param_ty, arg_ty
                            ),
                        });
                    }
                }

                Ok(*ret)
            }
            _ => Err(MrlError::TypeError {
                span,
                message: format!("Cannot call non-function type: {}", func_ty),
            }),
        }
    }

    /// Check that inline content doesn't contain blocks
    pub fn check_content_nesting(&self, content: &Content, span: Span) -> Result<()> {
        match content {
            Content::Inline(inline) => self.check_inline_nesting(inline, span),
            Content::Block(_) => Ok(()),
            Content::Sequence(items) => {
                for item in items {
                    self.check_content_nesting(item, span)?;
                }
                Ok(())
            }
        }
    }

    fn check_inline_nesting(&self, inline: &crate::content::Inline, span: Span) -> Result<()> {
        use crate::content::Inline;
        match inline {
            Inline::Text(_) | Inline::Code(_) | Inline::Image { .. } | Inline::Reference(_)
            | Inline::Math(_) => Ok(()),
            Inline::Emphasis(body) | Inline::Strong(body) | Inline::Span { body, .. } => {
                self.check_inline_nesting(body, span)
            }
            Inline::Link { body, .. } => self.check_inline_nesting(body, span),
            Inline::Sequence(items) => {
                for item in items {
                    self.check_inline_nesting(item, span)?;
                }
                Ok(())
            }
        }
    }

    /// Check a value's type
    pub fn check_value(&self, value: &ExpandValue, expected: &MrlType, span: Span) -> Result<()> {
        let actual = value.get_type();
        if !actual.is_subtype_of(expected) {
            Err(MrlError::TypeError {
                span,
                message: format!("Type mismatch: expected {}, got {}", expected, actual),
            })
        } else {
            Ok(())
        }
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    #[test]
    fn test_check_literal() {
        let mut checker = TypeChecker::new();
        let tokens = tokenize("42").unwrap();
        let shrub = parse(&tokens).unwrap();
        let ty = checker.check(&shrub).unwrap();
        assert_eq!(ty, MrlType::Int);
    }

    #[test]
    fn test_check_string() {
        let mut checker = TypeChecker::new();
        let tokens = tokenize(r#""hello""#).unwrap();
        let shrub = parse(&tokens).unwrap();
        let ty = checker.check(&shrub).unwrap();
        assert_eq!(ty, MrlType::String);
    }

    #[test]
    fn test_check_prose() {
        let mut checker = TypeChecker::new();
        let shrub = Shrubbery::Prose("Hello world".to_string(), Span::new(0, 11));
        let ty = checker.check(&shrub).unwrap();
        assert_eq!(ty, MrlType::Inline);
    }
}
