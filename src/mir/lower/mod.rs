//! HIR → MIR lowering.
//!
//! Converts each HIR body (expression tree) into a MIR body (control
//! flow graph of basic blocks + statements + terminators).
//!
//! Public entry point: [`lower_hir_body_to_mir`].

use crate::ast;
use crate::hir::*;
use crate::mir::body::TerminatorKind;
use crate::mir::body::*;
use crate::mir::dyn_trait::DynTraitMIRPlan;
use crate::mir::place::*;
use crate::mir::ty::*;
use crate::session::Span;
use crate::typeck::unify::UnificationTable;
use lasso::Rodeo;

mod adt_layout;
mod body_lower;
// Stage 18.132 §13.4 J1-J6: extract call lowering from expr_operand.rs
mod call_lower;
mod closure_capture;
mod control_flow;
// Stage 18.133 §13.4 J1-J6: extract expression variants from expr_operand.rs
mod expr_operand;
mod expr_variants;
mod field_resolution;
// Stage 18.273 §13.4 J1-J6: extract intrinsic lowering from expr_variants.rs
// Stage 18.305 §13.4 J1-J6: split intrinsic_lower.rs into sub-modules.
// Stage 33.1: vec_intrinsics removed — Vec::push/get now in prelude impl.
// Stage 36.6: format_intrinsics.rs + string_intrinsics.rs + box_intrinsics.rs
// DELETED — format! now uses prelude __landin_format_v2, String::from_str/
// push_str in prelude impl (Stage 31.6b/31.6c), Box::new in prelude impl
// (Stage 31.6f). All intrinsics migrated to prelude (通解).
// Stage 18.131 §13.4 J1-J6: extract method resolution from expr_operand.rs
mod method_resolution;
// Stage 18.309 §13.4 J1-J6: extract `lower_method_call_expr` from
// expr_variants.rs (largest single function, 634 LOC). Sibling of
// expr_variants.rs / call_lower.rs.
mod method_call_lower;
mod overflow_assert;
// Stage 18.284 §13.4 J1-J6: primitive intrinsic dispatch (post-resolution)
mod primitive_intrinsics;
// Stage 18.279 §13.4 J1-J6: extract pattern/match lowering from control_flow.rs
mod pattern_bindings;
mod pattern_lower;
mod ty_lower;
mod writeback;

// Stage 6.10 (TD-011): Re-export expression lowering functions from
// `expr_operand` so:
// (1) mod.rs's body entry points (`lower_hir_body_to_mir_full*`) can call
//     `lower_expr_to_operand` directly;
// (2) sibling lower modules (`control_flow`, `pattern_bindings`, etc.) can
//     keep using `super::lower_expr_to_operand` / `super::resolve_enum_variant`
//     unchanged — the names remain in mod.rs's namespace;
// (3) `mir/mod.rs` can keep `pub use lower::build_dyn_trait_call_terminator`
//     — `pub use` here preserves the public re-export chain.
//
// `lower_expr_to_place` is intentionally NOT re-exported here: it is only
// used internally within `expr_operand.rs` (4 call sites), never from
// mod.rs or sibling modules.
//
// Per §23 (API naming): no glob re-export — each name is listed explicitly.
// Stage 18.132: build_dyn_trait_call_terminator moved to call_lower.rs.
pub use call_lower::build_dyn_trait_call_terminator;
// Stage 15.6 (v0.2): Expose the uncached inner function so tests can
// verify cache semantics (cached result == uncached result). Per §29.1.3
// (Design-Impl-Test coverage): tests need direct access to verify the
// cache wrapper doesn't change behavior.
// Stage 18.131: moved to method_resolution.rs (extracted sub-responsibility).
pub use method_resolution::query_method_return_type_uncached;
// Stage 18.287: re-export name_of_primitive_hir_ty for driver codegen prep
// (needed to generate correct LLVM function names for primitive impl methods).
// `pub(crate)` re-export — only used within the compiler crate, not exposed
// to external callers.
pub(crate) use method_resolution::name_of_primitive_hir_ty;
// Stage 15.7 (v0.2): Expose consolidated writeback functions for the
// driver to call. Per §23 (API Naming): `pub use` of named functions
// (no glob). Per §16: driver is orchestrator-only — these functions
// contain the writeback logic, driver just calls them in order.
// Stage 18.131: resolve_enum_variant moved to method_resolution.rs.
pub(crate) use expr_operand::lower_expr_to_operand;
pub(crate) use method_resolution::resolve_enum_variant;
pub use writeback::{writeback_closures, writeback_fndef_substs, writeback_type_propagation};
// Stage 14.41: populate_adt_layouts was re-exported here so the driver
// could re-run it after writeback. Stage 15.8: the driver no longer calls
// it (uses build_crate_adt_layouts instead). The function is still called
// internally by lower_hir_body_to_mir via the adt_layout::module path.
// The re-export is removed to eliminate the unused-import warning.
//
// Stage 15.8 (v0.2): Crate-level ADT layouts builder. The driver calls
// this once after HIR resolution and shares the result via Arc<AdtLayouts>.
// Per §23 (API Naming): `pub use` of named function (no glob).
// Per §16: reads HIR (allowed in MIR lower), produces MIR data.
pub use adt_layout::build_crate_adt_layouts;

// Stage 18.203 (TD-BOX-SIZE-OF + TD-VEC-ELEM-SIZE-INFERENCE integrated fix):
// Single source of truth for type-size queries used by runtime intrinsics
// (Box::new, Vec::push, Vec::get). Per §10.1.4: explicit re-export, no glob.
// Per §12 (最优 > 最小): replaces 3× duplicated size tables in expr_variants.rs.
pub use adt_layout::compute_type_size;
pub use adt_layout::compute_type_size_with_fallback;

// Stage 18.129 §13.4 J1-J6: extract type lowering to ty_lower.rs
// Re-export functions called from outside mir::lower (driver.rs, siblings).
// Per §10.1.4: explicit list, no glob.
// Per §13.4.3 反模式 5: only re-export functions actually called externally.
pub(crate) use ty_lower::{
    lower_hir_ty_to_mir_ty, lower_hir_ty_to_mir_ty_with_generics, lower_hir_ty_to_mir_ty_with_hir,
    lower_hir_ty_to_mir_ty_with_hir_and_generics,
};
// Stage 18.130: test-only re-export (used by mod.rs tests, not by non-test code).
#[cfg(test)]
pub(crate) use ty_lower::lower_hir_ty_to_mir_ty_with_lifetimes;

// Stage 18.130 §13.4 J1-J6: extract body lowering to body_lower.rs
// Re-export functions called from outside mir::lower (driver.rs).
// Per §10.1.4: explicit list, no glob.
// Per §13.4.3 反模式 5: only re-export functions actually called externally.
pub use body_lower::{
    build_synthesized_closure_mir_body, find_elided_ref_span, lower_body, lower_body_full,
    lower_hir_body_to_mir, lower_hir_body_to_mir_full,
    lower_hir_body_to_mir_full_with_dyn_trait_plan, lower_hir_body_to_mir_with_return_ty,
};

