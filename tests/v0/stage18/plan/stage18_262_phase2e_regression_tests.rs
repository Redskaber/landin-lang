//! Stage 18.262 — TD-TUPLE-CTOR-CALL-ARG Phase 2e regression tests.
//!
//! Verifies that the fn_sigs-based expected-ty propagation in
//! `lower_call_expr` correctly closes the soundness hole for call args.
//!
//! Per §17.6 缺陷纳入: closes the gap identified in Stage 18.260.
//! Per §9.4.3 1:3+ ratio: 1 positive + 3+ negative per feature.
//!
//! Per §2 原則 3 (显式 > 隐式): declared type (fn sig input) is
//! "expected", actual value type is "found".
//! Per §1.0 原則 9 (正确 > 妥协): full soundness fix, not MVP.

#![cfg(test)]

use landin_compiler::compile;

// ============================================================================
// Positive cases — verify valid code still compiles correctly.
// ============================================================================

#[test]
fn test_phase_2e_valid_call_passes() {
    // `take_holder(Holder(42))` — valid call with correct arg type.
    let src = r#"
        struct Holder<T>(T);
        fn take_holder(h: Holder<i32>) -> i32 { 0 }
        fn main() -> i32 {
            take_holder(Holder(42))
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
fn test_phase_2e_valid_call_with_explicit_turbofish_passes() {
    // `take_holder(Holder::<i32>(42))` — explicit turbofish + valid arg.
    let src = r#"
        struct Holder<T>(T);
        fn take_holder(h: Holder<i32>) -> i32 { 0 }
        fn main() -> i32 {
            take_holder(Holder::<i32>(42))
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
// Negative cases — Phase 2e fix (fn_sigs propagation in lower_call_expr).
// ============================================================================

#[test]
fn test_phase_2e_call_arg_bool_vs_i32_errors() {
    // `take_holder(Holder(true))` where `fn take_holder(h: Holder<i32>)`.
    // Should error with "expected i32, found bool".
    let src = r#"
        struct Holder<T>(T);
        fn take_holder(h: Holder<i32>) -> i32 { 0 }
        fn main() -> i32 {
            take_holder(Holder(true))
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found bool"),
        "Phase 2e direction wrong: {msg}"
    );
}

#[test]
fn test_phase_2e_call_arg_str_vs_i32_errors() {
    // Different arg type mismatch — str vs i32.
    let src = r#"
        struct Holder<T>(T);
        fn take_holder(h: Holder<i32>) -> i32 { 0 }
        fn main() -> i32 {
            take_holder(Holder("hello"))
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found"),
        "Phase 2e direction wrong: {msg}"
    );
}

#[test]
fn test_phase_2e_call_arg_i64_vs_i32_errors() {
    // Different integer width — i64 vs i32.
    let src = r#"
        struct Holder<T>(T);
        fn take_holder(h: Holder<i32>) -> i32 { 0 }
        fn main() -> i32 {
            take_holder(Holder(42i64))
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found"),
        "Phase 2e direction wrong: {msg}"
    );
}

#[test]
fn test_phase_2e_call_arg_rawptr_vs_bool_errors() {
    // Wrapper with raw ptr field — different type signature.
    let src = r#"
        struct Wrapper<T>(*mut T);
        fn take_wrapper(w: Wrapper<i32>) -> i32 { 0 }
        fn main() -> i32 {
            take_wrapper(Wrapper(true))
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected") && msg.contains("*mut i32") && msg.contains("found bool"),
        "Phase 2e direction wrong: {msg}"
    );
}

#[test]
fn test_phase_2e_call_arg_wrong_second_param_errors() {
    // Multi-arg fn — second arg has wrong type.
    let src = r#"
        struct Pair<A, B>(A, B);
        fn take_pair(p: Pair<i32, bool>) -> i32 { 0 }
        fn main() -> i32 {
            take_pair(Pair(42, 99))
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected bool"),
        "Phase 2e direction wrong: {msg}"
    );
}

// ============================================================================
// Edge cases — verify fix doesn't break existing valid patterns.
// ============================================================================

#[test]
fn test_phase_2e_nested_calls_with_correct_types_passes() {
    // Nested call with correct types — should pass.
    let src = r#"
        struct Holder<T>(T);
        fn identity(h: Holder<i32>) -> Holder<i32> { h }
        fn consume(h: Holder<i32>) -> i32 { 0 }
        fn main() -> i32 {
            consume(identity(Holder(42)))
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
fn test_phase_2e_call_arg_with_let_binding_passes() {
    // Pre-bound local passed as arg.
    let src = r#"
        struct Holder<T>(T);
        fn take_holder(h: Holder<i32>) -> i32 { 0 }
        fn main() -> i32 {
            let h: Holder<i32> = Holder(42);
            take_holder(h)
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Unexpected errors: {:?}",
        result.errors.typeck
    );
}
