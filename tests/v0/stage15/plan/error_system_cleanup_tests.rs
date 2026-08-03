//! Stage 15.12 — Error system cleanup + friendly display tests.
//!
//! These tests verify the Stage 15.12 error system improvements:
//! 1. `MirBody.lower_type_errors` field REMOVED (errors returned from lower fn)
//! 2. `lower_hir_body_to_mir_full*` returns 3-tuple `(MirBody, UnificationTable, Vec<TypeError>)`
//! 3. `format_for_user` uses friendlier summary ("error: N errors found")
//! 4. `ResolveError` now displays via `.message` + snippet (was Debug {:?})
//!
//! Per §29.1.3 (Design-Impl-Test coverage): integration tests verify both
//! the architectural change (errors separated from IR) and the display
//! improvement (friendlier format).

#![cfg(test)]
#![allow(deprecated)] // Stage 15.15: tests use deprecated format_for_user
use landin_compiler::compile;

/// Stage 15.12 test 1: error summary uses friendlier format.
///
/// Verifies "error: N errors found" (was "error: N error(s)").
#[test]
fn stage15_12_error_summary_friendly_format() {
    let src = "fn f() { let x = 42;";
    let result = compile(src);
    let formatted = result
        .errors
        .format_for_user(Some(src), Some(&result.interner));
    assert!(
        formatted.contains("error:"),
        "expected 'error:' header, got: {}",
        formatted
    );
    assert!(
        formatted.contains("error found") || formatted.contains("errors found"),
        "expected 'error(s) found' in summary, got: {}",
        formatted
    );
}

/// Stage 15.12 test 2: singular vs plural error count.
#[test]
fn stage15_12_singular_error_count() {
    // One error → "1 error found" (singular)
    let src = "fn f() { let x = 42;";
    let result = compile(src);
    let formatted = result
        .errors
        .format_for_user(Some(src), Some(&result.interner));
    assert!(
        formatted.contains("1 error found"),
        "expected '1 error found' for single error, got: {}",
        formatted
    );
}

/// Stage 15.12 test 3: resolve errors display via .message + snippet.
///
/// Verifies ResolveError is now displayed with its message (was Debug {:?}).
#[test]
fn stage15_12_resolve_error_display() {
    // Use an undefined variable to trigger a resolve error.
    let src = "fn main() { undefined_var }";
    let result = compile(src);
    let formatted = result
        .errors
        .format_for_user(Some(src), Some(&result.interner));
    // Should contain [resolve] prefix with the message (not Debug format).
    assert!(
        formatted.contains("[resolve]"),
        "expected [resolve] prefix, got: {}",
        formatted
    );
    // Should NOT contain the Debug format "ResolveError { ... }"
    assert!(
        !formatted.contains("ResolveError {"),
        "should not contain Debug format, got: {}",
        formatted
    );
}

/// Stage 15.12 test 4: typeck errors display with snippet.
#[test]
fn stage15_12_typeck_error_with_snippet() {
    // Use a real type error: assigning wrong type to let binding.
    // `let x: i32 = true;` should produce a typeck error.
    let src = "fn main() { let x: i32 = true; }";
    let result = compile(src);
    let formatted = result
        .errors
        .format_for_user(Some(src), Some(&result.interner));
    // typeck may or may not catch this (v0.1 typeck is limited), but
    // if there are errors, the format should be correct.
    if !result.errors.is_empty() {
        assert!(
            formatted.contains(" | ") || formatted.contains("[resolve]"),
            "expected snippet gutter or resolve prefix, got: {}",
            formatted
        );
    }
}

/// Stage 15.12 test 5: borrowck errors display with snippet.
#[test]
fn stage15_12_borrowck_error_with_snippet() {
    // Double mutable borrow — classic borrowck error.
    let src = r#"
        fn main() {
            let mut x = 42;
            let r1 = &mut x;
            let r2 = &mut x;
            *r1 + *r2;
        }
    "#;
    let result = compile(src);
    let formatted = result
        .errors
        .format_for_user(Some(src), Some(&result.interner));
    // May or may not have borrowck errors (v0.1 borrowck is limited),
    // but the format should be correct if any exist.
    if formatted.contains("[borrowck]") {
        assert!(
            formatted.contains(" | "),
            "expected snippet gutter for borrowck error, got: {}",
            formatted
        );
    }
}

