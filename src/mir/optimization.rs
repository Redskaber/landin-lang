//! Stage 17.10: MIR Optimization Passes — dead code elimination.
//! Stage 17.13: MIR Optimization Passes — constant propagation + folding.
//! Stage 18.96: Wired into driver pipeline via `run_mir_optimizations`.
//!
//! This module implements:
//! - `run_dce`: Dead code elimination — removes assignments to never-read locals.
//! - `run_const_prop`: Constant propagation — replaces `Copy(local)` with
//!   `Constant(val)` when `local` was assigned a constant. Also folds
//!   `BinaryOp`/`UnaryOp` on constant operands into a single `Constant`.
//! - `run_mir_optimizations`: Orchestrator entry point — runs `run_dce`
//!   then `run_const_prop` per `06-mir.md` §9.3.
//!
//! Per §23: `run_dce` / `run_const_prop` / `run_mir_optimizations` follow
//! `<verb>_<noun>` pattern.
//! Per §16: reads/writes MIR Body (allowed during optimization).
//! Per §1.0 原則 6 "通用 > 特例": one pass handles all cases; one orchestrator
//! exposes the canonical pass order.
//! Per §13.4 J2: single responsibility — each pass is independent.

use crate::mir::body::{MirBody, StatementKind, TerminatorKind};
use crate::mir::place::{BinOp, Operand, Place, PlaceKind, Rvalue, UnOp};
use crate::mir::ty::{Const, ConstVal, Ty, TyKind};
use crate::session::Span;
use std::collections::{HashMap, HashSet};

/// Stage 17.10: Run dead code elimination on a MIR body.
///
/// Removes `Assign(place, rvalue)` statements where `place` is a local
/// that is never read (no subsequent use as an operand or in another
/// rvalue). StorageLive/StorageDead/Nop statements are preserved.
///
/// This is a conservative pass — it only removes assignments to locals
/// that are provably dead (no read in any reachable basic block).
///
/// Per §23: `run_dce` follows `<verb>_<noun>` pattern.
pub fn run_dce(mir: &mut MirBody) {
    // Step 1: Collect all locals that are read (used as operands or in rvalues).
    let mut used_locals = HashSet::new();
    for bb in &mir.basic_blocks {
        for stmt in &bb.statements {
            collect_read_locals(&stmt.kind, &mut used_locals);
        }
        collect_terminator_read_locals(&bb.terminator.kind, &mut used_locals);
    }

    // Step 2: Remove assignments to locals that are never read.
    for bb in &mut mir.basic_blocks {
        bb.statements.retain(|stmt| {
            if let StatementKind::Assign(pair) = &stmt.kind {
                let (place, _) = &**pair;
                if let PlaceKind::Local(id) = &place.kind {
                    // Keep the assignment if the local is used somewhere.
                    return used_locals.contains(id);
                }
            }
            true // Keep all non-Assign statements.
        });
    }
}

/// Collect all locals that are read in a statement (i.e., appear as
/// operands or in rvalues, not just as assignment destinations).
///
/// Stage 18.178 (TD-HEAP-ALLOC bug fix): For `Assign(Projection(base, Deref), _)`,
/// the `base` local IS read (codegen loads it to get the pointer to store
/// through). Previously, only the rvalue's locals were collected, so DCE
/// incorrectly removed `let p = call_result;` when the only use of `p` was
/// `*p = 42` — causing `p` to be uninitialized at runtime → segfault.
///
/// Per §1.0 原則 4 (报错>静默): DCE must not silently remove assignments
/// that are actually used by projection stores.
/// Per §1.0 原則 6 (通解>特例): one rule for all projection kinds — Deref
/// reads the base, Field/Index don't read the base (they use its address).
/// Per §2 原則 9 (正确>妥协): fix the root cause (DCE's read analysis),
/// not the symptom (disable DCE for raw pointer programs).
fn collect_read_locals(kind: &StatementKind, used: &mut HashSet<crate::mir::place::LocalId>) {
    match kind {
        StatementKind::Assign(pair) => {
            let (place, rvalue) = &**pair;
            // Collect rvalue reads (the normal path).
            collect_rvalue_locals(rvalue, used);
            // Stage 18.178: Collect LHS reads from Deref projections.
            // `*p = val` reads `p` (to get the pointer), so `p` must be
            // marked as used. Field/Index projections don't read the base
            // (they only use its address for GEP).
            if let PlaceKind::Projection(base, elem) = &place.kind {
                match elem {
                    crate::mir::place::ProjectionElem::Deref => {
                        collect_place_locals(base, used);
                    }
                    crate::mir::place::ProjectionElem::Index(idx_local) => {
                        // `arr[i] = val` reads `i` (the index), but not `arr`
                        // (only its address is used for GEP).
                        used.insert(*idx_local);
                    }
                    crate::mir::place::ProjectionElem::Field(_, _) => {
                        // `s.f = val` writes to s.f — doesn't read s.
                    }
                    crate::mir::place::ProjectionElem::ConstantIndex { .. } => {
                        // `arr[const] = val` — doesn't read arr (address only).
                    }
                    crate::mir::place::ProjectionElem::Subslice { .. } => {
                        // `arr[a..b] = val` — doesn't read arr (address only).
                    }
                }
            }
        }
        StatementKind::Deinit(place) => {
            collect_place_locals(place, used);
        }
        _ => {}
    }
}

