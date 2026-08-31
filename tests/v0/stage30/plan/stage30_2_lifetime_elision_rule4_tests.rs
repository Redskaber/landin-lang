//! Stage 30.2 (v0.13 TD-STUB-LIFETIME-ELISION-NOOP): Lifetime elision
//! Rule 4 enforcement tests.
//!
//! Per stage-committee-process.md §9.4.3 (1:3+ positive:negative ratio):
//!   - 8 positive tests (rules 1/2/3 + explicit + no-output-ref + static)
//!   - 12 negative tests (rule 4 violations in various shapes)
//!   - 2 regression tests (over-application fix: explicit lifetimes preserved)
//!
//! Per §1.0 原則 4 (报错 > 静默): rule 4 must REJECT ambiguous output refs.
//! Per §1.0 原則 9 (正确 > 妥协): root-cause fix, not silent acceptance.
//! Per §1.0 原則 6 (通解 > 特解): one rule covers all ambiguous cases.
//!
//! Background — RFC 141 Lifetime Elision Rules:
//!   1. Each elided input lifetime → fresh lifetime parameter.
//!   2. Single input lifetime → all elided output lifetimes take it.
//!   3. Multiple input lifetimes with `&self`/`&mut self` → output takes
//!      self's lifetime.
//!   4. Otherwise (multiple inputs, no self, OR no inputs) → output
//!      reference lifetimes MUST be explicitly annotated.
//!
//! Before Stage 30.2, rule 4 was NOT enforced — `fn f() -> &str { ... }`
//! and `fn f(x: &i32, y: &i32) -> &i32 { x }` were silently accepted
//! (soundness gap). These tests verify the fix.

use landin_compiler::driver::compile;

/// Check that the given source compiles with zero typeck errors.
fn assert_compiles(src: &str) {
    let result = compile(src);
    let n_typeck = result.errors.typeck.len();
    assert_eq!(
        n_typeck,
        0,
        "expected zero typeck errors, got {}:\n{:?}",
        n_typeck,
        result
            .errors
            .typeck
            .iter()
            .map(|e| &e.message)
            .collect::<Vec<_>>()
    );
}

/// Check that the given source produces at least one typeck error containing
/// the specified substring (rule 4 missing lifetime specifier).
fn assert_rule4_error(src: &str) {
    let result = compile(src);
    let has_rule4 = result
        .errors
        .typeck
        .iter()
        .any(|e| e.message.contains("missing lifetime specifier"));
    assert!(
        has_rule4,
        "expected a 'missing lifetime specifier' typeck error, got {} errors:\n{:?}",
        result.errors.typeck.len(),
        result
            .errors
            .typeck
            .iter()
            .map(|e| &e.message)
            .collect::<Vec<_>>()
    );
}

// ============================================================================
// POSITIVE TESTS — Rule 1, 2, 3, explicit, static, no-output-ref
// ============================================================================

/// Stage 30.2 positive 1: Rule 2 — single input, elided output.
/// `fn f(x: &i32) -> &i32 { x }` should compile cleanly.
#[test]
fn stage30_2_positive_rule_2_single_input() {
    assert_compiles("fn f(x: &i32) -> &i32 { x } fn main() { let v = 42; let _ = f(&v); }");
}

/// Stage 30.2 positive 2: Rule 3 — &self + no other ref param.
/// `fn get(&self) -> &i32 { &self.x }` should compile cleanly.
#[test]
fn stage30_2_positive_rule_3_self_only() {
    assert_compiles(
        "struct S { x: i32 } impl Copy for S {} impl S { fn get(&self) -> &i32 { &self.x } } \
         fn main() { let s = S{x:42}; let _ = s.get(); }",
    );
}

/// Stage 30.2 positive 3: Rule 3 — &self + another ref param.
/// `fn get_or(&self, _d: &i32) -> &i32 { &self.x }` should compile cleanly
/// (rule 3 fires, output takes self's lifetime).
#[test]
fn stage30_2_positive_rule_3_self_with_arg() {
    assert_compiles(
        "struct S { x: i32 } impl Copy for S {} impl S { fn get_or(&self, _d: &i32) -> &i32 { &self.x } } \
         fn main() { let s = S{x:42}; let d = 0; let _ = s.get_or(&d); }",
    );
}