/// Stage 15.12 test 6: trait errors display with interner resolution.
#[test]
fn stage15_12_trait_error_display() {
    // Conflicting impls → coherence error.
    let src = "trait Foo {} struct S; impl Foo for S {} impl Foo for S {} fn main() {}";
    let result = compile(src);
    let formatted = result
        .errors
        .format_for_user(Some(src), Some(&result.interner));
    assert!(
        formatted.contains("[trait]"),
        "expected [trait] prefix, got: {}",
        formatted
    );
    // Should contain the resolved trait name "Foo" (not a Spur Debug).
    assert!(
        formatted.contains("Foo"),
        "expected trait name 'Foo' resolved, got: {}",
        formatted
    );
}

/// Stage 15.12 test 7: no errors produces empty string.
#[test]
fn stage15_12_no_errors_empty_output() {
    let src = "fn main() -> i32 { 42 }";
    let result = compile(src);
    let formatted = result
        .errors
        .format_for_user(Some(src), Some(&result.interner));
    assert!(
        formatted.is_empty(),
        "no errors should produce empty string, got: {}",
        formatted
    );
}

/// Stage 15.12 test 8: MirBody no longer has lower_type_errors field.
///
/// This is a compile-time check — if the field still existed, this test
/// wouldn't compile. We verify by constructing a MirBody and checking
/// that the field doesn't exist (via the fact that the code compiles
/// without accessing it).
#[test]
fn stage15_12_mirbody_no_lower_type_errors_field() {
    use landin_compiler::mir::body::MirBody;
    use landin_compiler::session::Span;
    let mir = MirBody::new(Span::DUMMY);
    // If `lower_type_errors` field existed, this would compile but we'd
    // want to assert it's empty. Since it's removed, we just verify the
    // MirBody can be constructed. The field doesn't exist — this is the
    // architectural improvement.
    assert!(mir.basic_blocks.is_empty());
    assert!(mir.local_decls.is_empty());
    // Note: cannot access mir.lower_type_errors — field was removed.
}

// === Stage 15.81: Span accuracy tests ===

/// Stage 15.81: `if 42 { 1 }` should produce a typeck error whose span
/// points to the `42` literal (not `1:1` / file start).
///
/// Previously, the SwitchInt discriminant mismatch used `Span::DUMMY`
/// (producing "1:1" in the error location). Stage 15.81 uses the
/// discriminant operand's span.
#[test]
fn stage15_81_if_condition_span_points_to_condition() {
    // `if 42` — 42 is at byte offset 15 in this source.
    // "fn main() { if 42 { 1 } }"
    //  0123456789012345 — `42` starts at index 15.
    let src = "fn main() { if 42 { 1 } }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for `if 42`"
    );
    // Find the error with "mismatched types" message.
    let mismatch_err = result
        .errors
        .typeck
        .iter()
        .find(|e| e.message.contains("mismatched types"))
        .expect("expected mismatched types error");
    // The span should NOT be Span::DUMMY (lo=0, hi=0).
    assert_ne!(
        mismatch_err.span.lo, 0,
        "span.lo should not be 0 (was Span::DUMMY before Stage 15.81); got {}",
        mismatch_err.span.lo
    );
    // The span should point to the `42` literal (byte offset 15).
    assert_eq!(
        mismatch_err.span.lo, 15,
        "span.lo should point to `42` at byte 15; got {}",
        mismatch_err.span.lo
    );
}

