//! Round 5 deep inspection tests for cross-stage integration.
//!
//! These tests verify that the compiler produces *correct output structure*,
//! not just "no errors". They were added per §9.3.2 of process v3.3 to
//! verify cross-stage data flow consistency (HIR → MIR → typeck → borrowck).
//!
//! Each test checks a specific invariant of the compiler's output:
//!   - typeck writes resolved types back to local_decls
//!   - MIR contains StorageLive/StorageDead/Assert in expected places
//!   - Path resolution correctly maps to locals (G1 regression)
//!   - Fn sig is unified with body value type (fix #3 regression)
//!   - String literals have Str type (P1-2 regression)
//!   - Unsuffixed literals default correctly (i32/f64)
//!   - Short-circuit && produces control flow (P1-1 regression)

use landin_compiler::driver::compile;
use landin_compiler::mir::body::{AssertMessage, StatementKind, Terminator};
use landin_compiler::mir::lvalue::{BinOp, LocalId};
use landin_compiler::mir::ty::TyKind;

// =====================================================================
// typeck writeback (Phase 3)
// =====================================================================

#[test]
fn deep_typeck_writeback_i32() {
    let result = compile("fn f() { let x = 42; }");
    let mir = &result.mirs[0];
    let has_i32 = mir
        .local_decls
        .iter()
        .any(|ld| matches!(&ld.ty.kind, TyKind::Int(landin_compiler::ast::IntTy::I32)));
    assert!(
        has_i32,
        "expected at least one i32 local after typeck writeback"
    );
}

#[test]
fn deep_typeck_writeback_bool() {
    let result = compile("fn f() { let x = true; }");
    let mir = &result.mirs[0];
    let has_bool = mir
        .local_decls
        .iter()
        .any(|ld| matches!(&ld.ty.kind, TyKind::Bool));
    assert!(
        has_bool,
        "expected at least one bool local after typeck writeback"
    );
}

// =====================================================================
// StorageLive / StorageDead (P1-5)
// =====================================================================

#[test]
fn deep_storage_live_return_local() {
    let result = compile("fn f() {}");
    let mir = &result.mirs[0];
    let has_live_return = mir.basic_blocks[0]
        .statements
        .iter()
        .any(|s| matches!(s.kind, StatementKind::StorageLive(LocalId(0))));
    assert!(
        has_live_return,
        "expected StorageLive(LocalId(0)) at function entry"
    );
}

#[test]
fn deep_storage_live_params() {
    let result = compile("fn f(a: i32, b: bool) {}");
    let mir = &result.mirs[0];
    let has_live_a = mir.basic_blocks[0]
        .statements
        .iter()
        .any(|s| matches!(s.kind, StatementKind::StorageLive(LocalId(1))));
    let has_live_b = mir.basic_blocks[0]
        .statements
        .iter()
        .any(|s| matches!(s.kind, StatementKind::StorageLive(LocalId(2))));
    assert!(has_live_a, "expected StorageLive for param `a`");
    assert!(has_live_b, "expected StorageLive for param `b`");
}

#[test]
fn deep_storage_dead_before_return() {
    let result = compile("fn f(x: i32) { let y = x + 1; }");
    let mir = &result.mirs[0];
    let return_bb = mir
        .basic_blocks
        .iter()
        .find(|bb| matches!(bb.terminator, Terminator::Return))
        .expect("no Return terminator");
    let dead_count = return_bb
        .statements
        .iter()
        .filter(|s| matches!(s.kind, StatementKind::StorageDead(_)))
        .count();
    assert!(
        dead_count >= 2,
        "expected at least 2 StorageDead before Return, got {}",
        dead_count
    );
}

// =====================================================================
// Assert terminator (P1-6)
// =====================================================================

#[test]
fn deep_assert_overflow_for_addition() {
    let result = compile("fn f(a: i32, b: i32) -> i32 { a + b }");
    let mir = &result.mirs[0];
    let has_overflow_assert = mir.basic_blocks.iter().any(|bb| {
        matches!(
            &bb.terminator,
            Terminator::Assert {
                msg: AssertMessage::Overflow(BinOp::Add, _, _),
                ..
            }
        )
    });
    assert!(
        has_overflow_assert,
        "expected Assert(Overflow(Add, _, _)) for `a + b`"
    );
}

#[test]
fn deep_no_assert_for_comparison() {
    let result = compile("fn f(a: i32, b: i32) -> bool { a == b }");
    let mir = &result.mirs[0];
    let has_assert = mir
        .basic_blocks
        .iter()
        .any(|bb| matches!(&bb.terminator, Terminator::Assert { .. }));
    assert!(
        !has_assert,
        "expected NO Assert for comparison (comparisons can't overflow)"
    );
}