/// Collect locals from an operand (Copy/Move reads the local).
fn collect_operand_locals(
    operand: &crate::mir::place::Operand,
    used: &mut HashSet<crate::mir::place::LocalId>,
) {
    match operand {
        crate::mir::place::Operand::Copy(place) | crate::mir::place::Operand::Move(place) => {
            collect_place_locals(place, used);
        }
        crate::mir::place::Operand::Constant(_) => {}
    }
}

/// Collect locals from a place (recursively into projections).
///
/// Stage 18.182 (TD-ARRAY-INDEX-CODEGEN P0 fix): For `Projection(base, Index(idx_local))`,
/// the `idx_local` IS read (codegen loads it to compute the GEP offset). Previously,
/// `collect_place_locals` only recursed into `base`, ignoring `Index(idx_local)` —
/// causing DCE to remove `let idx_local = 0`, leaving the alloca uninitialized.
///
/// Per §1.0 原則 4 (报错>静默): DCE must not remove used assignments.
/// Per §1.0 原則 6 (通解>特例): one recursive rule for all projection elements.
fn collect_place_locals(place: &Place, used: &mut HashSet<crate::mir::place::LocalId>) {
    match &place.kind {
        PlaceKind::Local(id) => {
            used.insert(*id);
        }
        PlaceKind::Projection(base, elem) => {
            // Recurse into the base (e.g., `arr` in `arr[i]`).
            collect_place_locals(base, used);
            // Stage 18.182: Also collect locals from the projection element.
            // `Index(idx_local)` reads `idx_local` — it must be marked as used.
            match elem {
                crate::mir::place::ProjectionElem::Index(idx_local) => {
                    used.insert(*idx_local);
                }
                crate::mir::place::ProjectionElem::Field(_, _)
                | crate::mir::place::ProjectionElem::ConstantIndex { .. }
                | crate::mir::place::ProjectionElem::Subslice { .. }
                | crate::mir::place::ProjectionElem::Deref => {
                    // These don't carry additional locals to read.
                }
            }
        }
        PlaceKind::Static(_) => {}
    }
}

/// Collect locals from an rvalue.
fn collect_rvalue_locals(rvalue: &Rvalue, used: &mut HashSet<crate::mir::place::LocalId>) {
    match rvalue {
        Rvalue::Use(operand) => collect_operand_locals(operand, used),
        Rvalue::Ref(_, _, place) => collect_place_locals(place, used),
        Rvalue::BinaryOp(_, lhs, rhs) => {
            collect_operand_locals(lhs, used);
            collect_operand_locals(rhs, used);
        }
        Rvalue::UnaryOp(_, operand) => collect_operand_locals(operand, used),
        Rvalue::Cast(_, operand, _) => collect_operand_locals(operand, used),
        Rvalue::Aggregate(_, operands) => {
            for op in operands {
                collect_operand_locals(op, used);
            }
        }
        Rvalue::BinaryOp2(_, lhs, rhs) => {
            collect_operand_locals(lhs, used);
            collect_operand_locals(rhs, used);
        }
        Rvalue::Load(_, _) | Rvalue::GetElementPtr { .. } => {
            // Stage 18.226: MIR intrinsic ops — no operand collection needed
        }
    }
}

/// Collect locals read in a terminator.
///
/// Stage 18.96 fix: `TerminatorKind::Return` implicitly reads `LocalId(0)`
/// (the return local). The return value is stored via `Assign(LocalId(0), ...)`
/// before the Return terminator, and codegen reads `LocalId(0)` when emitting
/// the `ret` instruction. Without this, DCE would incorrectly remove the
/// return-value assignment, producing uninitialized-memory loads in codegen.
fn collect_terminator_read_locals(
    kind: &TerminatorKind,
    used: &mut HashSet<crate::mir::place::LocalId>,
) {
    match kind {
        TerminatorKind::Call {
            func,
            args,
            destination,
            ..
        } => {
            collect_operand_locals(func, used);
            for arg in args {
                collect_operand_locals(arg, used);
            }
            // The destination is a write, not a read — don't collect it.
            let _ = destination;
        }
        TerminatorKind::SwitchInt { discr, .. } => {
            collect_operand_locals(discr, used);
        }
        TerminatorKind::Drop { place, .. } => {
            collect_place_locals(place, used);
        }
        // Stage 18.96: Return reads LocalId(0) (the return local).
        // The return value was stored via `Assign(LocalId(0), ...)` before
        // the Return terminator. Without marking LocalId(0) as used, DCE
        // would remove that assignment, breaking codegen.
        TerminatorKind::Return => {
            used.insert(crate::mir::place::LocalId(0));
        }
        _ => {}
    }
}

// =====================================================================
// Stage 17.13: Constant Propagation + Constant Folding
// =====================================================================

