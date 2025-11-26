use crate::error::{MrlError, Result, Span};
use crate::shrubbery::{Scope, ScopeSet, Shrubbery, Symbol};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Global counter for generating unique space IDs
static SPACE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A Space represents a namespace context for identifiers.
///
/// Spaces allow the same identifier name to coexist in different contexts:
/// - "expr" space for expression-position identifiers
/// - "bind" space for binding-position identifiers
/// - "type" space for type-position identifiers
/// - "defn" space for definition forms
///
/// Each space has an associated scope that gets added to identifiers
/// when they're introduced into that space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Space {
    id: u64,
    scope: Scope,
}

impl Space {
    /// Create a new space with a unique ID and scope
    pub fn new() -> Self {
        let id = SPACE_COUNTER.fetch_add(1, Ordering::SeqCst);
        Self {
            id,
            scope: Scope::new(id),
        }
    }

    /// Create a space with a specific ID (for testing/interning)
    pub fn with_id(id: u64) -> Self {
        Self {
            id,
            scope: Scope::new(id),
        }
    }

    /// Get the space's unique ID
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Get the space's scope
    pub fn scope(&self) -> Scope {
        self.scope
    }

    /// Introduce syntax into this space (add the space's scope)
    pub fn introduce(&self, mut shrub: Shrubbery) -> Shrubbery {
        shrub.add_scope(self.scope);
        shrub
    }

    /// Remove syntax from this space (flip the space's scope)
    pub fn remove(&self, mut shrub: Shrubbery) -> Shrubbery {
        shrub.flip_scope(self.scope);
        shrub
    }
}

impl Default for Space {
    fn default() -> Self {
        Self::new()
    }
}

/// Registry of interned spaces for different contexts
#[derive(Debug, Clone)]
pub struct SpaceRegistry {
    /// Expression space - for identifiers in expression position
    pub expr: Space,
    /// Binding space - for identifiers being bound (def, let, fn params)
    pub bind: Space,
    /// Type space - for type annotations and references
    pub ty: Space,
    /// Definition space - for top-level forms (def, macro, etc.)
    pub defn: Space,
    /// Custom spaces by name
    named: HashMap<String, Space>,
}

impl SpaceRegistry {
    pub fn new() -> Self {
        Self {
            expr: Space::new(),
            bind: Space::new(),
            ty: Space::new(),
            defn: Space::new(),
            named: HashMap::new(),
        }
    }

    /// Get or create a named space
    pub fn get_or_create(&mut self, name: &str) -> Space {
        if let Some(space) = self.named.get(name) {
            *space
        } else {
            let space = Space::new();
            self.named.insert(name.to_string(), space);
            space
        }
    }

    /// Get a named space if it exists
    pub fn get(&self, name: &str) -> Option<Space> {
        self.named.get(name).copied()
    }

    /// Introduce syntax into the expression space
    pub fn in_expr(&self, shrub: Shrubbery) -> Shrubbery {
        self.expr.introduce(shrub)
    }

    /// Introduce syntax into the binding space
    pub fn in_bind(&self, shrub: Shrubbery) -> Shrubbery {
        self.bind.introduce(shrub)
    }

    /// Introduce syntax into the type space
    pub fn in_type(&self, shrub: Shrubbery) -> Shrubbery {
        self.ty.introduce(shrub)
    }

    /// Introduce syntax into the definition space
    pub fn in_defn(&self, shrub: Shrubbery) -> Shrubbery {
        self.defn.introduce(shrub)
    }
}

impl Default for SpaceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Binding information
#[derive(Debug, Clone)]
pub struct Binding {
    pub symbol: Symbol,
    pub scopes: ScopeSet,
    pub span: Span,
    /// The space this binding exists in (if any)
    pub space: Option<Space>,
}

impl Binding {
    pub fn new(symbol: Symbol, scopes: ScopeSet, span: Span) -> Self {
        Self {
            symbol,
            scopes,
            span,
            space: None,
        }
    }

    /// Create a binding in a specific space
    pub fn in_space(symbol: Symbol, scopes: ScopeSet, span: Span, space: Space) -> Self {
        // Add the space's scope to the binding's scopes
        let mut scopes_with_space = scopes;
        scopes_with_space.add(space.scope());
        Self {
            symbol,
            scopes: scopes_with_space,
            span,
            space: Some(space),
        }
    }
}

/// Environment for hygiene resolution
#[derive(Debug, Clone)]
pub struct HygieneEnv {
    /// Bindings: symbol -> list of bindings with scope sets
    bindings: HashMap<Symbol, Vec<Binding>>,
}

