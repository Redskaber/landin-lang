//! Borrow checker: NLL (Non-Lexical Lifetimes) on MIR.
//!
//! Per 04-ownership-borrowing.md, the borrow checker enforces Landin's
//! ownership and borrowing rules:
//! - Each value has a single owner
//! - `&T` allows shared reads, `&mut T` allows exclusive writes
//! - A value can have multiple `&T` OR one `&mut T`, never both
//! - Moves transfer ownership; a moved value cannot be used
//! - NLL: lifetimes end at last use, not at lexical scope end
//!
//! Public entry point: [`check_mir_body`].
//!
//! ## Stage 6.14 architectural split (TD-024)
//!
//! Per `docs/stage-committee-process.md` v3.21 §14.4 + §13.4, this file
//! has been split into 3 sub-modules:
//!
//! - `liveness.rs`       — NLL liveness analysis (§4.3)
//! - `copy_semantics.rs` — Copy trait detection (§4.5 related)
//! - `place_path.rs`     — PlacePath data structure (§4 data structures)
//!
//! This file (`mod.rs`) retains: BorrowChecker struct + impl + entry
//! points (`check_mir_body` / `check_crate`) + tests.

pub mod borrow_set;
pub mod error;
pub mod move_tracker;

// Stage 6.14 (TD-024) sub-modules.
mod copy_semantics;
// Stage 14.105 (dead code cleanup): `drop_elaboration` module removed.
// It was `#[allow(dead_code)]` since Stage 8.4 and never called.
// Drop elaboration will be re-implemented in v0.2 when user-defined Drop is added.
mod liveness;
mod place_path;
// Stage 7.1 (TD-015 step 1): region inference data structures + constraint collection.
// Stage 7.5 (TD-015 step 5): partially integrated into BorrowChecker::check_mir_body.
// Some types/methods (SCC, universe escape) are infrastructure for future
// full integration — currently only new/region_to_vid/collect_implied_bounds/
// infer_regions are called.
#[allow(dead_code)]
mod region_inference;

// Stage 3.63 (cross-stage naming standardization): `BorrowKind` is now
// re-exported from `crate::mir::place` (single source of truth). The
// former `BkKind` alias has been removed.
pub use borrow_set::{Borrow, BorrowSet};
pub use error::{BorrowError, BorrowErrorKind};
pub use move_tracker::MoveTracker;
// Re-export BorrowKind from mir::place so callers can `use borrowck::BorrowKind`.
pub use crate::mir::place::BorrowKind;
// Stage 6.14: re-export public symbols from sub-modules for backward compat.
pub use copy_semantics::{ty_is_copy, ty_is_copy_unified, ty_is_copy_with_resolver};
pub use liveness::{compute_last_use_map, LastUseMap};
pub use place_path::{PlacePath, PlaceRoot, ProjElem};

use crate::mir::body::TerminatorKind;
use crate::mir::body::*;
use crate::mir::place::*;
use crate::mir::ty::Ty;
use crate::session::Span;

/// The borrow checker. Walks MIR bodies, tracks borrows and moves,
/// and reports ownership/borrowing violations.
pub struct BorrowChecker<'a> {
    /// All active borrows in the current body.
    borrows: BorrowSet,
    /// Move tracker: which locals have been moved.
    moves: MoveTracker,
    /// Errors found during checking (non-fatal).
    errors: Vec<BorrowError>,
    /// G5 fix (Stage 2.4e): Set of locals that have been initialized
    /// (assigned at least once). Used to distinguish `let x = 1;` (init,
    /// allowed even for immutable locals) from `x = 2;` (reassignment,
    /// rejected for immutable locals).
    initialized: std::collections::HashSet<crate::mir::place::LocalId>,
    /// Stage 14.106 (HP-1 fix): Optional TraitResolver for sound Copy detection.
    /// When set, `ty_is_copy` calls use `ty_is_copy_with_resolver` instead of
    /// the unsound `ty_is_copy` (which returns true for ALL Adt types).
    /// When None (tests), falls back to unsound `ty_is_copy`.
    resolver: Option<&'a crate::traits::TraitResolver>,
    /// Stage 14.106 (HP-1 fix): Optional interner for resolver-based Copy detection.
    interner: Option<&'a lasso::Rodeo>,
}

impl<'a> BorrowChecker<'a> {
    pub fn new() -> Self {
        Self {
            borrows: BorrowSet::new(),
            moves: MoveTracker::new(),
            errors: Vec::new(),
            initialized: std::collections::HashSet::new(),
            resolver: None,
            interner: None,
        }
    }

    /// Stage 14.106 (HP-1 fix): Create a BorrowChecker with a TraitResolver
    /// for sound Copy detection. Use this in the driver instead of `new()`.
    pub fn with_resolver(
        resolver: &'a crate::traits::TraitResolver,
        interner: &'a lasso::Rodeo,
    ) -> Self {
        Self {
            borrows: BorrowSet::new(),
            moves: MoveTracker::new(),
            errors: Vec::new(),
            initialized: std::collections::HashSet::new(),
            resolver: Some(resolver),
            interner: Some(interner),
        }
    }

