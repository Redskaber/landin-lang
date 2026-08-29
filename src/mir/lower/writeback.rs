//! Stage 15.7 (v0.2): Consolidated type writeback passes.
//!
//! This module consolidates the 8 driver writeback passes (Stages 14.30-14.84)
//! into 2 functions:
//!
//! - [`writeback_type_propagation`] — merges passes 1-5 (Tuple literal types,
//!   Call dest types, Field projection Copy dests, Index projection Copy
//!   dests, Copy/Move chain propagation) into a single fixpoint walk.
//! - [`writeback_closures`] — merges passes 6-8 (Closure substs writeback,
//!   Closure local_decl.ty update, Closure extract locals update) into a
//!   single 3-sub-pass walk.
//!
//! # Why consolidate?
//!
//! Per `docs/develop/v0/stage-15/v0.2-preparation.md` Phase 1 Task 5:
//! the 8 incremental passes were correct for v0.1 (each fixed a real bug)
//! but v0.2 should consolidate for maintainability and performance.
//! The consolidation reduces 6× constant factor overhead and makes the
//! type-propagation logic easier to reason about.
//!
//! # Architecture
//!
//! Per §16 (interface isolation): these functions take `&mut MirBody` (and
//! `&FnSigTable` for type propagation) and mutate local_decls in place.
//! They are pure MIR-to-MIR transforms — no HIR access. The driver calls
//! them after typeck completes.
//!
//! Per §23 (API naming): both functions follow the `<verb>_<noun>` pattern
//! (`writeback_*`). The module name `writeback` matches the noun.
//!
//! Per §1.0 原则 1 "长期 > 短期": the consolidation is the right long-term
//! structure, even though the incremental passes were correct short-term.

use crate::hir::DefId;
use crate::mir::body::{MirBody, StatementKind, TerminatorKind};
use crate::mir::place::{AggregateKind, Operand, PlaceKind, ProjectionElem, Rvalue};
use crate::mir::ty::{Ty, TyKind};
use crate::session::Span;
use std::collections::HashMap;

/// Check if a Ty is Infer or Error (the two kinds that need writeback).
///
/// Per §1.0 原则 3 "显式 > 隐式": the helper makes the "needs writeback"
/// predicate explicit at every callsite.
fn needs_writeback(ty: &Ty) -> bool {
    // Stage 18.347 (P2 soundness fix): Also treat Param as "needs writeback".
    //
    // Was: only Infer(_) and Error were treated as needing writeback. But
    // Param(N) (unsubstituted generic placeholder) can leak into local_decls
    // when resolve_field_type returns the unsubstituted field type (because
    // the receiver's substs weren't available at lower time). Without
    // including Param in needs_writeback, the writeback fixpoint would
    // skip these locals — leaving them as Param, which codegen then
    // silently maps to EmitType::I32 (the default fallback for unknown
    // TyKind in mir_type_to_emit_type).
    //
    // Stage 18.351 (P2 soundness fix): Make needs_writeback RECURSIVE.
    // Was: only checked the OUTER kind (e.g., `RawPtr(_, Param(0))` →
    // outer is RawPtr → returned false → writeback skipped). This left
    // `p.local_decl.ty = RawPtr(Mutable, Param(0))` unresolved, causing
    // false "expected *mut <type param>, found *mut i64" errors in
    // post_check_statement.
    //
    // Per §1.0 原則 6 (通解 > 特解): one recursive check handles all
    // composite types (RawPtr/Ref/Slice/Array/Tuple/Adt/Closure/FnDef
    // with nested Param/Infer/Error).
    // Per §20 (iterative audit): same class as Stage 18.347 — Param leak
    // in nested types was missed by the outer-only check.
    type_needs_writeback(ty)
}

/// Stage 18.351: Recursive helper — does a Ty (recursively) contain
/// any Infer/Error/Param?
///
/// Per §1.0 原則 3 (显式 > 隐式): explicit predicate.
/// Per §16: pure MIR data predicate.
fn type_needs_writeback(ty: &Ty) -> bool {
    match &ty.kind {
        TyKind::Infer(_) | TyKind::Error | TyKind::Param(_) => true,
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) | TyKind::Slice(inner) => {
            type_needs_writeback(inner)
        }
        TyKind::Array(elem, _) => type_needs_writeback(elem),
        TyKind::Tuple(tys) => tys.iter().any(type_needs_writeback),
        TyKind::Adt(_, substs) => substs.iter().any(type_needs_writeback),
        TyKind::Closure(_, substs) => substs.iter().any(type_needs_writeback),
        TyKind::FnDef(_, substs) => substs.iter().any(type_needs_writeback),
        _ => false,
    }
}

/// Stage 18.347: Helper — does a Ty (recursively) contain any Param?
///
/// Used by Rule 3 (Field projection) to decide whether to apply
/// `substitute(field_ty, base_substs)` for unsubstituted generic placeholders.
///
/// Per §1.0 原則 3 (显式 > 隐式): explicit predicate makes the
/// "needs substitution" check visible at every callsite.
/// Per §16: pure MIR data predicate, no HIR access.
fn type_contains_param(ty: &Ty) -> bool {
    match &ty.kind {
        TyKind::Param(_) => true,
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) | TyKind::Slice(inner) => {
            type_contains_param(inner)
        }
        TyKind::Array(elem, _) => type_contains_param(elem),
        TyKind::Tuple(tys) => tys.iter().any(type_contains_param),
        TyKind::Adt(_, substs) => substs.iter().any(type_contains_param),
        TyKind::Closure(_, substs) => substs.iter().any(type_contains_param),
        TyKind::FnDef(_, substs) => substs.iter().any(type_contains_param),
        _ => false,
    }
}

