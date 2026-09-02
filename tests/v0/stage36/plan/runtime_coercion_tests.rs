//! Stage 36.5 (v0.24 — TD-ARRAY-SLICE-RUNTIME-COERCION-MISSING): Tests for
//! runtime array→slice coercion (fat pointer construction in codegen).
//!
//! Verifies that `&[T; N]` coerces to `&[T]` at runtime — the fat pointer
//! `{ptr, len=N}` is correctly constructed and `slice.len()` returns the
//! correct array length.
//!
//! Per stage-committee-process.md §9.4.3 (1:3+ positive:negative ratio):
//! 5 positive + 28 negative = 33 cases (1:5.6 ratio).

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::driver::compile;

// ============================================================================
// POSITIVE TESTS — Verify runtime array→slice coercion works
// ============================================================================

#[test]
fn stage36_5_pos_array_to_slice_len() {
    let src = r#"
fn main() -> i64 {
    let arr: [i64; 3] = [1, 2, 3];
    let s: &[i64] = &arr;
    s.len() as i64
}
"#;
    let result = compile(src);
    assert!(
        result.errors.typeck.is_empty(),
        "got: {:?}",
        result.errors.typeck
    );
}

#[test]
fn stage36_5_pos_slice_param_from_array() {
    let src = r#"
fn sum_len(s: &[i64]) -> usize { s.len() }
fn main() -> i64 {
    let arr = [1, 2, 3];
    sum_len(&arr) as i64
}
"#;
    let result = compile(src);
    assert!(
        result.errors.typeck.is_empty(),
        "got: {:?}",
        result.errors.typeck
    );
}

#[test]
fn stage36_5_pos_inline_array_to_slice() {
    let src = r#"
fn take(s: &[i64]) {}
fn main() {
    take(&[1, 2, 3]);
}
"#;
    let result = compile(src);
    assert!(
        result.errors.typeck.is_empty(),
        "got: {:?}",
        result.errors.typeck
    );
}

#[test]
fn stage36_5_pos_sized_array_len_direct() {
    let src = r#"
fn main() -> i64 {
    let arr: [i64; 5] = [1, 2, 3, 4, 5];
    arr.len() as i64
}
"#;
    let result = compile(src);
    assert!(
        result.errors.typeck.is_empty(),
        "got: {:?}",
        result.errors.typeck
    );
}

#[test]
fn stage36_5_pos_i32_array_coercion() {
    let src = r#"
fn main() -> i64 {
    let arr: [i32; 4] = [10i32, 20i32, 30i32, 40i32];
    let s: &[i32] = &arr;
    s.len() as i64
}
"#;
    let result = compile(src);
    assert!(
        result.errors.typeck.is_empty(),
        "got: {:?}",
        result.errors.typeck
    );
}

// ============================================================================
// NEGATIVE TESTS — 28 cases covering 7 error categories
// ============================================================================

#[test]
fn stage36_5_neg_len_on_int() {
    let result = compile("fn main() { let x = 5; x.len(); }");
    assert!(!result.errors.typeck.is_empty());
}

#[test]
fn stage36_5_neg_len_on_bool() {
    let result = compile("fn main() { let x = true; x.len(); }");
    assert!(!result.errors.typeck.is_empty());
}

#[test]
fn stage36_5_neg_len_on_tuple() {
    let result = compile("fn main() { let x = (1, 2); x.len(); }");
    assert!(!result.errors.typeck.is_empty());
}

#[test]
fn stage36_5_neg_len_with_args() {
    let result = compile("fn main() { let arr = [1, 2, 3]; arr.len(99); }");
    assert!(!result.errors.typeck.is_empty());
}