/// Lowering context for HIR→MIR conversion.
///
/// Holds the MIR body being built, a local-variable map (HIR HirId →
/// MIR LocalId), the current basic block being filled, and a
/// unification table for allocating fresh inference variables.
pub struct MirLowerCtxt<'a> {
    pub interner: &'a Rodeo,
    pub mir: MirBody,
    /// Map from HirId → LocalId for variables that have been assigned
    /// a MIR local.
    pub local_map: std::collections::HashMap<HirId, LocalId>,
    /// The current basic block being filled with statements.
    pub current_block: BasicBlockId,
    /// Unification table for allocating fresh TyVid/IntVid/FloatVar.
    /// Each call to `fresh_infer_ty()` produces a unique variable.
    pub unify: UnificationTable,
    /// Stage 3.30 (per §16): HIR crate reference for resolving ADT field
    /// types during lowering. Set by `lower_hir_body_to_mir_full`.
    /// `Option` because some test contexts construct MirLowerCtxt without
    /// a HIR crate (e.g., unit tests of helper functions).
    pub hir: Option<&'a HirCrate>,
    /// Stage 5.76: optional `DynTraitMIRPlan` for dyn Trait method call
    /// lowering. When set, the `HirExprKind::MethodCall` branch (Stage
    /// 5.77+) can query this plan via `find_dyn_trait_method_call_in_plan()`
    /// to retrieve the vtable slot index + param count for a dyn Trait
    /// method call.
    ///
    /// Per §16: the plan is built **upstream** (by the driver, using
    /// `build_dyn_trait_mir_plan_from_resolver()`) and passed in as a
    /// read-only value. `MirLowerCtxt` does not own a TraitResolver.
    pub dyn_trait_plan: Option<DynTraitMIRPlan>,
    /// Stage 18.262 (TD-TUPLE-CTOR-CALL-ARG Phase 2e): Optional pre-built
    /// `fn_sigs` map (DefId → Sig) for call-arg expected-ty propagation.
    /// When set, `lower_call_expr` can look up the callee's sig.inputs[i]
    /// to thread the expected arg type into `lower_expr_to_operand`.
    ///
    /// Per §11.2 (allowed cross-stage access — pre-computed data
    /// contract): `fn_sigs` is built **upstream** by the driver (per
    /// `driver/compile_inner.rs` lines 109-285) and passed in as a
    /// read-only reference. `MirLowerCtxt` does not own or modify it.
    /// Per §11.5 (data sinking preferred): the sig is pre-computed
    /// once and reused, not re-derived from HIR at each call site.
    /// Per §1.0 原則 6 (通解 > 特解): one fn_sigs-based path for all
    /// call arg expected-ty propagation, not a per-type special case.
    /// Per §13.4 J3 (one-way flow): fn_sigs flows driver → MIR lower
    /// → lower_call_expr → arg operands. No back-edges.
    pub fn_sigs: Option<&'a std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>>,
    /// Stage 16.85: Optional resolver for rich error messages.
    /// When set, "no method found" errors show actual type names
    /// (e.g., "MyStruct") instead of placeholders ("<adt>").
    /// Set by `set_resolver` before lowering begins.
    resolver: Option<&'a crate::traits::TraitResolver>,
    /// Stage 13.3a (TD-030): Side-table mapping the LocalId that holds a
    /// closure struct value → the closure's HIR body + params + capture
    /// info. Used by `HirExprKind::Call` to inline the closure body at the
    /// call site.
    ///
    /// The key is the LocalId of any local that holds a closure value —
    /// either the original closure literal's local, or a let-bound local
    /// that received the closure via Move/Copy (propagated by the let
    /// lowering in `control_flow::lower_block`).
    ///
    /// Stage 16.34 (Task 10 Step 5 — cleanup): Removed the `closure_bodies`
    /// side-table. The closure dispatch at the call site now uses the
    /// type-based check (`TyKind::Closure(_, _)`) instead of the side-table
    /// lookup. The `SynthesizedClosureFunction` metadata (in
    /// `synthesized_closure_functions` below) is the single source of truth
    /// for closure metadata.
    ///
    /// Per §1.0 原則 5 "去除兼容思维": dead side-table removed.
    /// Per §23 rule 5 (DRY): type + SynthesizedClosureFunction is the
    /// single source of truth.

    /// Stage 13.19: Stack of (continue_target, break_target) block IDs for
    /// the enclosing loops. Used by `break` and `continue` to emit the
    /// correct branch target. Empty when not inside a loop.
    ///
    /// Per §16: this is lowering context state, not MIR data. The resulting
    /// MIR has the correct Goto instructions; the stack is just how we track
    /// which loop we're currently inside.
    pub loop_stack: Vec<(BasicBlockId, BasicBlockId)>,
    /// Stage 14.24: Result locals for each enclosing loop. Used by `break expr`
    /// to assign the break value to the loop's result local before jumping to
    /// the break target. Parallel to `loop_stack` — push/pop together.
    pub loop_result_locals: Vec<crate::mir::place::LocalId>,
    /// Stage 14.30: Type errors collected during MIR lowering. These are
    /// merged into the driver's CompileErrors after lowering completes.
    /// Used for "报错 > 静默" — emit errors instead of silent placeholders.
    pub type_errors: Vec<crate::typeck::TypeError>,
    /// Stage 30.6 (v0.14 TD-DROP-SCOPE-TIMING): Scope stack for per-block
    /// `StorageDead` emission. Each entry is a `Vec<LocalId>` of locals
    /// created directly in that scope (via `new_local` /
    /// `new_local_with_mut`). When `lower_block` exits, it pops the top
    /// entry and emits `StorageDead` for each LocalId in reverse order.
    ///
    /// This replaces the old "all locals die at function return" sweep
    /// in `body_lower.rs`, fixing the soundness gap where block-scoped
    /// locals with `Drop` were dropped too late (after observable side
    /// effects that followed the block).
    ///
    /// Per §1.0 原則 9 (正确 > 妥协): root-cause fix is scope tracking,
    /// not a conservative approximation.
    /// Per §1.0 原則 6 (通解 > 特解): one mechanism (scope_stack) handles
    /// all block scopes — nested, if/else, loop bodies, etc.
    /// Per §23: `scope_stack` follows `<noun>_<noun>` pattern.
    pub scope_stack: Vec<Vec<crate::mir::place::LocalId>>,
    /// Stage 15.4 (perf): Lazy cache for `query_method_return_type`.
    /// Maps method DefId → return type. Populated on first lookup,
    /// reused for all subsequent lookups of the same method.
    /// Eliminates O(n) HIR scan per method call.
    pub method_return_type_cache: std::cell::RefCell<
        std::collections::HashMap<crate::hir::DefId, Option<crate::mir::ty::Ty>>,
    >,
    /// Stage 16.13 (Task 10 Step 1): Synthesized closure `call` functions.
    /// Each entry represents a closure literal that will get a synthesized
    /// `call` function in Strategy A (rustc-style). The function is built
    /// during MIR lowering and emitted as a separate MIR body by codegen
    /// (Step 2+).
    ///
    /// Keyed by the closure's DefId (allocated via `allocate_closure_def_id`).
    ///
    /// Per §16: side-table carries HIR-derived data downstream. Codegen
    /// reads this without needing HIR access.
    /// Per §23: `synthesized_closure_functions` follows `<adj>_<noun>_<noun>`
    /// pattern.
    pub synthesized_closure_functions:
        std::collections::HashMap<crate::hir::DefId, SynthesizedClosureFunction>,
    /// Stage 16.13: Counter for allocating unique closure DefIds.
    /// Uses the reserved range `CLOSURE_DEF_ID_BASE` downward.
    pub closure_def_id_counter: u32,
    /// Stage 18.105 (S6 fix): Generic type parameters of the function being
    /// lowered. Used by `lower_path_generic_args` to resolve bare type
    /// parameters (e.g., `T` in `Box<T>`) to `Param(N)`.
    /// Empty for non-generic functions.
    /// Per §16: pre-computed from HIR generics, sunk as data.
    pub generic_params: Vec<crate::mir::ty::ParamTy>,
    /// Stage 32.3 (TD-PRELUDE-MONO-ORDER): The DefId of the body's owner
    /// (the fn being lowered). Used by `resolve_trait_method` to look up
    /// the enclosing impl block's generic param trait bounds when resolving
    /// methods on `TyKind::Param(N)` receivers (e.g., `self.x.f()` where
    /// `x: X` and `X: T`).
    ///
    /// `None` when not set (e.g., test contexts that construct MirLowerCtxt
    /// without a body). In that case, Param-typed method resolution returns
    /// None (no error — caller falls through to error path).
    ///
    /// Per §16: pre-computed from body.hir_id.owner, sunk as data.
    /// Per §1.0 原则 3 (显式 > 隐式): owner is explicit, not inferred.
    /// Per §1.0 原则 10 (唯一可信数据源): the body is the source of truth
    /// for the owner DefId.
    pub owner_def_id: Option<crate::hir::DefId>,
}