/// Stage 15.7: Consolidated type-propagation writeback (passes 1-5).
///
/// Walks the MIR body in a fixpoint loop, applying all type-propagation
/// rules until no more changes are made. Each iteration walks all basic
/// blocks once, checking every Assign statement and Call terminator.
///
/// # Rules applied (per iteration)
///
/// 1. **Tuple Aggregate**: `loc = (a, b, c)` → build `Tuple([a_ty, b_ty, c_ty])`
///    from operand types and write to `loc`'s local_decl.
/// 2. **Call dest**: `loc = call f(...)` → look up `f`'s return type in
///    `fn_sigs` and write to `loc`'s local_decl.
/// 3. **Field projection Copy/Move**: `loc = Copy(tup.0)` → resolve field
///    type from `tup`'s Tuple type and write to `loc`.
/// 4. **Index projection Copy/Move**: `loc = Copy(arr[i])` → resolve
///    element type from `arr`'s Array type and write to `loc`.
/// 5. **Local-to-local Copy/Move**: `loc = Copy(other)` → propagate
///    `other`'s resolved type to `loc`.
///
/// # Fixpoint termination
///
/// The loop terminates when an iteration makes no changes. Each iteration
/// can only transition a local from `Infer`/`Error` to a concrete type —
/// never the reverse. So the loop runs at most `local_count + 1` iterations
/// (worst case: a chain of `loc_A = Copy(loc_B); loc_B = Copy(loc_C); ...`
/// where each iteration resolves one local).
///
/// # Parameters
///
/// - `mir`: the MIR body to mutate (local_decls updated in place)
/// - `fn_sigs`: map from fn DefId to MIR Sig (used by Call dest rule)
///
/// Per §23 (API Naming): `pub fn <verb>_<noun>(...)` pattern.
/// Per §16: pure MIR-to-MIR transform, no HIR access.
pub fn writeback_type_propagation(
    mir: &mut MirBody,
    fn_sigs: &HashMap<DefId, crate::mir::ty::Sig>,
) {
    loop {
        let mut changes: Vec<(usize, Ty)> = Vec::new();

        for bb in &mir.basic_blocks {
            // Rules 1, 3, 4, 5: walk Assign statements.
            for stmt in &bb.statements {
                if let StatementKind::Assign(boxed) = &stmt.kind {
                    let (place, rvalue) = &**boxed;
                    if let PlaceKind::Local(dest_id) = &place.kind {
                        let dest_idx = dest_id.0 as usize;
                        // Skip if dest already has a concrete type.
                        let dest_ty = &mir.local_decls[dest_idx].ty;
                        if !needs_writeback(dest_ty) {
                            continue;
                        }
                        // Try each rule; first match wins.
                        if let Some(new_ty) = compute_writeback_ty(rvalue, mir) {
                            // Stage 15.7 fix: only push if the new type is
                            // itself concrete. Without this check, if the
                            // source type is Infer (e.g., a generic method's
                            // return type T), we'd write Infer to dest,
                            // dest stays Infer, and the fixpoint never
                            // converges — infinite loop.
                            if !needs_writeback(&new_ty) {
                                changes.push((dest_idx, new_ty));
                            }
                        }
                    }
                }
            }

            // Rule 2: Call dest writeback (terminator-level).
            if let TerminatorKind::Call {
                func, destination, ..
            } = &bb.terminator.kind
            {
                if let PlaceKind::Local(id) = &destination.kind {
                    let dest_idx = id.0 as usize;
                    let dest_ty = &mir.local_decls[dest_idx].ty;
                    if !needs_writeback(dest_ty) {
                        continue;
                    }
                    if let Some(new_ty) = compute_call_dest_ty(func, fn_sigs) {
                        // Same fixpoint convergence check as above.
                        if !needs_writeback(&new_ty) {
                            changes.push((dest_idx, new_ty));
                        }
                    }
                }
            }
        }

        if changes.is_empty() {
            break;
        }
        for (idx, ty) in changes {
            mir.local_decls[idx].ty = ty;
        }
    }
}

/// Compute the writeback type for an Assign's RHS, if any rule applies.
///
/// Handles rules 1, 3, 4, 5 (Call dest is handled separately because it's
/// a terminator, not a statement).
///
/// Returns `None` if no rule applies or if the source types are still
/// Infer/Error (can't resolve yet — will retry in next fixpoint iteration).
fn compute_writeback_ty(rvalue: &Rvalue, mir: &MirBody) -> Option<Ty> {
    match rvalue {
        // Rule 1: Tuple Aggregate — build tuple type from operand types.
        Rvalue::Aggregate(AggregateKind::Tuple, operands) => {
            let elem_tys: Vec<Ty> = operands.iter().map(|op| operand_ty(op, mir)).collect();
            Some(Ty::new(TyKind::Tuple(elem_tys), Span::DUMMY))
        }

        // Rules 3, 4, 5: Use(Copy|Move(place)) — propagate from source.
        Rvalue::Use(Operand::Copy(src_place) | Operand::Move(src_place)) => {
            compute_use_writeback_ty(src_place, mir)
        }

        _ => None,
    }
}

