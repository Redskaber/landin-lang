//! Stage 18.267 — TD-ENUM-VARIANT-CTOR-EXPECTED-TY regression tests.
//!
//! Verifies that the enum variant ctor expected-ty propagation + field_tys
//! substitution fix correctly closes the soundness hole for:
//! 1. `Some(Holder(true))` (where `let x: Option<Holder<i32>>`)
//! 2. `Result::Ok(Holder(true))` (where `let x: Result<Holder<i32>, E>`)
//! 3. `Option::Some(Holder(true))` (with explicit Option:: prefix)
//!
//! Per §17.6 (缺陷纳入 — same class as TD-STRUCT-LITERAL-FIELD-EXPECTED-TY):
//! when one expected-ty propagation bug is found, audit all similar paths.
//! Per §9.4.3 1:3+ ratio: 2 positive + 6 negative.
//! Per §1.0 原則 9 (正确 > 妥协): full soundness fix.

#![cfg(test)]

use landin_compiler::compile;

// ============================================================================
// Option::Some — positive cases.
// ============================================================================

#[test]
fn test_option_some_valid_passes() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let x: Option<Holder<i32>> = Some(Holder(42));
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
fn test_option_some_with_explicit_turbofish_passes() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let x: Option<Holder<i32>> = Some::<Holder<i32>>(Holder(42));
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

// ============================================================================
// Option::Some — negative cases.
// ============================================================================

#[test]
fn test_option_some_bool_vs_i32_errors() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let x: Option<Holder<i32>> = Some(Holder(true));
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found bool"),
        "Option::Some direction wrong: {msg}"
    );
}

#[test]
fn test_option_some_str_vs_i32_errors() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let x: Option<Holder<i32>> = Some(Holder("hello"));
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found"),
        "Option::Some direction wrong: {msg}"
    );
}

#[test]
fn test_option_some_simple_int_mismatch_errors() {
    // Simpler case: Some(true) where expected is Option<i32>
    let src = r#"
        fn main() -> i32 {
            let x: Option<i32> = Some(true);
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found bool"),
        "Option::Some simple direction wrong: {msg}"
    );
}

// ============================================================================
// Result::Ok — positive cases.
// ============================================================================

#[test]
fn test_result_ok_valid_passes() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let x: Result<Holder<i32>, i32> = Ok(Holder(42));
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

// ============================================================================
// Result::Ok — negative cases.
// ============================================================================

#[test]
fn test_result_ok_bool_vs_i32_errors() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let x: Result<Holder<i32>, i32> = Ok(Holder(true));
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found bool"),
        "Result::Ok direction wrong: {msg}"
    );
}

#[test]
fn test_result_ok_str_vs_i32_errors() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let x: Result<Holder<i32>, i32> = Ok(Holder("hello"));
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found"),
        "Result::Ok direction wrong: {msg}"
    );
}

// ============================================================================
// Option::Some with explicit Option:: prefix.
// ============================================================================

#[test]
fn test_option_some_with_two_segment_path_errors() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let x: Option<Holder<i32>> = Option::Some(Holder(true));
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found bool"),
        "Option::Some (2-seg path) direction wrong: {msg}"
    );
}