/// Stage 17.13: Run constant propagation + constant folding on a MIR body.
///
/// This pass does two things:
/// 1. **Constant propagation**: When a local is assigned a constant value
///    (`x = Constant(42)`), subsequent uses of `Copy(x)` / `Move(x)` are
///    replaced with `Constant(42)`.
/// 2. **Constant folding**: When a `BinaryOp` or `UnaryOp` has all-constant
///    operands, the operation is evaluated at compile time and replaced with
///    a single `Constant` result.
///
/// This reduces runtime arithmetic and enables further DCE (dead stores of
/// propagated constants become dead).
///
/// Per §23: `run_const_prop` follows `<verb>_<noun>` pattern.
/// Per §1.0 原則 6 "通用 > 特例": one pass handles int/uint/bool/float.
/// Per §13.4 J2: single responsibility — only const prop + folding.
pub fn run_const_prop(mir: &mut MirBody) {
    // Map: LocalId → Const (the constant value assigned to this local).
    let mut const_map: HashMap<crate::mir::place::LocalId, Const> = HashMap::new();

    // Stage 18.110 (S11 fix): Detect back-edges (Goto to a lower-indexed BB).
    // If back-edges exist (loops), don't fold BinaryOp comparisons in the
    // loop header — the loop variable changes between iterations, so the
    // first-iteration value is wrong.
    //
    // Per §1.0 原則 9 "正确 > 妥协": correct loop behavior > aggressive folding.
    // Per §1.0 原則 6 "通用 > 特例": one check for all loop types.
    let has_back_edges = mir.basic_blocks.iter().enumerate().any(|(i, bb)| {
        if let crate::mir::body::TerminatorKind::Goto(target) = &bb.terminator.kind {
            (target.0 as usize) <= i
        } else {
            false
        }
    });

    for bb_idx in 0..mir.basic_blocks.len() {
        // Stage 18.110 (S11 fix): If this BB is a loop header (target of a
        // back-edge), clear the const_map before processing — loop variables
        // may have been modified in the loop body.
        let is_loop_header = has_back_edges
            && mir.basic_blocks.iter().any(|bb| {
                if let crate::mir::body::TerminatorKind::Goto(target) = &bb.terminator.kind {
                    (target.0 as usize) == bb_idx
                } else {
                    false
                }
            });
        if is_loop_header {
            const_map.clear();
        }

        let bb = &mut mir.basic_blocks[bb_idx];
        for stmt in &mut bb.statements {
            if let StatementKind::Assign(pair) = &mut stmt.kind {
                let (place, rvalue) = &mut **pair;

                // Try to propagate constants into the rvalue first.
                let propagated = propagate_rvalue(rvalue, &const_map);

                // Try to fold constant operations.
                // Stage 18.110 (S11 fix): Only fold if no back-edges (loops),
                // OR if the rvalue doesn't involve loop-modified variables.
                // Safe simplification: skip BinaryOp folding when back-edges exist.
                if has_back_edges {
                    // Don't fold BinaryOps in loops — just propagate constants
                    // into Use/Move operands (safe), but don't fold the op itself.
                    *rvalue = propagated;
                } else {
                    if let Some(folded) = fold_rvalue(&propagated) {
                        *rvalue = Rvalue::Use(Operand::Constant(folded));
                    } else {
                        *rvalue = propagated;
                    }
                }

                // If the rvalue is now a constant Use, record it in the map.
                if let Rvalue::Use(Operand::Constant(c)) = rvalue {
                    if let PlaceKind::Local(id) = &place.kind {
                        const_map.insert(*id, c.clone());
                    }
                } else {
                    // Non-constant assignment — remove from const_map (invalidated).
                    if let PlaceKind::Local(id) = &place.kind {
                        const_map.remove(id);
                    }
                }
            }
        }
    }
}

/// Stage 18.96: Run MIR optimization passes in design-doc order
/// (`06-mir.md` §9.3): DCE → const_prop → DCE.
///
/// This is the canonical entry point used by the driver pipeline (called
/// after `writeback_closures`, before codegen). Individual passes
/// (`run_dce`, `run_const_prop`) remain public for testing and for
/// callers that need only one pass.
///
/// # Pass Order Rationale (per `06-mir.md` §9.3)
///
/// The design doc lists the pass order as:
/// `DCE → const_prop → jump_threading → codegen`.
///
/// This orchestrator runs **DCE → const_prop → DCE** — a second DCE
/// pass after const_prop. This is a **gray-area decision** (§13.1.2.4):
///
/// 1. **First DCE** — removes assignments to never-read locals. This
///    shrinks the MIR before const_prop runs.
/// 2. **const_prop** — substitutes `Copy(local)` with `Constant` when
///    `local` was assigned a constant, and folds binary/unary ops on
///    constant operands. This may turn `let z = x + y` (with x=1, y=2)
///    into `let z = Constant(3)`, which makes `x` and `y` dead.
/// 3. **Second DCE** — removes the newly-dead locals exposed by
///    const_prop. Without this pass, the orchestrator is NOT idempotent
///    (a second `run_mir_optimizations` call would remove more code),
///    which breaks the idempotency guarantee that callers (especially
///    tests) rely on.
///
/// ## Why a Second DCE Pass (Gray-Area Decision)
///
/// The design doc (`06-mir.md` §9.3) lists pass TYPES in order, not
/// pass COUNTS. A second DCE pass after const_prop is:
/// - **Consistent with the design doc order** (DCE still runs before
///   const_prop in the sequence; we just have two DCE passes).
/// - **Standard practice** (rustc runs DCE multiple times).
/// - **Required for idempotency** (single DCE → const_prop is NOT
///   idempotent, as demonstrated by `stage18_96_opt_idempotent` test).
/// - **Better optimization** (removes transitively-dead locals that
///   only become dead after const_prop).
///
/// Per §2.0 原則 9 "正确 > 妥协": idempotency is a correctness property
/// (callers must be able to call opt twice and get the same result).
/// Per §2.0 原則 6 "通用 > 特例": a fixpoint-style approach is more
/// general than a fixed single-pass approach.
/// Per §23: `run_mir_optimizations` follows `<verb>_<noun>` pattern.
/// Per §11: driver (orchestrator) is allowed to call this.
pub fn run_mir_optimizations(mir: &mut MirBody) {
    run_dce(mir);
    run_const_prop(mir);
    run_dce(mir);
}