impl HygieneEnv {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    /// Add a binding to the environment
    pub fn add_binding(&mut self, binding: Binding) {
        self.bindings
            .entry(binding.symbol)
            .or_insert_with(Vec::new)
            .push(binding);
    }

    /// Resolve an identifier to its binding
    ///
    /// Resolution rule: find the binding whose scope set is a subset of the
    /// identifier's scope set, with the largest such set (most specific binding).
    pub fn resolve(&self, symbol: Symbol, use_scopes: &ScopeSet) -> Option<&Binding> {
        let candidates = self.bindings.get(&symbol)?;

        let mut best: Option<&Binding> = None;
        let mut best_size = 0;

        for binding in candidates {
            if binding.scopes.is_subset(use_scopes) {
                let size = binding.scopes.len();
                if size > best_size {
                    best = Some(binding);
                    best_size = size;
                }
            }
        }

        best
    }

    /// Check if an identifier is bound
    pub fn is_bound(&self, symbol: Symbol, use_scopes: &ScopeSet) -> bool {
        self.resolve(symbol, use_scopes).is_some()
    }

    /// Create a child environment (for nested scopes)
    pub fn child(&self) -> Self {
        Self {
            bindings: self.bindings.clone(),
        }
    }

    /// Merge another environment into this one
    pub fn merge(&mut self, other: &HygieneEnv) {
        for (symbol, bindings) in &other.bindings {
            self.bindings
                .entry(*symbol)
                .or_insert_with(Vec::new)
                .extend(bindings.clone());
        }
    }
}

impl Default for HygieneEnv {
    fn default() -> Self {
        Self::new()
    }
}

/// Context for macro expansion
#[derive(Debug, Clone)]
pub struct MacroContext {
    /// The macro's introduction scope
    pub macro_scope: Scope,
    /// The use-site scope
    pub use_scope: Scope,
    /// The hygiene environment
    pub env: HygieneEnv,
    /// Space registry for namespace-aware hygiene
    pub spaces: SpaceRegistry,
}

impl MacroContext {
    pub fn new(macro_scope: Scope, use_scope: Scope, env: HygieneEnv) -> Self {
        Self {
            macro_scope,
            use_scope,
            env,
            spaces: SpaceRegistry::new(),
        }
    }

    /// Create a macro context with a specific space registry
    pub fn with_spaces(
        macro_scope: Scope,
        use_scope: Scope,
        env: HygieneEnv,
        spaces: SpaceRegistry,
    ) -> Self {
        Self {
            macro_scope,
            use_scope,
            env,
            spaces,
        }
    }

    /// Apply hygiene to macro output
    ///
    /// This implements the "scope flipping" algorithm:
    /// 1. Add the macro scope to all identifiers in the macro body
    /// 2. Add the use-site scope to all identifiers from the macro arguments
    /// 3. Flip the macro scope in the final output
    pub fn apply_hygiene(&self, mut output: Shrubbery, _input: &Shrubbery) -> Shrubbery {
        // Step 1: Add macro scope to output
        output.add_scope(self.macro_scope);

        // Step 2: Add use-site scope to input references in output
        // (This is simplified - full implementation would track which parts came from input)

        // Step 3: Flip macro scope
        output.flip_scope(self.macro_scope);

        output
    }

    /// Mark an identifier as hygiene-broken (explicitly capture from use-site)
    pub fn break_hygiene(&self, mut ident: Shrubbery) -> Shrubbery {
        // Remove the macro scope to allow capture
        ident.flip_scope(self.macro_scope);
        ident
    }

    /// Syntax-local-introduce: flip the macro introduction scope
    ///
    /// This is the Rhombus-style scope flipping operation used to:
    /// - Make macro-introduced identifiers visible at use-site (flip adds scope)
    /// - Make use-site identifiers visible to macro internals (flip removes scope)
    pub fn syntax_local_introduce(&self, mut syntax: Shrubbery) -> Shrubbery {
        syntax.flip_scope(self.macro_scope);
        syntax
    }

    /// Introduce syntax into the expression space
    pub fn in_expr(&self, shrub: Shrubbery) -> Shrubbery {
        self.spaces.in_expr(shrub)
    }

    /// Introduce syntax into the binding space
    pub fn in_bind(&self, shrub: Shrubbery) -> Shrubbery {
        self.spaces.in_bind(shrub)
    }

    /// Introduce syntax into the type space
    pub fn in_type(&self, shrub: Shrubbery) -> Shrubbery {
        self.spaces.in_type(shrub)
    }

    /// Introduce syntax into the definition space
    pub fn in_defn(&self, shrub: Shrubbery) -> Shrubbery {
        self.spaces.in_defn(shrub)
    }
}

/// Hygiene checker
pub struct HygieneChecker {
    env: HygieneEnv,
}

