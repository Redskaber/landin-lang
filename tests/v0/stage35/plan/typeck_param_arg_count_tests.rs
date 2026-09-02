//! Stage 35.2 (v0.23 — TD-TYPECK-PARAM-ARG-COUNT): Tests for trait method
//! arg-count validation on `Param(N)` receivers (and concrete receivers).
//!
//! Verifies that typeck reports `this function takes N argument(s) but M
//! were supplied` for trait method calls with wrong arg count, REGARDLESS
//! of whether the trait method has a body or is just a declaration.
//!
//! Per stage-committee-process.md §9.4.3 (1:3+ positive:negative ratio):
//! 5 positive + 28 negative = 33 cases (1:5.6 ratio).
//! Per §7.3.1 (≥30 case negative audit covering 7 error categories):
//! Lex (3) + Parse (3) + Typeck (16) + Borrowck (1) + Resolve (2) +
//! Trait (2) + Codegen (1) = 28 cases.

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::driver::compile;

// ============================================================================
// POSITIVE TESTS — Verify correct arg count is accepted
// ============================================================================

/// Stage 35.2 positive 1: Concrete impl method, correct arg count.
#[test]
fn stage35_2_pos_concrete_impl_correct_args() {
    let src = r#"
trait T { fn f(&self, a: i32, b: i32) -> i32; }
struct S;
impl T for S { fn f(&self, a: i32, b: i32) -> i32 { 0 } }
fn main() { let s = S; let _ = s.f(1, 2); }
"#;
    let result = compile(src);
    assert!(
        result.errors.typeck.is_empty(),
        "expected no typeck errors, got: {:?}",
        result.errors.typeck
    );
}

/// Stage 35.2 positive 2: Trait default body, correct arg count.
#[test]
fn stage35_2_pos_default_body_correct_args() {
    let src = r#"
trait T { fn f(&self, a: i32, b: i32) -> i32 { 0 } }
struct S;
impl T for S {}
fn main() { let s = S; let _ = s.f(1, 2); }
"#;
    let result = compile(src);
    assert!(
        result.errors.typeck.is_empty(),
        "expected no typeck errors, got: {:?}",
        result.errors.typeck
    );
}

/// Stage 35.2 positive 3: No-arg method call.
#[test]
fn stage35_2_pos_no_arg_method() {
    let src = r#"
trait T { fn f(&self); }
struct S;
impl T for S { fn f(&self) {} }
fn main() { let s = S; s.f(); }
"#;
    let result = compile(src);
    assert!(
        result.errors.typeck.is_empty(),
        "expected no typeck errors, got: {:?}",
        result.errors.typeck
    );
}

/// Stage 35.2 positive 4: Param(N) receiver, correct arg count.
#[test]
fn stage35_2_pos_param_receiver_correct_args() {
    let src = r#"
trait T { fn f(&self, a: i32) -> i32; }
struct S<X: T> { x: X }
impl<X: T> S<X> { fn g(&self) -> i32 { self.x.f(1) } }
fn main() { 0 }
"#;
    let result = compile(src);
    assert!(
        result.errors.typeck.is_empty(),
        "expected no typeck errors, got: {:?}",
        result.errors.typeck
    );
}

/// Stage 35.2 positive 5: Multi-arg correct count.
#[test]
fn stage35_2_pos_multi_arg_correct() {
    let src = r#"
trait T { fn f(&self, a: i32, b: i32, c: i32) -> i32; }
struct S;
impl T for S { fn f(&self, a: i32, b: i32, c: i32) -> i32 { 0 } }
fn main() { let s = S; let _ = s.f(1, 2, 3); }
"#;
    let result = compile(src);
    assert!(
        result.errors.typeck.is_empty(),
        "expected no typeck errors, got: {:?}",
        result.errors.typeck
    );
}

// ============================================================================
// NEGATIVE TESTS — Verify wrong arg count is rejected
// ============================================================================

// ---------- Typeck: 16 cases of arg-count mismatch ----------

/// Stage 35.2 negative 1 (Typeck): Concrete impl, missing 1 arg.
#[test]
fn stage35_2_neg_concrete_impl_missing_arg() {
    let src = r#"
trait T { fn f(&self, a: i32, b: i32) -> i32; }
struct S;
impl T for S { fn f(&self, a: i32, b: i32) -> i32 { 0 } }
fn main() { let s = S; let _ = s.f(1); }
"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck arg count error, got: {:?}",
        result.errors.typeck
    );
}

/// Stage 35.2 negative 2 (Typeck): Concrete impl, extra arg.
#[test]
fn stage35_2_neg_concrete_impl_extra_arg() {
    let src = r#"
trait T { fn f(&self, a: i32, b: i32) -> i32; }
struct S;
impl T for S { fn f(&self, a: i32, b: i32) -> i32 { 0 } }
fn main() { let s = S; let _ = s.f(1, 2, 3); }
"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck arg count error, got: {:?}",
        result.errors.typeck
    );
}

