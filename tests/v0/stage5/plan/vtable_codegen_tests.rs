//! Stage 5.6: codegen vtable emission tests
//!
//! Tests that `codegen_crate` emits LLVM IR vtable globals for every
//! `(trait, type)` pair collected by `TraitResolver`. Each vtable should
//! appear as a module-level `@.vtable.<trait>.<type>` constant array of
//! opaque function pointers, one per trait method, pointing at the
//! concrete impl method symbol (`landin_<Type>_<method>`).
//!
//! Per §16: codegen reads pre-built TraitResolver data — no HIR access.
//! Per §17.3: tests live under `tests/v0/stage5/plan/` per standardized
//! test directory layout.

use landin_compiler::codegen::codegen_crate;
use landin_compiler::compile;

/// When `impl Foo for S` exists, the LLVM IR must contain a
/// `@.vtable.Foo.S` global with one `ptr @landin_S_bar` entry.
#[test]
fn test_vtable_global_emitted_for_impl() {
    let result =
        compile("trait Foo { fn bar(); } struct S; impl Foo for S { fn bar() {} } fn main() {}");
    let ir = codegen_crate(&result).expect("codegen should succeed for valid test input");

    // The vtable global should be present and named after (trait, type).
    assert!(
        ir.contains("@.vtable.Foo.S"),
        "expected `@.vtable.Foo.S` global in IR, got:\n{}",
        ir
    );

    // The vtable should reference the concrete impl method symbol.
    assert!(
        ir.contains("@landin_S_bar"),
        "expected vtable to reference `@landin_S_bar`, got:\n{}",
        ir
    );
}

/// Without `impl Foo for S`, no vtable global should be emitted.
#[test]
fn test_no_vtable_global_without_impl() {
    let result = compile("trait Foo { fn bar(); } struct S; fn main() {}");
    let ir = codegen_crate(&result).expect("codegen should succeed for valid test input");

    assert!(
        !ir.contains("@.vtable."),
        "expected no vtable globals, but found one in IR:\n{}",
        ir
    );
}

/// Multiple trait impls should produce multiple distinct vtable globals,
/// each referencing the correct impl method symbols.
#[test]
fn test_multiple_vtable_globals_emitted() {
    let result = compile(
        "trait Foo { fn bar(); } trait Baz { fn qux(); } struct S; \
         impl Foo for S { fn bar() {} } \
         impl Baz for S { fn qux() {} } \
         fn main() {}",
    );
    let ir = codegen_crate(&result).expect("codegen should succeed for valid test input");

    // Both vtables should be present.
    assert!(
        ir.contains("@.vtable.Foo.S"),
        "expected `@.vtable.Foo.S` in IR, got:\n{}",
        ir
    );
    assert!(
        ir.contains("@.vtable.Baz.S"),
        "expected `@.vtable.Baz.S` in IR, got:\n{}",
        ir
    );

    // Each vtable should reference its own method symbol.
    assert!(
        ir.contains("@landin_S_bar"),
        "expected `@landin_S_bar` reference, got:\n{}",
        ir
    );
    assert!(
        ir.contains("@landin_S_qux"),
        "expected `@landin_S_qux` reference, got:\n{}",
        ir
    );
}