/// Stage 30.2 positive 4: Rule 2 with tuple return.
/// `fn pair(x: &i32) -> (&i32, &i32) { (x, x) }` should compile cleanly.
#[test]
fn stage30_2_positive_rule_2_tuple_return() {
    assert_compiles(
        "fn pair(x: &i32) -> (&i32, &i32) { (x, x) } fn main() { let v = 42; let (a, b) = pair(&v); let _ = *a + *b; }",
    );
}

/// Stage 30.2 positive 5: Multi-input with no output reference.
/// `fn f(x: &i32, y: &i32) -> i32 { *x + *y }` — no elided output refs,
/// so rule 4 doesn't trigger.
#[test]
fn stage30_2_positive_multi_input_no_output_ref() {
    assert_compiles(
        "fn f(x: &i32, y: &i32) -> i32 { *x + *y } fn main() { let v = 42; let _ = f(&v, &v); }",
    );
}

/// Stage 30.2 positive 6: Multi-input with EXPLICIT output lifetime.
/// `fn f<'a>(x: &'a i32, y: &i32) -> &'a i32 { x }` — explicit, no elision.
#[test]
fn stage30_2_positive_explicit_output_lifetime() {
    assert_compiles(
        "fn f<'a>(x: &'a i32, y: &i32) -> &'a i32 { x } fn main() { let v = 42; let _ = f(&v, &v); }",
    );
}

/// Stage 30.2 positive 7: Static lifetime on output (no inputs).
/// `fn f() -> &'static str { "hello" }` — explicit 'static, no elision.
#[test]
fn stage30_2_positive_static_lifetime_output() {
    assert_compiles("fn f() -> &'static str { \"hello\" } fn main() { let _ = f(); }");
}

/// Stage 30.2 positive 8: Static lifetime on byte slice output.
#[test]
fn stage30_2_positive_static_byte_slice_output() {
    assert_compiles("fn f() -> &'static [u8] { b\"hello\" } fn main() { let _ = f(); }");
}

// ============================================================================
// NEGATIVE TESTS — Rule 4 violations (must produce "missing lifetime specifier")
// ============================================================================

/// Stage 30.2 negative 1: Rule 4 basic — multiple inputs, no self, elided output.
/// `fn f(x: &i32, y: &i32) -> &i32 { x }` MUST error.
#[test]
fn stage30_2_negative_rule_4_basic() {
    assert_rule4_error(
        "fn f(x: &i32, y: &i32) -> &i32 { x } fn main() { let v = 42; let _ = f(&v, &v); }",
    );
}

/// Stage 30.2 negative 2: Rule 4 zero inputs — `fn f() -> &i32 { loop {} }`.
/// No input lifetime to assign; output must be explicit.
#[test]
fn stage30_2_negative_rule_4_no_inputs() {
    assert_rule4_error("fn f() -> &i32 { loop {} } fn main() { }");
}

/// Stage 30.2 negative 3: Rule 4 zero inputs returning &str (the classic
/// pattern that was silently accepted before the fix).
/// `fn f() -> &str { "hello" }` MUST error (use `&'static str` instead).
#[test]
fn stage30_2_negative_rule_4_no_inputs_str() {
    assert_rule4_error("fn f() -> &str { \"hello\" } fn main() { let _ = f(); }");
}

/// Stage 30.2 negative 4: Rule 4 with tuple return containing elided refs.
/// `fn f(x: &i32, y: &i32) -> (&i32, &i32) { (x, x) }` MUST error.
#[test]
fn stage30_2_negative_rule_4_tuple_return() {
    assert_rule4_error(
        "fn f(x: &i32, y: &i32) -> (&i32, &i32) { (x, x) } fn main() { let v = 42; let _ = f(&v, &v); }",
    );
}