/// Compute the type for a `Use(Copy|Move(place))` writeback.
///
/// Handles:
/// - Rule 5: `place = Local(src_id)` → propagate src's type
/// - Rule 3: `place = Projection(Local(base), Field(i, ty))` → resolve field ty
/// - Rule 4: `place = Projection(Local(base), Index|ConstantIndex)` → resolve elem ty
fn compute_use_writeback_ty(src_place: &crate::mir::place::Place, mir: &MirBody) -> Option<Ty> {
    match &src_place.kind {
        // Rule 5: Local-to-local propagation.
        PlaceKind::Local(src_id) => {
            let src_ty = &mir.local_decls[src_id.0 as usize].ty;
            if !needs_writeback(src_ty) {
                Some(src_ty.clone())
            } else {
                None
            }
        }

        // Rules 3, 4: Projection from a local OR nested projection.
        PlaceKind::Projection(base, elem) => {
            // Stage 18.361 (P2 soundness fix): Recursively resolve
            // base type for nested projections (e.g., `o.inner.ptr`
            // where base = `o.inner` is itself a Projection).
            //
            // Was: `let PlaceKind::Local(base_id) = &base.kind else {
            // return None; }` — only handled single-level Field
            // projections from a Local. Nested projections like
            // `o.inner.ptr` (Projection(Projection(Local(o), Field(...)),
            // Field(...))) had base = Projection(...) which didn't match
            // Local → returned None → writeback skipped → local_decl.ty
            // stayed as unsubstituted Param → false type mismatch error.
            //
            // Fix: If base is not a Local, recursively compute its
            // writeback type via `compute_use_writeback_ty`. This handles
            // arbitrary nesting depths.
            //
            // Per §1.0 原則 6 (通解 > 特解): one recursive path covers
            // all nesting depths (o.inner.ptr.inner.ptr...).
            // Per §12 (最优 > 最小): root-cause fix at the writeback
            // resolution site, not a post-hoc re-run.
            // Per §20 (iterative audit): same class as Stage 18.347/18.360
            // — Param leak in nested field projection.
            let base_ld = if let PlaceKind::Local(base_id) = &base.kind {
                mir.local_decls.get(base_id.0 as usize)?
            } else {
                // Base is a nested Projection — recursively resolve it.
                let base_ty = compute_use_writeback_ty(base, mir)?;
                // Create a synthetic LocalDecl-like reference.
                // We can't get a real LocalDecl, so we use the resolved
                // type directly. We need to handle this below.
                // Instead of getting base_ld, we use base_ty directly.
                // Refactor: use base_ty instead of base_ld for the
                // substitute check below.
                match elem {
                    ProjectionElem::Field(field_id, field_ty) => {
                        if type_contains_param(field_ty) {
                            if let TyKind::Adt(_, substs) = &base_ty.kind {
                                if !substs.is_empty() {
                                    return Some(crate::mir::substitute::substitute(
                                        field_ty, substs,
                                    ));
                                }
                            }
                            if let TyKind::Ref(_, _, inner) = &base_ty.kind {
                                if let TyKind::Adt(_, substs) = &inner.kind {
                                    if !substs.is_empty() {
                                        return Some(crate::mir::substitute::substitute(
                                            field_ty, substs,
                                        ));
                                    }
                                }
                            }
                        }
                        if !needs_writeback(field_ty) {
                            return Some(field_ty.clone());
                        }
                        if let TyKind::Tuple(field_tys) = &base_ty.kind {
                            return field_tys.get(field_id.0 as usize).cloned();
                        }
                        return None;
                    }
                    ProjectionElem::Index(_) | ProjectionElem::ConstantIndex { .. } => {
                        if let TyKind::Array(elem_ty, _) = &base_ty.kind {
                            return Some(elem_ty.as_ref().clone());
                        }
                        return None;
                    }
                    _ => return None,
                }
            };
            match elem {
                // Rule 3: Field projection — resolve field ty from base Tuple.
                ProjectionElem::Field(field_id, field_ty) => {
                    // Stage 18.347 (P2 soundness fix): Apply generic substs
                    // from the base struct's resolved type to the field_ty.
                    //
                    // Was: only handled Infer field_ty via the Tuple branch
                    // below. But Param field_ty (unsubstituted generic
                    // placeholder) was treated as "concrete" and returned
                    // as-is, leaving the local_decl with a Param type that
                    // codegen silently maps to i32.
                    //
                    // Per §1.0 原則 3 (显式 > 隐式): explicit subst, not
                    // silent i32 fallback.
                    // Per §1.0 原則 6 (通解 > 特解): one subst path for all
                    // generic struct field accesses in writeback.
                    // Per §20 (iterative audit): same class as Stage 18.346
                    // (Aggregate path) — Field projection path was missed.
                    if type_contains_param(field_ty) {
                        if let TyKind::Adt(_, substs) = &base_ld.ty.kind {
                            if !substs.is_empty() {
                                return Some(crate::mir::substitute::substitute(field_ty, substs));
                            }
                        }
                        // Also handle Ref-to-Adt base (e.g., `&self.field`).
                        if let TyKind::Ref(_, _, inner) = &base_ld.ty.kind {
                            if let TyKind::Adt(_, substs) = &inner.kind {
                                if !substs.is_empty() {
                                    return Some(crate::mir::substitute::substitute(
                                        field_ty, substs,
                                    ));
                                }
                            }
                        }
                    }
                    if !needs_writeback(field_ty) {
                        // Field ty is already concrete — use it.
                        return Some(field_ty.clone());
                    }
                    // Field ty is Infer — try to resolve from base's Tuple type.
                    if let TyKind::Tuple(field_tys) = &base_ld.ty.kind {
                        return field_tys.get(field_id.0 as usize).cloned();
                    }
                    None
                }
                // Rule 4: Index/ConstantIndex projection — resolve elem ty from base Array.
                ProjectionElem::Index(_) | ProjectionElem::ConstantIndex { .. } => {
                    if let TyKind::Array(elem_ty, _) = &base_ld.ty.kind {
                        return Some(elem_ty.as_ref().clone());
                    }
                    None
                }
                _ => None,
            }
        }

        _ => None,
    }
}

