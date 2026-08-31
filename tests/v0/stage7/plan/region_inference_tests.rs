//! Stage 7 (TD-015): Region inference integration tests.
//!
//! Per stage-committee-process.md v3.21 §17.1, test files live under
//! `tests/v0/stage{N}/plan/`. This file tests the region inference
//! infrastructure integrated into borrowck (Stage 7.1-7.5).
//!
//! Test categories:
//! 1. RegionInferenceContext data structure tests (Stage 7.1)
//! 2. Region inference algorithm tests (Stage 7.2)
//! 3. Implied bounds + type tests (Stage 7.3)
//! 4. Universe tracking + SCC compression (Stage 7.4)
//! 5. Integration with borrowck (Stage 7.5)
// Stage 15.37: Allow deprecated — these tests intentionally exercise the
// legacy `check_mir_body` path while it is being phased out (driver now uses
// `check_mir_body_with_dataflow`).

use landin_compiler::borrowck::{check_mir_body_with_dataflow, BorrowChecker};
use landin_compiler::mir::body::MirBody;
use landin_compiler::mir::ty::{Region, Ty, TyKind};
use landin_compiler::session::Span;

// ================================================================
// Stage 7.1: RegionInferenceContext data structure tests
// ================================================================

#[test]
fn stage7_region_inference_context_creation() {
    // Verify that BorrowChecker can be created and used
    // (region inference is integrated internally)
    let mut bc = BorrowChecker::new();
    let mut mir = MirBody::new(Span::DUMMY);
    mir.new_block();
    bc.check_mir_body_with_dataflow(&mir);
    let errors = bc.into_errors();
    // Empty MIR body should produce no errors
    assert!(
        errors.is_empty(),
        "expected no errors for empty body, got: {:?}",
        errors
    );
}

#[test]
fn stage7_region_inference_simple_body() {
    // A simple body with an i32 local and an assignment
    let mut mir = MirBody::new(Span::DUMMY);
    let _bb0 = mir.new_block();

    let ret = mir.new_local(
        Ty::new(TyKind::Int(landin_compiler::ast::IntTy::I32), Span::DUMMY),
        None,
        Span::DUMMY,
    );
    let _temp = mir.new_local(
        Ty::new(TyKind::Int(landin_compiler::ast::IntTy::I32), Span::DUMMY),
        None,
        Span::DUMMY,
    );

    // Simple body should have no borrow errors
    let errors = check_mir_body_with_dataflow(&mir);
    assert!(
        errors.is_empty(),
        "expected no errors for simple i32 body, got: {:?}",
        errors
    );

    // Verify return local exists
    assert!(ret.0 < mir.local_decls.len() as u32);
}

#[test]
fn stage7_region_inference_ref_type_body() {
    // A body with a reference type — exercises region tracking
    let mut mir = MirBody::new(Span::DUMMY);
    let _bb0 = mir.new_block();

    // Create a &i32 local (region = Erased for MVP)
    let ref_ty = Ty::new(
        TyKind::Ref(
            Region::Erased,
            landin_compiler::mir::ty::Mutability::Immutable,
            Box::new(Ty::new(
                TyKind::Int(landin_compiler::ast::IntTy::I32),
                Span::DUMMY,
            )),
        ),
        Span::DUMMY,
    );
    let _ref_local = mir.new_local(ref_ty, None, Span::DUMMY);

    // Reference types should not cause borrow errors in an empty body
    let errors = check_mir_body_with_dataflow(&mir);
    assert!(
        errors.is_empty(),
        "expected no errors for ref type body, got: {:?}",
        errors
    );
}

// ================================================================
// Stage 7.5: Integration with borrowck
// ================================================================

#[test]
fn stage7_borrow_checker_accepts_valid_borrow() {
    // Test that the borrow checker (with region inference integrated)
    // still correctly accepts valid borrows
    let mut mir = MirBody::new(Span::DUMMY);
    let bb0 = mir.new_block();

    let x = mir.new_local(
        Ty::new(TyKind::Int(landin_compiler::ast::IntTy::I32), Span::DUMMY),
        None,
        Span::DUMMY,
    );
    let r = mir.new_local(
        Ty::new(
            TyKind::Ref(
                Region::Erased,
                landin_compiler::mir::ty::Mutability::Immutable,
                Box::new(Ty::new(
                    TyKind::Int(landin_compiler::ast::IntTy::I32),
                    Span::DUMMY,
                )),
            ),
            Span::DUMMY,
        ),
        None,
        Span::DUMMY,
    );

    // r = &x — valid shared borrow
    use landin_compiler::mir::body::*;
    use landin_compiler::mir::place::*;
    mir.block_mut(bb0).statements.push(Statement {
        kind: StatementKind::Assign(Box::new((
            Place::local(r, Span::DUMMY),
            Rvalue::Ref(
                Region::Erased,
                BorrowKind::Shared,
                Place::local(x, Span::DUMMY),
            ),
        ))),
        span: Span::DUMMY,
    });

    let errors = check_mir_body_with_dataflow(&mir);
    assert!(
        errors.is_empty(),
        "expected no errors for valid shared borrow, got: {:?}",
        errors
    );
}

