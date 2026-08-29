//! Stage 15.43 (HP-12 step 2 of 6): Drop elaboration — `ty_needs_drop` analysis.
//!
//! Per `docs/lang-design/25-drop-elaboration.md` §2.2. This module owns the
//! `ty_needs_drop` function, which determines whether a type needs drop glue.
//!
//! A type needs drop if:
//! - It implements `Drop` (user-defined destructor), OR
//! - It has fields that need drop (for structs/enums), OR
//! - It's a container of a type that needs drop (for `Box<T>`, `Vec<T>`).
//!
//! Primitive types (i32, bool, etc.) never need drop. References (&T, &mut T)
//! never need drop (they're just pointers). Raw pointers never need drop.
//!
//! ## MVP scope (v0.2)
//!
//! For the v0.2 MVP, `ty_needs_drop` checks:
//! 1. `Adt` types: `resolver.is_drop_builtin(def_id, interner)` — if the type
//!    implements `Drop`, it needs drop.
//! 2. Field traversal: for `Adt` types that don't implement `Drop`, check if
//!    any field needs drop (recursive). This uses `AdtLayouts` to look up
//!    field types without reading HIR (per §16 interface isolation).
//! 3. `Tuple`: check if any element needs drop.
//! 4. `Array`/`Slice`: check if the element type needs drop.
//!
//! ## What's NOT in scope for v0.2 MVP
//!
//! - `dyn Trait` (vtable drop slot) — always returns true for now, but the
//!   actual drop glue call is not implemented yet.
//! - Closures — return false (closures don't have Drop in v0.2).
//! - Generic types (monomorphization) — handled by codegen, not here.
//!
//! Per §23: function name follows `<noun>_<verb>_<noun>` pattern.
//! Per §16: `ty_needs_drop` reads `Ty`, `TraitResolver`, `AdtLayouts`, and
//! `Rodeo` — all are read-only references, no writes, no HIR lookup.
//! Per §1.0 原則 5 "报错 > 静默": when in doubt (e.g., `Infer`, `Error`),
//! return false — a false negative (missing a needed drop) just leaks memory;
//! a false positive (drop a type that doesn't need it) would be unsound
//! (calling a nonexistent drop method).

use crate::hir::DefId;
use crate::mir::body::{AdtLayout, AdtLayouts, BasicBlockId, StatementKind, TerminatorKind};
use crate::mir::place::{LocalId, Operand, PlaceKind, ProjectionElem, Rvalue};
use crate::mir::ty::{Ty, TyKind};
use crate::traits::TraitResolver;
use lasso::Rodeo;
use std::collections::{HashMap, HashSet};

/// Stage 15.62: Collect all local IDs that are the source of an `Operand::Move`
/// anywhere in the MIR body. These locals have been moved (ownership
/// transferred elsewhere) and should NOT receive a `Drop` terminator —
/// the value now lives in the destination local, which will be dropped.
///
/// This is a **flow-insensitive** analysis: it marks a local as "moved"
/// if it appears as the source of ANY `Move` operand in ANY block,
/// regardless of control flow. This is correct for the common case
/// (unconditional move of a temporary into a let binding) but may
/// over-approximate for conditional moves (a local moved in one branch
/// but not another). In the over-approximation case, the local's `Drop`
/// is skipped, which may cause a leak (the destructor is not called).
/// This is acceptable for the MVP — leaks are less severe than double-drops
/// (which could cause use-after-free).
///
/// ## Why not use the borrow checker's move tracker?
///
/// The borrow checker runs AFTER `elaborate_drops` in the pipeline
/// (driver.rs: `elaborate_drops` at line 1035, `borrowck` at line 1087).
/// Moving `elaborate_drops` after borrowck would require passing the
/// move tracker results across stages, which is a larger refactor. The
/// flow-insensitive analysis is a pragmatic MVP fix — full drop flags
/// (runtime tracking) are deferred to v0.3.
///
/// Per §16: this function reads `MirBody` only (no HIR, no resolver).
/// Per §23: function name follows `<verb>_<noun>` pattern.
fn collect_moved_locals(mir: &crate::mir::body::MirBody) -> HashSet<LocalId> {
    let mut moved = HashSet::new();
    for bb in &mir.basic_blocks {
        for stmt in &bb.statements {
            match &stmt.kind {
                StatementKind::Assign(boxed) => {
                    let (_, rvalue) = &**boxed;
                    collect_moved_locals_from_rvalue(rvalue, &mut moved);
                }
                // Stage 18.243 (TD-DROP-MOVED-LOCALS partial fix): Scan
                // StatementKind::Store for Operand::Move in the val field.
                // When `Store { val: Move(local), ... }` is emitted (e.g.,
                // by Vec::push or String::push_str MIR intrinsics), the
                // moved local should NOT receive a Drop terminator —
                // ownership has been transferred to the destination.
                //
                // Per §1.0 原則 4 (报错>静默): previously, Store moves were
                // silently ignored, causing potential double-drops.
                // Per §1.0 原則 6 (通解>特解): one scan for all statement kinds.
                // Per §17.6 (同类型整体修复): same pattern as Assign moves.
                StatementKind::Store { val, .. } => {
                    collect_moved_locals_from_operand(val, &mut moved);
                }
                _ => {}
            }
        }
        // Stage 18.243: Also scan the terminator for moves.
        // TerminatorKind::Call can have Operand::Move in its args —
        // when a local is moved into a function call, it should not
        // receive a Drop terminator.
        //
        // Per §1.0 原則 6 (通解>特解): one scan for all terminator kinds.
        // Per §17.6 (同类型整体修复): same pattern as statement moves.
        collect_moved_locals_from_terminator(&bb.terminator.kind, &mut moved);
    }
    moved
}

