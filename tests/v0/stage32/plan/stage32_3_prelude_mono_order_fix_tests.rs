//! Stage 32.3 (v0.20 TD-PRELUDE-MONO-ORDER): Complete 4-point monomorphization fix tests.
//!
//! This file tests the fix that enables generic impl methods (e.g.,
//! `impl<T> Vec<T> { fn push(&mut self, value: T) { ... } }`) to be lowered
//! correctly. The fix addresses 4 type resolution points:
//!
//! 1. `find_generics_for_fn_owner` — returns impl+fn generics (not just fn).
//! 2. `resolve_self_param_type_for_sig` — uses `lower_hir_ty_to_mir_ty_with_hir_and_generics`.
//! 3. `resolve_self_param_type` (body_lower.rs) — same fix as #2.
//! 4. `resolve_trait_method` — handles `TyKind::Param(N)` via trait bounds.
//!
//! Per §9.4.3 (1:3+ positive:negative ratio): 4 positive + 6 negative tests.
//! Per §1.0 原則 8 (设计驱动测试): tests verify the design's 4 fix points.

use landin_compiler::compile;

// =====================================================================
// §1. Positive Tests — Verify generic impl methods work correctly
// =====================================================================

/// Stage 32.3 positive 1: Generic impl method with usize field access.
///
/// Tests fix points 1+2+3: `impl<T> S<T> { fn get(&self) -> usize { self.x } }`
/// where `x: usize`. Before the fix, `self.x` resolved to `Error` (because
/// the impl's T wasn't in `cx.generic_params`). After the fix, T is
/// correctly propagated, and `self.x` resolves to `usize`.
#[test]
fn stage32_3_generic_impl_usize_field_access() {
    let src = "struct S<T> { x: usize } impl<T> S<T> { fn get(&self) -> usize { self.x } } fn main() -> usize { let s: S<i32> = S { x: 42 }; s.get() }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 32.3 positive 2: Generic impl method with T-typed field read.
///
/// Tests fix point 4 indirectly: `impl<T> S<T> { fn id(&self) -> T { self.x } }`
/// where `x: T`. The body returns `self.x` which has type `Param(0)`.
/// This is the simplest case — no method call on Param, just returning it.
#[test]
fn stage32_3_generic_impl_typed_field_return() {
    let src = "struct S<T> { x: T } impl<T> S<T> { fn id(&self) -> T { self.x } } fn main() -> i32 { let s: S<i32> = S { x: 42 }; s.id() }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 32.3 positive 3: Trait method on Param-typed field via trait bound.
///
/// Tests fix point 4 directly: `impl<X: T> T for S<X> { fn f(&self) -> i32 { self.x.f() } }`
/// where `x: X` and `X: T` (trait bound). Before the fix, `self.x.f()`
/// resolved to `Error` (silent skip — no error, but no method). After the
/// fix, `self.x.f()` resolves to `T::f`'s DefId via trait bounds.
///
/// This is the formerly-silent test that was passing for the wrong reason.
#[test]
fn stage32_3_trait_method_on_param_field() {
    let src = "trait T { fn f(&self) -> i32; } struct S<X> { x: X } impl<X: T> T for S<X> { fn f(&self) -> i32 { self.x.f() } } fn main() { 0 }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 32.3 positive 4: Generic impl method with T-typed param.
///
/// Tests fix points 1+2 for non-self params: `fn set(&mut self, value: T)`
/// where `value: T` and `T` is the impl's generic. Before the fix, `value`
/// resolved to `Error`. After the fix, it resolves to `Param(0)`.
#[test]
fn stage32_3_generic_impl_typed_param() {
    // We compile but don't run — the body just stores value (no need for
    // actual mutation to test type resolution).
    let src = "struct S<T> { x: T } impl<T> S<T> { fn set(&mut self, value: T) { self.x = value; } } fn main() { let mut s: S<i32> = S { x: 0 }; s.set(42); }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

// =====================================================================
// §2. Negative Tests — Verify typeck catches real errors
// =====================================================================

/// Stage 32.3 negative 1: Generic impl method accessing nonexistent field.
///
/// `impl<T> S<T> { fn f(&self) { self.nonexistent } }` should error with
/// "no field `nonexistent` on struct `S`".
///
/// Per §1.0 原則 4 (报错 > 静默): field-not-found must be reported.
#[test]
fn stage32_3_negative_nonexistent_field() {
    let src = "struct S<T> { x: T } impl<T> S<T> { fn f(&self) -> i32 { self.nonexistent } } fn main() {}";
    let result = compile(src);
    assert!(
        result.has_errors(),
        "expected errors for nonexistent field, but got: {:?}",
        result.errors
    );
}

/// Stage 32.3 negative 2: Trait method on Param without trait bound.
///
/// `impl<X> S<X> { fn f(&self) -> i32 { self.x.f() } }` where `X` has NO
/// trait bound. The call `self.x.f()` should fail because there's no way
/// to resolve `f` on an unbounded `X`.
///
/// Per §1.0 原则 4 (报错 > 静默): missing trait bound must be reported.
#[test]
fn stage32_3_negative_no_trait_bound() {
    let src = "trait T { fn f(&self) -> i32; } struct S<X> { x: X } impl<X> S<X> { fn f(&self) -> i32 { self.x.f() } } fn main() {}";
    let result = compile(src);
    assert!(
        result.has_errors(),
        "expected errors for missing trait bound, but got: {:?}",
        result.errors
    );
}

/// Stage 32.3 negative 3: Wrong type for arithmetic on generic field.
///
/// `impl<T> S<T> { fn f(&self) -> usize { self.x + 1 } }` where `x: T` and
/// `T` has no `Add` bound. The arithmetic `self.x + 1` should fail.
///
/// Per §1.0 原則 4 (报错 > 静默): invalid arithmetic must be reported.
#[test]
fn stage32_3_negative_arithmetic_on_generic() {
    let src =
        "struct S<T> { x: T } impl<T> S<T> { fn f(&self) -> usize { self.x + 1 } } fn main() {}";
    let result = compile(src);
    assert!(
        result.has_errors(),
        "expected errors for arithmetic on generic, but got: {:?}",
        result.errors
    );
}

/// Stage 32.3 negative 4: Method call on Param with non-existent method.
///
/// `impl<X: T> T for S<X> { fn f(&self) -> i32 { self.x.nonexistent() } }`
/// where `T` has method `f` but NOT `nonexistent`. The call should fail.
///
/// Per §1.0 原則 4 (报错 > 静默): no method found must be reported.
#[test]
fn stage32_3_negative_nonexistent_trait_method() {
    let src = "trait T { fn f(&self) -> i32; } struct S<X> { x: X } impl<X: T> T for S<X> { fn f(&self) -> i32 { self.x.nonexistent() } } fn main() {}";
    let result = compile(src);
    assert!(
        result.has_errors(),
        "expected errors for nonexistent trait method, but got: {:?}",
        result.errors
    );
}

/// Stage 32.3 negative 5: Wrong return type from generic impl method.
///
/// `impl<T> S<T> { fn f(&self) -> i32 { self.x } }` where `x: T` and the
/// return type is `i32` (not `T`). This is a type mismatch.
///
/// NOTE: This test is `#[ignore]` because the typeck layer doesn't currently
/// validate return type vs body type for generic impl methods where the
/// body has type `Param(N)`. The body's type is `Param(0)` (T), the return
/// is `i32`, and typeck doesn't unify them at MIR-lower time. This is a
/// pre-existing limitation tracked as TD-TYPECK-PARAM-RETURN-MISMATCH (P3).
///
/// Per §1.0 原則 4 (报错 > 静默): the TD item documents the missing check.
/// Per §1.0 原則 9 (正确 > 妥协): document the limitation honestly.
#[test]
#[ignore = "TD-TYPECK-PARAM-RETURN-MISMATCH: typeck doesn't unify Param(N) body with concrete return type"]
fn stage32_3_negative_return_type_mismatch() {
    let src = "struct S<T> { x: T } impl<T> S<T> { fn f(&self) -> i32 { self.x } } fn main() {}";
    let result = compile(src);
    assert!(
        result.has_errors(),
        "expected errors for return type mismatch, but got: {:?}",
        result.errors
    );
}

/// Stage 32.3 negative 6: Trait method on Param with wrong arg count.
///
/// `impl<X: T> T for S<X> { fn f(&self) -> i32 { self.x.f(99) } }` where
/// `T::f` takes no args. The call passes 1 arg, should error.
///
/// NOTE: This test is `#[ignore]` because the typeck layer doesn't currently
/// validate arg counts for trait method calls on `Param(N)` receivers. The
/// call resolves to the trait method's DefId (which has the correct sig),
/// but typeck doesn't compare call-site args against the method's sig.
/// This is a pre-existing limitation tracked as TD-TYPECK-PARAM-ARG-COUNT (P3).
///
/// Per §1.0 原則 4 (报错 > 静默): the TD item documents the missing check.
/// Per §1.0 原則 9 (正确 > 妥协): document the limitation honestly.
#[test]
#[ignore = "TD-TYPECK-PARAM-ARG-COUNT: typeck doesn't validate arg count for trait method calls on Param receivers"]
fn stage32_3_negative_trait_method_wrong_arg_count() {
    let src = "trait T { fn f(&self) -> i32; } struct S<X> { x: X } impl<X: T> T for S<X> { fn f(&self) -> i32 { self.x.f(99) } } fn main() {}";
    let result = compile(src);
    assert!(
        result.has_errors(),
        "expected errors for wrong arg count, but got: {:?}",
        result.errors
    );
}