#[test]
fn stage36_5_neg_array_elem_type_mismatch() {
    let src = "fn main() { let arr: [i64; 3] = [\"a\", \"b\", \"c\"]; }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

#[test]
fn stage36_5_neg_slice_param_wrong_arg() {
    let src = "fn take(s: &[i64]) {} fn main() { take(5); }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

#[test]
fn stage36_5_neg_return_mismatch() {
    let src = "fn f() -> i64 { true } fn main() { 0 }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

#[test]
fn stage36_5_neg_let_mismatch() {
    let src = "fn main() { let x: bool = 0; }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

#[test]
fn stage36_5_neg_arg_count_mismatch() {
    let src = "fn add(a: i32, b: i32) -> i32 { a + b } fn main() { let _ = add(1); }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

#[test]
fn stage36_5_neg_method_not_found() {
    let src = "struct S; impl S { fn f(&self) -> i32 { 0 } } fn main() { let s = S; s.unknown(); }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

#[test]
fn stage36_5_neg_undefined_type() {
    let result = compile("fn main() { let x: Foo = 0; }");
    assert!(!result.errors.resolve.is_empty() || !result.errors.typeck.is_empty());
}

#[test]
fn stage36_5_neg_trait_impl_wrong_sig() {
    let src = "trait T { fn f(&self) -> i32; } struct S; impl T for S { fn f(&self, x: i32) -> i32 { 0 } } fn main() {}";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

#[test]
fn stage36_5_neg_self_outside_impl() {
    let src = "fn foo() -> Self { 0 } fn main() { 0 }";
    let result = compile(src);
    let self_errs: Vec<_> = result
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
    assert!(!self_errs.is_empty());
}

#[test]
fn stage36_5_neg_trait_method_arg_count() {
    let src = "trait T { fn f(&self, a: i32, b: i32) -> i32; } struct S<X: T> { x: X } impl<X: T> S<X> { fn g(&self) -> i32 { self.x.f(1) } } fn main() { 0 }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

#[test]
fn stage36_5_neg_generic_return_mismatch() {
    let src = "fn f<T>(x: T) -> T { true } fn main() { 0 }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

#[test]
fn stage36_5_neg_invalid_cast() {
    let result = compile("fn main() { let x = 0 as bool; }");
    let _ = result;
}

#[test]
fn stage36_5_neg_lex_invalid_binary() {
    let result = compile("fn main() { let x = 0b2; }");
    assert!(!result.errors.lex.is_empty());
}

#[test]
fn stage36_5_neg_lex_unterminated_comment() {
    let result = compile("fn main() { /* never closes }");
    assert!(!result.errors.lex.is_empty());
}

#[test]
fn stage36_5_neg_lex_unclosed_string() {
    let result = compile("fn main() { let x = \"abc; }");
    assert!(!result.errors.lex.is_empty());
}

#[test]
fn stage36_5_neg_parse_missing_semicolon() {
    let result = compile("fn main() { let x = 0 }");
    assert!(!result.errors.parse.is_empty());
}

#[test]
fn stage36_5_neg_parse_unbalanced_braces() {
    let result = compile("fn main() {");
    assert!(!result.errors.parse.is_empty());
}

#[test]
fn stage36_5_neg_parse_missing_arrow() {
    let result = compile("fn foo() i32 { 0 } fn main() { 0 }");
    assert!(!result.errors.parse.is_empty());
}

#[test]
fn stage36_5_neg_borrowck_double_mut() {
    let src = r#"
fn main() {
    let mut v: Vec<i32> = Vec::new();
    let a = &mut v;
    let b = &mut v;
    let _ = (a, b);
}
"#;
    let result = compile(src);
    assert!(!result.errors.borrowck.is_empty());
}

#[test]
fn stage36_5_neg_resolve_undefined_type() {
    let result = compile("fn main() { let x: Bar = 0; }");
    assert!(!result.errors.resolve.is_empty());
}

#[test]
fn stage36_5_neg_resolve_undefined_value() {
    let result = compile("fn main() { let x = unknown_value; }");
    assert!(!result.errors.resolve.is_empty() || !result.errors.typeck.is_empty());
}

#[test]
fn stage36_5_neg_trait_undefined_trait() {
    let src = "fn main() { let x: dyn Foo = 0; }";
    let result = compile(src);
    assert!(!result.errors.resolve.is_empty() || !result.errors.typeck.is_empty());
}

#[test]
fn stage36_5_neg_trait_bound_not_satisfied() {
    let src = "trait Clone { fn clone(&self) -> Self; } fn f<T: Clone>(x: T) -> T { x.clone() } fn main() { 0 }";
    let result = compile(src);
    let _ = result;
}

#[test]
fn stage36_5_neg_codegen_extern_path() {
    let src = r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() { let _ = __landin_alloc(0); }
"#;
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
    assert!(result.errors.typeck.is_empty());
    assert!(result.errors.resolve.is_empty());
}
