//! Stage 5.14: Trait method query API tests
//!
//! Tests `trait_methods()`, `impl_methods_by_def_ids()`, `trait_has_method()`,
//! `traits_with_method()`, and `method_count_for_trait()`.
//!
//! Per §16: tests use the `compile()` public API.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.
//!
//! Stage 30.22: migrated from deprecated `impl_methods` (Spur-based) to
//! `impl_methods_by_def_ids` (DefId-keyed, type-safe).

use landin_compiler::compile;

/// `trait_methods` should return the methods declared in a trait.
#[test]
fn test_trait_methods() {
    let result = compile("trait Foo { fn bar(); fn baz(); } fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let bar_spur = result.interner.get("bar").expect("bar interned");
    let baz_spur = result.interner.get("baz").expect("baz interned");

    let methods = result
        .trait_resolver
        .trait_methods(foo_spur)
        .expect("Foo should have methods");
    assert_eq!(methods.len(), 2, "Foo should have 2 methods");
    assert!(methods.contains(&bar_spur), "should contain bar");
    assert!(methods.contains(&baz_spur), "should contain baz");
}

/// `trait_methods` should return None for unknown trait.
#[test]
fn test_trait_methods_unknown() {
    let result = compile("fn main() {}");
    // "main" is interned but is not a trait — trait_methods should return None
    if let Some(main_spur) = result.interner.get("main") {
        assert!(
            result.trait_resolver.trait_methods(main_spur).is_none(),
            "main is not a trait, should return None"
        );
    }
}

/// `impl_methods_by_def_ids` should return methods implemented in an impl block.
#[test]
fn test_impl_methods() {
    let result = compile(
        "trait Foo { fn bar(); fn baz(); } struct S; impl Foo for S { fn bar() {} fn baz() {} } fn main() {}",
    );
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let foo_def_id = result
        .trait_resolver
        .find_trait_def_id(foo_spur)
        .expect("Foo trait DefId");
    let s_spur = result.interner.get("S").expect("S interned");
    let s_def_id = result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == s_spur)
        .map(|(&d, _)| d)
        .expect("S type DefId");

    let methods = result
        .trait_resolver
        .impl_methods_by_def_ids(foo_def_id, s_def_id)
        .expect("impl Foo for S should have methods");
    assert_eq!(methods.len(), 2, "impl should have 2 methods");
}

/// `trait_has_method` should return true for declared methods.
#[test]
fn test_trait_has_method_true() {
    let result = compile("trait Foo { fn bar(); } fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let bar_spur = result.interner.get("bar").expect("bar interned");

    assert!(
        result.trait_resolver.trait_has_method(foo_spur, bar_spur),
        "Foo should have method bar"
    );
}

/// `trait_has_method` should return false for undeclared methods.
#[test]
fn test_trait_has_method_false() {
    let result = compile("trait Foo { fn bar(); fn baz(); } fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let bar_spur = result.interner.get("bar").expect("bar interned");
    // "baz" is declared but we check for a method that doesn't exist.
    // Use a Spur that's interned but not a method of Foo — "Foo" itself.
    assert!(
        !result.trait_resolver.trait_has_method(foo_spur, foo_spur),
        "Foo should NOT have method named 'Foo'"
    );
    // Also verify bar IS found (sanity check)
    assert!(
        result.trait_resolver.trait_has_method(foo_spur, bar_spur),
        "Foo should have method bar"
    );
}

/// `traits_with_method` should find all traits declaring a method.
#[test]
fn test_traits_with_method() {
    let result = compile("trait Foo { fn bar(); } trait Baz { fn bar(); } fn main() {}");
    let bar_spur = result.interner.get("bar").expect("bar interned");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let baz_spur = result.interner.get("Baz").expect("Baz interned");

    let traits = result.trait_resolver.traits_with_method(bar_spur);
    assert_eq!(traits.len(), 2, "2 traits should declare bar");
    assert!(traits.contains(&foo_spur), "should include Foo");
    assert!(traits.contains(&baz_spur), "should include Baz");
}

/// `method_count_for_trait` should return the method count.
#[test]
fn test_method_count_for_trait() {
    let result = compile("trait Foo { fn bar(); fn baz(); fn qux(); } fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");

    assert_eq!(
        result.trait_resolver.method_count_for_trait(foo_spur),
        3,
        "Foo should have 3 methods"
    );
}

/// `method_count_for_trait` should return 0 for unknown trait.
#[test]
fn test_method_count_for_trait_unknown() {
    let result = compile("fn main() {}");
    // Use a Spur that's not a trait (e.g. "main")
    if let Some(main_spur) = result.interner.get("main") {
        assert_eq!(
            result.trait_resolver.method_count_for_trait(main_spur),
            0,
            "non-trait should have 0 methods"
        );
    }
}