// =====================================================================
// typeck_results (P1-3)
// =====================================================================

#[test]
fn deep_typeck_results_populated() {
    let result = compile("fn f(x: i32) -> i32 { x }");
    assert!(
        !result.typeck_results.is_empty(),
        "expected typeck_results to be populated"
    );
    assert!(
        !result.typeck_results[0].local_types.is_empty(),
        "expected local_types map to be non-empty"
    );
}

// =====================================================================
// Fn sig unification (fix #3)
// =====================================================================

#[test]
fn deep_fn_sig_unified_with_body() {
    let result = compile("fn f() -> i32 { 42 }");
    let mir = &result.mirs[0];
    let return_ty = &mir.local_decls[0].ty;
    assert!(
        matches!(
            return_ty.kind,
            TyKind::Int(landin_compiler::ast::IntTy::I32)
        ),
        "expected return local to have Int(I32) from fn sig, got {:?}",
        return_ty.kind
    );
}

// =====================================================================
// Path resolution (G1 regression)
// =====================================================================

#[test]
fn deep_path_resolves_to_local() {
    // If G1 regressed, x would resolve to Error and y's type would be Error.
    let result = compile("fn f() { let x = 1; let y = x; }");
    let mir = &result.mirs[0];
    let has_i32 = mir
        .local_decls
        .iter()
        .any(|ld| matches!(&ld.ty.kind, TyKind::Int(landin_compiler::ast::IntTy::I32)));
    let no_error = mir
        .local_decls
        .iter()
        .all(|ld| !matches!(&ld.ty.kind, TyKind::Error));
    assert!(has_i32, "expected at least one i32 local (x or y)");
    assert!(
        no_error,
        "expected no Error-typed locals (G1 regression check)"
    );
}

// =====================================================================
// String literal type (P1-2 regression)
// =====================================================================

#[test]
fn deep_string_literal_has_str_type() {
    let result = compile("fn f() { let s = \"hello\"; }");
    let mir = &result.mirs[0];
    // Stage 3.42: string literals now have type &'static str (Ref to Str),
    // not Str. Check for either Str or Ref(_, _, Str).
    let has_str = mir.local_decls.iter().any(|ld| {
        matches!(&ld.ty.kind, TyKind::Str)
            || matches!(&ld.ty.kind, TyKind::Ref(_, _, inner) if matches!(inner.kind, TyKind::Str))
    });
    assert!(
        has_str,
        "expected at least one Str-typed or &str-typed local from string literal"
    );
}

// =====================================================================
// Unsuffixed literal defaults
// =====================================================================

#[test]
fn deep_unsuffixed_int_defaults_i32() {
    let result = compile("fn f() { let x = 42; }");
    let mir = &result.mirs[0];
    let has_i32 = mir
        .local_decls
        .iter()
        .any(|ld| matches!(&ld.ty.kind, TyKind::Int(landin_compiler::ast::IntTy::I32)));
    assert!(has_i32, "expected unsuffixed int to default to i32");
}

#[test]
fn deep_unsuffixed_float_defaults_f64() {
    let result = compile("fn f() { let x = 3.14; }");
    let mir = &result.mirs[0];
    let has_f64 = mir.local_decls.iter().any(|ld| {
        matches!(
            &ld.ty.kind,
            TyKind::Float(landin_compiler::ast::FloatTy::F64)
        )
    });
    assert!(has_f64, "expected unsuffixed float to default to f64");
}

// =====================================================================
// let ascription (fix #4)
// =====================================================================

#[test]
fn deep_let_u64_annotation_unifies() {
    let result = compile("fn f() { let z: u64 = 100; }");
    let mir = &result.mirs[0];
    let has_u64 = mir
        .local_decls
        .iter()
        .any(|ld| matches!(&ld.ty.kind, TyKind::Uint(landin_compiler::ast::UintTy::U64)));
    assert!(has_u64, "expected u64 local from let ascription");
}

// =====================================================================
// Short-circuit && (P1-1 regression)
// =====================================================================

#[test]
fn deep_short_circuit_and_produces_control_flow() {
    let result = compile("fn f(a: bool, b: bool) -> bool { a && b }");
    let mir = &result.mirs[0];
    // && should produce multiple basic blocks (short-circuit):
    //   bb0 (entry), bb_eval_rhs, bb_short_circuit, bb_true, bb_false, bb_cont
    assert!(
        mir.basic_blocks.len() >= 3,
        "expected ≥3 basic blocks for short-circuit &&, got {}",
        mir.basic_blocks.len()
    );
}
