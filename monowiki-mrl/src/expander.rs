use crate::content::Content;
use crate::error::{MrlError, Result, Span};
use crate::hygiene::{HygieneEnv, MacroContext};
use crate::shrubbery::{Scope, Shrubbery};
use crate::types::{ContentKind, MrlType};
use std::collections::HashMap;

/// A value at expand-time
#[derive(Debug, Clone)]
pub enum ExpandValue {
    /// Primitive values
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Symbol(String),

    /// Content values
    Content(Content),

    /// Quoted code
    Code(Box<Shrubbery>, ContentKind),

    /// Raw shrubbery (for macros)
    Shrubbery(Box<Shrubbery>),

    /// Arrays
    Array(Vec<ExpandValue>),

    /// Maps
    Map(HashMap<String, ExpandValue>),

    /// Functions (native or user-defined)
    Function(ExpandFunction),
}

impl ExpandValue {
    /// Get the type of this value
    pub fn get_type(&self) -> MrlType {
        match self {
            ExpandValue::None => MrlType::None,
            ExpandValue::Bool(_) => MrlType::Bool,
            ExpandValue::Int(_) => MrlType::Int,
            ExpandValue::Float(_) => MrlType::Float,
            ExpandValue::String(_) => MrlType::String,
            ExpandValue::Symbol(_) => MrlType::Symbol,
            ExpandValue::Content(c) => {
                if c.is_block() {
                    MrlType::Block
                } else if c.is_inline() {
                    MrlType::Inline
                } else {
                    MrlType::Content
                }
            }
            ExpandValue::Code(_, kind) => MrlType::Code(*kind),
            ExpandValue::Shrubbery(_) => MrlType::Shrubbery,
            ExpandValue::Array(items) => {
                if let Some(first) = items.first() {
                    MrlType::Array(Box::new(first.get_type()))
                } else {
                    MrlType::Array(Box::new(MrlType::Dyn))
                }
            }
            ExpandValue::Map(_) => MrlType::Map(Box::new(MrlType::String), Box::new(MrlType::Dyn)),
            ExpandValue::Function(_) => MrlType::Dyn, // Functions need full type info
        }
    }

    /// Try to extract content from this value
    pub fn as_content(&self) -> Option<&Content> {
        match self {
            ExpandValue::Content(c) => Some(c),
            _ => None,
        }
    }

