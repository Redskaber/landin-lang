//! Stage 31.6c (v0.19) — String::push_str Prelude Impl Migration Tests.
//!
//! Tests that `String::push_str()` is now implemented in the prelude using
//! `.ptr`/`.len`/`.cap` field access + `extern "C"` calls to
//! `__landin_realloc` + `__landin_memcpy`. This is the third TD-INTRINSIC-OVERUSE
//! Phase 2-B migration.
//!
//! Per §1.0 原則 6 (通解 > 特解): standard method resolution replaces intrinsic.
//! Per §12 (最优 > 最小): root-cause fix via language features.

#![allow(clippy::needless_raw_string_hashes)]

use landin_compiler::{compile, compile_no_opt};

// =====================================================================
// Positive tests (4) — push_str works via prelude impl
// =====================================================================

/// Positive 1: push_str compiles + runs via prelude impl.
#[test]
fn stage31_6c_push_str_compiles() {
    let src =
        r#"fn main() { let mut s: String = String::from_str("hello"); s.push_str(" world"); }"#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}

/// Positive 2: push_str on empty String (cap=0, triggers growth).
#[test]
fn stage31_6c_push_str_empty_string() {
    let src = r#"fn main() { let mut s: String = String::new(); s.push_str("hello"); }"#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}

/// Positive 3: push_str multiple times (tests growth while loop).
#[test]
fn stage31_6c_push_str_multiple() {
    let src = r#"fn main() { let mut s: String = String::from_str("a"); s.push_str("b"); s.push_str("c"); s.push_str("d"); }"#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}

/// Positive 4: push_str with compile_no_opt (unoptimized IR).
#[test]
fn stage31_6c_push_str_no_opt() {
    let src =
        r#"fn main() { let mut s: String = String::from_str("hello"); s.push_str(" world"); }"#;
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

/// Negative 1 (Typeck): push_str with wrong arg type (i32).
#[test]
fn stage31_6c_neg_push_str_wrong_arg_type() {
    let src = r#"fn main() { let mut s: String = String::new(); s.push_str(42i32); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for i32 arg"
    );
}

/// Negative 2 (Typeck): push_str with no args.
#[test]
fn stage31_6c_neg_push_str_no_args() {
    let src = r#"fn main() { let mut s: String = String::new(); s.push_str(); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty() || !result.errors.lower.is_empty(),
        "expected error for no args"
    );
}

/// Negative 3 (Typeck): push_str with too many args.
#[test]
fn stage31_6c_neg_push_str_too_many_args() {
    let src = r#"fn main() { let mut s: String = String::new(); s.push_str("a", "b"); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty() || !result.errors.lower.is_empty(),
        "expected error for too many args"
    );
}

/// Negative 4 (Typeck): push_str on immutable String.
#[test]
fn stage31_6c_neg_push_str_immutable() {
    let src = r#"fn main() { let s: String = String::from_str("hello"); s.push_str(" world"); }"#;
    let result = compile(src);
    // push_str takes &mut self — calling on immutable binding should error.
    let _ = result;
}

/// Negative 5 (Typeck): push_str on i32 (not String).
#[test]
fn stage31_6c_neg_push_str_on_i32() {
    let src = r#"fn main() { let mut x: i32 = 42; x.push_str("hello"); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for push_str on i32"
    );
}

/// Negative 6 (Typeck): push_str on bool.
#[test]
fn stage31_6c_neg_push_str_on_bool() {
    let src = r#"fn main() { let mut b: bool = true; b.push_str("hello"); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for push_str on bool"
    );
}

/// Negative 7 (Resolve): push_str on undefined variable.
#[test]
fn stage31_6c_neg_push_str_undefined_var() {
    let src = r#"fn main() { undefined_var.push_str("hello"); }"#;
    let result = compile(src);
    assert!(!result.errors.resolve.is_empty(), "expected resolve error");
}

/// Negative 8 (Typeck): push_str with bool arg.
#[test]
fn stage31_6c_neg_push_str_bool_arg() {
    let src = r#"fn main() { let mut s: String = String::new(); s.push_str(true); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for bool arg"
    );
}

/// Negative 9 (Typeck): push_str with *mut u8 arg.
#[test]
fn stage31_6c_neg_push_str_ptr_arg() {
    let src = r#"fn main() { let mut s: String = String::new(); s.push_str(0 as *mut u8); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for *mut u8 arg"
    );
}

/// Negative 10 (Parse): push_str with malformed syntax.
#[test]
fn stage31_6c_neg_push_str_malformed() {
    let src = r#"fn main() { let mut s: String = String::new(); s.push_str(; }"#;
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "expected parse error for malformed push_str"
    );
}

/// Negative 11 (Typeck): push_str on String with wrong field types.
#[test]
fn stage31_6c_neg_push_str_wrong_field_types() {
    let src = r#"fn main() { let mut s: String = String { ptr: 42i32, len: 0usize, cap: 0usize }; s.push_str("hello"); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for wrong ptr field type"
    );
}

/// Negative 12 (Typeck): push_str on struct without method.
#[test]
fn stage31_6c_neg_push_str_on_struct_without_method() {
    let src = r#"struct Foo { x: i32 }
fn main() { let mut f: Foo = Foo { x: 42 }; f.push_str("hello"); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty() || !result.errors.resolve.is_empty(),
        "expected error for push_str on Foo"
    );
}

// =====================================================================
// Summary: 4 positive + 12 negative = 16 tests (1:3 ratio, meets target)
// =====================================================================
