//! Stage 5.22: Driver validation integration tests
//!
//! Tests that the driver correctly reports trait coherence and completeness
//! errors via `CompileErrors.trait_errors`.
//!
//! Stage 15.9: trait_errors is now `Vec<TraitError>` (was `Vec<String>`).
//! Tests use `format_with_interner` to get the human-readable message.
//!
//! Per §16: tests use the `compile()` public API.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::compile;

/// Driver should report coherence errors when duplicate impls exist.
#[test]
fn test_driver_reports_coherence_error() {
    let result = compile("trait Foo {} struct S; impl Foo for S {} impl Foo for S {} fn main() {}");
    assert!(
        !result.errors.trait_errors.is_empty(),
        "should have trait_errors for conflicting impls"
    );
    // Stage 15.9: trait_errors[0] is now TraitError, format via interner.
    let msg = result.errors.trait_errors[0].format_with_interner(&result.interner);
    assert!(
        msg.contains("conflicting implementations"),
        "error message should mention conflicting implementations, got: {}",
        msg
    );
}

/// Driver should report completeness errors when impl is missing methods.
#[test]
fn test_driver_reports_completeness_error() {
    let result = compile(
        "trait Foo { fn bar(); fn baz(); } struct S; impl Foo for S { fn bar() {} } fn main() {}",
    );
    assert!(
        !result.errors.trait_errors.is_empty(),
        "should have trait_errors for incomplete impl"
    );
    let msg = result.errors.trait_errors[0].format_with_interner(&result.interner);
    assert!(
        msg.contains("missing method"),
        "error message should mention missing method, got: {}",
        msg
    );
    assert!(
        msg.contains("baz"),
        "error message should mention the missing method name, got: {}",
        msg
    );
}

/// Driver should report no trait errors when all impls are valid.
#[test]
fn test_driver_no_trait_errors_when_valid() {
    let result =
        compile("trait Foo { fn bar(); } struct S; impl Foo for S { fn bar() {} } fn main() {}");
    assert!(
        result.errors.trait_errors.is_empty(),
        "should have no trait_errors when all impls are valid"
    );
}

/// Driver should report no trait errors when no impls exist.
#[test]
fn test_driver_no_trait_errors_no_impls() {
    let result = compile("trait Foo { fn bar(); } fn main() {}");
    assert!(
        result.errors.trait_errors.is_empty(),
        "should have no trait_errors when no impls exist"
    );
}

/// total_count should include trait_errors.
#[test]
fn test_total_count_includes_trait_errors() {
    let result = compile("trait Foo {} struct S; impl Foo for S {} impl Foo for S {} fn main() {}");
    assert!(
        result.errors.total_count() >= 1,
        "total_count should include trait errors"
    );
}

/// is_empty should return false when trait_errors exist.
#[test]
fn test_is_empty_false_with_trait_errors() {
    let result = compile("trait Foo { fn bar(); } struct S; impl Foo for S {} fn main() {}");
    assert!(
        !result.errors.is_empty(),
        "is_empty should be false when trait_errors exist"
    );
}

/// Multiple errors (coherence + completeness) should all be reported.
#[test]
fn test_multiple_trait_errors() {
    let result = compile(
        "trait Foo { fn bar(); fn baz(); } struct S; impl Foo for S { fn bar() {} } impl Foo for S { fn bar() {} } fn main() {}",
    );
    // Should have at least: 1 coherence (duplicate Foo for S) + 2 incomplete (missing baz)
    assert!(
        result.errors.trait_errors.len() >= 2,
        "should have multiple trait_errors (coherence + completeness)"
    );
}
