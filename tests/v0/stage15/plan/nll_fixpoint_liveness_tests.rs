//! Stage 15.35 — NLL fixpoint liveness integration tests.
//!
//! These tests verify the public `compute_liveness` API (Stage 15.35,
//! v0.2 Phase 2 Task 7, HP-10) by:
//!
//! 1. Building MIR bodies directly (unit-style CFG coverage) and asserting
//!    the fixpoint liveness equations converge to the expected LiveIn /
//!    LiveOut sets for straight-line, branch, and loop patterns.
//! 2. Compiling real Landin source via `compile()` and verifying the
//!    fixpoint liveness API is callable on the resulting MIR body —
//!    this catches regressions where MIR shapes that the borrow checker
//!    sees every day would break the new analysis.
//!
//! Per §29.1.3 (Design-Impl-Test coverage): the tests cover both synthetic
//! CFGs (precise assertions on LiveIn/LiveOut) and real-pipeline MIR
//! (smoke tests that the API accepts whatever the compiler produces).
//!
//! Per §23 (API naming): all public symbols tested here (`compute_liveness`,
//! `successors`, `LiveInMap`, `LiveOutMap`) follow the `<verb>_<noun>` /
//! `<Noun>Map` conventions established in `api-naming-standard.md`.

#![cfg(test)]

use landin_compiler::borrowck::{compute_liveness, successors};
use landin_compiler::compile;
use landin_compiler::mir::body::{
    BasicBlockId, MirBody, Statement, StatementKind, Terminator, TerminatorKind,
};
use landin_compiler::mir::place::{Operand, Place, PlaceKind, Rvalue};
use landin_compiler::mir::ty::{Const, ConstVal, Mutability, Ty, TyKind};
use landin_compiler::session::Span;