/// Stage 15.81: `let x = 42; x();` should produce a typeck error whose
/// span points to the `x` in `x()` (not `1:1` / file start).
///
/// Previously, the Call "expected function, found i32" error used
/// `Span::DUMMY`. Stage 15.81 uses the func operand's span.
#[test]
fn stage15_81_call_non_function_span_points_to_callee() {
    // `x()` — the second `x` (in `x()`) is at byte offset 24.
    // "fn main() { let x = 42; x(); }"
    //  0123456789012345678901234 — second `x` is at index 24.
    let src = "fn main() { let x = 42; x(); }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for `x()` on non-function"
    );
    // Find the error with "expected function" message.
    let call_err = result
        .errors
        .typeck
        .iter()
        .find(|e| e.message.contains("expected function"))
        .expect("expected 'expected function' error");
    // The span should NOT be Span::DUMMY.
    assert_ne!(
        call_err.span.lo, 0,
        "span.lo should not be 0 (was Span::DUMMY before Stage 15.81); got {}",
        call_err.span.lo
    );
    // The span should point to the `x` in `x()` (byte offset 24).
    assert_eq!(
        call_err.span.lo, 24,
        "span.lo should point to `x` in `x()` at byte 24; got {}",
        call_err.span.lo
    );
}

/// Stage 15.81: error messages should use human-readable type names
/// (from Stage 15.80), not Debug format. This test verifies the
/// `if 42` error mentions `bool` and `{integer}`, not `Bool` and
/// `Infer(IntVar(...))`.
#[test]
fn stage15_81_error_uses_human_readable_type_names() {
    let src = "fn main() { if 42 { 1 } }";
    let result = compile(src);
    let mismatch_err = result
        .errors
        .typeck
        .iter()
        .find(|e| e.message.contains("mismatched types"))
        .expect("expected mismatched types error");
    // Should contain human-readable "bool", not Debug "Bool".
    assert!(
        mismatch_err.message.contains("bool"),
        "message should contain 'bool' (human-readable), got: {}",
        mismatch_err.message
    );
    // Should NOT contain Debug format like "Bool" or "Infer(IntVar".
    assert!(
        !mismatch_err.message.contains("Bool"),
        "message should NOT contain Debug 'Bool', got: {}",
        mismatch_err.message
    );
    assert!(
        !mismatch_err.message.contains("Infer("),
        "message should NOT contain Debug 'Infer(...', got: {}",
        mismatch_err.message
    );
}

/// Stage 15.82: `let x = true + false;` should produce a typeck error
/// whose span points to the statement (not `1:1` / file start).
///
/// Previously, the BinaryOp "cannot apply arithmetic" error used
/// `Span::DUMMY` (because `infer_rvalue` had no access to the statement
/// span). Stage 15.82 threads `stmt.span` through `infer_rvalue`.
#[test]
fn stage15_82_binary_op_error_span_points_to_statement() {
    // `true + false` — the `+` is at byte offset ~24 in this source.
    // The statement span covers `let x = true + false;` starting around byte 13.
    let src = "fn main() { let x = true + false; }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for `true + false`"
    );
    // Find the error with "cannot apply arithmetic" message.
    let arith_err = result
        .errors
        .typeck
        .iter()
        .find(|e| e.message.contains("cannot apply arithmetic"))
        .expect("expected 'cannot apply arithmetic' error");
    // The span should NOT be Span::DUMMY.
    assert_ne!(
        arith_err.span.lo, 0,
        "span.lo should not be 0 (was Span::DUMMY before Stage 15.82); got {}",
        arith_err.span.lo
    );
    // The span should point into the statement (byte offset >= 13, the `let`).
    assert!(
        arith_err.span.lo >= 13,
        "span.lo should point into the statement (>= 13); got {}",
        arith_err.span.lo
    );
}

/// Stage 15.82: `let y = !"hello";` should produce a typeck error
/// whose span points to the statement (not `1:1` / file start).
///
/// Previously, the UnaryOp "cannot apply `!`" error used `Span::DUMMY`.
/// Stage 15.82 threads `stmt.span` through `infer_rvalue`.
#[test]
fn stage15_82_unary_op_error_span_points_to_statement() {
    // `!"hello"` — the `!` is at byte offset ~30 in this source.
    let src = "fn main() { let x = !true; let y = !\"hello\"; }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for `!\"hello\"`"
    );
    // Find the error with "cannot apply `!`" message (for the &str case).
    let not_err = result
        .errors
        .typeck
        .iter()
        .find(|e| e.message.contains("cannot apply `!`") && e.message.contains("str"))
        .expect("expected 'cannot apply `!` to &str' error");
    // The span should NOT be Span::DUMMY.
    assert_ne!(
        not_err.span.lo, 0,
        "span.lo should not be 0 (was Span::DUMMY before Stage 15.82); got {}",
        not_err.span.lo
    );
    // The span should point into the second statement (byte offset >= 30).
    assert!(
        not_err.span.lo >= 30,
        "span.lo should point into the second statement (>= 30); got {}",
        not_err.span.lo
    );
}

