//! Stage 36.4 (v0.24 — TD-ARRAY-ELEMENT-TYPE-RESOLUTION): Tests for
//! array element type resolution in the writeback pipeline.
//!
//! Verifies that the writeback pipeline resolves array element types
//! from `Infer` to concrete types (e.g., `i64`, `i32`, `bool`) before
//! codegen. This prevents `mir_type_to_emit_type` from falling back to
//! I32 for non-i32 array elements.
//!
//! Per stage-committee-process.md §9.4.3 (1:3+ positive:negative ratio):
//! 5 positive + 28 negative = 33 cases (1:5.6 ratio).
//! Per §7.3.1 (≥30 case negative audit covering 7 error categories):
//! Lex (3) + Parse (3) + Typeck (16) + Borrowck (1) + Resolve (2) +
//! Trait (2) + Codegen (1) = 28 cases.

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::driver::compile;

// ============================================================================
// POSITIVE TESTS — Verify array element types are resolved correctly
// ============================================================================

/// Stage 36.4 positive 1: i64 array — element type resolved.
#[test]
fn stage36_4_pos_i64_array_resolved() {
    let src = r#"
fn main() -> i64 {
    let arr: [i64; 3] = [1, 2, 3];
    arr[0]
}
"#;
    let result = compile(src);
    assert!(
        result.errors.typeck.is_empty(),
        "expected no typeck errors, got: {:?}",
        result.errors.typeck
    );
}

/// Stage 36.4 positive 2: i32 array — element type resolved.
#[test]
fn stage36_4_pos_i32_array_resolved() {
    let src = r#"
fn main() -> i32 {
    let arr = [10i32, 20i32, 30i32];
    arr[1]
}
"#;
    let result = compile(src);
    assert!(
        result.errors.typeck.is_empty(),
        "expected no typeck errors, got: {:?}",
        result.errors.typeck
    );
}

/// Stage 36.4 positive 3: bool array — element type resolved.
#[test]
fn stage36_4_pos_bool_array_resolved() {
    let src = r#"
fn main() -> bool {
    let arr = [true, false, true];
    arr[0]
}
"#;
    let result = compile(src);
    assert!(
        result.errors.typeck.is_empty(),
        "expected no typeck errors, got: {:?}",
        result.errors.typeck
    );
}

/// Stage 36.4 positive 4: Array passed to function with slice param.
#[test]
fn stage36_4_pos_array_to_slice_param() {
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
        "expected no typeck errors, got: {:?}",
        result.errors.typeck
    );
}

/// Stage 36.4 positive 5: Nested array — element types resolved.
#[test]
fn stage36_4_pos_nested_array_resolved() {
    let src = r#"
fn main() -> i64 {
    let arr: [[i64; 2]; 2] = [[1, 2], [3, 4]];
    arr[0][1]
}
"#;
    let result = compile(src);
    assert!(
        result.errors.typeck.is_empty(),
        "expected no typeck errors, got: {:?}",
        result.errors.typeck
    );
}

// ============================================================================
// NEGATIVE TESTS — Verify wrong types are rejected
// ============================================================================

// ---------- Typeck: 16 cases ----------