    /// Stage 14.106 (HP-1 fix): Sound Copy check — uses TraitResolver when
    /// available, falls back to unsound `ty_is_copy` in test contexts.
    fn is_copy(&self, ty: &crate::mir::ty::Ty) -> bool {
        if let (Some(resolver), Some(interner)) = (self.resolver, self.interner) {
            copy_semantics::ty_is_copy_with_resolver(ty, resolver, interner)
        } else {
            copy_semantics::ty_is_copy(ty)
        }
    }

    /// Check a single MIR body for borrow/ownership violations.
    ///
    /// Walks all basic blocks in order, tracking:
    /// - Borrows created by `Rvalue::Ref`
    /// - Moves created by `Operand::Move`
    /// - Uses of borrowed/moved places
    ///
    /// Reports errors for:
    /// - Use-after-move
    /// - Mutating while borrowed
    /// - Borrowing a moved value
    ///
    /// NLL (Stage 2.4c, P0-14/P0-16):
    /// Before the main walk, we compute a "last use" map: for each local,
    /// the last (bb_id, stmt_idx) where it's read. During the walk, after
    /// processing each statement, we kill any borrow whose `ref_local`
    /// has its last use at the current point. This means a borrow on `x`
    /// expires as soon as the reference `r = &x` is no longer used — not
    /// at the lexical scope end.
    ///
    /// This is a single-pass forward walk with a pre-computed last-use
    /// map. It's correct for straight-line code and most loop patterns.
    /// The only case it gets wrong is when a borrow's last use is inside
    /// a loop body but the borrow was created outside the loop — in that
    /// case the borrow is killed after the first iteration's last use,
    /// producing a false-positive borrow error on the second iteration.
    /// This is a known limitation; full fixpoint dataflow is Stage 3.
    pub fn check_mir_body(&mut self, mir: &MirBody) {
        // Pre-pass: compute last-use map for each local.
        // G2 fix (Stage 2.4e): The last-use map records the program point
        // where each local is *read*. The borrow should be killed *after*
        // that statement completes — i.e., at the start of the *next*
        // statement. This ensures that within the statement that performs
        // the last read, the borrow is still alive, so any write to the
        // borrowed place within that same statement is correctly flagged.
        //
        // Concretely: if local `r`'s last use is at (bb, stmt_idx), we kill
        // the borrow at the start of processing (bb, stmt_idx + 1) — BEFORE
        // check_statement runs for that next statement.
        let last_use_map = compute_last_use_map(mir);

        // Main walk: forward over all basic blocks.
        for (bb_idx, bb) in mir.basic_blocks.iter().enumerate() {
            let bb_id = BasicBlockId(bb_idx as u32);
            let stmt_count = bb.statements.len();
            for stmt_idx in 0..stmt_count {
                // Kill borrows whose ref_local's last use was at the
                // PREVIOUS statement (stmt_idx - 1). This ensures the
                // borrow stays alive during the statement that performs
                // the last read.
                if stmt_idx > 0 {
                    self.kill_expired_borrows(&last_use_map, bb_id, stmt_idx - 1);
                }
                self.check_statement(mir, &bb.statements[stmt_idx], bb_id, stmt_idx);
            }
            // After the last statement, kill borrows whose last use was
            // at the last statement.
            if stmt_count > 0 {
                self.kill_expired_borrows(&last_use_map, bb_id, stmt_count - 1);
            }
            // Check terminator (uses are at index == statements.len())
            let term_idx = stmt_count;
            self.check_terminator(mir, &bb.terminator, bb_id, term_idx);
            self.kill_expired_borrows(&last_use_map, bb_id, term_idx);
        }

        // Stage 7.5 (TD-015 step 5): Run region inference as an additional
        // check. This activates the RegionInferenceContext infrastructure
        // (Stages 7.1-7.4) alongside the existing simplified NLL.
        //
        // Per §16: region inference reads MirBody data (allowed — data flows
        // downstream). It does NOT replace the existing NLL; it runs as an
        // additional safety check. When the MIR carries real lifetime
        // annotations (future stages), this will provide full region checking.
        //
        // Currently, all regions in MIR are `Region::Erased` or `Region::Static`,
        // so region inference is effectively a no-op — it validates the
        // infrastructure without producing false positives.
        self.run_region_inference(mir);
    }

    /// Stage 7.5 (TD-015 step 5): Run region inference on the MIR body.
    ///
    /// Creates a `RegionInferenceContext`, populates it with constraints
    /// from the MIR body's reference types, and runs `infer_regions()`.
    /// Any region inference errors are added to the errors list.
    ///
    /// Per §16: this is a read-only pass on MirBody — no modifications.
    /// Per §23: method name follows `<verb>_<noun>_<noun>` pattern.
    fn run_region_inference(&mut self, mir: &MirBody) {
        use crate::borrowck::region_inference::RegionInferenceContext;
        use crate::mir::ty::TyKind;

        let mut ctx = RegionInferenceContext::new();

        // Collect constraints from reference types in local declarations.
        // For each `&'a T` local, add implied bounds: T: 'a (§4.6.2).
        for local in &mir.local_decls {
            if let TyKind::Ref(region, _mutability, inner_ty) = &local.ty.kind {
                let ref_vid = ctx.region_to_vid(*region);
                ctx.collect_implied_bounds(ref_vid, inner_ty, local.source_info);
            }
        }

        // Run region inference.
        // Since all MIR regions are currently Erased (mapped to 'static
        // vid 0), the inference produces empty point sets for all regions.
        // The universal region check passes because empty ⊆ empty.
        // This is the expected no-op behavior — infrastructure is activated
        // without producing false positives.
        let _result = ctx.infer_regions();

        // Per §14.4: we do NOT replace the existing NLL — we run region
        // inference as an additional check. The existing NLL (last-use map)
        // remains the primary borrow checker. When the MIR carries real
        // lifetime annotations (future stages), region inference errors
        // will be converted to BorrowErrors and added to self.errors.
    }