/// Stage 16.13 (Task 10 Step 1): A synthesized `call` function for a closure.
///
/// Represents the metadata needed to synthesize a `call` function for a
/// closure literal. The actual MIR body synthesis is deferred to Step 2.
///
/// Per §23: `SynthesizedClosureFunction` follows `<Adj><Noun>` pattern.
#[derive(Clone, Debug)]
pub struct SynthesizedClosureFunction {
    /// The closure's DefId (unique per closure literal, allocated via
    /// `allocate_closure_def_id`).
    pub def_id: crate::hir::DefId,
    /// The closure's parameters (HIR). At the call site, each param is
    /// bound to the corresponding call argument.
    pub params: Vec<crate::hir::HirParam>,
    /// The closure's body expression (HIR). Lowered into the synthesized
    /// function's MIR in Step 2.
    pub body: Box<crate::hir::HirExpr>,
    /// The capture info: (HirId of captured binding, field index in
    /// closure struct, field type, field mutability). Used to extract
    /// captures from `self` in the synthesized function.
    ///
    /// Stage 16.31 (通解 — capture mutability): The 4th element is the
    /// mutability of the captured variable in the outer scope. This is
    /// propagated to the extract local in the closure MIR body so that
    /// borrowck doesn't flag `x += 1` (where `x` is a captured `mut`)
    /// as "cannot assign twice to immutable variable".
    pub captures: Vec<(
        crate::hir::HirId,
        u32,
        crate::mir::ty::Ty,
        crate::mir::ty::Mutability,
    )>,
    /// The closure struct type (for the `self` parameter).
    pub closure_struct_ty: crate::mir::ty::Ty,
    /// The function name for codegen (e.g., "closure_call_fn_0").
    /// Unique per closure literal.
    pub fn_name: String,
}

// Stage 16.34 (Task 10 Step 5 — cleanup): Removed `ClosureBodyInfo` struct.
// The `closure_bodies` side-table is no longer needed — the closure
// dispatch at the call site uses the type-based check
// (`TyKind::Closure(_, _)`), and the `SynthesizedClosureFunction`
// metadata carries all the information needed for the synthesized
// `call` function.
//
// Per §1.0 原則 5 "去除兼容思维": dead struct removed.
// Per §23 rule 5 (DRY): `SynthesizedClosureFunction` is the single
// source of truth for closure metadata.

impl<'a> MirLowerCtxt<'a> {
    pub fn new(interner: &'a Rodeo, span: Span) -> Self {
        let mut mir = MirBody::new(span);
        let current_block = mir.new_block();
        Self {
            interner,
            mir,
            local_map: std::collections::HashMap::new(),
            current_block,
            unify: UnificationTable::new(),
            hir: None,
            dyn_trait_plan: None,
            // Stage 18.262 (TD-TUPLE-CTOR-CALL-ARG Phase 2e): default
            // None — set by `set_fn_sigs` before lowering begins when
            // the driver has pre-built fn_sig_table.
            fn_sigs: None,
            resolver: None,
            loop_stack: Vec::new(),
            loop_result_locals: Vec::new(),
            type_errors: Vec::new(),
            // Stage 30.6 (v0.14 TD-DROP-SCOPE-TIMING): empty scope stack.
            scope_stack: Vec::new(),
            method_return_type_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
            synthesized_closure_functions: std::collections::HashMap::new(),
            closure_def_id_counter: 0,
            generic_params: Vec::new(),
            // Stage 32.3: owner_def_id is set by body_lower after construction.
            owner_def_id: None,
        }
    }

    /// Stage 18.262 (TD-TUPLE-CTOR-CALL-ARG Phase 2e): Set the pre-built
    /// `fn_sigs` map for call-arg expected-ty propagation.
    ///
    /// After calling this, `lower_call_expr` can look up the callee's
    /// `sig.inputs[i]` and thread the expected arg type into each arg's
    /// `lower_expr_to_operand`. This closes the soundness hole where
    /// `take_holder(Holder(true))` (where `fn take_holder(h: Holder<i32>)`)
    /// silently accepted type mismatches.
    ///
    /// Per §11.2 (allowed cross-stage access — pre-computed data
    /// contract): fn_sigs is built upstream by the driver.
    /// Per §23: `set_fn_sigs` follows `<verb>_<noun>` pattern.
    pub fn set_fn_sigs(
        &mut self,
        fn_sigs: &'a std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
    ) {
        self.fn_sigs = Some(fn_sigs);
    }