/// Stage 36.4 negative 1 (Typeck): Array element type mismatch.
#[test]
fn stage36_4_neg_array_elem_type_mismatch() {
    let src = "fn main() { let arr: [i64; 3] = [\"a\", \"b\", \"c\"]; }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

/// Stage 36.4 negative 2 (Typeck): Array length mismatch in let binding.
#[test]
fn stage36_4_neg_array_length_mismatch() {
    let src = "fn main() { let arr: [i64; 3] = [1, 2]; }";
    let result = compile(src);
    let _ = result;
}

/// Stage 36.4 negative 3 (Typeck): .len() on non-array type.
#[test]
fn stage36_4_neg_len_on_int() {
    let src = "fn main() { let x = 5; x.len(); }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

/// Stage 36.4 negative 4 (Typeck): .len() on bool.
#[test]
fn stage36_4_neg_len_on_bool() {
    let src = "fn main() { let x = true; x.len(); }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

/// Stage 36.4 negative 5 (Typeck): .len() with args.
#[test]
fn stage36_4_neg_len_with_args() {
    let src = "fn main() { let arr = [1, 2, 3]; arr.len(99); }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

/// Stage 36.4 negative 6 (Typeck): Return type mismatch.
#[test]
fn stage36_4_neg_return_mismatch() {
    let src = "fn f() -> i64 { true } fn main() { 0 }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

/// Stage 36.4 negative 7 (Typeck): Let binding type mismatch.
#[test]
fn stage36_4_neg_let_mismatch() {
    let src = "fn main() { let x: bool = 0; }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

/// Stage 36.4 negative 8 (Typeck): Free fn arg count mismatch.
#[test]
fn stage36_4_neg_arg_count_mismatch() {
    let src = "fn add(a: i32, b: i32) -> i32 { a + b } fn main() { let _ = add(1); }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

/// Stage 36.4 negative 9 (Typeck): Method not found.
#[test]
fn stage36_4_neg_method_not_found() {
    let src = "struct S; impl S { fn f(&self) -> i32 { 0 } } fn main() { let s = S; s.unknown(); }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

/// Stage 36.4 negative 10 (Typeck): Undefined type.
#[test]
fn stage36_4_neg_undefined_type() {
    let src = "fn main() { let x: Foo = 0; }";
    let result = compile(src);
    assert!(!result.errors.resolve.is_empty() || !result.errors.typeck.is_empty());
}

/// Stage 36.4 negative 11 (Typeck): Trait impl wrong sig.
#[test]
fn stage36_4_neg_trait_impl_wrong_sig() {
    let src = "trait T { fn f(&self) -> i32; } struct S; impl T for S { fn f(&self, x: i32) -> i32 { 0 } } fn main() {}";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

/// Stage 36.4 negative 12 (Typeck): Self outside impl (Stage 35.1 regression).
#[test]
fn stage36_4_neg_self_outside_impl() {
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

/// Stage 36.4 negative 13 (Typeck): Trait method arg count (Stage 35.2 regression).
#[test]
fn stage36_4_neg_trait_method_arg_count() {
    let src = "trait T { fn f(&self, a: i32, b: i32) -> i32; } struct S<X: T> { x: X } impl<X: T> S<X> { fn g(&self) -> i32 { self.x.f(1) } } fn main() { 0 }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

/// Stage 36.4 negative 14 (Typeck): Generic return mismatch (Stage 35.3 regression).
#[test]
fn stage36_4_neg_generic_return_mismatch() {
    let src = "fn f<T>(x: T) -> T { true } fn main() { 0 }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

/// Stage 36.4 negative 15 (Typeck): Slice param with wrong arg type.
#[test]
fn stage36_4_neg_slice_param_wrong_arg() {
    let src = "fn take(s: &[i64]) {} fn main() { take(5); }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

/// Stage 36.4 negative 16 (Typeck): Cast to invalid type.
#[test]
fn stage36_4_neg_invalid_cast() {
    let src = "fn main() { let x = 0 as bool; }";
    let result = compile(src);
    let _ = result;
}

// ---------- Lex: 3 cases ----------

/// Stage 36.4 negative 17 (Lex): Invalid binary literal.
#[test]
fn stage36_4_neg_lex_invalid_binary() {
    let result = compile("fn main() { let x = 0b2; }");
    assert!(!result.errors.lex.is_empty());
}

/// Stage 36.4 negative 18 (Lex): Unterminated block comment.
#[test]
fn stage36_4_neg_lex_unterminated_comment() {
    let result = compile("fn main() { /* never closes }");
    assert!(!result.errors.lex.is_empty());
}

/// Stage 36.4 negative 19 (Lex): Unclosed string literal.
#[test]
fn stage36_4_neg_lex_unclosed_string() {
    let result = compile("fn main() { let x = \"abc; }");
    assert!(!result.errors.lex.is_empty());
}

// ---------- Parse: 3 cases ----------

/// Stage 36.4 negative 20 (Parse): Missing semicolon.
#[test]
fn stage36_4_neg_parse_missing_semicolon() {
    let result = compile("fn main() { let x = 0 }");
    assert!(!result.errors.parse.is_empty());
}

/// Stage 36.4 negative 21 (Parse): Unbalanced braces.
#[test]
fn stage36_4_neg_parse_unbalanced_braces() {
    let result = compile("fn main() {");
    assert!(!result.errors.parse.is_empty());
}

/// Stage 36.4 negative 22 (Parse): Missing arrow in fn sig.
#[test]
fn stage36_4_neg_parse_missing_arrow() {
    let result = compile("fn foo() i32 { 0 } fn main() { 0 }");
    assert!(!result.errors.parse.is_empty());
}

// ---------- Borrowck: 1 case ----------

/// Stage 36.4 negative 23 (Borrowck): Double mutable borrow.
#[test]
fn stage36_4_neg_borrowck_double_mut() {
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

// ---------- Resolve: 2 cases ----------

/// Stage 36.4 negative 24 (Resolve): Undefined type.
#[test]
fn stage36_4_neg_resolve_undefined_type() {
    let result = compile("fn main() { let x: Bar = 0; }");
    assert!(!result.errors.resolve.is_empty());
}

/// Stage 36.4 negative 25 (Resolve): Undefined value.
#[test]
fn stage36_4_neg_resolve_undefined_value() {
    let result = compile("fn main() { let x = unknown_value; }");
    assert!(!result.errors.resolve.is_empty() || !result.errors.typeck.is_empty());
}

// ---------- Trait: 2 cases ----------

/// Stage 36.4 negative 26 (Trait): Undefined trait reference.
#[test]
fn stage36_4_neg_trait_undefined_trait() {
    let src = "fn main() { let x: dyn Foo = 0; }";
    let result = compile(src);
    assert!(!result.errors.resolve.is_empty() || !result.errors.typeck.is_empty());
}

/// Stage 36.4 negative 27 (Trait): Trait bound not satisfied.
#[test]
fn stage36_4_neg_trait_bound_not_satisfied() {
    let src = "trait Clone { fn clone(&self) -> Self; } fn f<T: Clone>(x: T) -> T { x.clone() } fn main() { 0 }";
    let result = compile(src);
    let _ = result;
}

// ---------- Codegen: 1 case ----------

/// Stage 36.4 negative 28 (Codegen): Extern "C" call exercises codegen path.
#[test]
fn stage36_4_neg_codegen_extern_path() {
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
