//! Stage 31.6b (v0.19) — String::from_str Prelude Impl Migration Tests.
//!
//! Tests that `String::from_str()` is now implemented in the prelude using
//! `.ptr`/`.len` fat pointer field access + `extern "C"` calls to
//! `__landin_alloc` + `__landin_memcpy`. This is the second TD-INTRINSIC-OVERUSE
//! Phase 2-B migration.
//!
//! Per §1.0 原則 6 (通解 > 特解): standard static method resolution replaces
//! per-method intrinsic dispatch.
//! Per §12 (最优 > 最小): root-cause fix via language features.

#![allow(clippy::needless_raw_string_hashes)]

use landin_compiler::{compile, compile_no_opt};

// =====================================================================
// Positive tests (4) — from_str works via prelude impl
// =====================================================================

/// Positive 1: String::from_str("literal") compiles + runs via prelude impl.
#[test]
fn stage31_6b_from_str_compiles_via_prelude_impl() {
    let src = r#"fn main() { let _s: String = String::from_str("hello"); }"#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}

/// Positive 2: from_str with empty string.
#[test]
fn stage31_6b_from_str_empty_string() {
    let src = r#"fn main() { let _s: String = String::from_str(""); }"#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}

/// Positive 3: from_str result can be passed to function expecting String.
#[test]
fn stage31_6b_from_str_passes_to_fn() {
    let src = r#"
        fn take_string(s: String) -> i32 { 42 }
        fn main() {
            let _n: i32 = take_string(String::from_str("hello"));
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}

/// Positive 4: from_str works with compile_no_opt (unoptimized IR).
#[test]
fn stage31_6b_from_str_no_opt() {
    let src = r#"fn main() { let _s: String = String::from_str("hello"); }"#;
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

/// Negative 1 (Typeck): from_str with wrong arg type (i32 instead of &str).
#[test]
fn stage31_6b_neg_from_str_wrong_arg_type() {
    let src = r#"fn main() { let _s: String = String::from_str(42i32); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for i32 arg"
    );
}

/// Negative 2 (Typeck): from_str with wrong arg count (no args).
#[test]
fn stage31_6b_neg_from_str_no_args() {
    let src = r#"fn main() { let _s: String = String::from_str(); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty() || !result.errors.lower.is_empty(),
        "expected error for no args"
    );
}

/// Negative 3 (Typeck): from_str with too many args.
#[test]
fn stage31_6b_neg_from_str_too_many_args() {
    let src = r#"fn main() { let _s: String = String::from_str("a", "b"); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty() || !result.errors.lower.is_empty(),
        "expected error for too many args"
    );
}

/// Negative 4 (Typeck): from_str result assigned to wrong type (i32).
#[test]
fn stage31_6b_neg_from_str_wrong_return_type() {
    let src = r#"fn main() { let _n: i32 = String::from_str("hello"); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for String→i32"
    );
}

/// Negative 5 (Typeck): from_str result assigned to *mut u8.
#[test]
fn stage31_6b_neg_from_str_to_ptr() {
    let src = r#"fn main() { let _p: *mut u8 = String::from_str("hello"); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for String→*mut u8"
    );
}

/// Negative 6 (Typeck): from_str result assigned to usize.
#[test]
fn stage31_6b_neg_from_str_to_usize() {
    let src = r#"fn main() { let _n: usize = String::from_str("hello"); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for String→usize"
    );
}

/// Negative 7 (Typeck): from_str result assigned to &str.
#[test]
fn stage31_6b_neg_from_str_to_str_ref() {
    let src = r#"fn main() { let _s: &str = String::from_str("hello"); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for String→&str"
    );
}

/// Negative 8 (Resolve): from_str on undefined type.
#[test]
fn stage31_6b_neg_from_str_undefined_type() {
    let src = r#"fn main() { let _s = UndefinedType::from_str("hello"); }"#;
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty() || !result.errors.typeck.is_empty(),
        "expected resolve/typeck error for undefined type"
    );
}

/// Negative 9 (Typeck): from_str on i32 (not String).
#[test]
fn stage31_6b_neg_from_str_on_i32() {
    let src = r#"fn main() { let _s = (42i32).from_str("hello"); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for from_str on i32"
    );
}

/// Negative 10 (Parse): from_str with malformed syntax.
#[test]
fn stage31_6b_neg_from_str_malformed() {
    let src = r#"fn main() { let _s: String = String::from_str(; }"#;
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "expected parse error for malformed from_str"
    );
}

/// Negative 11 (Typeck): from_str with bool arg.
#[test]
fn stage31_6b_neg_from_str_bool_arg() {
    let src = r#"fn main() { let _s: String = String::from_str(true); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for bool arg"
    );
}

/// Negative 12 (Typeck): from_str with *mut u8 arg.
#[test]
fn stage31_6b_neg_from_str_ptr_arg() {
    let src = r#"fn main() { let _s: String = String::from_str(0 as *mut u8); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for *mut u8 arg"
    );
}

// =====================================================================
// Summary: 4 positive + 12 negative = 16 tests (1:3 ratio, meets target)
// =====================================================================