    /// Stage 16.85: Set the resolver for rich error messages.
    ///
    /// After calling this, "no method found" errors use
    /// `type_to_string_with_resolver` to show actual type names.
    ///
    /// Per §23: `set_resolver` follows `<verb>_<noun>` pattern.
    pub fn set_resolver(&mut self, resolver: &'a crate::traits::TraitResolver) {
        self.resolver = Some(resolver);
    }

    /// Stage 16.85: Format a `Ty` for error messages, using resolver if available.
    ///
    /// Per §1.0 原則 3 "显式 > 隐式": user-facing type names are explicit.
    /// Per §23: `format_ty` follows `<verb>_<noun>` pattern.
    /// Stage 18.100 (TD-DUP2): delegates to `mir::ty::format_ty_with_optional_resolver`
    /// (single source of truth — was duplicated in 3 modules).
    pub fn format_ty(&self, ty: &Ty) -> String {
        crate::mir::ty::format_ty_with_optional_resolver(ty, self.resolver, Some(self.interner))
    }

    /// Stage 16.29 (通解): Construct a MirLowerCtxt with an EXISTING
    /// UnificationTable. Used by `build_synthesized_closure_mir_body` to
    /// share the unify table with the main body.
    ///
    /// This is the key fix for the typeck gap: the closure_struct_ty and
    /// cap_tys have Infer vars from the main body's unify table. If we
    /// create a fresh unify table for the closure MIR body, these Infer
    /// vars collide with the closure's fresh Infer vars (same TyVid
    /// values, different tables), causing cycles in the unify table
    /// during typeck.
    ///
    /// By sharing the unify table, all Infer vars are in the same
    /// namespace. typeck on the closure MIR body can correctly resolve
    /// the closure_struct_ty's Infer vars (which were created during
    /// main body lowering).
    ///
    /// Per §1.0 原則 6 "通用 > 特例": one unify table for the whole
    /// compilation unit (main body + all closures).
    /// Per §1.0 原則 9 "正确 > 妥协": fix the root cause (unify table
    /// isolation), not the symptom (cycle detection).
    pub fn new_with_unify(
        interner: &'a Rodeo,
        span: Span,
        unify: UnificationTable,
        closure_def_id_counter: u32,
    ) -> Self {
        let mut mir = MirBody::new(span);
        let current_block = mir.new_block();
        Self {
            interner,
            mir,
            local_map: std::collections::HashMap::new(),
            current_block,
            unify,
            hir: None,
            dyn_trait_plan: None,
            // Stage 18.262 (Phase 2e): default None — closure bodies
            // built via new_with_unify can call set_fn_sigs if needed.
            fn_sigs: None,
            resolver: None,
            loop_stack: Vec::new(),
            loop_result_locals: Vec::new(),
            type_errors: Vec::new(),
            // Stage 30.6 (v0.14 TD-DROP-SCOPE-TIMING): empty scope stack.
            scope_stack: Vec::new(),
            method_return_type_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
            synthesized_closure_functions: std::collections::HashMap::new(),
            closure_def_id_counter,
            generic_params: Vec::new(),
            // Stage 32.3: owner_def_id set by caller (body_lower).
            owner_def_id: None,
        }
    }

    /// Stage 16.29: Getter for closure_def_id_counter (to propagate to
    /// nested closure MIR body building).
    pub fn closure_def_id_counter(&self) -> u32 {
        self.closure_def_id_counter
    }

    /// Stage 16.13 (Task 10 Step 1): Allocate a unique DefId for a closure.
    ///
    /// Uses a reserved range (`CLOSURE_DEF_ID_BASE` downward) to avoid
    /// collision with user-defined items and builtin traits.
    ///
    /// Per §23: `allocate_closure_def_id` follows `<verb>_<noun>_<noun>`
    /// pattern.
    pub fn allocate_closure_def_id(&mut self) -> crate::hir::DefId {
        // CLOSURE_DEF_ID_BASE is u32::MAX - 1000, leaving room for builtin
        // traits (u32::MAX, u32::MAX-1, ...) above and user items below.
        const CLOSURE_DEF_ID_BASE: u32 = u32::MAX - 1000;
        let id = CLOSURE_DEF_ID_BASE - self.closure_def_id_counter;
        self.closure_def_id_counter += 1;
        crate::hir::DefId::new(id)
    }

    /// Stage 16.13 (Task 10 Step 1): Register a synthesized closure function.
    ///
    /// Stores the closure metadata in `synthesized_closure_functions` for
    /// later MIR body synthesis (Step 2) and codegen (Step 4).
    ///
    /// Per §23: `register_synthesized_closure_function` follows
    /// `<verb>_<adj>_<noun>_<noun>` pattern.
    pub fn register_synthesized_closure_function(&mut self, func: SynthesizedClosureFunction) {
        self.synthesized_closure_functions.insert(func.def_id, func);
    }

    /// Allocate a fresh inference type variable and return it as a Ty.
    /// Each call produces a unique TyVid — no sharing.
    ///
    /// Stage 15.29: Uses `from_kind_raw` to bypass the TypeInterner —
    /// inference variables are always unique (unique TyVid), so interning
    /// them wastes memory and pollutes the dedup map.
    pub fn fresh_infer_ty(&mut self, _span: Span) -> Ty {
        let vid = self.unify.new_ty_var();
        Ty::from_kind_raw(TyKind::Infer(InferVar::TyVar(vid)))
    }

    /// Allocate a fresh integer inference variable.
    pub fn fresh_int_ty(&mut self, _span: Span) -> Ty {
        let vid = self.unify.new_int_var();
        Ty::from_kind_raw(TyKind::Infer(InferVar::IntVar(vid)))
    }

    /// Allocate a fresh float inference variable.
    pub fn fresh_float_ty(&mut self, _span: Span) -> Ty {
        let vid = self.unify.new_float_var();
        Ty::from_kind_raw(TyKind::Infer(InferVar::FloatVar(vid)))
    }

    /// Allocate a new local variable for a HirId.
    pub fn new_local(
        &mut self,
        hir_id: HirId,
        ty: Ty,
        name: Option<crate::lexer::Symbol>,
    ) -> LocalId {
        let local_id = self.mir.new_local(ty, name, Span::DUMMY);
        self.local_map.insert(hir_id, local_id);
        // Stage 30.6 (v0.14 TD-DROP-SCOPE-TIMING): Track this local in
        // the current scope (if any) so `lower_block` can emit StorageDead
        // at scope end. Locals created outside any scope (e.g., return
        // local + params in body_lower) are not tracked — they're handled
        // by the function-end sweep for parameters.
        if let Some(scope) = self.scope_stack.last_mut() {
            scope.push(local_id);
        }
        local_id
    }

