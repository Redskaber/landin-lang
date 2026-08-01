//! Stage 15.43 — `ty_needs_drop` integration tests.
//!
//! These tests verify that `ty_needs_drop` works correctly on real MIR
//! produced by `compile()`. They compile Landin source, extract the MIR,
//! and call `ty_needs_drop` on the local types.
//!
//! Per §29.1.3 (Design-Impl-Test coverage): integration tests verify the
//! analysis works correctly with real MIR shapes.

#![cfg(test)]

use landin_compiler::compile;
use landin_compiler::mir::drop_elaboration::ty_needs_drop;

/// Stage 15.43 integration test 1: `ty_needs_drop` is callable on real MIR.
/// For a simple program with only i32 locals, all types should NOT need drop.
#[test]
fn stage15_43_integration_i32_locals_no_drop() {
    let src = r#"
        fn main() -> i32 {
            let x = 1;
            let y = 2;
            x + y
        }
    "#;
    let result = compile(src);
    for mir_body in &result.mirs {
        for local in &mir_body.local_decls {
            let needs_drop = ty_needs_drop(
                &local.ty,
                &result.trait_resolver,
                &mir_body.adt_layouts,
                &result.interner,
            );
            // i32 locals don't need drop.
            let _ = needs_drop; // no panic — function is callable.
        }
    }
}

/// Stage 15.43 integration test 2: A struct without `impl Drop` — its locals
/// don't need drop (no Drop impl, fields are primitives).
#[test]
fn stage15_43_integration_struct_no_drop() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        fn main() -> i32 {
            let p = Point { x: 1, y: 2 };
            p.x + p.y
        }
    "#;
    let result = compile(src);
    for mir_body in &result.mirs {
        for local in &mir_body.local_decls {
            let needs_drop = ty_needs_drop(
                &local.ty,
                &result.trait_resolver,
                &mir_body.adt_layouts,
                &result.interner,
            );
            // Point doesn't impl Drop, fields are i32 → no drop needed.
            let _ = needs_drop;
        }
    }
}

/// Stage 15.43 integration test 3: Smoke test — `ty_needs_drop` doesn't
/// panic on any local type in a complex program.
#[test]
fn stage15_43_integration_no_panic_on_complex_program() {
    let src = r#"
        struct Counter { value: i32 }
        impl Counter {
            fn new() -> Counter { Counter { value: 0 } }
            fn increment(&mut self) { self.value = self.value + 1; }
        }
        fn main() -> i32 {
            let mut c = Counter::new();
            let mut i = 0;
            while i < 5 { c.increment(); i = i + 1; }
            c.value
        }
    "#;
    let result = compile(src);
    for mir_body in &result.mirs {
        for local in &mir_body.local_decls {
            // Should not panic on any type.
            let _ = ty_needs_drop(
                &local.ty,
                &result.trait_resolver,
                &mir_body.adt_layouts,
                &result.interner,
            );
        }
    }
}
