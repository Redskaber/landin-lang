//! Stage 5.18: Trait coherence checking tests
//!
//! Tests `check_coherence()`, `has_coherence_error()`, and
//! `coherence_error_count()` for detecting conflicting impls.
//!
//! Per §16: tests use the `compile()` public API.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::compile;

/// `check_coherence` should return empty Vec when no conflicts.
#[test]
fn test_check_coherence_no_conflicts() {
    let result = compile(
        "trait Foo {} trait Bar {} struct S; impl Foo for S {} impl Bar for S {} fn main() {}",
    );
    let errors = result.trait_resolver.check_coherence();
    assert!(
        errors.is_empty(),
        "should have no coherence errors when impls are for different traits"
    );
}

/// `check_coherence` should detect conflicting impls (same trait + type).
#[test]
fn test_check_coherence_detects_conflict() {
    // Note: Landin parser accepts duplicate impls; collect() stores all
    // of them. check_coherence detects the conflict post-collection.
    let result = compile("trait Foo {} struct S; impl Foo for S {} impl Foo for S {} fn main() {}");
    let errors = result.trait_resolver.check_coherence();
    assert_eq!(
        errors.len(),
        1,
        "should detect 1 coherence error (Foo for S duplicated)"
    );

    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");
    let err = &errors[0];
    assert_eq!(err.trait_name, foo_spur, "error should be for trait Foo");
    assert_eq!(err.self_ty_name, s_spur, "error should be for type S");
    assert_eq!(
        err.impl_def_ids.len(),
        2,
        "should have 2 conflicting impl DefIds"
    );
}

/// `has_coherence_error` should return true for conflicting pair.
#[test]
fn test_has_coherence_error_true() {
    let result = compile("trait Foo {} struct S; impl Foo for S {} impl Foo for S {} fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");

    assert!(
        result.trait_resolver.has_coherence_error(foo_spur, s_spur),
        "should detect coherence error for (Foo, S)"
    );
}

/// `has_coherence_error` should return false for non-conflicting pair.
#[test]
fn test_has_coherence_error_false() {
    let result = compile("trait Foo {} struct S; impl Foo for S {} fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");

    assert!(
        !result.trait_resolver.has_coherence_error(foo_spur, s_spur),
        "should NOT detect coherence error for single impl"
    );
}

/// `coherence_error_count` should return the number of conflicting pairs.
#[test]
fn test_coherence_error_count() {
    let result = compile(
        "trait Foo {} trait Bar {} struct S; impl Foo for S {} impl Foo for S {} impl Bar for S {} impl Bar for S {} fn main() {}",
    );
    assert_eq!(
        result.trait_resolver.coherence_error_count(),
        2,
        "should have 2 coherence errors (Foo+S and Bar+S)"
    );
}

/// `coherence_error_count` should return 0 when no conflicts.
#[test]
fn test_coherence_error_count_zero() {
    let result = compile("trait Foo {} struct S; impl Foo for S {} fn main() {}");
    assert_eq!(
        result.trait_resolver.coherence_error_count(),
        0,
        "should have 0 coherence errors"
    );
}

/// Multiple types with same trait should NOT trigger coherence error.
#[test]
fn test_no_conflict_different_types() {
    let result = compile(
        "trait Foo {} struct A; struct B; impl Foo for A {} impl Foo for B {} fn main() {}",
    );
    assert_eq!(
        result.trait_resolver.coherence_error_count(),
        0,
        "same trait for different types should NOT be a coherence error"
    );
}