    /// Allocate a new local with explicit mutability and register it
    /// in the local_map. G5 fix: used by `let mut x = ...` lowering.
    pub fn new_local_with_mut(
        &mut self,
        hir_id: HirId,
        ty: Ty,
        name: Option<crate::lexer::Symbol>,
        mutability: crate::mir::ty::Mutability,
    ) -> LocalId {
        let local_id = self
            .mir
            .new_local_with_mut(ty, name, Span::DUMMY, mutability);
        self.local_map.insert(hir_id, local_id);
        // Stage 30.6: Track this local in the current scope (if any).
        if let Some(scope) = self.scope_stack.last_mut() {
            scope.push(local_id);
        }
        local_id
    }

    /// Look up the MIR LocalId for a HirId.
    pub fn find_local(&self, hir_id: HirId) -> Option<LocalId> {
        self.local_map.get(&hir_id).copied()
    }

    /// Stage 5.76: Attach a pre-built `DynTraitMIRPlan` to this lowering
    /// context.
    ///
    /// Subsequent `HirExprKind::MethodCall` lowering (Stage 5.77+) will
    /// query this plan via `find_dyn_trait_method_call_in_plan()` to
    /// retrieve the vtable slot index + param count for a dyn Trait
    /// method call.
    ///
    /// Calling this twice overwrites the previously-attached plan — the
    /// last call wins. There is intentionally no `unset_dyn_trait_plan`
    /// method: once a plan is attached, it stays for the lifetime of
    /// the lowering context (consistent with `hir` field semantics).
    ///
    /// Per §16: the plan is built **upstream** (by the driver, using
    /// `build_dyn_trait_mir_plan_from_resolver()`) and passed in as a
    /// read-only value. `MirLowerCtxt` does not own a `TraitResolver`.
    ///
    /// Per API-naming-standard §3 + §8.1: `set_dyn_trait_plan` follows
    /// the `<verb>_<noun>_<noun>_<noun>` pattern (setter verb `set_`
    /// prefix per Rust convention).
    pub fn set_dyn_trait_plan(&mut self, plan: DynTraitMIRPlan) {
        self.dyn_trait_plan = Some(plan);
    }

    /// Stage 5.76: Read-only access to the attached `DynTraitMIRPlan`, if
    /// any.
    ///
    /// Returns `None` when no plan has been attached via
    /// `set_dyn_trait_plan()`. In that case, the `HirExprKind::MethodCall`
    /// branch (Stage 5.77+) falls back to the legacy placeholder path.
    ///
    /// Per API-naming-standard §3 + §8.1: `dyn_trait_plan` follows the
    /// `<noun>_<noun>_<noun>` pattern (Rust getter convention — no `get_`
    /// prefix per C-GETTER convention in rust-api-guidelines).
    pub fn dyn_trait_plan(&self) -> Option<&DynTraitMIRPlan> {
        self.dyn_trait_plan.as_ref()
    }

    /// Stage 15.6 (perf): Cached method return type lookup.
    ///
    /// Given a method's DefId, returns the method's return type as a MIR Ty.
    /// Results are memoized in `method_return_type_cache` (a `RefCell<HashMap>`),
    /// so repeated lookups of the same DefId are O(1) after the first call.
    ///
    /// Returns `None` when:
    /// - `self.hir` is `None` (lowering context has no HIR attached), OR
    /// - The DefId doesn't resolve to any method (impl method, free fn, or
    ///   trait default body), OR
    /// - The return type can't be lowered.
    ///
    /// Caching `None` results is intentional — it avoids re-scanning HIR
    /// for known-unresolvable DefIds (e.g. primitives without methods).
    ///
    /// Per §23 (API Naming): public method follows `<verb>_<noun>` pattern.
    /// Per §1.0 原则 6 "通用 > 特例": one cache handles all owner kinds.
    /// Per §1.0 原则 3 "显式 > 隐式": caching is explicit in the method body.
    ///
    /// # Why now (Stage 15.6)
    ///
    /// Stage 15.4 added the cache field but couldn't activate it because Ty
    /// carried a `Span`, making equal-Ty-different-Span lookups cache-miss.
    /// Stage 15.5 removed Span from Ty (foundational for interning), unblocking
    /// activation. Per `docs/lang-design/19-ty-interning.md`.
    pub fn query_method_return_type(
        &self,
        method_def_id: crate::hir::DefId,
    ) -> Option<crate::mir::ty::Ty> {
        // Fast path: cache hit.
        if let Some(cached) = self.method_return_type_cache.borrow().get(&method_def_id) {
            return cached.clone();
        }
        // Slow path: scan HIR, memoize result (including None).
        let result = self.hir.and_then(|hir| {
            method_resolution::query_method_return_type_uncached(hir, method_def_id)
        });
        self.method_return_type_cache
            .borrow_mut()
            .insert(method_def_id, result.clone());
        result
    }

    /// Allocate a fresh basic block and return its ID.
    pub fn new_block(&mut self) -> BasicBlockId {
        self.mir.new_block()
    }

    /// Set the terminator of the current block and switch to a new block.
    pub fn terminate(&mut self, terminator: Terminator) {
        self.mir.block_mut(self.current_block).terminator = terminator;
    }

    /// Stage 14.112: Convenience method — terminate with a TerminatorKind
    /// and DUMMY span. Use `terminate()` with explicit span for debug info.
    pub fn terminate_kind(&mut self, kind: TerminatorKind) {
        self.mir.block_mut(self.current_block).terminator =
            Terminator::new(kind, crate::session::Span::DUMMY);
    }

    /// Stage 14.112: Convenience method — terminate with a TerminatorKind
    /// and explicit span (for debug info).
    pub fn terminate_kind_span(&mut self, kind: TerminatorKind, span: crate::session::Span) {
        self.mir.block_mut(self.current_block).terminator = Terminator::new(kind, span);
    }

    /// Stage 13.21: Check if the current block is already terminated (has a
    /// non-Unreachable terminator). Used by `if`/`match` lowering to skip
    /// the continuation Goto when the then/else block ends with `return`,
    /// `break`, or `continue`.
    ///
    /// Per §16: this is lowering context state, not MIR data.
    pub fn is_terminated(&self) -> bool {
        !matches!(
            self.mir.block(self.current_block).terminator.kind,
            TerminatorKind::Unreachable
        )
    }

    /// Set the terminator of the current block and switch to `next`.
    pub fn terminate_and_goto(&mut self, terminator: Terminator, next: BasicBlockId) {
        self.mir.block_mut(self.current_block).terminator = terminator;
        self.current_block = next;
    }

    /// Stage 14.112: Convenience — terminate with TerminatorKind + DUMMY span,
    /// then switch to `next`.
    pub fn terminate_kind_and_goto(&mut self, kind: TerminatorKind, next: BasicBlockId) {
        self.mir.block_mut(self.current_block).terminator =
            Terminator::new(kind, crate::session::Span::DUMMY);
        self.current_block = next;
    }

    /// Push a statement onto the current block.
    pub fn push_assign(&mut self, place: Place, rvalue: Rvalue, span: Span) {
        self.mir
            .block_mut(self.current_block)
            .statements
            .push(Statement {
                kind: StatementKind::Assign(Box::new((place, rvalue))),
                span,
            });
    }

