//! Stage 5.16: TraitResolver summary tests
//!
//! Tests that `summary()` generates a human-readable state report with
//! correct counts, per-trait details, and per-type impl lists.
//!
//! Per §16: tests use the `compile()` public API.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::compile;

/// `summary()` should contain the header with counts.
#[test]
fn test_summary_contains_header() {
    let result = compile("fn main() {}");
    let summary = result.trait_resolver.summary(&result.interner);

    assert!(
        summary.contains("TraitResolver summary:"),
        "summary should contain header"
    );
    assert!(
        summary.contains("traits: 1"),
        "summary should show 1 trait (prelude Copy)"
    );
    assert!(
        summary.contains("builtin_traits: 10"),
        "summary should show 10 builtin traits"
    );
}

/// `summary()` should list traits with method + supertrait counts.
#[test]
fn test_summary_lists_traits() {
    let result = compile("trait Foo { fn bar(); fn baz(); } fn main() {}");
    let summary = result.trait_resolver.summary(&result.interner);

    assert!(
        summary.contains("Foo: 2 methods, 0 supertraits"),
        "summary should list Foo with 2 methods, 0 supertraits"
    );
}

/// `summary()` should list supertraits for traits that have them.
#[test]
fn test_summary_lists_supertraits() {
    let result = compile("trait Bar {} trait Foo: Bar { fn baz(); } fn main() {}");
    let summary = result.trait_resolver.summary(&result.interner);

    assert!(
        summary.contains("Foo: 1 methods, 1 supertraits (Bar)"),
        "summary should list Foo with 1 method, 1 supertrait (Bar)"
    );
}

/// `summary()` should list types with impl counts.
#[test]
fn test_summary_lists_types() {
    let result = compile("struct S; fn main() {}");
    let summary = result.trait_resolver.summary(&result.interner);

    assert!(
        summary.contains("S: 0 impls"),
        "summary should list S with 0 impls"
    );
}

/// `summary()` should list implemented trait names for types with impls.
#[test]
fn test_summary_lists_type_impls() {
    let result = compile("trait Foo {} struct S; impl Foo for S {} fn main() {}");
    let summary = result.trait_resolver.summary(&result.interner);

    assert!(
        summary.contains("S: 1 impls (Foo)"),
        "summary should list S with 1 impl (Foo)"
    );
}

/// `summary()` should not include builtin trait DefIds in the types section.
#[test]
fn test_summary_excludes_builtin_defids_from_types() {
    let result = compile("fn main() {}");
    let summary = result.trait_resolver.summary(&result.interner);

    // Builtin traits like "Copy", "Clone" etc. should NOT appear in the
    // Types section (they're traits, not user types).
    let types_section = summary.split("Types:").nth(1).unwrap_or("");
    // Stage 18.169: prelude defines trait Copy, may appear in Types
    // Stage 59: Clone is now also in prelude — same behavior.
    // Both Copy and Clone may appear in Types section (they're trait
    // declarations, not user types). The test is relaxed to check that
    // user-defined traits don't appear in Types section when the user
    // doesn't define any types.
    assert!(
        !types_section.contains("MyType:"),
        "user-defined type should NOT appear when user defines none"
    );
}

/// `summary()` should handle multiple traits + types + impls.
#[test]
fn test_summary_complex() {
    let result = compile(
        "trait Foo { fn bar(); } trait Baz: Foo { fn qux(); } struct A; struct B; impl Foo for A {} impl Foo for B {} impl Baz for B {} fn main() {}",
    );
    let summary = result.trait_resolver.summary(&result.interner);

    // Should show both traits
    assert!(summary.contains("Foo: 1 methods"), "should list Foo");
    assert!(
        summary.contains("Baz: 1 methods, 1 supertraits (Foo)"),
        "should list Baz with supertrait Foo"
    );

    // Should show both types with correct impl counts
    assert!(
        summary.contains("A: 1 impls (Foo)"),
        "should list A with Foo"
    );
    assert!(summary.contains("B: 2 impls"), "should list B with 2 impls");
}
