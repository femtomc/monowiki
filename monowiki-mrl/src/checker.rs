use crate::content::Content;
use crate::enforest::{BinOp, Expr, UnOp};
use crate::error::{MrlError, Result, Span};
use crate::expander::ExpandValue;
use crate::shrubbery::{Literal, Shrubbery, Symbol};
use crate::types::{ContentKind, MrlType};
use std::collections::HashMap;

/// A binding with its type and stage level
#[derive(Debug, Clone)]
pub struct TypeBinding {
    pub ty: MrlType,
    /// Stage level at which this binding was introduced
    /// 0 = expand-time, 1+ = within quote
    pub stage: usize,
}

/// Type environment for type checking
#[derive(Debug, Clone)]
pub struct TypeEnv {
    bindings: HashMap<String, TypeBinding>,
}

impl TypeEnv {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    /// Bind a name with type and stage level
    pub fn bind_at_stage(&mut self, name: String, ty: MrlType, stage: usize) {
        self.bindings.insert(name, TypeBinding { ty, stage });
    }

    /// Bind a name at stage 0 (convenience method)
    pub fn bind(&mut self, name: String, ty: MrlType) {
        self.bind_at_stage(name, ty, 0);
    }

    /// Look up a binding's type (stage-unaware)
    pub fn lookup(&self, name: &str) -> Option<&MrlType> {
        self.bindings.get(name).map(|b| &b.ty)
    }

    /// Look up a binding with stage information
    pub fn lookup_binding(&self, name: &str) -> Option<&TypeBinding> {
        self.bindings.get(name)
    }

    /// Look up and check stage constraints (CSP)
    ///
    /// A binding at stage n can be used at stage m iff n <= m.
    /// This is Cross-Stage Persistence (CSP).
    pub fn lookup_at_stage(&self, name: &str, current_stage: usize) -> Option<&MrlType> {
        self.bindings.get(name).and_then(|binding| {
            if binding.stage <= current_stage {
                Some(&binding.ty)
            } else {
                None // Stage violation
            }
        })
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
    /// Reverse mapping from Symbol to name for Var lookup fallback
    symbol_names: HashMap<Symbol, String>,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut checker = Self {
            env: TypeEnv::new(),
            stage_level: 0,
            symbol_names: HashMap::new(),
        };
        checker.register_builtins();
        checker
    }

    /// Register a symbol's name for reverse lookup during Var type checking
    pub fn register_symbol(&mut self, sym: Symbol, name: &str) {
        self.symbol_names.insert(sym, name.to_string());
    }