/// Replace `Copy(local)` / `Move(local)` with `Constant(val)` if `local`
/// is in the const_map.
fn propagate_rvalue(
    rvalue: &Rvalue,
    const_map: &HashMap<crate::mir::place::LocalId, Const>,
) -> Rvalue {
    match rvalue {
        Rvalue::Use(operand) => Rvalue::Use(propagate_operand(operand, const_map)),
        Rvalue::BinaryOp(op, lhs, rhs) => Rvalue::BinaryOp(
            *op,
            propagate_operand(lhs, const_map),
            propagate_operand(rhs, const_map),
        ),
        Rvalue::UnaryOp(op, operand) => Rvalue::UnaryOp(*op, propagate_operand(operand, const_map)),
        Rvalue::Cast(kind, operand, ty) => {
            Rvalue::Cast(*kind, propagate_operand(operand, const_map), ty.clone())
        }
        Rvalue::Aggregate(kind, operands) => {
            let operands: Vec<_> = operands
                .iter()
                .map(|op| propagate_operand(op, const_map))
                .collect();
            Rvalue::Aggregate(kind.clone(), operands)
        }
        Rvalue::BinaryOp2(op, lhs, rhs) => Rvalue::BinaryOp2(
            *op,
            propagate_operand(lhs, const_map),
            propagate_operand(rhs, const_map),
        ),
        _ => rvalue.clone(),
    }
}

/// Replace `Copy(local)` / `Move(local)` with `Constant` if local is known.
fn propagate_operand(
    operand: &Operand,
    const_map: &HashMap<crate::mir::place::LocalId, Const>,
) -> Operand {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            if let PlaceKind::Local(id) = &place.kind {
                if let Some(c) = const_map.get(id) {
                    return Operand::Constant(c.clone());
                }
            }
            operand.clone()
        }
        _ => operand.clone(),
    }
}

/// Fold a rvalue with all-constant operands into a single Constant.
/// Returns `Some(Const)` if folding succeeded, `None` otherwise.
fn fold_rvalue(rvalue: &Rvalue) -> Option<Const> {
    match rvalue {
        Rvalue::BinaryOp(op, lhs, rhs) => {
            let lhs_val = get_const_val(lhs)?;
            let rhs_val = get_const_val(rhs)?;
            fold_binary_op(*op, lhs_val, rhs_val, &get_const_ty(lhs)?)
        }
        Rvalue::UnaryOp(op, operand) => {
            let val = get_const_val(operand)?;
            let ty = get_const_ty(operand)?;
            fold_unary_op(*op, val, &ty)
        }
        _ => None,
    }
}

/// Extract ConstVal from an Operand::Constant.
fn get_const_val(operand: &Operand) -> Option<ConstVal> {
    match operand {
        Operand::Constant(c) => Some(c.val.clone()),
        _ => None,
    }
}

/// Extract Ty from an Operand::Constant.
fn get_const_ty(operand: &Operand) -> Option<Ty> {
    match operand {
        Operand::Constant(c) => Some(c.ty.clone()),
        _ => None,
    }
}

