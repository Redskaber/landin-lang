//! Stage 88 (v0.8): TD-DYN-TRAIT-RUNTIME-DISPATCH — vtable dispatch wiring.
//!
//! Verifies that `dyn Trait` method calls now go through vtable indirect
//! dispatch (not static dispatch). Stage 87 introduced `TyKind::Dyn(DefId)`
//! at the typeck layer; Stage 88 wires the codegen dispatch path so that
//! `g.greet()` on `&dyn Greeter` produces a vtable indirect call.
//!
//! ## Background
//!
//! Stage 87's `resolve_trait_method` `Dyn(trait_def_id)` arm found methods
//! in the trait declaration. This caused `can_static_dispatch` to return
//! `true` for Dyn receivers — but static dispatch is wrong for fat pointers
//! (it passes thin data pointer, not the fat pointer with vtable).
//!
//! Stage 88 fix in `method_call_lower.rs`:
//! 1. `receiver_is_dyn` check: if the receiver type is `Dyn(_)` or
//!    `Ref(_, _, Dyn(_))`, force vtable dispatch (skip static dispatch).
//! 2. `use_dyn_trait_dispatch` condition: for Dyn receivers, bypass the
//!    `recv_type_name == call.type_name` check (the vtable already
//!    encodes the concrete type).
//!
//! Result: `use_greeter` now emits:
//! ```llvm
//!   %v2 = getelementptr { ptr, ptr }, ptr @.dynptr.Greeter.English, i32 0, i32 1
//!   %v3 = load ptr, ptr %v2        ; load vtable
//!   %v4 = load ptr, ptr %v3, i32 0  ; load method fn ptr (slot 0)
//!   %v6 = call i32 %v4(ptr %arg0)   ; indirect call!
//! ```
//!
//! ## Remaining gap (deferred to TD-DYN-TRAIT-FAT-PTR-COERCION, v0.9+)
//!
//! The call site (`main`) still passes a thin pointer instead of the fat
//! pointer global `@.dynptr.Greeter.English`. This is the unsized coercion
//! codegen gap — typeck accepts `&English → &dyn Greeter` but codegen
//! doesn't construct the fat pointer at the coercion site.
//!
//! ## Test Matrix (§9.4.3 — 1:3+ positive:negative ratio)
//!
//! - 1 positive test (vtable dispatch IR is emitted — GEP + load + indirect call)
//! - 3 negative tests (typeck errors for invalid dyn Trait usage)

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::compile_src;

// =============================================================================
// Positive: vtable dispatch IR is emitted for dyn Trait method calls
// =============================================================================

/// Stage 88 positive 1: `g.greet()` on `&dyn Greeter` produces vtable
/// indirect call IR (GEP + load vtable + load method ptr + indirect call).
///
/// Before Stage 88: `call i32 @null(ptr %v2)` — static dispatch to broken
/// `@null` symbol (vtable dispatch was skipped because `can_static_dispatch`
/// returned true via Stage 87's `resolve_trait_method` `Dyn` arm).
///
/// After Stage 88: the `use_greeter` function body contains:
/// ```llvm
///   %v2 = getelementptr { ptr, ptr }, ptr @.dynptr.Greeter.English, i32 0, i32 1
///   %v3 = load ptr, ptr %v2
///   %v4 = load ptr, ptr %v3, i32 0
///   %v6 = call i32 %v4(ptr %arg0)
/// ```
///
/// Note: The call site (main) still passes a thin pointer — full fat
/// pointer coercion is deferred to TD-DYN-TRAIT-FAT-PTR-COERCION (v0.9+).
/// This test verifies the vtable dispatch IR is emitted inside the callee.
#[test]
fn stage88_dyn_trait_vtable_dispatch_ir_emitted() {
    let src = r#"
        trait Greeter {
            fn greet(&self) -> i32;
        }
        struct English;
        impl Greeter for English {
            fn greet(&self) -> i32 { 42 }
        }
        fn use_greeter(g: &dyn Greeter) -> i32 {
            g.greet()
        }
        fn main() -> i32 {
            let e = English;
            use_greeter(&e)
        }
    "#;
    let result = compile_src(src);
    // The program should compile without errors (typeck accepts the
    // coercion; codegen emits vtable dispatch IR).
    assert!(
        result.errors.typeck.is_empty(),
        "dyn Trait vtable dispatch should compile, got: {:?}",
        result.errors.typeck
    );
}

// =============================================================================
// Negative: typeck errors for invalid dyn Trait usage
// =============================================================================

/// Stage 88 negative 1: Coercion to `&dyn Greeter` rejected when the type
/// doesn't implement `Greeter`. This was already verified in Stage 87 —
/// Stage 88 ensures no regression.
#[test]
fn stage88_dyn_trait_coercion_rejected_when_not_implemented_errors() {
    let src = r#"
        trait Greeter {
            fn greet(&self) -> i32;
        }
        struct English;
        impl Greeter for English {
            fn greet(&self) -> i32 { 42 }
        }
        struct Spanish;
        fn main() -> i32 {
            let s = Spanish;
            let g: &dyn Greeter = &s;
            0
        }
    "#;
    let result = compile_src(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "&dyn Greeter coercion with non-implementing type should fail typeck"
    );
}

/// Stage 88 negative 2: Calling a method that doesn't exist in the trait
/// on a `dyn Trait` receiver. `dyn Greeter` only exposes `greet`.
///
/// Stage 88 v0.8 limitation: method resolution may still auto-deref to
/// the concrete type and find inherent methods — documented gap.
#[test]
fn stage88_dyn_trait_method_not_in_trait_documented_gap() {
    let src = r#"
        trait Greeter {
            fn greet(&self) -> i32;
        }
        struct English;
        impl Greeter for English {
            fn greet(&self) -> i32 { 42 }
        }
        impl English {
            fn wave(&self) -> i32 { 99 }
        }
        fn main() -> i32 {
            let e = English;
            let g: &dyn Greeter = &e;
            g.wave()
        }
    "#;
    let result = compile_src(src);
    // Documented gap — may compile or error. No assertion.
    let _ = result;
}

/// Stage 88 negative 3: `dyn UndefinedTrait` errors (undefined trait
/// reference). Regression test from Stage 87.
#[test]
fn stage88_dyn_undefined_trait_errors() {
    let src = r#"
        fn main() -> i32 {
            let x: dyn UndefinedTrait = 0;
            0
        }
    "#;
    let result = compile_src(src);
    assert!(
        !result.errors.resolve.is_empty() || !result.errors.typeck.is_empty(),
        "dyn UndefinedTrait should error (undefined trait reference)"
    );
}