/// Stage 30.2 negative 5: Rule 4 with multiple EXPLICIT input lifetimes
/// but ELIDED output. Even when inputs are explicitly `'a`/`'b`, output
/// elision is still ambiguous if there's no `&self`.
/// `fn f<'a, 'b>(x: &'a i32, y: &'b i32) -> &i32 { x }` MUST error.
#[test]
fn stage30_2_negative_rule_4_explicit_inputs_elided_output() {
    assert_rule4_error(
        "fn f<'a, 'b>(x: &'a i32, y: &'b i32) -> &i32 { x } fn main() { let v = 42; let _ = f(&v, &v); }",
    );
}

/// Stage 30.2 negative 6: Rule 4 with self by VALUE — `self` (not `&self`)
/// does NOT count for rule 3, BUT when there's exactly one OTHER input ref,
/// rule 2 fires (single input lifetime). So this is actually a POSITIVE case
/// — replaced with a true negative below.
///
/// Real negative: `fn f(self, x: &i32, y: &i32) -> &i32 { x }` — multiple
/// input lifetimes, no `&self` → rule 4 must fire.
#[test]
fn stage30_2_negative_rule_4_self_by_value_multi_inputs() {
    assert_rule4_error(
        "struct S { x: i32 } impl Copy for S {} impl S { fn f(self, x: &i32, y: &i32) -> &i32 { x } } \
         fn main() { let s = S{x:0}; let v = 42; let _ = s.f(&v, &v); }",
    );
}

/// Stage 30.2 negative 7: Rule 4 with array of refs.
/// `fn f(x: &i32, y: &i32) -> [&i32; 1] { [x] }` MUST error.
#[test]
fn stage30_2_negative_rule_4_array_of_refs() {
    assert_rule4_error(
        "fn f(x: &i32, y: &i32) -> [&i32; 1] { [x] } fn main() { let v = 42; let _ = f(&v, &v); }",
    );
}

/// Stage 30.2 negative 8: Rule 4 with slice return.
/// `fn f(x: &[i32], y: &[i32]) -> &[i32] { x }` MUST error.
#[test]
fn stage30_2_negative_rule_4_slice_return() {
    assert_rule4_error(
        "fn f(x: &[i32], y: &[i32]) -> &[i32] { x } fn main() { let v: &[i32] = &[1]; let _ = f(v, v); }",
    );
}

/// Stage 30.2 negative 9: Rule 4 with str inputs.
/// `fn f(s: &str, t: &str) -> &str { s }` MUST error.
#[test]
fn stage30_2_negative_rule_4_str_inputs() {
    assert_rule4_error(
        "fn f(s: &str, t: &str) -> &str { s } fn main() { let _ = f(\"a\", \"b\"); }",
    );
}

/// Stage 30.2 negative 10: Rule 4 with raw pointer output (elided).
/// `fn f(x: *const i32, y: *const i32) -> *const &i32 { ... }` — multiple
/// inputs (raw ptrs aren't refs but the elided `&i32` inside the raw ptr
/// is elided; we use a different shape — output is a tuple containing
/// a raw pointer to an elided reference, which rule 4 must catch).
///
/// Actually, raw pointers don't carry lifetime annotations in Landin MVP
/// (they're `*const T` not `*const &'a T`). Use a simpler shape: multi-input
/// where the output is an elided reference inside a tuple with no explicit
/// annotation.
#[test]
fn stage30_2_negative_rule_4_tuple_with_elided_inner() {
    // fn f(x: &i32, y: &i32) -> (i32, &i32) — second tuple element is an
    // elided ref. Multiple inputs, no self → rule 4 should fire.
    assert_rule4_error(
        "fn f(x: &i32, y: &i32) -> (i32, &i32) { (0, x) } fn main() { let v = 42; let _ = f(&v, &v); }",
    );
}