    /// Try to extract a string from this value
    pub fn as_string(&self) -> Option<&str> {
        match self {
            ExpandValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Try to extract an integer from this value
    pub fn as_int(&self) -> Option<i64> {
        match self {
            ExpandValue::Int(i) => Some(*i),
            _ => None,
        }
    }
}

/// A function at expand-time
#[derive(Debug, Clone)]
pub enum ExpandFunction {
    /// Native function (built-in)
    Native {
        name: String,
        arity: usize,
        handler: fn(&[ExpandValue]) -> Result<ExpandValue>,
    },

    /// User-defined macro
    Macro {
        params: Vec<String>,
        body: Box<Shrubbery>,
        env: HygieneEnv,
    },
}

/// Expander state
pub struct Expander {
    /// Variable bindings
    env: HashMap<String, ExpandValue>,

    /// Hygiene environment
    hygiene: HygieneEnv,

    /// Next scope ID
    next_scope_id: u64,

    /// Macro definitions
    macros: HashMap<String, ExpandFunction>,
}

impl Expander {
    pub fn new() -> Self {
        let mut expander = Self {
            env: HashMap::new(),
            hygiene: HygieneEnv::new(),
            next_scope_id: 0,
            macros: HashMap::new(),
        };

        // Register built-in functions
        expander.register_builtins();

        expander
    }

    /// Create a fresh scope
    fn fresh_scope(&mut self) -> Scope {
        let scope = Scope::new(self.next_scope_id);
        self.next_scope_id += 1;
        scope
    }

    /// Register built-in functions
    fn register_builtins(&mut self) {
        // Content constructors
        self.register_native("text", 1, builtin_text);
        self.register_native("paragraph", 1, builtin_paragraph);
        self.register_native("heading", 2, builtin_heading);
        self.register_native("emphasis", 1, builtin_emphasis);
        self.register_native("strong", 1, builtin_strong);

        // Staging operations
        self.register_native("quote", 1, builtin_quote);
        self.register_native("splice", 1, builtin_splice);
        self.register_native("eval_expand", 1, builtin_eval_expand);
    }

    /// Register a native function
    fn register_native(
        &mut self,
        name: &str,
        arity: usize,
        handler: fn(&[ExpandValue]) -> Result<ExpandValue>,
    ) {
        self.env.insert(
            name.to_string(),
            ExpandValue::Function(ExpandFunction::Native {
                name: name.to_string(),
                arity,
                handler,
            }),
        );
    }

    /// Define a macro
    pub fn define_macro(&mut self, name: String, params: Vec<String>, body: Shrubbery) {
        self.macros.insert(
            name.clone(),
            ExpandFunction::Macro {
                params,
                body: Box::new(body),
                env: self.hygiene.child(),
            },
        );
    }

    /// Expand a shrubbery to a value
    pub fn expand(&mut self, shrub: &Shrubbery) -> Result<ExpandValue> {
        match shrub {
            Shrubbery::Literal(lit, _) => {
                use crate::shrubbery::Literal;
                match lit {
                    Literal::None => Ok(ExpandValue::None),
                    Literal::Bool(b) => Ok(ExpandValue::Bool(*b)),
                    Literal::Int(i) => Ok(ExpandValue::Int(*i)),
                    Literal::Float(f) => Ok(ExpandValue::Float(*f)),
                    Literal::String(s) => Ok(ExpandValue::String(s.clone())),
                    Literal::Symbol(s) => Ok(ExpandValue::Symbol(s.clone())),
                }
            }

            Shrubbery::Identifier(sym, scopes, span) => {
                // Look up in environment
                // For now, simplified lookup without full hygiene
                let name = format!("id:{}", sym.id());
                self.env.get(&name).cloned().ok_or_else(|| MrlError::UnboundIdentifier {
                    span: *span,
                    name: name.clone(),
                })
            }

            Shrubbery::Sequence(items, span) => {
                if items.is_empty() {
                    return Ok(ExpandValue::None);
                }

                // Check if this is a function call
                if let Some(Shrubbery::Identifier(_, _, _)) = items.first() {
                    self.expand_call(items, *span)
                } else {
                    // Expand all items and return the last one
                    let mut result = ExpandValue::None;
                    for item in items {
                        result = self.expand(item)?;
                    }
                    Ok(result)
                }
            }

            Shrubbery::Brackets(items, span) => {
                // Content literal - collect prose and inline content
                let mut content_items = Vec::new();
                for item in items {
                    let value = self.expand(item)?;
                    if let Some(content) = value.as_content() {
                        content_items.push(content.clone());
                    }
                }

                if content_items.len() == 1 {
                    Ok(ExpandValue::Content(content_items.into_iter().next().unwrap()))
                } else {
                    Ok(ExpandValue::Content(Content::Sequence(content_items)))
                }
            }

            Shrubbery::Prose(text, _) => {
                use crate::content::Inline;
                Ok(ExpandValue::Content(Content::Inline(Inline::Text(
                    text.clone(),
                ))))
            }

            _ => {
                // Other forms not yet implemented
                Ok(ExpandValue::Shrubbery(Box::new(shrub.clone())))
            }
        }
    }

    /// Expand a function call
    fn expand_call(&mut self, items: &[Shrubbery], span: Span) -> Result<ExpandValue> {
        if items.is_empty() {
            return Err(MrlError::ExpansionError {
                span,
                message: "Empty call".to_string(),
            });
        }

        // Get function name
        let func_name = if let Shrubbery::Identifier(sym, _, _) = &items[0] {
            format!("id:{}", sym.id())
        } else {
            return Err(MrlError::ExpansionError {
                span,
                message: "Call target must be identifier".to_string(),
            });
        };

        // Look up function
        let func = self.env.get(&func_name).cloned().ok_or_else(|| MrlError::UnboundIdentifier {
            span,
            name: func_name.clone(),
        })?;

        // Expand arguments
        let args: Result<Vec<_>> = items[1..].iter().map(|arg| self.expand(arg)).collect();
        let args = args?;

        // Call function
        match func {
            ExpandValue::Function(ExpandFunction::Native { handler, arity, .. }) => {
                if args.len() != arity {
                    return Err(MrlError::ArityMismatch {
                        span,
                        expected: arity,
                        got: args.len(),
                    });
                }
                handler(&args)
            }
            ExpandValue::Function(ExpandFunction::Macro { .. }) => {
                // Macro expansion requires special handling
                self.expand_macro(&func_name, &args, span)
            }
            _ => Err(MrlError::ExpansionError {
                span,
                message: format!("{} is not a function", func_name),
            }),
        }
    }

    /// Expand a macro invocation
    fn expand_macro(&mut self, name: &str, args: &[ExpandValue], span: Span) -> Result<ExpandValue> {
        let macro_def = self.macros.get(name).cloned().ok_or_else(|| MrlError::ExpansionError {
            span,
            message: format!("Undefined macro: {}", name),
        })?;

        match macro_def {
            ExpandFunction::Macro { params, body, env: _ } => {
                // Bind parameters
                let mut new_env = self.env.clone();
                for (param, arg) in params.iter().zip(args.iter()) {
                    new_env.insert(param.clone(), arg.clone());
                }

                // Expand body in new environment
                let saved_env = std::mem::replace(&mut self.env, new_env);
                let result = self.expand(&body);
                self.env = saved_env;

                result
            }
            _ => unreachable!(),
        }
    }
}

impl Default for Expander {
    fn default() -> Self {
        Self::new()
    }
}

// Built-in functions

fn builtin_text(args: &[ExpandValue]) -> Result<ExpandValue> {
    let s = args[0].as_string().ok_or_else(|| MrlError::ExpansionError {
        span: Span::default(),
        message: "text() requires string argument".to_string(),
    })?;

    use crate::content::Inline;
    Ok(ExpandValue::Content(Content::Inline(Inline::Text(
        s.to_string(),
    ))))
}

fn builtin_paragraph(args: &[ExpandValue]) -> Result<ExpandValue> {
    let content = args[0].as_content().ok_or_else(|| MrlError::ExpansionError {
        span: Span::default(),
        message: "paragraph() requires content argument".to_string(),
    })?;

    use crate::content::{Attributes, Block, Inline};
    let inline = match content {
        Content::Inline(i) => i.clone(),
        _ => {
            return Err(MrlError::ExpansionError {
                span: Span::default(),
                message: "paragraph() requires inline content".to_string(),
            })
        }
    };

    Ok(ExpandValue::Content(Content::Block(Block::Paragraph {
        body: Box::new(inline),
        attrs: Attributes::new(),
    })))
}

fn builtin_heading(args: &[ExpandValue]) -> Result<ExpandValue> {
    let level = args[0].as_int().ok_or_else(|| MrlError::ExpansionError {
        span: Span::default(),
        message: "heading() requires integer level".to_string(),
    })?;

    let content = args[1].as_content().ok_or_else(|| MrlError::ExpansionError {
        span: Span::default(),
        message: "heading() requires content argument".to_string(),
    })?;

    use crate::content::{Attributes, Block, Inline};
    let inline = match content {
        Content::Inline(i) => i.clone(),
        _ => {
            return Err(MrlError::ExpansionError {
                span: Span::default(),
                message: "heading() requires inline content".to_string(),
            })
        }
    };

    Ok(ExpandValue::Content(Content::Block(Block::Heading {
        level: level as u8,
        body: Box::new(inline),
        attrs: Attributes::new(),
    })))
}

fn builtin_emphasis(args: &[ExpandValue]) -> Result<ExpandValue> {
    let content = args[0].as_content().ok_or_else(|| MrlError::ExpansionError {
        span: Span::default(),
        message: "emphasis() requires content argument".to_string(),
    })?;

    use crate::content::Inline;
    let inline = match content {
        Content::Inline(i) => i.clone(),
        _ => {
            return Err(MrlError::ExpansionError {
                span: Span::default(),
                message: "emphasis() requires inline content".to_string(),
            })
        }
    };

    Ok(ExpandValue::Content(Content::Inline(Inline::Emphasis(
        Box::new(inline),
    ))))
}

fn builtin_strong(args: &[ExpandValue]) -> Result<ExpandValue> {
    let content = args[0].as_content().ok_or_else(|| MrlError::ExpansionError {
        span: Span::default(),
        message: "strong() requires content argument".to_string(),
    })?;

    use crate::content::Inline;
    let inline = match content {
        Content::Inline(i) => i.clone(),
        _ => {
            return Err(MrlError::ExpansionError {
                span: Span::default(),
                message: "strong() requires inline content".to_string(),
            })
        }
    };

    Ok(ExpandValue::Content(Content::Inline(Inline::Strong(
        Box::new(inline),
    ))))
}

fn builtin_quote(args: &[ExpandValue]) -> Result<ExpandValue> {
    match &args[0] {
        ExpandValue::Content(c) => {
            let kind = if c.is_block() {
                ContentKind::Block
            } else if c.is_inline() {
                ContentKind::Inline
            } else {
                ContentKind::Content
            };
            // Convert content back to shrubbery (simplified)
            Ok(ExpandValue::Code(
                Box::new(Shrubbery::Prose(format!("{}", c), Span::default())),
                kind,
            ))
        }
        ExpandValue::Shrubbery(s) => Ok(ExpandValue::Code(s.clone(), ContentKind::Content)),
        _ => Err(MrlError::ExpansionError {
            span: Span::default(),
            message: "quote() requires content or shrubbery".to_string(),
        }),
    }
}

fn builtin_splice(args: &[ExpandValue]) -> Result<ExpandValue> {
    match &args[0] {
        ExpandValue::Code(shrub, _) => {
            // Evaluate the quoted code
            // This is simplified - would need full expander context
            Ok(ExpandValue::Shrubbery(shrub.clone()))
        }
        _ => Err(MrlError::ExpansionError {
            span: Span::default(),
            message: "splice() requires quoted code".to_string(),
        }),
    }
}

fn builtin_eval_expand(args: &[ExpandValue]) -> Result<ExpandValue> {
    match &args[0] {
        ExpandValue::Code(_, _) => {
            // Evaluate at expand-time
            // This is simplified - would recursively call expander
            Ok(args[0].clone())
        }
        _ => Err(MrlError::ExpansionError {
            span: Span::default(),
            message: "eval_expand() requires quoted code".to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    #[test]
    fn test_expand_literal() {
        let mut expander = Expander::new();
        let tokens = tokenize("42").unwrap();
        let shrub = parse(&tokens).unwrap();
        let result = expander.expand(&shrub).unwrap();
        assert!(matches!(result, ExpandValue::Int(42)));
    }

    #[test]
    fn test_expand_string() {
        let mut expander = Expander::new();
        let tokens = tokenize(r#""hello""#).unwrap();
        let shrub = parse(&tokens).unwrap();
        let result = expander.expand(&shrub).unwrap();
        assert!(matches!(result, ExpandValue::String(_)));
    }
}