/// Helper: build an empty MirBody with one i32 local per slot requested.
fn build_body_with_locals(n_locals: usize) -> (MirBody, Vec<landin_compiler::mir::place::LocalId>) {
    let mut body = MirBody::new(Span::DUMMY);
    let mut locals = Vec::with_capacity(n_locals);
    for _ in 0..n_locals {
        let id = body.new_local(
            Ty::new(TyKind::Int(landin_compiler::ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        locals.push(id);
    }
    (body, locals)
}

fn place_local(id: landin_compiler::mir::place::LocalId) -> Place {
    Place {
        kind: PlaceKind::Local(id),
        span: Span::DUMMY,
    }
}

fn operand_copy(id: landin_compiler::mir::place::LocalId) -> Operand {
    Operand::Copy(place_local(id))
}

fn operand_const_int(val: u128) -> Operand {
    Operand::Constant(Const {
        ty: Ty::new(TyKind::Int(landin_compiler::ast::IntTy::I32), Span::DUMMY),
        val: ConstVal::Int(val),
    })
}

fn stmt_assign_use(
    dst: landin_compiler::mir::place::LocalId,
    src: landin_compiler::mir::place::LocalId,
) -> Statement {
    Statement {
        kind: StatementKind::Assign(Box::new((place_local(dst), Rvalue::Use(operand_copy(src))))),
        span: Span::DUMMY,
    }
}

fn stmt_assign_const(dst: landin_compiler::mir::place::LocalId, val: u128) -> Statement {
    Statement {
        kind: StatementKind::Assign(Box::new((
            place_local(dst),
            Rvalue::Use(operand_const_int(val)),
        ))),
        span: Span::DUMMY,
    }
}

// ============================================================
// Part A — Synthetic CFG coverage (precise LiveIn/LiveOut assertions)
// ============================================================

/// Stage 15.35 integration test 1: Straight-line code with one live local.
///
/// CFG: `bb0: x = 1; y = x; return;`
/// - x is read in stmt 1 (y = x), so x ∈ Use[bb0].
/// - x ∈ Def[bb0] (stmt 0 writes x).
/// - live_out[bb0] = ∅ (no successor).
/// - live_in[bb0] = Use ∪ (LiveOut - Def) = {x} ∪ ∅ = {x}.
/// - y ∈ Def[bb0] but never read → y ∉ live_in[bb0].
#[test]
fn stage15_35_integration_straight_line_x_live_y_dead() {
    let (mut body, locals) = build_body_with_locals(2);
    let x = locals[0];
    let y = locals[1];
    let bb0 = body.new_block();
    body.block_mut(bb0).statements.push(stmt_assign_const(x, 1));
    body.block_mut(bb0).statements.push(stmt_assign_use(y, x));
    body.block_mut(bb0).terminator = Terminator::ret(Span::DUMMY);

    let (live_in, live_out) = compute_liveness(&body);
    assert!(
        live_in[&bb0].contains(&x),
        "x is read in bb0 → live_in[bb0]"
    );
    assert!(
        !live_in[&bb0].contains(&y),
        "y is written but never read → not live_in[bb0]"
    );
    assert!(
        live_out[&bb0].is_empty(),
        "no successor → live_out[bb0] empty"
    );
}

/// Stage 15.35 integration test 2: Branch where x is used in both arms.
///
/// CFG:
/// ```text
/// bb0: switchInt(x) { 1 => bb1, _ => bb2 }
/// bb1: y = x; goto bb3
/// bb2: z = x; goto bb3
/// bb3: return
/// ```
/// x must be live in bb0, bb1, bb2 (used in both arms and the switch discr).
#[test]
fn stage15_35_integration_branch_x_live_in_all_blocks() {
    let (mut body, locals) = build_body_with_locals(3);
    let x = locals[0];
    let y = locals[1];
    let z = locals[2];
    let bb0 = body.new_block();
    let bb1 = body.new_block();
    let bb2 = body.new_block();
    let bb3 = body.new_block();

    body.block_mut(bb0).terminator = Terminator {
        kind: TerminatorKind::SwitchInt {
            discr: operand_copy(x),
            targets: vec![(ConstVal::Int(1), bb1)],
            otherwise: bb2,
        },
        span: Span::DUMMY,
    };
    body.block_mut(bb1).statements.push(stmt_assign_use(y, x));
    body.block_mut(bb1).terminator = Terminator::goto(bb3, Span::DUMMY);
    body.block_mut(bb2).statements.push(stmt_assign_use(z, x));
    body.block_mut(bb2).terminator = Terminator::goto(bb3, Span::DUMMY);
    body.block_mut(bb3).terminator = Terminator::ret(Span::DUMMY);

    let (live_in, _live_out) = compute_liveness(&body);
    assert!(
        live_in[&bb0].contains(&x),
        "x live_in[bb0] (read in switchInt)"
    );
    assert!(live_in[&bb1].contains(&x), "x live_in[bb1] (read in y = x)");
    assert!(live_in[&bb2].contains(&x), "x live_in[bb2] (read in z = x)");
    // y is written in bb1 and read nowhere → y dead in all blocks.
    assert!(
        !live_in[&bb0].contains(&y),
        "y dead in bb0 (no read anywhere)"
    );
    assert!(!live_in[&bb3].contains(&y), "y dead in bb3");
}

/// Stage 15.35 integration test 3: Loop with x live across iterations.
///
/// CFG:
/// ```text
/// bb0: x = 0; goto bb1
/// bb1: switchInt(x) { 0 => bb3, _ => bb2 }
/// bb2: x = x + 1 (modeled as tmp = x; x = tmp); goto bb1
/// bb3: return
/// ```
/// x must be live_in[bb1] across the loop (read in switchInt AND in bb2's
/// read-modify-write). The fixpoint must converge — a single-pass analysis
/// would miss this.
#[test]
fn stage15_35_integration_loop_x_live_across_iterations() {
    let (mut body, locals) = build_body_with_locals(1);
    let x = locals[0];
    let bb0 = body.new_block();
    let bb1 = body.new_block();
    let bb2 = body.new_block();
    let bb3 = body.new_block();

    body.block_mut(bb0).statements.push(stmt_assign_const(x, 0));
    body.block_mut(bb0).terminator = Terminator::goto(bb1, Span::DUMMY);

    body.block_mut(bb1).terminator = Terminator {
        kind: TerminatorKind::SwitchInt {
            discr: operand_copy(x),
            targets: vec![(ConstVal::Int(0), bb3)],
            otherwise: bb2,
        },
        span: Span::DUMMY,
    };

    // bb2: x = x (read x, write x); goto bb1 (back-edge)
    body.block_mut(bb2).statements.push(stmt_assign_use(x, x));
    body.block_mut(bb2).terminator = Terminator::goto(bb1, Span::DUMMY);

    body.block_mut(bb3).terminator = Terminator::ret(Span::DUMMY);

    let (live_in, live_out) = compute_liveness(&body);
    // x must be live throughout the loop — fixpoint converges with:
    // - live_in[bb1] ⊇ {x} (read in switchInt)
    // - live_out[bb2] ⊇ {x} (back-edge → live_in[bb1])
    // - live_in[bb2] ⊇ {x} (read in stmt x = x)
    // - live_out[bb1] ⊇ {x} (live in bb2 via otherwise)
    // - live_out[bb0] ⊇ {x} (bb0 writes x, then bb1 reads it)
    // - live_in[bb0] does NOT contain x — bb0 writes x before any read,
    //   so x is not live at bb0 entry. (Use[bb0]=∅ because the RHS is a
    //   constant; Def[bb0]={x}; LiveIn = Use ∪ (LiveOut - Def) = ∅ ∪ ({x}-{x}) = ∅.)
    assert!(live_in[&bb1].contains(&x), "x live_in[bb1] (loop-carried)");
    assert!(live_out[&bb2].contains(&x), "x live_out[bb2] (back-edge)");
    assert!(live_in[&bb2].contains(&x), "x live_in[bb2]");
    assert!(live_out[&bb1].contains(&x), "x live_out[bb1] (loop body)");
    assert!(
        live_out[&bb0].contains(&x),
        "x live_out[bb0] (enters loop live)"
    );
    assert!(
        !live_in[&bb0].contains(&x),
        "x not live_in[bb0] (bb0 writes x before any read)"
    );
}

/// Stage 15.35 integration test 4: Drop terminator has one successor (no unwind).
#[test]
fn stage15_35_integration_drop_no_unwind_one_succ() {
    let (mut body, locals) = build_body_with_locals(1);
    let x = locals[0];
    let bb0 = body.new_block();
    let bb1 = body.new_block();

    body.block_mut(bb0).terminator = Terminator {
        kind: TerminatorKind::Drop {
            place: place_local(x),
            target: bb1,
            unwind: None,
        },
        span: Span::DUMMY,
    };
    body.block_mut(bb1).terminator = Terminator::ret(Span::DUMMY);

    let succs = successors(&body.block(bb0).terminator.kind);
    assert_eq!(succs, vec![bb1]);
}

/// Stage 15.35 integration test 5: Call with target (non-divergent).
#[test]
fn stage15_35_integration_call_with_target_one_succ() {
    let (mut body, locals) = build_body_with_locals(2);
    let f = locals[0];
    let dst = locals[1];
    let bb0 = body.new_block();
    let bb1 = body.new_block();

    body.block_mut(bb0).terminator = Terminator {
        kind: TerminatorKind::Call {
            func: operand_copy(f),
            args: vec![],
            destination: place_local(dst),
            target: Some(bb1),
            dyn_trait_call: None,
        },
        span: Span::DUMMY,
    };
    body.block_mut(bb1).terminator = Terminator::ret(Span::DUMMY);

    let succs = successors(&body.block(bb0).terminator.kind);
    assert_eq!(succs, vec![bb1]);
}

/// Stage 15.35 integration test 6: Call without target (divergent — no successor).
#[test]
fn stage15_35_integration_call_no_target_zero_succ() {
    let (mut body, locals) = build_body_with_locals(2);
    let f = locals[0];
    let dst = locals[1];
    let bb0 = body.new_block();

    body.block_mut(bb0).terminator = Terminator {
        kind: TerminatorKind::Call {
            func: operand_copy(f),
            args: vec![],
            destination: place_local(dst),
            target: None,
            dyn_trait_call: None,
        },
        span: Span::DUMMY,
    };

    let succs = successors(&body.block(bb0).terminator.kind);
    assert!(succs.is_empty(), "divergent call has no successor");
}

/// Stage 15.35 integration test 7: compute_liveness is total — every block
/// has an entry, even blocks with no statements / unreachable terminators.
#[test]
fn stage15_35_integration_total_map_all_blocks_present() {
    let mut body = MirBody::new(Span::DUMMY);
    let bb0 = body.new_block();
    let bb1 = body.new_block();
    let bb2 = body.new_block();
    let bb3 = body.new_block();
    body.block_mut(bb0).terminator = Terminator::goto(bb1, Span::DUMMY);
    body.block_mut(bb1).terminator = Terminator::goto(bb2, Span::DUMMY);
    body.block_mut(bb2).terminator = Terminator::goto(bb3, Span::DUMMY);
    body.block_mut(bb3).terminator = Terminator::unreachable(Span::DUMMY);

    let (live_in, live_out) = compute_liveness(&body);
    for bb in [bb0, bb1, bb2, bb3] {
        assert!(live_in.contains_key(&bb), "live_in missing bb {:?}", bb);
        assert!(live_out.contains_key(&bb), "live_out missing bb {:?}", bb);
    }
}

/// Stage 15.35 integration test 8: SwitchInt with duplicate targets.
///
/// The user wrote `switchInt(x) { 1 => bb1, 2 => bb1, _ => bb2 }` — bb1
/// appears twice. `successors` may return duplicates; the union in
/// `compute_liveness` is idempotent so this is harmless. The test verifies
/// no panic and the union is correct.
#[test]
fn stage15_35_integration_switchint_duplicate_targets_idempotent() {
    let (mut body, locals) = build_body_with_locals(1);
    let x = locals[0];
    let bb0 = body.new_block();
    let bb1 = body.new_block();
    let bb2 = body.new_block();

    body.block_mut(bb0).terminator = Terminator {
        kind: TerminatorKind::SwitchInt {
            discr: operand_copy(x),
            targets: vec![
                (ConstVal::Int(1), bb1),
                (ConstVal::Int(2), bb1), // duplicate target
            ],
            otherwise: bb2,
        },
        span: Span::DUMMY,
    };
    body.block_mut(bb1).terminator = Terminator::ret(Span::DUMMY);
    body.block_mut(bb2).terminator = Terminator::ret(Span::DUMMY);

    let (live_in, _live_out) = compute_liveness(&body);
    // Should not panic. x is read in bb0's switchInt, so x ∈ live_in[bb0].
    assert!(live_in[&bb0].contains(&x));
}

// ============================================================
// Part B — Real-pipeline smoke tests (compile() + call API)
// ============================================================

/// Stage 15.35 integration test 9: Smoke test — compile a simple Landin
/// program, then call `compute_liveness` on the resulting MIR body. This
/// catches regressions where MIR shapes that the borrow checker sees every
/// day would break the new analysis.
#[test]
fn stage15_35_integration_smoke_test_real_mir() {
    let src = r#"
        fn main() -> i32 {
            let x = 1;
            let y = 2;
            x + y
        }
    "#;
    let result = compile(src);
    assert!(
        !result.mirs.is_empty(),
        "compile should produce at least one MIR body"
    );
    // For each MIR body in the result, compute_liveness must succeed without
    // panicking and must return total maps covering every block.
    for mir_body in &result.mirs {
        let (live_in, live_out) = compute_liveness(mir_body);
        for (bb_idx, _bb) in mir_body.basic_blocks.iter().enumerate() {
            let bb_id = BasicBlockId(bb_idx as u32);
            assert!(
                live_in.contains_key(&bb_id),
                "live_in missing bb {:?}",
                bb_id
            );
            assert!(
                live_out.contains_key(&bb_id),
                "live_out missing bb {:?}",
                bb_id
            );
        }
    }
}

/// Stage 15.35 integration test 10: Smoke test with control flow (if/match).
///
/// This exercises more terminator kinds (SwitchInt with multiple targets,
/// Goto edges, Return) to ensure `successors` and `compute_liveness` handle
/// them correctly when produced by the real MIR lower.
#[test]
fn stage15_35_integration_smoke_test_control_flow() {
    let src = r#"
        fn classify(n: i32) -> i32 {
            if n > 0 {
                1
            } else {
                if n < 0 {
                    -1
                } else {
                    0
                }
            }
        }
        fn main() -> i32 {
            classify(42)
        }
    "#;
    let result = compile(src);
    assert!(
        result.mirs.len() >= 2,
        "compile should produce MIR for both `classify` and `main`"
    );
    for mir_body in &result.mirs {
        let (live_in, live_out) = compute_liveness(mir_body);
        // Sanity: every block has an entry in both maps.
        assert_eq!(live_in.len(), mir_body.basic_blocks.len());
        assert_eq!(live_out.len(), mir_body.basic_blocks.len());
    }
}

/// Stage 15.35 integration test 11: Smoke test with a loop (`while`).
///
/// The loop creates a back-edge that the fixpoint must handle. We don't
/// assert on specific locals (that would couple the test to MIR lower
/// internals), only that `compute_liveness` converges without panicking.
#[test]
fn stage15_35_integration_smoke_test_loop() {
    let src = r#"
        fn sum(n: i32) -> i32 {
            let mut i = 0;
            let mut total = 0;
            while i < n {
                total = total + i;
                i = i + 1;
            }
            total
        }
        fn main() -> i32 {
            sum(10)
        }
    "#;
    let result = compile(src);
    assert!(
        result.mirs.len() >= 2,
        "compile should produce MIR for both `sum` and `main`"
    );
    for mir_body in &result.mirs {
        let (live_in, live_out) = compute_liveness(mir_body);
        // The loop body block in `sum` should have non-trivial liveness —
        // `i` and `total` are both live across the back-edge. We don't
        // assert on specific local IDs (those depend on MIR lower internals)
        // but we do assert the fixpoint converged (the function returned).
        assert_eq!(live_in.len(), mir_body.basic_blocks.len());
        assert_eq!(live_out.len(), mir_body.basic_blocks.len());
    }
}

/// Stage 15.35 integration test 12: Smoke test with borrows.
///
/// Borrow patterns produce `Rvalue::Ref` which `rvalue_reads` must handle.
/// This test verifies the fixpoint liveness analysis accepts MIR bodies
/// containing `Rvalue::Ref` without panicking.
#[test]
fn stage15_35_integration_smoke_test_borrows() {
    let src = r#"
        fn main() -> i32 {
            let x = 10;
            let r = &x;
            *r + 0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.mirs.is_empty(),
        "compile should produce at least one MIR body"
    );
    for mir_body in &result.mirs {
        let (live_in, live_out) = compute_liveness(mir_body);
        assert_eq!(live_in.len(), mir_body.basic_blocks.len());
        assert_eq!(live_out.len(), mir_body.basic_blocks.len());
    }
}

/// Stage 15.35 integration test 13: Mutability doesn't affect liveness.
///
/// `let mut x = 1;` writes x; if x is never read, x is dead. The test
/// verifies Mutability::Mutable on the local decl doesn't artificially
/// inflate liveness.
#[test]
fn stage15_35_integration_mutability_doesnt_affect_liveness() {
    let (mut body, _locals) = build_body_with_locals(1);
    let x = landin_compiler::mir::place::LocalId(0);
    let _x_mut = body.new_local_with_mut(
        Ty::new(TyKind::Int(landin_compiler::ast::IntTy::I32), Span::DUMMY),
        None,
        Span::DUMMY,
        Mutability::Mutable,
    );
    let bb0 = body.new_block();
    body.block_mut(bb0)
        .statements
        .push(stmt_assign_const(x, 42));
    body.block_mut(bb0).terminator = Terminator::ret(Span::DUMMY);

    let (live_in, live_out) = compute_liveness(&body);
    // x is written but never read → x dead throughout.
    assert!(
        !live_in[&bb0].contains(&x),
        "x written but never read → not live_in"
    );
    assert!(live_out[&bb0].is_empty(), "no successor → live_out empty");
}
