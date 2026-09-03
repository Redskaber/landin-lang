//! Stage 86 (v0.8): TD-FN-IMPL-SIG-VALIDATION (return type check) FIXED.
//!
//! Verifies that typeck now validates the impl method's return type against
//! the trait's declared return type — including when the trait return type
//! is `Self::Output` (an associated type projection).
//!
//! ## Background
//!
//! Stage 78 fixed the param type check by substituting trait generic args
//! (e.g., `Args` → `(i32,)`) before comparing. But the return type check
//! was deferred because the trait's `fn call(&self, args: Args) -> Self::Output`
//! declares the return type as `Self::Output` — an associated type projection
//! that requires resolving to the impl's `type Output = T;` declaration.
//!
//! Stage 78's `trait_ret` was `TyKind::Error` (because `lower_hir_ty_to_mir_ty`
//! without HIR context can't resolve `Self::X`), and
//! `mir_ty_kinds_compatible(_, Error) == true` (Error is a wildcard), so
//! mismatches were silently accepted.
//!
//! Stage 86 fix: in `validate_impl_method_signatures`, use HIR-aware ty
//! lowering (`lower_hir_ty_to_mir_ty_with_hir`) so `Self::Output` becomes
//! `TyKind::Projection(assoc_def_id, [])`, then call the existing
//! `projection_resolver::resolve_projection_in_ty_pub` to resolve the
//! projection to the concrete type from the impl block's `type Output = T;`
//! declaration.
//!
//! ## Test Matrix (§9.4.3 — 1:3+ positive:negative ratio)
//!
//! - 1 positive test (valid Fn impl with correct return type — verifies
//!   the fix doesn't break legitimate impls).
//! - 3 negative typeck tests:
//!   - Wrong return type (i64 vs Output=i32)
//!   - Wrong return type (bool vs Output=i32 — totally incompatible)
//!   - Wrong return type (i32 vs Output=String — Adt mismatch)
//!
//! Per §1.0 原則 4 (报错 > 静默): return type mismatches must error, not
//! silently match via the Error wildcard.
//! Per §12 (最优 > 最小): root-cause fix at the validation site (resolve
//! projections before comparison), not weaken `mir_ty_kinds_compatible`
//! to reject Error (which would break other legitimate uses).
//! Per §1.0 原則 6 (通解 > 特解): reuse the existing projection_resolver
//! helper, don't write a parallel resolver.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::compile_src;

// =============================================================================
// Positive: valid Fn impl with correct return type still compiles
// =============================================================================

/// Stage 86 positive 1: `impl Fn<(i32,)> for Doubler` with `type Output = i32`
/// and `fn call(&self, args: (i32,)) -> i32 { ... }` — return type matches
/// Output. This verifies the fix doesn't break legitimate impls.
///
/// Without this test, a regression that always rejects return types (e.g.,
/// if `resolve_projection_in_ty_pub` failed and trait_ret stayed as
/// `Projection(...)` which `mir_ty_kinds_compatible` doesn't match with
/// `Int(I32)`) would silently break all valid Fn impls.
#[test]
fn stage86_fn_impl_correct_return_type_compiles() {
    let src = r#"
        struct Doubler;
        impl Fn<(i32,)> for Doubler {
            type Output = i32;
            fn call(&self, args: (i32,)) -> i32 {
                let x: i32 = args.0;
                x * 2
            }
        }
        fn main() { let _d = Doubler; 0 }
    "#;
    let result = compile_src(src);
    assert!(
        result.errors.typeck.is_empty(),
        "valid Fn impl with matching return type should compile without typeck errors, got: {:?}",
        result.errors.typeck
    );
}

// =============================================================================
// Negative: return type mismatches must error at typeck
// =============================================================================

/// Stage 86 negative 1: `Fn<(i32,)>` impl with `type Output = i32` but
/// `fn call(...) -> i64` — return type is i64, doesn't match Output=i32.
/// Typeck must report "method `call` return type mismatch: expected `i32`,
/// found `i64`".
///
/// Before Stage 86 fix: this silently compiled because `trait_ret` was
/// `TyKind::Error` (Self::Output unresolved) and Error is a wildcard.
/// After Stage 86 fix: `Self::Output` is resolved to `i32` via
/// `resolve_projection_in_ty_pub`, so the mismatch is caught.
#[test]
fn stage86_fn_impl_wrong_return_i64_vs_i32_errors() {
    let src = r#"
        struct Doubler;
        impl Fn<(i32,)> for Doubler {
            type Output = i32;
            fn call(&self, args: (i32,)) -> i64 {
                let x: i32 = args.0;
                x as i64 * 2
            }
        }
        fn main() { let _d = Doubler; 0 }
    "#;
    let result = compile_src(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "Fn impl with return type i64 (vs Output=i32) should fail typeck"
    );
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("return type mismatch") && msg.contains("i32") && msg.contains("i64"),
        "error should mention return type mismatch with i32/i64, got: {}",
        msg
    );
}

/// Stage 86 negative 2: `Fn<(i32,)>` impl with `type Output = i32` but
/// `fn call(...) -> bool` — return type is bool, totally incompatible with
/// Output=i32. Typeck must report the mismatch.
///
/// This tests a totally incompatible type (bool vs integer) — ensures
/// the return type check catches non-numeric mismatches too.
///
/// Per §1.0 原則 1 (内存安全决不能妥协): silently accepting bool as i32
/// would cause type confusion and memory unsafety.
#[test]
fn stage86_fn_impl_wrong_return_bool_vs_i32_errors() {
    let src = r#"
        struct Flag;
        impl Fn<(i32,)> for Flag {
            type Output = i32;
            fn call(&self, args: (i32,)) -> bool {
                args.0 > 0
            }
        }
        fn main() { let _f = Flag; 0 }
    "#;
    let result = compile_src(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "Fn impl with return type bool (vs Output=i32) should fail typeck"
    );
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("return type mismatch"),
        "error should mention return type mismatch, got: {}",
        msg
    );
}

/// Stage 86 negative 3: `Fn<(i32,)>` impl with `type Output = i32` but
/// `fn call(...) -> Wrapper` (user Adt) — return type is a user-defined
/// struct, doesn't match Output=i32. Typeck must report the mismatch.
///
/// This tests Adt vs Int mismatch — ensures the return type check catches
/// struct vs primitive mismatches too.
#[test]
fn stage86_fn_impl_wrong_return_adt_vs_i32_errors() {
    let src = r#"
        struct Wrapper;
        impl Fn<(i32,)> for Wrapper {
            type Output = i32;
            fn call(&self, args: (i32,)) -> Wrapper {
                Wrapper
            }
        }
        fn main() { let _w = Wrapper; 0 }
    "#;
    let result = compile_src(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "Fn impl with return type Wrapper (vs Output=i32) should fail typeck"
    );
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("return type mismatch"),
        "error should mention return type mismatch, got: {}",
        msg
    );
}