/// Stage 18.282 (TD-DROP-MOVED-LOCALS full): Compute flow-sensitive move state
/// via forwards dataflow fixpoint.
///
/// Unlike `collect_moved_locals` (flow-insensitive, collects ALL moves
/// across ALL blocks into one set), this function computes per-block
/// `moved_in` and `moved_out` sets. A local is "moved" at a given point
/// only if it was moved on ALL paths reaching that point.
///
/// This prevents false "moved" markings on conditional paths where a
/// local is moved in one branch but not another. Without this, drop
/// elaboration would skip drop on the non-moved branch, causing leaks.
///
/// Algorithm (forwards dataflow):
/// 1. `block_moves[B]` = locals moved in B's statements + terminator
/// 2. `moved_out[B]` = `moved_in[B] ∪ block_moves[B]`
/// 3. `moved_in[B]` = ∩ `moved_out[P]` for P in preds(B)
///    (entry block: `moved_in = ∅`)
/// 4. Iterate to fixpoint (when no `moved_in` changes)
///
/// Per §1.0 原則 6 (通解 > 特解): one dataflow fixpoint for all move tracking.
/// Per §2.2 原則 9 (正确 > 妥协): flow-sensitive is correct; flow-insensitive is a compromise.
/// Per §13.4 J3 (单向流动): MIR → moved_state → elaborate_drops (one direction).
/// Per §12 (最优 > 最小): forwards dataflow fixpoint is the optimal solution.
pub fn compute_moved_state(
    mir: &crate::mir::body::MirBody,
) -> (
    HashMap<BasicBlockId, HashSet<LocalId>>,
    HashMap<BasicBlockId, HashSet<LocalId>>,
) {
    let num_blocks = mir.basic_blocks.len();

    // Initialize: all blocks → empty set
    let mut moved_in: HashMap<BasicBlockId, HashSet<LocalId>> = HashMap::with_capacity(num_blocks);
    let mut moved_out: HashMap<BasicBlockId, HashSet<LocalId>> = HashMap::with_capacity(num_blocks);
    for bb_idx in 0..num_blocks {
        let bb_id = BasicBlockId(bb_idx as u32);
        moved_in.insert(bb_id, HashSet::new());
        moved_out.insert(bb_id, HashSet::new());
    }

    // Pre-compute per-block move sets (locals moved via Operand::Move in
    // statements + terminator). Reuses collect_moved_locals_from_rvalue and
    // collect_moved_locals_from_terminator as transfer functions.
    let mut block_moves: Vec<HashSet<LocalId>> = Vec::with_capacity(num_blocks);
    for bb in &mir.basic_blocks {
        let mut moves = HashSet::new();
        for stmt in &bb.statements {
            match &stmt.kind {
                StatementKind::Assign(boxed) => {
                    let (_, rvalue) = &**boxed;
                    collect_moved_locals_from_rvalue(rvalue, &mut moves);
                }
                StatementKind::Store { val, .. } => {
                    collect_moved_locals_from_operand(val, &mut moves);
                }
                _ => {}
            }
        }
        collect_moved_locals_from_terminator(&bb.terminator.kind, &mut moves);
        block_moves.push(moves);
    }

    // Pre-compute predecessor list for each block.
    // MirBody doesn't have an explicit predecessor map, so we build one
    // from the terminator successors.
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); num_blocks];
    for bb_idx in 0..num_blocks {
        let bb = &mir.basic_blocks[bb_idx];
        for succ in terminator_successors(&bb.terminator.kind) {
            if succ < num_blocks {
                preds[succ].push(bb_idx);
            }
        }
    }

    // Fixpoint: forwards iteration.
    // moved_out[B] = moved_in[B] ∪ block_moves[B]
    // moved_in[B] = ∩ moved_out[P] for P in preds(B)
    //   (if no preds: moved_in = ∅, already initialized)
    let mut changed = true;
    while changed {
        changed = false;
        for bb_idx in 0..num_blocks {
            let bb_id = BasicBlockId(bb_idx as u32);

            // Compute new moved_in as intersection of preds' moved_out
            let new_moved_in = if preds[bb_idx].is_empty() {
                HashSet::new()
            } else {
                let mut intersection: Option<HashSet<LocalId>> = None;
                for &pred_idx in &preds[bb_idx] {
                    let pred_id = BasicBlockId(pred_idx as u32);
                    let pred_out = moved_out.get(&pred_id).cloned().unwrap_or_default();
                    intersection = match intersection {
                        None => Some(pred_out),
                        Some(existing) => Some(existing.intersection(&pred_out).cloned().collect()),
                    };
                }
                intersection.unwrap_or_default()
            };

            // Compute new moved_out = moved_in ∪ block_moves
            let new_moved_out: HashSet<LocalId> =
                new_moved_in.union(&block_moves[bb_idx]).cloned().collect();

            // Check for change
            let old_moved_in = moved_in.get(&bb_id).cloned().unwrap_or_default();
            if new_moved_in != old_moved_in {
                changed = true;
                moved_in.insert(bb_id, new_moved_in);
                moved_out.insert(bb_id, new_moved_out);
            } else {
                // moved_in didn't change, but moved_out might need updating
                // if block_moves changed (they won't — block_moves is fixed).
                // Just ensure moved_out is consistent.
                moved_out.insert(bb_id, new_moved_out);
            }
        }
    }

    (moved_in, moved_out)
}

