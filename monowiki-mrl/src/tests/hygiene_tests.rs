use crate::error::Span;
use crate::hygiene::{Binding, HygieneChecker, HygieneEnv, MacroContext};
use crate::shrubbery::{Scope, ScopeSet, Shrubbery, Symbol};

fn make_binding(symbol_id: u64, scope_ids: Vec<u64>) -> Binding {
    let symbol = Symbol::new(symbol_id);
    let scopes: ScopeSet = scope_ids.into_iter().map(Scope::new).collect();
    Binding::new(symbol, scopes, Span::new(0, 3))
}

#[test]
fn test_hygiene_env_binding() {
    let mut env = HygieneEnv::new();
    let binding = make_binding(0, vec![1, 2]);

    env.add_binding(binding.clone());

    let use_scopes: ScopeSet = vec![1, 2, 3].into_iter().map(Scope::new).collect();
    assert!(env.is_bound(Symbol::new(0), &use_scopes));
}

#[test]
fn test_hygiene_env_not_bound() {
    let mut env = HygieneEnv::new();
    let binding = make_binding(0, vec![1, 2]);

    env.add_binding(binding);

    // Scopes {3} are not a superset of {1, 2}
    let use_scopes: ScopeSet = vec![3].into_iter().map(Scope::new).collect();
    assert!(!env.is_bound(Symbol::new(0), &use_scopes));
}

#[test]
fn test_hygiene_env_most_specific() {
    let mut env = HygieneEnv::new();

    // Add two bindings for the same symbol with different scopes
    let binding1 = make_binding(0, vec![1]);
    let binding2 = make_binding(0, vec![1, 2]);

    env.add_binding(binding1);
    env.add_binding(binding2);

    // Should resolve to the most specific (largest subset)
    let use_scopes: ScopeSet = vec![1, 2, 3].into_iter().map(Scope::new).collect();
    let resolved = env.resolve(Symbol::new(0), &use_scopes);

    assert!(resolved.is_some());
    assert_eq!(resolved.unwrap().scopes.len(), 2);
}

#[test]
fn test_hygiene_env_child() {
    let mut env = HygieneEnv::new();
    let binding = make_binding(0, vec![1]);

    env.add_binding(binding);

    let child = env.child();
    let use_scopes: ScopeSet = vec![1, 2].into_iter().map(Scope::new).collect();
    assert!(child.is_bound(Symbol::new(0), &use_scopes));
}

#[test]
fn test_hygiene_env_merge() {
    let mut env1 = HygieneEnv::new();
    let mut env2 = HygieneEnv::new();

    env1.add_binding(make_binding(0, vec![1]));
    env2.add_binding(make_binding(1, vec![2]));

    env1.merge(&env2);

    let use_scopes1: ScopeSet = vec![1, 2].into_iter().map(Scope::new).collect();
    let use_scopes2: ScopeSet = vec![2, 3].into_iter().map(Scope::new).collect();

    assert!(env1.is_bound(Symbol::new(0), &use_scopes1));
    assert!(env1.is_bound(Symbol::new(1), &use_scopes2));
}

#[test]
fn test_macro_context_hygiene() {
    let macro_scope = Scope::new(1);
    let use_scope = Scope::new(2);
    let env = HygieneEnv::new();

    let ctx = MacroContext::new(macro_scope, use_scope, env);

    let ident = Shrubbery::Identifier(Symbol::new(0), ScopeSet::new(), Span::new(0, 3));

    let output = ctx.apply_hygiene(ident.clone(), &ident);

    // The macro scope should be flipped (not present)
    if let Shrubbery::Identifier(_, scopes, _) = output {
        assert!(!scopes.contains(&macro_scope));
    } else {
        panic!("Expected identifier");
    }
}

#[test]
fn test_macro_context_break_hygiene() {
    let macro_scope = Scope::new(1);
    let use_scope = Scope::new(2);
    let env = HygieneEnv::new();

    let ctx = MacroContext::new(macro_scope, use_scope, env);

    let ident = Shrubbery::Identifier(
        Symbol::new(0),
        ScopeSet::with_scope(macro_scope),
        Span::new(0, 3),
    );

    let broken = ctx.break_hygiene(ident);

    // The macro scope should be removed
    if let Shrubbery::Identifier(_, scopes, _) = broken {
        assert!(!scopes.contains(&macro_scope));
    } else {
        panic!("Expected identifier");
    }
}

#[test]
fn test_hygiene_checker_bound() {
    let mut env = HygieneEnv::new();
    env.add_binding(make_binding(0, vec![1]));

    let checker = HygieneChecker::new(env);

    let ident = Shrubbery::Identifier(
        Symbol::new(0),
        vec![1, 2].into_iter().map(Scope::new).collect(),
        Span::new(0, 3),
    );

    assert!(checker.check(&ident).is_ok());
}

#[test]
fn test_hygiene_checker_unbound() {
    let env = HygieneEnv::new();
    let checker = HygieneChecker::new(env);

    let ident = Shrubbery::Identifier(
        Symbol::new(0),
        vec![1].into_iter().map(Scope::new).collect(),
        Span::new(0, 3),
    );

    assert!(checker.check(&ident).is_err());
}

#[test]
fn test_hygiene_checker_nested() {
    let mut env = HygieneEnv::new();
    env.add_binding(make_binding(0, vec![1]));
    env.add_binding(make_binding(1, vec![1]));

    let checker = HygieneChecker::new(env);

    let scopes: ScopeSet = vec![1, 2].into_iter().map(Scope::new).collect();
    let ident1 = Shrubbery::Identifier(Symbol::new(0), scopes.clone(), Span::new(0, 3));
    let ident2 = Shrubbery::Identifier(Symbol::new(1), scopes, Span::new(4, 7));

    let parens = Shrubbery::Parens(vec![ident1, ident2], Span::new(0, 8));

    assert!(checker.check(&parens).is_ok());
}

#[test]
fn test_hygiene_checker_collect_bindings() {
    let mut checker = HygieneChecker::new(HygieneEnv::new());
    let scope = Scope::new(1);

    let ident = Shrubbery::Identifier(Symbol::new(0), ScopeSet::new(), Span::new(0, 3));

    checker.collect_bindings(&ident, scope);

    // Now the identifier should be bound with the given scope
    let use_scopes: ScopeSet = vec![1, 2].into_iter().map(Scope::new).collect();
    assert!(checker.check(&Shrubbery::Identifier(
        Symbol::new(0),
        use_scopes,
        Span::new(0, 3)
    ))
    .is_ok());
}