/// Compute the Call dest type from the callee's fn_sig.
///
/// Rule 2: `loc = call f(...)` → look up `f`'s return type in `fn_sigs`.
fn compute_call_dest_ty(
    func: &Operand,
    fn_sigs: &HashMap<DefId, crate::mir::ty::Sig>,
) -> Option<Ty> {
    let Operand::Constant(c) = func else {
        return None;
    };
    // Stage 18.375 (TD-AS-CAST-TRUNCATION): use try_from + expect instead of
    // `as u32`. Per §1.0 原則 1 (内存安全决不能妥协): silent truncation u128→u32
    // could mask a corrupted ConstVal. Per §2 原则 3 (显式 > 隐式): expect documents
    // the FnDef invariant.
    let did = match &c.val {
        crate::mir::ty::ConstVal::Uint(n) => {
            DefId(u32::try_from(*n).expect("FnDef ConstVal::Uint must fit u32 (DefId is u32)"))
        }
        crate::mir::ty::ConstVal::Int(n) => {
            DefId(u32::try_from(*n).expect("FnDef ConstVal::Int must fit u32 (DefId is u32)"))
        }
        _ => return None,
    };
    let sig = fn_sigs.get(&did)?;
    Some(sig.output.as_ref().clone())
}

/// Get the Ty of an operand (for Tuple Aggregate writeback).
///
/// For Copy/Move: reads from the source local's local_decl.
/// For Constant: returns the constant's Ty.
/// Falls back to a fresh Infer TyVid(0) if the source local is missing
/// (preserves existing v0.1 behavior — see Stage 14.49 commit).
fn operand_ty(op: &Operand, mir: &MirBody) -> Ty {
    match op {
        Operand::Copy(p) | Operand::Move(p) => {
            if let PlaceKind::Local(id) = &p.kind {
                mir.local_decls
                    .get(id.0 as usize)
                    .map(|ld| ld.ty.clone())
                    .unwrap_or_else(fresh_infer_ty)
            } else {
                fresh_infer_ty()
            }
        }
        Operand::Constant(c) => c.ty.clone(),
    }
}

/// Construct a fresh Infer TyVid(0) for fallback cases.
///
/// Per §1.0 原则 3 "显式 > 隐式": the fallback is explicit. The TyVid(0)
/// is a placeholder — typeck should have resolved it, but if not, the
/// writeback doesn't make things worse.
fn fresh_infer_ty() -> Ty {
    Ty::new(
        TyKind::Infer(crate::mir::ty::InferVar::TyVar(crate::mir::ty::TyVid(0))),
        Span::DUMMY,
    )
}

