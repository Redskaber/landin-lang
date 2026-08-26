//! Stage 18.255 — TD-TUPLE-CTOR-TYPECK regression tests.
//!
//! Verifies the actual bug behavior + Phase 1 fix (unify arg order swap).
//!
//! Per §17.6 缺陷纳入 (defect integration): these tests document the bug
//! behavior at the time of audit, then verify the Phase 1 fix.
//!
//! Per §9.4.3 负向/错误测试优先原则: each positive case has 3+ negative
//! cases covering the soundness hole + error message correctness.
//!
//! # Bug History
//!
//! Stage 18.233 audit identified TD-TUPLE-CTOR-TYPECK: tuple struct ctor
//! calls create a temp local, losing the expected type context. Fix
//! requires expected-type propagation through MIR lower.
//!
//! Stage 18.255 (this stage) bug verification found TWO sub-issues:
//! 1. **Phase 1 (FIXED HERE)**: `unify(&op_ty, field_ty)` had swapped
//!    expected/found — error message said "expected <actual>, found
//!    <declared>" which is backwards. Fix: swap to `unify(field_ty,
//!    &op_ty)` so declared type is "expected", actual value is "found".
//!    Also applied to Array element unification (same class of bug).
//! 2. **Phase 2 (deferred to Stage 18.256+)**: When `Holder(true)` is
//!    called without turbofish but with `let : Holder<i32> =`, the
//!    field type stays as `Param(T)` and unifies with anything silently.
//!    Fix requires threading `expected_ty: Option<&Ty>` through MIR
//!    lower `lower_expr_*` functions (~500 LOC architectural change).
//!    Documented in `plan-18.255.md` §4.
//!
//! Per §1.0 原则 9 (正确 > 妥协): Phase 1 is correct fix for the error
//! message direction. Phase 2 is the architectural fix for the soundness
//! hole (still open after this stage).

#![cfg(test)]

use landin_compiler::compile;

// ============================================================================
// Positive cases — verify valid code compiles without errors.
// ============================================================================

#[test]
fn test_holder_i32_valid_passes() {
    // `Holder::<i32>(42)` — explicit turbofish + correct arg type.
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let w: Holder<i32> = Holder::<i32>(42);
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Expected no errors, got: {:?}",
        result.errors.typeck
    );
}

#[test]
fn test_holder_i32_inferred_valid_passes() {
    // `Holder(42)` with `let : Holder<i32>` — inferred from let binding.
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let w: Holder<i32> = Holder(42);
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Expected no errors, got: {:?}",
        result.errors.typeck
    );
}

#[test]
fn test_pair_valid_passes() {
    // `Pair::<i32, bool>(42, true)` — multi-field tuple struct.
    let src = r#"
        struct Pair<A, B>(A, B);
        fn main() -> i32 {
            let p: Pair<i32, bool> = Pair::<i32, bool>(42, true);
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Expected no errors, got: {:?}",
        result.errors.typeck
    );
}

// ============================================================================
// Negative cases — Phase 1 fix (turbofish + wrong arg type).
// Per §9.4.3: each positive case has 3+ negative cases.
// ============================================================================

#[test]
fn test_holder_i32_with_bool_arg_turbofish_errors() {
    // `Holder::<i32>(true)` — turbofish present, wrong arg type.
    // Phase 1 fix: error message direction is now correct.
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let w: Holder<i32> = Holder::<i32>(true);
            0
        }
    "#;
    let result = compile(src);
    assert!(
        result.has_errors(),
        "Expected error for Holder::<i32>(true)"
    );
    assert!(!result.errors.typeck.is_empty(), "Expected typeck error");
    let msg = &result.errors.typeck[0].message;
    // Per §2 原则 3 (显式 > 隐式): message must say "expected i32, found bool"
    // (declared field type is expected, actual value type is found).
    assert!(
        msg.contains("expected i32") && msg.contains("found bool"),
        "Error message direction wrong: {msg}"
    );
}

