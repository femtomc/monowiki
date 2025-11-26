use crate::error::{MrlError, Result, Span};
use crate::shrubbery::{Scope, ScopeSet, Shrubbery, Symbol};
use std::collections::HashMap;

/// Binding information
#[derive(Debug, Clone)]
pub struct Binding {
    pub symbol: Symbol,
    pub scopes: ScopeSet,
    pub span: Span,
}

impl Binding {
    pub fn new(symbol: Symbol, scopes: ScopeSet, span: Span) -> Self {
        Self {
            symbol,
            scopes,
            span,
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
}

impl MacroContext {
    pub fn new(macro_scope: Scope, use_scope: Scope, env: HygieneEnv) -> Self {
        Self {
            macro_scope,
            use_scope,
            env,
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
}
