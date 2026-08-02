//! Stage 7.1 (TD-015 step 1): Region inference data structures + constraint collection.
//!
//! Per `docs/lang-design/04-ownership-borrowing.md` §4.6 (NLL 完整规范).
//! Extracted as a new module per `docs/stage-committee-process.md` v3.21
//! §13.4 (stage-start design alignment) + §14.4 (refactoring as architecture
//! realignment).
//!
//! Stage 15.32 (v0.2): Dead code is allowed via `#[allow(dead_code)]` on the
//! `mod region_inference;` declaration in borrowck/mod.rs. Only `new()`,
//! `region_to_vid()`, `collect_implied_bounds()`, and `infer_regions()` are
//! called. The remaining APIs are infrastructure for future v0.2 Phase 2 work.
//!
//! Per §1.0 原則 1 "长期 > 短期": the infrastructure is kept for future use.
//! Per §14.4 J1 (单一职责): the module is self-contained.
//! Per §15 "最优 > 最小": keeping the code is the right trade-off.

// Original module documentation continues:
// design).
//
// This module implements the data structures + constraint collection API
// for region inference. The actual inference algorithm (fixed-point iteration)
// is deferred to Stage 7.2 (TD-015 step 2).
//
// Design alignment (4.6):
// - 4.6.1 Universal region: RegionInfo::Universal + UniversalRegion
// - 4.6.2 Implied bounds: OutlivesConstraint (collected from &'a T -> T: 'a)
// - 4.6.3 Universe: UniverseCause + UniverseId
// - 4.6.4 Type tests: TypeTest { universal_region, ty, span }
// - 4.6.5 SCC: deferred to Stage 7.4
// - 4.6.6 RegionInferenceContext: complete data structure
//
// 16 compliance:
// This module is independent of BorrowChecker — it only reads MirBody
// data structures. Future Stage 7.5 will integrate it into borrowck,
// replacing the simplified NLL (last-use map).

use crate::mir::place::LocalId;
use crate::mir::ty::{Region, RegionVid, Ty};
use crate::session::Span;

/// Information about a single region (lifetime variable).
///
/// Per §4.6.1, regions are classified as:
/// - **Universal**: from function signature (`'a`, `'b`, `'static`) — not inferred
/// - **Inference**: created during constraint collection — inferred by algorithm
/// - **Placeholder**: used in canonical queries (HRTB) — Stage 7.4
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum RegionInfo {
    /// Universal region from function signature (e.g., `'a`, `'b`, `'static`).
    /// These are NOT inferred — they represent caller-provided lifetimes.
    Universal {
        /// The name of the lifetime parameter (e.g., `'a`, `'static`).
        /// `'static` is always present; others come from fn signature.
        name: &'static str,
        /// The `RegionVid` assigned to this universal region.
        vid: RegionVid,
    },
    /// Inference region — created during constraint collection.
    /// The inference algorithm (Stage 7.2) will resolve these to point sets.
    Inference {
        /// The `RegionVid` assigned to this inference region.
        vid: RegionVid,
        /// The universe this region belongs to (§4.6.3).
        /// Universe 0 is the root; higher universes are created by HRTB.
        universe: UniverseId,
    },
    /// Placeholder region — used in canonical queries (§4.6.1).
    /// Deferred to Stage 7.4 (HRTB support).
    Placeholder {
        /// The universe this placeholder belongs to.
        universe: UniverseId,
        /// The `RegionVid` assigned to this placeholder.
        vid: RegionVid,
    },
}

impl RegionInfo {
    /// Get the `RegionVid` of this region.
    pub(crate) fn vid(&self) -> RegionVid {
        match self {
            RegionInfo::Universal { vid, .. } => *vid,
            RegionInfo::Inference { vid, .. } => *vid,
            RegionInfo::Placeholder { vid, .. } => *vid,
        }
    }

    /// Returns `true` if this is a universal region.
    pub(crate) fn is_universal(&self) -> bool {
        matches!(self, RegionInfo::Universal { .. })
    }

    /// Returns `true` if this is an inference region.
    pub(crate) fn is_inference(&self) -> bool {
        matches!(self, RegionInfo::Inference { .. })
    }

    /// Returns `true` if this is a placeholder region.
    pub(crate) fn is_placeholder(&self) -> bool {
        matches!(self, RegionInfo::Placeholder { .. })
    }
}

/// A universe identifier (§4.6.3).
///
/// Universe 0 is the root universe (function body).
/// Higher universes are created by HRTB `for<'a>` (Stage 7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct UniverseId(pub u32);

impl UniverseId {
    /// The root universe (universe 0).
    pub(crate) const ROOT: UniverseId = UniverseId(0);

    /// Create a new universe with the next id.
    pub(crate) fn next(self) -> UniverseId {
        UniverseId(self.0 + 1)
    }
}

/// An outlives constraint `'a: 'b` (read: `'a` outlives `'b`,
/// i.e., `'a` is at least as long as `'b`).
///
/// Per §4.6.2, these are collected from:
/// - Function signatures (`fn f<'a, 'b>(x: &'a u32, y: &'b u32) where 'a: 'b`)
/// - Implied bounds (`&'a T` implies `T: 'a`)
/// - Borrow relationships (`&'a x` creates `'a: 'borrow_region`)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct OutlivesConstraint {
    /// The sup region (`'a` in `'a: 'b`).
    pub sup: RegionVid,
    /// The sub region (`'b` in `'a: 'b`).
    pub sub: RegionVid,
    /// Where this constraint came from (for error reporting).
    pub cause: ConstraintCause,
}

/// The cause of an outlives constraint (for error reporting).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ConstraintCause {
    /// From function signature: `fn f<'a, 'b>(...) where 'a: 'b`.
    FnSignature { span: Span },
    /// From implied bounds: `&'a T` implies `T: 'a` (§4.6.2).
    ImpliedBound { span: Span },
    /// From a borrow: `let r = &x;` creates a constraint between
    /// the borrow region and the borrowed place's region.
    Borrow { span: Span, borrowed_local: LocalId },
    /// From a type test: `T: 'a` verification (§4.6.4).
    TypeTest { span: Span },
}

/// A type test: verify that `ty` outlives `universal_region` (§4.6.4).
///
/// Type tests are checked AFTER region inference. If a type test fails,
/// the error is "T does not live long enough".
#[derive(Debug, Clone)]
pub(crate) struct TypeTest {
    /// The universal region that `ty` must outlive.
    pub universal_region: RegionVid,
    /// The type being tested.
    pub ty: Ty,
    /// Where this test came from (for error reporting).
    pub span: Span,
}

/// The cause of a universe creation (§4.6.3).
///
/// Each HRTB `for<'a>` creates a new universe with a fresh placeholder region.
#[derive(Debug, Clone)]
pub(crate) enum UniverseCause {
    /// The root universe (universe 0) — function body.
    Root,
    /// Created by HRTB `for<'a> fn(&'a T)`.
    Hrtb {
        /// The span of the `for<>` binder.
        span: Span,
    },
}

