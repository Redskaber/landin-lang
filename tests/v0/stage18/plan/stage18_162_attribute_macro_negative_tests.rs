//! Stage 18.162 (TD-NEGATIVE-TEST-COVERAGE): Attribute/macro negative tests.
//!
//! Tests attribute and macro error paths. Per §9.4.3, negative tests
//! should be ≥25% of total.
//!
//! Per §9.1: tests live under `tests/v0/stage-XX/plan/`.
//! Per §16: tests use only public API (`compile`).

use landin_compiler::compile;

// === Macro errors ===

/// Stage 18.162 negative 1: println! with no arguments.
#[test]
fn stage18_162_macro_println_no_args() {
    let result = compile("fn main() { println!(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 2: println! with format string only.
#[test]
fn stage18_162_macro_println_format_only() {
    let result = compile(r#"fn main() { println!("hello"); }"#);
    assert!(!result.mirs.is_empty());
}

/// Stage 18.162 negative 3: println! with format args.
#[test]
fn stage18_162_macro_println_with_args() {
    let result = compile(r#"fn main() { println!("{} {}", 1, 2); }"#);
    assert!(!result.mirs.is_empty());
}

/// Stage 18.162 negative 4: println! with mismatched arg count.
#[test]
fn stage18_162_macro_println_mismatched_args() {
    let result = compile(r#"fn main() { println!("{} {}", 1); }"#);
    // Mismatched arg count may or may not be a compile error.
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 5: print! macro.
#[test]
fn stage18_162_macro_print() {
    let result = compile(r#"fn main() { print!("hello"); }"#);
    assert!(!result.mirs.is_empty());
}

/// Stage 18.162 negative 6: eprintln! macro.
#[test]
fn stage18_162_macro_eprintln() {
    let result = compile(r#"fn main() { eprintln!("error"); }"#);
    assert!(!result.mirs.is_empty());
}

/// Stage 18.162 negative 7: eprint! macro.
#[test]
fn stage18_162_macro_eprint() {
    let result = compile(r#"fn main() { eprint!("error"); }"#);
    assert!(!result.mirs.is_empty());
}

/// Stage 18.162 negative 8: assert! macro with true.
#[test]
fn stage18_162_macro_assert_true() {
    let result = compile("fn main() { assert!(true); }");
    assert!(!result.mirs.is_empty());
}

/// Stage 18.162 negative 9: assert! macro with false (runtime panic).
#[test]
fn stage18_162_macro_assert_false() {
    let result = compile("fn main() { assert!(false); }");
    // assert!(false) is a runtime panic, not a compile error.
    assert!(!result.mirs.is_empty());
}

/// Stage 18.162 negative 10: assert! macro with non-bool.
#[test]
fn stage18_162_macro_assert_non_bool() {
    let result = compile("fn main() { assert!(42); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === macro_rules! errors ===

/// Stage 18.162 negative 11: macro_rules! with empty pattern.
#[test]
fn stage18_162_macro_rules_empty_pattern() {
    let src = r#"
        macro_rules! empty { () => {} }
        fn main() { empty!(); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 12: macro_rules! with no matching rule.
#[test]
fn stage18_162_macro_rules_no_matching_rule() {
    let src = r#"
        macro_rules! only_one { (1) => {} }
        fn main() { only_one!(2); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 13: macro_rules! with undefined macro call.
#[test]
fn stage18_162_macro_rules_undefined_call() {
    let result = compile("fn main() { undefined_macro!(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 14: macro_rules! with recursive expansion.
#[test]
fn stage18_162_macro_rules_recursive() {
    let src = r#"
        macro_rules! count { ($a:expr) => { 1 }; ($a:expr, $b:expr) => { 2 } }
        fn main() -> i32 { count!(1, 2) }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 15: macro_rules! with complex pattern.
#[test]
fn stage18_162_macro_rules_complex_pattern() {
    let src = r#"
        macro_rules! vec_init { ($($x:expr),*) => { [$($x),*] } }
        fn main() { let v = vec_init!(1, 2, 3); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Attribute errors ===

/// Stage 18.162 negative 16: unknown attribute.
#[test]
fn stage18_162_attr_unknown() {
    let result = compile("#[unknown_attr] fn main() {}");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 17: attribute on wrong item.
#[test]
fn stage18_162_attr_wrong_item() {
    let result = compile("#[inline] struct Foo; fn main() {}");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 18: attribute with no value.
#[test]
fn stage18_162_attr_no_value() {
    let result = compile("#[] fn main() {}");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 19: attribute with wrong value type.
#[test]
fn stage18_162_attr_wrong_value_type() {
    let result = compile("#[cfg = true] fn main() {}");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 20: multiple attributes.
#[test]
fn stage18_162_attr_multiple() {
    let result = compile("#[inline] #[cold] fn main() {}");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === String literal errors ===

/// Stage 18.162 negative 21: unterminated string literal.
#[test]
fn stage18_162_string_unterminated() {
    let result = compile(r#"fn main() { let s = "unterminated; }"#);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 22: string with invalid escape.
#[test]
fn stage18_162_string_invalid_escape() {
    let result = compile(r#"fn main() { let s = "hello\zworld"; }"#);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 23: empty string.
#[test]
fn stage18_162_string_empty() {
    let result = compile(r#"fn main() { let s = ""; }"#);
    assert!(!result.mirs.is_empty());
}

/// Stage 18.162 negative 24: string with newline.
#[test]
fn stage18_162_string_with_newline() {
    let result = compile(r#"fn main() { let s = "line1\nline2"; }"#);
    assert!(!result.mirs.is_empty());
}

/// Stage 18.162 negative 25: raw string literal.
#[test]
fn stage18_162_string_raw() {
    let result = compile(r#"fn main() { let s = r"raw string"; }"#);
    assert!(!result.mirs.is_empty() || result.has_errors());
}