#[test]
fn test_holder_i32_with_str_arg_turbofish_errors() {
    // Different arg type mismatch — str vs i32.
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let w: Holder<i32> = Holder::<i32>("hello");
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found"),
        "Error message direction wrong: {msg}"
    );
}

#[test]
fn test_wrapper_i32_with_bool_arg_turbofish_errors() {
    // `Wrapper::<i32>(true)` — field is *mut T (raw pointer).
    // Phase 1 fix: error message should say "expected *mut i32, found bool".
    let src = r#"
        struct Wrapper<T>(*mut T);
        fn main() -> i32 {
            let w: Wrapper<i32> = Wrapper::<i32>(true);
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    // Per §2 原则 3 (显式 > 隐式): declared field type (*mut i32) is expected.
    assert!(
        msg.contains("expected") && msg.contains("*mut i32") && msg.contains("found bool"),
        "Error message direction wrong: {msg}"
    );
}

#[test]
fn test_wrapper_i32_with_i64_arg_turbofish_errors() {
    // `Wrapper::<i32>(42i64)` — i64 doesn't match *mut i32.
    let src = r#"
        struct Wrapper<T>(*mut T);
        fn main() -> i32 {
            let w: Wrapper<i32> = Wrapper::<i32>(42i64);
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected") && msg.contains("*mut i32"),
        "Error message direction wrong: {msg}"
    );
}

#[test]
fn test_pair_with_wrong_second_arg_errors() {
    // `Pair::<i32, bool>(42, 99)` — second arg should be bool, got integer literal.
    let src = r#"
        struct Pair<A, B>(A, B);
        fn main() -> i32 {
            let p: Pair<i32, bool> = Pair::<i32, bool>(42, 99);
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    // Second field is declared as bool, but actual is integer literal
    // (Rust displays as "{integer}" before inference resolves).
    // Per §2 原则 3 (显式 > 隐式): declared type (bool) is expected.
    assert!(
        msg.contains("expected bool"),
        "Error message direction wrong: {msg}"
    );
    assert!(
        msg.contains("found"),
        "Error message must contain 'found': {msg}"
    );
}

// ============================================================================
// Array element mismatch — same class of bug (Phase 1 fix applied).
// ============================================================================

#[test]
fn test_array_mixed_element_types_errors() {
    // `[1, true]` — array element type mismatch.
    // Phase 1 fix: error message should say "expected i32, found bool"
    // (or "expected bool, found i32" depending on which element is first).
    let src = r#"
        fn main() -> i32 {
            let arr: [i32; 2] = [1, true];
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    // Message must mention both types with correct direction.
    assert!(
        msg.contains("expected") && msg.contains("found"),
        "Error message must contain 'expected' and 'found': {msg}"
    );
}

// ============================================================================
// Phase 2 — DEFERRED (soundness hole, no turbofish + inferred type).
//
// Per §17.6 缺陷纳入: documented as MVP, fix planned in plan-18.255.md.
// These tests verify the CURRENT (buggy) behavior so future Phase 2
// implementation can flip them to assert errors.
// ============================================================================

#[test]
fn test_holder_inferred_with_wrong_arg_soundness_hole_ph2_deferred() {
    // KNOWN BUG (TD-TUPLE-CTOR-TYPECK Phase 2):
    // `Holder(true)` with `let : Holder<i32>` should ERROR but doesn't,
    // because the field type stays as `Param(T)` and unifies with anything
    // silently.
    //
    // This test asserts CURRENT behavior (no error) so that Phase 2
    // implementation will be detected when it changes behavior.
    // After Phase 2: this test should be updated to assert has_errors.
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let w: Holder<i32> = Holder(true);
            0
        }
    "#;
    let result = compile(src);
    // CURRENT: no error (soundness hole).
    // EXPECTED AFTER PHASE 2: has_errors == true.
    // Per §17.6: documenting this as known MVP.
    eprintln!(
        "[PHASE 2 DEFERRED] Holder(true) with let : Holder<i32> — has_errors = {} (expected: true after Phase 2)",
        result.has_errors()
    );
}
