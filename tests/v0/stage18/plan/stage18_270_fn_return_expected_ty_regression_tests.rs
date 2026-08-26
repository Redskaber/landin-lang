//! Stage 18.270 — TD-GENERIC-FN-RETURN-EXPECTED-TY Phase 2d complete fix.
//!
//! Verifies that the fn body return type + Block expected-ty propagation
//! correctly closes the soundness hole for:
//! `fn make() -> Holder<i32> { Holder(true) }`
//!
//! Per §17.6 "直到审查不出问题为止": the Phase 2d fix in body_lower.rs
//! was incomplete because body.value is a Block, and the Block arm in
//! lower_expr_to_operand didn't pass expected_ty to lower_block. This
//! stage fixes that by adding expected_ty param to lower_block.
//!
//! Per §9.4.3 1:3+ ratio: 2 positive + 3 negative.

#![cfg(test)]

use landin_compiler::compile;

// ============================================================================
// Positive cases.
// ============================================================================

#[test]
fn test_fn_return_valid_passes() {
    let src = r#"
        struct Holder<T>(T);
        fn make_holder() -> Holder<i32> {
            Holder(42)
        }
        fn main() -> i32 { 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Unexpected errors: {:?}",
        result.errors.typeck
    );
}

#[test]
fn test_fn_return_with_explicit_turbofish_passes() {
    let src = r#"
        struct Holder<T>(T);
        fn make_holder() -> Holder<i32> {
            Holder::<i32>(42)
        }
        fn main() -> i32 { 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Unexpected errors: {:?}",
        result.errors.typeck
    );
}

// ============================================================================
// Negative cases — fn body return with wrong inner ctor.
// ============================================================================

#[test]
fn test_fn_return_bool_vs_i32_errors() {
    let src = r#"
        struct Holder<T>(T);
        fn make_holder() -> Holder<i32> {
            Holder(true)
        }
        fn main() -> i32 { 0 }
    "#;
    let result = compile(src);
    assert!(
        result.has_errors(),
        "fn return with wrong ctor should error"
    );
    assert!(!result.errors.typeck.is_empty(), "Expected typeck error");
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found bool"),
        "fn return direction wrong: {msg}"
    );
}

#[test]
fn test_fn_return_str_vs_i32_errors() {
    let src = r#"
        struct Holder<T>(T);
        fn make_holder() -> Holder<i32> {
            Holder("hello")
        }
        fn main() -> i32 { 0 }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found"),
        "fn return direction wrong: {msg}"
    );
}

#[test]
fn test_fn_return_i64_vs_i32_errors() {
    let src = r#"
        struct Holder<T>(T);
        fn make_holder() -> Holder<i32> {
            Holder(42i64)
        }
        fn main() -> i32 { 0 }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found"),
        "fn return direction wrong: {msg}"
    );
}