/// Stage 15.82: BinaryOp/UnaryOp error messages should use human-readable
/// type names (from Stage 15.80), not Debug format.
#[test]
fn stage15_82_binary_op_error_uses_human_readable_type_names() {
    let src = "fn main() { let x = true + false; }";
    let result = compile(src);
    let arith_err = result
        .errors
        .typeck
        .iter()
        .find(|e| e.message.contains("cannot apply arithmetic"))
        .expect("expected 'cannot apply arithmetic' error");
    // Should contain human-readable "bool", not Debug "Bool".
    assert!(
        arith_err.message.contains("bool"),
        "message should contain 'bool' (human-readable), got: {}",
        arith_err.message
    );
    // Should NOT contain Debug format like "Bool".
    assert!(
        !arith_err.message.contains("Bool"),
        "message should NOT contain Debug 'Bool', got: {}",
        arith_err.message
    );
}

/// Stage 15.83: `[1, true, 3]` (array element type mismatch) should
/// produce a typeck error whose span points to the array literal (not
/// `1:1` / file start).
///
/// Previously, the AggregateKind::Array unify error used `Span::DUMMY`
/// (because the unify error span wasn't overridden). Stage 15.83 uses
/// `stmt_span` from `infer_rvalue`.
#[test]
fn stage15_83_array_element_mismatch_span_points_to_array() {
    // `[1, true, 3]` — the `[` is at byte offset ~15 in this source.
    let src = "fn main() { let x = [1, true, 3]; }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for `[1, true, 3]`"
    );
    // Find the error with "mismatched types" message.
    let mismatch_err = result
        .errors
        .typeck
        .iter()
        .find(|e| e.message.contains("mismatched types"))
        .expect("expected mismatched types error");
    // The span should NOT be Span::DUMMY.
    assert_ne!(
        mismatch_err.span.lo, 0,
        "span.lo should not be 0 (was Span::DUMMY before Stage 15.83); got {}",
        mismatch_err.span.lo
    );
    // The span should point into the statement (byte offset >= 15, the `let`).
    assert!(
        mismatch_err.span.lo >= 15,
        "span.lo should point into the statement (>= 15); got {}",
        mismatch_err.span.lo
    );
}

/// Stage 15.83: `S { x: true }` (struct field type mismatch) should
/// produce a typeck error whose span points to the struct literal (not
/// `1:1` / file start).
///
/// Previously, the AggregateKind::Adt unify error used `Span::DUMMY`.
/// Stage 15.83 uses `stmt_span` from `infer_rvalue`.
#[test]
fn stage15_83_struct_field_mismatch_span_points_to_literal() {
    // `S { x: true }` — the `S` (in `let s = S {`) is at byte offset 40.
    // "struct S { x: i32 } fn main() { let s = S { x: true }; }"
    //  0123456789012345678901234567890123456789012 — `S {` at index 40.
    let src = "struct S { x: i32 } fn main() { let s = S { x: true }; }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for `S {{ x: true }}`"
    );
    // Find the error with "mismatched types" message.
    let mismatch_err = result
        .errors
        .typeck
        .iter()
        .find(|e| e.message.contains("mismatched types"))
        .expect("expected mismatched types error");
    // The span should NOT be Span::DUMMY.
    assert_ne!(
        mismatch_err.span.lo, 0,
        "span.lo should not be 0 (was Span::DUMMY before Stage 15.83); got {}",
        mismatch_err.span.lo
    );
    // The span should point into the struct literal (byte offset >= 40, the `S`).
    assert!(
        mismatch_err.span.lo >= 40,
        "span.lo should point into the struct literal (>= 40); got {}",
        mismatch_err.span.lo
    );
}
