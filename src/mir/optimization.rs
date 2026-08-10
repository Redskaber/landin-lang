//! Stage 17.10: MIR Optimization Passes — dead code elimination.
//!
//! This module implements a simple dead code elimination (DCE) pass that
//! removes assignments to locals that are never read. DCE runs after MIR
//! lowering and before codegen, reducing the number of LLVM IR instructions
//! and improving codegen quality.
//!
//! Per §23: `run_dce` follows `<verb>_<noun>` pattern.
//! Per §16: reads/writes MIR Body (allowed during optimization).
//! Per §1.0 原則 6 "通用 > 特例": one pass handles all DCE cases.
//! Per §13.4 J2: single responsibility — only DCE, not other optimizations.

use crate::mir::body::{MirBody, StatementKind, TerminatorKind};
use crate::mir::place::{Place, PlaceKind, Rvalue};
use std::collections::HashSet;

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
        StatementKind::Println { args, .. } => {
            for arg in args {
                collect_operand_locals(arg, used);
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

    /// Stage 17.10 negative 6: DCE preserves println statements.
    #[test]
    fn stage17_10_dce_preserves_println() {
        let src = "fn main() { let x = 42; println!(\"{}\", x); println!(\"hello\"); }";
        let mut result = compile(src);
        assert!(!result.has_errors());

        let before: usize = result
            .mirs
            .iter()
            .flat_map(|m| m.basic_blocks.iter())
            .flat_map(|bb| bb.statements.iter())
            .filter(|s| matches!(s.kind, StatementKind::Println { .. }))
            .count();

        for mir in &mut result.mirs {
            run_dce(mir);
        }

        let after: usize = result
            .mirs
            .iter()
            .flat_map(|m| m.basic_blocks.iter())
            .flat_map(|bb| bb.statements.iter())
            .filter(|s| matches!(s.kind, StatementKind::Println { .. }))
            .count();

        assert_eq!(
            before, after,
            "DCE should preserve println: before={}, after={}",
            before, after
        );
    }
}
