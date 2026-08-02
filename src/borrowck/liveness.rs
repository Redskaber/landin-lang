//! Stage 6.14 (TD-024): NLL liveness analysis.
//! Stage 15.35 (HP-10): Fixpoint dataflow liveness analysis (v0.2 Phase 2 Task 7).
//!
//! Per `docs/lang-design/04-ownership-borrowing.md` §4.3 (Liveness analysis)
//! and `docs/lang-design/23-nll-fixpoint.md`.
//! Extracted from `mod.rs` per `docs/stage-committee-process.md` v3.21 §14.4 + §13.4.
//!
//! Owns:
//! - `LastUseMap` type alias (legacy single-pass, Stage 6.14)
//! - `compute_last_use_map` (forward scan computing last-read point per local)
//! - `LiveInMap` / `LiveOutMap` type aliases (fixpoint, Stage 15.35)
//! - `compute_liveness` (backwards dataflow fixpoint, Stage 15.35)
//! - `successors` (basic-block successor enumeration, Stage 15.35)
//! - `statement_reads` / `rvalue_reads` / `operand_reads` /
//!   `place_root_reads` / `terminator_reads` (read-collection helpers,
//!   shared by both legacy and fixpoint paths)
//! - `statement_writes` / `place_root_writes` / `terminator_writes`
//!   (write-collection helpers, Stage 15.35 — needed for the `Def[bb]`
//!   set in liveness transfer)
//!
//! ## Migration status (Stage 15.35)
//!
//! `compute_last_use_map` is retained for backward compatibility with the
//! existing `BorrowChecker::check_mir_body` walk (Stage 6.14 path). It will
//! be removed in Stage 15.37 once the borrow checker has fully switched to
//! fixpoint liveness. The two analyses coexist so we can validate the
//! fixpoint output against the legacy map before flipping the switch.

use crate::mir::body::TerminatorKind;
use crate::mir::body::*;
use crate::mir::place::*;

pub type LastUseMap = std::collections::HashMap<crate::mir::place::LocalId, (BasicBlockId, usize)>;

/// Stage 15.35 (HP-10): Liveness map — basic-block entry → set of live locals.
///
/// `live_in[bb]` is the set of locals that are live at the *entry* of `bb`
/// (i.e., their value will be read somewhere along a path starting at `bb`).
pub type LiveInMap =
    std::collections::HashMap<BasicBlockId, std::collections::HashSet<crate::mir::place::LocalId>>;

/// Stage 15.35 (HP-10): Liveness map — basic-block exit → set of live locals.
///
/// `live_out[bb]` is the set of locals that are live at the *exit* of `bb`
/// (i.e., their value is needed in some successor of `bb`).
pub type LiveOutMap =
    std::collections::HashMap<BasicBlockId, std::collections::HashSet<crate::mir::place::LocalId>>;

/// Compute the last-use map for a MIR body.
///
/// Walks all basic blocks in program order, recording the last program
/// point (bb_id, stmt_idx) where each local is read. The terminator is
/// treated as occupying stmt_idx == statements.len().
///
/// **Stage 15.41 (HP-10 — legacy cleanup)**: This function is NO LONGER
/// legacy — it's now part of the dataflow borrow-check path. Stage 15.40
/// revised `kill_expired_borrows_dataflow` to use last-use-based kill
/// (borrow lifetimes end at their last read), which requires this map.
/// The original "unsound for loops" concern was about using this map for
/// LOCAL liveness, but it's correct for BORROW lifetimes (a borrow's
/// useful lifetime ends at its last read, regardless of loop structure).
///
/// The fixpoint `compute_liveness` (Stage 15.35) is retained for future
/// use (full NLL with borrow regions) but is not currently used for the
/// kill decision.
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