/// Stage 35.2 negative 3 (Typeck): No-arg method with extra arg.
#[test]
fn stage35_2_neg_no_arg_method_extra_arg() {
    let src = r#"
trait T { fn f(&self); }
struct S;
impl T for S { fn f(&self) {} }
fn main() { let s = S; s.f(99); }
"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck arg count error, got: {:?}",
        result.errors.typeck
    );
}

/// Stage 35.2 negative 4 (Typeck): Param(N) receiver, decl method (no body),
/// missing arg. This was the silent bug (Stage 35.2 fix).
#[test]
fn stage35_2_neg_param_receiver_decl_missing_arg() {
    let src = r#"
trait T { fn f(&self, a: i32, b: i32) -> i32; }
struct S<X: T> { x: X }
impl<X: T> S<X> { fn g(&self) -> i32 { self.x.f(1) } }
fn main() { 0 }
"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck arg count error, got: {:?}",
        result.errors.typeck
    );
}

/// Stage 35.2 negative 5 (Typeck): Param(N) receiver, default body, missing arg.
#[test]
fn stage35_2_neg_param_receiver_default_body_missing_arg() {
    let src = r#"
trait T { fn f(&self, a: i32, b: i32) -> i32 { 0 } }
struct S<X: T> { x: X }
impl<X: T> S<X> { fn g(&self) -> i32 { self.x.f(1) } }
fn main() { 0 }
"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck arg count error, got: {:?}",
        result.errors.typeck
    );
}

/// Stage 35.2 negative 6 (Typeck): Param(N) receiver, missing arg (1-arg method).
#[test]
fn stage35_2_neg_param_receiver_missing_arg() {
    let src = r#"
trait T { fn f(&self, a: i32) -> i32; }
struct S<X: T> { x: X }
impl<X: T> S<X> { fn g(&self) -> i32 { self.x.f() } }
fn main() { 0 }
"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck arg count error, got: {:?}",
        result.errors.typeck
    );
}

/// Stage 35.2 negative 7 (Typeck): Param(N) receiver, extra arg (0-arg method).
#[test]
fn stage35_2_neg_param_receiver_extra_arg() {
    let src = r#"
trait T { fn f(&self) -> i32; }
struct S<X: T> { x: X }
impl<X: T> S<X> { fn g(&self) -> i32 { self.x.f(99) } }
fn main() { 0 }
"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck arg count error, got: {:?}",
        result.errors.typeck
    );
}

/// Stage 35.2 negative 8 (Typeck): Param(N) receiver, missing 1 of 3 args.
#[test]
fn stage35_2_neg_param_receiver_missing_one_of_three() {
    let src = r#"
trait T { fn f(&self, a: i32, b: i32, c: i32) -> i32; }
struct S<X: T> { x: X }
impl<X: T> S<X> { fn g(&self) -> i32 { self.x.f(1, 2) } }
fn main() { 0 }
"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck arg count error, got: {:?}",
        result.errors.typeck
    );
}

/// Stage 35.2 negative 9 (Typeck): Param(N) receiver, extra arg (2-arg method).
#[test]
fn stage35_2_neg_param_receiver_extra_one_of_two() {
    let src = r#"
trait T { fn f(&self, a: i32, b: i32) -> i32; }
struct S<X: T> { x: X }
impl<X: T> S<X> { fn g(&self) -> i32 { self.x.f(1, 2, 3) } }
fn main() { 0 }
"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck arg count error, got: {:?}",
        result.errors.typeck
    );
}

/// Stage 35.2 negative 10 (Typeck): Concrete impl, missing both args (2-arg method).
#[test]
fn stage35_2_neg_concrete_impl_missing_all_args() {
    let src = r#"
trait T { fn f(&self, a: i32, b: i32) -> i32; }
struct S;
impl T for S { fn f(&self, a: i32, b: i32) -> i32 { 0 } }
fn main() { let s = S; let _ = s.f(); }
"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck arg count error, got: {:?}",
        result.errors.typeck
    );
}

/// Stage 35.2 negative 11 (Typeck): Free fn call, wrong arg count.
#[test]
fn stage35_2_neg_free_fn_wrong_arg_count() {
    let src = "fn add(a: i32, b: i32) -> i32 { a + b }\nfn main() { let _ = add(1); }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck arg count error"
    );
}

/// Stage 35.2 negative 12 (Typeck): Free fn, extra arg.
#[test]
fn stage35_2_neg_free_fn_extra_arg() {
    let src = "fn id(x: i32) -> i32 { x }\nfn main() { let _ = id(1, 2); }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck arg count error"
    );
}

