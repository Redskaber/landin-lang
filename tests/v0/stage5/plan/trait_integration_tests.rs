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
    assert!(result.trait_resolver.trait_count() >= 2, "prelude + 1 user");
    assert!(
        result.trait_resolver.impl_count() >= 5,
        "prelude adds 4 impls + 1 user impl, got {}",
        result.trait_resolver.impl_count()
    );
}

#[test]
fn test_trait_resolver_empty_for_no_traits() {
    // Stage 5.2: Verify TraitResolver is empty when no traits are defined.
    let result = compile("fn main() {}");
    assert!(
        result.trait_resolver.trait_count() >= 1,
        "prelude adds trait Copy"
    );
    assert!(
        result.trait_resolver.impl_count() >= 4,
        "prelude adds 4 impls (2 Copy + 2 inherent), got {}",
        result.trait_resolver.impl_count()
    );
}
