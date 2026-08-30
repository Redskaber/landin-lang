//! Type checker — writeback sub-responsibility.
//!
//! Per `docs/stage-committee-process.md` v6.4 §13.4 J1-J6 (Stage 18.128):
//! Split from `checker.rs` to satisfy J6 (科学合理粒度) + J2 (单一职责).
//! This file contains all `writeback_*` and `resolve_*_for_writeback` methods.
//!
//! ## Sub-responsibility
//! Type writeback: after typeck + borrowck complete, write resolved field
//! types back into MIR LocalDecls and FieldTyTable for downstream codegen.
//!
//! ## J1-J6 compliance
//! - J1: typeck design unchanged
//! - J2: this file has one clear responsibility (writeback)
//! - J3: no circular deps (methods operate on `&mut self`)
//! - J4: writeback sub-responsibility is complete
//! - J5: stays within typeck stage
//! - J6: LOC driven by responsibility

use crate::mir::body::*;
use crate::mir::place::*;
use crate::mir::ty::*;

use super::checker::TypeChecker;
use super::tables::FieldTyTable;

impl TypeChecker {
    /// Stage 18.388 (v0.5+ Phase 3 step 5): Phase 3.5 step 1 REMOVED.
    /// Code is preserved as dead code — codegen now resolves field types via
    /// `try_resolve_field_from_adt_layouts` (Stage 18.388). This method is
    /// kept for v0.5+ Phase 3 (FieldTyTable removal) reference and will be
    /// deleted when Phase 3 is complete.
    #[allow(dead_code)]
    pub(super) fn writeback_field_types_with_table(
        &mut self,
        mir: &mut MirBody,
        table: &FieldTyTable,
    ) {
        let mut updates: Vec<(
            usize,
            usize,
            Option<crate::mir::place::Place>,
            Option<crate::mir::place::Rvalue>,
        )> = Vec::new();
        for (bb_idx, bb) in mir.basic_blocks.iter().enumerate() {
            for (stmt_idx, stmt) in bb.statements.iter().enumerate() {
                if let StatementKind::Assign(boxed) = &stmt.kind {
                    let (place, rvalue) = &**boxed;
                    let mut new_place = place.clone();
                    let mut new_rvalue = rvalue.clone();
                    let place_changed = writeback_field_types_in_place_with_table(
                        &mut new_place,
                        mir,
                        table,
                        &mut self.unify,
                    );
                    let rvalue_changed = writeback_field_types_in_rvalue_with_table(
                        &mut new_rvalue,
                        mir,
                        table,
                        &mut self.unify,
                    );
                    if place_changed || rvalue_changed {
                        updates.push((
                            bb_idx,
                            stmt_idx,
                            if place_changed { Some(new_place) } else { None },
                            if rvalue_changed {
                                Some(new_rvalue)
                            } else {
                                None
                            },
                        ));
                    }
                }
            }
        }
        for (bb_idx, stmt_idx, new_place, new_rvalue) in updates {
            if let Some(bb) = mir.basic_blocks.get_mut(bb_idx) {
                if let Some(stmt) = bb.statements.get_mut(stmt_idx) {
                    if let StatementKind::Assign(boxed) = &mut stmt.kind {
                        let (place, rvalue) = &mut **boxed;
                        if let Some(np) = new_place {
                            *place = np;
                        }
                        if let Some(nr) = new_rvalue {
                            *rvalue = nr;
                        }
                    }
                }
            }
        }

        fn resolve_place_type_with_table(lv: &crate::mir::place::Place, mir: &MirBody) -> Ty {
            use crate::mir::place::PlaceKind;
            match &lv.kind {
                PlaceKind::Local(id) => mir
                    .local_decls
                    .get(id.0 as usize)
                    .map(|ld| ld.ty.clone())
                    .unwrap_or_else(|| Ty::new(TyKind::Error, lv.span)),
                PlaceKind::Projection(base, elem) => {
                    // Stage 18.358 (P2 soundness fix): Apply substitute()
                    // when resolving field_ty from a Projection.
                    //
                    // Was: returned `field_ty.clone()` directly — the
                    // unsubstituted `Param(N)` from MIR lower's
                    // `ProjectionElem::Field(_, field_ty)`. This caused
                    // nested generic field access to fail (e.g.,
                    // `o.inner.ptr` where `o: Outer<i64>` and
                    // `Outer<T> { inner: Inner<T> }`).
                    //
                    // Fix: Recursively resolve the base's type first, then
                    // if base is `Adt(_, substs)` with non-empty substs,
                    // apply `substitute(field_ty, substs)` before returning.
                    //
                    // Per §1.0 原則 6 (通解 > 特解): one recursive substitute
                    // covers all nesting depths.
                    // Per §12 (最优 > 最小): root-cause fix at the type
                    // resolution site, not a post-hoc re-run.
                    // Per §20 (iterative audit): same class as Stage 18.357
                    // — resolve_place_type_with_table was the missing piece.
                    let base_ty = resolve_place_type_with_table(base, mir);
                    match elem {
                        crate::mir::place::ProjectionElem::Field(_field_id, field_ty) => {
                            if let TyKind::Adt(_, substs) = &base_ty.kind {
                                if !substs.is_empty() {
                                    return crate::mir::substitute::substitute(field_ty, substs);
                                }
                            }
                            field_ty.clone()
                        }
                        // Stage 18.118: Deref/Index/ConstantIndex/Subslice
                        // projections on non-supported types return Error.
                        // This is intentional — the place type cannot be
                        // resolved, and the Error type surfaces in typeck.
                        _ => Ty::from_kind(TyKind::Error),
                    }
                }
                PlaceKind::Static(_) => Ty::from_kind(TyKind::Error),
            }
        }

        fn writeback_field_types_in_place_with_table(
            lv: &mut crate::mir::place::Place,
            mir: &MirBody,
            table: &FieldTyTable,
            unify: &mut crate::typeck::unify::UnificationTable,
        ) -> bool {
            use crate::mir::place::{PlaceKind, ProjectionElem};
            match &mut lv.kind {
                PlaceKind::Projection(base, elem) => {
                    let mut changed =
                        writeback_field_types_in_place_with_table(base, mir, table, unify);
                    if let ProjectionElem::Field(field_id, field_ty) = elem {
                        let base_ty = resolve_place_type_with_table(base, mir);
                        if let TyKind::Adt(def_id, substs) = &base_ty.kind {
                            if let Some(fields) = table.struct_fields(def_id) {
                                if let Some(resolved) = fields.get(field_id.0 as usize) {
                                    let current = unify.resolve(field_ty);
                                    match &current.kind {
                                        TyKind::Infer(InferVar::TyVar(vid)) => {
                                            // Stage 18.357: Apply substs before binding.
                                            let substituted = if !substs.is_empty() {
                                                crate::mir::substitute::substitute(resolved, substs)
                                            } else {
                                                resolved.clone()
                                            };
                                            unify.bind_ty_var(*vid, substituted.clone());
                                        }
                                        TyKind::Infer(InferVar::IntVar(vid)) => {
                                            let substituted = if !substs.is_empty() {
                                                crate::mir::substitute::substitute(resolved, substs)
                                            } else {
                                                resolved.clone()
                                            };
                                            if let TyKind::Int(int_ty) = &substituted.kind {
                                                unify.bind_int_var(*vid, *int_ty);
                                            }
                                        }
                                        _ => {}
                                    }
                                    // Stage 18.357 (P2 soundness fix): Apply
                                    // substitute() when overwriting field_ty
                                    // with the table's unresolved HIR type.
                                    //
                                    // Was: `*field_ty = resolved.clone()` which
                                    // overwrote Phase 0's substitute() result
                                    // with unsubstituted `Param(N)` from
                                    // FieldTyTable. This caused Phase 3.5
                                    // regression: local_decl.ty went from
                                    // `RawPtr(Mutable, Int(I64))` back to
                                    // `RawPtr(Mutable, Param(0))`.
                                    //
                                    // Fix: If base_ty is `Adt(_, substs)` with
                                    // non-empty substs, apply `substitute(resolved, substs)`
                                    // before writing to `field_ty`. This is the
                                    // root-cause fix — Phase 3.7's re-writeback
                                    // workaround is still needed for edge cases
                                    // but won't be needed for the common path.
                                    //
                                    // Per §1.0 原則 6 (通解 > 特解): one substitute
                                    // call covers all generic struct fields.
                                    // Per §12 (最优 > 最小): root-cause fix at
                                    // the overwrite site, not a post-hoc re-run.
                                    // Per §20 (iterative audit): same class as
                                    // Stage 18.355 — Phase 3.5 table overwrite
                                    // was the root cause identified in 18.354.
                                    *field_ty = if !substs.is_empty() {
                                        crate::mir::substitute::substitute(resolved, substs)
                                    } else {
                                        resolved.clone()
                                    };
                                    changed = true;
                                }
                            }
                        }
                    }
                    changed
                }
                _ => false,
            }
        }

        fn writeback_field_types_in_rvalue_with_table(
            rv: &mut crate::mir::place::Rvalue,
            mir: &MirBody,
            table: &FieldTyTable,
            unify: &mut crate::typeck::unify::UnificationTable,
        ) -> bool {
            use crate::mir::place::Rvalue;
            match rv {
                Rvalue::Use(op) => {
                    writeback_field_types_in_operand_with_table(op, mir, table, unify)
                }
                Rvalue::BinaryOp(_, a, b) | Rvalue::BinaryOp2(_, a, b) => {
                    writeback_field_types_in_operand_with_table(a, mir, table, unify)
                        | writeback_field_types_in_operand_with_table(b, mir, table, unify)
                }
                Rvalue::Load(_, _) | Rvalue::GetElementPtr { .. } => false,

                Rvalue::UnaryOp(_, op) => {
                    writeback_field_types_in_operand_with_table(op, mir, table, unify)
                }
                Rvalue::Cast(_, op, _) => {
                    writeback_field_types_in_operand_with_table(op, mir, table, unify)
                }
                Rvalue::Aggregate(_, operands) => {
                    let mut changed = false;
                    for op in operands {
                        changed |=
                            writeback_field_types_in_operand_with_table(op, mir, table, unify);
                    }
                    // Stage 18.376 (TD-ARCH-NESTED-GENERIC-FIELD-ACCESS):
                    // For AggregateKind::Adt, the `field_tys` Vec may contain
                    // unsubstituted Param(N) (e.g., nested struct literal
                    // `Outer { inner: Inner<T> }` where field_tys[0] = Inner<Param(0)>).
                    // Apply substitute(field_ty, adt_substs) so codegen sees
                    // the resolved type (Inner<i64>) instead of Param.
                    //
                    // Per §1.0 原則 6 (通解 > 特解): one substitute path for all
                    // nested Adt aggregates. Per §12 (最优 > 最小): root-cause fix
                    // at the writeback site, not a codegen-layer hack.
                    // Per §20 (iterative audit): same class as Stage 18.347
                    // (Field projection) — Aggregate field_tys path was missed.
                    if let Rvalue::Aggregate(
                        AggregateKind::Adt(_, _variant, substs, field_tys),
                        _,
                    ) = rv
                    {
                        if !substs.is_empty() {
                            for field_ty in field_tys.iter_mut() {
                                if typeck_type_contains_param(field_ty) {
                                    let new_ty =
                                        crate::mir::substitute::substitute(field_ty, substs);
                                    if new_ty != *field_ty {
                                        *field_ty = new_ty;
                                        changed = true;
                                    }
                                }
                            }
                        }
                    }
                    changed
                }
                Rvalue::Ref(_, _, lv) => {
                    writeback_field_types_in_place_with_table(lv, mir, table, unify)
                }
            }
        }

        fn writeback_field_types_in_operand_with_table(
            op: &mut crate::mir::place::Operand,
            mir: &MirBody,
            table: &FieldTyTable,
            unify: &mut crate::typeck::unify::UnificationTable,
        ) -> bool {
            use crate::mir::place::Operand;
            match op {
                Operand::Copy(lv) | Operand::Move(lv) => {
                    writeback_field_types_in_place_with_table(lv, mir, table, unify)
                }
                _ => false,
            }
        }
    }

