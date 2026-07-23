//! Stage 5.20: Trait impl validation report tests
//!
//! Tests `validate_impls()`, `impls_are_valid()`, and
//! `all_impls_complete()` for single-pass validation of all impls.
//!
//! Per §16: tests use the `compile()` public API.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::compile;

/// `validate_impls` should return valid report when no issues.
#[test]
fn test_validate_impls_valid() {
    let result =
        compile("trait Foo { fn bar(); } struct S; impl Foo for S { fn bar() {} } fn main() {}");
    let report = result.trait_resolver.validate_impls();
    assert!(report.is_valid, "should be valid");
    assert!(
        report.coherence_errors.is_empty(),
        "should have no coherence errors"
    );
    assert!(
        report.incomplete_impls.is_empty(),
        "should have no incomplete impls"
    );
}

/// `validate_impls` should detect coherence errors.
#[test]
fn test_validate_impls_coherence_error() {
    let result = compile("trait Foo {} struct S; impl Foo for S {} impl Foo for S {} fn main() {}");
    let report = result.trait_resolver.validate_impls();
    assert!(!report.is_valid, "should be invalid (coherence error)");
    assert_eq!(
        report.coherence_errors.len(),
        1,
        "should have 1 coherence error"
    );
}

/// `validate_impls` should detect incomplete impls.
#[test]
fn test_validate_impls_incomplete() {
    let result = compile(
        "trait Foo { fn bar(); fn baz(); } struct S; impl Foo for S { fn bar() {} } fn main() {}",
    );
    let report = result.trait_resolver.validate_impls();
    assert!(!report.is_valid, "should be invalid (incomplete impl)");
    assert_eq!(
        report.incomplete_impls.len(),
        1,
        "should have 1 incomplete impl"
    );
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");
    let baz_spur = result.interner.get("baz").expect("baz interned");
    let inc = &report.incomplete_impls[0];
    assert_eq!(inc.trait_name, foo_spur);
    assert_eq!(inc.self_ty_name, s_spur);
    assert!(inc.missing_methods.contains(&baz_spur));
}

/// `impls_are_valid` should return true when all valid.
#[test]
fn test_impls_are_valid_true() {
    let result =
        compile("trait Foo { fn bar(); } struct S; impl Foo for S { fn bar() {} } fn main() {}");
    assert!(result.trait_resolver.impls_are_valid(), "should be valid");
}

/// `impls_are_valid` should return false with coherence error.
#[test]
fn test_impls_are_valid_false_coherence() {
    let result = compile("trait Foo {} struct S; impl Foo for S {} impl Foo for S {} fn main() {}");
    assert!(
        !result.trait_resolver.impls_are_valid(),
        "should be invalid (coherence)"
    );
}

/// `impls_are_valid` should return false with incomplete impl.
#[test]
fn test_impls_are_valid_false_incomplete() {
    let result = compile(
        "trait Foo { fn bar(); fn baz(); } struct S; impl Foo for S { fn bar() {} } fn main() {}",
    );
    assert!(
        !result.trait_resolver.impls_are_valid(),
        "should be invalid (incomplete)"
    );
}

/// `all_impls_complete` should return true when all complete.
#[test]
fn test_all_impls_complete_true() {
    let result =
        compile("trait Foo { fn bar(); } struct S; impl Foo for S { fn bar() {} } fn main() {}");
    assert!(
        result.trait_resolver.all_impls_complete(),
        "all impls should be complete"
    );
}

/// `all_impls_complete` should return false when incomplete.
#[test]
fn test_all_impls_complete_false() {
    let result = compile(
        "trait Foo { fn bar(); fn baz(); } struct S; impl Foo for S { fn bar() {} } fn main() {}",
    );
    assert!(
        !result.trait_resolver.all_impls_complete(),
        "should be incomplete"
    );
}

/// No impls at all → valid + complete.
#[test]
fn test_validate_no_impls() {
    let result = compile("trait Foo { fn bar(); } fn main() {}");
    let report = result.trait_resolver.validate_impls();
    assert!(report.is_valid, "no impls should be valid");
    assert!(result.trait_resolver.all_impls_complete());
    assert!(result.trait_resolver.impls_are_valid());
}