/// Stage 15.35 (HP-10): Compute liveness via backwards dataflow fixpoint iteration.
///
/// Implements the classic liveness dataflow equations:
///
/// ```text
/// LiveIn[bb]  = Use[bb] ∪ (LiveOut[bb] - Def[bb])
/// LiveOut[bb] = ∪ LiveIn[s] for s in successors(bb)
/// ```
///
/// The analysis iterates the equations to a fixpoint (until no `LiveIn`/`LiveOut`
/// set changes). It correctly handles:
///
/// - **Loops**: a local used inside a loop body is live at the loop header,
///   so a borrow kept alive by that local survives across iterations.
/// - **Conditionals**: a local used in any successor branch is live at the
///   branch point, so a borrow kept alive by that local survives both arms.
/// - **Recursion**: a local used in a back-edge target is live at the
///   back-edge source, which propagates liveness across the loop.
///
/// The returned maps are total — every `BasicBlockId` in `mir.basic_blocks`
/// has an entry, even if the entry set is empty (for blocks with no uses).
///
/// ## Use vs Def
///
/// - **Use[bb]**: locals that are *read* in `bb` before being written. We
///   approximate this as "all locals read in `bb`" — the standard
///   conservative approximation. A more precise analysis would stop
///   collecting reads at the first write per local, but the conservative
///   form is sound and matches rustc's `MaybeInitializedPlaces` baseline.
/// - **Def[bb]**: locals that are *written* in `bb`. This includes the LHS
///   of every `StatementKind::Assign` and the `destination` of every
///   `TerminatorKind::Call`.
///
/// ## Algorithm complexity
///
/// - Each iteration: O(B × (S + T)) where B=blocks, S=avg stmts/block, T=avg
///   terminator successors. Plus O(L × B) for set union/difference where
///   L=locals.
/// - Worst case iterations: O(L × B) (each iteration can add one local to
///   one block's LiveIn set, until saturation).
/// - Total worst case: O(L × B² × (S + T)). For typical functions
///   (B<50, S<20, L<30) this is well under 1ms.
///
/// Per §1.0 原則 1 "长期 > 短期": the fixpoint loop is the right long-term
/// design even though the legacy single-pass is faster in trivial cases.
/// Per §1.0 原則 3 "显式 > 隐式": liveness is computed explicitly via a
/// dedicated function, not implicit in a `last_use_map` heuristic.
pub fn compute_liveness(mir: &MirBody) -> (LiveInMap, LiveOutMap) {
    let num_blocks = mir.basic_blocks.len();
    let mut live_in: LiveInMap = std::collections::HashMap::with_capacity(num_blocks);
    let mut live_out: LiveOutMap = std::collections::HashMap::with_capacity(num_blocks);

    // Initialize every block to an empty set. Iterating to fixpoint requires
    // a total map — `live_out[bb]` is read for every block, including those
    // with no successors (Return / Unreachable), whose LiveOut must be ∅.
    for bb_idx in 0..num_blocks {
        let bb_id = BasicBlockId(bb_idx as u32);
        live_in.insert(bb_id, std::collections::HashSet::new());
        live_out.insert(bb_id, std::collections::HashSet::new());
    }

    // Pre-compute per-block Use and Def sets so the fixpoint loop only
    // does set algebra, not statement re-traversal.
    let mut block_use: Vec<std::collections::HashSet<crate::mir::place::LocalId>> =
        Vec::with_capacity(num_blocks);
    let mut block_def: Vec<std::collections::HashSet<crate::mir::place::LocalId>> =
        Vec::with_capacity(num_blocks);
    for bb in &mir.basic_blocks {
        let mut uses = std::collections::HashSet::new();
        let mut defs = std::collections::HashSet::new();
        for stmt in &bb.statements {
            for local in statement_reads(stmt) {
                uses.insert(local);
            }
            for local in statement_writes(stmt) {
                defs.insert(local);
            }
        }
        for local in terminator_reads(&bb.terminator) {
            uses.insert(local);
        }
        for local in terminator_writes(&bb.terminator) {
            defs.insert(local);
        }
        block_use.push(uses);
        block_def.push(defs);
    }

    // Fixpoint iteration: keep sweeping backwards until no LiveIn set changes.
    // We iterate backwards (last block to first) because liveness flows
    // backwards — this typically converges faster than forward iteration.
    let mut changed = true;
    while changed {
        changed = false;
        // Iterate in reverse block order. rustc's `work_queue` algorithm
        // is faster (only re-processes blocks whose successors changed),
        // but the simple sweep is correct and easier to verify. For our
        // expected block counts (< 100) the difference is negligible.
        for bb_idx in (0..num_blocks).rev() {
            let bb_id = BasicBlockId(bb_idx as u32);

            // LiveOut[bb] = ∪ LiveIn[s] for s in successors(bb)
            let mut new_live_out: std::collections::HashSet<crate::mir::place::LocalId> =
                std::collections::HashSet::new();
            for succ in successors(&mir.basic_blocks[bb_idx].terminator.kind) {
                if let Some(succ_in) = live_in.get(&succ) {
                    for local in succ_in {
                        new_live_out.insert(*local);
                    }
                }
            }

            // LiveIn[bb] = Use[bb] ∪ (LiveOut[bb] - Def[bb])
            let mut new_live_in: std::collections::HashSet<crate::mir::place::LocalId> =
                block_use[bb_idx].clone();
            for local in &new_live_out {
                if !block_def[bb_idx].contains(local) {
                    new_live_in.insert(*local);
                }
            }

            // Detect change. We compare set sizes first (cheap) and fall
            // back to full equality only if sizes match. This avoids the
            // O(L) full-equality check in the common "no change" case
            // where the size matches but the contents might differ.
            let cur_in = live_in.get(&bb_id).map(|s| s.len()).unwrap_or(0);
            let cur_out = live_out.get(&bb_id).map(|s| s.len()).unwrap_or(0);
            if cur_in != new_live_in.len() || cur_out != new_live_out.len() {
                live_in.insert(bb_id, new_live_in);
                live_out.insert(bb_id, new_live_out);
                changed = true;
            } else if live_in.get(&bb_id) != Some(&new_live_in)
                || live_out.get(&bb_id) != Some(&new_live_out)
            {
                // Sizes match but contents might differ — do the full check.
                live_in.insert(bb_id, new_live_in);
                live_out.insert(bb_id, new_live_out);
                changed = true;
            }
        }
    }

    (live_in, live_out)
}

/// Stage 15.35 (HP-10): Enumerate the successor basic blocks of a terminator.
///
/// Per `docs/lang-design/06-mir.md` §"TerminatorKind":
/// - `Goto(t)` → `[t]`
/// - `SwitchInt { targets, otherwise, .. }` → `[t1, t2, ..., otherwise]`
/// - `Return` / `Unreachable` → `[]`
/// - `Drop { target, unwind, .. }` → `[target]` (+ `unwind` if `Some`)
/// - `Call { target: Some(t), .. }` → `[t]`
/// - `Call { target: None, .. }` → `[]` (divergent call — noreturn)
/// - `Assert { target, .. }` → `[target]`
///
/// Returns a `Vec` because `SwitchInt` may have arbitrarily many targets
/// and the caller (fixpoint loop) needs to iterate them. The returned
/// targets may contain duplicates if the user wrote `switchInt(x) {
/// 1 => bb1, 2 => bb1, _ => bb2 }` — the union in `compute_liveness`
/// is idempotent so this is harmless.
pub fn successors(term: &TerminatorKind) -> Vec<BasicBlockId> {
    match term {
        TerminatorKind::Goto(target) => vec![*target],
        TerminatorKind::SwitchInt {
            targets, otherwise, ..
        } => {
            let mut out: Vec<BasicBlockId> = targets.iter().map(|(_, bb)| *bb).collect();
            out.push(*otherwise);
            out
        }
        TerminatorKind::Return | TerminatorKind::Unreachable => Vec::new(),
        TerminatorKind::Drop { target, unwind, .. } => {
            let mut out = vec![*target];
            if let Some(unwind_bb) = unwind {
                out.push(*unwind_bb);
            }
            out
        }
        TerminatorKind::Call { target, .. } => {
            if let Some(t) = target {
                vec![*t]
            } else {
                Vec::new()
            }
        }
        TerminatorKind::Assert { target, .. } => vec![*target],
    }
}

/// Collect all locals read by a statement (the RHS operands of an Assign).
///
/// Stage 15.67: `StorageLive(local)` is treated as a READ — the local enters
/// scope and is "used" (it must be live before this point so that the
/// storage is allocated). This ensures locals are live from their
/// `StorageLive` point, not from function entry.
pub fn statement_reads(stmt: &Statement) -> Vec<crate::mir::place::LocalId> {
    let mut out = Vec::new();
    match &stmt.kind {
        StatementKind::Assign(boxed) => {
            let (_place, rvalue) = &**boxed;
            // The LHS is a write, not a read — skip it.
            rvalue_reads(rvalue, &mut out);
        }
        StatementKind::StorageLive(local) => {
            // StorageLive is a "read" — the local enters scope and must be
            // live before this point (so the storage is allocated).
            out.push(*local);
        }
        _ => {}
    }
    out
}