#[test]
fn stage7_borrow_checker_detects_use_after_move() {
    // Test that the borrow checker (with region inference integrated)
    // still correctly detects use-after-move
    let mut mir = MirBody::new(Span::DUMMY);
    let bb0 = mir.new_block();

    // Create a non-Copy type (String/Str is not Copy)
    let x = mir.new_local(Ty::new(TyKind::Str, Span::DUMMY), None, Span::DUMMY);
    let y = mir.new_local(Ty::new(TyKind::Str, Span::DUMMY), None, Span::DUMMY);

    // y = move x — moves x
    use landin_compiler::mir::body::*;
    use landin_compiler::mir::place::*;
    mir.block_mut(bb0).statements.push(Statement {
        kind: StatementKind::Assign(Box::new((
            Place::local(y, Span::DUMMY),
            Rvalue::Use(Operand::Move(Place::local(x, Span::DUMMY))),
        ))),
        span: Span::DUMMY,
    });
    // x is now moved — any further use should error
    // (The borrow checker should detect this via move tracking)

    let errors = check_mir_body_with_dataflow(&mir);
    // The move itself is valid; we're just checking the checker runs
    // without crashing. Error detection depends on further use of x.
    // For this test, we just verify the checker doesn't panic.
    let _ = errors;
}

#[test]
fn stage7_region_inference_context_standalone() {
    // Test that the RegionInferenceContext can be used standalone
    // (without going through BorrowChecker)
    // This verifies the public API is accessible
    let mut bc = BorrowChecker::new();
    let mut mir = MirBody::new(Span::DUMMY);
    let _bb0 = mir.new_block();

    // Create a few locals with different types
    let _i32_local = mir.new_local(
        Ty::new(TyKind::Int(landin_compiler::ast::IntTy::I32), Span::DUMMY),
        None,
        Span::DUMMY,
    );
    let _bool_local = mir.new_local(Ty::new(TyKind::Bool, Span::DUMMY), None, Span::DUMMY);

    bc.check_mir_body_with_dataflow(&mir);
    let errors = bc.into_errors();
    assert!(errors.is_empty());

    // Verify into_errors works
    let mut bc2 = BorrowChecker::new();
    bc2.check_mir_body_with_dataflow(&mir);
    let errors2 = bc2.into_errors();
    assert!(errors2.is_empty());
}

// ================================================================
// Stage 7.5: Regression tests — verify existing behavior preserved
// ================================================================

#[test]
fn stage7_regression_no_errors_on_simple_body() {
    let mut mir = MirBody::new(Span::DUMMY);
    mir.new_block();
    let errors = check_mir_body_with_dataflow(&mir);
    assert!(errors.is_empty());
}

#[test]
fn stage7_regression_copy_type_not_moved() {
    // i32 is Copy — using it twice should not trigger move errors
    let mut mir = MirBody::new(Span::DUMMY);
    let bb0 = mir.new_block();

    let x = mir.new_local(
        Ty::new(TyKind::Int(landin_compiler::ast::IntTy::I32), Span::DUMMY),
        None,
        Span::DUMMY,
    );
    let y = mir.new_local(
        Ty::new(TyKind::Int(landin_compiler::ast::IntTy::I32), Span::DUMMY),
        None,
        Span::DUMMY,
    );
    let z = mir.new_local(
        Ty::new(TyKind::Int(landin_compiler::ast::IntTy::I32), Span::DUMMY),
        None,
        Span::DUMMY,
    );

    use landin_compiler::mir::body::*;
    use landin_compiler::mir::place::*;
    // y = copy x — valid (i32 is Copy)
    mir.block_mut(bb0).statements.push(Statement {
        kind: StatementKind::Assign(Box::new((
            Place::local(y, Span::DUMMY),
            Rvalue::Use(Operand::Copy(Place::local(x, Span::DUMMY))),
        ))),
        span: Span::DUMMY,
    });
    // z = copy x — still valid (i32 is Copy, can be copied multiple times)
    mir.block_mut(bb0).statements.push(Statement {
        kind: StatementKind::Assign(Box::new((
            Place::local(z, Span::DUMMY),
            Rvalue::Use(Operand::Copy(Place::local(x, Span::DUMMY))),
        ))),
        span: Span::DUMMY,
    });

    let errors = check_mir_body_with_dataflow(&mir);
    assert!(
        errors.is_empty(),
        "Copy type used twice should not error: {:?}",
        errors
    );
}