    /// Stage 3.60: Writeback field-load locals using FieldTyTable instead of HIR.
    ///
    /// Stage 18.411 (v0.5+ Phase 2 L3 step 2 refactor): Split the former
    /// `writeback_field_load_locals_with_table` into two logically distinct
    /// methods. The original function bundled two INDEPENDENT concerns:
    ///
    ///   Pass 1 (this method): Resolve field-access result local types
    ///   using FieldTyTable. This is the ARCHITECTURALLY CORRECT position
    ///   for field type resolution — it runs AFTER Phase 3 (writeback_to_
    ///   local_decls), so receiver types are concrete (resolved by typeck
    ///   Phase 1 unify). When the receiver was Infer at MIR lower time
    ///   (function params without HIR fallback, let bindings without
    ///   annotation, Call results without expected_ty), `find_receiver_
    ///   struct_def_id` returned None → field_ty = Infer. This pass
    ///   resolves field_ty to the concrete type from FieldTyTable, with
    ///   substitute() applied for generic structs.
    ///
    ///   Per §1.0 原則 6 (通解 > 特解): one pass handles ALL field-access
    ///   result locals, regardless of why the receiver was Infer.
    ///   Per §1.6 终极检验: this IS the root-cause fix — field types
    ///   cannot be resolved at lower time when receiver is Infer; the
    ///   correct position is post-Phase-3 writeback.
    ///   Per §5.2 (true limit): confirmed 7 consecutive experiments
    ///   (Stage 18.389→18.405). This pass CANNOT be removed in v0.5+
    ///   without restructuring typeck to run before MIR lower (v0.6+).
    pub(super) fn writeback_field_load_locals_with_table(
        &mut self,
        mir: &mut MirBody,
        table: &FieldTyTable,
    ) {
        use crate::mir::place::{Operand, PlaceKind, ProjectionElem, Rvalue};
        for bb in &mir.basic_blocks {
            for stmt in &bb.statements {
                if let StatementKind::Assign(boxed) = &stmt.kind {
                    let (place, rvalue) = &**boxed;
                    if let PlaceKind::Local(dest_id) = &place.kind {
                        if let Rvalue::Use(op) = rvalue {
                            let lv = match op {
                                Operand::Copy(lv) | Operand::Move(lv) => lv,
                                _ => continue,
                            };
                            if let PlaceKind::Projection(base, ProjectionElem::Field(field_id, _)) =
                                &lv.kind
                            {
                                let base_ty = self.resolve_place_for_writeback(mir, base);
                                if let TyKind::Adt(def_id, substs) = &base_ty.kind {
                                    if let Some(fields) = table.struct_fields(def_id) {
                                        if let Some(field_ty) = fields.get(field_id.0 as usize) {
                                            if let Some(dest_local) =
                                                mir.local_decls.get_mut(dest_id.0 as usize)
                                            {
                                                // Stage 18.380 (v0.5+ Phase 1 step 2):
                                                // Apply substitute() when writing
                                                // field_ty to dest_local.ty.
                                                //
                                                // Was: `dest_local.ty = field_ty.clone()`
                                                // which overwrote Phase 0 + Phase 3.5
                                                // step 1's substitute() result with
                                                // unsubstituted `Param(N)` from
                                                // FieldTyTable. This caused the 4
                                                // test failures observed in Stage 18.379
                                                // experiment (RawPtr field access).
                                                //
                                                // Fix: If base_ty is `Adt(_, substs)`
                                                // with non-empty substs, apply
                                                // `substitute(field_ty, substs)` before
                                                // writing to dest_local.ty. This is the
                                                // root-cause fix — Phase 3.7's re-writeback
                                                // workaround becomes redundant for this path.
                                                //
                                                // Per §1.0 原則 6 (通解 > 特解): one
                                                // substitute call covers all generic
                                                // struct field loads.
                                                // Per §12 (最优 > 最小): root-cause fix
                                                // at the overwrite site.
                                                // Per §20 (iterative audit): same class
                                                // as Stage 18.357 — FieldTyTable overwrite
                                                // was the root cause.
                                                dest_local.ty = if !substs.is_empty() {
                                                    crate::mir::substitute::substitute(
                                                        field_ty, substs,
                                                    )
                                                } else {
                                                    field_ty.clone()
                                                };
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Stage 18.411→18.413 (v0.5+ Phase 2 L3 step 2): REMOVED
    // `writeback_binaryop_results` — was a workaround for typeck's Shl/Shr
    // arm not checking LHS type. Stage 18.412 added the LHS check in
    // `infer_rvalue`, so typeck now reports `&str << 2` and `() << 2`
    // errors directly. The writeback workaround is no longer needed.
    //
    // Historical documentation (kept for reference, per §1.0 原則 13):
    //
    // This was formerly "Pass 2" of `writeback_field_load_locals_with_table`.
    // It was logically UNRELATED to field access — it handled BinaryOp
    // (arithmetic/shift/bitwise) result types.
    //
    // Stage 18.410 surgical experiments (LANDIN_EXP_NO_PASS1/NO_PASS2)
    // confirmed:
    //   - Disable Pass 1 → 3 failures (field-access paths)
    //   - Disable Pass 2 → 2 failures (Shl on `&str` and `()`)
    //
    // The 2 Pass 2 failures were NOT field-access paths. They were caused
    // by typeck's `infer_rvalue` Shl/Shr arm not checking the LHS type.
    // For `&str << 2`, the Shl arm returned `a_ty = &str` (no error), and
    // Pass 2 overwrote dest to `i32` (from b_ty), which then caused a
    // codegen type mismatch (i32 alloca vs &str operand).
    //
    // Root-cause fix (Stage 18.412): Add LHS type check in Shl/Shr arm.
    // Per §1.6 终极检验: typeck catches the error at the right layer.
    // Per §1.0 原則 4 (报错 > 静默): error reported explicitly at typeck.
    // Per §1.0 原則 5 (去除兼容思维): workaround fully removed.
    // Per §1.0 原則 6 (通解 > 特解): one LHS check replaces writeback.

    // Stage 18.413 (v0.5+ Phase 2 L3 step 2): REMOVED
    // `resolve_operand_for_writeback` — was only called by the removed
    // `writeback_binaryop_results` (Pass 2). Per §1.0 原則 5 (去除兼容思维):
    // unused code removed.
    //
    // Historical note: was a thin wrapper around
    // `resolve_place_for_writeback` for the Operand::Copy/Move/Constant
    // cases. Pass 1 (writeback_field_load_locals_with_table) calls
    // `resolve_place_for_writeback` directly (no Operand wrapper needed
    // because Pass 1 already extracts the place from the Operand).

    /// Resolve a place's type for the writeback pass (post-Phase 3, so
    /// local_decls have resolved types).
    pub(super) fn resolve_place_for_writeback(
        &self,
        mir: &MirBody,
        lv: &crate::mir::place::Place,
    ) -> Ty {
        use crate::mir::place::PlaceKind;
        match &lv.kind {
            PlaceKind::Local(id) => mir
                .local_decls
                .get(id.0 as usize)
                .map(|ld| ld.ty.clone())
                .unwrap_or_else(|| Ty::new(TyKind::Error, lv.span)),
            PlaceKind::Projection(base, elem) => {
                let base_ty = self.resolve_place_for_writeback(mir, base);
                match elem {
                    crate::mir::place::ProjectionElem::Field(_, field_ty) => {
                        // Stage 18.399 (v0.5+ Phase 2 step 4): Apply substitute
                        // when field_ty contains Param and base_ty is Adt with substs.
                        // This mirrors resolve_field_ty_with_substs in codegen
                        // (Stage 18.384) and resolve_place_type_with_table
                        // (Stage 18.358).
                        //
                        // Was: `field_ty.clone()` — returned unsubstituted Param
                        // from MIR ProjectionElem::Field.
                        //
                        // Per §1.0 原則 6 (通解 > 特解): one substitute path.
                        // Per §12 (最优 > 最小): root-cause fix at resolution site.
                        if let TyKind::Adt(_, substs) = &base_ty.kind {
                            if !substs.is_empty() {
                                return crate::mir::substitute::substitute(field_ty, substs);
                            }
                        }
                        field_ty.clone()
                    }
                    _ => base_ty,
                }
            }
            PlaceKind::Static(_) => Ty::new(TyKind::Error, lv.span),
        }
    }
}

/// Stage 18.376 (TD-ARCH-NESTED-GENERIC-FIELD-ACCESS): Local helper for
/// `writeback_field_types_in_rvalue_with_table` to check if a Ty (recursively)
/// contains any `Param`. Mirrors `type_contains_param` in `mir/lower/writeback.rs`.
///
/// Used to decide whether to apply `substitute(field_ty, adt_substs)` for
/// unsubstituted generic placeholders in `AggregateKind::Adt` field_tys.
///
/// Per §1.0 原則 3 (显式 > 隐式): explicit predicate makes the
/// "needs substitution" check visible at the callsite.
/// Per §16: pure MIR data predicate, no HIR access.
/// Stage 18.388 (v0.5+ Phase 3 step 5): Preserved as dead code.
/// Was used by `writeback_field_types_with_table` (Phase 3.5 step 1, now removed).
#[allow(dead_code)]
fn typeck_type_contains_param(ty: &Ty) -> bool {
    match &ty.kind {
        TyKind::Param(_) => true,
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) | TyKind::Slice(inner) => {
            typeck_type_contains_param(inner)
        }
        TyKind::Array(elem, _) => typeck_type_contains_param(elem),
        TyKind::Tuple(tys) => tys.iter().any(typeck_type_contains_param),
        TyKind::Adt(_, substs) => substs.iter().any(typeck_type_contains_param),
        TyKind::Closure(_, substs) => substs.iter().any(typeck_type_contains_param),
        TyKind::FnDef(_, substs) => substs.iter().any(typeck_type_contains_param),
        _ => false,
    }
}