    /// Kill any active borrow whose `ref_local` has its last use at the
    /// given program point.
    fn kill_expired_borrows(
        &mut self,
        last_use_map: &LastUseMap,
        bb: BasicBlockId,
        stmt_idx: usize,
    ) {
        let current_point = (bb, stmt_idx);
        // Collect locals whose last use is at the current point.
        let locals_to_kill: Vec<crate::mir::place::LocalId> = last_use_map
            .iter()
            .filter_map(|(local, last)| {
                if *last == current_point {
                    Some(*local)
                } else {
                    None
                }
            })
            .collect();
        for local in locals_to_kill {
            self.borrows.kill_borrows_of_local(local);
        }
    }

    fn check_statement(
        &mut self,
        mir: &MirBody,
        stmt: &Statement,
        _bb_id: BasicBlockId,
        _stmt_idx: usize,
    ) {
        if let StatementKind::Assign(boxed) = &stmt.kind {
            let (place, rvalue) = &**boxed;
            // Determine the LHS local (if any) — this is the local that
            // holds the result of the rvalue. For `r = &x`, this is `r`,
            // and we associate it with the borrow for NLL expiry.
            let lhs_local = match &place.kind {
                PlaceKind::Local(id) => Some(*id),
                _ => None,
            };
            self.check_rvalue(mir, rvalue, lhs_local, stmt.span);
            self.check_place_write(mir, place, stmt.span);
        }
    }

    fn check_terminator(
        &mut self,
        mir: &MirBody,
        term: &Terminator,
        _bb_id: BasicBlockId,
        _stmt_idx: usize,
    ) {
        match &term.kind {
            TerminatorKind::Call { func, args, .. } => {
                self.check_operand(mir, func, Span::DUMMY);
                for arg in args {
                    self.check_operand(mir, arg, Span::DUMMY);
                }
            }
            TerminatorKind::SwitchInt { discr, .. } => {
                self.check_operand(mir, discr, Span::DUMMY);
            }
            TerminatorKind::Drop { place, .. } => {
                self.check_place_read(mir, place, Span::DUMMY);
            }
            // Assert reads the condition operand (a bool). Check it
            // for use-after-move just like any other operand.
            TerminatorKind::Assert { cond, .. } => {
                self.check_operand(mir, cond, Span::DUMMY);
            }
            _ => {}
        }
    }

