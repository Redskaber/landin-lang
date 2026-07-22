//! Stage 5.19: Trait impl completeness check tests
//!
//! Tests `impl_covers_trait()`, `missing_impl_methods()`, and
//! `missing_method_count()` for detecting incomplete impls.
//!
//! Per §16: tests use the `compile()` public API.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::compile;

/// `impl_covers_trait` should return true when impl has all trait methods.
#[test]
fn test_impl_covers_trait_complete() {
    let result = compile(
        "trait Foo { fn bar(); fn baz(); } struct S; impl Foo for S { fn bar() {} fn baz() {} } fn main() {}",
    );
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");

    assert!(
        result.trait_resolver.impl_covers_trait(foo_spur, s_spur),
        "impl should cover all trait methods"
    );
}

/// `impl_covers_trait` should return false when impl is missing methods.
#[test]
fn test_impl_covers_trait_incomplete() {
    let result = compile(
        "trait Foo { fn bar(); fn baz(); } struct S; impl Foo for S { fn bar() {} } fn main() {}",
    );
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");

    assert!(
        !result.trait_resolver.impl_covers_trait(foo_spur, s_spur),
        "impl missing baz should NOT cover trait"
    );
}

/// `impl_covers_trait` should return false when no impl exists.
#[test]
fn test_impl_covers_trait_no_impl() {
    let result = compile("trait Foo { fn bar(); } struct S; fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");

    assert!(
        !result.trait_resolver.impl_covers_trait(foo_spur, s_spur),
        "no impl should return false"
    );
}

/// `missing_impl_methods` should return empty Vec for complete impl.
#[test]
fn test_missing_impl_methods_empty() {
    let result = compile(
        "trait Foo { fn bar(); fn baz(); } struct S; impl Foo for S { fn bar() {} fn baz() {} } fn main() {}",
    );
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");

    let missing = result.trait_resolver.missing_impl_methods(foo_spur, s_spur);
    assert!(
        missing.is_empty(),
        "complete impl should have no missing methods"
    );
}

/// `missing_impl_methods` should return the missing method names.
#[test]
fn test_missing_impl_methods_finds_missing() {
    let result = compile(
        "trait Foo { fn bar(); fn baz(); fn qux(); } struct S; impl Foo for S { fn bar() {} } fn main() {}",
    );
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");
    let baz_spur = result.interner.get("baz").expect("baz interned");
    let qux_spur = result.interner.get("qux").expect("qux interned");

    let missing = result.trait_resolver.missing_impl_methods(foo_spur, s_spur);
    assert_eq!(missing.len(), 2, "should have 2 missing methods");
    assert!(missing.contains(&baz_spur), "should contain baz");
    assert!(missing.contains(&qux_spur), "should contain qux");
}

/// `missing_method_count` should return the correct count.
#[test]
fn test_missing_method_count() {
    let result = compile(
        "trait Foo { fn bar(); fn baz(); fn qux(); } struct S; impl Foo for S { fn bar() {} } fn main() {}",
    );
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");

    assert_eq!(
        result.trait_resolver.missing_method_count(foo_spur, s_spur),
        2,
        "should have 2 missing methods"
    );
}

/// `missing_method_count` should return 0 for complete impl.
#[test]
fn test_missing_method_count_zero() {
    let result =
        compile("trait Foo { fn bar(); } struct S; impl Foo for S { fn bar() {} } fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");

    assert_eq!(
        result.trait_resolver.missing_method_count(foo_spur, s_spur),
        0,
        "complete impl should have 0 missing"
    );
}

/// Empty trait with empty impl should be complete.
#[test]
fn test_empty_trait_empty_impl_complete() {
    let result = compile("trait Foo {} struct S; impl Foo for S {} fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");

    assert!(
        result.trait_resolver.impl_covers_trait(foo_spur, s_spur),
        "empty trait + empty impl should be complete"
    );
}