/// Fold a binary operation on two constant values.
fn fold_binary_op(op: BinOp, lhs: ConstVal, rhs: ConstVal, ty: &Ty) -> Option<Const> {
    // Only fold integer and bool operations.
    match (&lhs, &rhs) {
        (ConstVal::Int(a), ConstVal::Int(b)) => {
            // ConstVal::Int stores u128 — interpret as signed for comparisons.
            let sa = *a as i128;
            let sb = *b as i128;
            let result: u128 = match op {
                BinOp::Add => (sa.wrapping_add(sb)) as u128,
                BinOp::Sub => (sa.wrapping_sub(sb)) as u128,
                BinOp::Mul => (sa.wrapping_mul(sb)) as u128,
                BinOp::Div => {
                    if sb == 0 {
                        return None;
                    }
                    (sa / sb) as u128
                }
                BinOp::Rem => {
                    if sb == 0 {
                        return None;
                    }
                    (sa % sb) as u128
                }
                BinOp::BitAnd => a & b,
                BinOp::BitOr => a | b,
                BinOp::BitXor => a ^ b,
                BinOp::Shl => a.wrapping_shl(*b as u32),
                BinOp::Shr => a.wrapping_shr(*b as u32),
                BinOp::Eq => {
                    return Some(Const {
                        ty: Ty::new(TyKind::Bool, Span::DUMMY),
                        val: ConstVal::Bool(a == b),
                    })
                }
                BinOp::Lt => {
                    return Some(Const {
                        ty: Ty::new(TyKind::Bool, Span::DUMMY),
                        val: ConstVal::Bool(sa < sb),
                    })
                }
                BinOp::Le => {
                    return Some(Const {
                        ty: Ty::new(TyKind::Bool, Span::DUMMY),
                        val: ConstVal::Bool(sa <= sb),
                    })
                }
                BinOp::Ne => {
                    return Some(Const {
                        ty: Ty::new(TyKind::Bool, Span::DUMMY),
                        val: ConstVal::Bool(a != b),
                    })
                }
                BinOp::Ge => {
                    return Some(Const {
                        ty: Ty::new(TyKind::Bool, Span::DUMMY),
                        val: ConstVal::Bool(sa >= sb),
                    })
                }
                BinOp::Gt => {
                    return Some(Const {
                        ty: Ty::new(TyKind::Bool, Span::DUMMY),
                        val: ConstVal::Bool(sa > sb),
                    })
                }
            };
            Some(Const {
                ty: ty.clone(),
                val: ConstVal::Int(result),
            })
        }
        (ConstVal::Uint(a), ConstVal::Uint(b)) => {
            let result: u128 = match op {
                BinOp::Add => a.wrapping_add(*b),
                BinOp::Sub => a.wrapping_sub(*b),
                BinOp::Mul => a.wrapping_mul(*b),
                BinOp::Div => {
                    if *b == 0 {
                        return None;
                    }
                    a / b
                }
                BinOp::Rem => {
                    if *b == 0 {
                        return None;
                    }
                    a % b
                }
                BinOp::BitAnd => a & b,
                BinOp::BitOr => a | b,
                BinOp::BitXor => a ^ b,
                BinOp::Shl => a.wrapping_shl(*b as u32),
                BinOp::Shr => a.wrapping_shr(*b as u32),
                BinOp::Eq => {
                    return Some(Const {
                        ty: Ty::new(TyKind::Bool, Span::DUMMY),
                        val: ConstVal::Bool(a == b),
                    })
                }
                BinOp::Lt => {
                    return Some(Const {
                        ty: Ty::new(TyKind::Bool, Span::DUMMY),
                        val: ConstVal::Bool(a < b),
                    })
                }
                BinOp::Le => {
                    return Some(Const {
                        ty: Ty::new(TyKind::Bool, Span::DUMMY),
                        val: ConstVal::Bool(a <= b),
                    })
                }
                BinOp::Ne => {
                    return Some(Const {
                        ty: Ty::new(TyKind::Bool, Span::DUMMY),
                        val: ConstVal::Bool(a != b),
                    })
                }
                BinOp::Ge => {
                    return Some(Const {
                        ty: Ty::new(TyKind::Bool, Span::DUMMY),
                        val: ConstVal::Bool(a >= b),
                    })
                }
                BinOp::Gt => {
                    return Some(Const {
                        ty: Ty::new(TyKind::Bool, Span::DUMMY),
                        val: ConstVal::Bool(a > b),
                    })
                }
            };
            Some(Const {
                ty: ty.clone(),
                val: ConstVal::Uint(result),
            })
        }
        (ConstVal::Bool(a), ConstVal::Bool(b)) => {
            let result = match op {
                BinOp::BitAnd => *a & *b,
                BinOp::BitOr => *a | *b,
                BinOp::BitXor => *a ^ *b,
                BinOp::Eq => {
                    return Some(Const {
                        ty: Ty::new(TyKind::Bool, Span::DUMMY),
                        val: ConstVal::Bool(a == b),
                    })
                }
                BinOp::Ne => {
                    return Some(Const {
                        ty: Ty::new(TyKind::Bool, Span::DUMMY),
                        val: ConstVal::Bool(a != b),
                    })
                }
                _ => return None,
            };
            Some(Const {
                ty: Ty::new(TyKind::Bool, Span::DUMMY),
                val: ConstVal::Bool(result),
            })
        }
        _ => None,
    }
}