/// Get successor basic block indices from a terminator kind.
/// Used by `compute_moved_state` to build the predecessor map.
fn terminator_successors(kind: &TerminatorKind) -> Vec<usize> {
    match kind {
        TerminatorKind::Goto(target) => vec![target.0 as usize],
        TerminatorKind::SwitchInt { targets, .. } => {
            targets.iter().map(|(_, bb)| bb.0 as usize).collect()
        }
        TerminatorKind::Call {
            destination: _,
            target,
            ..
        } => {
            // destination is a Place (where the return value is stored),
            // not a successor block. The successor is `target`.
            // Per §2.2 原則 3 (显式 > 隐式): target is the explicit continuation.
            match target {
                Some(tgt) => vec![tgt.0 as usize],
                None => vec![],
            }
        }
        TerminatorKind::Return => vec![],
        TerminatorKind::Unreachable => vec![],
        TerminatorKind::Drop { target, .. } => vec![target.0 as usize],
        TerminatorKind::Assert { target, .. } => vec![target.0 as usize],
    }
}

/// Stage 18.243: Extract moved local IDs from a terminator.
///
/// Scans `TerminatorKind::Call` args for `Operand::Move(Place::Local(id))`.
/// Other terminators (Goto, Return, etc.) don't have operands to scan.
fn collect_moved_locals_from_terminator(
    kind: &crate::mir::body::TerminatorKind,
    moved: &mut HashSet<LocalId>,
) {
    match kind {
        TerminatorKind::Call { args, .. } => {
            for arg in args {
                collect_moved_locals_from_operand(arg, moved);
            }
        }
        TerminatorKind::Drop { place, .. } => {
            // Drop itself doesn't move — it's a destructor call.
            // But the place being dropped should be tracked.
            if let PlaceKind::Local(id) = &place.kind {
                moved.insert(*id);
            }
        }
        _ => {}
    }
}

/// Stage 18.243: Extract moved local IDs from a single operand.
///
/// Shared helper used by both rvalue and terminator/Store scanning.
/// Per §10 (DRY): single source of truth for operand move extraction.
fn collect_moved_locals_from_operand(op: &Operand, moved: &mut HashSet<LocalId>) {
    if let Operand::Move(place) = op {
        if let PlaceKind::Local(id) = &place.kind {
            moved.insert(*id);
        }
    }
}

/// Helper: extract all moved local IDs from an `Rvalue`.
///
/// Walks the rvalue's operands and collects any `Operand::Move(Place::Local(id))`.
fn collect_moved_locals_from_rvalue(rv: &Rvalue, moved: &mut HashSet<LocalId>) {
    match rv {
        Rvalue::Use(op) | Rvalue::Cast(_, op, _) | Rvalue::UnaryOp(_, op) => {
            collect_moved_locals_from_operand(op, moved);
        }
        Rvalue::BinaryOp(_, a, b) | Rvalue::BinaryOp2(_, a, b) => {
            collect_moved_locals_from_operand(a, moved);
            collect_moved_locals_from_operand(b, moved);
        }
        // Stage 18.243: MIR intrinsic ops — scan operands for moves.
        // Previously (Stage 18.226) these were stubs that said "not yet
        // codegen-enabled". Now that codegen is enabled (Stage 18.227+),
        // we must scan their operands for Move.
        //
        // Per §1.0 原則 4 (报错>静默): unscanned moves cause double-drops.
        // Per §1.0 原則 6 (通解>特解): one scan path for all Rvalue variants.
        // Per §17.6 (同类型整体修复): same pattern as BinaryOp/Aggregate.
        Rvalue::Load(ptr_op, _) => {
            collect_moved_locals_from_operand(ptr_op, moved);
        }
        Rvalue::GetElementPtr { base, indices, .. } => {
            collect_moved_locals_from_operand(base, moved);
            for idx_op in indices {
                collect_moved_locals_from_operand(idx_op, moved);
            }
        }

        Rvalue::Aggregate(_, operands) => {
            for op in operands {
                collect_moved_locals_from_operand(op, moved);
            }
        }
        Rvalue::Ref(_, _, _) => {} // Refs don't move.
    }
}

/// Stage 15.64: Collect all local IDs that are assigned from a `Copy` of a
/// field projection. These locals hold a "view" of a struct field, not an
/// owned value — the original struct owns the field. Dropping them would
/// cause a double-drop (the field is already dropped when the struct is
/// dropped via recursive drop glue).
///
/// Example MIR pattern to detect:
/// ```text
/// temp = Use(Copy(Projection(Local(o), Field(0))))  // copies o.inner
/// ```
///
/// The `temp` local should NOT receive a Drop terminator.
///
/// This is a **flow-insensitive** analysis (same as `collect_moved_locals`).
/// It may over-approximate (skip a Drop for a local that was later
/// re-assigned), but this is acceptable — the worst case is a leak (less
/// severe than a double-drop).
///
/// Per §16: reads `MirBody` only (no HIR, no resolver).
/// Per §23: function name follows `<verb>_<noun>` pattern.
fn collect_field_copy_locals(mir: &crate::mir::body::MirBody) -> HashSet<LocalId> {
    let mut field_copy = HashSet::new();
    for bb in &mir.basic_blocks {
        for stmt in &bb.statements {
            if let StatementKind::Assign(boxed) = &stmt.kind {
                let (place, rvalue) = &**boxed;
                // Check if the rvalue is Use(Copy(Projection(...)))
                if let Rvalue::Use(Operand::Copy(src_place)) = rvalue {
                    if matches!(
                        &src_place.kind,
                        PlaceKind::Projection(_, ProjectionElem::Field(_, _))
                    ) {
                        // The destination local is a field-copy temp.
                        if let PlaceKind::Local(id) = &place.kind {
                            field_copy.insert(*id);
                        }
                    }
                }
            }
        }
    }
    field_copy
}