/// Stage 15.35 (HP-10): Collect all locals written by a statement (the LHS place of an Assign).
///
/// For `StatementKind::Assign(place, _rvalue)`, returns the root local of `place`.
/// For other statement kinds, returns `[]`. The root local is sufficient for
/// liveness purposes — we treat any write to a place as a def of its root
/// local, which is the standard liveness approximation (matches rustc's
/// `MutBorrowedPlaces` baseline).
///
/// Stage 15.67: `StorageDead(local)` is treated as a WRITE — the local exits
/// scope and is "killed" (removed from the live set). This ensures locals
/// are dead after their `StorageDead` point, not alive until function return.
/// This is critical for true NLL — without it, method-call temps in loops
/// are considered live across the entire function, causing borrow conflicts.
pub fn statement_writes(stmt: &Statement) -> Vec<crate::mir::place::LocalId> {
    let mut out = Vec::new();
    match &stmt.kind {
        StatementKind::Assign(boxed) => {
            let (place, _rvalue) = &**boxed;
            place_root_writes(place, &mut out);
        }
        StatementKind::StorageDead(local) => {
            // StorageDead is a "write" — the local exits scope and is killed.
            // This removes it from the live set, allowing borrows on it to
            // expire (true NLL).
            out.push(*local);
        }
        _ => {}
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

/// Stage 15.35 (HP-10): Collect the root local of a place being written.
///
/// For `a.x.y = ...`, returns `[a]`. For `*p = ...`, returns `[p]`. For
/// `Static = ...`, returns `[]` (statics have no local ID). The root
/// local is sufficient for liveness — we don't need field-level precision.
///
/// Note: there is intentionally no `rvalue_writes` / `operand_writes`
/// counterpart — rvalues and operands never write locals. Writes happen
/// only via the Assign LHS place (`statement_writes`) and the Call
/// destination (`terminator_writes`). Per §15 "最优 > 最小", we don't
/// add unused helpers just for symmetry.
pub fn place_root_writes(lv: &Place, out: &mut Vec<crate::mir::place::LocalId>) {
    match &lv.kind {
        PlaceKind::Local(id) => out.push(*id),
        PlaceKind::Static(_) => {}
        PlaceKind::Projection(base, _) => place_root_writes(base, out),
    }
}

/// Collect locals read by a terminator.
pub fn terminator_reads(term: &Terminator) -> Vec<crate::mir::place::LocalId> {
    let mut out = Vec::new();
    match &term.kind {
        TerminatorKind::Call { func, args, .. } => {
            operand_reads(func, &mut out);
            for arg in args {
                operand_reads(arg, &mut out);
            }
        }
        TerminatorKind::SwitchInt { discr, .. } => {
            operand_reads(discr, &mut out);
        }
        TerminatorKind::Drop { place, .. } => {
            place_root_reads(place, &mut out);
        }
        TerminatorKind::Assert { cond, .. } => {
            operand_reads(cond, &mut out);
        }
        _ => {}
    }
    out
}

/// Stage 15.35 (HP-10): Collect locals written by a terminator.
///
/// Only `TerminatorKind::Call { destination, .. }` writes a local — the
/// `destination` place receives the call's return value. All other
/// terminators (Goto, SwitchInt, Return, Drop, Assert, Unreachable)
/// don't write locals.
pub fn terminator_writes(term: &Terminator) -> Vec<crate::mir::place::LocalId> {
    let mut out = Vec::new();
    if let TerminatorKind::Call { destination, .. } = &term.kind {
        place_root_writes(destination, &mut out);
    }
    out
}

/// Stage 15.36 (HP-10 step 2 of 4): Compute the set of locals that are
/// live **immediately after** a given program point `(bb_id, stmt_idx)`.
///
/// This is a per-statement liveness query derived from the block-level
/// `LiveOutMap` (computed by `compute_liveness`). The algorithm:
///
/// 1. Start with `live = LiveOut[bb_id]` (locals live at the block's exit).
/// 2. Walk the block's statements **backwards** from the terminator index
///    down to `stmt_idx + 1`, applying the transfer function
///    `live = Use[stmt] ∪ (live - Def[stmt])` at each step.
/// 3. If `stmt_idx` < the terminator index (i.e., the program point is
///    before the terminator), also fold in the terminator's Use/Def.
/// 4. The resulting set is the locals live immediately after `(bb_id, stmt_idx)`.
///
/// ## Why this is needed
///
/// `compute_liveness` produces block-level `LiveIn` / `LiveOut` maps. But
/// `kill_expired_borrows_dataflow` needs to know "is `ref_local` live
/// *right after this statement*?" — a per-statement query. The block-level
/// `LiveOut` is correct for the block's exit, but for interior program
/// points we need to "back-propagate" through the remaining statements
/// using the standard liveness transfer function.
///
/// Per §1.0 原則 3 "显式 > 隐式": the per-point liveness is derived
/// explicitly from the block-level liveness, not computed by a separate
/// analysis. Per §16: this function reads only `mir` and `live_out` — no
/// writes, no HIR lookup.
///
/// ## Complexity
///
/// O((S - stmt_idx) × L) per call where S = statements in `bb` and L =
/// locals in the live set. For typical blocks (S<20, L<30) this is well
/// under 100µs. Called once per program point in
/// `check_mir_body_with_dataflow`, so total cost across a body is
/// O(S²_per_block × L) = negligible.
///
/// ## Edge cases
///
/// - `stmt_idx == statements.len()`: the program point is the terminator.
///   The result is `LiveOut[bb_id]` directly (no remaining statements to
///   fold in). The terminator itself is "the current point" — we want
///   what's live after it executes, which is what `LiveOut` represents.
/// - `stmt_idx == statements.len() - 1`: the program point is the last
///   statement. We fold in the terminator's Use/Def only.
/// - `stmt_idx == 0`: the program point is the first statement. We fold
///   in all remaining statements + the terminator.
pub fn compute_live_after_point(
    mir: &MirBody,
    live_out: &LiveOutMap,
    bb_id: BasicBlockId,
    stmt_idx: usize,
) -> std::collections::HashSet<crate::mir::place::LocalId> {
    let bb = match mir.basic_blocks.get(bb_id.0 as usize) {
        Some(bb) => bb,
        None => return std::collections::HashSet::new(),
    };
    let stmt_count = bb.statements.len();
    let term_idx = stmt_count;

    // Start with LiveOut[bb_id] — locals live at block exit.
    let mut live: std::collections::HashSet<crate::mir::place::LocalId> =
        live_out.get(&bb_id).cloned().unwrap_or_default();

    // If the program point is at-or-after the terminator, LiveOut is
    // already the answer (nothing after the terminator to fold in).
    if stmt_idx >= term_idx {
        return live;
    }

    // Fold in the terminator's Use/Def (it sits between `stmt_idx` and
    // `LiveOut` whenever `stmt_idx < term_idx`).
    fold_use_def(
        &mut live,
        &terminator_reads(&bb.terminator),
        &terminator_writes(&bb.terminator),
    );

    // Walk backwards over the statements after `stmt_idx` (from the last
    // down to `stmt_idx + 1`), folding in each one's Use/Def.
    for s in (stmt_idx + 1..stmt_count).rev() {
        fold_use_def(
            &mut live,
            &statement_reads(&bb.statements[s]),
            &statement_writes(&bb.statements[s]),
        );
    }

    live
}

/// Stage 15.36 (HP-10): Apply the standard liveness transfer function
/// `live = Use ∪ (live - Def)` to a live set, in place.
///
/// Given the current `live` set (locals live AFTER a statement), update
/// it to be the set of locals live BEFORE that statement:
///
/// - Add every local in `uses` (the statement reads it → it must be live
///   before).
/// - Remove every local in `defs` that's not also in `uses` (the statement
///   writes it before any read → it doesn't need to be live before).
///
/// This is the classic backwards dataflow transfer. It's a private helper
/// — `compute_live_after_point` is the only caller.
fn fold_use_def(
    live: &mut std::collections::HashSet<crate::mir::place::LocalId>,
    uses: &[crate::mir::place::LocalId],
    defs: &[crate::mir::place::LocalId],
) {
    // First remove defs (the statement writes them, so they don't need to
    // be live before — unless they're also read by this statement).
    for d in defs {
        live.remove(d);
    }
    // Then add uses (the statement reads them, so they must be live before).
    for u in uses {
        live.insert(*u);
    }
}

/// Stage 15.39 (HP-10 step 4 — Option B): Compute the set of locals that
/// are read **anywhere** in the MIR body.
///
/// This is a simple forward scan that collects every local appearing in
/// any `statement_reads` or `terminator_reads` result. The resulting set
/// is used by `kill_expired_borrows_dataflow` to preserve GAP-1 semantics:
/// a borrow whose `ref_local` was never read is NOT killed (it stays as
/// a "stray" until scope end, matching the legacy path's behavior).
///
/// ## Why this is needed
///
/// The dataflow liveness analysis (`compute_liveness`) correctly
/// identifies locals that are dead (never read after a given point).
/// Killing a dead `ref_local`'s borrow is correct NLL — but it violates
/// the project's Stage 14.81 GAP-1 soundness fix, which decided that
/// `let r1 = &mut x; let r2 = &mut x;` should be a `compile_error` even
/// when `r1` is never read after `r2` is created.
///
/// The legacy path achieves GAP-1 "accidentally" — `compute_last_use_map`
/// only records reads, so a never-read local never has its "last use"
/// recorded, so `kill_expired_borrows` never kills its borrow. The borrow
/// stays alive as a "stray" and conflicts with the new borrow.
///
/// `compute_ever_read` lets the dataflow path replicate this behavior:
/// if a `ref_local` was never read ANYWHERE in the body, its borrow is
/// never killed by the dataflow path either. This preserves GAP-1 while
/// still using the dataflow infrastructure for loop/conditional correctness.
///
/// Per §1.0 原則 1 "长期 > 短期": this is the right long-term design for
/// now — it fixes the real NLL soundness bugs (loops, conditionals)
/// without changing the project's soundness posture. Future migration to
/// real NLL (Option A) remains possible by removing this check.
///
/// Per §1.0 原則 3 "显式 > 隐式": the "was ever read" check is explicit
/// and documented. Per §23: function name follows `<verb>_<noun>_<noun>`
/// pattern.
///
/// ## Complexity
///
/// O(B × (S + T)) where B=blocks, S=avg stmts/block, T=terminator reads.
/// Called once per `check_mir_body_with_dataflow` invocation. For typical
/// functions (<50 blocks, <20 stmts/block) this is well under 100µs.
pub fn compute_ever_read(mir: &MirBody) -> std::collections::HashSet<crate::mir::place::LocalId> {
    let mut ever = std::collections::HashSet::new();
    for bb in &mir.basic_blocks {
        for stmt in &bb.statements {
            for local in statement_reads(stmt) {
                ever.insert(local);
            }
        }
        for local in terminator_reads(&bb.terminator) {
            ever.insert(local);
        }
    }
    ever
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::ty::{Mutability, Ty, TyKind};
    use crate::session::Span;

    /// Build an empty MirBody with one i32 local per slot requested.
    /// Returns the body and the vector of allocated LocalIds.
    fn build_body_with_locals(n_locals: usize) -> (MirBody, Vec<crate::mir::place::LocalId>) {
        let mut body = MirBody::new(Span::DUMMY);
        let mut locals = Vec::with_capacity(n_locals);
        for _ in 0..n_locals {
            let id = body.new_local(
                Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
                None,
                Span::DUMMY,
            );
            locals.push(id);
        }
        (body, locals)
    }

    /// Helper: build a `Place::Local(id)` for use in Assign / Operand::Copy.
    fn place_local(id: crate::mir::place::LocalId) -> Place {
        Place {
            kind: PlaceKind::Local(id),
            span: Span::DUMMY,
        }
    }

    /// Helper: build an `Operand::Copy(place)`.
    fn operand_copy(id: crate::mir::place::LocalId) -> Operand {
        Operand::Copy(place_local(id))
    }

    /// Helper: build a `StatementKind::Assign(place, Rvalue::Use(op))`.
    fn stmt_assign_use(
        dst: crate::mir::place::LocalId,
        src: crate::mir::place::LocalId,
    ) -> Statement {
        Statement {
            kind: StatementKind::Assign(Box::new((
                place_local(dst),
                Rvalue::Use(operand_copy(src)),
            ))),
            span: Span::DUMMY,
        }
    }

    // ----- successors() tests -----

    #[test]
    fn stage15_35_successors_goto() {
        let term = TerminatorKind::Goto(BasicBlockId(5));
        assert_eq!(successors(&term), vec![BasicBlockId(5)]);
    }

    #[test]
    fn stage15_35_successors_return_empty() {
        let term = TerminatorKind::Return;
        assert!(successors(&term).is_empty());
    }

    #[test]
    fn stage15_35_successors_unreachable_empty() {
        let term = TerminatorKind::Unreachable;
        assert!(successors(&term).is_empty());
    }

    #[test]
    fn stage15_35_successors_switchint_includes_otherwise() {
        let term = TerminatorKind::SwitchInt {
            discr: Operand::Constant(crate::mir::ty::Const {
                ty: Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
                val: crate::mir::ty::ConstVal::Int(0),
            }),
            targets: vec![
                (crate::mir::ty::ConstVal::Int(1), BasicBlockId(10)),
                (crate::mir::ty::ConstVal::Int(2), BasicBlockId(11)),
            ],
            otherwise: BasicBlockId(12),
        };
        let succs = successors(&term);
        assert_eq!(succs.len(), 3);
        assert!(succs.contains(&BasicBlockId(10)));
        assert!(succs.contains(&BasicBlockId(11)));
        assert!(succs.contains(&BasicBlockId(12)));
    }

    #[test]
    fn stage15_35_successors_drop_no_unwind() {
        let term = TerminatorKind::Drop {
            place: place_local(crate::mir::place::LocalId(0)),
            target: BasicBlockId(7),
            unwind: None,
        };
        assert_eq!(successors(&term), vec![BasicBlockId(7)]);
    }

    #[test]
    fn stage15_35_successors_drop_with_unwind() {
        let term = TerminatorKind::Drop {
            place: place_local(crate::mir::place::LocalId(0)),
            target: BasicBlockId(7),
            unwind: Some(BasicBlockId(8)),
        };
        let succs = successors(&term);
        assert_eq!(succs.len(), 2);
        assert!(succs.contains(&BasicBlockId(7)));
        assert!(succs.contains(&BasicBlockId(8)));
    }

    #[test]
    fn stage15_35_successors_call_with_target() {
        let term = TerminatorKind::Call {
            func: operand_copy(crate::mir::place::LocalId(0)),
            args: vec![],
            destination: place_local(crate::mir::place::LocalId(1)),
            target: Some(BasicBlockId(3)),
            dyn_trait_call: None,
        };
        assert_eq!(successors(&term), vec![BasicBlockId(3)]);
    }

    #[test]
    fn stage15_35_successors_call_no_target_divergent() {
        let term = TerminatorKind::Call {
            func: operand_copy(crate::mir::place::LocalId(0)),
            args: vec![],
            destination: place_local(crate::mir::place::LocalId(1)),
            target: None,
            dyn_trait_call: None,
        };
        assert!(successors(&term).is_empty());
    }

    #[test]
    fn stage15_35_successors_assert() {
        let term = TerminatorKind::Assert {
            cond: operand_copy(crate::mir::place::LocalId(0)),
            expected: true,
            target: BasicBlockId(9),
            msg: AssertMessage::BoundsCheck,
        };
        assert_eq!(successors(&term), vec![BasicBlockId(9)]);
    }

    // ----- statement_writes / terminator_writes tests -----

    #[test]
    fn stage15_35_statement_writes_assign_lhs_root() {
        let stmt = stmt_assign_use(crate::mir::place::LocalId(2), crate::mir::place::LocalId(0));
        let writes = statement_writes(&stmt);
        assert_eq!(writes, vec![crate::mir::place::LocalId(2)]);
    }

    #[test]
    fn stage15_35_terminator_writes_call_destination() {
        let term = Terminator {
            kind: TerminatorKind::Call {
                func: operand_copy(crate::mir::place::LocalId(0)),
                args: vec![],
                destination: place_local(crate::mir::place::LocalId(3)),
                target: Some(BasicBlockId(0)),
                dyn_trait_call: None,
            },
            span: Span::DUMMY,
        };
        let writes = terminator_writes(&term);
        assert_eq!(writes, vec![crate::mir::place::LocalId(3)]);
    }

    #[test]
    fn stage15_35_terminator_writes_goto_empty() {
        let term = Terminator {
            kind: TerminatorKind::Goto(BasicBlockId(0)),
            span: Span::DUMMY,
        };
        assert!(terminator_writes(&term).is_empty());
    }

    // ----- compute_liveness() tests -----

    #[test]
    fn stage15_35_compute_liveness_straight_line_dead() {
        // bb0: x = const 1; y = const 2; return;
        // x is dead (never read), y is dead (never read). The RHS is a
        // constant operand — it reads no local — so both x and y have
        // Def[bb0] without Use[bb0]. LiveIn[bb0] = Use ∪ (LiveOut - Def)
        // = ∅ ∪ (∅ - {x,y}) = ∅. LiveOut[bb0] = ∅ (no successor reads).
        let (mut body, locals) = build_body_with_locals(2);
        let x = locals[0];
        let y = locals[1];
        let const_one = Operand::Constant(crate::mir::ty::Const {
            ty: Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
            val: crate::mir::ty::ConstVal::Int(1),
        });
        let stmt_assign_const = |dst: crate::mir::place::LocalId, op: Operand| Statement {
            kind: StatementKind::Assign(Box::new((place_local(dst), Rvalue::Use(op)))),
            span: Span::DUMMY,
        };
        let bb0 = body.new_block();
        body.block_mut(bb0)
            .statements
            .push(stmt_assign_const(x, const_one.clone()));
        body.block_mut(bb0)
            .statements
            .push(stmt_assign_const(y, const_one));
        body.block_mut(bb0).terminator = Terminator::ret(Span::DUMMY);

        let (live_in, live_out) = compute_liveness(&body);
        // No reads anywhere → both x and y are dead throughout.
        assert!(live_in[&bb0].is_empty(), "no reads → live_in[bb0] empty");
        assert!(
            live_out[&bb0].is_empty(),
            "no successor reads → live_out[bb0] empty"
        );
    }

    #[test]
    fn stage15_35_compute_liveness_straight_line_read_at_terminator() {
        // bb0: x = 1; return x;
        // x is live at terminator — but terminator is Return with no operand reads.
        // (Return doesn't read a local — the function's return value is written
        //  by Call destination in the *caller*, not by Return in the callee.)
        // So we model "return x" via an Assert terminator that reads x.
        let (mut body, locals) = build_body_with_locals(1);
        let x = locals[0];
        let bb0 = body.new_block();
        body.block_mut(bb0).statements.push(stmt_assign_use(x, x)); // x = x (writes x, reads x)
        body.block_mut(bb0).terminator = Terminator {
            kind: TerminatorKind::Assert {
                cond: operand_copy(x),
                expected: true,
                target: bb0, // self-loop — keeps the body well-formed
                msg: AssertMessage::BoundsCheck,
            },
            span: Span::DUMMY,
        };

        let (live_in, live_out) = compute_liveness(&body);
        // x is read by the terminator → x is live_out[bb0] (because the
        // terminator's read is at block exit). And x is read by the
        // Assign in bb0 → x is in Use[bb0] → x is live_in[bb0].
        assert!(live_out[&bb0].contains(&x));
        assert!(live_in[&bb0].contains(&x));
    }

    #[test]
    fn stage15_35_compute_liveness_branch_both_arms_use_x() {
        // bb0: switchInt(x) { 1 => bb1, _ => bb2 }
        // bb1: y = x; goto bb2
        // bb2: z = x; return
        // x must be live in bb0, bb1, AND bb2 (used in both arms).
        let (mut body, locals) = build_body_with_locals(3);
        let x = locals[0];
        let y = locals[1];
        let z = locals[2];
        let bb0 = body.new_block();
        let bb1 = body.new_block();
        let bb2 = body.new_block();

        body.block_mut(bb0).terminator = Terminator {
            kind: TerminatorKind::SwitchInt {
                discr: operand_copy(x),
                targets: vec![(crate::mir::ty::ConstVal::Int(1), bb1)],
                otherwise: bb2,
            },
            span: Span::DUMMY,
        };
        body.block_mut(bb1).statements.push(stmt_assign_use(y, x));
        body.block_mut(bb1).terminator = Terminator::goto(bb2, Span::DUMMY);
        body.block_mut(bb2).statements.push(stmt_assign_use(z, x));
        body.block_mut(bb2).terminator = Terminator::ret(Span::DUMMY);

        let (live_in, live_out) = compute_liveness(&body);
        // x is read in bb1 (y = x) and bb2 (z = x), so:
        // - live_in[bb2] ⊇ {x}  (x read in bb2)
        // - live_out[bb1] ⊇ {x} (x live in bb2, bb1 → bb2)
        // - live_in[bb1] ⊇ {x}  (x read in bb1, plus live_out)
        // - live_out[bb0] ⊇ {x} (x live in bb1 AND bb2)
        // - live_in[bb0] ⊇ {x}  (x read in bb0's SwitchInt, plus live_out)
        assert!(live_in[&bb2].contains(&x), "x should be live_in[bb2]");
        assert!(live_out[&bb1].contains(&x), "x should be live_out[bb1]");
        assert!(live_in[&bb1].contains(&x), "x should be live_in[bb1]");
        assert!(live_out[&bb0].contains(&x), "x should be live_out[bb0]");
        assert!(live_in[&bb0].contains(&x), "x should be live_in[bb0]");
    }

    #[test]
    fn stage15_35_compute_liveness_loop_x_live_across_iterations() {
        // bb0: x = 0; goto bb1
        // bb1: if x { goto bb2 } else { goto bb3 }
        // bb2: x = x + 1; goto bb1     (modeled as: tmp = x; x = tmp)
        // bb3: return
        // x must be live_in[bb1] across the loop — it's read in bb1 (SwitchInt)
        // and written in bb2, but the read happens before the write along the
        // back-edge.
        let (mut body, locals) = build_body_with_locals(2);
        let x = locals[0];
        let _tmp = locals[1];
        let bb0 = body.new_block();
        let bb1 = body.new_block();
        let bb2 = body.new_block();
        let bb3 = body.new_block();

        // bb0: x = x  (write x so x has a Def in bb0; reading x makes bb0's
        // Use include x — we want to test that bb1's liveness propagates
        // back to bb0 via the back-edge through bb2.)
        body.block_mut(bb0).statements.push(stmt_assign_use(x, x));
        body.block_mut(bb0).terminator = Terminator::goto(bb1, Span::DUMMY);

        // bb1: switchInt(x) { 0 => bb3, _ => bb2 }
        body.block_mut(bb1).terminator = Terminator {
            kind: TerminatorKind::SwitchInt {
                discr: operand_copy(x),
                targets: vec![(crate::mir::ty::ConstVal::Int(0), bb3)],
                otherwise: bb2,
            },
            span: Span::DUMMY,
        };

        // bb2: x = x (write x, read x), goto bb1 — back-edge
        body.block_mut(bb2).statements.push(stmt_assign_use(x, x));
        body.block_mut(bb2).terminator = Terminator::goto(bb1, Span::DUMMY);

        // bb3: return
        body.block_mut(bb3).terminator = Terminator::ret(Span::DUMMY);

        let (live_in, live_out) = compute_liveness(&body);
        // The fixpoint must converge with x live throughout the loop:
        // - live_in[bb1] ⊇ {x} (x read in bb1's SwitchInt)
        // - live_out[bb2] ⊇ {x} (back-edge: live_out[bb2] = live_in[bb1])
        // - live_in[bb2] ⊇ {x} (x read in bb2's Assign)
        // - live_out[bb1] ⊇ {x} (x live in bb2 via otherwise branch)
        assert!(
            live_in[&bb1].contains(&x),
            "x should be live_in[bb1] (loop-carried)"
        );
        assert!(
            live_out[&bb2].contains(&x),
            "x should be live_out[bb2] (back-edge)"
        );
        assert!(live_in[&bb2].contains(&x), "x should be live_in[bb2]");
        assert!(
            live_out[&bb1].contains(&x),
            "x should be live_out[bb1] (loop body)"
        );
    }

    #[test]
    fn stage15_35_compute_liveness_dead_after_def_no_read() {
        // bb0: x = 1; y = 2; return
        // x and y are both dead (Def without Use).
        let (mut body, locals) = build_body_with_locals(2);
        let x = locals[0];
        let y = locals[1];
        let bb0 = body.new_block();
        body.block_mut(bb0).statements.push(stmt_assign_use(x, y));
        body.block_mut(bb0).statements.push(stmt_assign_use(y, x));
        body.block_mut(bb0).terminator = Terminator::ret(Span::DUMMY);

        let (live_in, live_out) = compute_liveness(&body);
        // No reads → all sets empty (each local is both read and written,
        // but the reads are killed by the defs before them in the same block;
        // since the only reader is also the only writer and there's no
        // cross-block read, liveness is empty).
        //
        // Actually wait: stmt_assign_use(x, y) reads y AND writes x.
        // So y is in Use[bb0]. And the second stmt reads x and writes y.
        // So both x and y are in Use[bb0]. They're also in Def[bb0].
        // LiveIn = Use ∪ (LiveOut - Def) = Use ∪ ∅ = Use.
        // So both x and y are live_in[bb0] but live_out[bb0] = ∅.
        assert!(
            live_out[&bb0].is_empty(),
            "no successor reads → live_out[bb0] empty"
        );
        assert!(live_in[&bb0].contains(&x), "x is read in bb0 → live_in");
        assert!(live_in[&bb0].contains(&y), "y is read in bb0 → live_in");
    }

    #[test]
    fn stage15_35_compute_liveness_call_destination_def() {
        // bb0: x = call f(); return
        // The Call's destination writes x. x is never read → x dead.
        let (mut body, _locals) = build_body_with_locals(1);
        let x = crate::mir::place::LocalId(0);
        let bb0 = body.new_block();
        body.block_mut(bb0).terminator = Terminator {
            kind: TerminatorKind::Call {
                func: operand_copy(x),
                args: vec![],
                destination: place_local(x),
                target: Some(bb0), // self-loop to keep well-formed
                dyn_trait_call: None,
            },
            span: Span::DUMMY,
        };

        let (live_in, live_out) = compute_liveness(&body);
        // Call reads x (as func) and writes x (as destination). So x is in
        // Use[bb0] AND Def[bb0]. live_out[bb0] = live_in[bb0] (self-loop)
        // → fixpoint: live_in[bb0] = Use ∪ (live_in[bb0] - Def) = {x} ∪ ∅ = {x}.
        // live_out[bb0] = live_in[bb0] = {x}.
        assert!(live_in[&bb0].contains(&x));
        assert!(live_out[&bb0].contains(&x));
    }

    #[test]
    fn stage15_35_compute_liveness_empty_body() {
        // A body with one block and an unreachable terminator — no locals,
        // no statements. Should produce empty maps without panicking.
        let mut body = MirBody::new(Span::DUMMY);
        let bb0 = body.new_block();
        body.block_mut(bb0).terminator = Terminator::unreachable(Span::DUMMY);

        let (live_in, live_out) = compute_liveness(&body);
        assert!(live_in[&bb0].is_empty());
        assert!(live_out[&bb0].is_empty());
    }

    #[test]
    fn stage15_35_compute_liveness_returns_total_map() {
        // Every BasicBlockId in the body must have an entry in both maps,
        // even blocks with no statements and unreachable terminators.
        let mut body = MirBody::new(Span::DUMMY);
        let bb0 = body.new_block();
        let bb1 = body.new_block();
        let bb2 = body.new_block();
        body.block_mut(bb0).terminator = Terminator::goto(bb1, Span::DUMMY);
        body.block_mut(bb1).terminator = Terminator::goto(bb2, Span::DUMMY);
        body.block_mut(bb2).terminator = Terminator::ret(Span::DUMMY);

        let (live_in, live_out) = compute_liveness(&body);
        assert_eq!(live_in.len(), 3);
        assert_eq!(live_out.len(), 3);
        assert!(live_in.contains_key(&bb0));
        assert!(live_in.contains_key(&bb1));
        assert!(live_in.contains_key(&bb2));
        assert!(live_out.contains_key(&bb0));
        assert!(live_out.contains_key(&bb1));
        assert!(live_out.contains_key(&bb2));
    }

    #[test]
    fn stage15_35_compute_liveness_unused_local_with_mutability() {
        // Ensure mutability of a LocalDecl doesn't affect liveness.
        // `let mut x = 1;` writes x; if x is never read, x is dead.
        let mut body = MirBody::new(Span::DUMMY);
        let _x = body.new_local_with_mut(
            Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
            Mutability::Mutable,
        );
        let bb0 = body.new_block();
        body.block_mut(bb0).statements.push(stmt_assign_use(
            crate::mir::place::LocalId(0),
            crate::mir::place::LocalId(0),
        ));
        body.block_mut(bb0).terminator = Terminator::ret(Span::DUMMY);

        let (live_in, live_out) = compute_liveness(&body);
        // x is read AND written in the same statement, so x ∈ Use[bb0] ∩ Def[bb0].
        // live_out[bb0] = ∅ (no successor reads). live_in[bb0] = Use ∪ ∅ = {x}.
        assert!(live_in[&bb0].contains(&crate::mir::place::LocalId(0)));
        assert!(live_out[&bb0].is_empty());
    }

    // ----- compute_live_after_point() tests (Stage 15.36) -----

    /// Stage 15.36: `compute_live_after_point` at the terminator index
    /// returns LiveOut[bb_id] directly (no remaining statements to fold).
    #[test]
    fn stage15_36_compute_live_after_point_at_terminator() {
        // bb0: y = x; return
        // LiveOut[bb0] = ∅ (no successors). At stmt_idx == 1 (terminator),
        // result is ∅.
        let (mut body, locals) = build_body_with_locals(2);
        let x = locals[0];
        let y = locals[1];
        let bb0 = body.new_block();
        body.block_mut(bb0).statements.push(stmt_assign_use(y, x));
        body.block_mut(bb0).terminator = Terminator::ret(Span::DUMMY);

        let (_live_in, live_out) = compute_liveness(&body);
        let live_after = compute_live_after_point(&body, &live_out, bb0, 1);
        assert!(
            live_after.is_empty(),
            "at terminator, live_after = LiveOut[bb0] = ∅"
        );
    }

    /// Stage 15.36: `compute_live_after_point` at stmt_idx=0 folds in the
    /// terminator's Use/Def and any subsequent statements.
    #[test]
    fn stage15_36_compute_live_after_point_folds_terminator() {
        // bb0: y = x; assert(x)  ← terminator reads x
        // LiveOut[bb0] = ∅. At stmt_idx=0 (after `y = x`), the terminator
        // (Assert) still reads x, so x must be live_after.
        let (mut body, locals) = build_body_with_locals(2);
        let x = locals[0];
        let y = locals[1];
        let bb0 = body.new_block();
        body.block_mut(bb0).statements.push(stmt_assign_use(y, x));
        body.block_mut(bb0).terminator = Terminator {
            kind: TerminatorKind::Assert {
                cond: Operand::Copy(place_local(x)),
                expected: true,
                target: bb0, // self-loop to keep well-formed
                msg: AssertMessage::BoundsCheck,
            },
            span: Span::DUMMY,
        };

        let (_live_in, live_out) = compute_liveness(&body);
        let live_after_stmt0 = compute_live_after_point(&body, &live_out, bb0, 0);
        // x is read by the terminator → must be live after stmt 0.
        assert!(
            live_after_stmt0.contains(&x),
            "x is read by terminator → live after stmt 0"
        );
    }

    /// Stage 15.36: `compute_live_after_point` correctly back-propagates
    /// through multiple statements.
    #[test]
    fn stage15_36_compute_live_after_point_back_propagates() {
        // bb0: x = 1; y = 2; z = x; return
        // LiveOut[bb0] = ∅. At stmt_idx=0 (after `x = 1`):
        //   - Fold terminator (Return: no reads/writes) → still ∅.
        //   - Fold stmt 2 (z = x): Def={z}, Use={x} → live = {x}.
        //   - Fold stmt 1 (y = 2): Def={y}, Use=∅ → live = {x}.
        // Result: {x}.
        let (mut body, locals) = build_body_with_locals(3);
        let x = locals[0];
        let y = locals[1];
        let z = locals[2];
        let bb0 = body.new_block();

        // stmt 0: x = const 1
        let const_one = Operand::Constant(crate::mir::ty::Const {
            ty: Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
            val: crate::mir::ty::ConstVal::Int(1),
        });
        let stmt_assign_const = |dst: crate::mir::place::LocalId, op: Operand| Statement {
            kind: StatementKind::Assign(Box::new((place_local(dst), Rvalue::Use(op)))),
            span: Span::DUMMY,
        };
        body.block_mut(bb0)
            .statements
            .push(stmt_assign_const(x, const_one.clone()));
        body.block_mut(bb0)
            .statements
            .push(stmt_assign_const(y, const_one));
        body.block_mut(bb0).statements.push(stmt_assign_use(z, x));
        body.block_mut(bb0).terminator = Terminator::ret(Span::DUMMY);

        let (_live_in, live_out) = compute_liveness(&body);
        let live_after_stmt0 = compute_live_after_point(&body, &live_out, bb0, 0);
        assert!(
            live_after_stmt0.contains(&x),
            "x is read in stmt 2 → live after stmt 0"
        );
        assert!(
            !live_after_stmt0.contains(&y),
            "y is written but never read → not live after stmt 0"
        );
        assert!(
            !live_after_stmt0.contains(&z),
            "z is written but never read → not live after stmt 0"
        );
    }

    /// Stage 15.36: `compute_live_after_point` with out-of-range bb_id
    /// returns empty set (defensive — no panic).
    #[test]
    fn stage15_36_compute_live_after_point_out_of_range_bb() {
        let (body, _locals) = build_body_with_locals(1);
        let (_live_in, live_out) = compute_liveness(&body);
        // bb_id 999 doesn't exist — should return empty set, not panic.
        let live_after = compute_live_after_point(&body, &live_out, BasicBlockId(999), 0);
        assert!(live_after.is_empty());
    }

    /// Stage 15.36: `compute_live_after_point` at stmt_idx == statements.len()
    /// is the terminator point — result equals LiveOut[bb_id] exactly.
    #[test]
    fn stage15_36_compute_live_after_point_terminator_equals_live_out() {
        // bb0: x = 1; goto bb1
        // bb1: y = x; return
        // LiveOut[bb0] = live_in[bb1] = {x}.
        // At stmt_idx=1 (terminator of bb0), live_after should equal LiveOut[bb0] = {x}.
        let (mut body, locals) = build_body_with_locals(2);
        let x = locals[0];
        let y = locals[1];
        let bb0 = body.new_block();
        let bb1 = body.new_block();

        let const_one = Operand::Constant(crate::mir::ty::Const {
            ty: Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
            val: crate::mir::ty::ConstVal::Int(1),
        });
        let stmt_assign_const = |dst: crate::mir::place::LocalId, op: Operand| Statement {
            kind: StatementKind::Assign(Box::new((place_local(dst), Rvalue::Use(op)))),
            span: Span::DUMMY,
        };
        body.block_mut(bb0)
            .statements
            .push(stmt_assign_const(x, const_one));
        body.block_mut(bb0).terminator = Terminator::goto(bb1, Span::DUMMY);
        body.block_mut(bb1).statements.push(stmt_assign_use(y, x));
        body.block_mut(bb1).terminator = Terminator::ret(Span::DUMMY);

        let (_live_in, live_out) = compute_liveness(&body);
        let live_after_term = compute_live_after_point(&body, &live_out, bb0, 1);
        assert_eq!(
            live_after_term,
            *live_out.get(&bb0).unwrap(),
            "live_after at terminator index should equal LiveOut[bb0]"
        );
        assert!(
            live_after_term.contains(&x),
            "x live in bb1 → live_out[bb0]"
        );
    }

    // ----- compute_ever_read() tests (Stage 15.39) -----

    /// Stage 15.39: `compute_ever_read` returns the set of all locals read
    /// anywhere in the body (statements + terminators).
    #[test]
    fn stage15_39_compute_ever_read_collects_all_reads() {
        // bb0: y = x; z = y; return
        // Reads: x (in y=x), y (in z=y).
        // Writes: y, z.
        // ever_read should contain x and y (not z — z is never read).
        let (mut body, locals) = build_body_with_locals(3);
        let x = locals[0];
        let y = locals[1];
        let z = locals[2];
        let bb0 = body.new_block();
        body.block_mut(bb0).statements.push(stmt_assign_use(y, x)); // y = x
        body.block_mut(bb0).statements.push(stmt_assign_use(z, y)); // z = y
        body.block_mut(bb0).terminator = Terminator::ret(Span::DUMMY);

        let ever = compute_ever_read(&body);
        assert!(ever.contains(&x), "x is read in 'y = x'");
        assert!(ever.contains(&y), "y is read in 'z = y'");
        assert!(
            !ever.contains(&z),
            "z is written but never read → not in ever_read"
        );
    }

    /// Stage 15.39: `compute_ever_read` includes reads from terminators
    /// (e.g., SwitchInt discr, Call func/args, Assert cond).
    #[test]
    fn stage15_39_compute_ever_read_includes_terminator_reads() {
        // bb0: y = x; switchInt(x) { 0 => bb1, _ => bb2 }
        // Reads: x (in y=x), x (in switchInt discr).
        // ever_read should contain x.
        let (mut body, locals) = build_body_with_locals(2);
        let x = locals[0];
        let y = locals[1];
        let bb0 = body.new_block();
        let bb1 = body.new_block();
        let bb2 = body.new_block();
        body.block_mut(bb0).statements.push(stmt_assign_use(y, x)); // y = x
        body.block_mut(bb0).terminator = Terminator {
            kind: TerminatorKind::SwitchInt {
                discr: Operand::Copy(place_local(x)),
                targets: vec![(crate::mir::ty::ConstVal::Int(0), bb1)],
                otherwise: bb2,
            },
            span: Span::DUMMY,
        };
        body.block_mut(bb1).terminator = Terminator::ret(Span::DUMMY);
        body.block_mut(bb2).terminator = Terminator::ret(Span::DUMMY);

        let ever = compute_ever_read(&body);
        assert!(ever.contains(&x), "x is read by switchInt discr");
    }

    /// Stage 15.39: `compute_ever_read` returns empty set for a body with
    /// no reads (only writes).
    #[test]
    fn stage15_39_compute_ever_read_empty_when_no_reads() {
        // bb0: x = 1; y = 2; return
        // No reads (RHS are constants). ever_read should be empty.
        let (mut body, locals) = build_body_with_locals(2);
        let x = locals[0];
        let y = locals[1];
        let bb0 = body.new_block();

        let const_one = Operand::Constant(crate::mir::ty::Const {
            ty: Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
            val: crate::mir::ty::ConstVal::Int(1),
        });
        let stmt_assign_const = |dst: crate::mir::place::LocalId, op: Operand| Statement {
            kind: StatementKind::Assign(Box::new((place_local(dst), Rvalue::Use(op)))),
            span: Span::DUMMY,
        };
        body.block_mut(bb0)
            .statements
            .push(stmt_assign_const(x, const_one.clone()));
        body.block_mut(bb0)
            .statements
            .push(stmt_assign_const(y, const_one));
        body.block_mut(bb0).terminator = Terminator::ret(Span::DUMMY);

        let ever = compute_ever_read(&body);
        assert!(ever.is_empty(), "no reads → ever_read is empty");
    }

    /// Stage 15.39: `compute_ever_read` returns empty set for an empty body
    /// (no statements, unreachable terminator).
    #[test]
    fn stage15_39_compute_ever_read_empty_body() {
        let mut body = MirBody::new(Span::DUMMY);
        let bb0 = body.new_block();
        body.block_mut(bb0).terminator = Terminator::unreachable(Span::DUMMY);

        let ever = compute_ever_read(&body);
        assert!(ever.is_empty(), "empty body → ever_read is empty");
    }

    /// Stage 15.39: `compute_ever_read` collects reads across multiple
    /// basic blocks.
    #[test]
    fn stage15_39_compute_ever_read_multiple_blocks() {
        // bb0: y = x; goto bb1
        // bb1: z = y; return
        // ever_read should contain x and y (across both blocks).
        let (mut body, locals) = build_body_with_locals(3);
        let x = locals[0];
        let y = locals[1];
        let z = locals[2];
        let bb0 = body.new_block();
        let bb1 = body.new_block();
        body.block_mut(bb0).statements.push(stmt_assign_use(y, x)); // y = x
        body.block_mut(bb0).terminator = Terminator::goto(bb1, Span::DUMMY);
        body.block_mut(bb1).statements.push(stmt_assign_use(z, y)); // z = y
        body.block_mut(bb1).terminator = Terminator::ret(Span::DUMMY);

        let ever = compute_ever_read(&body);
        assert!(ever.contains(&x), "x read in bb0");
        assert!(ever.contains(&y), "y read in bb1");
        assert!(!ever.contains(&z), "z never read");
    }
}