    /// Push a Nop statement (for debugging / placeholders).
    pub fn push_nop(&mut self, span: Span) {
        self.mir
            .block_mut(self.current_block)
            .statements
            .push(Statement {
                kind: StatementKind::Nop,
                span,
            });
    }

    /// Stage 18.229 (v0.2.5e): Push an arbitrary `StatementKind` onto the
    /// current block. Used by `lower_vec_push_intrinsic` to emit
    /// `StatementKind::Store` (MIR intrinsic ops, Stage 18.226).
    ///
    /// Per §1.0 原則 6 (通解>特例): one method for all non-Assign statements.
    /// Per §10 DRY: reuses the same block_mut pattern as push_assign.
    pub fn push_statement(&mut self, stmt: Statement, _span: crate::session::Span) {
        self.mir.block_mut(self.current_block).statements.push(stmt);
    }

    /// Allocate a temporary local and assign the given rvalue to it.
    pub fn eval_rvalue_to_temp(&mut self, rvalue: Rvalue, ty: Ty, span: Span) -> LocalId {
        let temp = self.mir.new_local(ty, None, span);
        self.push_assign(Place::local(temp, span), rvalue, span);
        // Stage 30.6 (v0.14 TD-DROP-SCOPE-TIMING): Track this temp in the
        // current scope (if any) so it gets StorageDead'd at scope end.
        if let Some(scope) = self.scope_stack.last_mut() {
            scope.push(temp);
        }
        temp
    }

    /// Stage 30.6 (v0.14 TD-DROP-SCOPE-TIMING): Allocate a temporary local
    /// (no HirId) and track it in the current scope for per-block
    /// `StorageDead` emission.
    ///
    /// Use this instead of `cx.mir.new_local(...)` for temps created during
    /// expression lowering. This ensures the temp gets `StorageDead` at the
    /// correct scope end (not function end).
    ///
    /// Per §23: `new_temp` follows `<adj>_<noun>` pattern (constructor).
    /// Per §1.0 原則 6 (通解 > 特解): one helper for all temp allocations.
    pub fn new_temp(&mut self, ty: Ty, span: Span) -> LocalId {
        let local_id = self.mir.new_local(ty, None, span);
        if let Some(scope) = self.scope_stack.last_mut() {
            scope.push(local_id);
        }
        local_id
    }

