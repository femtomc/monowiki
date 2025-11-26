use crate::content::Content;
use crate::error::{MrlError, Result, Span};
use crate::hygiene::{Binding, HygieneEnv, MacroContext, Space, SpaceRegistry};
use crate::rules::{apply_show_rules, RuleSet, Selector, SelectorBase, SetRule, SetValue, ShowRule};
use crate::shrubbery::{Scope, ScopeSet, Shrubbery, Symbol};
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

    /// User-defined macro with hygiene support
    Macro {
        name: String,
        params: Vec<String>,
        body: Box<Shrubbery>,
        /// The scope introduced by this macro definition
        macro_scope: Scope,
        /// Captured hygiene environment
        env: HygieneEnv,
    },
}

/// Expander state
pub struct Expander {
    /// Variable bindings
    env: HashMap<String, ExpandValue>,

    /// Hygiene environment
    hygiene: HygieneEnv,

    /// Space registry for namespace-aware lookup
    spaces: SpaceRegistry,

    /// Next scope ID
    next_scope_id: u64,

    /// Macro definitions
    macros: HashMap<String, ExpandFunction>,

    /// Collected show/set rules
    rules: RuleSet,

    /// Symbol table for resolving symbol IDs to names
    symbols: HashMap<u64, String>,

    /// Bindings organized by (symbol_id, space) for space-aware lookup
    space_bindings: HashMap<(u64, Option<u64>), ExpandValue>,
}

impl Expander {
    pub fn new() -> Self {
        let mut expander = Self {
            env: HashMap::new(),
            hygiene: HygieneEnv::new(),
            spaces: SpaceRegistry::new(),
            next_scope_id: 0,
            macros: HashMap::new(),
            rules: RuleSet::new(),
            symbols: HashMap::new(),
            space_bindings: HashMap::new(),
        };

        // Register built-in functions
        expander.register_builtins();

        expander
    }

    /// Get the space registry
    pub fn spaces(&self) -> &SpaceRegistry {
        &self.spaces
    }

    /// Bind a value in a specific space
    pub fn bind_in_space(&mut self, sym: Symbol, space: Option<Space>, value: ExpandValue) {
        let space_id = space.map(|s| s.id());
        self.space_bindings.insert((sym.id(), space_id), value);
    }

    /// Look up a value considering spaces in the scope set
    fn lookup_with_spaces(&self, sym: Symbol, scopes: &ScopeSet) -> Option<ExpandValue> {
        // Check if any scope in the set matches a space
        let expr_space_id = self.spaces.expr.id();
        let bind_space_id = self.spaces.bind.id();

        // Check if scopes contain the expr space
        let in_expr_space = scopes.iter().any(|s| s.id() == expr_space_id);
        let in_bind_space = scopes.iter().any(|s| s.id() == bind_space_id);

        // Try space-specific lookup first
        if in_expr_space {
            if let Some(v) = self.space_bindings.get(&(sym.id(), Some(expr_space_id))) {
                return Some(v.clone());
            }
        }
        if in_bind_space {
            if let Some(v) = self.space_bindings.get(&(sym.id(), Some(bind_space_id))) {
                return Some(v.clone());
            }
        }

        // Fall back to unspaced binding
        if let Some(v) = self.space_bindings.get(&(sym.id(), None)) {
            return Some(v.clone());
        }

        None
    }

    /// Set the symbol table for resolving symbol IDs to names
    pub fn set_symbols(&mut self, symbols: HashMap<u64, String>) {
        self.symbols = symbols;
    }

    /// Register a symbol
    pub fn register_symbol(&mut self, id: u64, name: String) {
        self.symbols.insert(id, name);
    }

    /// Get the collected rules
    pub fn rules(&self) -> &RuleSet {
        &self.rules
    }

