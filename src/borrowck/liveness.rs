//! Stage 6.14 (TD-024): NLL liveness analysis.
//!
//! Per 04-ownership-borrowing.md §4.3 (Liveness analysis). Extracted from
//! `mod.rs` per `docs/stage-committee-process.md` v3.21 §14.4 + §13.4.
//!
//! Owns:
//! - `LastUseMap` type alias
//! - `compute_last_use_map` (forward scan computing last-read point per local)
//! - 5 read-collection helpers: `statement_reads` / `rvalue_reads` /
//!   `operand_reads` / `place_root_reads` / `terminator_reads`

use crate::mir::body::*;
use crate::mir::place::*;

pub type LastUseMap = std::collections::HashMap<crate::mir::place::LocalId, (BasicBlockId, usize)>;

/// Compute the last-use map for a MIR body.
///
/// Walks all basic blocks in program order, recording the last program
/// point (bb_id, stmt_idx) where each local is read. The terminator is
/// treated as occupying stmt_idx == statements.len().
pub fn compute_last_use_map(mir: &MirBody) -> LastUseMap {
    let mut map: LastUseMap = std::collections::HashMap::new();
    for (bb_idx, bb) in mir.basic_blocks.iter().enumerate() {
        let bb_id = BasicBlockId(bb_idx as u32);
        for (stmt_idx, stmt) in bb.statements.iter().enumerate() {
            let point = (bb_id, stmt_idx);
            for local in statement_reads(stmt) {
                map.insert(local, point);
            }
        }
        // Terminator reads happen at idx == statements.len()
        let term_point = (bb_id, bb.statements.len());
        for local in terminator_reads(&bb.terminator) {
            map.insert(local, term_point);
        }
    }
    map
}

/// Collect all locals read by a statement (the RHS operands of an Assign).
pub fn statement_reads(stmt: &Statement) -> Vec<crate::mir::place::LocalId> {
    let mut out = Vec::new();
    if let StatementKind::Assign(boxed) = &stmt.kind {
        let (_place, rvalue) = &**boxed;
        // The LHS is a write, not a read — skip it.
        rvalue_reads(rvalue, &mut out);
    }
    out
}

/// Collect locals read by an rvalue.
pub fn rvalue_reads(rv: &Rvalue, out: &mut Vec<crate::mir::place::LocalId>) {
    match rv {
        Rvalue::Use(op) | Rvalue::Cast(_, op, _) => operand_reads(op, out),
        Rvalue::BinaryOp(_, a, b) | Rvalue::BinaryOp2(_, a, b) => {
            operand_reads(a, out);
            operand_reads(b, out);
        }
        Rvalue::UnaryOp(_, op) => operand_reads(op, out),
        Rvalue::Ref(_, _, lv) => place_root_reads(lv, out),
        Rvalue::Aggregate(_, operands) => {
            for op in operands {
                operand_reads(op, out);
            }
        }
    }
}

/// Collect locals read by an operand.
pub fn operand_reads(op: &Operand, out: &mut Vec<crate::mir::place::LocalId>) {
    match op {
        Operand::Copy(lv) | Operand::Move(lv) => place_root_reads(lv, out),
        Operand::Constant(_) => {}
    }
}

/// Collect the root local of a place (e.g., `a.x.y` → push `a`).
/// For `*p`, also push `p` (the pointer local).
pub fn place_root_reads(lv: &Place, out: &mut Vec<crate::mir::place::LocalId>) {
    match &lv.kind {
        PlaceKind::Local(id) => out.push(*id),
        PlaceKind::Static(_) => {}
        PlaceKind::Projection(base, _) => place_root_reads(base, out),
    }
}

/// Collect locals read by a terminator.
pub fn terminator_reads(term: &Terminator) -> Vec<crate::mir::place::LocalId> {
    let mut out = Vec::new();
    match term {
        Terminator::Call { func, args, .. } => {
            operand_reads(func, &mut out);
            for arg in args {
                operand_reads(arg, &mut out);
            }
        }
        Terminator::SwitchInt { discr, .. } => {
            operand_reads(discr, &mut out);
        }
        Terminator::Drop { place, .. } => {
            place_root_reads(place, &mut out);
        }
        Terminator::Assert { cond, .. } => {
            operand_reads(cond, &mut out);
        }
        _ => {}
    }
    out
}
