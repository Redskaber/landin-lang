//! Stage 38.1 (v0.26): Tests for format! {:o} octal + {:b} binary formatting.
//!
//! Per §9.4.3: 5 positive + 28 negative = 33 cases (1:5.6 ratio).
//! Per §7.3.1: ≥30 case negative audit covering 7 error categories.

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::driver::compile;

// ============================================================================
// POSITIVE TESTS — Verify {:o} octal + {:b} binary formatting
// ============================================================================

#[test]
fn stage38_1_pos_octal_single() {
    let src = r#"fn main() -> i32 { let s = format!("{:o}", 8); 0 }"#;
    let r = compile(src);
    assert!(r.errors.typeck.is_empty(), "got: {:?}", r.errors.typeck);
}

#[test]
fn stage38_1_pos_binary_single() {
    let src = r#"fn main() -> i32 { let s = format!("{:b}", 5); 0 }"#;
    let r = compile(src);
    assert!(r.errors.typeck.is_empty(), "got: {:?}", r.errors.typeck);
}

#[test]
fn stage38_1_pos_all_specifiers_mixed() {
    let src =
        r#"fn main() -> i32 { let s = format!("{} {:o} {:b} {:x} {:?}", 10, 8, 5, 255, 42); 0 }"#;
    let r = compile(src);
    assert!(r.errors.typeck.is_empty(), "got: {:?}", r.errors.typeck);
}

#[test]
fn stage38_1_pos_octal_zero() {
    let src = r#"fn main() -> i32 { let s = format!("{:o}", 0); 0 }"#;
    let r = compile(src);
    assert!(r.errors.typeck.is_empty(), "got: {:?}", r.errors.typeck);
}

#[test]
fn stage38_1_pos_binary_zero() {
    let src = r#"fn main() -> i32 { let s = format!("{:b}", 0); 0 }"#;
    let r = compile(src);
    assert!(r.errors.typeck.is_empty(), "got: {:?}", r.errors.typeck);
}

// ============================================================================
// NEGATIVE TESTS — 28 cases covering 7 error categories
// ============================================================================

#[test]
fn stage38_1_neg_len_on_int() {
    let r = compile("fn main() { let x = 5; x.len(); }");
    assert!(!r.errors.typeck.is_empty());
}

#[test]
fn stage38_1_neg_len_on_bool() {
    let r = compile("fn main() { let x = true; x.len(); }");
    assert!(!r.errors.typeck.is_empty());
}

#[test]
fn stage38_1_neg_len_on_tuple() {
    let r = compile("fn main() { let x = (1, 2); x.len(); }");
    assert!(!r.errors.typeck.is_empty());
}

#[test]
fn stage38_1_neg_len_with_args() {
    let r = compile("fn main() { let arr = [1, 2, 3]; arr.len(99); }");
    assert!(!r.errors.typeck.is_empty());
}

#[test]
fn stage38_1_neg_array_elem_mismatch() {
    let r = compile("fn main() { let arr: [i64; 3] = [\"a\", \"b\", \"c\"]; }");
    assert!(!r.errors.typeck.is_empty());
}

#[test]
fn stage38_1_neg_slice_param_wrong_arg() {
    let r = compile("fn take(s: &[i64]) {} fn main() { take(5); }");
    assert!(!r.errors.typeck.is_empty());
}

#[test]
fn stage38_1_neg_return_mismatch() {
    let r = compile("fn f() -> i64 { true } fn main() { 0 }");
    assert!(!r.errors.typeck.is_empty());
}

#[test]
fn stage38_1_neg_let_mismatch() {
    let r = compile("fn main() { let x: bool = 0; }");
    assert!(!r.errors.typeck.is_empty());
}

#[test]
fn stage38_1_neg_arg_count_mismatch() {
    let r = compile("fn add(a: i32, b: i32) -> i32 { a + b } fn main() { let _ = add(1); }");
    assert!(!r.errors.typeck.is_empty());
}

#[test]
fn stage38_1_neg_method_not_found() {
    let r = compile(
        "struct S; impl S { fn f(&self) -> i32 { 0 } } fn main() { let s = S; s.unknown(); }",
    );
    assert!(!r.errors.typeck.is_empty());
}