    /// Convert a HIR LitKind to a MIR Const.
    ///
    /// Takes `&mut self` because unsuffixed integer/float literals
    /// allocate fresh inference variables (IntVar/FloatVar) from the
    /// unification table.
    pub fn lit_to_const(&mut self, lit: &HirLitKind) -> (Const, Ty) {
        match lit {
            HirLitKind::Bool(b) => (
                Const {
                    ty: Ty::new(TyKind::Bool, Span::DUMMY),
                    val: ConstVal::Bool(*b),
                },
                Ty::new(TyKind::Bool, Span::DUMMY),
            ),
            HirLitKind::Int(n, suffix) => {
                // Stage 18.445 (§20 audit round 10): Check literal value
                // fits in the target type. `let x: i8 = 200` should error
                // because 200 > i8::MAX (127).
                //
                // Per §1.0 原則 4 (报错 > 静默): out-of-range literals must
                // be reported, not silently truncated.
                // Per §1.0 原則 6 (通解 > 特解): one check covers all int types.
                if let Some(int_ty) = suffix {
                    let val = *n as i128;
                    let max = match int_ty {
                        ast::IntTy::I8 => i8::MAX as i128,
                        ast::IntTy::I16 => i16::MAX as i128,
                        ast::IntTy::I32 => i32::MAX as i128,
                        ast::IntTy::I64 => i64::MAX as i128,
                        ast::IntTy::I128 => i128::MAX,
                        ast::IntTy::Isize => isize::MAX as i128,
                    };
                    let min = match int_ty {
                        ast::IntTy::I8 => i8::MIN as i128,
                        ast::IntTy::I16 => i16::MIN as i128,
                        ast::IntTy::I32 => i32::MIN as i128,
                        ast::IntTy::I64 => i64::MIN as i128,
                        ast::IntTy::I128 => i128::MIN,
                        ast::IntTy::Isize => isize::MIN as i128,
                    };
                    if val > max || val < min {
                        self.type_errors.push(crate::typeck::TypeError::new(
                            format!(
                                "literal out of range for `{:?}`: value {} exceeds range [{}, {}]",
                                int_ty, val, min, max
                            ),
                            Span::DUMMY,
                        ));
                    }
                }
                // If the literal has a suffix, use the exact type.
                // If no suffix, use an IntVar so the literal can unify
                // with whatever type the context expects (i32, u64, etc).
                // This is the standard Rust behavior — unsuffixed integer
                // literals defer to inference.
                let ty_kind = match suffix {
                    Some(ast::IntTy::I8) => TyKind::Int(ast::IntTy::I8),
                    Some(ast::IntTy::I16) => TyKind::Int(ast::IntTy::I16),
                    Some(ast::IntTy::I32) => TyKind::Int(ast::IntTy::I32),
                    Some(ast::IntTy::I64) => TyKind::Int(ast::IntTy::I64),
                    Some(ast::IntTy::I128) => TyKind::Int(ast::IntTy::I128),
                    Some(ast::IntTy::Isize) => TyKind::Int(ast::IntTy::Isize),
                    None => {
                        // No suffix — return an IntVar (deferred to inference).
                        // The literal value is still preserved as ConstVal::Int.
                        let var = self.unify.new_int_var();
                        TyKind::Infer(InferVar::IntVar(var))
                    }
                };
                let ty = Ty::new(ty_kind, Span::DUMMY);
                (
                    Const {
                        ty: ty.clone(),
                        val: ConstVal::Int(*n),
                    },
                    ty,
                )
            }
            HirLitKind::Uint(n, suffix) => {
                // Stage 18.445 (§20 audit round 10): Check literal value
                // fits in the target unsigned type.
                if let Some(uint_ty) = suffix {
                    let max = match uint_ty {
                        ast::UintTy::U8 => u8::MAX as u128,
                        ast::UintTy::U16 => u16::MAX as u128,
                        ast::UintTy::U32 => u32::MAX as u128,
                        ast::UintTy::U64 => u64::MAX as u128,
                        ast::UintTy::U128 => u128::MAX,
                        ast::UintTy::Usize => usize::MAX as u128,
                    };
                    if *n > max {
                        self.type_errors.push(crate::typeck::TypeError::new(
                            format!(
                                "literal out of range for `{:?}`: value {} exceeds max {}",
                                uint_ty, n, max
                            ),
                            Span::DUMMY,
                        ));
                    }
                }
                let ty_kind = match suffix {
                    Some(ast::UintTy::U8) => TyKind::Uint(ast::UintTy::U8),
                    Some(ast::UintTy::U16) => TyKind::Uint(ast::UintTy::U16),
                    Some(ast::UintTy::U32) => TyKind::Uint(ast::UintTy::U32),
                    Some(ast::UintTy::U64) => TyKind::Uint(ast::UintTy::U64),
                    Some(ast::UintTy::U128) => TyKind::Uint(ast::UintTy::U128),
                    Some(ast::UintTy::Usize) => TyKind::Uint(ast::UintTy::Usize),
                    None => {
                        // No suffix — defer to inference via IntVar.
                        let var = self.unify.new_int_var();
                        TyKind::Infer(InferVar::IntVar(var))
                    }
                };
                let ty = Ty::new(ty_kind, Span::DUMMY);
                (
                    Const {
                        ty: ty.clone(),
                        val: ConstVal::Uint(*n),
                    },
                    ty,
                )
            }
            HirLitKind::Float(f, suffix) => {
                // If the literal has a suffix, use the exact type.
                // If no suffix, use a FloatVar so it can unify with f32 or f64.
                // Default (if no constraint) is f64 (matching Rust).
                let ty_kind = match suffix {
                    Some(ast::FloatTy::F32) => TyKind::Float(ast::FloatTy::F32),
                    Some(ast::FloatTy::F64) => TyKind::Float(ast::FloatTy::F64),
                    None => {
                        let var = self.unify.new_float_var();
                        TyKind::Infer(InferVar::FloatVar(var))
                    }
                };
                let ty = Ty::new(ty_kind, Span::DUMMY);
                (
                    Const {
                        ty: ty.clone(),
                        val: ConstVal::Float(f.to_bits()),
                    },
                    ty,
                )
            }
            HirLitKind::Char(c) => (
                Const {
                    ty: Ty::new(TyKind::Char, Span::DUMMY),
                    val: ConstVal::Char(*c),
                },
                Ty::new(TyKind::Char, Span::DUMMY),
            ),
            HirLitKind::Str(sym) => {
                // Stage 3.42: String literals have type &'static str,
                // not str (which is unsized). Was: TyKind::Str — caused
                // type mismatches when passing strings to functions
                // expecting &str, and string comparison failed.
                // Per §15: root-cause fix (correct the type at the source).
                let str_ty = Ty::new(TyKind::Str, Span::DUMMY);
                let ref_str_ty = Ty::new(
                    TyKind::Ref(
                        Region::Static,
                        crate::mir::ty::Mutability::Immutable,
                        Box::new(str_ty),
                    ),
                    Span::DUMMY,
                );
                (
                    Const {
                        ty: ref_str_ty.clone(),
                        val: ConstVal::Str(*sym),
                    },
                    ref_str_ty,
                )
            }
            HirLitKind::ByteStr(sym) => {
                // `b"..."` has type `&'static [u8; N]` in Rust, but Landin
                // models it as `&'static [u8]` (a reference to a slice).
                //
                // Stage 3.49 (L13 closure): the type must be `Ref(_, _, Slice(u8))`
                // so codegen produces a fat pointer `{ i8*, i64 }` (data ptr + length).
                // Was (Stage 2.4d-3.48): produced `Slice(u8)` directly, which codegen
                // mapped to a thin `i8*` pointer — losing the length and producing
                // invalid IR when `ConstVal::Str` tried to `insertvalue` into it.
                let elem_ty = Ty::new(TyKind::Uint(ast::UintTy::U8), Span::DUMMY);
                let slice_ty = Ty::new(TyKind::Slice(Box::new(elem_ty)), Span::DUMMY);
                let ref_slice_ty = Ty::new(
                    TyKind::Ref(
                        crate::mir::ty::Region::Erased,
                        crate::mir::ty::Mutability::Immutable,
                        Box::new(slice_ty),
                    ),
                    Span::DUMMY,
                );
                (
                    Const {
                        ty: ref_slice_ty.clone(),
                        // Reuse Str variant — codegen will interpret
                        // the symbol as bytes when the type is Ref(_, _, Slice(u8)).
                        val: ConstVal::Str(*sym),
                    },
                    ref_slice_ty,
                )
            }
            HirLitKind::Byte(b) => (
                Const {
                    ty: Ty::new(TyKind::Uint(ast::UintTy::U8), Span::DUMMY),
                    val: ConstVal::Uint(*b as u128),
                },
                Ty::new(TyKind::Uint(ast::UintTy::U8), Span::DUMMY),
            ),
        }
    }

    /// Convert a HIR BinOp to a MIR BinOp.
    ///
    /// **Note**: `HirBinOp::And` and `HirBinOp::Or` (logical `&&`/`||`)
    /// are NOT real binary ops in MIR — they must be lowered to control
    /// flow (short-circuit evaluation) via `lower_short_circuit`. If this
    /// function is called with `And` or `Or`, it emits an internal warning
    /// and returns `BinOp::BitAnd` as a best-effort fallback (Stage 18.76 P1-B:
    /// was panic!, now graceful fallback).
    pub fn lower_bin_op(op: HirBinOp) -> BinOp {
        match op {
            HirBinOp::Add => BinOp::Add,
            HirBinOp::Sub => BinOp::Sub,
            HirBinOp::Mul => BinOp::Mul,
            HirBinOp::Div => BinOp::Div,
            HirBinOp::Rem => BinOp::Rem,
            HirBinOp::BitAnd => BinOp::BitAnd,
            HirBinOp::BitOr => BinOp::BitOr,
            HirBinOp::BitXor => BinOp::BitXor,
            HirBinOp::Shl => BinOp::Shl,
            HirBinOp::Shr => BinOp::Shr,
            HirBinOp::Eq => BinOp::Eq,
            HirBinOp::Ne => BinOp::Ne,
            HirBinOp::Lt => BinOp::Lt,
            HirBinOp::Le => BinOp::Le,
            HirBinOp::Gt => BinOp::Gt,
            HirBinOp::Ge => BinOp::Ge,
            // Stage 18.76 P1-B: Logical and/or must be lowered to control
            // flow, not BitOp. Caller must route them through lower_short_circuit.
            // Previously this panicked, crashing the compiler on caller bugs.
            // Now returns a best-effort fallback (BitAnd) so compilation
            // can continue and other errors can be reported.
            // Per §1.0 原則 4 "报错 > 静默": this is a best-effort fallback
            // for an internal compiler error path — the user will see
            // incorrect results, but the compiler won't crash.
            HirBinOp::And | HirBinOp::Or => {
                // Stage 18.78 P1 (N9): Use Display instead of Debug format.
                eprintln!(
                    "internal warning: lower_bin_op called with And/Or — \
                     caller must route And/Or to lower_short_circuit. \
                     Using BitAnd as fallback."
                );
                BinOp::BitAnd
            }
        }
    }

