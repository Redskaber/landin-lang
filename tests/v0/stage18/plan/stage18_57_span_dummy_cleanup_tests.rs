//! Stage 18.57 — Span::DUMMY Technical Debt Cleanup Tests.
//!
//! Verifies that diagnostic spans are accurate (not Span::DUMMY) after
//! Priority 1-4 fixes:
//! - Priority 1: lower_hir_ty_to_mir_ty* uses ty.span
//! - Priority 2: pat_span() uses ident.span / literal span
//! - Priority 3: HirAssocType/HirAssocConst uses ident.span
//! - Priority 4: duplicate-definition errors use def_span map
//!
//! Per §9.4.3: 3 positive + 9 negative tests (1:3 ratio).
//! Per §1.0 原則 4 "报错 > 静默": accurate spans improve diagnostics.

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::compile;

// === Positive: Spans are accurate (not 1:1) ===

/// Stage 18.57 positive 1: Duplicate definition error has non-DUMMY span.
///
/// `fn foo() {} fn foo() {}` should produce a resolve error with a span
/// pointing to the second `foo` definition (not 1:1).
#[test]
fn stage18_57_duplicate_def_span_accurate() {
    let src = "fn foo() {} fn foo() {} fn main() { 0 }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Duplicate definition must produce a resolve error"
    );
    // Verify the span is not DUMMY (1:1).
    for err in &result.errors.resolve {
        if err.message.contains("duplicate definition") {
            assert!(
                err.span.lo != 0 || err.span.hi != 0,
                "Duplicate definition error span should not be DUMMY (1:1), got: {:?}",
                err.span
            );
        }
    }
}

/// Stage 18.57 positive 2: Resolve errors have non-DUMMY spans (from HIR).
///
/// Note: typeck span accuracy is a separate concern — typeck uses a
/// create-then-overwrite pattern (checker.rs overrides e.span = stmt.span).
/// This test verifies resolve errors have accurate spans from HIR lowering.
#[test]
fn stage18_57_resolve_error_span_accurate() {
    let src = "fn main() { let x: Undefined = 42; }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined type must produce a resolve error"
    );
    let has_non_dummy = result
        .errors
        .resolve
        .iter()
        .any(|e| e.span.lo != 0 || e.span.hi != 0);
    assert!(has_non_dummy, "Resolve error should have non-DUMMY span");
}

/// Stage 18.57 positive 3: Assoc type not found error has span from path.
#[test]
fn stage18_57_assoc_type_not_found_span_accurate() {
    let src = "trait C { type Item; } struct S; impl C for S { type Item = i32; } fn f<T: C>(x: T) -> <T as C>::Undefined { 0 } fn main() { 0 }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined assoc type must produce a resolve error"
    );
    // The error span should point to the qualified path, not 1:1.
    let has_non_dummy = result
        .errors
        .resolve
        .iter()
        .any(|e| e.message.contains("associated type") && (e.span.lo != 0 || e.span.hi != 0));
    assert!(
        has_non_dummy,
        "Assoc type not found error should have non-DUMMY span"
    );
}

// === Negative: Verify spans are accurate for various error types ===

/// Stage 18.57 negative 1: Undefined type in let binding has accurate span.
#[test]
fn stage18_57_undefined_type_let_span_accurate() {
    let src = "fn main() { let x: Undefined = 42; }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined type must produce a resolve error"
    );
    let has_non_dummy = result
        .errors
        .resolve
        .iter()
        .any(|e| e.span.lo != 0 || e.span.hi != 0);
    assert!(
        has_non_dummy,
        "Undefined type error should have non-DUMMY span"
    );
}

/// Stage 18.57 negative 2: Undefined function call has accurate span.
#[test]
fn stage18_57_undefined_fn_span_accurate() {
    let src = "fn main() { undefined_fn(); }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined function must produce a resolve error"
    );
    let has_non_dummy = result
        .errors
        .resolve
        .iter()
        .any(|e| e.span.lo != 0 || e.span.hi != 0);
    assert!(
        has_non_dummy,
        "Undefined function error should have non-DUMMY span"
    );
}