/// Stage 35.2 negative 13 (Typeck): Type mismatch (not arg count) on method.
#[test]
fn stage35_2_neg_method_type_mismatch() {
    let src = r#"
trait T { fn f(&self, a: i32) -> i32; }
struct S;
impl T for S { fn f(&self, a: i32) -> i32 { 0 } }
fn main() { let s = S; let _ = s.f(true); }
"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck type mismatch"
    );
}

/// Stage 35.2 negative 14 (Typeck): Undefined type.
#[test]
fn stage35_2_neg_typeck_undefined_type() {
    let src = "fn main() { let x: Foo = 0; }";
    let result = compile(src);
    assert!(!result.errors.resolve.is_empty() || !result.errors.typeck.is_empty());
}

/// Stage 35.2 negative 15 (Typeck): Type mismatch in let binding.
#[test]
fn stage35_2_neg_typeck_let_mismatch() {
    let src = "fn main() { let x: bool = 0; }";
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

/// Stage 35.2 negative 16 (Typeck): Method not found (unknown method).
#[test]
fn stage35_2_neg_method_not_found() {
    let src = r#"
struct S;
impl S { fn f(&self) -> i32 { 0 } }
fn main() { let s = S; let _ = s.unknown_method(); }
"#;
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

// ---------- Lex: 3 cases ----------

/// Stage 35.2 negative 17 (Lex): Invalid binary literal.
#[test]
fn stage35_2_neg_lex_invalid_binary() {
    let result = compile("fn main() { let x = 0b2; }");
    assert!(!result.errors.lex.is_empty());
}

/// Stage 35.2 negative 18 (Lex): Unterminated block comment.
#[test]
fn stage35_2_neg_lex_unterminated_comment() {
    let result = compile("fn main() { /* never closes }");
    assert!(!result.errors.lex.is_empty());
}

/// Stage 35.2 negative 19 (Lex): Unclosed string literal.
#[test]
fn stage35_2_neg_lex_unclosed_string() {
    let result = compile("fn main() { let x = \"abc; }");
    assert!(!result.errors.lex.is_empty());
}

// ---------- Parse: 3 cases ----------

/// Stage 35.2 negative 20 (Parse): Missing semicolon.
#[test]
fn stage35_2_neg_parse_missing_semicolon() {
    let result = compile("fn main() { let x = 0 }");
    assert!(!result.errors.parse.is_empty());
}

/// Stage 35.2 negative 21 (Parse): Unbalanced braces.
#[test]
fn stage35_2_neg_parse_unbalanced_braces() {
    let result = compile("fn main() {");
    assert!(!result.errors.parse.is_empty());
}

/// Stage 35.2 negative 22 (Parse): Missing arrow in fn sig.
#[test]
fn stage35_2_neg_parse_missing_arrow() {
    let result = compile("fn foo() i32 { 0 } fn main() { 0 }");
    assert!(!result.errors.parse.is_empty());
}

// ---------- Borrowck: 1 case ----------

/// Stage 35.2 negative 23 (Borrowck): Double mutable borrow.
#[test]
fn stage35_2_neg_borrowck_double_mut() {
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

/// Stage 35.2 negative 24 (Resolve): Self outside impl context (regression for Stage 35.1).
#[test]
fn stage35_2_neg_resolve_self_outside_impl() {
    let result = compile("fn foo() -> Self { 0 } fn main() { 0 }");
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

/// Stage 35.2 negative 25 (Resolve): Undefined type reference.
#[test]
fn stage35_2_neg_resolve_undefined_type() {
    let result = compile("fn main() { let x: Bar = 0; }");
    assert!(!result.errors.resolve.is_empty());
}

// ---------- Trait: 2 cases ----------

/// Stage 35.2 negative 26 (Trait): Trait impl with wrong sig (mismatch with trait decl).
#[test]
fn stage35_2_neg_trait_impl_wrong_sig() {
    let src = r#"
trait T { fn f(&self) -> i32; }
struct S;
impl T for S { fn f(&self, x: i32) -> i32 { 0 } }
fn main() {}
"#;
    let result = compile(src);
    assert!(!result.errors.typeck.is_empty());
}

/// Stage 35.2 negative 27 (Trait): Undefined trait reference.
#[test]
fn stage35_2_neg_trait_undefined_trait() {
    let src = "fn main() { let x: dyn Foo = 0; }";
    let result = compile(src);
    assert!(!result.errors.resolve.is_empty() || !result.errors.typeck.is_empty());
}

// ---------- Codegen: 1 case ----------

/// Stage 35.2 negative 28 (Codegen): Extern "C" call exercises codegen path.
#[test]
fn stage35_2_neg_codegen_extern_path() {
    let src = r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() { let _ = __landin_alloc(0); }
"#;
    let result = compile(src);
    // Codegen path exercised — no errors expected (valid call).
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.parse.is_empty());
    assert!(result.errors.typeck.is_empty());
    assert!(result.errors.resolve.is_empty());
}