/// The complete region inference context (§4.6.6).
///
/// This data structure holds all the information needed for region inference:
/// - Universal regions (from fn signature)
/// - All region definitions
/// - Outlives constraints
/// - Type tests
/// - Universe causes
///
/// The actual inference algorithm (fixed-point iteration) is deferred to
/// Stage 7.2. This struct only provides the **data structure + constraint
/// collection API**.
#[derive(Debug, Clone)]
pub(crate) struct RegionInferenceContext {
    /// All universal regions (from function signature: `'a`, `'b`, `'static`).
    /// Per §4.6.1, these are NOT inferred — they represent caller-provided lifetimes.
    universal_regions: Vec<RegionVid>,

    /// All region definitions, indexed by `RegionVid`.
    /// `region_defs[vid.0 as usize]` gives the `RegionInfo` for `vid`.
    region_defs: Vec<RegionInfo>,

    /// All outlives constraints (`'a: 'b`).
    constraints: Vec<OutlivesConstraint>,

    /// All type tests (`T: 'a`).
    type_tests: Vec<TypeTest>,

    /// All universe causes, indexed by `UniverseId`.
    /// `universe_causes[universe.0 as usize]` gives the cause for that universe.
    universe_causes: Vec<UniverseCause>,

    /// The next fresh `RegionVid` to allocate.
    next_region_vid: u32,

    /// The next fresh `UniverseId` to allocate.
    next_universe: u32,

    /// Stage 7.2: Use points for each region, indexed by `RegionVid`.
    /// `use_points[vid.0 as usize]` gives the use points for `vid`.
    /// Populated by `add_use_point()` before `infer_regions()` is called.
    use_points: Vec<Vec<PointIndex>>,

    /// Stage 7.2: Inferred point sets, indexed by `RegionVid`.
    /// `region_points[vid.0 as usize]` gives the inferred point set for `vid`.
    /// Populated by `infer_regions()`.
    region_points: Vec<RegionSet>,
}

impl Default for RegionInferenceContext {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionInferenceContext {
    /// Create a new empty `RegionInferenceContext`.
    ///
    /// Initializes with:
    /// - `'static` as universal region 0
    /// - Universe 0 (root) created
    pub(crate) fn new() -> Self {
        let mut ctx = Self {
            universal_regions: Vec::new(),
            region_defs: Vec::new(),
            constraints: Vec::new(),
            type_tests: Vec::new(),
            universe_causes: vec![UniverseCause::Root],
            next_region_vid: 0,
            next_universe: 1, // Universe 0 is root; next is 1
            use_points: Vec::new(),
            region_points: Vec::new(),
        };
        // Allocate `'static` as universal region 0.
        ctx.add_universal_region("'static");
        ctx
    }

    /// Allocate a new universal region (from function signature).
    ///
    /// Per §4.6.1, universal regions are NOT inferred — they represent
    /// caller-provided lifetimes. The inference algorithm (Stage 7.2) will
    /// treat these as fixed.
    pub(crate) fn add_universal_region(&mut self, name: &'static str) -> RegionVid {
        let vid = RegionVid(self.next_region_vid);
        self.next_region_vid += 1;
        self.region_defs.push(RegionInfo::Universal { name, vid });
        self.universal_regions.push(vid);
        vid
    }

    /// Allocate a new inference region.
    ///
    /// Inference regions are created during constraint collection (e.g.,
    /// for each `&x` borrow). The inference algorithm (Stage 7.2) will
    /// resolve these to point sets.
    pub(crate) fn add_inference_region(&mut self, universe: UniverseId) -> RegionVid {
        let vid = RegionVid(self.next_region_vid);
        self.next_region_vid += 1;
        self.region_defs
            .push(RegionInfo::Inference { vid, universe });
        vid
    }

    /// Add an outlives constraint `'sup: 'sub` (§4.6.2).
    ///
    /// This means `sup` outlives `sub` (i.e., `sup` is at least as long as `sub`).
    /// The inference algorithm (Stage 7.2) will propagate point sets accordingly.
    pub(crate) fn add_outlives_constraint(
        &mut self,
        sup: RegionVid,
        sub: RegionVid,
        cause: ConstraintCause,
    ) {
        self.constraints
            .push(OutlivesConstraint { sup, sub, cause });
    }

    /// Add a type test: verify that `ty` outlives `universal_region` (§4.6.4).
    ///
    /// Type tests are checked AFTER region inference. If a type test fails,
    /// the error is "T does not live long enough".
    pub(crate) fn add_type_test(&mut self, universal_region: RegionVid, ty: Ty, span: Span) {
        self.type_tests.push(TypeTest {
            universal_region,
            ty,
            span,
        });
    }

    /// Create a new universe (for HRTB `for<'a>`, §4.6.3).
    ///
    /// Returns the new `UniverseId`. Each HRTB creates a fresh universe
    /// with its own placeholder region set.
    pub(crate) fn new_universe(&mut self, cause: UniverseCause) -> UniverseId {
        let uid = UniverseId(self.next_universe);
        self.next_universe += 1;
        self.universe_causes.push(cause);
        uid
    }

    /// Get all universal regions.
    pub(crate) fn universal_regions(&self) -> &[RegionVid] {
        &self.universal_regions
    }

    /// Get all region definitions.
    pub(crate) fn region_defs(&self) -> &[RegionInfo] {
        &self.region_defs
    }

    /// Get all outlives constraints.
    pub(crate) fn constraints(&self) -> &[OutlivesConstraint] {
        &self.constraints
    }

    /// Get all type tests.
    pub(crate) fn type_tests(&self) -> &[TypeTest] {
        &self.type_tests
    }

    /// Get the `RegionInfo` for a given `RegionVid`.
    ///
    /// Returns `None` if the `RegionVid` is out of range.
    pub(crate) fn region_info(&self, vid: RegionVid) -> Option<&RegionInfo> {
        self.region_defs.get(vid.0 as usize)
    }

    /// Get the total number of regions (universal + inference + placeholder).
    pub(crate) fn num_regions(&self) -> usize {
        self.region_defs.len()
    }

    /// Get the total number of constraints.
    pub(crate) fn num_constraints(&self) -> usize {
        self.constraints.len()
    }

    /// Get the total number of type tests.
    pub(crate) fn num_type_tests(&self) -> usize {
        self.type_tests.len()
    }