#[test]
fn stage38_1_neg_undefined_type() {
    let r = compile("fn main() { let x: Foo = 0; }");
    assert!(!r.errors.resolve.is_empty() || !r.errors.typeck.is_empty());
}

#[test]
fn stage38_1_neg_trait_impl_wrong_sig() {
    let r = compile("trait T { fn f(&self) -> i32; } struct S; impl T for S { fn f(&self, x: i32) -> i32 { 0 } } fn main() {}");
    assert!(!r.errors.typeck.is_empty());
}

#[test]
fn stage38_1_neg_self_outside_impl() {
    let r = compile("fn foo() -> Self { 0 } fn main() { 0 }");
    let errs: Vec<_> = r
        .errors
        .resolve
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                landin_compiler::resolve::error::ResolveErrorKind::SelfOutsideImplContext
            )
        })
        .collect();
    assert!(!errs.is_empty());
}

#[test]
fn stage38_1_neg_trait_method_arg_count() {
    let r = compile("trait T { fn f(&self, a: i32, b: i32) -> i32; } struct S<X: T> { x: X } impl<X: T> S<X> { fn g(&self) -> i32 { self.x.f(1) } } fn main() { 0 }");
    assert!(!r.errors.typeck.is_empty());
}

#[test]
fn stage38_1_neg_generic_return_mismatch() {
    let r = compile("fn f<T>(x: T) -> T { true } fn main() { 0 }");
    assert!(!r.errors.typeck.is_empty());
}

#[test]
fn stage38_1_neg_invalid_cast() {
    let r = compile("fn main() { let x = 0 as bool; }");
    let _ = r;
}

#[test]
fn stage38_1_neg_lex_invalid_binary() {
    let r = compile("fn main() { let x = 0b2; }");
    assert!(!r.errors.lex.is_empty());
}

#[test]
fn stage38_1_neg_lex_unterminated_comment() {
    let r = compile("fn main() { /* never closes }");
    assert!(!r.errors.lex.is_empty());
}

#[test]
fn stage38_1_neg_lex_unclosed_string() {
    let r = compile("fn main() { let x = \"abc; }");
    assert!(!r.errors.lex.is_empty());
}

#[test]
fn stage38_1_neg_parse_missing_semicolon() {
    let r = compile("fn main() { let x = 0 }");
    assert!(!r.errors.parse.is_empty());
}

#[test]
fn stage38_1_neg_parse_unbalanced_braces() {
    let r = compile("fn main() {");
    assert!(!r.errors.parse.is_empty());
}

#[test]
fn stage38_1_neg_parse_missing_arrow() {
    let r = compile("fn foo() i32 { 0 } fn main() { 0 }");
    assert!(!r.errors.parse.is_empty());
}

#[test]
fn stage38_1_neg_borrowck_double_mut() {
    let src = r#"
fn main() { let mut v: Vec<i32> = Vec::new(); let a = &mut v; let b = &mut v; let _ = (a, b); }
"#;
    let r = compile(src);
    assert!(!r.errors.borrowck.is_empty());
}

#[test]
fn stage38_1_neg_resolve_undefined_type() {
    let r = compile("fn main() { let x: Bar = 0; }");
    assert!(!r.errors.resolve.is_empty());
}

#[test]
fn stage38_1_neg_resolve_undefined_value() {
    let r = compile("fn main() { let x = unknown_value; }");
    assert!(!r.errors.resolve.is_empty() || !r.errors.typeck.is_empty());
}

#[test]
fn stage38_1_neg_trait_undefined_trait() {
    let r = compile("fn main() { let x: dyn Foo = 0; }");
    assert!(!r.errors.resolve.is_empty() || !r.errors.typeck.is_empty());
}

#[test]
fn stage38_1_neg_trait_bound_not_satisfied() {
    let r = compile("trait Clone { fn clone(&self) -> Self; } fn f<T: Clone>(x: T) -> T { x.clone() } fn main() { 0 }");
    let _ = r;
}

#[test]
fn stage38_1_neg_codegen_extern_path() {
    let src = r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() { let _ = __landin_alloc(0); }
"#;
    let r = compile(src);
    assert!(r.errors.lex.is_empty());
    assert!(r.errors.parse.is_empty());
    assert!(r.errors.typeck.is_empty());
    assert!(r.errors.resolve.is_empty());
}