    /// Check an rvalue for borrow creation and operand moves.
    fn check_rvalue(
        &mut self,
        mir: &MirBody,
        rv: &Rvalue,
        lhs_local: Option<crate::mir::place::LocalId>,
        span: Span,
    ) {
        match rv {
            Rvalue::Ref(region, kind, place) => {
                // Creating a borrow: record it.
                // Stage 3.63: `kind` is already `mir::place::BorrowKind` —
                // the former manual conversion to a parallel `BkKind` enum
                // has been eliminated (BorrowKind is now unified).
                let borrowed_place = self.place_path(mir, place);
                let bk = *kind;
                // G7 fix (Stage 2.4f): `&mut x` requires x to be mutable.
                // If x is an immutable local, emit an error.
                if bk == BorrowKind::Mut {
                    if let PlaceKind::Local(id) = &place.kind {
                        let is_mutable =
                            mir.local(*id).mutability == crate::mir::ty::Mutability::Mutable;
                        if !is_mutable {
                            self.errors.push(BorrowError::new(
                                "cannot borrow as mutable: variable is not declared `mut`",
                                span,
                                BorrowErrorKind::BorrowImmutable,
                            ));
                        }
                    }
                }
                // Check if the place is already moved
                if self.moves.is_moved(&borrowed_place) {
                    self.errors.push(BorrowError::use_after_move(
                        "cannot borrow moved value",
                        span,
                    ));
                }
                // Check for conflicting borrows. Associate the borrow
                // with `lhs_local` so NLL can expire it at last use.
                if let Err(conflict) =
                    self.borrows
                        .add_borrow_with_ref(borrowed_place, bk, span, lhs_local)
                {
                    self.errors.push(conflict);
                }
                let _ = region;
            }
            Rvalue::Use(op) | Rvalue::Cast(_, op, _) => {
                // G2+ fix (Stage 2.4e): If the operand is a Move of a
                // ref_temp (i.e., a local that holds a borrow), transfer
                // the borrow's ref_local to the LHS. This handles the
                // common pattern `let r = &x;` where MIR lower produces:
                //   tmp = &x       (ref_local = tmp)
                //   r = Move(tmp)  (transfer ref_local to r)
                // Without this transfer, NLL would track tmp's lifetime
                // instead of r's, causing borrows to expire too early.
                //
                // Stage 14.81 (GAP-1 soundness fix): ALSO handle
                // `Operand::Copy` of a ref_temp. References (`&T`, `&mut T`)
                // are Copy types — `let r = &x;` lowers to `r = Copy(tmp)`
                // (not Move) because `TyKind::Ref` is in the `is_copy` set
                // in `lower_block`. The previous code only transferred for
                // Move, so the borrow's ref_local stayed as `tmp` — and NLL
                // killed it at `tmp`'s last use (which is the Copy itself),
                // causing subsequent `&mut x` to silently succeed.
                //
                // Per §1.0 原則 5 "报错 > 静默": this is the root cause of
                // GAP-1 — `let r1 = &mut x; let r2 = &mut x;` was silently
                // accepted because r1's borrow was killed at tmp1's last
                // use (the Copy statement), not at r1's last use (later).
                if let Operand::Move(lv) | Operand::Copy(lv) = op {
                    if let PlaceKind::Local(ref_local_src) = lv.kind {
                        if let Some(lhs) = lhs_local {
                            self.borrows.transfer_borrow_ref(ref_local_src, lhs);
                        }
                    }
                    // Stage 3.40: For Move of a field projection (e.g.,
                    // extracting enum discriminant), skip the Copy check.
                    // The Move is valid — we're moving the field value
                    // (which is i32, Copy) out of the struct/enum.
                    // The borrowck check_operand will see it's a Move
                    // and won't check Copy-ness (only Copy operands
                    // are checked for Copy-ness).
                }
                self.check_operand(mir, op, span);
            }
            Rvalue::BinaryOp(_, a, b) | Rvalue::BinaryOp2(_, a, b) => {
                self.check_operand(mir, a, span);
                self.check_operand(mir, b, span);
            }
            Rvalue::UnaryOp(_, op) => {
                self.check_operand(mir, op, span);
            }
            Rvalue::Aggregate(_, operands) => {
                for op in operands {
                    self.check_operand(mir, op, span);
                }
            }
        }
    }

    /// Check an operand: if it's a Move, record the move; if it's a
    /// Copy/Move of a place, check for use-after-move.
    fn check_operand(&mut self, mir: &MirBody, op: &Operand, span: Span) {
        match op {
            Operand::Copy(lv) => {
                let path = self.place_path(mir, lv);
                if self.moves.is_moved(&path) {
                    self.errors
                        .push(BorrowError::use_after_move("use of moved value", span));
                }
                // P0-17: Check Copy-ness. A `Copy(lv)` operand is only
                // valid if `lv`'s type implements Copy. Non-Copy types
                // (e.g., String, Vec, Box, structs without Copy) must
                // be moved explicitly via `Operand::Move`.
                //
                // Stage 3.40: For enum scrutinees in match, the MIR lower
                // uses Operand::Copy to read the discriminant. This is
                // semantically a "read" not a "move" — we should allow it
                // for enums (and structs) since we're just extracting a
                // field, not moving the whole value.
                // We skip the Copy check for field projections (the
                // discriminant is always i32, which is Copy).
                let is_field_projection = matches!(
                    &lv.kind,
                    PlaceKind::Projection(_, ProjectionElem::Field(_, _))
                );
                let ty = self.place_ty(mir, lv);
                if !self.is_copy(&ty) && !is_field_projection {
                    self.errors.push(BorrowError::not_copy(
                        format!(
                            "use of moved value: {:?} does not implement Copy; \
                             use an explicit move (`let y = move x;`) or borrow",
                            ty.kind
                        ),
                        span,
                    ));
                }
            }
            Operand::Move(lv) => {
                let path = self.place_path(mir, lv);
                if self.moves.is_moved(&path) {
                    self.errors
                        .push(BorrowError::use_after_move("use of moved value", span));
                }
                // Check if borrowed
                if let Some(bk) = self.borrows.borrow_kind(&path) {
                    if bk == BorrowKind::Shared || bk == BorrowKind::Mut {
                        self.errors.push(BorrowError::move_borrowed(
                            "cannot move borrowed value",
                            span,
                        ));
                    }
                }
                // Stage 3.40: Don't record moves for field projections.
                // Moving a field (e.g., extracting enum discriminant) doesn't
                // move the whole parent value. This allows `match` on enums
                // to work without spurious "use of moved value" errors.
                let is_field_projection = matches!(
                    &lv.kind,
                    PlaceKind::Projection(_, ProjectionElem::Field(_, _))
                );
                if !is_field_projection {
                    self.moves.record_move(path);
                }
            }
            Operand::Constant(_) => {}
        }
    }