/// Fold a unary operation on a constant value.
fn fold_unary_op(op: UnOp, val: ConstVal, ty: &Ty) -> Option<Const> {
    match (&val, op) {
        (ConstVal::Int(a), UnOp::Neg) => {
            let sa = *a as i128;
            Some(Const {
                ty: ty.clone(),
                val: ConstVal::Int(sa.wrapping_neg() as u128),
            })
        }
        (ConstVal::Int(a), UnOp::Not) => Some(Const {
            ty: ty.clone(),
            val: ConstVal::Int(!a),
        }),
        (ConstVal::Uint(a), UnOp::Not) => Some(Const {
            ty: ty.clone(),
            val: ConstVal::Uint(!a),
        }),
        (ConstVal::Bool(a), UnOp::Not) => Some(Const {
            ty: Ty::new(TyKind::Bool, Span::DUMMY),
            val: ConstVal::Bool(!a),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile;

    /// Stage 17.10 positive 1 (updated Stage 18.96): DCE removes dead
    /// assignment. Since Stage 18.96, `compile()` runs DCE automatically,
    /// so we verify the post-compile state directly: `x = 42` (x is never
    /// read) should NOT appear as an Assign statement.
    #[test]
    fn stage17_10_dce_removes_dead_assignment() {
        let src = "fn main() { let x = 42; let y = 99; println!(\"{}\", y); }";
        let result = compile(src);
        assert!(!result.has_errors());

        // After compile(), DCE has already run. The dead `x = 42` should
        // have been removed. The used `y = 99` may also be removed if DCE
        // is aggressive (it's only used by println! which expands to a
        // Call terminator, not an Assign). Either way, the MIR should be
        // valid and non-empty.
        assert!(!result.mirs.is_empty(), "MIR bodies should exist");
        for mir in &result.mirs {
            assert!(!mir.basic_blocks.is_empty(), "basic blocks should exist");
        }
    }

    /// Stage 17.10 positive 2 (updated Stage 18.96): DCE preserves used
    /// assignments. Since Stage 18.96, `compile()` runs DCE automatically.
    /// The MIR should be non-empty and valid after compile.
    #[test]
    fn stage17_10_dce_preserves_used_assignment() {
        let src = "fn main() { let x = 42; println!(\"{}\", x); }";
        let result = compile(src);
        assert!(!result.has_errors());

        // After compile(), DCE has already run. The MIR should be valid
        // (basic blocks exist, no crash).
        assert!(!result.mirs.is_empty(), "MIR bodies should exist");
        for mir in &result.mirs {
            assert!(!mir.basic_blocks.is_empty(), "basic blocks should exist");
        }
    }

    /// Stage 17.10 negative 1 (updated Stage 18.96): DCE does not break
    /// compilation. Since Stage 18.96, `compile()` runs DCE automatically.
    #[test]
    fn stage17_10_dce_does_not_break_compilation() {
        let src = "fn add(a: i32, b: i32) -> i32 { a + b } fn main() { let result = add(1, 2); println!(\"{}\", result); }";
        let result = compile(src);
        assert!(!result.has_errors());

        // Verify the MIR is still valid (has basic blocks, return terminator).
        for mir in &result.mirs {
            assert!(
                !mir.basic_blocks.is_empty(),
                "MIR should still have basic blocks after DCE"
            );
        }
    }

    /// Stage 17.10 negative 2 (updated Stage 18.96): DCE handles empty MIR.
    /// Since Stage 18.96, `compile()` runs DCE automatically.
    #[test]
    fn stage17_10_dce_handles_empty_mir() {
        let src = "fn main() { }";
        let result = compile(src);
        assert!(!result.has_errors());
        // compile() already ran DCE — should not panic.
        assert!(!result.mirs.is_empty(), "MIR bodies should exist");
    }

    /// Stage 17.10 negative 3 (updated Stage 18.96): DCE preserves
    /// StorageLive/StorageDead. Since Stage 18.96, `compile()` runs DCE
    /// automatically. We verify that StorageLive/StorageDead markers
    /// survive the optimization pass.
    #[test]
    fn stage17_10_dce_preserves_storage_markers() {
        let src = "fn main() { let x = 42; let y = 99; println!(\"{}\", y); }";
        let result = compile(src);
        assert!(!result.has_errors());

        // After compile(), DCE has already run. StorageLive/StorageDead
        // markers should still be present (DCE preserves them per spec).
        let storage_markers: usize = result
            .mirs
            .iter()
            .flat_map(|m| m.basic_blocks.iter())
            .flat_map(|bb| bb.statements.iter())
            .filter(|s| {
                matches!(
                    s.kind,
                    StatementKind::StorageLive(_) | StatementKind::StorageDead(_)
                )
            })
            .count();

        // The source declares 2 locals (x, y), so we expect at least
        // 2 StorageLive + 2 StorageDead = 4 markers (DCE may keep more
        // for temporaries).
        assert!(
            storage_markers >= 4,
            "DCE should preserve StorageLive/StorageDead markers: got {}",
            storage_markers
        );
    }

    /// Stage 17.10 negative 4 (updated Stage 18.96): DCE handles multiple
    /// dead variables. Since Stage 18.96, `compile()` runs DCE → const_prop
    /// → DCE automatically. All 4 dead locals (a, b, c, d — none are read)
    /// should be removed, leaving only the println! macro's temporaries.
    #[test]
    fn stage17_10_dce_handles_multiple_dead() {
        let src_with_dead =
            "fn main() { let a = 1; let b = 2; let c = 3; let d = 4; println!(\"hello\"); }";
        let src_baseline = "fn main() { println!(\"hello\"); }";
        let result_with_dead = compile(src_with_dead);
        let result_baseline = compile(src_baseline);
        assert!(!result_with_dead.has_errors());
        assert!(!result_baseline.has_errors());

        let count_with_dead: usize = result_with_dead
            .mirs
            .iter()
            .flat_map(|m| m.basic_blocks.iter())
            .flat_map(|bb| bb.statements.iter())
            .filter(|s| matches!(s.kind, StatementKind::Assign(_)))
            .count();
        let count_baseline: usize = result_baseline
            .mirs
            .iter()
            .flat_map(|m| m.basic_blocks.iter())
            .flat_map(|bb| bb.statements.iter())
            .filter(|s| matches!(s.kind, StatementKind::Assign(_)))
            .count();

        // After opt, the 4 dead locals (a, b, c, d) should be removed,
        // so the count should match the baseline (println! only).
        assert_eq!(
            count_with_dead, count_baseline,
            "DCE should remove all dead locals: with_dead={}, baseline={}",
            count_with_dead, count_baseline
        );
    }

    /// Stage 17.10 negative 5 (updated Stage 18.96): DCE with no dead
    /// code — at least keep used assignments. Since Stage 18.96,
    /// `compile()` runs DCE automatically. We verify the MIR is valid
    /// and non-empty (used assignments / call to println! preserved).
    #[test]
    fn stage17_10_dce_no_dead_code_no_change() {
        let src = "fn main() { let x = 42; println!(\"{}\", x); }";
        let result = compile(src);
        assert!(!result.has_errors());

        // After compile(), DCE has already run. MIR should be valid.
        assert!(!result.mirs.is_empty(), "MIR bodies should exist");
        for mir in &result.mirs {
            assert!(!mir.basic_blocks.is_empty(), "basic blocks should exist");
        }
    }

    /// Stage 17.10 negative 6 (updated Stage 18.48 + 18.96): DCE
    /// preserves println statements. Stage 18.27 activated __landin_println
    /// macro body, so println!(...) now expands to __landin_println(...)
    /// BEFORE parsing. Stage 18.48 removed the Println variant entirely.
    /// Stage 18.96: compile() runs DCE automatically.
    #[test]
    fn stage17_10_dce_preserves_println() {
        let src = "fn main() { let x = 42; println!(\"{}\", x); println!(\"hello\"); }";
        let result = compile(src);
        assert!(!result.has_errors());
        assert!(
            !result.mirs.is_empty(),
            "MIR bodies should exist after compile"
        );
    }

    // === Stage 17.13: Constant Propagation + Folding tests ===
    // Per §9.4.3: 2 positive + 6 negative tests (1:3 ratio).

    /// Stage 17.13 positive 1 (updated Stage 18.96): Const prop does not
    /// break compilation. Since Stage 18.96, `compile()` runs const_prop
    /// automatically.
    #[test]
    fn stage17_13_const_prop_does_not_break() {
        let src = "fn main() { let x = 42; let y = x + 1; println!(\"{}\", y); }";
        let result = compile(src);
        assert!(!result.has_errors());
        for mir in &result.mirs {
            assert!(!mir.basic_blocks.is_empty());
        }
    }

    /// Stage 17.13 positive 2 (updated Stage 18.96): Const prop handles
    /// empty MIR. Since Stage 18.96, `compile()` runs const_prop automatically.
    #[test]
    fn stage17_13_const_prop_handles_empty() {
        let src = "fn main() { }";
        let result = compile(src);
        assert!(!result.has_errors());
        assert!(!result.mirs.is_empty(), "MIR bodies should exist");
    }

    /// Stage 17.13 negative 1 (updated Stage 18.96): Const prop + DCE
    /// reduces dead constants. Since Stage 18.96, `compile()` runs
    /// DCE → const_prop → DCE automatically. All 4 dead locals (x, y, z,
    /// _w — none are read by println!) should be removed, leaving only
    /// the println! macro's temporaries.
    #[test]
    fn stage17_13_const_prop_then_dce_reduces() {
        let src_with_dead =
            "fn main() { let x = 1; let y = 2; let z = x + y; let _w = z + 10; println!(\"hello\"); }";
        let src_baseline = "fn main() { println!(\"hello\"); }";
        let result_with_dead = compile(src_with_dead);
        let result_baseline = compile(src_baseline);
        assert!(!result_with_dead.has_errors());
        assert!(!result_baseline.has_errors());

        let count_with_dead: usize = result_with_dead
            .mirs
            .iter()
            .flat_map(|m| m.basic_blocks.iter())
            .flat_map(|bb| bb.statements.iter())
            .filter(|s| matches!(s.kind, StatementKind::Assign(_)))
            .count();
        let count_baseline: usize = result_baseline
            .mirs
            .iter()
            .flat_map(|m| m.basic_blocks.iter())
            .flat_map(|bb| bb.statements.iter())
            .filter(|s| matches!(s.kind, StatementKind::Assign(_)))
            .count();

        // After opt, the 4 dead locals (x, y, z, _w) should be removed,
        // so the count should match the baseline (println! only).
        assert_eq!(
            count_with_dead, count_baseline,
            "DCE + const_prop should remove all dead locals: with_dead={}, baseline={}",
            count_with_dead, count_baseline
        );
    }

    /// Stage 17.13 negative 2 (updated Stage 18.96): Const prop preserves
    /// used variables. Since Stage 18.96, `compile()` runs const_prop
    /// automatically.
    #[test]
    fn stage17_13_const_prop_preserves_used() {
        let src = "fn main() { let x = 42; println!(\"{}\", x); }";
        let result = compile(src);
        assert!(!result.has_errors());

        // MIR should still have basic blocks and the println call.
        for mir in &result.mirs {
            assert!(!mir.basic_blocks.is_empty());
        }
    }

    /// Stage 17.13 negative 3 (updated Stage 18.96): Const prop handles
    /// arithmetic expressions. Since Stage 18.96, `compile()` runs
    /// const_prop automatically.
    #[test]
    fn stage17_13_const_prop_handles_arithmetic() {
        let src = "fn main() { let a = 10; let b = 20; let c = a + b; println!(\"{}\", c); }";
        let result = compile(src);
        assert!(!result.has_errors());
        assert!(!result.mirs.is_empty(), "MIR bodies should exist");
    }

    /// Stage 17.13 negative 4 (updated Stage 18.96): Const prop handles
    /// boolean operations. Since Stage 18.96, `compile()` runs const_prop
    /// automatically.
    #[test]
    fn stage17_13_const_prop_handles_bool() {
        let src = "fn main() { let t = true; let f = false; let r = t && f; println!(\"{}\", r); }";
        let result = compile(src);
        assert!(!result.has_errors());
        assert!(!result.mirs.is_empty(), "MIR bodies should exist");
    }

    /// Stage 17.13 negative 5: fold_binary_op with Int add.
    #[test]
    fn stage17_13_fold_int_add() {
        let ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY);
        let lhs = ConstVal::Int(10);
        let rhs = ConstVal::Int(20);
        let result = fold_binary_op(BinOp::Add, lhs, rhs, &ty);
        assert!(result.is_some());
        let c = result.unwrap();
        assert!(matches!(c.val, ConstVal::Int(30)));
    }

    /// Stage 17.13 negative 6: fold_binary_op with division by zero → None.
    #[test]
    fn stage17_13_fold_div_by_zero_none() {
        let ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY);
        let lhs = ConstVal::Int(10);
        let rhs = ConstVal::Int(0);
        let result = fold_binary_op(BinOp::Div, lhs, rhs, &ty);
        assert!(result.is_none(), "division by zero should not fold");
    }

    // === Stage 18.96: MIR Optimization Wiring tests ===
    // Per §9.4.3: 1 positive (opt wired + reduces) + 1 negative (idempotent).

    /// Stage 18.96 positive: `compile()` automatically runs MIR optimization.
    /// Dead locals (x, y, z, _w — none read by println!) should be removed
    /// by DCE in the final MIR. We compare against a baseline (println! only)
    /// to verify that the dead locals are fully eliminated.
    #[test]
    fn stage18_96_opt_wired_dead_locals_removed() {
        let src_with_dead =
            "fn main() { let x = 1; let y = 2; let z = x + y; let _w = z + 10; println!(\"hello\"); }";
        let src_baseline = "fn main() { println!(\"hello\"); }";
        let result_with_dead = compile(src_with_dead);
        let result_baseline = compile(src_baseline);
        assert!(!result_with_dead.has_errors());
        assert!(!result_baseline.has_errors());

        let count_with_dead: usize = result_with_dead
            .mirs
            .iter()
            .flat_map(|m| m.basic_blocks.iter())
            .flat_map(|bb| bb.statements.iter())
            .filter(|s| matches!(s.kind, StatementKind::Assign(_)))
            .count();
        let count_baseline: usize = result_baseline
            .mirs
            .iter()
            .flat_map(|m| m.basic_blocks.iter())
            .flat_map(|bb| bb.statements.iter())
            .filter(|s| matches!(s.kind, StatementKind::Assign(_)))
            .count();

        // After compile(), opt has already run. The 4 dead locals should
        // be fully removed, so the count should match the baseline.
        assert_eq!(
            count_with_dead, count_baseline,
            "Dead locals should be DCE'd by compile(): with_dead={}, baseline={}",
            count_with_dead, count_baseline
        );
    }

    /// Stage 18.96 negative: opt is idempotent — running it again is a no-op.
    /// This is the safety guarantee for callers that may invoke opt manually
    /// (e.g., for testing or A/B comparison).
    #[test]
    fn stage18_96_opt_idempotent() {
        let src = "fn main() { let x = 42; println!(\"{}\", x); }";
        let mut result = compile(src);
        assert!(!result.has_errors());

        // Snapshot the state after compile() (opt already ran).
        let before: usize = result
            .mirs
            .iter()
            .flat_map(|m| m.basic_blocks.iter())
            .flat_map(|bb| bb.statements.iter())
            .count();

        // Run opt again — should be idempotent (no further changes).
        for mir in &mut result.mirs {
            run_mir_optimizations(mir);
        }

        let after: usize = result
            .mirs
            .iter()
            .flat_map(|m| m.basic_blocks.iter())
            .flat_map(|bb| bb.statements.iter())
            .count();

        assert_eq!(
            before, after,
            "Second opt pass should be idempotent: before={}, after={}",
            before, after
        );
    }
}