    /// Convert a `Region` (from MIR types) to a `RegionVid`.
    ///
    /// - `Region::Static` → the `'static` universal region (vid 0)
    /// - `Region::Var(vid)` → `vid`
    /// - `Region::Erased` → `'static` (erased regions are treated as `'static`)
    pub(crate) fn region_to_vid(&self, region: Region) -> RegionVid {
        match region {
            Region::Static => RegionVid(0), // `'static` is always vid 0
            Region::Var(vid) => vid,
            Region::Erased => RegionVid(0), // erased = `'static` for inference
        }
    }
}

// ================================================================
// Stage 7.2 (TD-015 step 2): Region inference algorithm
// ================================================================

/// A point in the MIR control-flow graph (§4.2).
///
/// Encoded as `u32` for simplicity: `(bb_id << 16) | stmt_idx`.
/// `bb_id` is the basic block index (0..65535), `stmt_idx` is the
/// statement index within the block (0..65535). The terminator
/// occupies `stmt_idx == statements.len()`.
pub(crate) type PointIndex = u32;

/// Encode a point from basic block id and statement index.
pub(crate) fn make_point(bb_id: u32, stmt_idx: u32) -> PointIndex {
    (bb_id << 16) | (stmt_idx & 0xFFFF)
}

/// Decode the basic block id from a point.
pub(crate) fn point_bb(p: PointIndex) -> u32 {
    p >> 16
}

/// Decode the statement index from a point.
pub(crate) fn point_stmt(p: PointIndex) -> u32 {
    p & 0xFFFF
}

/// A sorted set of CFG points — the inferred value of a region (§4.2).
///
/// Represents the set of program points where a region is "live"
/// (the region outlives all points in its set).
pub(crate) type RegionSet = Vec<PointIndex>;

/// An error detected during region inference (§4.2 universal region check
/// + §4.6.4 type test verification).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RegionInferenceError {
    /// A non-universal region `r` escaped a universal region `ur`:
    /// `r` contains points not in `ur`'s point set.
    ///
    /// This means a value with lifetime `r` would outlive the scope
    /// it's allowed to live in — a use-after-free risk.
    RegionEscapesUniversal {
        /// The non-universal region that escaped.
        escaping_region: RegionVid,
        /// The universal region it should have been contained in.
        universal_region: RegionVid,
        /// The points that caused the escape (in escaping but not in universal).
        escape_points: Vec<PointIndex>,
    },
    /// A type test failed: `ty` does not outlive `universal_region` (§4.6.4).
    ///
    /// This means a value of type `ty` was used in a context that requires
    /// it to live at least as long as `universal_region`, but the regions
    /// in `ty` don't satisfy that constraint.
    TypeTestFailed {
        /// The universal region that `ty` was supposed to outlive.
        universal_region: RegionVid,
        /// The type that failed the test.
        ty: Ty,
        /// The span where the test was registered.
        span: Span,
        /// The regions in `ty` that don't outlive `universal_region`.
        failing_regions: Vec<RegionVid>,
    },
}

impl RegionInferenceContext {
    /// Add a use point for a region (§4.2).
    ///
    /// Use points are the program points where a region is "used"
    /// (e.g., a reference with that lifetime is read). The inference
    /// algorithm adds these to the region's point set.
    ///
    /// Per §16: this API is called by borrowck (Stage 7.5 integration)
    /// to populate use points before calling `infer_regions()`.
    pub(crate) fn add_use_point(&mut self, vid: RegionVid, point: PointIndex) {
        // Ensure use_points vec is large enough
        let idx = vid.0 as usize;
        if idx >= self.use_points.len() {
            self.use_points.resize(idx + 1, Vec::new());
        }
        self.use_points[idx].push(point);
    }