    /// Register multiple symbols from a symbol table
    pub fn register_symbols(&mut self, symbols: &HashMap<String, Symbol>) {
        for (name, sym) in symbols {
            self.symbol_names.insert(*sym, name.clone());
        }
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
        self.env.bind(
            "code".to_string(),
            MrlType::Function {
                params: vec![MrlType::String],
                ret: Box::new(MrlType::Inline),
            },
        );

        // Block constructors
        self.env.bind(
            "codeblock".to_string(),
            MrlType::Function {
                params: vec![MrlType::String, MrlType::String],
                ret: Box::new(MrlType::Block),
            },
        );
        self.env.bind(
            "blockquote".to_string(),
            MrlType::Function {
                params: vec![MrlType::Content],
                ret: Box::new(MrlType::Block),
            },
        );
        self.env.bind(
            "directive".to_string(),
            MrlType::Function {
                params: vec![MrlType::String, MrlType::Content],
                ret: Box::new(MrlType::Block),
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
                // First try to resolve symbol ID to actual name
                if let Some(name) = self.symbol_names.get(sym).cloned() {
                    // Look up by actual name (handles builtins like "text", "emphasis", etc.)
                    if let Some(ty) = self.env.lookup(&name) {
                        return Ok(ty.clone());
                    }
                    // Name found but not in env
                    return Err(MrlError::UnboundIdentifier {
                        span: *span,
                        name,
                    });
                }
                // Fallback: try "id:N" format
                let id_name = format!("id:{}", sym.id());
                self.env.lookup(&id_name).cloned().ok_or_else(|| MrlError::UnboundIdentifier {
                    span: *span,
                    name: id_name,
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
            Content::Live(_) => {
                // Live cells are not checked for nesting - they're evaluated at render-time
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

    // =========================================================================
    // Expr type checking (from enforest output)
    // =========================================================================

    /// Check an enforested expression
    pub fn check_expr(&mut self, expr: &Expr) -> Result<MrlType> {
        match expr {
            Expr::Literal(lit, _span) => Ok(self.type_of_literal(lit)),

            Expr::Var(sym, _scopes, span) => {
                // Look up in environment by symbol id first
                let key = format!("id:{}", sym.id());
                let name = self.symbol_names.get(sym).cloned().unwrap_or_else(|| key.clone());

                // Try id-based lookup with stage check
                if let Some(binding) = self.env.lookup_binding(&key) {
                    // CSP check: binding stage must be <= current stage
                    if binding.stage <= self.stage_level {
                        return Ok(binding.ty.clone());
                    } else {
                        return Err(MrlError::StageLevelError {
                            span: *span,
                            message: format!(
                                "Cannot use '{}' at stage {} (bound at stage {}). \
                                 CSP violation: bindings from higher stages cannot be used at lower stages.",
                                name, self.stage_level, binding.stage
                            ),
                        });
                    }
                }

                // Fall back to name-based lookup (for implicit bindings like `it`)
                if let Some(binding) = self.env.lookup_binding(&name) {
                    if binding.stage <= self.stage_level {
                        return Ok(binding.ty.clone());
                    } else {
                        return Err(MrlError::StageLevelError {
                            span: *span,
                            message: format!(
                                "Cannot use '{}' at stage {} (bound at stage {}). \
                                 CSP violation: bindings from higher stages cannot be used at lower stages.",
                                name, self.stage_level, binding.stage
                            ),
                        });
                    }
                }

                Err(MrlError::UnboundIdentifier {
                    span: *span,
                    name,
                })
            }

            Expr::BinOp(left, op, right, span) => {
                let left_ty = self.check_expr(left)?;
                let right_ty = self.check_expr(right)?;
                self.check_binop(&left_ty, op, &right_ty, *span)
            }

            Expr::UnOp(op, operand, span) => {
                let operand_ty = self.check_expr(operand)?;
                self.check_unop(op, &operand_ty, *span)
            }

            Expr::Call(func, args, span) => {
                let func_ty = self.check_expr(func)?;
                let arg_tys: Result<Vec<_>> = args.iter().map(|a| self.check_expr(a)).collect();
                let arg_tys = arg_tys?;
                self.check_call_expr(&func_ty, &arg_tys, *span)
            }

            Expr::FieldAccess(base, field, span) => {
                let base_ty = self.check_expr(base)?;
                self.check_field_access(&base_ty, *field, *span)
            }

            Expr::Subscript(base, index, span) => {
                let base_ty = self.check_expr(base)?;
                let index_ty = self.check_expr(index)?;
                self.check_subscript(&base_ty, &index_ty, *span)
            }

            Expr::If(cond, then_branch, else_branch, span) => {
                let cond_ty = self.check_expr(cond)?;
                if cond_ty != MrlType::Bool {
                    return Err(MrlError::TypeError {
                        span: cond.span(),
                        message: format!("If condition must be Bool, got {}", cond_ty),
                    });
                }

                let then_ty = self.check_expr(then_branch)?;

                if let Some(else_br) = else_branch {
                    let else_ty = self.check_expr(else_br)?;
                    // Unify then and else types
                    self.unify_types(&then_ty, &else_ty, *span)
                } else {
                    Ok(then_ty)
                }
            }

            Expr::For(var, iterable, body, _span) => {
                let iter_ty = self.check_expr(iterable)?;

                // Get element type from iterable
                let elem_ty = match iter_ty {
                    MrlType::Array(elem) => *elem,
                    _ => MrlType::Dyn, // Allow iterating over Dyn
                };

                // Bind loop variable at current stage
                let mut child_env = self.env.child();
                child_env.bind_at_stage(
                    format!("id:{}", var.id()),
                    elem_ty,
                    self.stage_level,
                );

                // Check body in extended environment
                let old_env = std::mem::replace(&mut self.env, child_env);
                let body_ty = self.check_expr(body)?;
                self.env = old_env;

                // For loops produce arrays of body results
                Ok(MrlType::Array(Box::new(body_ty)))
            }

            Expr::Quote(inner, _span) => {
                // Quote increases stage level - bindings inside are at higher stage
                self.stage_level += 1;
                let inner_ty = self.check_expr(inner)?;
                self.stage_level -= 1;

                // Quote produces Code<K> where K is the content kind
                let kind = inner_ty.as_content_kind().unwrap_or(ContentKind::Content);
                Ok(MrlType::Code(kind))
            }

            Expr::Splice(inner, span) => {
                // Splice decreases stage level - must have stage > 0
                if self.stage_level == 0 {
                    return Err(MrlError::StageLevelError {
                        span: *span,
                        message: "Cannot splice at stage 0 (expand-time). \
                                  Splices can only appear inside quotes.".to_string(),
                    });
                }

                self.stage_level -= 1;
                let inner_ty = self.check_expr(inner)?;
                self.stage_level += 1;

                // Splice unwraps Code<K> to K
                match inner_ty {
                    MrlType::Code(kind) => Ok(match kind {
                        ContentKind::Block => MrlType::Block,
                        ContentKind::Inline => MrlType::Inline,
                        ContentKind::Content => MrlType::Content,
                    }),
                    _ => Err(MrlError::TypeError {
                        span: *span,
                        message: format!("Splice requires Code<K>, got {}", inner_ty),
                    }),
                }
            }

            Expr::Content(content, _span) => {
                // Determine the specific content type
                match content {
                    crate::content::Content::Block(_) => Ok(MrlType::Block),
                    crate::content::Content::Inline(_) => Ok(MrlType::Inline),
                    crate::content::Content::Sequence(_) => Ok(MrlType::Content),
                    crate::content::Content::Live(_) => {
                        // Live cells produce Content at render-time
                        Ok(MrlType::Content)
                    }
                }
            }

            Expr::Block(exprs, span) => {
                if exprs.is_empty() {
                    return Ok(MrlType::None);
                }
                // Type of block is type of last expression
                let mut result_ty = MrlType::None;
                for e in exprs {
                    result_ty = self.check_expr(e)?;
                }
                Ok(result_ty)
            }

            Expr::Sequence(_, _span) => {
                // Unevaluated shrubbery sequence - type is Shrubbery
                Ok(MrlType::Shrubbery)
            }

            // Definition forms
            Expr::Def { name, params, return_type, body, span } => {
                self.check_def(*name, params, return_type.as_deref(), body, *span)
            }

            Expr::Staged(body, span) => {
                self.check_staged(body, *span)
            }

            Expr::ShowRule { selector, transform, span } => {
                self.check_show_rule(selector, transform, *span)
            }

            Expr::SetRule { selector, properties, span } => {
                self.check_set_rule(selector, properties, *span)
            }

            Expr::Live { deps, body, span } => {
                self.check_live(deps.as_ref(), body, *span)
            }
        }
    }

    /// Type of a literal
    fn type_of_literal(&self, lit: &Literal) -> MrlType {
        match lit {
            Literal::None => MrlType::None,
            Literal::Bool(_) => MrlType::Bool,
            Literal::Int(_) => MrlType::Int,
            Literal::Float(_) => MrlType::Float,
            Literal::String(_) => MrlType::String,
            Literal::Symbol(_) => MrlType::Symbol,
        }
    }

    /// Check binary operation
    fn check_binop(&self, left: &MrlType, op: &BinOp, right: &MrlType, span: Span) -> Result<MrlType> {
        match op {
            // Arithmetic: Int/Float -> Int/Float
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod | BinOp::Pow => {
                match (left, right) {
                    (MrlType::Int, MrlType::Int) => Ok(MrlType::Int),
                    (MrlType::Float, MrlType::Float) => Ok(MrlType::Float),
                    (MrlType::Int, MrlType::Float) | (MrlType::Float, MrlType::Int) => Ok(MrlType::Float),
                    _ => Err(MrlError::TypeError {
                        span,
                        message: format!("Cannot apply {:?} to {} and {}", op, left, right),
                    }),
                }
            }

            // Comparison: any comparable -> Bool
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                Ok(MrlType::Bool)
            }

            // Logical: Bool -> Bool
            BinOp::And | BinOp::Or => {
                if *left != MrlType::Bool || *right != MrlType::Bool {
                    return Err(MrlError::TypeError {
                        span,
                        message: format!("Logical operators require Bool, got {} and {}", left, right),
                    });
                }
                Ok(MrlType::Bool)
            }

            // Concatenation: String/Array/Content
            BinOp::Concat => {
                match (left, right) {
                    (MrlType::String, MrlType::String) => Ok(MrlType::String),
                    (MrlType::Array(t1), MrlType::Array(t2)) if t1 == t2 => {
                        Ok(MrlType::Array(t1.clone()))
                    }
                    (l, r) if l.is_content() && r.is_content() => {
                        // Content concatenation - find common supertype
                        if l == r {
                            Ok(l.clone())
                        } else {
                            Ok(MrlType::Content)
                        }
                    }
                    _ => Err(MrlError::TypeError {
                        span,
                        message: format!("Cannot concatenate {} and {}", left, right),
                    }),
                }
            }

            // Assignment
            BinOp::Assign => {
                // For now, just return the right-hand type
                Ok(right.clone())
            }
        }
    }

    /// Check unary operation
    fn check_unop(&self, op: &UnOp, operand: &MrlType, span: Span) -> Result<MrlType> {
        match op {
            UnOp::Neg => {
                match operand {
                    MrlType::Int => Ok(MrlType::Int),
                    MrlType::Float => Ok(MrlType::Float),
                    _ => Err(MrlError::TypeError {
                        span,
                        message: format!("Cannot negate {}", operand),
                    }),
                }
            }
            UnOp::Not => {
                if *operand != MrlType::Bool {
                    return Err(MrlError::TypeError {
                        span,
                        message: format!("Cannot apply 'not' to {}", operand),
                    });
                }
                Ok(MrlType::Bool)
            }
        }
    }

    /// Check function call
    fn check_call_expr(&self, func_ty: &MrlType, arg_tys: &[MrlType], span: Span) -> Result<MrlType> {
        match func_ty {
            MrlType::Function { params, ret } => {
                if arg_tys.len() != params.len() {
                    return Err(MrlError::ArityMismatch {
                        span,
                        expected: params.len(),
                        got: arg_tys.len(),
                    });
                }

                for (arg_ty, param_ty) in arg_tys.iter().zip(params.iter()) {
                    if !arg_ty.is_subtype_of(param_ty) {
                        return Err(MrlError::TypeError {
                            span,
                            message: format!("Argument type mismatch: expected {}, got {}", param_ty, arg_ty),
                        });
                    }
                }

                Ok(*ret.clone())
            }
            MrlType::Dyn => Ok(MrlType::Dyn),
            _ => Err(MrlError::TypeError {
                span,
                message: format!("Cannot call non-function type: {}", func_ty),
            }),
        }
    }

    /// Check subscript operation
    fn check_subscript(&self, base: &MrlType, index: &MrlType, span: Span) -> Result<MrlType> {
        match base {
            MrlType::Array(elem) => {
                if *index != MrlType::Int {
                    return Err(MrlError::TypeError {
                        span,
                        message: format!("Array index must be Int, got {}", index),
                    });
                }
                Ok(*elem.clone())
            }
            MrlType::Map(key, value) => {
                if !index.is_subtype_of(key) {
                    return Err(MrlError::TypeError {
                        span,
                        message: format!("Map key type mismatch: expected {}, got {}", key, index),
                    });
                }
                Ok(*value.clone())
            }
            MrlType::String => {
                if *index != MrlType::Int {
                    return Err(MrlError::TypeError {
                        span,
                        message: format!("String index must be Int, got {}", index),
                    });
                }
                Ok(MrlType::String)
            }
            MrlType::Dyn => Ok(MrlType::Dyn),
            _ => Err(MrlError::TypeError {
                span,
                message: format!("Cannot subscript type: {}", base),
            }),
        }
    }

    /// Check field access operation
    fn check_field_access(&self, base: &MrlType, field: Symbol, span: Span) -> Result<MrlType> {
        let field_name = self.symbol_names.get(&field)
            .cloned()
            .unwrap_or_else(|| format!("field_{}", field.id()));

        match base {
            // Record types - look up field directly
            MrlType::Record(fields) => {
                for (name, ty) in fields {
                    if name == &field_name {
                        return Ok(ty.clone());
                    }
                }
                Err(MrlError::TypeError {
                    span,
                    message: format!("Record has no field '{}'", field_name),
                })
            }

            // Content/Block types have known fields
            MrlType::Block | MrlType::Content => {
                match field_name.as_str() {
                    "attrs" => Ok(MrlType::Record(vec![
                        ("id".to_string(), MrlType::String),
                        ("classes".to_string(), MrlType::Array(Box::new(MrlType::String))),
                    ])),
                    _ => Ok(MrlType::Dyn) // Allow dynamic access for other fields
                }
            }

            // Inline types
            MrlType::Inline => {
                match field_name.as_str() {
                    "attrs" => Ok(MrlType::Record(vec![
                        ("id".to_string(), MrlType::String),
                        ("classes".to_string(), MrlType::Array(Box::new(MrlType::String))),
                    ])),
                    _ => Ok(MrlType::Dyn)
                }
            }

            // Dyn allows any field access
            MrlType::Dyn => Ok(MrlType::Dyn),

            // Other types don't support field access
            _ => Err(MrlError::TypeError {
                span,
                message: format!("Type {} does not support field access", base),
            }),
        }
    }

    /// Interpret a type expression from shrubbery
    /// Converts parsed type annotations to MrlType
    fn interpret_type(&self, shrub: &crate::shrubbery::Shrubbery) -> MrlType {
        use crate::shrubbery::Shrubbery;

        match shrub {
            Shrubbery::Identifier(sym, _, _) => {
                let name = self.symbol_names.get(sym)
                    .cloned()
                    .unwrap_or_else(|| format!("type_{}", sym.id()));

                // Match known type names
                match name.as_str() {
                    "None" | "none" => MrlType::None,
                    "Bool" | "bool" => MrlType::Bool,
                    "Int" | "int" => MrlType::Int,
                    "Float" | "float" => MrlType::Float,
                    "String" | "string" | "str" => MrlType::String,
                    "Symbol" | "symbol" => MrlType::Symbol,
                    "Content" | "content" => MrlType::Content,
                    "Block" | "block" => MrlType::Block,
                    "Inline" | "inline" => MrlType::Inline,
                    "Dyn" | "dyn" | "Any" | "any" => MrlType::Dyn,
                    _ => MrlType::Dyn // Unknown type defaults to Dyn
                }
            }

            // Array<T> or [T]
            Shrubbery::Brackets(items, _) if items.len() == 1 => {
                let elem = self.interpret_type(&items[0]);
                MrlType::Array(Box::new(elem))
            }

            // Generic type application: Array<Int>, Code<Block>, etc.
            Shrubbery::Sequence(items, _) if items.len() >= 2 => {
                if let Some((sym, _)) = items[0].as_identifier() {
                    let name = self.symbol_names.get(&sym)
                        .cloned()
                        .unwrap_or_default();

                    match name.as_str() {
                        "Array" | "List" => {
                            if items.len() > 1 {
                                let elem = self.interpret_type(&items[1]);
                                return MrlType::Array(Box::new(elem));
                            }
                        }
                        "Code" => {
                            if items.len() > 1 {
                                let kind = self.interpret_content_kind(&items[1]);
                                return MrlType::Code(kind);
                            }
                        }
                        "Signal" => {
                            if items.len() > 1 {
                                let inner = self.interpret_type(&items[1]);
                                return MrlType::Signal(Box::new(inner));
                            }
                        }
                        "Selector" => {
                            if items.len() > 1 {
                                let kind = self.interpret_content_kind(&items[1]);
                                return MrlType::Selector(kind);
                            }
                        }
                        _ => {}
                    }
                }
                MrlType::Dyn
            }

            // Record type: { field: Type, ... }
            Shrubbery::Braces(items, _) => {
                let mut fields = Vec::new();
                for item in items {
                    // Look for `name: type` patterns
                    if let Shrubbery::Sequence(parts, _) = item {
                        if parts.len() >= 2 {
                            if let Some((sym, _)) = parts[0].as_identifier() {
                                let field_name = self.symbol_names.get(&sym)
                                    .cloned()
                                    .unwrap_or_else(|| format!("f{}", sym.id()));
                                let field_ty = self.interpret_type(&parts[1]);
                                fields.push((field_name, field_ty));
                            }
                        }
                    }
                }
                if fields.is_empty() {
                    MrlType::Dyn
                } else {
                    MrlType::Record(fields)
                }
            }

            // Function type: (A, B) -> C
            // For now, default to Dyn
            _ => MrlType::Dyn,
        }
    }

    /// Interpret a ContentKind from a type expression
    fn interpret_content_kind(&self, shrub: &crate::shrubbery::Shrubbery) -> ContentKind {
        if let Some((sym, _)) = shrub.as_identifier() {
            let name = self.symbol_names.get(&sym)
                .cloned()
                .unwrap_or_default();

            match name.as_str() {
                "Block" | "block" => ContentKind::Block,
                "Inline" | "inline" => ContentKind::Inline,
                _ => ContentKind::Content,
            }
        } else {
            ContentKind::Content
        }
    }

    /// Unify two types, returning their common type
    fn unify_types(&self, t1: &MrlType, t2: &MrlType, span: Span) -> Result<MrlType> {
        if t1.is_subtype_of(t2) {
            Ok(t2.clone())
        } else if t2.is_subtype_of(t1) {
            Ok(t1.clone())
        } else {
            Err(MrlError::TypeError {
                span,
                message: format!("Cannot unify types {} and {}", t1, t2),
            })
        }
    }

    // =========================================================================
    // Definition form type checking
    // =========================================================================

    /// Check a function/macro definition
    fn check_def(
        &mut self,
        name: Symbol,
        params: &[crate::shrubbery::Param],
        return_type: Option<&Expr>,
        body: &Expr,
        span: Span,
    ) -> Result<MrlType> {
        // Build parameter types from annotations (default to Dyn if unspecified)
        let param_tys: Vec<MrlType> = params
            .iter()
            .map(|p| {
                if let Some(type_ann) = &p.type_annotation {
                    self.interpret_type(type_ann)
                } else {
                    MrlType::Dyn
                }
            })
            .collect();

        // Expected return type from annotation
        let expected_ret = if let Some(rt_expr) = return_type {
            // Return type is an Expr, need to convert if it's a type expression
            // For now, check if it's an identifier and interpret as type
            match rt_expr {
                Expr::Var(sym, _, _) => {
                    let name = self.symbol_names.get(sym)
                        .cloned()
                        .unwrap_or_default();
                    match name.as_str() {
                        "None" | "none" => MrlType::None,
                        "Bool" | "bool" => MrlType::Bool,
                        "Int" | "int" => MrlType::Int,
                        "Float" | "float" => MrlType::Float,
                        "String" | "string" | "str" => MrlType::String,
                        "Content" | "content" => MrlType::Content,
                        "Block" | "block" => MrlType::Block,
                        "Inline" | "inline" => MrlType::Inline,
                        _ => MrlType::Dyn
                    }
                }
                _ => MrlType::Dyn
            }
        } else {
            MrlType::Dyn
        };

        // Create child environment with parameters bound at current stage
        let mut child_env = self.env.child();
        for (param, ty) in params.iter().zip(param_tys.iter()) {
            child_env.bind_at_stage(
                format!("id:{}", param.name.id()),
                ty.clone(),
                self.stage_level,
            );
        }

        // Check body in extended environment
        let old_env = std::mem::replace(&mut self.env, child_env);
        let body_ty = self.check_expr(body)?;
        self.env = old_env;

        // Verify return type if specified
        if expected_ret != MrlType::Dyn && !body_ty.is_subtype_of(&expected_ret) {
            return Err(MrlError::TypeError {
                span,
                message: format!(
                    "Function body type {} doesn't match declared return type {}",
                    body_ty, expected_ret
                ),
            });
        }

        // Build function type
        let func_ty = MrlType::Function {
            params: param_tys,
            ret: Box::new(body_ty),
        };

        // Bind the function name at current stage
        self.env.bind_at_stage(
            format!("id:{}", name.id()),
            func_ty.clone(),
            self.stage_level,
        );

        // Definitions return None (they're statements)
        Ok(MrlType::None)
    }

    /// Check a staged block
    fn check_staged(&mut self, body: &Expr, span: Span) -> Result<MrlType> {
        // Increment stage level while checking body
        self.stage_level += 1;
        let body_ty = self.check_expr(body)?;
        self.stage_level -= 1;

        // Staged blocks evaluate at expand-time and produce their result
        // The type is Code<K> if body produces content, otherwise just the body type
        if let Some(kind) = body_ty.as_content_kind() {
            Ok(MrlType::Code(kind))
        } else {
            // Non-content staged code just produces its value
            Ok(body_ty)
        }
    }

    /// Check a show rule: !show selector: transform
    ///
    /// Show rules have the typing:
    /// - selector: Selector<K> for some content kind K
    /// - transform: K -> K (with implicit `it` binding of type K)
    /// - result: None (rules are expand-time effects)
    fn check_show_rule(&mut self, selector: &Expr, transform: &Expr, span: Span) -> Result<MrlType> {
        // Check selector - should be Selector<K>
        let selector_ty = self.check_expr(selector)?;

        let content_kind = match &selector_ty {
            MrlType::Selector(k) => *k,
            // Allow bare identifiers to act as selectors (e.g., `heading`)
            MrlType::Dyn => ContentKind::Content,
            other => {
                return Err(MrlError::TypeError {
                    span: selector.span(),
                    message: format!("Show rule selector must be Selector<K>, got {}", other),
                });
            }
        };

        // The transform gets an implicit `it` binding of the selected type
        let it_ty = match content_kind {
            ContentKind::Block => MrlType::Block,
            ContentKind::Inline => MrlType::Inline,
            ContentKind::Content => MrlType::Content,
        };

        // Create child environment with `it` bound at current stage
        let mut child_env = self.env.child();
        child_env.bind_at_stage("it".to_string(), it_ty.clone(), self.stage_level);

        // Check transform in extended environment
        let old_env = std::mem::replace(&mut self.env, child_env);
        let transform_ty = self.check_expr(transform)?;
        self.env = old_env;

        // Transform must return the same kind (K -> K)
        if !transform_ty.is_subtype_of(&it_ty) {
            return Err(MrlError::TypeError {
                span: transform.span(),
                message: format!(
                    "Show transform must return {}, got {} (transforms must preserve kind)",
                    it_ty, transform_ty
                ),
            });
        }

        // Show rules are expand-time effects, return None
        Ok(MrlType::None)
    }

    /// Check a set rule: !set selector { properties }
    ///
    /// Set rules have the typing:
    /// - selector: Selector<K> for some content kind K
    /// - properties: must match valid properties for the selector
    /// - result: None (rules are expand-time effects)
    fn check_set_rule(
        &mut self,
        selector: &Expr,
        properties: &[(Symbol, Expr)],
        span: Span,
    ) -> Result<MrlType> {
        // Check selector
        let selector_ty = self.check_expr(selector)?;

        let _content_kind = match &selector_ty {
            MrlType::Selector(k) => *k,
            MrlType::Dyn => ContentKind::Content,
            other => {
                return Err(MrlError::TypeError {
                    span: selector.span(),
                    message: format!("Set rule selector must be Selector<K>, got {}", other),
                });
            }
        };

        // Check each property value
        // TODO: Validate property names against known properties for the selector
        for (prop_sym, prop_expr) in properties {
            let prop_ty = self.check_expr(prop_expr)?;
            // For now, accept any type for properties
            // A full implementation would look up valid properties by selector
        }

        // Set rules are expand-time effects, return None
        Ok(MrlType::None)
    }

    /// Check a live block: !live[deps] { body }
    ///
    /// Live blocks have the typing:
    /// - deps: optional list of dependencies
    /// - body: any expression (evaluated at render-time)
    /// - result: None at expand-time (live blocks are captured for render-time)
    fn check_live(
        &mut self,
        deps: Option<&Vec<Symbol>>,
        body: &Expr,
        span: Span,
    ) -> Result<MrlType> {
        // Check that dependencies are bound
        if let Some(dep_list) = deps {
            for dep in dep_list {
                let key = format!("id:{}", dep.id());
                if self.env.lookup(&key).is_none() {
                    return Err(MrlError::UnboundIdentifier {
                        span,
                        name: key,
                    });
                }
            }
        }

        // Check the body (but don't enforce type constraints - live blocks
        // can contain arbitrary render-time code)
        let body_ty = self.check_expr(body)?;

        // Live blocks at expand-time return None (they're captured for later)
        // The actual evaluation happens at render-time
        Ok(MrlType::None)
    }

    /// Bind a selector type for a given name
    pub fn bind_selector(&mut self, name: &str, kind: ContentKind) {
        self.env.bind(name.to_string(), MrlType::Selector(kind));
    }

    /// Bind a symbol to a type
    pub fn bind_symbol(&mut self, sym: Symbol, ty: MrlType) {
        self.env.bind(format!("id:{}", sym.id()), ty);
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
    use crate::enforest::Expr;
    use crate::lexer::tokenize;
    use crate::parser::parse;
    use crate::shrubbery::{Param, ScopeSet};

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

    // =========================================================================
    // Expr type checking tests
    // =========================================================================

    #[test]
    fn test_check_expr_literal() {
        let mut checker = TypeChecker::new();
        let expr = Expr::Literal(Literal::Int(42), Span::new(0, 2));
        let ty = checker.check_expr(&expr).unwrap();
        assert_eq!(ty, MrlType::Int);
    }

    #[test]
    fn test_check_expr_binop_arithmetic() {
        let mut checker = TypeChecker::new();
        let expr = Expr::BinOp(
            Box::new(Expr::Literal(Literal::Int(1), Span::new(0, 1))),
            BinOp::Add,
            Box::new(Expr::Literal(Literal::Int(2), Span::new(4, 5))),
            Span::new(0, 5),
        );
        let ty = checker.check_expr(&expr).unwrap();
        assert_eq!(ty, MrlType::Int);
    }

    #[test]
    fn test_check_expr_binop_comparison() {
        let mut checker = TypeChecker::new();
        let expr = Expr::BinOp(
            Box::new(Expr::Literal(Literal::Int(1), Span::new(0, 1))),
            BinOp::Lt,
            Box::new(Expr::Literal(Literal::Int(2), Span::new(4, 5))),
            Span::new(0, 5),
        );
        let ty = checker.check_expr(&expr).unwrap();
        assert_eq!(ty, MrlType::Bool);
    }

    #[test]
    fn test_check_expr_unop_neg() {
        let mut checker = TypeChecker::new();
        let expr = Expr::UnOp(
            UnOp::Neg,
            Box::new(Expr::Literal(Literal::Int(42), Span::new(1, 3))),
            Span::new(0, 3),
        );
        let ty = checker.check_expr(&expr).unwrap();
        assert_eq!(ty, MrlType::Int);
    }

    #[test]
    fn test_check_expr_if() {
        let mut checker = TypeChecker::new();
        let expr = Expr::If(
            Box::new(Expr::Literal(Literal::Bool(true), Span::new(3, 7))),
            Box::new(Expr::Literal(Literal::Int(1), Span::new(9, 10))),
            Some(Box::new(Expr::Literal(Literal::Int(2), Span::new(16, 17)))),
            Span::new(0, 17),
        );
        let ty = checker.check_expr(&expr).unwrap();
        assert_eq!(ty, MrlType::Int);
    }

    #[test]
    fn test_check_expr_if_type_error() {
        let mut checker = TypeChecker::new();
        // if 42 then 1 else 2 - condition not Bool
        let expr = Expr::If(
            Box::new(Expr::Literal(Literal::Int(42), Span::new(3, 5))),
            Box::new(Expr::Literal(Literal::Int(1), Span::new(11, 12))),
            None,
            Span::new(0, 12),
        );
        let result = checker.check_expr(&expr);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_expr_quote_splice() {
        let mut checker = TypeChecker::new();

        // Content::text creates Inline content, so Quote produces Code<Inline>
        let quote_expr = Expr::Quote(
            Box::new(Expr::Content(
                crate::content::Content::text("hello"),
                Span::new(1, 7),
            )),
            Span::new(0, 8),
        );
        let ty = checker.check_expr(&quote_expr).unwrap();
        assert_eq!(ty, MrlType::Code(ContentKind::Inline));

        // Test that splice at stage 0 is disallowed
        let splice_at_0 = Expr::Splice(Box::new(quote_expr.clone()), Span::new(0, 10));
        let result = checker.check_expr(&splice_at_0);
        assert!(result.is_err()); // CSP: cannot splice at stage 0
    }

    #[test]
    fn test_check_expr_splice_inside_quote() {
        let mut checker = TypeChecker::new();

        // Bind a variable x with type Code<Inline> at stage 0
        let x_sym = Symbol::new(100);
        checker.bind_symbol(x_sym, MrlType::Code(ContentKind::Inline));
        checker.register_symbol(x_sym, "x");

        // quote[ $x ] - splice x inside a quote (valid: stage 1, splice to stage 0)
        // The splice accesses x at stage 0, which is valid (0 <= 1 after splice decrements)
        let expr = Expr::Quote(
            Box::new(Expr::Splice(
                Box::new(Expr::Var(x_sym, ScopeSet::new(), Span::new(8, 9))),
                Span::new(7, 10),
            )),
            Span::new(0, 12),
        );

        let ty = checker.check_expr(&expr).unwrap();
        // The splice unwraps Code<Inline> to Inline, then Quote wraps to Code<Inline>
        assert_eq!(ty, MrlType::Code(ContentKind::Inline));
    }

    #[test]
    fn test_check_expr_csp_violation() {
        let mut checker = TypeChecker::new();

        // Bind x at stage 1 (inside a quote)
        let x_sym = Symbol::new(100);
        checker.register_symbol(x_sym, "x");

        // Create an expression that binds x inside a quote and tries to use it outside
        // This is: quote[ for x in [...]: $x ]
        // where $x tries to access x, but x is bound at stage 1, and after splice we're at stage 0
        // This should fail CSP: binding stage (1) > current stage (0)

        // Simpler test: just test binding lookup with explicit stage setup
        // Bind x at stage 1
        checker.env.bind_at_stage(format!("id:{}", x_sym.id()), MrlType::Int, 1);

        // Try to access x at stage 0 - should fail
        let var_expr = Expr::Var(x_sym, ScopeSet::new(), Span::new(0, 1));
        let result = checker.check_expr(&var_expr);
        assert!(result.is_err(), "CSP violation: cannot use stage-1 binding at stage 0");
    }

    #[test]
    fn test_check_expr_csp_valid_higher_stage() {
        let mut checker = TypeChecker::new();

        // Bind x at stage 0
        let x_sym = Symbol::new(100);
        checker.bind_symbol(x_sym, MrlType::Int);
        checker.register_symbol(x_sym, "x");

        // Access x inside a quote (stage 1) - should succeed
        // CSP: binding stage (0) <= current stage (1)
        let expr = Expr::Quote(
            Box::new(Expr::Var(x_sym, ScopeSet::new(), Span::new(7, 8))),
            Span::new(0, 10),
        );

        let ty = checker.check_expr(&expr).unwrap();
        // Quote wraps Int to... well, Int isn't content, so it defaults to Code<Content>
        // Actually, let's check what type we get
        assert_eq!(ty, MrlType::Code(ContentKind::Content));
    }

    #[test]
    fn test_check_expr_def() {
        let mut checker = TypeChecker::new();
        let name = Symbol::new(100);
        let expr = Expr::Def {
            name,
            params: vec![],
            return_type: None,
            body: Box::new(Expr::Literal(Literal::Int(42), Span::new(10, 12))),
            span: Span::new(0, 12),
        };

        // Def should type check and return None
        let ty = checker.check_expr(&expr).unwrap();
        assert_eq!(ty, MrlType::None);

        // Function should be bound in environment
        let func_ty = checker.env.lookup("id:100").unwrap();
        match func_ty {
            MrlType::Function { params, ret } => {
                assert!(params.is_empty());
                assert_eq!(**ret, MrlType::Int);
            }
            _ => panic!("Expected function type"),
        }
    }

    #[test]
    fn test_check_expr_staged() {
        let mut checker = TypeChecker::new();
        // Content::text creates Inline content
        let expr = Expr::Staged(
            Box::new(Expr::Content(
                crate::content::Content::text("hello"),
                Span::new(8, 14),
            )),
            Span::new(0, 15),
        );
        let ty = checker.check_expr(&expr).unwrap();
        // Staged inline content produces Code<Inline>
        assert_eq!(ty, MrlType::Code(ContentKind::Inline));
    }

    #[test]
    fn test_check_expr_show_rule() {
        let mut checker = TypeChecker::new();

        // Bind 'heading' as a selector
        checker.bind_selector("heading", ContentKind::Block);
        let heading_sym = Symbol::new(50);
        checker.bind_symbol(heading_sym, MrlType::Selector(ContentKind::Block));

        // Show rule with Content literal transform
        let expr = Expr::ShowRule {
            selector: Box::new(Expr::Var(heading_sym, ScopeSet::new(), Span::new(5, 12))),
            transform: Box::new(Expr::Content(
                crate::content::Content::Block(crate::content::Block::Paragraph {
                    body: Box::new(crate::content::Inline::Text("test".to_string())),
                    attrs: crate::content::Attributes::new(),
                }),
                Span::new(14, 20),
            )),
            span: Span::new(0, 20),
        };

        let ty = checker.check_expr(&expr).unwrap();
        assert_eq!(ty, MrlType::None);
    }

    #[test]
    fn test_check_expr_show_rule_with_it() {
        let mut checker = TypeChecker::new();

        // Bind 'heading' as a Block selector
        let heading_sym = Symbol::new(50);
        checker.bind_symbol(heading_sym, MrlType::Selector(ContentKind::Block));

        // Symbol for 'it' (as would be created by parser)
        let it_sym = Symbol::new(51);
        checker.register_symbol(it_sym, "it");

        // !show heading: it
        // Transform just returns `it` directly - should type as Block
        let expr = Expr::ShowRule {
            selector: Box::new(Expr::Var(heading_sym, ScopeSet::new(), Span::new(5, 12))),
            transform: Box::new(Expr::Var(it_sym, ScopeSet::new(), Span::new(14, 16))),
            span: Span::new(0, 20),
        };

        // This should succeed: `it` is implicitly bound to Block, transform returns Block
        let ty = checker.check_expr(&expr).unwrap();
        assert_eq!(ty, MrlType::None);
    }

    #[test]
    fn test_check_expr_show_rule_it_wrong_kind() {
        let mut checker = TypeChecker::new();

        // Bind 'heading' as a Block selector
        let heading_sym = Symbol::new(50);
        checker.bind_symbol(heading_sym, MrlType::Selector(ContentKind::Block));

        // Symbol for 'it'
        let it_sym = Symbol::new(51);
        checker.register_symbol(it_sym, "it");

        // Symbol for a function that returns Inline
        let text_fn_sym = Symbol::new(52);
        checker.bind_symbol(text_fn_sym, MrlType::Function {
            params: vec![MrlType::Block],
            ret: Box::new(MrlType::Inline),
        });
        checker.register_symbol(text_fn_sym, "extract_text");

        // !show heading: extract_text(it)
        // Transform returns Inline but selector is Block - should fail
        let expr = Expr::ShowRule {
            selector: Box::new(Expr::Var(heading_sym, ScopeSet::new(), Span::new(5, 12))),
            transform: Box::new(Expr::Call(
                Box::new(Expr::Var(text_fn_sym, ScopeSet::new(), Span::new(14, 26))),
                vec![Expr::Var(it_sym, ScopeSet::new(), Span::new(27, 29))],
                Span::new(14, 30),
            )),
            span: Span::new(0, 30),
        };

        // Should fail: transform returns Inline, but selector expects Block
        let result = checker.check_expr(&expr);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_expr_show_rule_kind_mismatch() {
        let mut checker = TypeChecker::new();

        // Bind 'heading' as Block selector
        let heading_sym = Symbol::new(50);
        checker.bind_symbol(heading_sym, MrlType::Selector(ContentKind::Block));

        // Transform returns Inline (should fail for Block selector)
        let expr = Expr::ShowRule {
            selector: Box::new(Expr::Var(heading_sym, ScopeSet::new(), Span::new(5, 12))),
            transform: Box::new(Expr::Content(
                crate::content::Content::Inline(crate::content::Inline::Text("inline".to_string())),
                Span::new(14, 20),
            )),
            span: Span::new(0, 20),
        };

        let result = checker.check_expr(&expr);
        // This should fail because Inline is not a subtype of Block
        assert!(result.is_err());
    }

    #[test]
    fn test_check_expr_set_rule() {
        let mut checker = TypeChecker::new();

        let heading_sym = Symbol::new(50);
        checker.bind_symbol(heading_sym, MrlType::Selector(ContentKind::Block));

        let size_sym = Symbol::new(51);
        let expr = Expr::SetRule {
            selector: Box::new(Expr::Var(heading_sym, ScopeSet::new(), Span::new(4, 11))),
            properties: vec![
                (size_sym, Expr::Literal(Literal::Int(14), Span::new(20, 22))),
            ],
            span: Span::new(0, 25),
        };

        let ty = checker.check_expr(&expr).unwrap();
        assert_eq!(ty, MrlType::None);
    }

    #[test]
    fn test_check_expr_live() {
        let mut checker = TypeChecker::new();

        // Bind a dependency
        let dep_sym = Symbol::new(200);
        checker.bind_symbol(dep_sym, MrlType::Int);

        let expr = Expr::Live {
            deps: Some(vec![dep_sym]),
            body: Box::new(Expr::Literal(Literal::Int(42), Span::new(15, 17))),
            span: Span::new(0, 20),
        };

        let ty = checker.check_expr(&expr).unwrap();
        assert_eq!(ty, MrlType::None);
    }

    #[test]
    fn test_check_expr_live_unbound_dep() {
        let mut checker = TypeChecker::new();

        // Reference an unbound dependency
        let dep_sym = Symbol::new(999);

        let expr = Expr::Live {
            deps: Some(vec![dep_sym]),
            body: Box::new(Expr::Literal(Literal::Int(42), Span::new(15, 17))),
            span: Span::new(0, 20),
        };

        let result = checker.check_expr(&expr);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_expr_block() {
        let mut checker = TypeChecker::new();

        let expr = Expr::Block(
            vec![
                Expr::Literal(Literal::Int(1), Span::new(0, 1)),
                Expr::Literal(Literal::String("hello".to_string()), Span::new(3, 10)),
            ],
            Span::new(0, 10),
        );

        // Block type is type of last expression
        let ty = checker.check_expr(&expr).unwrap();
        assert_eq!(ty, MrlType::String);
    }

    #[test]
    fn test_check_expr_for() {
        let mut checker = TypeChecker::new();

        // Bind an array
        let arr_sym = Symbol::new(10);
        checker.bind_symbol(arr_sym, MrlType::Array(Box::new(MrlType::Int)));

        let loop_var = Symbol::new(11);
        let expr = Expr::For(
            loop_var,
            Box::new(Expr::Var(arr_sym, ScopeSet::new(), Span::new(8, 11))),
            Box::new(Expr::Literal(Literal::String("x".to_string()), Span::new(15, 18))),
            Span::new(0, 20),
        );

        let ty = checker.check_expr(&expr).unwrap();
        // For produces Array<body_type>
        assert_eq!(ty, MrlType::Array(Box::new(MrlType::String)));
    }

    #[test]
    fn test_check_field_access_record() {
        let mut checker = TypeChecker::new();

        // Create a record variable
        let rec_sym = Symbol::new(100);
        let record_ty = MrlType::Record(vec![
            ("name".to_string(), MrlType::String),
            ("age".to_string(), MrlType::Int),
        ]);
        checker.bind_symbol(rec_sym, record_ty);

        // Register field symbols
        let name_sym = Symbol::new(101);
        let age_sym = Symbol::new(102);
        checker.register_symbol(name_sym, "name");
        checker.register_symbol(age_sym, "age");

        // Test: rec.name should be String
        let expr = Expr::FieldAccess(
            Box::new(Expr::Var(rec_sym, ScopeSet::new(), Span::new(0, 3))),
            name_sym,
            Span::new(0, 8),
        );
        let ty = checker.check_expr(&expr).unwrap();
        assert_eq!(ty, MrlType::String);

        // Test: rec.age should be Int
        let expr2 = Expr::FieldAccess(
            Box::new(Expr::Var(rec_sym, ScopeSet::new(), Span::new(0, 3))),
            age_sym,
            Span::new(0, 7),
        );
        let ty2 = checker.check_expr(&expr2).unwrap();
        assert_eq!(ty2, MrlType::Int);
    }

    #[test]
    fn test_check_field_access_record_error() {
        let mut checker = TypeChecker::new();

        // Create a record variable
        let rec_sym = Symbol::new(100);
        let record_ty = MrlType::Record(vec![
            ("name".to_string(), MrlType::String),
        ]);
        checker.bind_symbol(rec_sym, record_ty);

        // Register a field symbol that doesn't exist on the record
        let invalid_sym = Symbol::new(999);
        checker.register_symbol(invalid_sym, "missing_field");

        // Test: rec.missing_field should error
        let expr = Expr::FieldAccess(
            Box::new(Expr::Var(rec_sym, ScopeSet::new(), Span::new(0, 3))),
            invalid_sym,
            Span::new(0, 17),
        );
        let result = checker.check_expr(&expr);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_field_access_block_attrs() {
        let mut checker = TypeChecker::new();

        // Create a Block variable
        let block_sym = Symbol::new(100);
        checker.bind_symbol(block_sym, MrlType::Block);

        // Register attrs field
        let attrs_sym = Symbol::new(101);
        checker.register_symbol(attrs_sym, "attrs");

        // Test: block.attrs should return a record type
        let expr = Expr::FieldAccess(
            Box::new(Expr::Var(block_sym, ScopeSet::new(), Span::new(0, 5))),
            attrs_sym,
            Span::new(0, 11),
        );
        let ty = checker.check_expr(&expr).unwrap();
        assert!(matches!(ty, MrlType::Record(_)));
    }

    #[test]
    fn test_check_field_access_dyn() {
        let mut checker = TypeChecker::new();

        // Create a Dyn variable
        let dyn_sym = Symbol::new(100);
        checker.bind_symbol(dyn_sym, MrlType::Dyn);

        // Register any field
        let any_sym = Symbol::new(101);
        checker.register_symbol(any_sym, "anything");

        // Test: dyn.anything should be Dyn
        let expr = Expr::FieldAccess(
            Box::new(Expr::Var(dyn_sym, ScopeSet::new(), Span::new(0, 3))),
            any_sym,
            Span::new(0, 12),
        );
        let ty = checker.check_expr(&expr).unwrap();
        assert_eq!(ty, MrlType::Dyn);
    }

    #[test]
    fn test_check_def_with_type_annotations() {
        let mut checker = TypeChecker::new();

        // Create symbols
        let func_name = Symbol::new(100);
        let param_name = Symbol::new(101);
        let string_type_sym = Symbol::new(102);

        // Register symbols
        checker.register_symbol(func_name, "greet");
        checker.register_symbol(param_name, "name");
        checker.register_symbol(string_type_sym, "String");

        // Create type annotation shrubbery for String
        let type_ann = Shrubbery::Identifier(string_type_sym, ScopeSet::new(), Span::new(6, 12));

        // Create param with type annotation
        let param = Param::new(param_name, Span::new(0, 4))
            .with_type(type_ann);

        // def greet(name: String): "Hello"
        let expr = Expr::Def {
            name: func_name,
            params: vec![param],
            return_type: None,
            body: Box::new(Expr::Literal(Literal::String("Hello".to_string()), Span::new(20, 27))),
            span: Span::new(0, 27),
        };

        let ty = checker.check_expr(&expr).unwrap();
        assert_eq!(ty, MrlType::None);

        // Check the bound function has correct type
        let func_ty = checker.env.lookup(&format!("id:{}", func_name.id())).unwrap();
        match func_ty {
            MrlType::Function { params, ret } => {
                assert_eq!(params.len(), 1);
                assert_eq!(params[0], MrlType::String); // Type annotation was respected
                assert_eq!(**ret, MrlType::String);
            }
            _ => panic!("Expected function type"),
        }
    }
}