    /// Get mutable access to the rules
    pub fn rules_mut(&mut self) -> &mut RuleSet {
        &mut self.rules
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

    /// Define a macro with hygiene support
    ///
    /// Each macro gets its own scope, used to maintain hygiene during expansion.
    /// The macro scope is added to all identifiers in the macro body, then flipped
    /// at expansion time to distinguish macro-internal from user-visible bindings.
    pub fn define_macro(&mut self, name: String, params: Vec<String>, body: Shrubbery) {
        // Each macro definition gets a fresh scope
        let macro_scope = self.fresh_scope();

        // Add the macro scope to all identifiers in the body
        let mut body = body;
        body.add_scope(macro_scope);

        // Add parameter bindings to the hygiene environment
        let mut macro_env = self.hygiene.child();
        for (i, param) in params.iter().enumerate() {
            // Create a binding for each parameter with the macro scope
            let param_symbol = Symbol::new(i as u64);
            let mut param_scopes = ScopeSet::new();
            param_scopes.add(macro_scope);
            macro_env.add_binding(Binding::new(param_symbol, param_scopes, Span::default()));

            // Also register the symbol name for lookup
            self.symbols.insert(i as u64, param.clone());
        }

        self.macros.insert(
            name.clone(),
            ExpandFunction::Macro {
                name: name.clone(),
                params,
                body: Box::new(body),
                macro_scope,
                env: macro_env,
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
                // 1. Try space-aware lookup first (uses scope set to determine space)
                if let Some(value) = self.lookup_with_spaces(*sym, scopes) {
                    return Ok(value);
                }

                // 2. Try hygiene-aware resolution via scope sets
                if let Some(_binding) = self.hygiene.resolve(*sym, scopes) {
                    // Found a binding via scope set resolution
                    // Look up the value by the symbol's registered name
                    if let Some(name) = self.symbols.get(&sym.id()) {
                        if let Some(value) = self.env.get(name) {
                            return Ok(value.clone());
                        }
                    }
                }

                // 3. Fallback: look up by registered symbol name directly
                if let Some(name) = self.symbols.get(&sym.id()) {
                    if let Some(value) = self.env.get(name) {
                        return Ok(value.clone());
                    }
                    return Err(MrlError::UnboundIdentifier {
                        span: *span,
                        name: name.clone(),
                    });
                }

                // 4. Final fallback for unregistered symbols (legacy)
                let name = format!("id:{}", sym.id());
                self.env.get(&name).cloned().ok_or_else(|| MrlError::UnboundIdentifier {
                    span: *span,
                    name,
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

            Shrubbery::Brackets(items, _span) => {
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

            Shrubbery::ShowRule { selector, transform, span } => {
                // Collect the show rule
                let sel = self.parse_selector(selector)?;
                self.rules.add_show_rule(ShowRule {
                    selector: sel,
                    transform: transform.clone(),
                    span: *span,
                });
                Ok(ExpandValue::None)
            }

            Shrubbery::SetRule { selector, properties, span } => {
                // Collect the set rule
                let sel = self.parse_selector(selector)?;
                let mut props = HashMap::new();
                for (sym, value_shrub) in properties {
                    let name = self.symbols.get(&sym.id())
                        .cloned()
                        .unwrap_or_else(|| format!("prop_{}", sym.id()));
                    let value = self.expand(value_shrub)?;
                    let set_value = match value {
                        ExpandValue::String(s) => SetValue::String(s),
                        ExpandValue::Int(i) => SetValue::Int(i),
                        ExpandValue::Bool(b) => SetValue::Bool(b),
                        _ => SetValue::String(format!("{:?}", value)),
                    };
                    props.insert(name, set_value);
                }
                self.rules.add_set_rule(SetRule {
                    selector: sel,
                    properties: props,
                    span: *span,
                });
                Ok(ExpandValue::None)
            }

            Shrubbery::DefBlock { name, params, body, span, .. } => {
                // Define a macro
                let name_str = self.symbols.get(&name.id())
                    .cloned()
                    .unwrap_or_else(|| format!("macro_{}", name.id()));
                let param_names: Vec<String> = params.iter()
                    .map(|p| self.symbols.get(&p.name.id())
                        .cloned()
                        .unwrap_or_else(|| format!("param_{}", p.name.id())))
                    .collect();

                // Wrap body in a sequence if multiple items
                let body_shrub = if body.len() == 1 {
                    body[0].clone()
                } else {
                    Shrubbery::Sequence(body.clone(), *span)
                };

                self.define_macro(name_str, param_names, body_shrub);
                Ok(ExpandValue::None)
            }

            Shrubbery::Quote { body, .. } => {
                // Return quoted code
                Ok(ExpandValue::Code(body.clone(), ContentKind::Content))
            }

            Shrubbery::Splice { expr, span } => {
                // Evaluate and splice
                let value = self.expand(expr)?;
                match value {
                    ExpandValue::Code(shrub, _) => self.expand(&shrub),
                    _ => Err(MrlError::ExpansionError {
                        span: *span,
                        message: "Splice requires quoted code".to_string(),
                    }),
                }
            }

            Shrubbery::If { condition, then_branch, else_branch, span: _ } => {
                let cond = self.expand(condition)?;
                let is_true = match cond {
                    ExpandValue::Bool(b) => b,
                    ExpandValue::None => false,
                    ExpandValue::Int(i) => i != 0,
                    ExpandValue::String(s) => !s.is_empty(),
                    _ => true,
                };
                if is_true {
                    self.expand(then_branch)
                } else if let Some(else_br) = else_branch {
                    self.expand(else_br)
                } else {
                    Ok(ExpandValue::None)
                }
            }

            Shrubbery::For { pattern, iterable, body, span } => {
                let iter_val = self.expand(iterable)?;
                let items = match iter_val {
                    ExpandValue::Array(arr) => arr,
                    _ => return Err(MrlError::ExpansionError {
                        span: *span,
                        message: "For loop requires array".to_string(),
                    }),
                };

                let pattern_name = self.symbols.get(&pattern.id())
                    .cloned()
                    .unwrap_or_else(|| format!("var_{}", pattern.id()));

                let mut results = Vec::new();
                for item in items {
                    // Bind pattern to item
                    let saved = self.env.insert(pattern_name.clone(), item);
                    let result = self.expand(body)?;
                    if let Some(content) = result.as_content() {
                        results.push(content.clone());
                    }
                    // Restore
                    if let Some(v) = saved {
                        self.env.insert(pattern_name.clone(), v);
                    } else {
                        self.env.remove(&pattern_name);
                    }
                }

                if results.is_empty() {
                    Ok(ExpandValue::None)
                } else if results.len() == 1 {
                    Ok(ExpandValue::Content(results.into_iter().next().unwrap()))
                } else {
                    Ok(ExpandValue::Content(Content::Sequence(results)))
                }
            }

            Shrubbery::StagedBlock { body, span: _ } => {
                // Execute staged code at expand-time
                // This is the core of the three-phase execution model
                let mut results = Vec::new();
                for item in body {
                    let value = self.expand(item)?;
                    if let Some(content) = value.as_content() {
                        results.push(content.clone());
                    }
                }

                if results.is_empty() {
                    Ok(ExpandValue::None)
                } else if results.len() == 1 {
                    Ok(ExpandValue::Content(results.into_iter().next().unwrap()))
                } else {
                    Ok(ExpandValue::Content(Content::Sequence(results)))
                }
            }

            Shrubbery::LiveBlock { deps, body, span: _ } => {
                // Live blocks produce render-time code
                // For now, expand the body and wrap in a marker
                // Full implementation would produce JS/WASM for client-side execution
                let mut results = Vec::new();
                for item in body {
                    let value = self.expand(item)?;
                    if let Some(content) = value.as_content() {
                        results.push(content.clone());
                    }
                }

                // Return a Live marker with the expanded content
                // This would be processed by the renderer to generate reactive code
                let inner = if results.len() == 1 {
                    results.into_iter().next().unwrap()
                } else {
                    Content::Sequence(results)
                };

                // For now, just return the content - full implementation
                // would track dependencies and generate reactive bindings
                let _ = deps; // Will be used for dependency tracking
                Ok(ExpandValue::Content(inner))
            }

            Shrubbery::ContentBlock(items, _span) => {
                // Content block - collect and merge content
                let mut results = Vec::new();
                for item in items {
                    let value = self.expand(item)?;
                    if let Some(content) = value.as_content() {
                        results.push(content.clone());
                    }
                }

                if results.is_empty() {
                    Ok(ExpandValue::None)
                } else if results.len() == 1 {
                    Ok(ExpandValue::Content(results.into_iter().next().unwrap()))
                } else {
                    Ok(ExpandValue::Content(Content::Sequence(results)))
                }
            }

            _ => {
                // Other forms not yet implemented
                Ok(ExpandValue::Shrubbery(Box::new(shrub.clone())))
            }
        }
    }

    /// Parse a selector from shrubbery
    fn parse_selector(&self, shrub: &Shrubbery) -> Result<Selector> {
        match shrub {
            Shrubbery::Selector { base, predicate: _, span } => {
                let base_name = self.symbols.get(&base.id())
                    .ok_or_else(|| MrlError::ExpansionError {
                        span: *span,
                        message: format!("Unknown selector base: {}", base.id()),
                    })?;
                let base = SelectorBase::from_name(base_name)
                    .ok_or_else(|| MrlError::ExpansionError {
                        span: *span,
                        message: format!("Invalid selector type: {}", base_name),
                    })?;
                // TODO: Parse predicate
                Ok(Selector::new(base))
            }
            Shrubbery::Identifier(sym, _, span) => {
                let name = self.symbols.get(&sym.id())
                    .ok_or_else(|| MrlError::ExpansionError {
                        span: *span,
                        message: format!("Unknown identifier: {}", sym.id()),
                    })?;
                let base = SelectorBase::from_name(name)
                    .ok_or_else(|| MrlError::ExpansionError {
                        span: *span,
                        message: format!("Invalid selector type: {}", name),
                    })?;
                Ok(Selector::new(base))
            }
            _ => Err(MrlError::ExpansionError {
                span: shrub.span(),
                message: "Expected selector".to_string(),
            }),
        }
    }

    /// Expand shrubbery and apply collected rules to the result
    pub fn expand_with_rules(&mut self, shrub: &Shrubbery) -> Result<ExpandValue> {
        // First pass: expand and collect rules
        let value = self.expand(shrub)?;

        // If we have content and rules, apply them
        if let ExpandValue::Content(content) = value {
            let mut content = content;

            // Apply set rules first (they modify attributes)
            self.rules.apply_set_rules(&mut content);

            // Apply show rules (they transform content)
            if !self.rules.show_rules.is_empty() {
                let rules = self.rules.show_rules.clone();
                content = apply_show_rules(content, &rules, &mut |matched, transform| {
                    self.apply_show_transform(matched, transform)
                })?;
            }

            Ok(ExpandValue::Content(content))
        } else {
            Ok(value)
        }
    }

    /// Apply a show rule transform with `it` bound to the matched element
    fn apply_show_transform(&mut self, matched: &Content, transform: &Shrubbery) -> Result<Content> {
        // Bind `it` to the matched content
        let saved_it = self.env.insert("it".to_string(), ExpandValue::Content(matched.clone()));

        // Expand the transform
        let result = self.expand(transform)?;

        // Restore `it`
        if let Some(v) = saved_it {
            self.env.insert("it".to_string(), v);
        } else {
            self.env.remove("it");
        }

        // Extract content from result
        match result {
            ExpandValue::Content(c) => Ok(c),
            _ => Err(MrlError::ExpansionError {
                span: transform.span(),
                message: "Show rule transform must produce content".to_string(),
            }),
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

        // Get function name from identifier
        let func_name = if let Shrubbery::Identifier(sym, _scopes, _) = &items[0] {
            // Try to resolve via symbol table first
            self.symbols.get(&sym.id())
                .cloned()
                .unwrap_or_else(|| format!("id:{}", sym.id()))
        } else {
            return Err(MrlError::ExpansionError {
                span,
                message: "Call target must be identifier".to_string(),
            });
        };

        // First check if this is a macro
        if self.macros.contains_key(&func_name) {
            // Expand arguments
            let args: Result<Vec<_>> = items[1..].iter().map(|arg| self.expand(arg)).collect();
            let args = args?;
            return self.expand_macro(&func_name, &args, span);
        }

        // Look up function in environment
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
                // Macro expansion (shouldn't reach here since we check above, but handle anyway)
                self.expand_macro(&func_name, &args, span)
            }
            _ => Err(MrlError::ExpansionError {
                span,
                message: format!("{} is not a function", func_name),
            }),
        }
    }

    /// Expand a macro invocation with hygiene
    ///
    /// This implements the Rhombus-style scope-flipping algorithm:
    /// 1. Create a fresh use-site scope for this invocation
    /// 2. Add use-site scope to all arguments
    /// 3. Substitute arguments into the body
    /// 4. Apply scope flipping via MacroContext
    /// 5. Expand the hygienically-adjusted body
    fn expand_macro(&mut self, name: &str, args: &[ExpandValue], span: Span) -> Result<ExpandValue> {
        let macro_def = self.macros.get(name).cloned().ok_or_else(|| MrlError::ExpansionError {
            span,
            message: format!("Undefined macro: {}", name),
        })?;

        match macro_def {
            ExpandFunction::Macro { params, body, macro_scope, env: macro_env, .. } => {
                // Step 1: Create a fresh use-site scope for this invocation
                let use_scope = self.fresh_scope();

                // Step 2: Create the MacroContext for hygiene application
                let ctx = MacroContext::new(macro_scope, use_scope, macro_env.clone());

                // Step 3: Bind parameters in the value environment
                // The parameters were already registered with the macro scope when the macro was defined.
                // Now we bind them to the actual argument values.
                let mut new_env = self.env.clone();
                for (param, arg) in params.iter().zip(args.iter()) {
                    new_env.insert(param.clone(), arg.clone());
                }

                // Step 4: Clone the body and apply hygiene (scope flipping)
                // The body already has macro_scope added during define_macro.
                // apply_hygiene will add macro_scope again and then flip it,
                // leaving identifiers introduced by the macro unmarked by macro_scope,
                // while identifiers from arguments (which had use_scope added) remain distinguishable.
                let hygienic_body = ctx.apply_hygiene((*body).clone(), &Shrubbery::Sequence(vec![], span));

                // Step 5: Merge the macro's hygiene env into our current one for resolution
                let child_hygiene = self.hygiene.child();
                let saved_hygiene = std::mem::replace(&mut self.hygiene, child_hygiene);
                self.hygiene.merge(&macro_env);

                // Step 6: Expand the hygienically-adjusted body in the new value environment
                let saved_env = std::mem::replace(&mut self.env, new_env);
                let result = self.expand(&hygienic_body);

                // Restore environments
                self.env = saved_env;
                self.hygiene = saved_hygiene;

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

    #[test]
    fn test_define_macro_creates_scope() {
        let mut expander = Expander::new();

        // Define a simple macro
        let body = Shrubbery::Literal(crate::shrubbery::Literal::Int(42), Span::new(0, 2));
        expander.define_macro("test_macro".to_string(), vec!["x".to_string()], body);

        // Verify the macro was registered
        assert!(expander.macros.contains_key("test_macro"));

        // Verify the macro has a scope assigned
        if let Some(ExpandFunction::Macro { macro_scope, .. }) = expander.macros.get("test_macro") {
            assert!(macro_scope.id() < expander.next_scope_id);
        } else {
            panic!("Expected Macro");
        }
    }

    #[test]
    fn test_expander_fresh_scope_increments() {
        let mut expander = Expander::new();

        let s1 = expander.fresh_scope();
        let s2 = expander.fresh_scope();
        let s3 = expander.fresh_scope();

        // Each scope should have a unique, incrementing ID
        assert_eq!(s1.id(), 0);
        assert_eq!(s2.id(), 1);
        assert_eq!(s3.id(), 2);
    }

    #[test]
    fn test_symbol_registration() {
        let mut expander = Expander::new();

        expander.register_symbol(42, "my_var".to_string());
        assert_eq!(expander.symbols.get(&42), Some(&"my_var".to_string()));

        // Set multiple symbols at once
        let mut symbols = std::collections::HashMap::new();
        symbols.insert(1, "x".to_string());
        symbols.insert(2, "y".to_string());
        expander.set_symbols(symbols);

        assert_eq!(expander.symbols.get(&1), Some(&"x".to_string()));
        assert_eq!(expander.symbols.get(&2), Some(&"y".to_string()));
    }
}
