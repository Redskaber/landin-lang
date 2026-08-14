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

// === Stage 17.09: Coherence Enhancement tests ===
// Per §9.4.3: 2 positive + 6 negative tests (1:3 ratio).

/// Stage 17.09 positive 1: Single impl per (trait, type) → no error.
#[test]
fn stage17_09_single_impl_no_error() {
    let result = compile("trait Foo { fn foo(&self); } struct S; impl Foo for S { fn foo(&self) {} } fn main() { 0 }");
    assert_eq!(
        result.trait_resolver.coherence_error_count(),
        0,
        "single impl should have no coherence error"
    );
}

/// Stage 17.09 positive 2: Multiple traits on same type → no error.
#[test]
fn stage17_09_multiple_traits_same_type_no_error() {
    let result = compile("trait Foo { fn foo(&self); } trait Bar { fn bar(&self); } struct S; impl Foo for S { fn foo(&self) {} } impl Bar for S { fn bar(&self) {} } fn main() { 0 }");
    assert_eq!(
        result.trait_resolver.coherence_error_count(),
        0,
        "different traits on same type should have no coherence error"
    );
}

/// Stage 17.09 negative 1: Duplicate impl blocks → coherence error.
#[test]
fn stage17_09_duplicate_impl_error() {
    let result = compile("trait Foo { fn foo(&self); } struct S; impl Foo for S { fn foo(&self) {} } impl Foo for S { fn foo(&self) {} } fn main() { 0 }");
    assert_eq!(
        result.trait_resolver.coherence_error_count(),
        1,
        "duplicate impl should have 1 coherence error"
    );
}

/// Stage 17.09 negative 2: Multiple duplicate pairs → multiple errors.
#[test]
fn stage17_09_multiple_duplicate_pairs() {
    let result = compile("trait Foo { fn foo(&self); } trait Bar { fn bar(&self); } struct S; impl Foo for S { fn foo(&self) {} } impl Foo for S { fn foo(&self) {} } impl Bar for S { fn bar(&self) {} } impl Bar for S { fn bar(&self) {} } fn main() { 0 }");
    assert_eq!(
        result.trait_resolver.coherence_error_count(),
        2,
        "two duplicate pairs should have 2 coherence errors"
    );
}

/// Stage 17.09 negative 3: has_coherence_error returns true for duplicate.
#[test]
fn stage17_09_has_coherence_error_duplicate() {
    let result = compile("trait Foo { fn foo(&self); } struct S; impl Foo for S { fn foo(&self) {} } impl Foo for S { fn foo(&self) {} } fn main() { 0 }");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");
    assert!(
        result.trait_resolver.has_coherence_error(foo_spur, s_spur),
        "should detect coherence error for (Foo, S)"
    );
}

/// Stage 17.09 negative 4: validate_impls reports coherence errors.
#[test]
fn stage17_09_validate_impls_reports_coherence() {
    let result = compile("trait Foo { fn foo(&self); } struct S; impl Foo for S { fn foo(&self) {} } impl Foo for S { fn foo(&self) {} } fn main() { 0 }");
    let report = result.trait_resolver.validate_impls();
    assert!(!report.is_valid, "should be invalid (coherence error)");
    assert_eq!(
        report.coherence_errors.len(),
        1,
        "should have 1 coherence error"
    );
}

/// Stage 17.09 negative 5: Coherence error in compile result trait_errors.
#[test]
fn stage17_09_compile_trait_errors_contain_coherence() {
    let result = compile("trait Foo { fn foo(&self); } struct S; impl Foo for S { fn foo(&self) {} } impl Foo for S { fn foo(&self) {} } fn main() { 0 }");
    assert!(
        !result.errors.trait_errors.is_empty(),
        "should have trait errors"
    );
    // The trait_errors should contain Coherence variant.
    let has_coherence = result
        .errors
        .trait_errors
        .iter()
        .any(|e| matches!(e, landin_compiler::TraitError::Coherence(_)));
    assert!(
        has_coherence,
        "should have Coherence variant in trait_errors"
    );
}

/// Stage 17.09 negative 6: Different types same trait → no coherence error.
#[test]
fn stage17_09_different_types_same_trait_no_error() {
    let result = compile("trait Foo { fn foo(&self); } struct A; struct B; impl Foo for A { fn foo(&self) {} } impl Foo for B { fn foo(&self) {} } fn main() { 0 }");
    assert_eq!(
        result.trait_resolver.coherence_error_count(),
        0,
        "same trait for different types should NOT be a coherence error"
    );
}