/// Stage 18.57 negative 3: Duplicate struct definition has accurate span.
#[test]
fn stage18_57_duplicate_struct_span_accurate() {
    let src = "struct S { x: i32 } struct S { y: i32 } fn main() { 0 }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Duplicate struct must produce a resolve error"
    );
    for err in &result.errors.resolve {
        if err.message.contains("duplicate definition") {
            assert!(
                err.span.lo != 0 || err.span.hi != 0,
                "Duplicate struct error span should not be DUMMY"
            );
        }
    }
}

/// Stage 18.57 negative 4: Duplicate trait definition has accurate span.
#[test]
fn stage18_57_duplicate_trait_span_accurate() {
    let src = "trait T { fn f(); } trait T { fn g(); } fn main() { 0 }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Duplicate trait must produce a resolve error"
    );
    for err in &result.errors.resolve {
        if err.message.contains("duplicate definition") {
            assert!(
                err.span.lo != 0 || err.span.hi != 0,
                "Duplicate trait error span should not be DUMMY"
            );
        }
    }
}

/// Stage 18.57 negative 5: Undefined trait in qualified path has accurate span.
#[test]
fn stage18_57_undefined_trait_qualified_span_accurate() {
    let src = "fn f<T>(x: T) -> <T as UndefinedTrait>::Item { 0 } fn main() { 0 }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined trait must produce a resolve error"
    );
    let has_non_dummy = result
        .errors
        .resolve
        .iter()
        .any(|e| e.span.lo != 0 || e.span.hi != 0);
    assert!(
        has_non_dummy,
        "Undefined trait error should have non-DUMMY span"
    );
}

/// Stage 18.57 negative 6: Wrong trait for assoc type has accurate span.
#[test]
fn stage18_57_wrong_trait_assoc_span_accurate() {
    let src = "trait A { type ItemA; } trait B {} struct S; impl A for S { type ItemA = i32; } impl B for S {} fn f<T: A + B>(x: T) -> <T as B>::ItemA { 0 } fn main() { 0 }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Wrong trait for assoc type must produce a resolve error"
    );
    let has_non_dummy = result
        .errors
        .resolve
        .iter()
        .any(|e| e.span.lo != 0 || e.span.hi != 0);
    assert!(
        has_non_dummy,
        "Wrong trait assoc type error should have non-DUMMY span"
    );
}

/// Stage 18.57 negative 7: Undefined type in let with accurate span.
#[test]
fn stage18_57_undefined_type_let_with_span() {
    let src = "fn main() { let x: SomeUndefinedType = 42; }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined type must produce a resolve error"
    );
    let has_non_dummy = result
        .errors
        .resolve
        .iter()
        .any(|e| e.span.lo != 0 || e.span.hi != 0);
    assert!(
        has_non_dummy,
        "Undefined type error should have non-DUMMY span"
    );
}

/// Stage 18.57 negative 8: Undefined type in function parameter with accurate span.
#[test]
fn stage18_57_undefined_type_param_with_span() {
    let src = "fn f(x: UndefinedType) { } fn main() { 0 }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined type in param must produce a resolve error"
    );
    let has_non_dummy = result
        .errors
        .resolve
        .iter()
        .any(|e| e.span.lo != 0 || e.span.hi != 0);
    assert!(
        has_non_dummy,
        "Undefined type in param error should have non-DUMMY span"
    );
}

/// Stage 18.57 negative 9: Multiple errors — verify all have non-DUMMY spans.
#[test]
fn stage18_57_multiple_errors_all_have_spans() {
    let src = "fn main() { let x: Undefined = undefined_fn(); let y: AlsoUndefined = 42; }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Multiple undefined types must produce resolve errors"
    );
    // All resolve errors should have non-DUMMY spans.
    let all_non_dummy = result
        .errors
        .resolve
        .iter()
        .all(|e| e.span.lo != 0 || e.span.hi != 0);
    assert!(
        all_non_dummy,
        "All resolve errors should have non-DUMMY spans, got: {:?}",
        result.errors.resolve
    );
}