    /// Convert a HIR UnaryOp to a MIR UnOp.
    ///
    /// **Note**: `HirUnaryOp::Deref` is NOT a real unary op in MIR —
    /// it is a projection (`*p` reads the place `Projection(p, Deref)`).
    /// Callers must handle `Deref` separately before calling this function.
    /// This function only converts `Neg`/`Not`; if passed `Deref` it returns
    /// `Not` as a best-effort fallback (Stage 18.76: was panic!).
    pub fn lower_un_op(op: HirUnaryOp) -> UnOp {
        match op {
            HirUnaryOp::Neg => UnOp::Neg,
            HirUnaryOp::Not => UnOp::Not,
            // Stage 18.76 P1-B: Previously panicked, crashing the compiler.
            // Now returns Not as best-effort fallback so compilation
            // can continue and other errors can be reported.
            HirUnaryOp::Deref => {
                eprintln!(
                    "internal warning: lower_un_op called with Deref — \
                     caller must route Deref to lower_deref_expr. \
                     Using Not as fallback."
                );
                UnOp::Not
            }
        }
    }
}

/// Lower a HIR body to MIR.
///
/// This is the main entry point for HIR→MIR lowering. It takes a
/// HIR Body (expression tree) and produces a MirBody (CFG).
///
/// The returned MirBody has:
/// - LocalId(0) as the return value placeholder
/// - LocalId(1..N) for fn params
/// - LocalId(N+1..) for local variables and temporaries
/// - At least 1 basic block (the entry block)
///
/// StorageLive/StorageDead markers are emitted for:
/// - The return local (StorageLive at entry, no StorageDead — lives until Return)
/// - Each fn param (StorageLive at entry, no StorageDead — lives until Return)
/// - Each `let` binding (StorageLive at the `let`, StorageDead at scope end — Stage 3)
///
/// Stage 3.30 (per §16): now takes `hir: &HirCrate` so MIR lower can resolve
/// ADT field types at lowering time and store them in `AggregateKind::Adt`'s
/// `field_tys` field. This is the "data sink" approach — codegen reads the
/// field types from MIR instead of re-querying HIR.
#[cfg(test)]
mod stage15_92_tests {
    use super::*;
    use crate::ast::{Ident, Lifetime, Mutability};
    use crate::hir::{HirTy, HirTyKind};
    use crate::lexer::Symbol;
    use crate::mir::ty::{Region, TyKind};
    use crate::session::Span;

    /// Stage 15.92: Verify that explicit lifetimes with the same name
    /// share the same RegionVid.
    #[test]
    fn explicit_lifetime_deduplication() {
        // Create a type: &'a i32 with lifetime name "a"
        let lifetime_a = Lifetime {
            ident: Ident::new(Symbol::default(), Span::DUMMY),
            span: Span::DUMMY,
        };
        // We can't easily construct a Symbol for "a" without an interner,
        // but we can test the logic with default Symbol (both use the same).
        let inner = HirTy {
            hir_id: crate::hir::HirId::new(crate::hir::DefId(0), crate::hir::ItemLocalId(0)),
            kind: HirTyKind::Int(crate::ast::IntTy::I32),
            inferred: None,
            span: Span::DUMMY,
        };
        let ref_ty = HirTy {
            hir_id: crate::hir::HirId::new(crate::hir::DefId(0), crate::hir::ItemLocalId(1)),
            kind: HirTyKind::Ref(
                Some(lifetime_a.clone()),
                Mutability::Immutable,
                Box::new(inner.clone()),
            ),
            inferred: None,
            span: Span::DUMMY,
        };

        let mut region_counter = 0u32;
        let mut lifetime_map = std::collections::HashMap::new();

        // Lower the first reference — should get vid 0.
        let ty1 = lower_hir_ty_to_mir_ty_with_lifetimes(
            &ref_ty,
            &mut region_counter,
            &mut lifetime_map,
            &[],
        );
        let vid1 = match &ty1.kind {
            TyKind::Ref(Region::Var(vid), _, _) => *vid,
            _ => panic!("expected Ref with Region::Var"),
        };

        // Lower the second reference with the same lifetime — should reuse vid 0.
        let ref_ty2 = HirTy {
            hir_id: crate::hir::HirId::new(crate::hir::DefId(0), crate::hir::ItemLocalId(2)),
            kind: HirTyKind::Ref(Some(lifetime_a), Mutability::Immutable, Box::new(inner)),
            inferred: None,
            span: Span::DUMMY,
        };
        let ty2 = lower_hir_ty_to_mir_ty_with_lifetimes(
            &ref_ty2,
            &mut region_counter,
            &mut lifetime_map,
            &[],
        );
        let vid2 = match &ty2.kind {
            TyKind::Ref(Region::Var(vid), _, _) => *vid,
            _ => panic!("expected Ref with Region::Var"),
        };

        // Both should have the same vid (deduplication).
        assert_eq!(
            vid1, vid2,
            "explicit lifetimes with same name should share vid"
        );
    }

    /// Stage 15.92: Verify that elided lifetimes get different vids.
    #[test]
    fn elided_lifetime_no_deduplication() {
        let inner = HirTy {
            hir_id: crate::hir::HirId::new(crate::hir::DefId(0), crate::hir::ItemLocalId(0)),
            kind: HirTyKind::Int(crate::ast::IntTy::I32),
            inferred: None,
            span: Span::DUMMY,
        };
        let ref_ty = HirTy {
            hir_id: crate::hir::HirId::new(crate::hir::DefId(0), crate::hir::ItemLocalId(1)),
            kind: HirTyKind::Ref(None, Mutability::Immutable, Box::new(inner.clone())),
            inferred: None,
            span: Span::DUMMY,
        };

        let mut region_counter = 0u32;
        let mut lifetime_map = std::collections::HashMap::new();

        let ty1 = lower_hir_ty_to_mir_ty_with_lifetimes(
            &ref_ty,
            &mut region_counter,
            &mut lifetime_map,
            &[],
        );
        let vid1 = match &ty1.kind {
            TyKind::Ref(Region::Var(vid), _, _) => *vid,
            _ => panic!("expected Ref with Region::Var"),
        };

        let ref_ty2 = HirTy {
            hir_id: crate::hir::HirId::new(crate::hir::DefId(0), crate::hir::ItemLocalId(2)),
            kind: HirTyKind::Ref(None, Mutability::Immutable, Box::new(inner)),
            inferred: None,
            span: Span::DUMMY,
        };
        let ty2 = lower_hir_ty_to_mir_ty_with_lifetimes(
            &ref_ty2,
            &mut region_counter,
            &mut lifetime_map,
            &[],
        );
        let vid2 = match &ty2.kind {
            TyKind::Ref(Region::Var(vid), _, _) => *vid,
            _ => panic!("expected Ref with Region::Var"),
        };

        // Elided lifetimes should get different vids.
        assert_ne!(vid1, vid2, "elided lifetimes should NOT share vid");
    }
}
