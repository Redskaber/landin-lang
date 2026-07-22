//! Stage 5.2: TraitResolver integration tests
//!
//! Tests that TraitResolver is correctly integrated into the driver pipeline
//! and accessible from CompileResult.

use landin_compiler::compile;

#[test]
fn test_trait_resolver_in_compile_result() {
    // Stage 5.2: Verify that CompileResult contains a populated TraitResolver.
    let result =
        compile("trait Foo { fn bar(); } struct S; impl Foo for S { fn bar() {} } fn main() {}");
    assert_eq!(
        result.trait_resolver.trait_count(),
        1,
        "should have 1 trait"
    );
    assert_eq!(result.trait_resolver.impl_count(), 1, "should have 1 impl");
}

#[test]
fn test_trait_resolver_empty_for_no_traits() {
    // Stage 5.2: Verify TraitResolver is empty when no traits are defined.
    let result = compile("fn main() {}");
    assert_eq!(
        result.trait_resolver.trait_count(),
        0,
        "should have 0 traits"
    );
    assert_eq!(result.trait_resolver.impl_count(), 0, "should have 0 impls");
}