/// Stage 30.2 negative 11: Rule 4 with no inputs and &[u8] (byte slice).
/// `fn f() -> &[u8] { b\"hello\" }` MUST error (use `&'static [u8]`).
#[test]
fn stage30_2_negative_rule_4_no_inputs_byte_slice() {
    assert_rule4_error("fn f() -> &[u8] { b\"hello\" } fn main() { let _ = f(); }");
}

/// Stage 30.2 negative 12: Rule 4 with `&mut` output (mutable ref).
/// `fn f(x: &mut i32, y: &mut i32) -> &mut i32 { x }` MUST error.
#[test]
fn stage30_2_negative_rule_4_mut_output() {
    assert_rule4_error(
        "fn f(x: &mut i32, y: &mut i32) -> &mut i32 { x } fn main() { let mut v = 42; let _ = f(&mut v, &mut v); }",
    );
}

/// Stage 30.2 negative 13: Rule 4 with `&self` but output type is also
/// `&mut` — multiple input lifetimes (self + arg), rule 3 only fires for
/// `&self` (not when self is by value or there are extra elided refs in
/// non-self position). This case is rule 3 valid: `fn f(&self, _d: &i32) -> &i32`
/// is OK. So we test the rule-4-violating variant: `fn f(_d: &i32, &self)` —
/// wait, self comes first in Landin. Let me use a clearer rule 4 case:
/// `fn f(a: &i32, b: &i32, c: &i32) -> &i32 { a }` — three inputs, no self.
#[test]
fn stage30_2_negative_rule_4_three_inputs() {
    assert_rule4_error(
        "fn f(a: &i32, b: &i32, c: &i32) -> &i32 { a } fn main() { let v = 42; let _ = f(&v, &v, &v); }",
    );
}

// ============================================================================
// REGRESSION TESTS — Over-application bug fix (explicit vids preserved)
// ============================================================================

/// Stage 30.2 regression 1: Rule 2 with explicit output lifetime that
/// MATCHES the input — should compile (both 'a, single input 'a → rule 2
/// would replace output's vid with input's vid, but they're already equal
/// so no semantic change).
#[test]
fn stage30_2_regression_rule_2_explicit_match() {
    assert_compiles(
        "fn f<'a>(x: &'a i32) -> &'a i32 { x } fn main() { let v = 42; let _ = f(&v); }",
    );
}

/// Stage 30.2 regression 2: Rule 2 with TUPLE return where one element is
/// explicit 'a and the other is elided. Rule 2 should replace ONLY the elided
/// one (preserve 'a on the explicit element). Both end up as 'a, so the
/// function signature is consistent.
/// `fn f<'a>(x: &'a i32) -> (&'a i32, &i32) { (x, x) }` — should compile.
#[test]
fn stage30_2_regression_rule_2_mixed_explicit_elided() {
    assert_compiles(
        "fn f<'a>(x: &'a i32) -> (&'a i32, &i32) { (x, x) } fn main() { let v = 42; let _ = f(&v); }",
    );
}

// ============================================================================
// UNIT TESTS for `find_elided_ref_span` helper
// ============================================================================

/// Stage 30.2 unit 1: find_elided_ref_span returns None for non-reference type.
#[test]
fn stage30_2_unit_find_elided_ref_span_no_ref() {
    use landin_compiler::hir::{HirTy, HirTyKind};
    use landin_compiler::session::Span;

    // Build a HIR `i32` type manually (no Ref → None).
    let ty = HirTy {
        hir_id: landin_compiler::hir::HirId {
            owner: landin_compiler::hir::DefId(0),
            local_id: landin_compiler::hir::ItemLocalId(0),
        },
        kind: HirTyKind::Int(landin_compiler::ast::IntTy::I32),
        inferred: None,
        span: Span::DUMMY,
    };
    assert!(landin_compiler::mir::lower::find_elided_ref_span(&ty).is_none());
}

