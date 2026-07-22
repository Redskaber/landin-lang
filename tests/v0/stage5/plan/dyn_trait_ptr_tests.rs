//! Stage 5.7: `dyn Trait` fat-pointer construction tests
//!
//! Tests that `codegen_crate` emits LLVM IR `dyn Trait` fat-pointer
//! constant globals for every `(trait, type)` pair collected by
//! `TraitResolver`. Each fat pointer should appear as a module-level
//! `@.dynptr.<trait>.<type>` constant of type `{ ptr, ptr }`, with the
//! first `ptr` referencing a data global and the second `ptr` referencing
//! the vtable global emitted by Stage 5.6.
//!
//! Per §16: codegen reads pre-built TraitResolver data — no HIR access.
//! Per §17.3: tests live under `tests/v0/stage5/plan/` per standardized
//! test directory layout.

use landin_compiler::codegen::codegen_crate;
use landin_compiler::compile;

/// When `impl Foo for S` exists, the LLVM IR must contain a
/// `@.dynptr.Foo.S` global of type `{ ptr, ptr }`.
#[test]
fn test_dyn_trait_ptr_emitted_for_impl() {
    let result =
        compile("trait Foo { fn bar(); } struct S; impl Foo for S { fn bar() {} } fn main() {}");
    let ir = codegen_crate(&result);

    // The dyn fat-pointer global should be present.
    assert!(
        ir.contains("@.dynptr.Foo.S"),
        "expected `@.dynptr.Foo.S` global in IR, got:\n{}",
        ir
    );

    // It should be a { ptr, ptr } constant.
    assert!(
        ir.contains("{ ptr, ptr } { ptr @.data.S, ptr @.vtable.Foo.S }"),
        "expected `{{ ptr, ptr }} {{ ptr @.data.S, ptr @.vtable.Foo.S }}` in IR, got:\n{}",
        ir
    );
}

/// Without `impl Foo for S`, no dyn fat-pointer global should be emitted.
#[test]
fn test_no_dyn_trait_ptr_without_impl() {
    let result = compile("trait Foo { fn bar(); } struct S; fn main() {}");
    let ir = codegen_crate(&result);

    assert!(
        !ir.contains("@.dynptr."),
        "expected no dyn fat-pointer globals, but found one in IR:\n{}",
        ir
    );
}

/// Multiple trait impls should produce multiple distinct dyn fat-pointer
/// globals, each referencing the correct vtable.
#[test]
fn test_multiple_dyn_trait_ptrs_emitted() {
    let result = compile(
        "trait Foo { fn bar(); } trait Baz { fn qux(); } struct S; \
         impl Foo for S { fn bar() {} } \
         impl Baz for S { fn qux() {} } \
         fn main() {}",
    );
    let ir = codegen_crate(&result);

    // Both dyn fat pointers should be present.
    assert!(
        ir.contains("@.dynptr.Foo.S"),
        "expected `@.dynptr.Foo.S` in IR, got:\n{}",
        ir
    );
    assert!(
        ir.contains("@.dynptr.Baz.S"),
        "expected `@.dynptr.Baz.S` in IR, got:\n{}",
        ir
    );

    // Each should reference its own vtable.
    assert!(
        ir.contains("ptr @.vtable.Foo.S"),
        "expected `ptr @.vtable.Foo.S` reference, got:\n{}",
        ir
    );
    assert!(
        ir.contains("ptr @.vtable.Baz.S"),
        "expected `ptr @.vtable.Baz.S` reference, got:\n{}",
        ir
    );
}

/// The `emit_dyn_trait_ptr_type` helper should return a `{ ptr, ptr }`
/// EmitType (two opaque pointers — data + vtable).
#[test]
fn test_emit_dyn_trait_ptr_type_shape() {
    use landin_compiler::codegen::{emit_dyn_trait_ptr_type, EmitType};

    let ty = emit_dyn_trait_ptr_type();
    match ty {
        EmitType::Struct(fields) => {
            assert_eq!(
                fields.len(),
                2,
                "dyn Trait fat pointer should have exactly 2 fields (data + vtable)"
            );
            assert!(
                matches!(fields[0], EmitType::OpaquePtr),
                "first field (data) should be OpaquePtr"
            );
            assert!(
                matches!(fields[1], EmitType::OpaquePtr),
                "second field (vtable) should be OpaquePtr"
            );
        }
        other => panic!("expected EmitType::Struct, got {:?}", other),
    }
}
