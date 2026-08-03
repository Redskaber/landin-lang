//! Stage 16.05 — Field-not-found error reporting tests.
//!
//! These tests verify that accessing an undefined struct field now
//! produces a clear type error (per §1.0 原則 4 "报错 > 静默").
//!
//! Before Stage 16.05: `resolve_field_index` took `cx: &MirLowerCtxt`
//! (immutable) and could not push errors to `cx.type_errors`. The
//! fallback was `return 0` (silent wrong behavior), with a comment
//! saying "typeck should catch it in most cases."
//!
//! After Stage 16.05: `resolve_field_index` takes `cx: &mut MirLowerCtxt`
//! and pushes a `TypeError` directly when the field is not found in the
//! receiver's struct. The error message is `no field \`{name}\` on struct
//! \`{struct}\``. The fallback return value of 0 is preserved for codegen
//! recovery (the error will abort compilation before codegen runs).
//!
//! Per §29.1.3 (Design-Impl-Test coverage): integration tests verify
//! both the error is reported AND the error message is clear.

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.05 test 1: undefined field access produces an error.
///
/// `s.y` where `struct S { x: i32 }` should produce a type error.
#[test]
fn stage16_05_undefined_field_reports_error() {
    let src = "struct S { x: i32 } fn main() { let s = S { x: 1 }; s.y; }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected at least one type error for `s.y` on struct S (which has no field `y`); got 0 errors"
    );
}

/// Stage 16.05 test 2: error message contains the field name.
///
/// The error message should mention the missing field name (`y`) so the
/// user can identify which field access was wrong.
#[test]
fn stage16_05_error_message_contains_field_name() {
    let src = "struct S { x: i32 } fn main() { let s = S { x: 1 }; s.y; }";
    let result = compile(src);
    let has_field_name = result
        .errors
        .typeck
        .iter()
        .any(|e| e.message.contains("no field") && e.message.contains('y'));
    assert!(
        has_field_name,
        "expected error message to contain 'no field' and 'y'; got: {:?}",
        result
            .errors
            .typeck
            .iter()
            .map(|e| &e.message)
            .collect::<Vec<_>>()
    );
}

/// Stage 16.05 test 3: error message contains the struct name.
///
/// The error message should mention the struct name (`S`) so the user
/// can identify which struct is missing the field.
#[test]
fn stage16_05_error_message_contains_struct_name() {
    let src = "struct S { x: i32 } fn main() { let s = S { x: 1 }; s.y; }";
    let result = compile(src);
    let has_struct_name = result
        .errors
        .typeck
        .iter()
        .any(|e| e.message.contains("on struct") && e.message.contains('S'));
    assert!(
        has_struct_name,
        "expected error message to contain 'on struct' and 'S'; got: {:?}",
        result
            .errors
            .typeck
            .iter()
            .map(|e| &e.message)
            .collect::<Vec<_>>()
    );
}

/// Stage 16.05 test 4: error span points to the receiver expression.
///
/// The error span should point to `s` (the receiver), not to the struct
/// definition or some other location. This helps the user find the
/// erroneous field access quickly.
#[test]
fn stage16_05_error_span_points_to_receiver() {
    let src = "struct S { x: i32 } fn main() { let s = S { x: 1 }; s.y; }";
    let result = compile(src);
    let err = result
        .errors
        .typeck
        .iter()
        .find(|e| e.message.contains("no field"));
    assert!(
        err.is_some(),
        "expected a 'no field' error; got: {:?}",
        result
            .errors
            .typeck
            .iter()
            .map(|e| &e.message)
            .collect::<Vec<_>>()
    );
    let err = err.unwrap();
    // The span should NOT be Span::DUMMY (lo=0, hi=0).
    assert_ne!(
        err.span.lo, 0,
        "span.lo should not be 0 (was Span::DUMMY before Stage 16.05); got {}",
        err.span.lo
    );
    // The span should point into the `s.y` part (after the struct def + main fn start).
    // The `s.y` access starts around byte offset 50+ in this source.
    assert!(
        err.span.lo >= 40,
        "span.lo should point to the receiver expression (>= 40); got {}",
        err.span.lo
    );
}

/// Stage 16.05 test 5: valid field access produces no error.
///
/// Regression test: `s.x` where `struct S { x: i32 }` should NOT produce
/// an error. This ensures the new error reporting doesn't accidentally
/// fire for valid field accesses.
#[test]
fn stage16_05_valid_field_access_no_error() {
    let src = "struct S { x: i32 } fn main() { let s = S { x: 1 }; let _ = s.x; }";
    let result = compile(src);
    let has_field_error = result
        .errors
        .typeck
        .iter()
        .any(|e| e.message.contains("no field"));
    assert!(
        !has_field_error,
        "valid field access `s.x` should not produce a 'no field' error; got: {:?}",
        result
            .errors
            .typeck
            .iter()
            .map(|e| &e.message)
            .collect::<Vec<_>>()
    );
}

/// Stage 16.05 test 6: multiple undefined fields each produce an error.
///
/// Both `s.y` and `s.z` should produce separate errors (non-fatal typeck
/// continues after errors).
#[test]
fn stage16_05_multiple_undefined_fields_each_error() {
    let src = "struct S { x: i32 } fn main() { let s = S { x: 1 }; s.y; s.z; }";
    let result = compile(src);
    let field_errors: Vec<_> = result
        .errors
        .typeck
        .iter()
        .filter(|e| e.message.contains("no field"))
        .collect();
    assert!(
        field_errors.len() >= 2,
        "expected at least 2 'no field' errors (one for `s.y`, one for `s.z`); got {}: {:?}",
        field_errors.len(),
        field_errors.iter().map(|e| &e.message).collect::<Vec<_>>()
    );
}
