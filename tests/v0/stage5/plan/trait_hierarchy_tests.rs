//! Stage 5.15: Trait hierarchy (supertraits) tests
//!
//! Tests that `trait_supertraits()`, `trait_has_supertrait()`, and
//! `supertrait_count_for_trait()` correctly query supertrait information.
//!
//! Per §16: tests use the `compile()` public API.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::compile;

/// `trait_supertraits` should return supertrait names for `trait Foo: Bar`.
#[test]
fn test_trait_supertraits() {
    let result = compile("trait Bar {} trait Foo: Bar {} fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let bar_spur = result.interner.get("Bar").expect("Bar interned");

    let supertraits = result
        .trait_resolver
        .trait_supertraits(foo_spur)
        .expect("Foo should have supertraits");
    assert_eq!(supertraits.len(), 1, "Foo should have 1 supertrait");
    assert!(
        supertraits.contains(&bar_spur),
        "Foo supertraits should contain Bar"
    );
}

/// `trait_supertraits` should return empty for trait without supertraits.
#[test]
fn test_trait_supertraits_empty() {
    let result = compile("trait Foo {} fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");

    let supertraits = result
        .trait_resolver
        .trait_supertraits(foo_spur)
        .expect("Foo should exist");
    assert!(
        supertraits.is_empty(),
        "Foo without supertraits should have empty supertraits"
    );
}

/// `trait_supertraits` should return None for unknown trait.
#[test]
fn test_trait_supertraits_unknown() {
    let result = compile("fn main() {}");
    if let Some(main_spur) = result.interner.get("main") {
        assert!(
            result.trait_resolver.trait_supertraits(main_spur).is_none(),
            "main is not a trait, should return None"
        );
    }
}

/// `trait_has_supertrait` should return true for declared supertrait.
#[test]
fn test_trait_has_supertrait_true() {
    let result = compile("trait Bar {} trait Foo: Bar {} fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let bar_spur = result.interner.get("Bar").expect("Bar interned");

    assert!(
        result
            .trait_resolver
            .trait_has_supertrait(foo_spur, bar_spur),
        "Foo should have supertrait Bar"
    );
}

/// `trait_has_supertrait` should return false for non-supertrait.
#[test]
fn test_trait_has_supertrait_false() {
    let result = compile("trait Bar {} trait Baz {} trait Foo: Bar {} fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let baz_spur = result.interner.get("Baz").expect("Baz interned");

    assert!(
        !result
            .trait_resolver
            .trait_has_supertrait(foo_spur, baz_spur),
        "Foo should NOT have supertrait Baz"
    );
}

/// `supertrait_count_for_trait` should return the supertrait count.
#[test]
fn test_supertrait_count_for_trait() {
    let result = compile("trait A {} trait B {} trait C: A + B {} fn main() {}");
    let c_spur = result.interner.get("C").expect("C interned");

    assert_eq!(
        result.trait_resolver.supertrait_count_for_trait(c_spur),
        2,
        "C should have 2 supertraits (A + B)"
    );
}

/// `supertrait_count_for_trait` should return 0 for trait without supertraits.
#[test]
fn test_supertrait_count_for_trait_zero() {
    let result = compile("trait Foo {} fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");

    assert_eq!(
        result.trait_resolver.supertrait_count_for_trait(foo_spur),
        0,
        "Foo without supertraits should have 0"
    );
}

/// Multiple supertraits should all be collected.
#[test]
fn test_multiple_supertraits() {
    let result = compile("trait A {} trait B {} trait C {} trait D: A + B + C {} fn main() {}");
    let d_spur = result.interner.get("D").expect("D interned");
    let a_spur = result.interner.get("A").expect("A interned");
    let b_spur = result.interner.get("B").expect("B interned");
    let c_spur = result.interner.get("C").expect("C interned");

    let supertraits = result
        .trait_resolver
        .trait_supertraits(d_spur)
        .expect("D should have supertraits");
    assert_eq!(supertraits.len(), 3, "D should have 3 supertraits");
    assert!(supertraits.contains(&a_spur), "should contain A");
    assert!(supertraits.contains(&b_spur), "should contain B");
    assert!(supertraits.contains(&c_spur), "should contain C");
}