/// Stage 15.7: Consolidated closure writeback (passes 6-8).
///
/// Walks the MIR body in 3 sub-passes (all linear, no fixpoint needed):
///
/// 1. **Closure substs + local_decl**: For each `loc = Aggregate(Closure(def_id, _), operands)`:
///    - Resolve each subst from the corresponding operand's source local type.
///    - Update the Aggregate's substs in place.
///    - Update `loc`'s local_decl.ty to `Closure(def_id, resolved_substs)`.
///
/// 2. **Closure Move propagation**: For each `loc = Use(Move(closure_tmp))`:
///    - If `closure_tmp`'s type is now `Closure(_, [resolved])` (no Infer),
///      propagate it to `loc`'s local_decl.ty.
///
/// 3. **Closure extract locals**: For each `loc = Use(Copy(Projection(closure_local, Field(i, _))))`:
///    - Look up `closure_local`'s resolved subst at index `i`.
///    - Write it to `loc`'s local_decl.ty.
///
/// # Why no fixpoint?
///
/// Sub-pass 1 produces the resolved substs. Sub-pass 2 consumes them to
/// propagate to user-visible locals. Sub-pass 3 consumes sub-pass 2's
/// result to update extract locals. The dependency chain is linear —
/// each sub-pass runs exactly once, in order.
///
/// # Parameters
///
/// - `mir`: the MIR body to mutate (local_decls + Aggregate substs updated)
///
/// Per §23 (API Naming): `pub fn <verb>_<noun>(...)` pattern.
/// Per §16: pure MIR-to-MIR transform, no HIR access.
pub fn writeback_closures(mir: &mut MirBody) {
    // Sub-pass 1: Closure substs + local_decl.ty update.
    // Snapshot-then-mutate pattern: we read from local_decls to compute
    // resolved substs, then mutate both the Aggregate and the closure
    // local's local_decl.ty.
    for bb in &mut mir.basic_blocks {
        for stmt_idx in 0..bb.statements.len() {
            let (lhs_local_id, resolved_substs, closure_def_id) = {
                let stmt = &bb.statements[stmt_idx];
                let StatementKind::Assign(boxed) = &stmt.kind else {
                    continue;
                };
                let (place, rvalue) = &**boxed;
                let Rvalue::Aggregate(AggregateKind::Closure(def_id, _substs), operands) = rvalue
                else {
                    continue;
                };
                // Get the LHS localId (the closure tmp local).
                let lhs_local_id = match &place.kind {
                    PlaceKind::Local(id) => Some(*id),
                    _ => None,
                };
                // Snapshot source local IDs and resolved types.
                let src_local_ids: Vec<Option<crate::mir::place::LocalId>> = operands
                    .iter()
                    .map(|op| match op {
                        Operand::Copy(p) | Operand::Move(p) => {
                            if let PlaceKind::Local(id) = &p.kind {
                                Some(*id)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    })
                    .collect();
                let resolved_substs: Vec<Option<Ty>> = src_local_ids
                    .iter()
                    .map(|src_id_opt| {
                        src_id_opt
                            .and_then(|src_id| mir.local_decls.get(src_id.0 as usize))
                            .map(|ld| ld.ty.clone())
                            .filter(|ty| !needs_writeback(ty))
                    })
                    .collect();
                (lhs_local_id, resolved_substs, Some(*def_id))
            };

            // Now mutate: update AggregateKind substs AND closure local's local_decl.ty.
            if let Some(closure_def_id) = closure_def_id {
                if !resolved_substs.is_empty() {
                    let new_substs: Vec<Ty> = resolved_substs
                        .iter()
                        .map(|opt| {
                            opt.clone()
                                .unwrap_or_else(|| Ty::new(TyKind::Error, Span::DUMMY))
                        })
                        .collect();
                    if let StatementKind::Assign(boxed) = &mut bb.statements[stmt_idx].kind {
                        let (_, rv) = &mut **boxed;
                        if let Rvalue::Aggregate(AggregateKind::Closure(_, substs), _) = rv {
                            // Stage 15.10: SubstsRef is now Rc<[Ty]> (immutable).
                            // Rebuild the Vec, mutate, convert back to Rc<[Ty]>.
                            let mut new_substs_vec: Vec<Ty> = substs.iter().cloned().collect();
                            for (i, resolved_ty_opt) in resolved_substs.iter().enumerate() {
                                if let Some(ty) = resolved_ty_opt {
                                    if i < new_substs_vec.len() {
                                        new_substs_vec[i] = ty.clone();
                                    }
                                }
                            }
                            *substs = new_substs_vec.into();
                        }
                    }
                    // Also update the closure local's local_decl.ty so the alloca size matches.
                    if let Some(lhs_id) = lhs_local_id {
                        if let Some(lhs_ld) = mir.local_decls.get_mut(lhs_id.0 as usize) {
                            let new_closure_ty = Ty::new(
                                TyKind::Closure(closure_def_id, new_substs.clone().into()),
                                Span::DUMMY,
                            );
                            lhs_ld.ty = new_closure_ty;
                        }
                    }
                }
            }
        }
    }

    // Sub-pass 2: Propagate updated closure types through `f = Move(closure_tmp)`.
    // After sub-pass 1, closure_tmp has the correct `Closure(_, [resolved])` type.
    // The user-visible `f` local (assigned via Move(closure_tmp)) still has the
    // stale `Closure(_, [Infer])` type — propagate.
    #[allow(clippy::collapsible_match)]
    for bb in &mut mir.basic_blocks {
        for stmt in &mut bb.statements {
            let StatementKind::Assign(boxed) = &mut stmt.kind else {
                continue;
            };
            let (place, rvalue) = &**boxed;
            let Rvalue::Use(op) = rvalue else {
                continue;
            };
            let Operand::Move(src_place) = op else {
                continue;
            };
            let PlaceKind::Local(src_id) = &src_place.kind else {
                continue;
            };
            let Some(src_ld) = mir.local_decls.get(src_id.0 as usize) else {
                continue;
            };
            let src_ty = src_ld.ty.clone();
            // Source must be a Closure with all substs resolved (no Infer).
            let src_ok = matches!(&src_ty.kind,
                TyKind::Closure(_, substs)
                if !substs.iter().any(needs_writeback)
            );
            if !src_ok {
                continue;
            }
            let PlaceKind::Local(lhs_id) = &place.kind else {
                continue;
            };
            if let Some(lhs_ld) = mir.local_decls.get_mut(lhs_id.0 as usize) {
                // Only update if LHS is still Infer or stale Closure (with Infer substs).
                let needs_update = needs_writeback(&lhs_ld.ty)
                    || matches!(&lhs_ld.ty.kind,
                        TyKind::Closure(_, substs)
                        if substs.iter().any(needs_writeback)
                    );
                if needs_update {
                    lhs_ld.ty = src_ty;
                }
            }
        }
    }

    // Sub-pass 3: Update extract locals' types for closure captures.
    // Pattern: `extract_local = Use(Copy(Projection(closure_local, Field(i, _))))`.
    // The extract local's type should match `closure_local.substs[i]`.
    #[allow(clippy::collapsible_match)]
    for bb in &mut mir.basic_blocks {
        for stmt in &mut bb.statements {
            let StatementKind::Assign(boxed) = &mut stmt.kind else {
                continue;
            };
            let (place, rvalue) = &**boxed;
            let Rvalue::Use(op) = rvalue else {
                continue;
            };
            let Operand::Copy(src_place) = op else {
                continue;
            };
            let PlaceKind::Projection(base, elem) = &src_place.kind else {
                continue;
            };
            let ProjectionElem::Field(field_id, _) = elem else {
                continue;
            };
            let PlaceKind::Local(closure_local_id) = &base.kind else {
                continue;
            };
            // Get the closure's resolved subst at field_id.
            let Some(resolved_cap_ty) =
                mir.local_decls
                    .get(closure_local_id.0 as usize)
                    .and_then(|ld| {
                        if let TyKind::Closure(_, substs) = &ld.ty.kind {
                            substs.get(field_id.0 as usize).cloned()
                        } else {
                            None
                        }
                    })
            else {
                continue;
            };
            // Update the extract local's type.
            let PlaceKind::Local(lhs_id) = &place.kind else {
                continue;
            };
            if let Some(lhs_ld) = mir.local_decls.get_mut(lhs_id.0 as usize) {
                if needs_writeback(&lhs_ld.ty) {
                    lhs_ld.ty = resolved_cap_ty;
                }
            }
        }
    }
}

/// Stage 18.102 (TD-MONO-INFER fix): Writeback inferred substs into FnDef types.
///
/// After typeck, generic function calls like `id(42)` (without turbofish) have
/// `FnDef(def_id, [])` in MIR — the substs are empty because MIR lowering
/// happens before type inference. This pass walks all `Call` terminators,
/// matches the arg types against the function's param types (which contain
/// `Param(N)`), and writes back the inferred substs.
///
/// # Algorithm
///
/// For each `Call { func: Copy(local), args, .. }`:
/// 1. Read `local_decls[local].ty` — if `FnDef(def_id, [])` (empty substs):
/// 2. Look up `fn_sigs[def_id]` to get the sig (inputs contain `Param(N)`)
/// 3. For each `(arg, input_ty)` pair:
///    - Resolve the arg's type from local_decls
///    - If `input_ty` is `Param(N)`, record `bindings[N] = arg_ty`
/// 4. Also check the return type: if `sig.output` is `Param(N)`, use the
///    destination local's type as `bindings[N]`
/// 5. Build the substs vector from bindings (ordered by param index)
/// 6. Write back `FnDef(def_id, substs)` to `local_decls[local].ty`
///
/// # Limitations (documented as design simplifications)
///
/// - **S1**: Only top-level `Param` types in sig inputs/output are matched.
///   Nested Params (e.g., `fn foo<T>(x: Vec<T>)` where the input is
///   `Adt(Vec, [Param(0)])`) are NOT extracted. This is a simplification —
///   full nested param extraction requires recursive type matching.
///   **Impact**: Generic functions with nested param types in their sig
///   (e.g., `fn wrap<T>(x: Vec<T>)`) won't get substs for `T`.
///   **Fix plan**: v0.2 Phase 2 — recursive param extraction in `collect_param_bindings`.
///
/// - **S2**: Only `Operand::Copy(Place::local(id))` func operands are handled.
///   `Operand::Constant(Const { ty: FnDef(...), .. })` func operands (used in
///   some method call paths) are NOT handled. This is a simplification —
///   the constant case requires mutating the Const's type, which is more
///   complex. **Fix plan**: v0.2 Phase 2 — handle Constant func operands.
///
/// Per §23: `writeback_fndef_substs` follows `<verb>_<noun>_<noun>` pattern.
/// Per §16: takes pre-computed `fn_sigs` (data, not HIR) + `generics_map`.
/// Per §1.0 原則 6 "通用 > 特例": one pass handles all generic function calls.
/// Per §2.0 原則 9 "正确 > 妥协": implicit inference now works (was empty substs).
pub fn writeback_fndef_substs(
    mir: &mut MirBody,
    fn_sigs: &HashMap<DefId, crate::mir::ty::Sig>,
    generics_map: &HashMap<DefId, Vec<crate::mir::ty::ParamTy>>,
) {
    // Collect changes first, then apply (avoid borrow conflicts).
    let mut changes: Vec<(usize, Ty)> = Vec::new();
    // Stage 18.111 (S9 fix): Destination local type changes (substituted
    // with callee substs so generic return types become concrete).
    let mut dest_changes: Vec<(usize, Ty)> = Vec::new();
    // Stage 18.112 (S2 fix): Terminator Constant func type changes
    // (for method calls where func is Operand::Constant).
    let mut terminator_changes: Vec<(usize, Ty)> = Vec::new();

    for (bb_idx, bb) in mir.basic_blocks.iter().enumerate() {
        let term = &bb.terminator;
        if let TerminatorKind::Call {
            func,
            args,
            destination,
            ..
        } = &term.kind
        {
            // Get the func's local_id from the operand.
            // Stage 18.112 (S2 fix): Handle both Copy/Move (regular calls)
            // and Constant (method calls) func operands.
            let (func_local_idx, constant_func_def_id, constant_func_substs) = match func {
                Operand::Copy(place) | Operand::Move(place) => {
                    if let PlaceKind::Local(id) = &place.kind {
                        (Some(id.0 as usize), None, None)
                    } else {
                        (None, None, None)
                    }
                }
                Operand::Constant(c) => {
                    // Method call: func is Constant(Const { ty: FnDef(def_id, substs), val: Uint(def_id) })
                    if let TyKind::FnDef(def_id, substs) = &c.ty.kind {
                        (None, Some(*def_id), Some(substs.clone()))
                    } else {
                        (None, None, None)
                    }
                }
            };

            // Extract def_id and existing_substs based on operand type.
            let (def_id, existing_substs, func_local_idx_opt) = if let Some(idx) = func_local_idx {
                // Copy/Move path: read from local_decls
                let current_ty = match mir.local_decls.get(idx) {
                    Some(ld) => ld.ty.clone(),
                    None => continue,
                };
                match &current_ty.kind {
                    TyKind::FnDef(def_id, substs) => (*def_id, substs.clone(), Some(idx)),
                    _ => continue,
                }
            } else if let (Some(did), Some(substs)) = (constant_func_def_id, constant_func_substs) {
                // Constant path: from the Const's type directly
                (did, substs, None)
            } else {
                continue;
            };

            // Stage 18.111 (S9 fix): Even if substs are already populated
            // (turbofish case), we still need to substitute the destination
            // local's type. So don't `continue` — use existing substs.
            let (substs, is_inferred): (Vec<Ty>, bool) = if !existing_substs.is_empty() {
                (existing_substs.iter().cloned().collect::<Vec<_>>(), false)
            } else {
                // Empty substs — try to infer from args (implicit case).
                // Skip if the function is not generic.
                let generic_params = match generics_map.get(&def_id) {
                    Some(params) if !params.is_empty() => params,
                    _ => continue, // Non-generic function
                };

                // Look up the function's sig.
                let sig = match fn_sigs.get(&def_id) {
                    Some(sig) => sig.clone(),
                    None => continue,
                };

                // Build param bindings from arg types.
                let mut bindings: HashMap<u32, Ty> = HashMap::new();

                for (arg, input_ty) in args.iter().zip(sig.inputs.iter()) {
                    let arg_ty = match arg {
                        Operand::Copy(place) | Operand::Move(place) => {
                            if let PlaceKind::Local(id) = &place.kind {
                                mir.local_decls.get(id.0 as usize).map(|ld| ld.ty.clone())
                            } else {
                                None
                            }
                        }
                        Operand::Constant(c) => Some(c.ty.clone()),
                    };
                    if let Some(arg_ty) = arg_ty {
                        collect_param_bindings(input_ty, &arg_ty, &mut bindings);
                    }
                }

                if let PlaceKind::Local(id) = &destination.kind {
                    if let Some(dest_ld) = mir.local_decls.get(id.0 as usize) {
                        collect_param_bindings(&sig.output, &dest_ld.ty, &mut bindings);
                    }
                }

                let inferred: Vec<Ty> = generic_params
                    .iter()
                    .map(|param| {
                        bindings
                            .get(&param.index)
                            .cloned()
                            .unwrap_or_else(|| Ty::new(TyKind::Error, Span::DUMMY))
                    })
                    .collect();

                if !inferred.is_empty()
                    && inferred.iter().any(|ty| !matches!(ty.kind, TyKind::Error))
                {
                    // Stage 18.112 (S2 fix): For Copy/Move func operands,
                    // write back the FnDef type to local_decls. For Constant
                    // func operands (method calls), write to terminator_changes.
                    if let Some(idx) = func_local_idx_opt {
                        let new_ty =
                            Ty::new(TyKind::FnDef(def_id, inferred.clone().into()), Span::DUMMY);
                        changes.push((idx, new_ty));
                    } else {
                        // Constant func operand: record terminator change
                        // to update the Const's type with inferred substs.
                        let new_ty =
                            Ty::new(TyKind::FnDef(def_id, inferred.clone().into()), Span::DUMMY);
                        terminator_changes.push((bb_idx, new_ty));
                    }
                }
                (inferred, true)
            };

            // Stage 18.111 (S9 fix): Substitute the destination local's type
            // with the callee's substs (whether from turbofish or inferred).
            if !substs.is_empty()
                && substs
                    .iter()
                    .any(|ty| !matches!(ty.kind, TyKind::Error | TyKind::Param(_)))
            {
                let sig = match fn_sigs.get(&def_id) {
                    Some(sig) => sig.clone(),
                    None => continue,
                };
                let substituted_output = crate::mir::substitute(&sig.output, &substs);
                if let PlaceKind::Local(dest_id) = &destination.kind {
                    let dest_idx = dest_id.0 as usize;
                    let current_dest_ty = mir.local_decls.get(dest_idx).map(|ld| ld.ty.clone());
                    if let Some(current) = current_dest_ty {
                        if current.kind != substituted_output.kind {
                            dest_changes.push((dest_idx, substituted_output));
                        }
                    }
                }
            }

            let _ = is_inferred;
        }
    }

    // Apply FnDef type changes.
    for (idx, new_ty) in changes {
        if let Some(ld) = mir.local_decls.get_mut(idx) {
            ld.ty = new_ty;
        }
    }

    // Apply destination local type changes (S9 fix).
    for (idx, new_ty) in dest_changes {
        if let Some(ld) = mir.local_decls.get_mut(idx) {
            ld.ty = new_ty;
        }
    }

    // Apply terminator Constant func type changes (S2 fix).
    for (bb_idx, new_ty) in terminator_changes {
        if let Some(bb) = mir.basic_blocks.get_mut(bb_idx) {
            if let TerminatorKind::Call {
                func: Operand::Constant(c),
                ..
            } = &mut bb.terminator.kind
            {
                c.ty = new_ty;
            }
        }
    }
}

/// Helper: collect Param → Ty bindings from matching two types.
///
/// If `param_ty` is `Param(N)`, records `bindings[N] = concrete_ty`.
/// S1 limitation: does NOT recurse into nested types (e.g., `Vec<T>`).
fn collect_param_bindings(param_ty: &Ty, concrete_ty: &Ty, bindings: &mut HashMap<u32, Ty>) {
    if let TyKind::Param(param) = &param_ty.kind {
        bindings.insert(param.index, concrete_ty.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::IntTy;
    use crate::mir::body::{BasicBlock, LocalDecl, MirBody, Statement, Terminator};
    use crate::mir::place::{AggregateKind, Operand, Place, Rvalue};
    use crate::mir::ty::{InferVar, Mutability, TyKind, TyVid};
    use crate::session::Span;

    /// Build a LocalDecl with the given type and Immutable mutability.
    fn local_decl(ty: Ty) -> LocalDecl {
        LocalDecl {
            ty,
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        }
    }

    /// Build a minimal MirBody with one basic block and the given statements.
    fn build_mir(local_decls: Vec<LocalDecl>, statements: Vec<Statement>) -> MirBody {
        let mut mir = MirBody::new(Span::DUMMY);
        mir.local_decls = local_decls;
        let bb = BasicBlock {
            statements,
            terminator: Terminator::unreachable(Span::DUMMY),
            span: Span::DUMMY,
            terminator_span: Span::DUMMY,
        };
        mir.basic_blocks = vec![bb];
        mir
    }

    /// Stage 15.7 test 1: Tuple Aggregate writeback.
    ///
    /// `loc = (a, b)` where a:i32, b:bool → loc's type resolves to Tuple([i32, bool]).
    #[test]
    fn stage15_7_tuple_aggregate_writeback() {
        let local_decls = vec![
            local_decl(Ty::new(TyKind::Int(IntTy::I32), Span::DUMMY)), // 0: a
            local_decl(Ty::new(TyKind::Bool, Span::DUMMY)),            // 1: b
            local_decl(fresh_infer_test()),                            // 2: loc (dest)
        ];
        let statements = vec![Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(crate::mir::place::LocalId(2), Span::DUMMY),
                Rvalue::Aggregate(
                    AggregateKind::Tuple,
                    vec![
                        Operand::Copy(Place::local(crate::mir::place::LocalId(0), Span::DUMMY)),
                        Operand::Copy(Place::local(crate::mir::place::LocalId(1), Span::DUMMY)),
                    ],
                ),
            ))),
            span: Span::DUMMY,
        }];
        let mut mir = build_mir(local_decls, statements);
        let fn_sigs = HashMap::new();
        writeback_type_propagation(&mut mir, &fn_sigs);
        assert!(
            matches!(&mir.local_decls[2].ty.kind, TyKind::Tuple(tys) if tys.len() == 2),
            "dest local must be Tuple with 2 elements after writeback"
        );
    }

    /// Stage 15.7 test 2: Local-to-local Copy propagation.
    ///
    /// `loc = Copy(src)` where src:i32 → loc's type resolves to i32.
    #[test]
    fn stage15_7_local_copy_writeback() {
        let local_decls = vec![
            local_decl(Ty::new(TyKind::Int(IntTy::I32), Span::DUMMY)), // 0: src
            local_decl(fresh_infer_test()),                            // 1: loc (dest)
        ];
        let statements = vec![Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(crate::mir::place::LocalId(1), Span::DUMMY),
                Rvalue::Use(Operand::Copy(Place::local(
                    crate::mir::place::LocalId(0),
                    Span::DUMMY,
                ))),
            ))),
            span: Span::DUMMY,
        }];
        let mut mir = build_mir(local_decls, statements);
        let fn_sigs = HashMap::new();
        writeback_type_propagation(&mut mir, &fn_sigs);
        assert!(
            matches!(&mir.local_decls[1].ty.kind, TyKind::Int(IntTy::I32)),
            "dest local must be i32 after Copy propagation"
        );
    }

    /// Stage 15.7 test 3: Fixpoint converges on Copy chain.
    ///
    /// `c = Copy(b); b = Copy(a); a = i32_literal`
    /// (a is i32, b is Infer, c is Infer — fixpoint must resolve both b and c.)
    #[test]
    fn stage15_7_fixpoint_copy_chain() {
        let local_decls = vec![
            local_decl(Ty::new(TyKind::Int(IntTy::I32), Span::DUMMY)), // 0: a
            local_decl(fresh_infer_test()),                            // 1: b
            local_decl(fresh_infer_test()),                            // 2: c
        ];
        let statements = vec![
            Statement {
                kind: StatementKind::Assign(Box::new((
                    Place::local(crate::mir::place::LocalId(1), Span::DUMMY),
                    Rvalue::Use(Operand::Copy(Place::local(
                        crate::mir::place::LocalId(0),
                        Span::DUMMY,
                    ))),
                ))),
                span: Span::DUMMY,
            },
            Statement {
                kind: StatementKind::Assign(Box::new((
                    Place::local(crate::mir::place::LocalId(2), Span::DUMMY),
                    Rvalue::Use(Operand::Copy(Place::local(
                        crate::mir::place::LocalId(1),
                        Span::DUMMY,
                    ))),
                ))),
                span: Span::DUMMY,
            },
        ];
        let mut mir = build_mir(local_decls, statements);
        let fn_sigs = HashMap::new();
        writeback_type_propagation(&mut mir, &fn_sigs);
        assert!(
            matches!(&mir.local_decls[1].ty.kind, TyKind::Int(IntTy::I32)),
            "b must be i32 after fixpoint"
        );
        assert!(
            matches!(&mir.local_decls[2].ty.kind, TyKind::Int(IntTy::I32)),
            "c must be i32 after fixpoint (chain resolved)"
        );
    }

    /// Stage 15.7 test 4: No writeback when dest is already concrete.
    #[test]
    fn stage15_7_no_writeback_when_concrete() {
        let local_decls = vec![
            local_decl(Ty::new(TyKind::Bool, Span::DUMMY)), // 0: src
            local_decl(Ty::new(TyKind::Int(IntTy::I32), Span::DUMMY)), // 1: dest (already i32)
        ];
        let statements = vec![Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(crate::mir::place::LocalId(1), Span::DUMMY),
                Rvalue::Use(Operand::Copy(Place::local(
                    crate::mir::place::LocalId(0),
                    Span::DUMMY,
                ))),
            ))),
            span: Span::DUMMY,
        }];
        let mut mir = build_mir(local_decls, statements);
        let fn_sigs = HashMap::new();
        writeback_type_propagation(&mut mir, &fn_sigs);
        // dest should remain i32 (not overwritten with Bool).
        assert!(
            matches!(&mir.local_decls[1].ty.kind, TyKind::Int(IntTy::I32)),
            "concrete dest must not be overwritten"
        );
    }

    /// Stage 15.7 test 5: needs_writeback helper.
    #[test]
    fn stage15_7_needs_writeback_helper() {
        assert!(needs_writeback(&fresh_infer_test()));
        assert!(needs_writeback(&Ty::new(TyKind::Error, Span::DUMMY)));
        assert!(!needs_writeback(&Ty::new(TyKind::Bool, Span::DUMMY)));
        assert!(!needs_writeback(&Ty::new(
            TyKind::Int(IntTy::I32),
            Span::DUMMY
        )));
    }

    /// Helper: fresh Infer TyVid(0) for tests.
    fn fresh_infer_test() -> Ty {
        Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(0))), Span::DUMMY)
    }
}
