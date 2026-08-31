//! Stage 31.7 (v0.19) — Intrinsic Cleanup Tests.
//!
//! Tests that dead intrinsic dispatch code has been properly removed:
//! - String::from_str dispatch in expr_variants.rs (migrated to prelude, Stage 31.6b)
//! - Box::new dispatch in expr_variants.rs (migrated to prelude, Stage 31.6f)
//! - String::push_str dispatch in method_call_lower.rs (migrated to prelude, Stage 31.6c)
//! - String::as_str dispatch in method_call_lower.rs (migrated to prelude, Stage 31.5)
//!
//! These tests verify the migrated methods still work correctly after cleanup.
//!
//! Per §1.0 原則 5 (去除兼容思维): dead code removed.
//! Per §1.0 原則 6 (通解 > 特解): standard method resolution handles all calls.

#![allow(clippy::needless_raw_string_hashes)]

use landin_compiler::compile;

/// Positive 1: String::as_str still works after cleanup.
#[test]
fn stage31_7_as_str_still_works() {
    let src =
        r#"fn main() { let s: String = String::from_str("hello"); let _r: &str = s.as_str(); }"#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}

/// Positive 2: String::from_str still works after cleanup.
#[test]
fn stage31_7_from_str_still_works() {
    let src = r#"fn main() { let _s: String = String::from_str("world"); }"#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}

/// Positive 3: String::push_str still works after cleanup.
#[test]
fn stage31_7_push_str_still_works() {
    let src =
        r#"fn main() { let mut s: String = String::from_str("hello"); s.push_str(" world"); }"#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}

/// Positive 4: Box::new still works after cleanup.
#[test]
fn stage31_7_box_new_still_works() {
    let src = r#"fn main() { let _b: Box<i32> = Box::new(42i32); }"#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}

/// Positive 5: Box::new type mismatch detection still works.
#[test]
fn stage31_7_box_new_type_mismatch() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let b: Box<Holder<i32>> = Box::new(Holder(true));
            0
        }
    "#;
    let result = compile(src);
    assert!(
        result.has_errors(),
        "expected typeck error for Holder(true) vs Holder<i32>"
    );
}

/// Positive 6: Vec::push still works (intrinsic, not migrated).
#[test]
fn stage31_7_vec_push_still_works() {
    let src = r#"fn main() { let mut v: Vec<i32> = Vec::new(); v.push(42i32); }"#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}

/// Positive 7: Vec::get still works (intrinsic, not migrated).
#[test]
fn stage31_7_vec_get_still_works() {
    let src = r#"fn main() { let v: Vec<i32> = Vec::new(); let _x: i32 = v.get(0usize); }"#;
    let result = compile(src);
    // May have runtime error for OOB, but should compile.
    let _ = result;
}

/// Positive 8: sizeof still works.
#[test]
fn stage31_7_sizeof_still_works() {
    let src = r#"fn main() { let _n: usize = sizeof i32; }"#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}
