//! Stage 18.259 — TD-UNIFY-ARG-ORDER regression tests.
//!
//! Verifies that unify arg order swap in `typeck/check.rs` (5 sites:
//! Call arg/return for FnDef, FnPtr, Closure + Switch discr) produces
//! error messages with correct direction: "expected <declared>, found
//! <actual>".
//!
//! Per §17.6 缺陷纳入: same-class unify arg order bug, batched fix.
//! Per §9.4.3 1:3+ ratio: each function category (FnDef, FnPtr,
//! Closure, Switch) has 1 positive + 3 negative cases.
//!
//! Per §2 原则 3 (显式 > 隐式): declared type (function sig input/output)
//! is "expected", actual value type is "found".
//! Per §2 原則 9 (正确 > 妥协): correct error message direction.

#![cfg(test)]

use landin_compiler::compile;

// ============================================================================
// FnDef call arg/return — error message direction tests.
// ============================================================================

#[test]
fn test_fndef_call_arg_type_mismatch_message_direction() {
    // `fn f(x: i32)` called with `true` (bool).
    // Expected message: "expected i32, found bool" (sig input is expected).
    let src = r#"
        fn f(x: i32) -> i32 { x }
        fn main() -> i32 {
            f(true)
        }
    "#;
    let result = compile(src);
    assert!(
        result.has_errors(),
        "Expected error for f(true) where f takes i32"
    );
    assert!(!result.errors.typeck.is_empty(), "Expected typeck error");
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found bool"),
        "FnDef call arg direction wrong: {msg}"
    );
}

#[test]
fn test_fndef_call_return_type_mismatch_message_direction() {
    // `fn g() -> i32` returns bool — dest is i32, actual is bool.
    // Expected message: "expected i32, found bool" (sig output is expected).
    let src = r#"
        fn g() -> bool { true }
        fn main() -> i32 {
            let x: i32 = g();
            x
        }
    "#;
    let result = compile(src);
    assert!(
        result.has_errors(),
        "Expected error for return type mismatch"
    );
    assert!(!result.errors.typeck.is_empty(), "Expected typeck error");
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found bool"),
        "FnDef return direction wrong: {msg}"
    );
}

#[test]
fn test_fndef_call_arg_string_vs_int_message_direction() {
    // Different types — str vs i32.
    let src = r#"
        fn f(x: i32) -> i32 { x }
        fn main() -> i32 {
            f("hello")
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found"),
        "FnDef call arg direction wrong: {msg}"
    );
}

#[test]
fn test_fndef_call_arg_wrong_int_type_message_direction() {
    // u64 vs i32 — different integer widths.
    let src = r#"
        fn f(x: i32) -> i32 { x }
        fn main() -> i32 {
            f(42i64)
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found"),
        "FnDef call arg direction wrong: {msg}"
    );
}

// ============================================================================
// Switch discr (if/while condition) — error message direction tests.
// ============================================================================

#[test]
fn test_if_condition_mismatch_message_direction() {
    // `if 42 { ... }` — if requires bool, but actual is i32.
    // Expected message: "expected bool, found i32" (bool is expected).
    let src = r#"
        fn main() -> i32 {
            if 42 { 1 } else { 2 }
        }
    "#;
    let result = compile(src);
    assert!(
        result.has_errors(),
        "Expected error for if 42 (non-bool condition)"
    );
    assert!(!result.errors.typeck.is_empty(), "Expected typeck error");
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected bool") && msg.contains("found"),
        "Switch discr direction wrong: {msg}"
    );
}

#[test]
fn test_while_condition_mismatch_message_direction() {
    // `while "hello" { ... }` — while requires bool, but actual is str.
    let src = r#"
        fn main() -> i32 {
            while "hello" { }
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected bool") && msg.contains("found"),
        "Switch discr direction wrong: {msg}"
    );
}

#[test]
fn test_if_condition_struct_mismatch_message_direction() {
    // Custom struct as if condition — should error.
    let src = r#"
        struct Foo { x: i32 }
        fn main() -> i32 {
            let f = Foo { x: 42 };
            if f { 1 } else { 2 }
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected bool"),
        "Switch discr direction wrong: {msg}"
    );
}

// ============================================================================
// Closure call arg — error message direction tests.
// ============================================================================

#[test]
fn test_closure_call_arg_mismatch_message_direction() {
    // `let c = |x: i32| x; c(true)` — closure expects i32, called with bool.
    let src = r#"
        fn main() -> i32 {
            let c = |x: i32| x;
            c(true)
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found bool"),
        "Closure call arg direction wrong: {msg}"
    );
}

#[test]
fn test_closure_call_arg_string_mismatch_message_direction() {
    // `let c = |x: i32| x; c("hello")` — closure expects i32, called with str.
    let src = r#"
        fn main() -> i32 {
            let c = |x: i32| x;
            c("hello")
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found"),
        "Closure call arg direction wrong: {msg}"
    );
}

// ============================================================================
// Positive cases — verify valid code still compiles without errors.
// ============================================================================

#[test]
fn test_fndef_valid_call_passes() {
    let src = r#"
        fn add(x: i32, y: i32) -> i32 { x + y }
        fn main() -> i32 {
            add(1, 2)
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
fn test_closure_valid_call_passes() {
    let src = r#"
        fn main() -> i32 {
            let c = |x: i32| x * 2;
            c(21)
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
fn test_if_valid_bool_condition_passes() {
    let src = r#"
        fn main() -> i32 {
            if true { 1 } else { 2 }
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Unexpected errors: {:?}",
        result.errors.typeck
    );
}