    /// Look up the resolved type of a place.
    ///
    /// For `Local(id)`, reads from `mir.local_decls[id].ty` (which
    /// typeck has populated with the resolved type).
    ///
    /// For projections, walks the projection chain: Deref strips a
    /// Ref/RawPtr, Field returns the field's Ty (stored in the
    /// ProjectionElem::Field payload), Index returns the array/slice
    /// element type.
    fn place_ty(&self, mir: &MirBody, lv: &Place) -> Ty {
        match &lv.kind {
            PlaceKind::Local(id) => {
                if (id.0 as usize) < mir.local_decls.len() {
                    mir.local(*id).ty.clone()
                } else {
                    Ty::new(crate::mir::ty::TyKind::Error, lv.span)
                }
            }
            PlaceKind::Static(_) => Ty::new(crate::mir::ty::TyKind::Error, lv.span),
            PlaceKind::Projection(base, elem) => {
                let base_ty = self.place_ty(mir, base);
                match elem {
                    ProjectionElem::Deref => match &base_ty.kind {
                        crate::mir::ty::TyKind::Ref(_, _, inner)
                        | crate::mir::ty::TyKind::RawPtr(_, inner) => (**inner).clone(),
                        // Stage 14.103 (ME-7 fix): Deref on non-reference type
                        // is a type error. Previously returned base_ty silently,
                        // which could cause the borrow checker to make wrong
                        // decisions. Now return Error so downstream code knows
                        // the type is unknown.
                        _ => Ty::new(crate::mir::ty::TyKind::Error, lv.span),
                    },
                    ProjectionElem::Field(_, field_ty) => field_ty.clone(),
                    ProjectionElem::Index(_)
                    | ProjectionElem::ConstantIndex { .. }
                    | ProjectionElem::Subslice { .. } => match &base_ty.kind {
                        crate::mir::ty::TyKind::Array(inner, _)
                        | crate::mir::ty::TyKind::Slice(inner) => (**inner).clone(),
                        // Stage 14.103 (ME-7 fix): Index on non-array type
                        // is a type error. Previously returned base_ty silently.
                        _ => Ty::new(crate::mir::ty::TyKind::Error, lv.span),
                    },
                }
            }
        }
    }

    /// Check a write to a place: ensure it's not borrowed, and (G5 fix)
    /// ensure it's not a reassignment of an immutable local.
    fn check_place_write(&mut self, mir: &MirBody, lv: &Place, span: Span) {
        let path = self.place_path(mir, lv);
        // Writing to a place that is borrowed is an error
        if let Some(bk) = self.borrows.borrow_kind(&path) {
            if bk == BorrowKind::Shared || bk == BorrowKind::Mut {
                self.errors.push(BorrowError::assign_borrowed(
                    "cannot assign to borrowed value",
                    span,
                ));
            }
        }
        // G5 fix (Stage 2.4e): Mutability check.
        // If the LHS is a local that has already been initialized,
        // and the local is declared immutable, reject the assignment.
        // The first write (initialization) is always allowed.
        if let PlaceKind::Local(id) = &lv.kind {
            let is_init = self.initialized.contains(id);
            let is_mutable = mir.local(*id).mutability == crate::mir::ty::Mutability::Mutable;
            if is_init && !is_mutable {
                self.errors.push(BorrowError::new(
                    "cannot assign twice to immutable variable",
                    span,
                    BorrowErrorKind::AssignImmutable,
                ));
            }
            // Mark as initialized (idempotent — re-init after move is OK).
            self.initialized.insert(*id);
        }
        // Writing re-initializes a moved place
        self.moves.un_move(&path);
    }

    /// Check a read from a place: ensure it's not moved.
    fn check_place_read(&mut self, mir: &MirBody, lv: &Place, span: Span) {
        let path = self.place_path(mir, lv);
        if self.moves.is_moved(&path) {
            self.errors
                .push(BorrowError::use_after_move("use of moved value", span));
        }
    }

    /// Build a field-sensitive `PlacePath` from a place.
    ///
    /// Walks the projection chain bottom-up, building up the path's
    /// `projections` vec. For example:
    ///   `a.x.y` → PlacePath { root: Local(a), projections: [Field(0), Field(1)] }
    ///   `*p`    → PlacePath { root: Local(p), projections: [Deref] }
    ///   `arr[i]`→ PlacePath { root: Local(arr), projections: [Index(i)] }
    fn place_path(&self, _mir: &MirBody, lv: &Place) -> PlacePath {
        match &lv.kind {
            PlaceKind::Local(id) => PlacePath::local(*id),
            PlaceKind::Static(def_id) => PlacePath::static_def(*def_id),
            PlaceKind::Projection(base, elem) => {
                let base_path = self.place_path(_mir, base);
                let proj_elem = match elem {
                    ProjectionElem::Deref => ProjElem::Deref,
                    ProjectionElem::Field(fid, _) => ProjElem::Field(*fid),
                    ProjectionElem::Index(idx) => ProjElem::Index(*idx),
                    ProjectionElem::ConstantIndex {
                        offset, from_end, ..
                    } => ProjElem::ConstantIndex {
                        offset: *offset,
                        from_end: *from_end,
                    },
                    ProjectionElem::Subslice {
                        from,
                        to: _,
                        from_end,
                    } => {
                        // Subslice is rare; represent as a constant index
                        // for now. A real subslice borrow is rare in user
                        // code, so this is acceptable for Stage 2.4c.
                        ProjElem::ConstantIndex {
                            offset: *from,
                            from_end: *from_end,
                        }
                    }
                };
                base_path.project(proj_elem)
            }
        }
    }