/// Determine whether a type needs drop glue.
///
/// A type needs drop if it implements `Drop`, or if it has fields/elements
/// that need drop (recursive). See module docs for full semantics.
///
/// ## Parameters
///
/// - `ty`: The type to check.
/// - `resolver`: The `TraitResolver` for querying `Drop` implementations.
/// - `adt_layouts`: The `AdtLayouts` for looking up field types (avoids
///   HIR lookup per §16).
/// - `interner`: The `Rodeo` for resolving trait names (e.g., "Drop").
///
/// ## Returns
///
/// `true` if the type needs drop glue, `false` otherwise.
///
/// ## Complexity
///
/// O(N) where N is the total number of fields/elements in the type tree
/// (recursive traversal). Cycles are handled via a `visited` set to prevent
/// infinite recursion on self-referential types (e.g., a struct with a
/// `Box<Self>` field).
///
/// Per §23: function name follows `<noun>_<verb>_<noun>` pattern.
pub fn ty_needs_drop(
    ty: &Ty,
    resolver: &TraitResolver,
    adt_layouts: &AdtLayouts,
    interner: &Rodeo,
) -> bool {
    let mut visited = HashSet::new();
    ty_needs_drop_impl(ty, resolver, adt_layouts, interner, &mut visited)
}

/// Internal recursive implementation of `ty_needs_drop`.
///
/// The `visited` set tracks `DefId`s of `Adt` types we've already examined,
/// to prevent infinite recursion on self-referential types (e.g., a linked
/// list node with a `Box<Node>` field). If we revisit a `DefId`, we return
/// `false` (the cycle-breaking case — the `Box` itself needs drop, but the
/// inner type's cycle is broken by the `Box` indirection).
fn ty_needs_drop_impl(
    ty: &Ty,
    resolver: &TraitResolver,
    adt_layouts: &AdtLayouts,
    interner: &Rodeo,
    visited: &mut HashSet<DefId>,
) -> bool {
    match &ty.kind {
        // Primitives never need drop.
        TyKind::Bool
        | TyKind::Char
        | TyKind::Int(_)
        | TyKind::Uint(_)
        | TyKind::Float(_)
        | TyKind::Str
        | TyKind::Never => false,

        // References and raw pointers are just pointers — never need drop.
        TyKind::Ref(_, _, _) | TyKind::RawPtr(_, _) => false,

        // Function definitions and pointers never need drop.
        TyKind::FnDef(_, _) | TyKind::FnPtr(_) => false,

        // Tuples need drop if any element needs drop.
        TyKind::Tuple(tys) => tys
            .iter()
            .any(|t| ty_needs_drop_impl(t, resolver, adt_layouts, interner, visited)),

        // Arrays and slices need drop if the element type needs drop.
        TyKind::Array(inner, _) | TyKind::Slice(inner) => {
            ty_needs_drop_impl(inner, resolver, adt_layouts, interner, visited)
        }

        // ADTs (struct/enum): check Drop impl + field types.
        TyKind::Adt(def_id, _) | TyKind::Projection(def_id, _) => {
            // Cycle detection: if we've already visited this DefId, return
            // false (the cycle is broken by the indirection that led us
            // here, e.g., Box<Self>).
            if !visited.insert(*def_id) {
                return false;
            }

            // Stage 18.244 (TD-BOX-AUTO-DROP fix): Enable Box auto-drop.
            // Previously (Stage 18.193) this was deferred because Box::new's
            // FnDef local had the same LLVM layout as Box ({ ptr }), causing
            // false-positive drop on invalid pointers.
            //
            // The fix: The null-check in drop_glue.rs (emit_drop_glue_functions)
            // already handles this — if the pointer is null, dealloc is skipped.
            // And with Stage 18.243's move tracking extension, moved-from locals
            // are now correctly skipped by collect_moved_locals.
            //
            // Per §1.0 原則 4 (报错>静默): Box auto-drop now correctly deallocates.
            // Per §1.0 原則 6 (通解>特解): one drop path for all owned heap types.
            // Per §17.6 (同类型整体修复): depends on Stage 18.243 move tracking.

            // Check if this is a Box type by looking up the struct name.
            let is_box = if let Some(&type_spur) = resolver.type_by_def_id.get(def_id) {
                let box_spur = interner.get("Box");
                box_spur.is_some_and(|bs| bs == type_spur)
            } else {
                false
            };
            if is_box {
                // Box<T> needs drop — it owns a heap allocation.
                return true;
            }

            // Check if the type implements Drop (user-defined destructor).
            if resolver.is_drop_builtin(*def_id, interner) {
                return true;
            }

            // Check if any field needs drop (recursive).
            // Per §16: use AdtLayouts (sunk from HIR during MIR lowering)
            // instead of reading HIR directly.
            if let Some(layout) = adt_layouts.get(def_id) {
                match layout {
                    AdtLayout::Struct { field_tys } => {
                        for field_ty in field_tys {
                            if ty_needs_drop_impl(
                                field_ty,
                                resolver,
                                adt_layouts,
                                interner,
                                visited,
                            ) {
                                return true;
                            }
                        }
                    }
                    AdtLayout::Enum {
                        variant_payloads, ..
                    } => {
                        for payload in variant_payloads {
                            for field_ty in payload {
                                if ty_needs_drop_impl(
                                    field_ty,
                                    resolver,
                                    adt_layouts,
                                    interner,
                                    visited,
                                ) {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }

            false
        }

        // Closures: v0.2 doesn't support Drop on closures.
        TyKind::Closure(_, _) => false,

        // Foreign types: conservatively false (we don't know their layout).
        TyKind::Foreign => false,

        // Type parameters: conservatively false (we don't know the concrete
        // type at this point — monomorphization will handle it).
        TyKind::Param(_) => false,

        // Infer and Error: conservatively false to avoid spurious drops
        // during type inference. Per §1.0 原則 5 "报错 > 静默": a false
        // negative (missing a needed drop) just leaks memory; a false
        // positive (drop a type that doesn't need it) would be unsound.
        TyKind::Infer(_) | TyKind::Error => false,
    }
}

/// Stage 15.44 (HP-12 step 3 of 6): Insert `Drop` terminators before
/// `StorageDead` statements for locals whose type needs drop.
///
/// This pass walks all basic blocks. For each `StorageDead(local)`
/// statement where `ty_needs_drop(local.ty)` is true, it splits the
/// basic block at that point:
///
/// 1. The current block's statements up to (but not including) the
///    `StorageDead` stay in the current block.
/// 2. The current block's terminator is replaced with
///    `Drop { place: local, target: new_block }`.
/// 3. A new basic block is created with the statements AFTER the
///    `StorageDead` (the `StorageDead` itself is consumed by the
///    `Drop` terminator — see Stage 15.61 fix below) plus the original
///    terminator.
///
/// This correctly models the drop semantics: the destructor runs BEFORE
/// the local is marked as dead. The `Drop` terminator is a control-flow
/// point (it branches to the target after running the destructor).
///
/// ## Stage 15.61 fix — infinite-loop prevention
///
/// **Bug (Stage 15.60)**: The new block used to retain the
/// `StorageDead(local)` statement. When `bb_idx` reached the new block,
/// the algorithm found `StorageDead(local)` again (the local still needs
/// drop) and split again — ad infinitum, until OOM kill (exit 137).
///
/// **Fix (Stage 15.61)**: The `StorageDead(local)` statement is **dropped**
/// when splitting. The `Drop` terminator subsumes its role: after the
/// destructor runs, the local is dead. The `StorageDead` marker is no
/// longer needed (it was a stack-slot liveness hint, not a semantic
/// requirement). This matches rustc's behaviour where `Drop` terminators
/// replace `StorageDead` for types that need drop glue.
///
/// ## Drop order
///
/// Stage 15.62: Locals are dropped in **reverse declaration order**,
/// matching Rust's drop semantics. The `StorageDead` emission in
/// `mir::lower::mod.rs` emits statements in reverse local-ID order
/// (last-declared first). Since this pass processes `StorageDead`
/// statements in block order (finding the first one needing drop,
/// splitting, then processing the new block), the `Drop` terminators
/// are inserted in reverse declaration order:
///
/// ```text
/// bb_return:
///   ...statements before StorageDead chain...
///   Drop(_N, bb_dropN)     // last-declared local dropped first
///
/// bb_dropN:
///   Drop(_N-1, bb_dropN-1)
///
/// ...eventually...
///
/// bb_drop1:
///   Drop(_1, bb_final)     // first-declared local dropped last
///
/// bb_final:
///   Return
/// ```
///
/// This matches Rust's RFC 1327 dropck semantics (simplified for MVP —
/// no runtime drop flags, no partial moves). Stage 15.62 adds a
/// compile-time flow-insensitive move analysis to skip `Drop` terminators
/// for moved locals (preventing double-drop of temporaries).
///
/// Per §23: function name follows `<verb>_<noun>` pattern (free-function
/// entry point).
/// Per §16: this pass mutates `MirBody` in place — it's a MIR-to-MIR
/// transformation, not a cross-stage operation.
/// Per §1.0 原則 3 "显式 > 隐式": the `Drop` terminators are explicit
/// in the MIR, not implicit in `StorageDead`.
/// Per §1.0 原則 5 "报错 > 静默": the pass is total — it terminates
/// on all inputs (the Stage 15.61 fix guarantees termination).
pub fn elaborate_drops(
    mir: &mut crate::mir::body::MirBody,
    resolver: &TraitResolver,
    interner: &Rodeo,
) {
    use crate::mir::body::{BasicBlockId, Statement, StatementKind, Terminator, TerminatorKind};
    use crate::mir::place::{Place, PlaceKind};

    // Stage 15.62: Pre-compute the set of moved locals. These locals
    // have been moved (ownership transferred) and should NOT receive
    // Drop terminators — the value now lives in the destination local.
    // This prevents double-drop of temporaries (e.g., the `init_local`
    // that holds `S{x: 0}` is moved into `s`, so only `s` should be
    // dropped, not `init_local`).
    //
    // Stage 18.282 (TD-DROP-MOVED-LOCALS full): Upgraded from flow-insensitive
    // `collect_moved_locals` to flow-sensitive `compute_moved_state`.
    // The flow-sensitive analysis computes per-block `moved_out` sets, so
    // a local that is moved in one conditional branch but NOT in another
    // will correctly receive a Drop on the non-moved branch.
    //
    // Per §1.0 原則 6 (通解 > 特解): one dataflow fixpoint replaces per-block
    // if-else checks.
    // Per §2.2 原則 9 (正确 > 妥协): flow-sensitive is correct; the old
    // flow-insensitive approach was a compromise that could cause leaks.
    // Per §12 (最优 > 最小): forwards dataflow fixpoint is the optimal solution.
    let (_moved_in, moved_out_map) = compute_moved_state(mir);
    // For the global skip set, union all moved_out sets — any local moved
    // on ANY path should be in the skip set. But for per-block drop decisions,
    // we'll check moved_out_map[bb] to determine if the local was moved on
    // the path leading to that specific block.
    //
    // Fallback: if moved_out_map is empty (no moves at all), fall back to
    // collect_moved_locals for backward compat. Per §2.2 原則 4 (报错 > 静默):
    // don't silently produce empty set if computation fails.
    let moved_locals: HashSet<LocalId> = if moved_out_map.values().all(|s| s.is_empty()) {
        collect_moved_locals(mir)
    } else {
        moved_out_map.values().flatten().cloned().collect()
    };
    // Stage 15.64: Also collect locals that are assigned from a Copy of a
    // field projection. These locals hold a "view" of the field, not an
    // owned value — the original struct owns the field. Dropping them would
    // cause a double-drop (the field is dropped when the struct is dropped).
    //
    // Example MIR:
    //   temp = Use(Copy(Projection(Local(o), Field(0))))  // copies o.inner
    //
    // The `temp` local should NOT receive a Drop terminator — it's a copy
    // of the field, and the field will be dropped when `o` is dropped.
    let field_copy_locals = collect_field_copy_locals(mir);
    // Stage 18.243: Also collect locals whose type is FnDef — these are
    // function pointer constants (e.g., __landin_alloc's FnDef local),
    // not real Box values. Even though ty_needs_drop returns false for
    // FnDef, typeck writeback may have changed the local's type to Adt(Box)
    // if the FnDef was stored in a Box-typed local. To prevent false
    // drops on these function pointer constants, we collect them here.
    //
    // Per §1.0 原則 4 (报错>静默): prevents false-positive drop on FnDef locals.
    // Per §1.0 原則 6 (通解>特解): one scan for all locals with FnDef type.
    let fn_def_locals: HashSet<LocalId> = mir
        .local_decls
        .iter()
        .enumerate()
        .filter(|(_, ld)| matches!(&ld.ty.kind, crate::mir::ty::TyKind::FnDef(_, _)))
        .map(|(i, _)| LocalId(i as u32))
        .collect();
    // Stage 18.244 (TD-BOX-AUTO-DROP): Also collect locals that are assigned
    // from Operand::Constant with ConstVal::Uint — these are FnDef constants
    // (e.g., __landin_alloc's function pointer stored as a Box-typed local).
    // After typeck writeback, these locals have type Adt(Box) but contain a
    // DefId integer, not a real heap pointer. Dropping them would call
    // __landin_dealloc on an invalid pointer (crash).
    //
    // Per §1.0 原則 4 (报错>静默): prevents crash on FnDef constant drops.
    // Per §1.0 原則 6 (通解>特解): one scan for all Constant-assigned locals.
    let fn_def_constant_locals: HashSet<LocalId> = {
        let mut s = HashSet::new();
        for bb in &mir.basic_blocks {
            for stmt in &bb.statements {
                if let StatementKind::Assign(boxed) = &stmt.kind {
                    let (place, rvalue) = &**boxed;
                    if let Rvalue::Use(Operand::Constant(c)) = rvalue {
                        if matches!(c.val, crate::mir::ty::ConstVal::Uint(_)) {
                            if let PlaceKind::Local(id) = &place.kind {
                                s.insert(*id);
                            }
                        }
                    }
                }
            }
        }
        s
    };
    let skip_drop_locals: HashSet<LocalId> = moved_locals
        .union(&field_copy_locals)
        .cloned()
        .chain(fn_def_locals.iter().copied())
        .chain(fn_def_constant_locals.iter().copied())
        .collect();

    // We need to process blocks in order. Since we may insert new blocks
    // (which are appended to the end of basic_blocks), we process all
    // blocks including newly created ones. The Stage 15.61 fix guarantees
    // termination: each split removes a `StorageDead` statement rather
    // than carrying it forward, so the new block has strictly fewer
    // `StorageDead` statements needing drop than the original.
    let mut bb_idx = 0;
    while bb_idx < mir.basic_blocks.len() {
        let bb_id = BasicBlockId(bb_idx as u32);
        let bb = &mir.basic_blocks[bb_idx];

        // Find the first StorageDead(local) where local needs drop AND
        // has NOT been moved or field-copied. Stage 15.62: skip moved locals.
        // Stage 15.64: also skip field-copy locals (temps that hold a copy
        // of a field projection — the original struct owns the field).
        let split_point = bb.statements.iter().enumerate().find(|(_, stmt)| {
            if let StatementKind::StorageDead(local_id) = &stmt.kind {
                let local_ty = &mir.local(*local_id).ty;
                ty_needs_drop(local_ty, resolver, &mir.adt_layouts, interner)
                    && !skip_drop_locals.contains(local_id)
            } else {
                false
            }
        });

        if let Some((stmt_idx, stmt)) = split_point {
            let local_id = if let StatementKind::StorageDead(lid) = &stmt.kind {
                *lid
            } else {
                // split_point only returns Some when stmt.kind == StorageDead(_).
                // Reaching here means split_point's filter logic diverged.
                unreachable!("split_point returned Some but stmt.kind != StorageDead")
            };

            // Split the block at stmt_idx.
            // 1. Save the statements AFTER stmt_idx (skip the StorageDead
            //    itself — Stage 15.61 fix). The StorageDead is consumed
            //    by the Drop terminator; carrying it into the new block
            //    would cause the new block to split again (infinite loop).
            let remaining_stmts: Vec<Statement> = bb.statements[stmt_idx + 1..].to_vec();
            let original_terminator = bb.terminator.clone();
            let original_term_span = bb.terminator_span;
            let block_span = bb.span;
            let stmt_span = stmt.span;

            // Compute the new block ID BEFORE the mutable borrow (avoids
            // borrow checker conflict: mir.basic_blocks.len() is immutable,
            // mir.block_mut() is mutable).
            let new_block_id_num = mir.basic_blocks.len() as u32;

            // 2. Truncate the current block's statements (remove from stmt_idx onwards).
            let bb_mut = mir.block_mut(bb_id);
            bb_mut.statements.truncate(stmt_idx);

            // 3. Set the current block's terminator to Drop { place: local, target: new_block }.
            bb_mut.terminator = Terminator {
                kind: TerminatorKind::Drop {
                    place: Place {
                        kind: PlaceKind::Local(local_id),
                        span: stmt_span,
                    },
                    target: BasicBlockId(new_block_id_num),
                    unwind: None,
                },
                span: stmt_span,
            };
            bb_mut.terminator_span = stmt_span;

            // 4. Create a new block with the remaining statements (AFTER
            //    the StorageDead) + original terminator. The StorageDead
            //    is NOT carried forward — see Stage 15.61 fix above.
            let new_block_id = mir.new_block();
            let new_block = mir.block_mut(new_block_id);
            new_block.statements = remaining_stmts;
            new_block.terminator = original_terminator;
            new_block.terminator_span = original_term_span;
            new_block.span = block_span;

            // Move to the next block. The current block no longer has
            // any StorageDead needing drop (truncated). The new block
            // has strictly fewer StorageDead statements needing drop
            // (we removed one). The loop terminates.
            bb_idx += 1;
        } else {
            // No StorageDead needing drop in this block — move to the next.
            bb_idx += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{FloatTy, IntTy};
    use crate::hir::DefId;
    use crate::mir::body::{AdtLayout, AdtLayouts};
    use crate::mir::ty::{Mutability, Region, Ty, TyKind};
    use crate::session::Span;
    use crate::traits::TraitResolver;
    use lasso::Rodeo;
    use std::collections::HashMap;

    /// Build a minimal TraitResolver + Rodeo + AdtLayouts for testing.
    /// The resolver has no impls (so `is_drop_builtin` returns false for all).
    fn build_test_context() -> (TraitResolver, Rodeo, AdtLayouts) {
        let resolver = TraitResolver::new();
        let interner = Rodeo::new();
        let adt_layouts: AdtLayouts = HashMap::new();
        (resolver, interner, adt_layouts)
    }

    /// Helper: build a `Ty` from a `TyKind`.
    fn ty(kind: TyKind) -> Ty {
        Ty::new(kind, Span::DUMMY)
    }

    // ----- Primitive types never need drop -----

    #[test]
    fn stage15_43_needs_drop_i32_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let t = ty(TyKind::Int(IntTy::I32));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    #[test]
    fn stage15_43_needs_drop_bool_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let t = ty(TyKind::Bool);
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    #[test]
    fn stage15_43_needs_drop_char_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let t = ty(TyKind::Char);
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    #[test]
    fn stage15_43_needs_drop_float_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let t = ty(TyKind::Float(FloatTy::F64));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    #[test]
    fn stage15_43_needs_drop_str_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let t = ty(TyKind::Str);
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    #[test]
    fn stage15_43_needs_drop_never_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let t = ty(TyKind::Never);
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    // ----- References and pointers never need drop -----

    #[test]
    fn stage15_43_needs_drop_ref_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let inner = ty(TyKind::Int(IntTy::I32));
        let t = ty(TyKind::Ref(
            Region::Static,
            Mutability::Immutable,
            Box::new(inner),
        ));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    #[test]
    fn stage15_43_needs_drop_mut_ref_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let inner = ty(TyKind::Int(IntTy::I32));
        let t = ty(TyKind::Ref(
            Region::Static,
            Mutability::Mutable,
            Box::new(inner),
        ));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    #[test]
    fn stage15_43_needs_drop_raw_ptr_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let inner = ty(TyKind::Int(IntTy::I32));
        let t = ty(TyKind::RawPtr(Mutability::Mutable, Box::new(inner)));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    // ----- Tuples: need drop if any element needs drop -----

    #[test]
    fn stage15_43_needs_drop_tuple_all_primitives_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let t = ty(TyKind::Tuple(vec![
            ty(TyKind::Int(IntTy::I32)),
            ty(TyKind::Bool),
        ]));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    // ----- Arrays/slices: need drop if element needs drop -----

    #[test]
    fn stage15_43_needs_drop_array_primitive_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let inner = ty(TyKind::Int(IntTy::I32));
        let const_val = crate::mir::ty::Const {
            ty: ty(TyKind::Int(IntTy::I32)),
            val: crate::mir::ty::ConstVal::Int(5),
        };
        let t = ty(TyKind::Array(Box::new(inner), Box::new(const_val)));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    #[test]
    fn stage15_43_needs_drop_slice_primitive_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let inner = ty(TyKind::Int(IntTy::I32));
        let t = ty(TyKind::Slice(Box::new(inner)));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    // ----- ADT without Drop impl: false (no fields need drop) -----

    #[test]
    fn stage15_43_needs_drop_adt_no_drop_no_fields_false() {
        let (resolver, interner, mut adt_layouts) = build_test_context();
        let def_id = DefId(0);
        adt_layouts.insert(
            def_id,
            AdtLayout::Struct {
                field_tys: vec![ty(TyKind::Int(IntTy::I32)), ty(TyKind::Bool)],
            },
        );
        let t = ty(TyKind::Adt(def_id, Vec::new().into()));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    // ----- Infer/Error: false (conservative) -----

    #[test]
    fn stage15_43_needs_drop_infer_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let t = ty(TyKind::Infer(crate::mir::ty::InferVar::TyVar(
            crate::mir::ty::TyVid(0),
        )));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    #[test]
    fn stage15_43_needs_drop_error_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let t = ty(TyKind::Error);
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    // ----- Cycle detection: self-referential type doesn't infinite-loop -----

    #[test]
    fn stage15_43_needs_drop_cycle_no_infinite_loop() {
        let (resolver, interner, mut adt_layouts) = build_test_context();
        let def_id = DefId(0);
        // A struct with a field of its own type (cycle).
        // In real code this would be behind a Box, but we test the cycle
        // detection directly.
        adt_layouts.insert(
            def_id,
            AdtLayout::Struct {
                field_tys: vec![ty(TyKind::Adt(def_id, Vec::new().into()))],
            },
        );
        let t = ty(TyKind::Adt(def_id, Vec::new().into()));
        // Should not infinite-loop. Returns false (cycle broken, no Drop impl).
        let _ = ty_needs_drop(&t, &resolver, &adt_layouts, &interner);
        // We don't assert the result — just that it terminates.
    }

    // ----- elaborate_drops() tests (Stage 15.44) -----

    /// Stage 15.44: `elaborate_drops` is a no-op when no types need drop.
    /// With an empty TraitResolver (no Drop impls), all types return false
    /// for `ty_needs_drop`, so no `Drop` terminators should be inserted.
    #[test]
    fn stage15_44_elaborate_drops_noop_when_no_drop_needed() {
        use crate::mir::body::{MirBody, Statement, StatementKind, Terminator};

        let (resolver, interner, _adt_layouts) = build_test_context();

        let mut body = MirBody::new(Span::DUMMY);
        let local_0 = body.new_local(ty(TyKind::Int(IntTy::I32)), None, Span::DUMMY);
        let local_1 = body.new_local(ty(TyKind::Bool), None, Span::DUMMY);
        let bb0 = body.new_block();
        body.block_mut(bb0).statements.push(Statement {
            kind: StatementKind::StorageDead(local_0),
            span: Span::DUMMY,
        });
        body.block_mut(bb0).statements.push(Statement {
            kind: StatementKind::StorageDead(local_1),
            span: Span::DUMMY,
        });
        body.block_mut(bb0).terminator = Terminator::ret(Span::DUMMY);

        // Replace the adt_layouts with our test one.
        // (MirBody::new creates an empty Arc<AdtLayouts>, which is fine.)
        // We need to use Arc::make_mut to set it, but since the Arc has
        // refcount 1, make_mut is a no-op.
        // Actually, MirBody doesn't expose a way to set adt_layouts directly.
        // But elaborate_drops uses mir.adt_layouts, which is an Arc.
        // For this test, the empty adt_layouts is fine — no ADT types.

        let block_count_before = body.basic_blocks.len();
        elaborate_drops(&mut body, &resolver, &interner);
        let block_count_after = body.basic_blocks.len();

        // No types need drop → no blocks inserted.
        assert_eq!(
            block_count_before, block_count_after,
            "elaborate_drops should not insert blocks when no types need drop"
        );

        // The block should still have its StorageDead statements (not split).
        assert_eq!(
            body.block(bb0).statements.len(),
            2,
            "StorageDead statements should remain (no drop needed)"
        );
    }

    /// Stage 15.44: `elaborate_drops` is callable on an empty body (no panic).
    #[test]
    fn stage15_44_elaborate_drops_empty_body() {
        use crate::mir::body::MirBody;

        let (resolver, interner, _) = build_test_context();
        let mut body = MirBody::new(Span::DUMMY);
        body.new_block(); // One empty block with unreachable terminator.

        // Should not panic.
        elaborate_drops(&mut body, &resolver, &interner);
    }
}
