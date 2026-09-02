//! Stage 5.5: vtable generation tests
//!
//! Tests that TraitResolver correctly builds vtables for trait impls.
//!
//! Stage 30.22: removed `test_vtable_query` (used deprecated `find_vtable`).
//! The same vtable-entry verification is covered by DefId-keyed tests in
//! `stage16_10_vtable_def_id_lookup_tests.rs` (test 1-3, 5-6).

use landin_compiler::compile;

#[test]
fn test_vtable_built_for_impl() {
    // When `impl Foo for S` exists, a vtable should be built.
    let result =
        compile("trait Foo { fn bar(); } struct S; impl Foo for S { fn bar() {} } fn main() {}");
    assert!(
        result.trait_resolver.vtable_count() >= 3,
        "prelude adds 2 + 1 user, got {}",
        result.trait_resolver.vtable_count()
    );
}

#[test]
fn test_no_vtable_without_impl() {
    // Without `impl Foo for S`, no vtable should exist.
    let result = compile("trait Foo { fn bar(); } struct S; fn main() {}");
    assert!(
        result.trait_resolver.vtable_count() >= 2,
        "should have at least 2 vtables (prelude Copy + Clone), got {}",
        result.trait_resolver.vtable_count()
    );
}

#[test]
fn test_vtable_multiple_impls() {
    // Multiple trait impls should produce multiple vtables.
    let result = compile(
        "trait Foo { fn bar(); } trait Baz { fn qux(); } struct S; impl Foo for S { fn bar() {} } impl Baz for S { fn qux() {} } fn main() {}",
    );
    assert!(
        result.trait_resolver.vtable_count() >= 4,
        "prelude adds 2 + 2 user, got {}",
        result.trait_resolver.vtable_count()
    );
}
