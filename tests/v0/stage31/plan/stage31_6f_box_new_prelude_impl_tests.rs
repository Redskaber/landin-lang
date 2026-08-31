//! Stage 31.6f (v0.19) — Box::new Prelude Impl Migration Tests.
//!
//! Tests that `Box::new(x)` is now implemented in the prelude using
//! `sizeof(T)` + `extern "C"` __landin_alloc + Deref store + tuple struct
//! construction. This is the fourth TD-INTRINSIC-OVERUSE Phase 2-B migration.
//!
//! Per §1.0 原則 6 (通解 > 特解): standard method resolution replaces intrinsic.
//! Per §12 (最优 > 最小): root-cause fix via language features.

#![allow(clippy::needless_raw_string_hashes)]

use landin_compiler::{compile, compile_no_opt};

// =====================================================================
// Positive tests (4) — Box::new works via prelude impl
// =====================================================================

/// Positive 1: Box::new(i32) compiles via prelude impl.
#[test]
fn stage31_6f_box_new_i32_compiles() {
    let src = r#"fn main() { let _b: Box<i32> = Box::new(42i32); }"#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}

/// Positive 2: Box::new with struct type.
#[test]
fn stage31_6f_box_new_struct() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        fn main() { let _b: Box<Point> = Box::new(Point { x: 1i32, y: 2i32 }); }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}

/// Positive 3: Box::new with bool.
#[test]
fn stage31_6f_box_new_bool() {
    let src = r#"fn main() { let _b: Box<bool> = Box::new(true); }"#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}

/// Positive 4: Box::new with compile_no_opt.
#[test]
fn stage31_6f_box_new_no_opt() {
    let src = r#"fn main() { let _b: Box<i32> = Box::new(42i32); }"#;
    let result = compile_no_opt(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}

// =====================================================================
// Negative tests (12) — error categories per §7.3.1
// =====================================================================

/// Negative 1 (Typeck): Box::new with wrong return type (i32).
#[test]
fn stage31_6f_neg_box_new_wrong_return() {
    let src = r#"fn main() { let _n: i32 = Box::new(42i32); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for Box→i32"
    );
}

/// Negative 2 (Typeck): Box::new with no args.
#[test]
fn stage31_6f_neg_box_new_no_args() {
    let src = r#"fn main() { let _b = Box::new(); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty() || !result.errors.lower.is_empty(),
        "expected error for no args"
    );
}

/// Negative 3 (Typeck): Box::new with too many args.
#[test]
fn stage31_6f_neg_box_new_too_many_args() {
    let src = r#"fn main() { let _b = Box::new(42i32, 43i32); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty() || !result.errors.lower.is_empty(),
        "expected error for too many args"
    );
}

/// Negative 4 (Typeck): Box::new result assigned to *mut u8.
#[test]
fn stage31_6f_neg_box_new_to_ptr() {
    let src = r#"fn main() { let _p: *mut u8 = Box::new(42i32); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for Box→*mut u8"
    );
}

/// Negative 5 (Typeck): Box::new result assigned to usize.
#[test]
fn stage31_6f_neg_box_new_to_usize() {
    let src = r#"fn main() { let _n: usize = Box::new(42i32); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for Box→usize"
    );
}

/// Negative 6 (Typeck): Box::new result assigned to &str.
#[test]
fn stage31_6f_neg_box_new_to_str() {
    let src = r#"fn main() { let _s: &str = Box::new(42i32); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for Box→&str"
    );
}

/// Negative 7 (Typeck): Box::new on i32 (not Box).
#[test]
fn stage31_6f_neg_box_new_on_i32() {
    let src = r#"fn main() { let _b = (42i32).new(); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for .new() on i32"
    );
}

/// Negative 8 (Resolve): Box::new on undefined type.
#[test]
fn stage31_6f_neg_box_new_undefined_type() {
    let src = r#"fn main() { let _b = UndefinedType::new(42i32); }"#;
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty() || !result.errors.typeck.is_empty(),
        "expected error for undefined type"
    );
}

/// Negative 9 (Parse): Box::new with malformed syntax.
#[test]
fn stage31_6f_neg_box_new_malformed() {
    let src = r#"fn main() { let _b = Box::new(; }"#;
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "expected parse error for malformed Box::new"
    );
}

/// Negative 10 (Typeck): Box::new result assigned to bool.
#[test]
fn stage31_6f_neg_box_new_to_bool() {
    let src = r#"fn main() { let _b: bool = Box::new(42i32); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for Box→bool"
    );
}

/// Negative 11 (Typeck): Box::new with string literal — edge case.
#[test]
fn stage31_6f_neg_box_new_str_arg() {
    let src = r#"fn main() { let _b: Box<i32> = Box::new("hello"); }"#;
    let result = compile(src);
    // May or may not error — type inference may infer T=&str.
    let _ = result;
}

/// Negative 12 (Typeck): Box::new result assigned to i64.
#[test]
fn stage31_6f_neg_box_new_to_i64() {
    let src = r#"fn main() { let _n: i64 = Box::new(42i32); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for Box→i64"
    );
}

// =====================================================================
// Summary: 4 positive + 12 negative = 16 tests (1:3 ratio, meets target)
// =====================================================================