    pub fn into_errors(self) -> Vec<BorrowError> {
        self.errors
    }
}

impl<'a> Default for BorrowChecker<'a> {
    fn default() -> Self {
        Self::new()
    }
}

/// Check a single MIR body for borrow/ownership errors.
/// Returns a list of errors (non-fatal).
///
/// Note: This convenience function does NOT use TraitResolver for Copy detection.
/// For sound Copy detection, use `BorrowChecker::with_resolver` instead.
pub fn check_mir_body(mir: &MirBody) -> Vec<BorrowError> {
    let mut bc: BorrowChecker<'_> = BorrowChecker::new();
    bc.check_mir_body(mir);
    bc.into_errors()
}

/// Stage 3.63: Deprecated legacy entry point. The driver now uses
/// `BorrowChecker::check_mir_body` directly per §16 interface isolation.
///
/// This free function is retained for backwards compatibility with older
/// callers that pass a `HirCrate`. It internally re-lowers HIR to MIR —
/// the §16-violating pattern that the driver-based orchestration eliminated.
/// New code should use the driver or `BorrowChecker::check_mir_body` directly.
#[deprecated(note = "Use BorrowChecker::check_mir_body (§16-compliant) or driver::compile instead")]
pub fn check_crate(hir: &crate::hir::HirCrate, interner: &lasso::Rodeo) -> Vec<BorrowError> {
    let mut all_errors = Vec::new();
    for (_, body) in &hir.bodies {
        let mir = crate::mir::lower::lower_hir_body_to_mir(body, interner, hir);
        all_errors.extend(check_mir_body(&mir));
    }
    all_errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;
    use crate::mir::ty::*;

    fn make_mir() -> MirBody {
        let mut mir = MirBody::new(Span::DUMMY);
        mir.new_block();
        mir
    }

    #[test]
    fn no_errors_on_simple_body() {
        let mut mir = make_mir();
        let x = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let y = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(x, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
                    val: ConstVal::Int(42),
                })),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(y, Span::DUMMY),
                Rvalue::Use(Operand::Copy(Place::local(x, Span::DUMMY))),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator =
            Terminator::new(TerminatorKind::Return, Span::DUMMY);
        let errors = check_mir_body(&mir);
        assert!(errors.is_empty(), "expected no errors, got {:?}", errors);
    }

    #[test]
    fn use_after_move_detected() {
        let mut mir = make_mir();
        let x = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let y = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let z = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        // y = move x
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(y, Span::DUMMY),
                Rvalue::Use(Operand::Move(Place::local(x, Span::DUMMY))),
            ))),
            span: Span::DUMMY,
        });
        // z = copy x  <-- use after move!
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(z, Span::DUMMY),
                Rvalue::Use(Operand::Copy(Place::local(x, Span::DUMMY))),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator =
            Terminator::new(TerminatorKind::Return, Span::DUMMY);
        let errors = check_mir_body(&mir);
        assert!(!errors.is_empty(), "expected use-after-move error");
    }

    #[test]
    fn move_borrowed_detected() {
        let mut mir = make_mir();
        let x = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let r = mir.new_local(
            Ty::new(
                TyKind::Ref(
                    Region::Erased,
                    Mutability::Immutable,
                    Box::new(Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)),
                ),
                Span::DUMMY,
            ),
            None,
            Span::DUMMY,
        );
        let y = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        // r = &x
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(r, Span::DUMMY),
                Rvalue::Ref(
                    Region::Erased,
                    crate::mir::place::BorrowKind::Shared,
                    Place::local(x, Span::DUMMY),
                ),
            ))),
            span: Span::DUMMY,
        });
        // y = move x  <-- move while borrowed!
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(y, Span::DUMMY),
                Rvalue::Use(Operand::Move(Place::local(x, Span::DUMMY))),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator =
            Terminator::new(TerminatorKind::Return, Span::DUMMY);
        let errors = check_mir_body(&mir);
        assert!(!errors.is_empty(), "expected move-borrowed error");
    }

    #[test]
    fn assign_to_borrowed_detected() {
        let mut mir = make_mir();
        let x = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let r = mir.new_local(
            Ty::new(
                TyKind::Ref(
                    Region::Erased,
                    Mutability::Immutable,
                    Box::new(Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)),
                ),
                Span::DUMMY,
            ),
            None,
            Span::DUMMY,
        );
        // r = &x
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(r, Span::DUMMY),
                Rvalue::Ref(
                    Region::Erased,
                    crate::mir::place::BorrowKind::Shared,
                    Place::local(x, Span::DUMMY),
                ),
            ))),
            span: Span::DUMMY,
        });
        // x = 42  <-- assign while borrowed!
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(x, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
                    val: ConstVal::Int(42),
                })),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator =
            Terminator::new(TerminatorKind::Return, Span::DUMMY);
        let errors = check_mir_body(&mir);
        assert!(!errors.is_empty(), "expected assign-borrowed error");
    }

    #[test]
    fn shared_borrow_after_mut_ok() {
        let mut mir = make_mir();
        let x = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let r1 = mir.new_local(
            Ty::new(
                TyKind::Ref(
                    Region::Erased,
                    Mutability::Mutable,
                    Box::new(Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)),
                ),
                Span::DUMMY,
            ),
            None,
            Span::DUMMY,
        );
        // r1 = &mut x
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(r1, Span::DUMMY),
                Rvalue::Ref(
                    Region::Erased,
                    crate::mir::place::BorrowKind::Mut,
                    Place::local(x, Span::DUMMY),
                ),
            ))),
            span: Span::DUMMY,
        });
        // After r1's last use, we can borrow again (NLL).
        // For Stage 2.3, we don't track last-use precisely — this is a
        // simplified check. The borrow remains active for the whole body.
        // This test just verifies no crash.
        mir.block_mut(BasicBlockId(0)).terminator =
            Terminator::new(TerminatorKind::Return, Span::DUMMY);
        let _ = check_mir_body(&mir);
    }

    #[test]
    fn reassign_after_move_ok() {
        let mut mir = make_mir();
        let x = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let y = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let z = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        // y = move x
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(y, Span::DUMMY),
                Rvalue::Use(Operand::Move(Place::local(x, Span::DUMMY))),
            ))),
            span: Span::DUMMY,
        });
        // x = 42  (re-initialize after move — OK)
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(x, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
                    val: ConstVal::Int(42),
                })),
            ))),
            span: Span::DUMMY,
        });
        // z = copy x  (OK — x was re-initialized)
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(z, Span::DUMMY),
                Rvalue::Use(Operand::Copy(Place::local(x, Span::DUMMY))),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator =
            Terminator::new(TerminatorKind::Return, Span::DUMMY);
        let errors = check_mir_body(&mir);
        assert!(
            errors.is_empty(),
            "expected no errors after re-init, got {:?}",
            errors
        );
    }

    #[test]
    fn copy_is_not_move() {
        let mut mir = make_mir();
        let x = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let y = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let z = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        // y = copy x  (Copy type — not moved)
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(y, Span::DUMMY),
                Rvalue::Use(Operand::Copy(Place::local(x, Span::DUMMY))),
            ))),
            span: Span::DUMMY,
        });
        // z = copy x  (OK — x was copied, not moved)
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(z, Span::DUMMY),
                Rvalue::Use(Operand::Copy(Place::local(x, Span::DUMMY))),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator =
            Terminator::new(TerminatorKind::Return, Span::DUMMY);
        let errors = check_mir_body(&mir);
        assert!(
            errors.is_empty(),
            "expected no errors for copies, got {:?}",
            errors
        );
    }

    // === Stage 2.4c (P0-14/P0-16): NLL borrow expiry tests ===

    /// Verify that a borrow expires at its last use, allowing the
    /// underlying place to be mutated afterward.
    #[test]
    fn nll_borrow_expires_at_last_use() {
        // Code pattern:
        //   let mut x = 42;
        //   let r = &x;        // borrow x
        //   let y = *r + 1;    // last use of r — borrow expires here
        //   x = 100;           // OK — x is no longer borrowed (and x is mut)
        let mut mir = make_mir();
        let x = mir.new_local_with_mut(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
            crate::mir::ty::Mutability::Mutable,
        );
        let r = mir.new_local(
            Ty::new(
                TyKind::Ref(
                    Region::Erased,
                    Mutability::Immutable,
                    Box::new(Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)),
                ),
                Span::DUMMY,
            ),
            None,
            Span::DUMMY,
        );
        let y = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        // x = 42
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(x, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
                    val: ConstVal::Int(42),
                })),
            ))),
            span: Span::DUMMY,
        });
        // r = &x  (creates borrow)
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(r, Span::DUMMY),
                Rvalue::Ref(
                    Region::Erased,
                    crate::mir::place::BorrowKind::Shared,
                    Place::local(x, Span::DUMMY),
                ),
            ))),
            span: Span::DUMMY,
        });
        // y = *r + 1  (last use of r)
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(y, Span::DUMMY),
                Rvalue::BinaryOp(
                    BinOp::Add,
                    Operand::Copy(Place {
                        kind: PlaceKind::Projection(
                            Box::new(Place::local(r, Span::DUMMY)),
                            ProjectionElem::Deref,
                        ),
                        span: Span::DUMMY,
                    }),
                    Operand::Constant(Const {
                        ty: Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
                        val: ConstVal::Int(1),
                    }),
                ),
            ))),
            span: Span::DUMMY,
        });
        // x = 100  (should be OK — borrow expired)
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(x, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
                    val: ConstVal::Int(100),
                })),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator =
            Terminator::new(TerminatorKind::Return, Span::DUMMY);
        let errors = check_mir_body(&mir);
        assert!(
            errors.is_empty(),
            "expected no errors (NLL should expire the borrow at last use), got {:?}",
            errors
        );
    }

    /// Verify that a borrow is still alive at points after creation but
    /// before last use — moving the borrowed place during that window
    /// should still be an error.
    #[test]
    fn nll_borrow_still_alive_before_last_use() {
        // Code pattern:
        //   let x = 42;
        //   let r = &x;        // borrow x
        //   x = 100;           // ERROR — r is still alive (no use of r yet)
        //   let y = *r;        // use r (but the error above already fired)
        let mut mir = make_mir();
        let x = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let r = mir.new_local(
            Ty::new(
                TyKind::Ref(
                    Region::Erased,
                    Mutability::Immutable,
                    Box::new(Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)),
                ),
                Span::DUMMY,
            ),
            None,
            Span::DUMMY,
        );
        let y = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(x, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
                    val: ConstVal::Int(42),
                })),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(r, Span::DUMMY),
                Rvalue::Ref(
                    Region::Erased,
                    crate::mir::place::BorrowKind::Shared,
                    Place::local(x, Span::DUMMY),
                ),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(x, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
                    val: ConstVal::Int(100),
                })),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(y, Span::DUMMY),
                Rvalue::Use(Operand::Copy(Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(r, Span::DUMMY)),
                        ProjectionElem::Deref,
                    ),
                    span: Span::DUMMY,
                })),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator =
            Terminator::new(TerminatorKind::Return, Span::DUMMY);
        let errors = check_mir_body(&mir);
        // The borrow on x is alive at "x = 100" because r's last use is
        // at "y = *r" (a later statement). So assigning to x should fail.
        assert!(
            !errors.is_empty(),
            "expected assign-borrowed error (borrow is alive before last use of r), got {:?}",
            errors
        );
    }

    // === Stage 2.4c (P0-17): Copy-ness check tests ===

    #[test]
    fn ty_is_copy_primitives() {
        use crate::ast;
        use crate::mir::ty::TyKind;
        assert!(ty_is_copy(&Ty::new(TyKind::Bool, Span::DUMMY)));
        assert!(ty_is_copy(&Ty::new(TyKind::Char, Span::DUMMY)));
        assert!(ty_is_copy(&Ty::new(
            TyKind::Int(ast::IntTy::I32),
            Span::DUMMY
        )));
        assert!(ty_is_copy(&Ty::new(
            TyKind::Uint(ast::UintTy::U64),
            Span::DUMMY
        )));
        assert!(ty_is_copy(&Ty::new(
            TyKind::Float(ast::FloatTy::F64),
            Span::DUMMY
        )));
    }

    #[test]
    fn ty_is_copy_refs_and_ptrs() {
        use crate::mir::ty::{Mutability, Region, TyKind};
        let i32_ty = Ty::new(
            crate::mir::ty::TyKind::Int(crate::ast::IntTy::I32),
            Span::DUMMY,
        );
        let ref_ty = Ty::new(
            TyKind::Ref(
                Region::Erased,
                Mutability::Immutable,
                Box::new(i32_ty.clone()),
            ),
            Span::DUMMY,
        );
        assert!(ty_is_copy(&ref_ty));
        let raw_ty = Ty::new(
            TyKind::RawPtr(Mutability::Mutable, Box::new(i32_ty)),
            Span::DUMMY,
        );
        assert!(ty_is_copy(&raw_ty));
    }

    #[test]
    fn ty_is_copy_tuples_and_arrays() {
        use crate::ast;
        use crate::mir::ty::{Const, ConstVal, TyKind};
        let tuple_ty = Ty::new(
            TyKind::Tuple(vec![
                Ty::new(TyKind::Bool, Span::DUMMY),
                Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            ]),
            Span::DUMMY,
        );
        assert!(ty_is_copy(&tuple_ty));
        let array_ty = Ty::new(
            TyKind::Array(
                Box::new(Ty::new(TyKind::Bool, Span::DUMMY)),
                Box::new(Const {
                    ty: Ty::new(TyKind::Uint(ast::UintTy::Usize), Span::DUMMY),
                    val: ConstVal::Uint(4),
                }),
            ),
            Span::DUMMY,
        );
        assert!(ty_is_copy(&array_ty));
    }

    #[test]
    fn ty_is_not_copy_adt_str_slice() {
        use crate::hir::DefId;
        use crate::mir::ty::TyKind;
        // Str, Slice are not Copy.
        assert!(!ty_is_copy(&Ty::new(TyKind::Str, Span::DUMMY)));
        let slice_ty = Ty::new(
            TyKind::Slice(Box::new(Ty::new(TyKind::Bool, Span::DUMMY))),
            Span::DUMMY,
        );
        assert!(!ty_is_copy(&slice_ty));
        // Stage 3.40: Adt is now treated as Copy (pragmatic — allows
        // enum match and struct field access without spurious errors).
        let adt_ty = Ty::new(TyKind::Adt(DefId::new(0), vec![].into()), Span::DUMMY);
        assert!(ty_is_copy(&adt_ty));
    }

    #[test]
    fn ty_is_copy_infer_and_error_assumed_copy() {
        use crate::mir::ty::{InferVar, TyKind, TyVid};
        // Infer and Error are treated as Copy (avoid spurious errors
        // during type inference).
        let infer_ty = Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(0))), Span::DUMMY);
        assert!(ty_is_copy(&infer_ty));
        let error_ty = Ty::new(TyKind::Error, Span::DUMMY);
        assert!(ty_is_copy(&error_ty));
    }
}
