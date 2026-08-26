//! Stage 18.256 — Phase 2a regression tests.
//!
//! Verifies that adding `expected_ty: Option<&Ty>` parameter to
//! `lower_expr_to_operand` and `lower_expr_to_place` is purely additive —
//! all existing behavior preserved when callers pass `None`.
//!
//! Per §13.4 J2 (单一职责): Phase 2a is purely scaffolding — the param
//! exists but is unused. Phase 2b+ will start using it.
//! Per §1.0 原則 9 (正确 > 妥协): the scaffolding is necessary to enable
//! Phase 2b-2e without breaking the build mid-stage.

#![cfg(test)]

use landin_compiler::compile;

// ============================================================================
// Smoke tests — verify existing valid programs still compile correctly.
// ============================================================================

#[test]
fn test_phase_2a_smoke_simple_program() {
    let src = r#"
        fn main() -> i32 {
            let x = 42;
            x
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Unexpected errors: {:?}",
        result.errors.typeck
    );
}

#[test]
fn test_phase_2a_smoke_struct_ctor() {
    // `Holder::<i32>(42)` — explicit turbofish + valid arg.
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
        "Unexpected errors: {:?}",
        result.errors.typeck
    );
}

#[test]
fn test_phase_2a_smoke_arithmetic() {
    let src = r#"
        fn main() -> i32 {
            let a = 10;
            let b = 20;
            let c = a + b;
            c * 2
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Unexpected errors: {:?}",
        result.errors.typeck
    );
}

#[test]
fn test_phase_2a_smoke_closure() {
    let src = r#"
        fn main() -> i32 {
            let add = |x: i32, y: i32| x + y;
            add(3, 4)
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Unexpected errors: {:?}",
        result.errors.typeck
    );
}

#[test]
fn test_phase_2a_smoke_if_else() {
    let src = r#"
        fn main() -> i32 {
            let x = if true { 1 } else { 2 };
            x
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Unexpected errors: {:?}",
        result.errors.typeck
    );
}

#[test]
fn test_phase_2a_smoke_loop() {
    let src = r#"
        fn main() -> i32 {
            let mut i = 0;
            let mut sum = 0;
            while i < 10 {
                sum = sum + i;
                i = i + 1;
            }
            sum
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Unexpected errors: {:?}",
        result.errors.typeck
    );
}

// ============================================================================
// Regression — Phase 1 fix (Stage 18.255) still works.
// ============================================================================

#[test]
fn test_phase_2a_regression_turbofish_wrong_arg_still_errors() {
    // From Stage 18.255 regression tests — must still error correctly.
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let w: Holder<i32> = Holder::<i32>(true);
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found bool"),
        "Phase 1 fix must still work: {msg}"
    );
}

#[test]
fn test_phase_2c_soundness_hole_now_closed() {
    // Stage 18.258 (Phase 2c): soundness hole CLOSED.
    // `Holder(true)` with `let : Holder<i32>` now errors because
    // expected_ty threading extracts substs from the let annotation
    // when turbofish is absent, allowing field_tys to be substituted
    // correctly.
    //
    // Per §1.0 原則 9 (正确 > 妥协): full soundness fix.
    // Per §17.6: MVP marker converted to assert.
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let w: Holder<i32> = Holder(true);
            0
        }
    "#;
    let result = compile(src);
    assert!(
        result.has_errors(),
        "Phase 2c must close soundness hole: Holder(true) with let : Holder<i32> should error"
    );
}

#[test]
fn test_phase_2e_call_arg_soundness_hole_now_closed() {
    // Stage 18.262 (Phase 2e): soundness hole CLOSED for call args.
    // `take_holder(Holder(true))` where `fn take_holder(h: Holder<i32>)`
    // now errors because:
    // 1. Driver pre-builds fn_sig_table
    // 2. Passes fn_sigs as read-only data contract to MirLowerCtxt
    // 3. `lower_call_expr` looks up callee's sig.inputs[i]
    // 4. Threads expected_ty into arg's `lower_expr_to_operand`
    // 5. Adt ctor path uses expected_ty to extract substs (Phase 2c)
    //
    // Per §1.0 原則 9 (正确 > 妥协): full soundness fix.
    // Per §17.6: MVP marker converted to assert.
    let src = r#"
        struct Holder<T>(T);
        fn take_holder(h: Holder<i32>) -> i32 { 0 }
        fn main() -> i32 {
            take_holder(Holder(true))
        }
    "#;
    let result = compile(src);
    assert!(
        result.has_errors(),
        "Phase 2e must close soundness hole: take_holder(Holder(true)) should error"
    );
}
