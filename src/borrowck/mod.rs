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
//! points (`check_mir_body_with_dataflow` canonical, `check_mir_body`
//! free-function convenience wrapper) + tests.

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
// Stage 18.64: ty_is_copy is deprecated but re-exported for test backward compat.
// The #[allow(deprecated)] is necessary because test code imports it.
#[allow(deprecated)]
pub use copy_semantics::ty_is_copy;
pub use copy_semantics::{ty_is_copy_unified, ty_is_copy_with_resolver};
// Stage 15.35 (HP-10): re-export fixpoint liveness analysis API for v0.2 Phase 2.
// The legacy `compute_last_use_map` is retained until Stage 15.37 migration.
// Stage 15.36 (HP-10 step 2): also re-export `compute_live_after_point` —
// the per-statement liveness helper used by `kill_expired_borrows_dataflow`.
// Stage 15.68: `compute_ever_read` and `compute_last_use_map` REMOVED.
// True Rust NLL (Stage 15.67) uses liveness-based kill exclusively.
pub use liveness::{compute_live_after_point, compute_liveness, successors, LiveInMap, LiveOutMap};
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
    /// Stage 15.71: Optional fn_sigs map for region inference constraints.
    /// Maps DefId → Sig for all functions in the crate. Used by
    /// `run_region_inference` to add proper outlives constraints between
    /// call argument regions and parameter regions (instead of the
    /// simplified `'static` constraint).
    fn_sigs: Option<&'a std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>>,
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
            fn_sigs: None,
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
            fn_sigs: None,
        }
    }

    /// Stage 15.71: Create a BorrowChecker with fn_sigs for region inference
    /// constraints, WITHOUT enabling sound Copy detection (resolver stays
    /// None, so `is_copy` falls back to the unsound `ty_is_copy`).
    ///
    /// This is used by the driver to pass fn_sigs for proper call-argument
    /// region constraints while maintaining backward compatibility with
    /// existing tests that expect all Adt types to be Copy.
    ///
    /// Per §23: `with_fn_sigs` follows `<prep>_<noun>_<noun>` pattern.
    pub fn with_fn_sigs(
        fn_sigs: &'a std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
    ) -> Self {
        Self {
            borrows: BorrowSet::new(),
            moves: MoveTracker::new(),
            errors: Vec::new(),
            initialized: std::collections::HashSet::new(),
            resolver: None,
            interner: None,
            fn_sigs: Some(fn_sigs),
        }
    }

    /// Stage 15.99: Create a BorrowChecker with BOTH resolver (for sound
    /// Copy detection) AND fn_sigs (for region inference constraints).
    ///
    /// This is the **preferred constructor** for production use — it
    /// enables both sound Copy detection (HP-1) and proper region
    /// inference constraints (HP-5). The driver should use this instead
    /// of `with_fn_sigs` to close the last unsound simplification.
    ///
    /// Per §23: `with_resolver_and_sigs` follows `<prep>_<noun>_<prep>_<noun>`
    /// pattern.
    /// Per §1.0 原則 9 "正确 > 妥协": sound Copy detection is the correct
    /// approach, not the unsound fallback.
    pub fn with_resolver_and_sigs(
        resolver: &'a crate::traits::TraitResolver,
        interner: &'a lasso::Rodeo,
        fn_sigs: &'a std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
    ) -> Self {
        Self {
            borrows: BorrowSet::new(),
            moves: MoveTracker::new(),
            errors: Vec::new(),
            initialized: std::collections::HashSet::new(),
            resolver: Some(resolver),
            interner: Some(interner),
            fn_sigs: Some(fn_sigs),
        }
    }

    /// Stage 14.106 (HP-1 fix): Sound Copy check — uses TraitResolver when
    /// available, falls back to unsound `ty_is_copy` in test contexts.
    fn is_copy(&self, ty: &crate::mir::ty::Ty) -> bool {
        if let (Some(resolver), Some(interner)) = (self.resolver, self.interner) {
            copy_semantics::ty_is_copy_with_resolver(ty, resolver, interner)
        } else {
            copy_semantics::is_copy_recursive(ty)
        }
    }

    /// Stage 16.82: Format a `Ty` for error messages, using resolver if available.
    ///
    /// When resolver is set, uses `type_to_string_with_resolver` to show
    /// actual type names (e.g., "MyStruct" instead of "<adt>").
    /// Otherwise falls back to `type_to_string`.
    ///
    /// Per §1.0 原則 3 "显式 > 隐式": user-facing type names are explicit.
    /// Per §23: `format_ty` follows `<verb>_<noun>` pattern.
    /// Stage 18.100 (TD-DUP2): delegates to `mir::ty::format_ty_with_optional_resolver`
    /// (single source of truth — was duplicated in 3 modules).
    fn format_ty(&self, ty: &Ty) -> String {
        crate::mir::ty::format_ty_with_optional_resolver(ty, self.resolver, self.interner)
    }

    /// Stage 16.82: Format a `Place` for error messages.
    ///
    /// Returns "local#N" for local places. Full variable name resolution
    /// requires HIR access (deferred to a future stage).
    ///
    /// Per §13.4 J2: single responsibility — place formatting only.
    fn format_place(&self, place: &Place) -> String {
        match &place.kind {
            PlaceKind::Local(id) => format!("local#{}", id.0),
            PlaceKind::Static(def_id) => format!("static#{}", def_id.0),
            PlaceKind::Projection(base, _) => self.format_place(base),
        }
    }

    /// Stage 16.82: Format a `PlacePath` for error messages.
    ///
    /// PlacePath is borrowck's internal place representation. This formats
    /// the root local/static for error messages.
    fn format_place_path(&self, path: &PlacePath) -> String {
        match path.root {
            crate::borrowck::place_path::PlaceRoot::Local(id) => format!("local#{}", id.0),
            crate::borrowck::place_path::PlaceRoot::Static(def_id) => {
                format!("static#{}", def_id.0)
            }
        }
    }

    // Stage 15.72: Removed deprecated `check_mir_body` alias.
    // Use `check_mir_body_with_dataflow` directly — it's the sole entry point.

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

        // Stage 15.50: Collect constraints from MIR statements and terminators.
        // Stage 15.71: Pass fn_sigs for proper call-argument region constraints
        // (instead of the simplified 'static constraint).
        ctx.collect_mir_constraints_with_sigs(mir, self.fn_sigs);

        // Run region inference.
        // Stage 15.49: MIR now has real Region::Var(vid) for each reference.
        // Stage 15.50: constraints collected from MIR statements.
        // Stage 15.51: errors are now converted to BorrowErrors.
        let result = ctx.infer_regions();

        // Stage 15.51: Convert region inference errors to BorrowErrors.
        // Per §1.0 原則 5 "报错 > 静默": errors are reported, not silently
        // ignored. Per §23: BorrowErrorKind::LifetimeError follows the
        // `<Noun>Error` naming convention.
        if let Err(region_errors) = result {
            for err in region_errors {
                let (message, span) = match &err {
                    crate::borrowck::region_inference::RegionInferenceError::RegionEscapesUniversal {
                        escaping_region,
                        universal_region,
                        span,
                        ..
                    } => {
                        (
                            // Stage 15.84: use human-readable region names
                            // (was: {:?} Debug format leaking RegionVid(N)).
                            format!(
                                "lifetime error: region {} escapes universal region {}",
                                crate::mir::ty::region_vid_to_string(*escaping_region),
                                crate::mir::ty::region_vid_to_string(*universal_region),
                            ),
                            // Stage 16.04: use span from constraint cause
                            // (was: Span::DUMMY, producing "1:1").
                            *span,
                        )
                    }
                    crate::borrowck::region_inference::RegionInferenceError::TypeTestFailed {
                        universal_region,
                        ty,
                        span,
                        ..
                    } => {
                        (
                            // Stage 15.84: use human-readable type + region
                            // names (was: {:?} Debug format leaking
                            // TyKind + RegionVid(N)).
                            // Stage 16.82: use resolver-backed type names
                            // (shows "MyStruct" instead of "<adt>").
                            format!(
                                "lifetime error: type {} does not outlive region {}",
                                self.format_ty(ty),
                                crate::mir::ty::region_vid_to_string(*universal_region),
                            ),
                            *span,
                        )
                    }
                };
                self.errors.push(BorrowError::new(
                    &message,
                    span,
                    BorrowErrorKind::LifetimeError,
                ));
            }
        }

        // Per §14.4: we do NOT replace the existing NLL — we run region
        // inference as an additional check. The dataflow borrow checker
        // remains the primary borrow checker. Region inference provides
        // additional lifetime checking.
    }

    // Stage 15.41: The legacy `kill_expired_borrows` method (the single-pass
    // walk version) has been REMOVED. The legacy `check_mir_body` now
    // delegates directly to `check_mir_body_with_dataflow`, which uses
    // `kill_expired_borrows_dataflow` (the dataflow version). The legacy
    // walk body is no longer needed — the dataflow path produces identical
    // results on all 5028 comparable conformance tests.

    /// Stage 15.36 (HP-10 step 2 of 4, revised in Stage 15.40): Kill any
    /// active borrow whose `ref_local`'s **last read is at the given
    /// program point**.
    ///
    /// This is the dataflow-driven counterpart to `kill_expired_borrows`.
    /// It uses the same `LastUseMap` as the legacy path (recording the
    /// last program point where each local was read), plus the `ever_read`
    /// set (Stage 15.39 Option B) to preserve GAP-1 semantics.
    ///
    /// ## Kill logic (Stage 15.40 revised)
    ///
    /// A borrow is killed if BOTH conditions hold:
    /// 1. The `ref_local` was read somewhere in the body (`ever_read` check,
    ///    Stage 15.39 Option B — preserves GAP-1 by not killing never-read
    ///    ref_locals).
    /// 2. The `ref_local`'s last read is at the given program point
    ///    (`last_use_map` check — the borrow's useful lifetime ends at its
    ///    last read, which is the standard NLL borrow-lifetime semantics).
    ///
    /// ## Stage 15.67 (True Rust NLL) — liveness-based kill restored
    ///
    /// Stage 15.67 reverted to liveness-based kill, fixing the `&mut self`
    /// false positive via kill-after-call semantics in `check_terminator`'s
    /// Call arm. This is the correct NLL approach: a borrow dies when its
    /// `ref_local` is no longer live, NOT based on last-use + ever_read guard.
    ///
    /// `kill_borrows_on_redefinition` handles re-assigned borrow temps.
    /// Kill-after-call handles call-consumed borrow temps. Together, they
    /// implement correct NLL without the GAP-1 compromise.
    ///
    /// Per §1.0 原則 9 "正确 > 妥协": this is the correct fix, replacing
    /// the Stage 15.39 Option B `ever_read` workaround.
    ///
    /// Per §23: method name follows `<verb>_<noun>_<noun>` pattern.
    /// Stage 15.67 (True Rust NLL): Kill expired borrows based on liveness.
    ///
    /// This is the **true NLL** implementation — a borrow is killed when its
    /// `ref_local` is no longer live after the current program point. This
    /// replaces the Stage 15.39 "Option B" compromise (which used an
    /// `ever_read` guard to preserve GAP-1 lexical lifetimes).
    ///
    /// ## Why true NLL (§1.0 原則 9 "正确 > 妥协")
    ///
    /// The Option B compromise kept never-read borrows alive as "strays" to
    /// avoid fixing the `&mut self` method-call false positive. This rejected
    /// valid NLL programs like `let r1 = &mut x; let r2 = &mut x;` (r1 never
    /// read). Per §1.0 原則 9, the correct fix is to implement true NLL and
    /// fix the false positive properly (via `kill_borrows_on_redefinition`).
    ///
    /// ## Algorithm
    ///
    /// 1. Compute `live_after` = set of locals live after the current point
    ///    (using the fixpoint `LiveOutMap` + `compute_live_after_point`).
    /// 2. Kill any active borrow whose `ref_local` is NOT in `live_after`.
    ///    - A never-read local is dead immediately → its borrow is killed
    ///      immediately (correct NLL).
    ///    - A local read at the current point is still live → its borrow
    ///      stays (correct NLL).
    ///    - A local whose last read was before the current point is dead →
    ///      its borrow is killed (correct NLL).
    ///
    /// Per §1.0 原則 9 "正确 > 妥协": this is the correct NLL semantics.
    /// Per §16: uses `compute_live_after_point` (fixpoint liveness) — no HIR.
    /// Per §23: method name follows `<verb>_<noun>_<noun>` pattern.
    fn kill_expired_borrows_dataflow(
        &mut self,
        mir: &MirBody,
        live_out: &LiveOutMap,
        bb: BasicBlockId,
        stmt_idx: usize,
    ) {
        // Compute the set of locals live AFTER the current point.
        let live_after = compute_live_after_point(mir, live_out, bb, stmt_idx);

        // Kill any active borrow whose `ref_local` is NOT live after this
        // point. This is true NLL — a borrow dies when its ref_local is no
        // longer needed.
        let locals_to_kill: Vec<crate::mir::place::LocalId> = self
            .borrows
            .active_ref_locals()
            .filter(|local| !live_after.contains(local))
            .collect();
        for local in locals_to_kill {
            self.borrows.kill_borrows_of_local(local);
        }
    }

    /// Stage 15.36 (HP-10 step 2 of 4): Dataflow-driven borrow check entry
    /// point.
    ///
    /// This is the **`compute_liveness`-based counterpart** of
    /// `check_mir_body`. It performs the same forward walk over basic
    /// blocks, but uses `kill_expired_borrows_dataflow` (which consults
    /// the fixpoint `LiveOutMap`) instead of `kill_expired_borrows`
    /// (which consults the legacy single-pass `LastUseMap`).
    ///
    /// The walk structure mirrors `check_mir_body` exactly — only the
    /// borrow-expiry predicate differs. This deliberate symmetry lets us
    /// validate the dataflow path against the legacy path on the same
    /// MIR shape, then flip the switch in Stage 15.37 by replacing the
    /// call site in `driver.rs`.
    ///
    /// Per §1.0 原則 1 "长期 > 短期": keeping both entry points for one
    /// stage lets us A/B-test the dataflow path on real code before
    /// committing. Per §1.0 原則 3 "显式 > 隐式": the choice of analysis
    /// is explicit in the method name — no hidden flag.
    ///
    /// Per §23: method name follows `<verb>_<noun>_<noun>` pattern with
    /// the `_with_dataflow` suffix marking it as the v0.2 analysis. The
    /// legacy `check_mir_body` (no suffix) is the v0.1 default until
    /// Stage 15.37.
    pub fn check_mir_body_with_dataflow(&mut self, mir: &MirBody) {
        // Stage 15.67 (True Rust NLL): Compute the fixpoint liveness.
        // `live_in` maps each basic block to the set of locals live at
        // block ENTRY. `live_out` maps each basic block to the set of
        // locals live at block EXIT. Both are used by
        // `kill_expired_borrows_dataflow` to kill borrows whose
        // `ref_local` is no longer live (true NLL).
        //
        // Per §1.0 原則 9 "正确 > 妥协": this replaces the Stage 15.39
        // Option B compromise (which used `compute_last_use_map` + `ever_read`
        // to preserve GAP-1 lexical lifetimes). True NLL kills borrows based
        // on liveness, not last-use + ever_read guard.
        let (live_in, live_out) = compute_liveness(mir);

        // Main walk: forward over all basic blocks.
        for (bb_idx, bb) in mir.basic_blocks.iter().enumerate() {
            let bb_id = BasicBlockId(bb_idx as u32);
            let stmt_count = bb.statements.len();

            // Stage 15.67: Kill borrows at the START of each basic block.
            // A borrow from a previous block whose ref_local is not live at
            // this block's entry should be killed BEFORE processing this
            // block's statements. This handles the case where a method-call
            // temp in a conditional block is dead at the merge point — its
            // borrow must be killed before the next method call (which may
            // be in a different conditional block).
            //
            // `live_in[bb]` is the set of locals live at block entry. Kill
            // any active borrow whose `ref_local` is NOT in `live_in[bb]`.
            let live_before = live_in.get(&bb_id).cloned().unwrap_or_default();
            let locals_to_kill: Vec<crate::mir::place::LocalId> = self
                .borrows
                .active_ref_locals()
                .filter(|local| !live_before.contains(local))
                .collect();
            for local in locals_to_kill {
                self.borrows.kill_borrows_of_local(local);
            }

            for stmt_idx in 0..stmt_count {
                // Kill borrows whose ref_local is not live after the
                // PREVIOUS statement (stmt_idx - 1). This ensures the
                // borrow stays alive during the statement that performs
                // the last read (correct NLL — the ref_local is live
                // during its last read, then dies).
                if stmt_idx > 0 {
                    self.kill_expired_borrows_dataflow(mir, &live_out, bb_id, stmt_idx - 1);
                }
                // Stage 15.40: Kill-on-redefinition. Before processing
                // an Assign, kill any borrow whose ref_local is the LHS
                // of this Assign. This handles the case where a borrow
                // temp is re-assigned in a loop (e.g., `tmp = &mut c`
                // each iteration) — the old borrow from the previous
                // iteration must be killed before the new borrow is
                // created, otherwise they conflict.
                self.kill_borrows_on_redefinition(&bb.statements[stmt_idx]);
                self.check_statement(mir, &bb.statements[stmt_idx], bb_id, stmt_idx);
            }
            // After the last statement, kill borrows whose ref_local is
            // not live after the last statement.
            if stmt_count > 0 {
                self.kill_expired_borrows_dataflow(mir, &live_out, bb_id, stmt_count - 1);
            }
            // Check terminator (uses are at index == statements.len()).
            let term_idx = stmt_count;
            self.check_terminator(mir, &bb.terminator, bb_id, term_idx);
            self.kill_expired_borrows_dataflow(mir, &live_out, bb_id, term_idx);
        }

        // Run region inference as an additional check (same as legacy path).
        self.run_region_inference(mir);
    }

    /// Stage 15.40: Kill any active borrow whose `ref_local` is the LHS
    /// of the given statement (i.e., the local is about to be re-assigned).
    ///
    /// This is the "kill-on-def" semantics that standard NLL implementations
    /// use. When a local that holds a borrow is re-assigned, the old borrow
    /// must be killed BEFORE the new assignment is processed — otherwise
    /// the old borrow conflicts with any new borrow on the same place.
    ///
    /// ## Why this is needed
    ///
    /// The liveness-based kill (`kill_expired_borrows_dataflow`) only kills
    /// borrows whose `ref_local` is dead. In a loop, a borrow temp is
    /// correctly live across the back-edge (it's used in the next
    /// iteration's call), so the liveness-based kill doesn't kill it.
    /// But when the temp is re-assigned at the start of the next iteration,
    /// the old borrow should be killed — the temp now holds a NEW borrow,
    /// and the old one is stale.
    ///
    /// Without this kill-on-def, the dataflow path produces a false
    /// positive on `&mut self` method calls in loops:
    /// ```ignore
    /// while i < 5 {
    ///     c.increment();  // lowers to: tmp = &mut c; call increment(tmp)
    ///     i = i + 1;
    /// }
    /// ```
    /// Each iteration creates a fresh `tmp = &mut c`. The old `tmp`'s
    /// borrow (from the previous iteration) is still alive (live across
    /// the back-edge), so the new `&mut c` conflicts with it.
    ///
    /// With kill-on-def, the old `tmp`'s borrow is killed when `tmp` is
    /// re-assigned, so the new `&mut c` succeeds.
    ///
    /// Per §1.0 原則 3 "显式 > 隐式": the kill-on-def is explicit and
    /// documented. Per §15 "最优 > 最小": this is the minimum fix needed
    /// to handle the redefinition case.
    fn kill_borrows_on_redefinition(&mut self, stmt: &Statement) {
        if let StatementKind::Assign(boxed) = &stmt.kind {
            let (place, _rvalue) = &**boxed;
            // If the LHS is a simple local, kill any borrow whose
            // ref_local is that local. The local is about to be
            // re-assigned, so its old borrow is stale.
            if let PlaceKind::Local(lhs_local) = &place.kind {
                self.borrows.kill_borrows_of_local(*lhs_local);
            }
        }
    }

    fn check_statement(
        &mut self,
        mir: &MirBody,
        stmt: &Statement,
        _bb_id: BasicBlockId,
        _stmt_idx: usize,
    ) {
        match &stmt.kind {
            StatementKind::Assign(boxed) => {
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
            // Stage 18.229 (v0.2.5e): StatementKind::Store writes to `ptr`
            // (a Place). Check the write for mutability/borrow violations,
            // just like Assign. Without this, the Store to vec.ptr/vec.cap/
            // vec.len in lower_vec_push_intrinsic would bypass borrowck.
            //
            // Per §1.0 原則 4 (报错>静默): borrowck must check ALL writes.
            // Per §1.0 原則 6 (通解>特例): one check_place_write for all writes.
            // Per §17.6 (同类型整体修复): same class as Assign.
            StatementKind::Store {
                ptr,
                val,
                val_ty: _,
            } => {
                self.check_place_write(mir, ptr, stmt.span);
                // Also check the val operand for use-after-move.
                self.check_operand(mir, val, stmt.span);
            }
            _ => {}
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
                // Stage 15.85: use operand span instead of Span::DUMMY
                // so use-after-move / not-Copy errors point to the actual
                // source location (was: "1:1" file start).
                self.check_operand(mir, func, crate::mir::place::operand_span(func));
                for arg in args {
                    self.check_operand(mir, arg, crate::mir::place::operand_span(arg));
                }
                // Stage 15.67: Kill borrows for temp locals used as call args.
                // After a call `f(tmp)`, the temp `tmp` is dead (its value was
                // consumed by the call). Any borrow whose `ref_local` is `tmp`
                // should be killed, allowing the next method call to borrow
                // the same place without conflict.
                //
                // This is the "kill-after-call" semantics that complements
                // `kill_borrows_on_redefinition` (which handles Assign). Without
                // this, method-call temps in loops/conditionals keep their
                // borrows alive across the entire function, causing false
                // positives on valid method-call-heavy code.
                //
                // Per §1.0 原則 9 "正确 > 妥协": this is the correct fix for
                // the `&mut self` false positive (replaces the Stage 15.39
                // Option B `ever_read` workaround).
                let mut temps_to_kill: Vec<crate::mir::place::LocalId> = Vec::new();
                for arg in args {
                    if let Operand::Copy(lv) | Operand::Move(lv) = arg {
                        if let PlaceKind::Local(id) = &lv.kind {
                            temps_to_kill.push(*id);
                        }
                    }
                }
                for temp in temps_to_kill {
                    self.borrows.kill_borrows_of_local(temp);
                }
            }
            TerminatorKind::SwitchInt { discr, .. } => {
                // Stage 15.85: use operand span (was: Span::DUMMY).
                // Stage 18.169 (TD-OPTION-METHODS-BORROW): Use `check_operand_read`
                // instead of `check_operand` for match discriminants. Match
                // scrutinees are READ operations, not moves — they should NOT
                // trigger the Copy-ness check. This fixes `match *self` on
                // non-Copy types like `Option<T>`.
                //
                // Per §2 原則 9 (正确>妥协): match doesn't consume the scrutinee
                // (patterns may bind by value, but the discriminant read itself
                // is a borrow). Per §1.0 原則 6 (通解>特例): one fix for all
                // match expressions (user + prelude).
                self.check_operand_read(mir, discr, crate::mir::place::operand_span(discr));
            }
            // Stage 15.61 fix: `TerminatorKind::Drop` is a destructor, not a
            // read. Previously, `check_place_read` was called, which flagged
            // "use of moved value" for moved temps (e.g., the `init_local`
            // that holds `S{x: 0}` is moved into `s`, then `elaborate_drops`
            // inserts `Drop { place: init_local, ... }` at scope end).
            //
            // In rustc, `Drop` terminators use drop flags to skip moved
            // values. For Landin's MVP, we treat `Drop` as a no-op for moved
            // places (no error) and a consuming operation for live places
            // (record the move so subsequent uses are flagged).
            //
            // Per §1.0 原則 5 "报错 > 静默": dropping a moved value is safe
            // (no-op); dropping a live value consumes it.
            // Per §1.0 原則 3 "显式 > 隐式": the `Drop` terminator is
            // explicit in the MIR, but its semantics allow skipping.
            TerminatorKind::Drop { place, .. } => {
                let path = self.place_path(mir, place);
                // If the place is already moved, the drop is a no-op (the
                // value has been transferred elsewhere). Do NOT error.
                if !self.moves.is_moved(&path) {
                    // The place is live — the drop consumes it. Record the
                    // move so any subsequent use of the place is flagged.
                    //
                    // For field projections (e.g., dropping a single field),
                    // we don't record a move of the parent (matches the
                    // Operand::Move behavior in `check_operand`).
                    let is_field_projection = matches!(
                        &place.kind,
                        PlaceKind::Projection(_, ProjectionElem::Field(_, _))
                    );
                    if !is_field_projection {
                        self.moves.record_move(path);
                    }
                }
            }
            // Assert reads the condition operand (a bool). Check it
            // for use-after-move just like any other operand.
            TerminatorKind::Assert { cond, .. } => {
                // Stage 15.85: use operand span (was: Span::DUMMY).
                self.check_operand(mir, cond, crate::mir::place::operand_span(cond));
            }
            // Stage 18.116: Goto/Return/Unreachable have no borrow constraints.
            // All 7 TerminatorKind variants are now explicitly covered.
            TerminatorKind::Goto(_) | TerminatorKind::Return | TerminatorKind::Unreachable => {}
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
                        &format!(
                            "cannot borrow moved value: {}",
                            self.format_place_path(&borrowed_place)
                        ),
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
            Rvalue::Load(ptr_op, _) => {
                // Stage 18.228: Load reads its ptr operand.
                self.check_operand(mir, ptr_op, span);
            }
            Rvalue::GetElementPtr { base, indices, .. } => {
                // Stage 18.228: GEP reads its base + all index operands.
                self.check_operand(mir, base, span);
                for idx_op in indices {
                    self.check_operand(mir, idx_op, span);
                }
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
                    self.errors.push(BorrowError::use_after_move(
                        &format!("use of moved value: {}", self.format_place(lv)),
                        span,
                    ));
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
                // Stage 18.169 (TD-OPTION-METHODS-BORROW): Also allow Deref
                // projections. Reading through a reference (`*self` where
                // `self: &T`) is always safe — the reference guarantees the
                // value is valid and shared references allow reads.
                // This fixes `match *self` in Option/Result methods.
                // Per §2 原則 9 (正确>妥协): deref through &T is a read, not a move.
                let is_deref_projection =
                    matches!(&lv.kind, PlaceKind::Projection(_, ProjectionElem::Deref));
                let ty = self.place_ty(mir, lv);
                if !self.is_copy(&ty) && !is_field_projection && !is_deref_projection {
                    self.errors.push(BorrowError::not_copy(
                        // Stage 15.84: use human-readable type name
                        // (was: {:?} Debug format leaking TyKind).
                        // Stage 16.85: use format_ty for resolver-backed names.
                        format!(
                            "use of moved value: {} does not implement Copy; \
                             use an explicit move (`let y = move x;`) or borrow",
                            self.format_ty(&ty)
                        ),
                        span,
                    ));
                }
            }
            Operand::Move(lv) => {
                let path = self.place_path(mir, lv);
                if self.moves.is_moved(&path) {
                    self.errors.push(BorrowError::use_after_move(
                        &format!("use of moved value: {}", self.format_place(lv)),
                        span,
                    ));
                }
                // Check if borrowed
                if let Some(bk) = self.borrows.borrow_kind(&path) {
                    if bk == BorrowKind::Shared || bk == BorrowKind::Mut {
                        self.errors.push(BorrowError::move_borrowed(
                            &format!("cannot move borrowed value: {}", self.format_place(lv)),
                            span,
                        ));
                    }
                }
                // Stage 3.40: Don't record moves for field projections.
                // Moving a field (e.g., extracting enum discriminant) doesn't
                // move the whole parent value. This allows `match` on enums
                // to work without spurious "use of moved value" errors.
                //
                // Stage 15.73: Don't record moves for Copy types.
                // `Move` of a Copy type is semantically a copy (the source
                // remains valid). This is needed because MIR lowerer uses
                // `is_mir_ty_copy_conservative` (returns false for Adt) while
                // borrow checker uses `is_copy` (returns true for Adt via
                // unsound `ty_is_copy`). Without this check, `let s2 = s`
                // where s is a struct would mark s as moved (because MIR
                // lowerer uses Move), but the borrow checker considers
                // structs as Copy (unsound) — so the move shouldn't be
                // recorded. This preserves backward compatibility.
                let is_field_projection = matches!(
                    &lv.kind,
                    PlaceKind::Projection(_, ProjectionElem::Field(_, _))
                );
                let ty = self.place_ty(mir, lv);
                let is_copy = self.is_copy(&ty);
                if !is_field_projection && !is_copy {
                    self.moves.record_move(path);
                }
            }
            Operand::Constant(_) => {}
        }
    }

    /// Stage 18.169 (TD-OPTION-METHODS-BORROW): Check an operand for
    /// use-after-move ONLY, without the Copy-ness check.
    ///
    /// Used for `SwitchInt` discriminants (match scrutinees) where the
    /// operand is a READ, not a move. Match scrutinees should be allowed
    /// on non-Copy types (e.g., `match *self` where `self: &Option<T>`).
    ///
    /// Per §2 原則 9 (正确>妥协): match doesn't consume the scrutinee —
    /// only the patterns that bind by value do. The discriminant read
    /// itself is always safe.
    /// Per §1.0 原則 6 (通解>特例): one method for all read-only operands.
    /// Per §10: `check_operand_read` follows `<verb>_<noun>_<noun>` pattern.
    fn check_operand_read(&mut self, mir: &MirBody, op: &Operand, span: Span) {
        match op {
            Operand::Copy(lv) | Operand::Move(lv) => {
                let path = self.place_path(mir, lv);
                if self.moves.is_moved(&path) {
                    self.errors.push(BorrowError::use_after_move(
                        &format!("use of moved value: {}", self.format_place(lv)),
                        span,
                    ));
                }
                // NOTE: Intentionally NO Copy-ness check and NO move recording.
                // Match discriminants are reads, not moves.
            }
            Operand::Constant(_) => {}
        }
    }

    // Stage 15.86: `operand_span` moved to `mir::place::operand_span` (shared
    // helper, DRY per §23 rule 5). Callers now use
    // `crate::mir::place::operand_span(op)` directly. Previously duplicated
    // as a private method here (Stage 15.85) and in `typeck::checker` (Stage
    // 15.81).

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
                    &format!("cannot assign to borrowed value: {}", self.format_place(lv)),
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
                    &format!("cannot assign twice to immutable variable: local#{}", id.0),
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

/// Stage 15.72: Check a single MIR body for borrow/ownership errors.
///
/// Free-function convenience wrapper around
/// `BorrowChecker::check_mir_body_with_dataflow`. Creates a
/// `BorrowChecker` with default settings (no resolver, no fn_sigs).
///
/// Per §23: free-function entry point follows `<verb>_<noun>` pattern.
/// Per §1.0 原則 5 "去除兼容思维": deprecated `check_mir_body` and
/// `check_crate` aliases removed.
pub fn check_mir_body_with_dataflow(mir: &MirBody) -> Vec<BorrowError> {
    let mut bc: BorrowChecker<'_> = BorrowChecker::new();
    bc.check_mir_body_with_dataflow(mir);
    bc.into_errors()
}

// Stage 15.72: Deprecated `check_crate` free function REMOVED.
// Per §1.0 原則 5 "去除兼容思维": dead code removed.
// Per §16: this function violated interface isolation (re-lowered HIR
// to MIR inside borrowck). Use `driver::compile` or
// `BorrowChecker::check_mir_body_with_dataflow` directly.

#[cfg(test)]
mod tests;