    /// Run the region inference algorithm (§4.2 fixed-point iteration).
    ///
    /// Algorithm:
    /// 1. Initialize: each region's point set = empty
    /// 2. Fixed-point iteration:
    ///    - For each constraint `'sup: 'sub`: `sup.points = sup.points ∪ sub.points`
    ///    - Add each region's use_points to its point set
    ///    - Repeat until no change
    /// 3. Check universal regions: for each universal `ur`, every non-universal
    ///    `r` must have `r.points ⊆ ur.points`
    ///
    /// Returns `Ok(())` if all checks pass, or `Err(Vec<RegionInferenceError>)`
    /// listing all violations.
    ///
    /// Per §4.2: complexity O(R² × P), R=regions, P=points. In practice
    /// R and P are small, so this is nearly linear.
    pub(crate) fn infer_regions(&mut self) -> Result<(), Vec<RegionInferenceError>> {
        // Step 1: Initialize point sets to empty
        let num_regions = self.region_defs.len();
        let mut region_points: Vec<RegionSet> = vec![Vec::new(); num_regions];

        // Step 2: Fixed-point iteration
        let mut changed = true;
        while changed {
            changed = false;

            // 2a: Propagate constraints: 'sup: 'sub means sup ⊇ sub
            for constraint in &self.constraints {
                let sup_idx = constraint.sup.0 as usize;
                let sub_idx = constraint.sub.0 as usize;
                if sup_idx < num_regions && sub_idx < num_regions {
                    let sub_points = region_points[sub_idx].clone();
                    let sup_points = &mut region_points[sup_idx];
                    let old_len = sup_points.len();
                    for p in &sub_points {
                        if !sup_points.contains(p) {
                            sup_points.push(*p);
                        }
                    }
                    if sup_points.len() != old_len {
                        changed = true;
                    }
                }
            }

            // 2b: Add each region's use_points to its point set
            for (idx, use_pts) in self.use_points.iter().enumerate() {
                if idx >= num_regions {
                    break;
                }
                let pts = &mut region_points[idx];
                let old_len = pts.len();
                for p in use_pts {
                    if !pts.contains(p) {
                        pts.push(*p);
                    }
                }
                if pts.len() != old_len {
                    changed = true;
                }
            }
        }

        // Sort point sets for deterministic comparison (subset checks)
        for pts in &mut region_points {
            pts.sort_unstable();
            pts.dedup();
        }

        // Step 3: Check universal regions
        // For each universal region `ur`, every non-universal region `r`
        // must have r.points ⊆ ur.points.
        let mut errors = Vec::new();
        let universal_vids: Vec<RegionVid> = self
            .region_defs
            .iter()
            .filter(|info| info.is_universal())
            .map(|info| info.vid())
            .collect();

        for ur_vid in &universal_vids {
            let ur_idx = ur_vid.0 as usize;
            let ur_points = &region_points[ur_idx];
            for (idx, info) in self.region_defs.iter().enumerate() {
                if info.is_universal() {
                    continue; // universal regions don't check against themselves
                }
                let r_points = &region_points[idx];
                // Check r.points ⊆ ur.points
                let escape_points: Vec<PointIndex> = r_points
                    .iter()
                    .filter(|p| !ur_points.contains(p))
                    .copied()
                    .collect();
                if !escape_points.is_empty() {
                    errors.push(RegionInferenceError::RegionEscapesUniversal {
                        escaping_region: RegionVid(idx as u32),
                        universal_region: *ur_vid,
                        escape_points,
                    });
                }
            }
        }

        // Store the inferred point sets
        self.region_points = region_points;

        // Step 4 (§4.6.4): Check type tests
        // After region inference, verify that each type test `T: 'a` passes.
        // For each type test, extract the regions in `ty` and check they
        // all outlive `universal_region` (i.e., their point sets are subsets).
        for tt in &self.type_tests {
            let ur_idx = tt.universal_region.0 as usize;
            if ur_idx >= self.region_points.len() {
                continue;
            }
            let ur_points = &self.region_points[ur_idx];

            // Extract all regions from `ty`
            let ty_regions = extract_regions_from_ty(&tt.ty);

            // Check each region in `ty` outlives `universal_region`
            let failing_regions: Vec<RegionVid> = ty_regions
                .iter()
                .filter(|r| {
                    let r_idx = r.0 as usize;
                    if r_idx >= self.region_points.len() {
                        return false;
                    }
                    let r_points = &self.region_points[r_idx];
                    // r outlives ur iff r_points ⊆ ur_points
                    !r_points.iter().all(|p| ur_points.contains(p))
                })
                .copied()
                .collect();

            if !failing_regions.is_empty() {
                errors.push(RegionInferenceError::TypeTestFailed {
                    universal_region: tt.universal_region,
                    ty: tt.ty.clone(),
                    span: tt.span,
                    failing_regions,
                });
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Collect implied bounds from a reference type `&'a T` (§4.6.2).
    ///
    /// Per §4.6.2: `&'a T` implies `T: 'a` (all regions in `T` outlive `'a`).
    /// This method extracts all regions from `T` and adds `region: 'a`
    /// outlives constraints for each.
    ///
    /// Per §16: this API is called by borrowck (Stage 7.5 integration)
    /// when processing reference types in function signatures and bodies.
    pub(crate) fn collect_implied_bounds(
        &mut self,
        ref_region: RegionVid,
        inner_ty: &Ty,
        span: Span,
    ) {
        let inner_regions = extract_regions_from_ty(inner_ty);
        for r in inner_regions {
            // `r: ref_region` means r outlives ref_region (§4.6.2: T: 'a)
            self.add_outlives_constraint(r, ref_region, ConstraintCause::ImpliedBound { span });
        }
    }

    /// Stage 15.50 (HP-5 step 3): Collect outlives constraints from MIR
    /// statements and terminators.
    ///
    /// Walks all basic blocks in the MIR body. For each statement and
    /// terminator that involves references, adds outlives constraints
    /// between the regions:
    ///
    /// - `r = &x` (Rvalue::Ref): `region(r) >= region(x)` — the borrow's
    ///   region must outlive the borrowed place's region.
    /// - `r = Copy(x)` where `x` is `&T`: `region(r) >= region(x)` —
    ///   copying a reference propagates the lifetime.
    /// - `call f(&x, &y)` (TerminatorKind::Call): for each `&T` argument,
    ///   `region(arg) >= region(param)` (simplified: just collect the
    ///   argument's region for the function's constraint set).
    ///
    /// Per §23: method name follows `<verb>_<noun>_<noun>` pattern.
    /// Per §16: reads only `&MirBody` — no writes, no HIR lookup.
    pub(crate) fn collect_mir_constraints(&mut self, mir: &crate::mir::body::MirBody) {
        use crate::mir::body::{StatementKind, TerminatorKind};
        use crate::mir::place::Rvalue;
        use crate::mir::place::PlaceKind;

        for (bb_idx, bb) in mir.basic_blocks.iter().enumerate() {
            let _bb_id = crate::mir::body::BasicBlockId(bb_idx as u32);

            // Collect constraints from statements.
            for stmt in &bb.statements {
                if let StatementKind::Assign(boxed) = &stmt.kind {
                    let (place, rvalue) = &**boxed;
                    match rvalue {
                        Rvalue::Ref(region, _kind, borrowed_place) => {
                            // `r = &'a x` → the borrowed place's regions
                            // must outlive 'a.
                            let ref_vid = self.region_to_vid(*region);
                            // Get the borrowed place's type.
                            let borrowed_ty = self.place_ty(mir, borrowed_place);
                            let borrowed_regions = extract_regions_from_ty(&borrowed_ty);
                            for r in borrowed_regions {
                                // r outlives ref_vid
                                self.add_outlives_constraint(
                                    r,
                                    ref_vid,
                                    ConstraintCause::Borrow {
                                        span: stmt.span,
                                        borrowed_local: match &borrowed_place.kind {
                                            PlaceKind::Local(id) => *id,
                                            _ => crate::mir::place::LocalId(0),
                                        },
                                    },
                                );
                            }
                        }
                        Rvalue::Use(op) | Rvalue::Cast(_, op, _) => {
                            // `r = Copy(x)` where x is `&T` → propagate lifetime.
                            if let crate::mir::place::Operand::Copy(lv)
                            | crate::mir::place::Operand::Move(lv) = op
                            {
                                let src_ty = self.place_ty(mir, lv);
                                let src_regions = extract_regions_from_ty(&src_ty);
                                if !src_regions.is_empty() {
                                    // The LHS place's type might also have regions.
                                    let lhs_ty = self.place_ty(mir, place);
                                    let lhs_regions = extract_regions_from_ty(&lhs_ty);
                                    // For each pair, add: src_region outlives lhs_region.
                                    // Simplified: just use the first region of each.
                                    if let (Some(&src_r), Some(&lhs_r)) =
                                        (src_regions.first(), lhs_regions.first())
                                    {
                                        self.add_outlives_constraint(
                                            src_r,
                                            lhs_r,
                                            ConstraintCause::Borrow {
                                                span: stmt.span,
                                                borrowed_local: match &lv.kind {
                                                    PlaceKind::Local(id) => *id,
                                                    _ => crate::mir::place::LocalId(0),
                                                },
                                            },
                                        );
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Collect constraints from terminators (function calls).
            if let TerminatorKind::Call { args, .. } = &bb.terminator.kind {
                for arg in args {
                    if let crate::mir::place::Operand::Copy(lv)
                    | crate::mir::place::Operand::Move(lv) = arg
                    {
                        let arg_ty = self.place_ty(mir, lv);
                        let arg_regions = extract_regions_from_ty(&arg_ty);
                        // For each region in the argument type, add a
                        // constraint that it outlives 'static (vid 0).
                        // This is a simplified constraint — the real
                        // constraint would be between the argument's
                        // region and the parameter's region, but we don't
                        // have access to the function signature here.
                        // TODO (Stage 15.51): use fn_sigs for proper constraints.
                        for r in arg_regions {
                            if r != RegionVid(0) {
                                // r outlives 'static (always true, but
                                // exercises the constraint collection path).
                                self.add_outlives_constraint(
                                    r,
                                    RegionVid(0),
                                    ConstraintCause::FnSignature { span: bb.terminator.span },
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// Helper: look up the type of a place in the MIR body.
    fn place_ty(&self, mir: &crate::mir::body::MirBody, lv: &crate::mir::place::Place) -> crate::mir::ty::Ty {
        use crate::mir::place::PlaceKind;
        match &lv.kind {
            PlaceKind::Local(id) => {
                if (id.0 as usize) < mir.local_decls.len() {
                    mir.local(*id).ty.clone()
                } else {
                    crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Error, lv.span)
                }
            }
            PlaceKind::Static(_) => {
                crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Error, lv.span)
            }
            PlaceKind::Projection(base, elem) => {
                let base_ty = self.place_ty(mir, base);
                match elem {
                    crate::mir::place::ProjectionElem::Deref => match &base_ty.kind {
                        crate::mir::ty::TyKind::Ref(_, _, inner)
                        | crate::mir::ty::TyKind::RawPtr(_, inner) => (**inner).clone(),
                        _ => crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Error, lv.span),
                    },
                    crate::mir::place::ProjectionElem::Field(_, field_ty) => field_ty.clone(),
                    crate::mir::place::ProjectionElem::Index(_)
                    | crate::mir::place::ProjectionElem::ConstantIndex { .. }
                    | crate::mir::place::ProjectionElem::Subslice { .. } => match &base_ty.kind {
                        crate::mir::ty::TyKind::Array(inner, _)
                        | crate::mir::ty::TyKind::Slice(inner) => (**inner).clone(),
                        _ => crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Error, lv.span),
                    },
                }
            }
        }
    }

    /// Get the inferred point set for a region (after `infer_regions()`).
    ///
    /// Returns `None` if `infer_regions()` has not been called yet,
    /// or if the `RegionVid` is out of range.
    pub(crate) fn region_points(&self, vid: RegionVid) -> Option<&RegionSet> {
        self.region_points.get(vid.0 as usize)
    }

    // ================================================================
    // Stage 7.4 (TD-015 step 4): Universe tracking + SCC compression
    // ================================================================

    /// Get the universe of a region (§4.6.3).
    ///
    /// Returns the `UniverseId` the region belongs to:
    /// - Universal regions: always `UniverseId::ROOT` (they come from the
    ///   function signature, which is in the root universe)
    /// - Inference regions: the universe specified when created
    /// - Placeholder regions: the universe specified when created
    pub(crate) fn region_universe(&self, vid: RegionVid) -> Option<UniverseId> {
        let info = self.region_info(vid)?;
        Some(match info {
            RegionInfo::Universal { .. } => UniverseId::ROOT,
            RegionInfo::Inference { universe, .. } => *universe,
            RegionInfo::Placeholder { universe, .. } => *universe,
        })
    }

    /// Check that no inference region escapes its universe (§4.6.3).
    ///
    /// Per §4.6.3, each universe has an independent placeholder region set.
    /// An inference region in universe N must not be constrained to outlive
    /// a region in a lower universe M < N — that would mean a higher-universe
    /// region "escaped" into a lower universe, which is unsound.
    ///
    /// This check is run AFTER `infer_regions()`. Returns a list of
    /// `UniverseEscapeError` for each violation.
    pub(crate) fn check_universe_escapes(&self) -> Vec<UniverseEscapeError> {
        let mut errors = Vec::new();
        for constraint in &self.constraints {
            let sup_info = match self.region_info(constraint.sup) {
                Some(info) => info,
                None => continue,
            };
            let sub_info = match self.region_info(constraint.sub) {
                Some(info) => info,
                None => continue,
            };

            // Only check inference regions (placeholders are universal-like)
            let sup_universe = match sup_info {
                RegionInfo::Inference { universe, .. } => *universe,
                _ => continue, // universal/placeholder sup is always fine
            };
            let sub_universe = match sub_info {
                RegionInfo::Inference { universe, .. } => *universe,
                RegionInfo::Placeholder { universe, .. } => *universe,
                RegionInfo::Universal { .. } => UniverseId::ROOT,
            };

            // If sup is in a higher universe than sub, that's an escape
            if sup_universe.0 > sub_universe.0 {
                errors.push(UniverseEscapeError {
                    escaping_region: constraint.sup,
                    target_region: constraint.sub,
                    escaping_universe: sup_universe,
                    target_universe: sub_universe,
                });
            }
        }
        errors
    }

    /// Compute SCC (Strongly Connected Components) of the region constraint
    /// graph (§4.6.5).
    ///
    /// Per §4.6.5, the constraint graph is compressed using SCC to avoid
    /// O(R²×P) degrading to exponential complexity. Regions in the same
    /// SCC have equivalent point sets (they mutually outlive each other).
    ///
    /// Returns a `Vec<SccId>` where `sccs[vid.0 as usize]` gives the SCC
    /// ID for region `vid`. Regions in the same SCC share the same ID.
    ///
    /// Algorithm: Tarjan's SCC (single DFS, O(V+E)).
    pub(crate) fn compute_sccs(&self) -> Vec<SccId> {
        let num_regions = self.region_defs.len();
        let mut sccs = vec![SccId(0); num_regions];
        let mut index_counter = 0u32;
        let mut stack: Vec<RegionVid> = Vec::new();
        let mut on_stack = vec![false; num_regions];
        let mut indices = vec![None; num_regions];
        let mut low_links = vec![0u32; num_regions];

        // Build adjacency list from constraints (sup → sub edges)
        // 'sup: 'sub means sup ⊇ sub, so in the constraint graph there's
        // an edge sup → sub (sup depends on sub).
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); num_regions];
        for constraint in &self.constraints {
            let sup_idx = constraint.sup.0 as usize;
            let sub_idx = constraint.sub.0 as usize;
            if sup_idx < num_regions && sub_idx < num_regions {
                adj[sup_idx].push(sub_idx);
            }
        }

        // Tarjan's SCC algorithm
        #[allow(clippy::too_many_arguments)]
        fn strongconnect(
            v: usize,
            adj: &[Vec<usize>],
            index_counter: &mut u32,
            stack: &mut Vec<RegionVid>,
            on_stack: &mut Vec<bool>,
            indices: &mut Vec<Option<u32>>,
            low_links: &mut Vec<u32>,
            sccs: &mut Vec<SccId>,
            scc_id_counter: &mut u32,
        ) {
            indices[v] = Some(*index_counter);
            low_links[v] = *index_counter;
            *index_counter += 1;
            stack.push(RegionVid(v as u32));
            on_stack[v] = true;

            for &w in &adj[v] {
                match indices[w] {
                    None => {
                        strongconnect(
                            w,
                            adj,
                            index_counter,
                            stack,
                            on_stack,
                            indices,
                            low_links,
                            sccs,
                            scc_id_counter,
                        );
                        low_links[v] = low_links[v].min(low_links[w]);
                    }
                    Some(_) if on_stack[w] => {
                        low_links[v] = low_links[v].min(indices[w].unwrap());
                    }
                    _ => {}
                }
            }

            // If v is a root node, pop the SCC
            if low_links[v] == indices[v].unwrap() {
                loop {
                    let w = stack.pop().unwrap();
                    let w_idx = w.0 as usize;
                    on_stack[w_idx] = false;
                    sccs[w_idx] = SccId(*scc_id_counter);
                    if w_idx == v {
                        break;
                    }
                }
                *scc_id_counter += 1;
            }
        }

        let mut scc_id_counter = 0u32;
        for v in 0..num_regions {
            if indices[v].is_none() {
                strongconnect(
                    v,
                    &adj,
                    &mut index_counter,
                    &mut stack,
                    &mut on_stack,
                    &mut indices,
                    &mut low_links,
                    &mut sccs,
                    &mut scc_id_counter,
                );
            }
        }

        sccs
    }
}

/// An SCC (Strongly Connected Component) identifier (§4.6.5).
///
/// Regions in the same SCC have equivalent point sets — they mutually
/// outlive each other. The SCC compression replaces each SCC with a
/// single node, reducing the graph size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SccId(pub u32);

/// An error detected during universe escape checking (§4.6.3).
///
/// An inference region in a higher universe was constrained to outlive
/// a region in a lower universe — a soundness violation.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UniverseEscapeError {
    /// The region that escaped (in a higher universe).
    pub escaping_region: RegionVid,
    /// The region it was constrained to outlive (in a lower universe).
    pub target_region: RegionVid,
    /// The universe of the escaping region.
    pub escaping_universe: UniverseId,
    /// The universe of the target region.
    pub target_universe: UniverseId,
}

/// Extract all `RegionVid`s from a `Ty` (for implied bounds + type tests).
///
/// Per §4.6.2: `&'a T` implies `T: 'a`. To collect this, we need to
/// find all regions appearing inside `T`. This function walks the type
/// recursively and collects all `Region::Var(vid)` / `Region::Static`.
///
/// Returns a deduplicated `Vec<RegionVid>`.
pub(crate) fn extract_regions_from_ty(ty: &Ty) -> Vec<RegionVid> {
    let mut regions = Vec::new();
    collect_regions_recursive(ty, &mut regions);
    // Deduplicate
    regions.sort_unstable();
    regions.dedup();
    regions
}

/// Recursive helper for `extract_regions_from_ty`.
fn collect_regions_recursive(ty: &Ty, out: &mut Vec<RegionVid>) {
    use crate::mir::ty::TyKind;
    match &ty.kind {
        TyKind::Ref(region, _mutability, inner_ty) => {
            // Extract the region from the reference
            let vid = match region {
                Region::Static => RegionVid(0), // `'static` is vid 0
                Region::Var(vid) => *vid,
                Region::Erased => RegionVid(0), // erased = `'static`
            };
            if !out.contains(&vid) {
                out.push(vid);
            }
            // Recurse into the inner type
            collect_regions_recursive(inner_ty, out);
        }
        TyKind::Tuple(tys) => {
            for t in tys {
                collect_regions_recursive(t, out);
            }
        }
        TyKind::Array(inner, _const) => {
            collect_regions_recursive(inner, out);
        }
        TyKind::Slice(inner) => {
            collect_regions_recursive(inner, out);
        }
        TyKind::RawPtr(_mutability, inner) => {
            collect_regions_recursive(inner, out);
        }
        // Types without regions: Bool, Char, Int, Uint, Float, Never,
        // FnDef, FnPtr, Str, Adt, Closure, Infer, Error, Foreign, Param
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_context_has_static() {
        let ctx = RegionInferenceContext::new();
        // `'static` is universal region 0
        assert_eq!(ctx.num_regions(), 1);
        assert_eq!(ctx.universal_regions().len(), 1);
        assert_eq!(ctx.universal_regions()[0], RegionVid(0));
        // RegionInfo for vid 0 should be Universal { name: "'static" }
        let info = ctx.region_info(RegionVid(0)).unwrap();
        assert!(info.is_universal());
        match info {
            RegionInfo::Universal { name, vid } => {
                assert_eq!(*name, "'static");
                assert_eq!(*vid, RegionVid(0));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_add_universal_region() {
        let mut ctx = RegionInferenceContext::new();
        let vid_a = ctx.add_universal_region("'a");
        assert_eq!(vid_a, RegionVid(1)); // 0 is `'static`
        assert_eq!(ctx.num_regions(), 2);
        assert_eq!(ctx.universal_regions().len(), 2);
        let info = ctx.region_info(vid_a).unwrap();
        assert!(info.is_universal());
    }

    #[test]
    fn test_add_inference_region() {
        let mut ctx = RegionInferenceContext::new();
        let vid = ctx.add_inference_region(UniverseId::ROOT);
        assert_eq!(vid, RegionVid(1)); // 0 is `'static`
        assert_eq!(ctx.num_regions(), 2);
        let info = ctx.region_info(vid).unwrap();
        assert!(info.is_inference());
        match info {
            RegionInfo::Inference { vid: v, universe } => {
                assert_eq!(*v, RegionVid(1));
                assert_eq!(*universe, UniverseId::ROOT);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_add_outlives_constraint() {
        let mut ctx = RegionInferenceContext::new();
        let vid_a = ctx.add_universal_region("'a");
        let vid_b = ctx.add_universal_region("'b");
        ctx.add_outlives_constraint(
            vid_a,
            vid_b,
            ConstraintCause::FnSignature { span: Span::DUMMY },
        );
        assert_eq!(ctx.num_constraints(), 1);
        let c = &ctx.constraints()[0];
        assert_eq!(c.sup, vid_a);
        assert_eq!(c.sub, vid_b);
    }

    #[test]
    fn test_add_type_test() {
        let mut ctx = RegionInferenceContext::new();
        let vid_static = RegionVid(0); // `'static`
        let ty = Ty::new(
            crate::mir::ty::TyKind::Int(crate::ast::IntTy::I32),
            Span::DUMMY,
        );
        ctx.add_type_test(vid_static, ty.clone(), Span::DUMMY);
        assert_eq!(ctx.num_type_tests(), 1);
        let tt = &ctx.type_tests()[0];
        assert_eq!(tt.universal_region, vid_static);
        assert_eq!(tt.ty, ty);
    }

    #[test]
    fn test_new_universe() {
        let mut ctx = RegionInferenceContext::new();
        assert_eq!(ctx.universe_causes.len(), 1); // Root
        let uid = ctx.new_universe(UniverseCause::Hrtb { span: Span::DUMMY });
        assert_eq!(uid, UniverseId(1));
        assert_eq!(ctx.universe_causes.len(), 2);
    }

    #[test]
    fn test_region_to_vid() {
        let ctx = RegionInferenceContext::new();
        // `'static` → vid 0
        assert_eq!(ctx.region_to_vid(Region::Static), RegionVid(0));
        // `Region::Var(5)` → vid 5
        assert_eq!(ctx.region_to_vid(Region::Var(RegionVid(5))), RegionVid(5));
        // `Region::Erased` → vid 0 (treated as `'static`)
        assert_eq!(ctx.region_to_vid(Region::Erased), RegionVid(0));
    }

    #[test]
    fn test_universe_next() {
        assert_eq!(UniverseId::ROOT, UniverseId(0));
        assert_eq!(UniverseId::ROOT.next(), UniverseId(1));
        assert_eq!(UniverseId(5).next(), UniverseId(6));
    }

    #[test]
    fn test_region_info_predicates() {
        let universal = RegionInfo::Universal {
            name: "'a",
            vid: RegionVid(1),
        };
        assert!(universal.is_universal());
        assert!(!universal.is_inference());
        assert!(!universal.is_placeholder());

        let inference = RegionInfo::Inference {
            vid: RegionVid(2),
            universe: UniverseId::ROOT,
        };
        assert!(!inference.is_universal());
        assert!(inference.is_inference());
        assert!(!inference.is_placeholder());

        let placeholder = RegionInfo::Placeholder {
            universe: UniverseId(1),
            vid: RegionVid(3),
        };
        assert!(!placeholder.is_universal());
        assert!(!placeholder.is_inference());
        assert!(placeholder.is_placeholder());
    }
}

// ================================================================
// Stage 7.2 (TD-015 step 2): Region inference algorithm tests
// ================================================================

#[test]
fn test_infer_regions_empty() {
    let mut ctx = RegionInferenceContext::new();
    // No constraints, no use points → all point sets empty, no errors
    let result = ctx.infer_regions();
    assert!(result.is_ok());
    // `'static` (vid 0) has empty point set
    assert_eq!(ctx.region_points(RegionVid(0)), Some(&Vec::new()));
}

#[test]
fn test_infer_regions_use_points() {
    let mut ctx = RegionInferenceContext::new();
    let vid_static = RegionVid(0); // `'static`
    let vid_a = ctx.add_inference_region(UniverseId::ROOT); // vid 1

    // Add use points for region 'a
    ctx.add_use_point(vid_a, make_point(0, 1));
    ctx.add_use_point(vid_a, make_point(0, 2));

    // Also add use points for 'static so 'a's points don't escape
    ctx.add_use_point(vid_static, make_point(0, 1));
    ctx.add_use_point(vid_static, make_point(0, 2));

    let result = ctx.infer_regions();
    assert!(result.is_ok());

    // 'a should have points {0,1}, {0,2}
    let pts = ctx.region_points(vid_a).unwrap();
    assert_eq!(pts.len(), 2);
    assert!(pts.contains(&make_point(0, 1)));
    assert!(pts.contains(&make_point(0, 2)));

    // 'static also has the same points
    let static_pts = ctx.region_points(vid_static).unwrap();
    assert_eq!(static_pts.len(), 2);
}

#[test]
fn test_infer_regions_constraint_propagation() {
    let mut ctx = RegionInferenceContext::new();
    let vid_static = RegionVid(0); // `'static`
    let vid_a = ctx.add_inference_region(UniverseId::ROOT); // vid 1
    let vid_b = ctx.add_inference_region(UniverseId::ROOT); // vid 2

    // 'a: 'b (a outlives b) → a ⊇ b
    ctx.add_outlives_constraint(
        vid_a,
        vid_b,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );

    // 'b has use point, 'a does not
    ctx.add_use_point(vid_b, make_point(0, 5));
    // 'static also has the use point so 'a doesn't escape
    ctx.add_use_point(vid_static, make_point(0, 5));

    let result = ctx.infer_regions();
    assert!(result.is_ok());

    // 'a should inherit 'b's points via constraint propagation
    let a_pts = ctx.region_points(vid_a).unwrap();
    let b_pts = ctx.region_points(vid_b).unwrap();
    assert!(b_pts.contains(&make_point(0, 5)));
    assert!(a_pts.contains(&make_point(0, 5))); // propagated
}

#[test]
fn test_infer_regions_universal_escape_detected() {
    let mut ctx = RegionInferenceContext::new();
    // vid 0 = 'static (universal)
    let vid_a = ctx.add_inference_region(UniverseId::ROOT); // vid 1

    // 'a has use point but no constraint linking it to 'static
    ctx.add_use_point(vid_a, make_point(0, 1));

    // 'static has no use points → empty point set
    // 'a has point {0,1} which is NOT subset of 'static's empty set
    // → RegionEscapesUniversal error
    let result = ctx.infer_regions();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1);
    match &errors[0] {
        RegionInferenceError::RegionEscapesUniversal {
            escaping_region,
            universal_region,
            escape_points,
        } => {
            assert_eq!(*escaping_region, vid_a);
            assert_eq!(*universal_region, RegionVid(0)); // 'static
            assert_eq!(escape_points.len(), 1);
            assert!(escape_points.contains(&make_point(0, 1)));
        }
        RegionInferenceError::TypeTestFailed { .. } => {
            panic!("expected RegionEscapesUniversal, got TypeTestFailed");
        }
    }
}

#[test]
fn test_infer_regions_universal_no_escape() {
    let mut ctx = RegionInferenceContext::new();
    let vid_static = RegionVid(0); // 'static (universal)
    let vid_a = ctx.add_inference_region(UniverseId::ROOT); // vid 1

    // 'a: 'static (a outlives 'static) → a ⊇ 'static
    // This means 'a can have any points 'static has (but 'static is empty)
    ctx.add_outlives_constraint(
        vid_a,
        vid_static,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );

    // 'a has no use points → empty point set
    // empty ⊆ empty → no error
    let result = ctx.infer_regions();
    assert!(result.is_ok());
}

#[test]
fn test_point_encoding() {
    let p = make_point(3, 7);
    assert_eq!(point_bb(p), 3);
    assert_eq!(point_stmt(p), 7);

    let p2 = make_point(0, 0);
    assert_eq!(point_bb(p2), 0);
    assert_eq!(point_stmt(p2), 0);

    let p3 = make_point(65535, 65535);
    assert_eq!(point_bb(p3), 65535);
    assert_eq!(point_stmt(p3), 65535);
}

#[test]
fn test_infer_regions_fixed_point_convergence() {
    // Test that the fixed-point iteration converges with chained constraints
    let mut ctx = RegionInferenceContext::new();
    let vid_static = RegionVid(0); // 'static
    let vid_a = ctx.add_inference_region(UniverseId::ROOT); // vid 1
    let vid_b = ctx.add_inference_region(UniverseId::ROOT); // vid 2
    let vid_c = ctx.add_inference_region(UniverseId::ROOT); // vid 3

    // Chain: 'a: 'b, 'b: 'c → 'a ⊇ 'b ⊇ 'c
    ctx.add_outlives_constraint(
        vid_a,
        vid_b,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );
    ctx.add_outlives_constraint(
        vid_b,
        vid_c,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );

    // Only 'c has use points
    ctx.add_use_point(vid_c, make_point(1, 0));
    // 'static also has the use point so 'a doesn't escape
    ctx.add_use_point(vid_static, make_point(1, 0));

    let result = ctx.infer_regions();
    assert!(result.is_ok());

    // All three should have the same point set (propagated through chain)
    let a_pts = ctx.region_points(vid_a).unwrap();
    let b_pts = ctx.region_points(vid_b).unwrap();
    let c_pts = ctx.region_points(vid_c).unwrap();
    assert!(c_pts.contains(&make_point(1, 0)));
    assert!(b_pts.contains(&make_point(1, 0))); // propagated from c
    assert!(a_pts.contains(&make_point(1, 0))); // propagated from b
}

// ================================================================
// Stage 7.3 (TD-015 step 3): Implied bounds + type tests
// ================================================================

#[test]
fn test_extract_regions_from_ref() {
    use crate::mir::ty::{Mutability, TyKind};
    // &'a i32
    let ty = Ty::new(
        TyKind::Ref(
            Region::Var(RegionVid(5)),
            Mutability::Immutable,
            Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
        ),
        Span::DUMMY,
    );
    let regions = extract_regions_from_ty(&ty);
    assert_eq!(regions, vec![RegionVid(5)]);
}

#[test]
fn test_extract_regions_from_nested_ref() {
    use crate::mir::ty::{Mutability, TyKind};
    // &'a &'b i32
    let inner = Ty::new(
        TyKind::Ref(
            Region::Var(RegionVid(3)),
            Mutability::Immutable,
            Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
        ),
        Span::DUMMY,
    );
    let outer = Ty::new(
        TyKind::Ref(
            Region::Var(RegionVid(5)),
            Mutability::Immutable,
            Box::new(inner),
        ),
        Span::DUMMY,
    );
    let regions = extract_regions_from_ty(&outer);
    assert_eq!(regions, vec![RegionVid(3), RegionVid(5)]);
}

#[test]
fn test_extract_regions_from_non_ref() {
    // i32 has no regions
    let ty = Ty::new(
        crate::mir::ty::TyKind::Int(crate::ast::IntTy::I32),
        Span::DUMMY,
    );
    let regions = extract_regions_from_ty(&ty);
    assert!(regions.is_empty());
}

#[test]
fn test_collect_implied_bounds() {
    use crate::mir::ty::{Mutability, TyKind};
    let mut ctx = RegionInferenceContext::new();
    let vid_a = ctx.add_universal_region("'a"); // vid 1
    let vid_b = ctx.add_universal_region("'b"); // vid 2

    // &'a &'b i32 — implies 'b: 'a
    let inner_ty = Ty::new(
        TyKind::Ref(
            Region::Var(vid_b),
            Mutability::Immutable,
            Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
        ),
        Span::DUMMY,
    );
    ctx.collect_implied_bounds(vid_a, &inner_ty, Span::DUMMY);

    // Should have added a constraint 'b: 'a (vid_b outlives vid_a)
    assert_eq!(ctx.num_constraints(), 1);
    let c = &ctx.constraints()[0];
    assert_eq!(c.sup, vid_b); // 'b outlives 'a
    assert_eq!(c.sub, vid_a);
}

#[test]
fn test_type_test_passes() {
    use crate::mir::ty::TyKind;
    let mut ctx = RegionInferenceContext::new();
    let vid_static = RegionVid(0);

    // i32 has no regions → type test always passes
    let ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY);
    ctx.add_type_test(vid_static, ty, Span::DUMMY);

    let result = ctx.infer_regions();
    assert!(result.is_ok());
}

#[test]
fn test_type_test_fails() {
    use crate::mir::ty::{Mutability, TyKind};
    let mut ctx = RegionInferenceContext::new();
    let vid_static = RegionVid(0); // 'static, no use points
    let vid_a = ctx.add_inference_region(UniverseId::ROOT); // vid 1

    // &'a i32 — has region vid_a
    let ty = Ty::new(
        TyKind::Ref(
            Region::Var(vid_a),
            Mutability::Immutable,
            Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
        ),
        Span::DUMMY,
    );

    // Add a use point for 'a (but NOT for 'static)
    ctx.add_use_point(vid_a, make_point(0, 1));

    // Type test: &'a i32 must outlive 'static
    // 'a has point {0,1}, 'static has {} → 'a doesn't outlive 'static
    ctx.add_type_test(vid_static, ty.clone(), Span::DUMMY);

    let result = ctx.infer_regions();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    // Should have both RegionEscapesUniversal AND TypeTestFailed
    let has_type_test_error = errors
        .iter()
        .any(|e| matches!(e, RegionInferenceError::TypeTestFailed { .. }));
    assert!(has_type_test_error, "expected TypeTestFailed error");
}

// ================================================================
// Stage 7.4 (TD-015 step 4): Universe tracking + SCC compression tests
// ================================================================

#[test]
fn test_region_universe() {
    let mut ctx = RegionInferenceContext::new();
    // 'static (vid 0) is in ROOT universe
    assert_eq!(ctx.region_universe(RegionVid(0)), Some(UniverseId::ROOT));

    // Inference region in ROOT universe
    let vid_a = ctx.add_inference_region(UniverseId::ROOT);
    assert_eq!(ctx.region_universe(vid_a), Some(UniverseId::ROOT));

    // Create a new universe (e.g., for HRTB)
    let uid1 = ctx.new_universe(UniverseCause::Hrtb { span: Span::DUMMY });
    assert_eq!(uid1, UniverseId(1));

    // Inference region in universe 1
    let vid_b = ctx.add_inference_region(uid1);
    assert_eq!(ctx.region_universe(vid_b), Some(UniverseId(1)));
}

#[test]
fn test_check_universe_escapes_no_violation() {
    let mut ctx = RegionInferenceContext::new();
    let vid_a = ctx.add_inference_region(UniverseId::ROOT);
    let vid_b = ctx.add_inference_region(UniverseId::ROOT);

    // 'a: 'b (both in ROOT universe — no escape)
    ctx.add_outlives_constraint(
        vid_a,
        vid_b,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );

    let errors = ctx.check_universe_escapes();
    assert!(errors.is_empty());
}

#[test]
fn test_check_universe_escapes_detected() {
    let mut ctx = RegionInferenceContext::new();
    let uid1 = ctx.new_universe(UniverseCause::Hrtb { span: Span::DUMMY });
    let vid_a = ctx.add_inference_region(uid1); // universe 1
    let vid_b = ctx.add_inference_region(UniverseId::ROOT); // universe 0

    // 'a: 'b where 'a is in universe 1, 'b is in universe 0
    // This is an escape: higher-universe 'a constrained to outlive lower-universe 'b
    ctx.add_outlives_constraint(
        vid_a,
        vid_b,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );

    let errors = ctx.check_universe_escapes();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].escaping_region, vid_a);
    assert_eq!(errors[0].target_region, vid_b);
    assert_eq!(errors[0].escaping_universe, UniverseId(1));
    assert_eq!(errors[0].target_universe, UniverseId::ROOT);
}

#[test]
fn test_scc_no_constraints() {
    let mut ctx = RegionInferenceContext::new();
    ctx.add_inference_region(UniverseId::ROOT);
    ctx.add_inference_region(UniverseId::ROOT);

    let sccs = ctx.compute_sccs();
    // 3 regions ('static + 2 inference), no constraints → 3 SCCs
    assert_eq!(sccs.len(), 3);
    // Each region is its own SCC (all distinct)
    let unique_sccs: Vec<SccId> = {
        let mut s = sccs.clone();
        s.sort_by_key(|s| s.0);
        s.dedup();
        s
    };
    assert_eq!(unique_sccs.len(), 3);
}

#[test]
fn test_scc_mutual_constraints() {
    let mut ctx = RegionInferenceContext::new();
    let vid_a = ctx.add_inference_region(UniverseId::ROOT); // vid 1
    let vid_b = ctx.add_inference_region(UniverseId::ROOT); // vid 2

    // 'a: 'b AND 'b: 'a → mutual → same SCC
    ctx.add_outlives_constraint(
        vid_a,
        vid_b,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );
    ctx.add_outlives_constraint(
        vid_b,
        vid_a,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );

    let sccs = ctx.compute_sccs();
    // vid 1 and vid 2 should be in the same SCC
    assert_eq!(sccs[1], sccs[2]);
    // 'static (vid 0) should be in a different SCC
    assert_ne!(sccs[0], sccs[1]);
}

#[test]
fn test_scc_chain() {
    let mut ctx = RegionInferenceContext::new();
    let vid_a = ctx.add_inference_region(UniverseId::ROOT); // vid 1
    let vid_b = ctx.add_inference_region(UniverseId::ROOT); // vid 2
    let vid_c = ctx.add_inference_region(UniverseId::ROOT); // vid 3

    // Chain: 'a: 'b, 'b: 'c (no cycle) → 3 distinct SCCs (plus 'static)
    ctx.add_outlives_constraint(
        vid_a,
        vid_b,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );
    ctx.add_outlives_constraint(
        vid_b,
        vid_c,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );

    let sccs = ctx.compute_sccs();
    // All 4 regions should be in distinct SCCs
    let unique_sccs: Vec<SccId> = {
        let mut s = sccs.clone();
        s.sort_by_key(|s| s.0);
        s.dedup();
        s
    };
    assert_eq!(unique_sccs.len(), 4);
}