impl HygieneChecker {
    pub fn new(env: HygieneEnv) -> Self {
        Self { env }
    }

    /// Check that all identifiers in the shrubbery are bound
    pub fn check(&self, shrub: &Shrubbery) -> Result<()> {
        match shrub {
            Shrubbery::Identifier(symbol, scopes, span) => {
                if !self.env.is_bound(*symbol, scopes) {
                    return Err(MrlError::HygieneError {
                        span: *span,
                        message: format!("Unbound identifier: {:?}", symbol),
                    });
                }
            }
            Shrubbery::Parens(items, _)
            | Shrubbery::Brackets(items, _)
            | Shrubbery::Braces(items, _)
            | Shrubbery::ContentBlock(items, _)
            | Shrubbery::Sequence(items, _) => {
                for item in items {
                    self.check(item)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Collect all bindings introduced in a shrubbery (e.g., in a `def` form)
    pub fn collect_bindings(&mut self, shrub: &Shrubbery, scope: Scope) {
        match shrub {
            Shrubbery::Identifier(symbol, scopes, span) => {
                let mut binding_scopes = scopes.clone();
                binding_scopes.add(scope);
                self.env
                    .add_binding(Binding::new(*symbol, binding_scopes, *span));
            }
            Shrubbery::Parens(items, _)
            | Shrubbery::Brackets(items, _)
            | Shrubbery::Braces(items, _)
            | Shrubbery::ContentBlock(items, _)
            | Shrubbery::Sequence(items, _) => {
                for item in items {
                    self.collect_bindings(item, scope);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_binding(id: u64, scopes: Vec<u64>, span: Span) -> Binding {
        let symbol = Symbol::new(id);
        let scope_set: ScopeSet = scopes.into_iter().map(Scope::new).collect();
        Binding::new(symbol, scope_set, span)
    }

    #[test]
    fn test_binding_resolution() {
        let mut env = HygieneEnv::new();

        // Add binding for symbol 0 with scopes {1, 2}
        let binding1 = make_binding(0, vec![1, 2], Span::new(0, 3));
        env.add_binding(binding1.clone());

        // Add binding for symbol 0 with scopes {1}
        let binding2 = make_binding(0, vec![1], Span::new(4, 7));
        env.add_binding(binding2.clone());

        // Resolve with scopes {1, 2, 3}
        let use_scopes: ScopeSet = vec![1, 2, 3].into_iter().map(Scope::new).collect();
        let resolved = env.resolve(Symbol::new(0), &use_scopes);

        // Should resolve to binding1 (most specific)
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().scopes.len(), 2);
    }

    #[test]
    fn test_binding_not_found() {
        let mut env = HygieneEnv::new();

        // Add binding for symbol 0 with scopes {1, 2}
        let binding = make_binding(0, vec![1, 2], Span::new(0, 3));
        env.add_binding(binding);

        // Try to resolve with scopes {3}
        let use_scopes: ScopeSet = vec![3].into_iter().map(Scope::new).collect();
        let resolved = env.resolve(Symbol::new(0), &use_scopes);

        // Should not resolve (scope set is not a superset)
        assert!(resolved.is_none());
    }

    #[test]
    fn test_macro_context_hygiene() {
        let macro_scope = Scope::new(1);
        let use_scope = Scope::new(2);
        let env = HygieneEnv::new();
        let ctx = MacroContext::new(macro_scope, use_scope, env);

        let ident = Shrubbery::Identifier(Symbol::new(0), ScopeSet::new(), Span::new(0, 3));

        // Apply hygiene
        let output = ctx.apply_hygiene(ident.clone(), &ident);

        // The macro scope should be flipped (not present in output)
        if let Shrubbery::Identifier(_, scopes, _) = output {
            assert!(!scopes.contains(&macro_scope));
        } else {
            panic!("Expected identifier");
        }
    }

    #[test]
    fn test_hygiene_checker() {
        let mut env = HygieneEnv::new();
        let binding = make_binding(0, vec![1], Span::new(0, 3));
        env.add_binding(binding);

        let checker = HygieneChecker::new(env);

        // Check bound identifier
        let bound_ident = Shrubbery::Identifier(
            Symbol::new(0),
            vec![1].into_iter().map(Scope::new).collect(),
            Span::new(4, 7),
        );
        assert!(checker.check(&bound_ident).is_ok());

        // Check unbound identifier
        let unbound_ident = Shrubbery::Identifier(
            Symbol::new(1),
            vec![1].into_iter().map(Scope::new).collect(),
            Span::new(8, 11),
        );
        assert!(checker.check(&unbound_ident).is_err());
    }

    #[test]
    fn test_space_creation() {
        let space1 = Space::new();
        let space2 = Space::new();

        // Spaces should have unique IDs
        assert_ne!(space1.id(), space2.id());

        // Spaces should have different scopes
        assert_ne!(space1.scope(), space2.scope());
    }

    #[test]
    fn test_space_introduce_and_remove() {
        let space = Space::with_id(100);

        let ident = Shrubbery::Identifier(
            Symbol::new(0),
            ScopeSet::new(),
            Span::new(0, 3),
        );

        // Introduce adds the space's scope
        let introduced = space.introduce(ident.clone());
        if let Shrubbery::Identifier(_, scopes, _) = &introduced {
            assert!(scopes.contains(&space.scope()));
        } else {
            panic!("Expected identifier");
        }

        // Remove flips the scope (removes it since it was added)
        let removed = space.remove(introduced);
        if let Shrubbery::Identifier(_, scopes, _) = removed {
            assert!(!scopes.contains(&space.scope()));
        } else {
            panic!("Expected identifier");
        }
    }

    #[test]
    fn test_space_registry() {
        let registry = SpaceRegistry::new();

        // Built-in spaces should exist and be different
        assert_ne!(registry.expr.id(), registry.bind.id());
        assert_ne!(registry.bind.id(), registry.ty.id());
        assert_ne!(registry.ty.id(), registry.defn.id());
    }

    #[test]
    fn test_space_registry_named() {
        let mut registry = SpaceRegistry::new();

        // Create a custom space
        let custom = registry.get_or_create("custom");

        // Getting it again should return the same space
        let custom2 = registry.get_or_create("custom");
        assert_eq!(custom.id(), custom2.id());

        // Lookup should work
        assert!(registry.get("custom").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_binding_in_space() {
        let space = Space::with_id(50);
        let symbol = Symbol::new(0);
        let base_scopes: ScopeSet = vec![1, 2].into_iter().map(Scope::new).collect();

        let binding = Binding::in_space(symbol, base_scopes, Span::new(0, 3), space);

        // Binding should have the space's scope
        assert!(binding.scopes.contains(&space.scope()));
        assert!(binding.space.is_some());
        assert_eq!(binding.space.unwrap().id(), space.id());
    }

    #[test]
    fn test_different_spaces_different_bindings() {
        let mut env = HygieneEnv::new();
        let spaces = SpaceRegistry::new();

        let symbol = Symbol::new(0);
        let base_scopes: ScopeSet = vec![1].into_iter().map(Scope::new).collect();

        // Create binding in expr space
        let expr_binding = Binding::in_space(symbol, base_scopes.clone(), Span::new(0, 3), spaces.expr);
        env.add_binding(expr_binding);

        // Create binding in type space
        let type_binding = Binding::in_space(symbol, base_scopes.clone(), Span::new(4, 7), spaces.ty);
        env.add_binding(type_binding);

        // Lookup with expr space scope should find expr binding
        let mut expr_lookup_scopes = base_scopes.clone();
        expr_lookup_scopes.add(spaces.expr.scope());
        let resolved_expr = env.resolve(symbol, &expr_lookup_scopes);
        assert!(resolved_expr.is_some());
        assert!(resolved_expr.unwrap().scopes.contains(&spaces.expr.scope()));

        // Lookup with type space scope should find type binding
        let mut type_lookup_scopes = base_scopes.clone();
        type_lookup_scopes.add(spaces.ty.scope());
        let resolved_type = env.resolve(symbol, &type_lookup_scopes);
        assert!(resolved_type.is_some());
        assert!(resolved_type.unwrap().scopes.contains(&spaces.ty.scope()));
    }

    #[test]
    fn test_syntax_local_introduce() {
        let macro_scope = Scope::new(10);
        let use_scope = Scope::new(20);
        let env = HygieneEnv::new();
        let ctx = MacroContext::new(macro_scope, use_scope, env);

        // Create an identifier without macro scope
        let ident = Shrubbery::Identifier(Symbol::new(0), ScopeSet::new(), Span::new(0, 3));

        // First syntax_local_introduce should add the macro scope
        let flipped = ctx.syntax_local_introduce(ident.clone());
        if let Shrubbery::Identifier(_, scopes, _) = &flipped {
            assert!(scopes.contains(&macro_scope));
        } else {
            panic!("Expected identifier");
        }

        // Second syntax_local_introduce should remove it (flip again)
        let double_flipped = ctx.syntax_local_introduce(flipped);
        if let Shrubbery::Identifier(_, scopes, _) = double_flipped {
            assert!(!scopes.contains(&macro_scope));
        } else {
            panic!("Expected identifier");
        }
    }
}