/// Stage 30.2 unit 2: find_elided_ref_span returns Some for `Ref(None, ...)`.
#[test]
fn stage30_2_unit_find_elided_ref_span_elided_ref() {
    use landin_compiler::hir::{HirTy, HirTyKind};
    use landin_compiler::session::Span;

    let inner = Box::new(HirTy {
        hir_id: landin_compiler::hir::HirId {
            owner: landin_compiler::hir::DefId(0),
            local_id: landin_compiler::hir::ItemLocalId(1),
        },
        kind: HirTyKind::Int(landin_compiler::ast::IntTy::I32),
        inferred: None,
        span: Span::DUMMY,
    });
    let ty = HirTy {
        hir_id: landin_compiler::hir::HirId {
            owner: landin_compiler::hir::DefId(0),
            local_id: landin_compiler::hir::ItemLocalId(2),
        },
        kind: HirTyKind::Ref(None, landin_compiler::ast::Mutability::Immutable, inner),
        inferred: None,
        span: Span::DUMMY,
    };
    assert!(landin_compiler::mir::lower::find_elided_ref_span(&ty).is_some());
}

/// Stage 30.2 unit 3: find_elided_ref_span returns None for `Ref(Some(lt), ...)`.
#[test]
fn stage30_2_unit_find_elided_ref_span_explicit_ref() {
    use landin_compiler::ast::{Ident, Lifetime, Mutability};
    use landin_compiler::hir::{HirTy, HirTyKind};
    use landin_compiler::session::Span;

    let inner = Box::new(HirTy {
        hir_id: landin_compiler::hir::HirId {
            owner: landin_compiler::hir::DefId(0),
            local_id: landin_compiler::hir::ItemLocalId(1),
        },
        kind: HirTyKind::Int(landin_compiler::ast::IntTy::I32),
        inferred: None,
        span: Span::DUMMY,
    });
    let lifetime = Lifetime {
        ident: Ident::new(landin_compiler::lexer::Symbol::default(), Span::DUMMY),
        span: Span::DUMMY,
    };
    let ty = HirTy {
        hir_id: landin_compiler::hir::HirId {
            owner: landin_compiler::hir::DefId(0),
            local_id: landin_compiler::hir::ItemLocalId(2),
        },
        kind: HirTyKind::Ref(Some(lifetime), Mutability::Immutable, inner),
        inferred: None,
        span: Span::DUMMY,
    };
    assert!(landin_compiler::mir::lower::find_elided_ref_span(&ty).is_none());
}

/// Stage 30.2 unit 4: find_elided_ref_span walks into Tuple and finds elided.
#[test]
fn stage30_2_unit_find_elided_ref_span_in_tuple() {
    use landin_compiler::hir::{HirTy, HirTyKind};
    use landin_compiler::session::Span;

    fn hir_int(local_id: u32) -> HirTy {
        HirTy {
            hir_id: landin_compiler::hir::HirId {
                owner: landin_compiler::hir::DefId(0),
                local_id: landin_compiler::hir::ItemLocalId(local_id),
            },
            kind: HirTyKind::Int(landin_compiler::ast::IntTy::I32),
            inferred: None,
            span: Span::DUMMY,
        }
    }
    fn hir_elided_ref(local_id: u32, inner: HirTy) -> HirTy {
        HirTy {
            hir_id: landin_compiler::hir::HirId {
                owner: landin_compiler::hir::DefId(0),
                local_id: landin_compiler::hir::ItemLocalId(local_id),
            },
            kind: HirTyKind::Ref(
                None,
                landin_compiler::ast::Mutability::Immutable,
                Box::new(inner),
            ),
            inferred: None,
            span: Span::DUMMY,
        }
    }

    let inner1 = hir_int(1);
    let elided_ref = hir_elided_ref(2, hir_int(3));
    let tuple = HirTy {
        hir_id: landin_compiler::hir::HirId {
            owner: landin_compiler::hir::DefId(0),
            local_id: landin_compiler::hir::ItemLocalId(4),
        },
        kind: HirTyKind::Tuple(vec![inner1, elided_ref]),
        inferred: None,
        span: Span::DUMMY,
    };
    // Tuple contains an elided Ref at index 1 → should return Some.
    assert!(landin_compiler::mir::lower::find_elided_ref_span(&tuple).is_some());
}
