//! Stage 18.264 — TD-STRUCT-LITERAL-FIELD-EXPECTED-TY + TD-BOX-NEW-EXPECTED-TY
//! regression tests.
//!
//! Verifies the two soundness holes closed in Stage 18.264:
//! 1. Struct literal field values: `Outer { f: Holder(true) }` (f: Holder<i32>)
//! 2. Box::new intrinsic arg: `Box::new(Holder(true))` (b: Box<Holder<i32>>)
//!
//! Per §17.6 (缺陷纳入 — same class as TD-TUPLE-CTOR-CALL-ARG):
//! when one expected-ty propagation bug is found, audit all similar paths.
//!
//! Per §9.4.3 1:3+ ratio: 2 positive + 6 negative per fix.
//! Per §1.0 原則 9 (正确 > 妥协): full soundness fix.

#![cfg(test)]

use landin_compiler::compile;

// ============================================================================
// Struct literal field values — Phase 1: positive cases.
// ============================================================================

#[test]
fn test_struct_literal_field_valid_passes() {
    let src = r#"
        struct Holder<T>(T);
        struct Outer { f: Holder<i32> }
        fn main() -> i32 {
            let o = Outer { f: Holder(42) };
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
fn test_struct_literal_field_with_explicit_turbofish_passes() {
    let src = r#"
        struct Holder<T>(T);
        struct Outer { f: Holder<i32> }
        fn main() -> i32 {
            let o = Outer { f: Holder::<i32>(42) };
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
// Struct literal field values — Phase 2: negative cases.
// ============================================================================

#[test]
fn test_struct_literal_field_bool_vs_i32_errors() {
    // `Outer { f: Holder(true) }` where `f: Holder<i32>`.
    let src = r#"
        struct Holder<T>(T);
        struct Outer { f: Holder<i32> }
        fn main() -> i32 {
            let o = Outer { f: Holder(true) };
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found bool"),
        "Struct field direction wrong: {msg}"
    );
}

#[test]
fn test_struct_literal_field_str_vs_i32_errors() {
    let src = r#"
        struct Holder<T>(T);
        struct Outer { f: Holder<i32> }
        fn main() -> i32 {
            let o = Outer { f: Holder("hello") };
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found"),
        "Struct field direction wrong: {msg}"
    );
}

#[test]
fn test_struct_literal_field_i64_vs_i32_errors() {
    let src = r#"
        struct Holder<T>(T);
        struct Outer { f: Holder<i32> }
        fn main() -> i32 {
            let o = Outer { f: Holder(42i64) };
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found"),
        "Struct field direction wrong: {msg}"
    );
}

// ============================================================================
// Box::new intrinsic arg — Phase 1: positive cases.
// ============================================================================

#[test]
fn test_box_new_valid_arg_passes() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let b: Box<Holder<i32>> = Box::new(Holder(42));
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
fn test_box_new_with_explicit_turbofish_passes() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let b: Box<Holder<i32>> = Box::new(Holder::<i32>(42));
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
// Box::new intrinsic arg — Phase 2: negative cases.
// ============================================================================

#[test]
fn test_box_new_bool_vs_i32_errors() {
    // `Box::new(Holder(true))` where `b: Box<Holder<i32>>`.
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let b: Box<Holder<i32>> = Box::new(Holder(true));
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found bool"),
        "Box::new arg direction wrong: {msg}"
    );
}

#[test]
fn test_box_new_str_vs_i32_errors() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let b: Box<Holder<i32>> = Box::new(Holder("hello"));
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found"),
        "Box::new arg direction wrong: {msg}"
    );
}

#[test]
fn test_box_new_i64_vs_i32_errors() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let b: Box<Holder<i32>> = Box::new(Holder(42i64));
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors());
    let msg = &result.errors.typeck[0].message;
    assert!(
        msg.contains("expected i32") && msg.contains("found"),
        "Box::new arg direction wrong: {msg}"
    );
}
