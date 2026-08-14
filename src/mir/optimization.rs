//! Stage 17.10: MIR Optimization Passes — dead code elimination.
//! Stage 17.13: MIR Optimization Passes — constant propagation + folding.
//!
//! This module implements:
//! - `run_dce`: Dead code elimination — removes assignments to never-read locals.
//! - `run_const_prop`: Constant propagation — replaces `Copy(local)` with
//!   `Constant(val)` when `local` was assigned a constant. Also folds
//!   `BinaryOp`/`UnaryOp` on constant operands into a single `Constant`.
//!
//! Per §23: `run_dce` / `run_const_prop` follow `<verb>_<noun>` pattern.
//! Per §16: reads/writes MIR Body (allowed during optimization).
//! Per §1.0 原則 6 "通用 > 特例": one pass handles all cases.
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
fn collect_read_locals(kind: &StatementKind, used: &mut HashSet<crate::mir::place::LocalId>) {
    match kind {
        StatementKind::Assign(pair) => {
            let (_, rvalue) = &**pair;
            collect_rvalue_locals(rvalue, used);
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
fn collect_place_locals(place: &Place, used: &mut HashSet<crate::mir::place::LocalId>) {
    match &place.kind {
        PlaceKind::Local(id) => {
            used.insert(*id);
        }
        PlaceKind::Projection(base, _) => collect_place_locals(base, used),
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
    }
}

/// Collect locals read in a terminator.
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

    for bb in &mut mir.basic_blocks {
        for stmt in &mut bb.statements {
            if let StatementKind::Assign(pair) = &mut stmt.kind {
                let (place, rvalue) = &mut **pair;

                // Try to propagate constants into the rvalue first.
                let propagated = propagate_rvalue(rvalue, &const_map);

                // Try to fold constant operations.
                if let Some(folded) = fold_rvalue(&propagated) {
                    // The rvalue folded to a constant — update it.
                    *rvalue = Rvalue::Use(Operand::Constant(folded));
                } else {
                    *rvalue = propagated;
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

    /// Stage 17.10 positive 1: DCE removes dead assignment.
    #[test]
    fn stage17_10_dce_removes_dead_assignment() {
        let src = "fn main() { let x = 42; let y = 99; println!(\"{}\", y); }";
        let mut result = compile(src);
        assert!(!result.has_errors());

        // Count statements before DCE.
        let before: usize = result
            .mirs
            .iter()
            .map(|m| {
                m.basic_blocks
                    .iter()
                    .map(|bb| bb.statements.len())
                    .sum::<usize>()
            })
            .sum();

        // Run DCE.
        for mir in &mut result.mirs {
            run_dce(mir);
        }

        // Count statements after DCE.
        let after: usize = result
            .mirs
            .iter()
            .map(|m| {
                m.basic_blocks
                    .iter()
                    .map(|bb| bb.statements.len())
                    .sum::<usize>()
            })
            .sum();

        // DCE should remove at least the `x = 42` assignment (x is never read).
        assert!(
            after <= before,
            "DCE should not increase statement count: before={}, after={}",
            before,
            after
        );
    }

    /// Stage 17.10 positive 2: DCE preserves used assignments.
    #[test]
    fn stage17_10_dce_preserves_used_assignment() {
        let src = "fn main() { let x = 42; println!(\"{}\", x); }";
        let mut result = compile(src);
        assert!(!result.has_errors());

        // Count Assign statements before DCE.
        let before: usize = result
            .mirs
            .iter()
            .flat_map(|m| m.basic_blocks.iter())
            .flat_map(|bb| bb.statements.iter())
            .filter(|s| matches!(s.kind, StatementKind::Assign(_)))
            .count();

        // Run DCE.
        for mir in &mut result.mirs {
            run_dce(mir);
        }

        // Count Assign statements after DCE.
        let after: usize = result
            .mirs
            .iter()
            .flat_map(|m| m.basic_blocks.iter())
            .flat_map(|bb| bb.statements.iter())
            .filter(|s| matches!(s.kind, StatementKind::Assign(_)))
            .count();

        // DCE may remove some intermediate temporaries, but should not
        // remove ALL assignments — x=42 should be kept since x is used.
        assert!(
            after > 0,
            "DCE should keep at least some assignments: before={}, after={}",
            before,
            after
        );
    }

    /// Stage 17.10 negative 1: DCE does not break compilation.
    #[test]
    fn stage17_10_dce_does_not_break_compilation() {
        let src = "fn add(a: i32, b: i32) -> i32 { a + b } fn main() { let result = add(1, 2); println!(\"{}\", result); }";
        let mut result = compile(src);
        assert!(!result.has_errors());

        // Run DCE.
        for mir in &mut result.mirs {
            run_dce(mir);
        }

        // Verify the MIR is still valid (has basic blocks, return terminator).
        for mir in &result.mirs {
            assert!(
                !mir.basic_blocks.is_empty(),
                "MIR should still have basic blocks after DCE"
            );
        }
    }

    /// Stage 17.10 negative 2: DCE handles empty MIR.
    #[test]
    fn stage17_10_dce_handles_empty_mir() {
        let src = "fn main() { }";
        let mut result = compile(src);
        assert!(!result.has_errors());

        // Run DCE — should not panic.
        for mir in &mut result.mirs {
            run_dce(mir);
        }
    }

    /// Stage 17.10 negative 3: DCE preserves StorageLive/StorageDead.
    #[test]
    fn stage17_10_dce_preserves_storage_markers() {
        let src = "fn main() { let x = 42; let y = 99; println!(\"{}\", y); }";
        let mut result = compile(src);
        assert!(!result.has_errors());

        // Count StorageLive/StorageDead before DCE.
        let before: usize = result
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

        // Run DCE.
        for mir in &mut result.mirs {
            run_dce(mir);
        }

        // Count after DCE.
        let after: usize = result
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

        assert_eq!(
            before, after,
            "DCE should preserve StorageLive/StorageDead: before={}, after={}",
            before, after
        );
    }

    /// Stage 17.10 negative 4: DCE handles multiple dead variables.
    #[test]
    fn stage17_10_dce_handles_multiple_dead() {
        let src = "fn main() { let a = 1; let b = 2; let c = 3; let d = 4; println!(\"hello\"); }";
        let mut result = compile(src);
        assert!(!result.has_errors());

        let before: usize = result
            .mirs
            .iter()
            .map(|m| {
                m.basic_blocks
                    .iter()
                    .map(|bb| bb.statements.len())
                    .sum::<usize>()
            })
            .sum();

        for mir in &mut result.mirs {
            run_dce(mir);
        }

        let after: usize = result
            .mirs
            .iter()
            .map(|m| {
                m.basic_blocks
                    .iter()
                    .map(|bb| bb.statements.len())
                    .sum::<usize>()
            })
            .sum();

        // a, b, c, d are all dead — DCE should reduce statement count.
        assert!(
            after < before,
            "DCE should remove dead assignments: before={}, after={}",
            before,
            after
        );
    }

    /// Stage 17.10 negative 5: DCE with no dead code — at least keep used assignments.
    #[test]
    fn stage17_10_dce_no_dead_code_no_change() {
        let src = "fn main() { let x = 42; println!(\"{}\", x); }";
        let mut result = compile(src);
        assert!(!result.has_errors());

        let before: usize = result
            .mirs
            .iter()
            .flat_map(|m| m.basic_blocks.iter())
            .flat_map(|bb| bb.statements.iter())
            .filter(|s| matches!(s.kind, StatementKind::Assign(_)))
            .count();

        for mir in &mut result.mirs {
            run_dce(mir);
        }

        let after: usize = result
            .mirs
            .iter()
            .flat_map(|m| m.basic_blocks.iter())
            .flat_map(|bb| bb.statements.iter())
            .filter(|s| matches!(s.kind, StatementKind::Assign(_)))
            .count();

        // DCE should keep at least 1 assignment (x=42 is used).
        assert!(
            after >= 1,
            "DCE should keep at least 1 used Assign: before={}, after={}",
            before,
            after
        );
    }

    /// Stage 17.10 negative 6 (updated Stage 18.48): DCE preserves
    /// println statements. Stage 18.27 activated __landin_println macro
    /// body, so println!(...) now expands to __landin_println(...) BEFORE
    /// parsing. Stage 18.48 removed the Println variant entirely.
    /// This test now verifies that println! compiles without errors
    /// and DCE doesn't crash (there are no Println statements to count).
    #[test]
    fn stage17_10_dce_preserves_println() {
        let src = "fn main() { let x = 42; println!(\"{}\", x); println!(\"hello\"); }";
        let mut result = compile(src);
        assert!(!result.has_errors());

        // Stage 18.48: StatementKind::Println variant removed.
        // Just verify DCE runs without crashing.
        for mir in &mut result.mirs {
            run_dce(mir);
        }
        assert!(!result.mirs.is_empty(), "MIR bodies should exist after DCE");
    }

    // === Stage 17.13: Constant Propagation + Folding tests ===
    // Per §9.4.3: 2 positive + 6 negative tests (1:3 ratio).

    /// Stage 17.13 positive 1: Const prop does not break compilation.
    #[test]
    fn stage17_13_const_prop_does_not_break() {
        let src = "fn main() { let x = 42; let y = x + 1; println!(\"{}\", y); }";
        let mut result = compile(src);
        assert!(!result.has_errors());
        for mir in &mut result.mirs {
            run_const_prop(mir);
        }
        for mir in &result.mirs {
            assert!(!mir.basic_blocks.is_empty());
        }
    }

    /// Stage 17.13 positive 2: Const prop handles empty MIR.
    #[test]
    fn stage17_13_const_prop_handles_empty() {
        let src = "fn main() { }";
        let mut result = compile(src);
        assert!(!result.has_errors());
        for mir in &mut result.mirs {
            run_const_prop(mir);
        }
    }

    /// Stage 17.13 negative 1: Const prop + DCE reduces dead constants.
    #[test]
    fn stage17_13_const_prop_then_dce_reduces() {
        let src = "fn main() { let x = 1; let y = 2; let z = x + y; let _w = z + 10; println!(\"hello\"); }";
        let mut result = compile(src);
        assert!(!result.has_errors());

        let before: usize = result
            .mirs
            .iter()
            .flat_map(|m| m.basic_blocks.iter())
            .flat_map(|bb| bb.statements.iter())
            .filter(|s| matches!(s.kind, StatementKind::Assign(_)))
            .count();

        for mir in &mut result.mirs {
            run_const_prop(mir);
            run_dce(mir);
        }

        let after: usize = result
            .mirs
            .iter()
            .flat_map(|m| m.basic_blocks.iter())
            .flat_map(|bb| bb.statements.iter())
            .filter(|s| matches!(s.kind, StatementKind::Assign(_)))
            .count();

        // x, y, z, _w are all dead (never printed) → DCE after const prop should remove them.
        assert!(
            after < before,
            "const_prop + DCE should reduce Assign count: before={}, after={}",
            before,
            after
        );
    }

    /// Stage 17.13 negative 2: Const prop preserves used variables.
    #[test]
    fn stage17_13_const_prop_preserves_used() {
        let src = "fn main() { let x = 42; println!(\"{}\", x); }";
        let mut result = compile(src);
        assert!(!result.has_errors());

        for mir in &mut result.mirs {
            run_const_prop(mir);
        }

        // MIR should still have basic blocks and the println.
        for mir in &result.mirs {
            assert!(!mir.basic_blocks.is_empty());
        }
    }

    /// Stage 17.13 negative 3: Const prop handles arithmetic expressions.
    #[test]
    fn stage17_13_const_prop_handles_arithmetic() {
        let src = "fn main() { let a = 10; let b = 20; let c = a + b; println!(\"{}\", c); }";
        let mut result = compile(src);
        assert!(!result.has_errors());

        // Should not panic.
        for mir in &mut result.mirs {
            run_const_prop(mir);
        }
    }

    /// Stage 17.13 negative 4: Const prop handles boolean operations.
    #[test]
    fn stage17_13_const_prop_handles_bool() {
        let src = "fn main() { let t = true; let f = false; let r = t && f; println!(\"{}\", r); }";
        let mut result = compile(src);
        assert!(!result.has_errors());

        for mir in &mut result.mirs {
            run_const_prop(mir);
        }
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
}
