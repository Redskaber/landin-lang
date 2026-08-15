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
mod closure_capture;
mod control_flow;
mod expr_operand;
mod field_resolution;
mod overflow_assert;
mod pattern_bindings;
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
pub use expr_operand::build_dyn_trait_call_terminator;
// Stage 15.6 (v0.2): Expose the uncached inner function so tests can
// verify cache semantics (cached result == uncached result). Per §29.1.3
// (Design-Impl-Test coverage): tests need direct access to verify the
// cache wrapper doesn't change behavior.
pub use expr_operand::query_method_return_type_uncached;
// Stage 15.7 (v0.2): Expose consolidated writeback functions for the
// driver to call. Per §23 (API Naming): `pub use` of named functions
// (no glob). Per §16: driver is orchestrator-only — these functions
// contain the writeback logic, driver just calls them in order.
pub(crate) use expr_operand::{lower_expr_to_operand, resolve_enum_variant};
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
            resolver: None,
            loop_stack: Vec::new(),
            loop_result_locals: Vec::new(),
            type_errors: Vec::new(),
            method_return_type_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
            synthesized_closure_functions: std::collections::HashMap::new(),
            closure_def_id_counter: 0,
            generic_params: Vec::new(),
        }
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
            resolver: None,
            loop_stack: Vec::new(),
            loop_result_locals: Vec::new(),
            type_errors: Vec::new(),
            method_return_type_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
            synthesized_closure_functions: std::collections::HashMap::new(),
            closure_def_id_counter,
            generic_params: Vec::new(),
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
        let result = self
            .hir
            .and_then(|hir| expr_operand::query_method_return_type_uncached(hir, method_def_id));
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

    /// Allocate a temporary local and assign the given rvalue to it.
    pub fn eval_rvalue_to_temp(&mut self, rvalue: Rvalue, ty: Ty, span: Span) -> LocalId {
        let temp = self.mir.new_local(ty, None, span);
        self.push_assign(Place::local(temp, span), rvalue, span);
        temp
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
pub fn lower_hir_body_to_mir(body: &Body, interner: &Rodeo, hir: &HirCrate) -> MirBody {
    lower_hir_body_to_mir_with_return_ty(body, interner, hir, None)
}

/// Lower a HIR body to MIR with an explicit return type (from the fn sig).
///
/// When `return_ty` is `Some(ty)`, the return local (LocalId(0)) is
/// initialized with that type instead of a fresh inference variable.
/// This lets the type checker unify the body's value with the declared
/// return type — fixing the "fn sig not unified with body value type"
/// limitation from Stage 2.4d gate review (fix #3).
pub fn lower_hir_body_to_mir_with_return_ty(
    body: &Body,
    interner: &Rodeo,
    hir: &HirCrate,
    return_ty: Option<HirTy>,
) -> MirBody {
    // Stage 15.12: lower_full now returns 4-tuple (mir, unify, type_errors, closures).
    // The convenience wrappers discard unify + type_errors + closures for
    // callers that only need the MirBody (e.g., tests).
    lower_hir_body_to_mir_full(body, interner, hir, return_ty).0
}

/// Full version of `lower_hir_body_to_mir_with_return_ty` that also
/// returns the UnificationTable used during lowering.
///
/// The unify table contains the IntVar/FloatVar allocated for unsuffixed
/// integer/float literals. The type checker needs this table to properly
/// resolve these variables after type inference (defaulting unresolved
/// int vars to i32, float vars to f64).
///
/// Without returning the unify table, the type checker would create a
/// fresh (empty) table and lose track of the IntVars allocated during
/// lowering — causing literals to stay as unresolved Infer vars even
/// after typeck.
///
/// Stage 15.12: Now returns 3-tuple `(MirBody, UnificationTable, Vec<TypeError>)`.
/// The type_errors were previously stored on `MirBody.lower_type_errors` —
/// this was an architectural smell (IR carrying error collection). Now
/// errors are returned from the lowering function, separating concerns.
pub fn lower_hir_body_to_mir_full(
    body: &Body,
    interner: &Rodeo,
    hir: &HirCrate,
    return_ty: Option<HirTy>,
) -> (
    MirBody,
    UnificationTable,
    Vec<crate::typeck::TypeError>,
    std::collections::HashMap<crate::hir::DefId, SynthesizedClosureFunction>,
) {
    // Stage 5.80: delegate to the new entry point with plan = None.
    // Backward-compatible: all existing callers see identical behavior.
    // Stage 16.85: resolver = None (legacy path, no rich error messages).
    lower_hir_body_to_mir_full_with_dyn_trait_plan(body, interner, hir, return_ty, None, None)
}

/// Stage 5.80: Full lowering entry point with optional `DynTraitMIRPlan`.
///
/// When `plan` is `Some`, attaches it to the `MirLowerCtxt` via
/// `cx.set_dyn_trait_plan(plan.clone())` — this activates the
/// `HirExprKind::MethodCall` dyn Trait path (Stage 5.78). The clone
/// happens once per body (acceptable cost; the plan is small — a few
/// hundred bytes for typical crates).
///
/// When `plan` is `None`, behavior is identical to
/// `lower_hir_body_to_mir_full` (legacy path — no dyn Trait lowering).
///
/// # Driver integration
///
/// The driver (Stage 5.80) builds the plan once via
/// `build_dyn_trait_mir_plan_from_resolver(&trait_resolver, &interner)`
/// before the per-body loop, then passes `Some(&plan)` to this function
/// for each body. This activates end-to-end dyn Trait MIR lowering:
/// HIR `receiver.method(args)` → MIR `TerminatorKind::Call` with Const marker
/// → codegen vtable indirect call IR.
///
/// # §16 compliance
///
/// The plan is built upstream by the driver (which is the sole orchestrator
/// allowed to read TraitResolver). `MirLowerCtxt` does not own a
/// TraitResolver — it receives the plan as data. Data flow:
/// driver → plan → cx → lower → mir::body side-table → codegen.
///
/// # §23 compliance
///
/// `lower_hir_body_to_mir_full_with_dyn_trait_plan` follows the
/// `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>_<noun>_<noun>` pattern.
/// The `_with_dyn_trait_plan` suffix is the Rust API-guidelines convention
/// for "extended variant with additional feature" (mirrors `Vec::with_capacity`,
/// `HashMap::with_hasher`).
pub fn lower_hir_body_to_mir_full_with_dyn_trait_plan(
    body: &Body,
    interner: &Rodeo,
    hir: &HirCrate,
    return_ty: Option<HirTy>,
    plan: Option<&DynTraitMIRPlan>,
    resolver: Option<&crate::traits::TraitResolver>,
) -> (
    MirBody,
    UnificationTable,
    Vec<crate::typeck::TypeError>,
    std::collections::HashMap<crate::hir::DefId, SynthesizedClosureFunction>,
) {
    let mut cx = MirLowerCtxt::new(interner, body.span);
    cx.hir = Some(hir);

    // Stage 18.105 (S6 fix): Set generic_params from the function's HIR generics.
    // This allows lower_path_generic_args to resolve bare type parameters (e.g., `T`
    // in `Box<T>`) to Param(N) instead of Error.
    let owner_def_id: crate::hir::DefId = body.hir_id.owner;
    cx.generic_params = crate::hir::generics::find_generics(owner_def_id, hir);

    // Stage 16.85: Set resolver for rich error messages (Adt type names).
    if let Some(resolver) = resolver {
        cx.set_resolver(resolver);
    }

    // Stage 5.80: attach the dyn Trait plan if provided.
    // Per §16: plan was built upstream by the driver via
    // `build_dyn_trait_mir_plan_from_resolver()`. The lower does not
    // query TraitResolver directly.
    if let Some(plan) = plan {
        cx.set_dyn_trait_plan(plan.clone());
    }

    // Stage 15.49: Region counter for assigning fresh RegionVids to
    // reference types during lowering. Each `&T` gets a unique vid,
    // giving the region inference infrastructure real region variables.
    let mut region_counter = 0u32;

    // Stage 15.90: Lifetime elision rule 3 — if the function has exactly
    // one input lifetime (elided or explicit), that lifetime is assigned
    // to all elided output lifetimes.
    //
    // To implement this, we lower params first (collecting their region
    // vids), then lower the return type. If the return type has elided
    // lifetimes, we replace them with the single input lifetime's vid
    // (rule 3) or leave them as fresh vids (rule 1, each gets its own).
    //
    // Rust elision rules (RFC 141):
    //   1. Each elided input lifetime gets its own fresh lifetime.
    //   2. If there's exactly one input lifetime (elided or explicit),
    //      it's assigned to all elided output lifetimes.
    //   3. If there are multiple input lifetimes but one is &self/&mut self,
    //      that lifetime is assigned to all elided output lifetimes.
    //
    // Stage 15.90 implements rule 2 (the most common case). Rule 3 (self)
    // is deferred — requires tracking which param is self.
    //
    // Per §1.0 原則 3 "显式 > 隐式": elision rules are explicitly applied.
    // Per §23: function names follow conventions.

    // Lower param types first, collecting region vids.
    // Stage 15.90/15.91: We need to collect region vids from params
    // for lifetime elision rules 2 and 3.
    // - Rule 2: exactly one input lifetime → use it for output.
    // - Rule 3: multiple input lifetimes, but if one is &self/&mut self,
    //   use the self lifetime for output.
    let mut param_region_vids_collected: Vec<crate::mir::ty::RegionVid> = Vec::new();
    // Stage 15.91: Track the self param's region vid for rule 3.
    let mut self_region_vid: Option<crate::mir::ty::RegionVid> = None;
    // Stage 15.92: Map from lifetime name (Spur) → RegionVid, for explicit
    // lifetime deduplication. References with the same lifetime name share
    // the same vid.
    let mut lifetime_map: std::collections::HashMap<
        crate::lexer::Symbol,
        crate::mir::ty::RegionVid,
    > = std::collections::HashMap::new();
    // Stage 15.90: Store lowered param types so we don't lower them twice
    // (once for elision collection, once for local allocation). Reusing
    // ensures the region vids match.
    let mut lowered_param_types: Vec<Option<Ty>> = Vec::with_capacity(body.params.len());

    // Allocate LocalId(0) as the return value placeholder.
    // We lower the return type AFTER params so elision rules 2/3 can apply.
    let return_mir_ty = {
        // First, lower all param types to collect region vids.
        for param in &body.params {
            if let Some(t) = &param.ty {
                if param.self_kind.is_some() {
                    // Stage 15.91: For &self/&mut self, resolve the self type
                    // and collect its region vid for elision rule 3.
                    let self_ty = resolve_self_param_type(&cx, body, param.self_kind);
                    if let Some(ref mir_ty) = self_ty {
                        // Collect region vids from the self type.
                        let mut self_vids = Vec::new();
                        collect_region_vids(mir_ty, &mut self_vids);
                        if let Some(&vid) = self_vids.first() {
                            self_region_vid = Some(vid);
                            param_region_vids_collected.push(vid);
                        }
                    }
                    lowered_param_types.push(None);
                } else {
                    // Stage 15.92: Use lifetime_map for explicit lifetime
                    // deduplication — references with the same lifetime name
                    // share the same RegionVid.
                    let mir_ty = lower_hir_ty_to_mir_ty_with_lifetimes(
                        t,
                        &mut region_counter,
                        &mut lifetime_map,
                    );
                    // Collect region vids from this param type.
                    collect_region_vids(&mir_ty, &mut param_region_vids_collected);
                    lowered_param_types.push(Some(mir_ty));
                }
            } else {
                lowered_param_types.push(None);
            }
        }
        // Now lower the return type with the accumulated region counter.
        match &return_ty {
            Some(t) => {
                let raw_return_ty = lower_hir_ty_to_mir_ty_with_lifetimes(
                    t,
                    &mut region_counter,
                    &mut lifetime_map,
                );
                // Stage 15.90/15.91: Apply elision rules 2 and 3.
                apply_elision_rules(
                    &raw_return_ty,
                    &param_region_vids_collected,
                    self_region_vid,
                )
            }
            // Stage 18.71 P0-5: For void functions (`fn f() { ... }` with
            // no declared return type), use unit `Tuple([])` as the return
            // local's type — NOT a fresh Infer variable.
            //
            // Previously this used `fresh_infer_ty`, which let
            // `fn f() { return 42; }` unify Infer with Int and silently
            // accept the type mismatch. With explicit unit type, the
            // typeck Assign check fires: place=Tuple([]), rvalue=Int →
            // mismatch error.
            //
            // Per §1.0 原则 3 "显式 > 隐式": void return type is explicit unit.
            // Per §1.0 原则 4 "报错 > 静默": return-with-value in void fn
            // must be reported, not silently accepted.
            None => Ty::new(TyKind::Tuple(vec![]), Span::DUMMY),
        }
    };
    // G5 fix: return_local is assigned multiple times (once per Return
    // terminator path + once at function end), so it must be Mutable.
    let return_local = cx.mir.new_local_with_mut(
        return_mir_ty,
        None,
        Span::DUMMY,
        crate::mir::ty::Mutability::Mutable,
    );
    debug_assert_eq!(return_local, LocalId(0));
    // StorageLive for the return local at function entry.
    cx.mir
        .block_mut(cx.current_block)
        .statements
        .push(Statement {
            kind: StatementKind::StorageLive(return_local),
            span: body.span,
        });

    // Allocate locals for fn params.
    // Stage 15.90: Reuse the lowered param types from the elision pass
    // above (ensures region vids match). Self params are still resolved
    // here because they need the cx context.
    for (param_idx, param) in body.params.iter().enumerate() {
        let ty = if let Some(pre_lowered) =
            lowered_param_types.get(param_idx).and_then(|t| t.as_ref())
        {
            // Reuse the pre-lowered type (non-self params).
            pre_lowered.clone()
        } else {
            match &param.ty {
                Some(t) => {
                    // Stage 13.18: For self params, the parser sets ty to a Path
                    // with "Self" as the segment. This resolves to Res::SelfTy
                    // which lower_hir_ty_to_mir_ty doesn't handle (returns Error).
                    // So for self params, we resolve the type from the impl block's
                    // self_ty directly.
                    // Stage 14.18 (GAP-31): &self/&mut self Ref wrapping was attempted
                    // but reverted — codegen doesn't correctly handle Deref projections
                    // for field access through references. The full fix requires codegen
                    // changes to handle ProjectionElem::Deref in field access paths.
                    // See docs/worklog.md Stage 14.18 for details.
                    if param.self_kind.is_some() {
                        resolve_self_param_type(&cx, body, param.self_kind).unwrap_or_else(|| {
                            lower_hir_ty_to_mir_ty_with_regions(t, &mut region_counter)
                        })
                    } else {
                        lower_hir_ty_to_mir_ty_with_regions(t, &mut region_counter)
                    }
                }
                None => {
                    if param.self_kind.is_some() {
                        resolve_self_param_type(&cx, body, param.self_kind)
                            .unwrap_or_else(|| cx.fresh_infer_ty(Span::DUMMY))
                    } else {
                        cx.fresh_infer_ty(Span::DUMMY)
                    }
                }
            }
        };
        // Stage 15.79 (parser bug fix follow-up): propagate the param
        // pattern's mutability into the local. Previously this used the
        // default `new_local` (Immutable), so `fn f(mut n: i32) { n = 0; }`
        // would fail with AssignImmutable — the param was correctly
        // parsed as `BindingMode::ByValue(Mutable)` but the local was
        // always immutable. Symmetric with the `let mut x` lowering in
        // control_flow.rs (which uses pat_mutability + new_local_with_mut).
        //
        // Per §1.0 原則 3 "显式 > 隐式": mutability is explicitly
        // propagated from pattern to local, not silently dropped.
        // Per §1.0 原則 6 "通用 > 特例": same code path as `let` bindings.
        let mutability = pattern_bindings::pat_mutability(&param.pat);
        let param_local = cx.new_local_with_mut(param.pat.hir_id, ty, None, mutability);
        // StorageLive for each parameter at function entry.
        cx.mir
            .block_mut(cx.current_block)
            .statements
            .push(Statement {
                kind: StatementKind::StorageLive(param_local),
                span: param.span,
            });
    }

    // Lower the body's value expression into the return local.
    let value_local = lower_expr_to_operand(&mut cx, &body.value);

    // Stage 14.23: If the current block is already terminated (e.g. by a
    // `return` statement inside the body), skip the assignment to the return
    // local. The return local was already assigned by the `return` expression's
    // lowering. Without this check, we'd emit an assignment AFTER the Return
    // terminator, which is dead code that overwrites the return value with
    // an uninitialized local.
    //
    // Stage 18.71 P0-5: For void functions (`fn f() { ... }` with no declared
    // return type), the return local's type is unit `Tuple([])`. We must NOT
    // assign the body's trailing expression to the return local — instead,
    // the trailing expression is evaluated for side effects (like a statement)
    // and its result is discarded. This matches Rust's behavior: in a void
    // function, the trailing expression is treated as a discarded statement.
    //
    // Why always skip for void fns: The trailing expression's type may be
    // Infer(IntVar) (unsuffixed int literal) which would later resolve to
    // i32. Assigning it to a unit return local would trigger a spurious
    // type mismatch in post_check_statement. By skipping the assign for
    // all void fns, we correctly handle `fn f() { 42 }`, `fn f() { () }`,
    // `fn f() { add(1, 2) }`, etc.
    //
    // For non-void fns (`fn f() -> T { expr }`), the assign happens
    // normally, and post_check_statement catches any type mismatch
    // (e.g., `fn f() -> i32 { true }`).
    //
    // Per §1.0 原則 9 "正确 > 妥协": match Rust's semantics for void fns.
    let return_ty = cx.mir.local(return_local).ty.clone();
    let return_is_unit = matches!(&return_ty.kind, TyKind::Tuple(tys) if tys.is_empty());
    let skip_assign = cx.is_terminated() || return_is_unit;

    if !skip_assign {
        // Stage 16.06: Use Operand::Move for the function body's tail
        // expression. The tail value semantically moves into the return
        // slot (LocalId(0)). Using Operand::Copy was unsound for non-Copy
        // types (e.g., structs with `impl Drop`) — the borrow checker
        // would reject "use of moved value: does not implement Copy".
        // With field-level Copy derivation (Stage 16.06), non-Copy types
        // are now correctly identified, so we must use Move for correctness.
        // For Copy types, Move is equivalent to Copy (no move recorded).
        cx.push_assign(
            Place::local(return_local, Span::DUMMY),
            Rvalue::Use(Operand::Move(Place::local(value_local, Span::DUMMY))),
            body.span,
        );
    }

    // Emit StorageDead for all locals (except the return local) before
    // the function returns. This is a conservative approximation —
    // ideally we'd emit StorageDead at each local's scope end, but that
    // requires scope tracking (Stage 3). For now, all locals die at
    // function return.
    //
    // We skip LocalId(0) (the return local) because it's still alive
    // at the point of Return.
    //
    // Stage 15.62: Emit StorageDead in REVERSE declaration order so that
    // `elaborate_drops` produces `Drop` terminators in reverse declaration
    // order — matching Rust's drop semantics (last-declared local is
    // dropped first). Previously, forward emission produced forward drop
    // order, which was incorrect.
    //
    // Per §1.0 原則 6 "通用 > 特例": one rule (reverse iteration) handles
    // all drop-ordering cases — no special-casing per local type.
    // Per §23: no API change (internal MIR lowering detail).
    let local_count = cx.mir.local_decls.len();
    for i in (1..local_count).rev() {
        cx.mir
            .block_mut(cx.current_block)
            .statements
            .push(Statement {
                kind: StatementKind::StorageDead(LocalId(i as u32)),
                span: body.span,
            });
    }

    // Terminate the current block with Return.
    cx.terminate_kind(TerminatorKind::Return);

    // Stage 3.47 (L-PIPE-1 closure per §16): sink ADT layouts from HIR into
    // MIR's `adt_layouts` side-table. This lets codegen resolve
    // `TyKind::Adt(def_id, _)` storage layouts **without reading HIR** —
    // closing the pipeline-coupling debt carried since Stage 3.30.
    //
    // We walk every local's type and register any Adt we encounter. The
    // walk is shallow (we don't recurse into nested Adts — they'll be
    // registered when their own DefId appears in some local's type). This
    // covers all Adt construction paths:
    //   - `lower_hir_ty_to_mir_ty` (free fn — params, returns, let bindings)
    //   - Direct `TyKind::Adt(def_id, …)` construction in lower_expr paths
    //     (struct/enum literals, Call→Aggregate rewrite)
    //   - Field types sunk into `AggregateKind::Adt`
    adt_layout::populate_adt_layouts(&mut cx.mir, hir);

    // Extract the unify table + type_errors before consuming cx.
    // Stage 15.12: type_errors now returned from the lowering function
    // (was stored on MirBody.lower_type_errors — mixed IR + error collection).
    // Stage 16.13: synthesized_closure_functions also returned for codegen.
    let unify = std::mem::take(&mut cx.unify);
    let type_errors = std::mem::take(&mut cx.type_errors);
    let synthesized_closure_functions = std::mem::take(&mut cx.synthesized_closure_functions);
    (cx.mir, unify, type_errors, synthesized_closure_functions)
}

/// Stage 16.14 (Task 10 Step 2): Build a MIR body for a synthesized closure
/// `call` function.
///
/// Given the `SynthesizedClosureFunction` metadata (collected during the
/// main function's MIR lowering), this function builds a separate MirBody
/// representing the closure's `call` function:
///
/// ```text
/// fn closure_call_fn_N(self: Closure_N, param1: T1, param2: T2, ...) -> Ret {
///     // Extract captures from self:
///     local_cap_0 = Projection(self, Field(0, cap_ty_0))
///     local_cap_1 = Projection(self, Field(1, cap_ty_1))
///     ...
///     // Lower closure body (references to captures resolve to local_cap_i)
///     <body>
///     return <body_result>
/// }
/// ```
///
/// Stage 16.29 (通解 — Typeck on synthesized closure MIR bodies):
/// This function now takes `unify: UnificationTable` as input (the SHARED
/// unify table from the main body) and returns it back. The closure MIR
/// body's fresh Infer vars are allocated from this shared table, so they
/// don't collide with the closure_struct_ty's Infer vars (which were
/// created during main body lowering).
///
/// Stage 16.29 (nested closures): This function ALSO returns any nested
/// `synthesized_closure_functions` discovered while lowering the closure
/// body (e.g., `|| || x` — the outer closure's body contains an inner
/// closure literal). The driver processes these recursively.
///
/// The driver flow:
///   1. Lower main body → main_mir, main_unify, synthesized_closures
///   2. For each closure: pass main_unify into this function, get back
///      (closure_mir, main_unify, errors, nested_closures). main_unify is
///      updated with the closure's fresh Infer vars.
///   3. Typeck main body with main_unify → resolves closure_struct_ty's
///      Infer vars.
///   4. Typeck closure MIR bodies with main_unify → resolves closure
///      body's Infer vars.
///   5. Recursively process nested closures (from step 2).
///
/// Per §1.0 原則 6 "通用 > 特例": one unify table for main body + all
/// closures (including nested) — no special-case handling per closure type.
/// Per §1.0 原則 9 "正确 > 妥协": fix the root cause (unify table
/// isolation), not the symptom (cycle detection in resolve_ty_var).
/// Per §16: this function reads HIR (the closure body) — allowed during
/// MIR lowering.
/// Per §23: `build_synthesized_closure_mir_body` follows
/// `<verb>_<adj>_<noun>_<noun>` pattern.
pub fn build_synthesized_closure_mir_body(
    func: &SynthesizedClosureFunction,
    interner: &Rodeo,
    hir: &HirCrate,
    unify: UnificationTable,
    closure_def_id_counter: u32,
) -> (
    MirBody,
    UnificationTable,
    Vec<crate::typeck::TypeError>,
    std::collections::HashMap<crate::hir::DefId, SynthesizedClosureFunction>,
    u32,
) {
    let mut cx =
        MirLowerCtxt::new_with_unify(interner, func.body.span, unify, closure_def_id_counter);
    cx.hir = Some(hir);

    // Stage 16.20: MirBody::new() creates an empty local_decls vec.
    // We need to explicitly create LocalId(0) as the return local FIRST,
    // then LocalId(1) as `self`, then LocalId(2+) as closure params.
    //
    // LocalId(0): return local (fresh infer type — will be resolved
    // from the body expression type by typeck writeback).
    //
    // Stage 16.31 (通解 — return local mutability): The return local
    // is Mutable, matching the main body's lowering (G5 fix). This
    // allows `return expr;` inside closure bodies to assign to
    // LocalId(0) without borrowck flagging "cannot assign twice to
    // immutable variable" (the first assign is the body result, the
    // second is the early return — both are valid writes to the
    // mutable return local).
    let return_ty = cx.fresh_infer_ty(func.body.span);
    let return_local = cx.mir.new_local_with_mut(
        return_ty,
        None,
        func.body.span,
        crate::mir::ty::Mutability::Mutable,
    );
    debug_assert_eq!(return_local, crate::mir::place::LocalId(0));

    // LocalId(1): `self` parameter — the closure struct.
    let self_local = cx
        .mir
        .new_local(func.closure_struct_ty.clone(), None, func.body.span);
    // Note: LocalId(0) is the return local, LocalId(1) is `self`.

    // LocalId(2), (3), ...: closure parameters.
    let mut param_locals: Vec<crate::mir::place::LocalId> = Vec::new();
    for param in &func.params {
        let ty = cx.fresh_infer_ty(param.pat.span);
        let local = cx.mir.new_local(ty, None, param.pat.span);
        // Register param's hir_id → local in local_map.
        cx.local_map.insert(param.pat.hir_id, local);
        param_locals.push(local);
    }

    // Extract captures from `self` and register their hir_ids.
    // Stage 16.23: `self` is passed as a pointer (OpaquePtr) in codegen.
    // To access capture fields, we need to first Deref the pointer, then
    // project the field. This generates GEP in LLVM:
    //   getelementptr inbounds { ty0, ty1, ... }, ptr %self, i32 0, i32 field_idx
    //
    // Stage 16.31 (通解 — capture mutability): The extract local is
    // created with the captured variable's mutability (from the outer
    // scope). This allows the closure body to mutate the captured
    // variable (e.g., `x += 1` where `x` is a captured `mut`).
    // Without this, borrowck would flag the assignment as
    // "cannot assign twice to immutable variable".
    for (cap_hir_id, field_idx, cap_ty, cap_mutability) in &func.captures {
        let extract_local =
            cx.mir
                .new_local_with_mut(cap_ty.clone(), None, func.body.span, *cap_mutability);
        // Assign: extract_local = Copy(Projection(Projection(self, Deref), Field(field_idx, cap_ty)))
        cx.push_assign(
            crate::mir::place::Place::local(extract_local, func.body.span),
            crate::mir::place::Rvalue::Use(crate::mir::place::Operand::Copy(
                crate::mir::place::Place {
                    kind: crate::mir::place::PlaceKind::Projection(
                        Box::new(crate::mir::place::Place {
                            kind: crate::mir::place::PlaceKind::Projection(
                                Box::new(crate::mir::place::Place::local(
                                    self_local,
                                    func.body.span,
                                )),
                                crate::mir::place::ProjectionElem::Deref,
                            ),
                            span: func.body.span,
                        }),
                        crate::mir::place::ProjectionElem::Field(
                            crate::mir::place::FieldId(*field_idx),
                            cap_ty.clone(),
                        ),
                    ),
                    span: func.body.span,
                },
            )),
            func.body.span,
        );
        // Register captured binding's hir_id → extract_local.
        cx.local_map.insert(*cap_hir_id, extract_local);
    }

    // Lower the closure body expression into a local.
    let body_result_local = lower_expr_to_operand(&mut cx, &func.body);

    // Assign the body result to the return local (LocalId(0)).
    if !cx.is_terminated() {
        cx.push_assign(
            crate::mir::place::Place::local(
                crate::mir::place::LocalId(0),
                crate::session::Span::DUMMY,
            ),
            crate::mir::place::Rvalue::Use(crate::mir::place::Operand::Move(
                crate::mir::place::Place::local(body_result_local, crate::session::Span::DUMMY),
            )),
            func.body.span,
        );
    }

    // Terminate with Return.
    cx.terminate_kind(crate::mir::body::TerminatorKind::Return);

    // Populate adt_layouts (same as main function lowering).
    adt_layout::populate_adt_layouts(&mut cx.mir, hir);

    // Stage 16.17: Set the DefId on the MirBody so codegen can resolve
    // the function name via fn_name_by_def_id.
    cx.mir.def_id = Some(func.def_id);

    // Stage 16.29 (通解): Return the unify table and type errors so the
    // driver can run TypeChecker::with_unify + check_mir_body_with_tables
    // on this MIR body. This resolves all Infer types (return type, param
    // types) — eliminating the typeck gap that forced the
    // `has_complex_captures` special-case routing.
    //
    // Stage 16.29 (nested closures): Also return any nested
    // synthesized_closure_functions discovered while lowering the closure
    // body. The driver processes these recursively.
    let unify = std::mem::take(&mut cx.unify);
    let type_errors = std::mem::take(&mut cx.type_errors);
    let nested_closures = std::mem::take(&mut cx.synthesized_closure_functions);
    let closure_def_id_counter = cx.closure_def_id_counter();
    (
        cx.mir,
        unify,
        type_errors,
        nested_closures,
        closure_def_id_counter,
    )
}

// ================================================================
// Stage 3.65: convenience aliases
// ================================================================
//
// Per `docs/develop/v0/api-naming-standard.md` §2.2, each stage should
// expose a `<verb>_<noun>` free-function entry point. The MIR lower
// stage historically used the verbose `lower_hir_body_to_mir_*` names
// (which are explicit but break the verb-object pattern set by
// `lower_crate` / `resolve_crate` / `codegen_crate`). These thin
// wrappers provide the short form without removing the long form.

/// Stage 3.65: convenience alias for `lower_hir_body_to_mir`.
///
/// Mirrors the entry-point style of `hir::lower::lower_crate` (verb_noun).
/// The long-form `lower_hir_body_to_mir` remains available for callers
/// who prefer the explicit name.
pub fn lower_body(body: &Body, interner: &Rodeo, hir: &HirCrate) -> MirBody {
    lower_hir_body_to_mir(body, interner, hir)
}

/// Stage 3.65: convenience alias for `lower_hir_body_to_mir_full`.
///
/// Returns both the `MirBody` and the `UnificationTable` (the latter is
/// passed to `TypeChecker::with_unify` so typeck can resolve inference
/// variables created during lowering).
///
/// Stage 15.12: Now also returns `Vec<TypeError>` (was stored on MirBody).
pub fn lower_body_full(
    body: &Body,
    interner: &Rodeo,
    hir: &HirCrate,
    return_ty: Option<HirTy>,
) -> (
    MirBody,
    UnificationTable,
    Vec<crate::typeck::TypeError>,
    std::collections::HashMap<crate::hir::DefId, SynthesizedClosureFunction>,
) {
    lower_hir_body_to_mir_full(body, interner, hir, return_ty)
}

/// Best-effort const-eval for array length expressions.
///
/// Stage 2.4c only handles literal integer expressions (e.g., `[T; 4]`).
/// Full const-eval (including const fns, paths, arithmetic) is Stage 3+.
/// If the expression is not a literal, falls back to `ConstVal::Uint(0)`
/// with `Ty::Error` to signal that the length couldn't be evaluated
/// (the type checker will flag the array as ill-typed).
fn const_eval_array_len(expr: &HirExpr, span: Span) -> Const {
    match &expr.kind {
        HirExprKind::Lit(HirLitKind::Int(n, _)) => Const {
            ty: Ty::new(TyKind::Uint(ast::UintTy::Usize), span),
            val: ConstVal::Uint(*n),
        },
        HirExprKind::Lit(HirLitKind::Uint(n, _)) => Const {
            ty: Ty::new(TyKind::Uint(ast::UintTy::Usize), span),
            val: ConstVal::Uint(*n),
        },
        // Non-literal: emit an Error-typed const so typeck flags it.
        _ => Const {
            ty: Ty::new(TyKind::Error, span),
            val: ConstVal::Uint(0),
        },
    }
}

/// Stage 15.90: Collect all `RegionVid`s from a `Ty`'s reference types.
///
/// Recursively walks the type and collects every `Region::Var(vid)` found
/// in `TyKind::Ref` variants. Used to gather input lifetime vids for
/// lifetime elision rule 2 (output lifetime = input lifetime).
fn collect_region_vids(ty: &Ty, vids: &mut Vec<crate::mir::ty::RegionVid>) {
    use crate::mir::ty::{Region, TyKind};
    match &ty.kind {
        TyKind::Ref(region, _, inner) => {
            if let Region::Var(vid) = region {
                vids.push(*vid);
            }
            collect_region_vids(inner, vids);
        }
        TyKind::RawPtr(_, inner) => {
            collect_region_vids(inner, vids);
        }
        TyKind::Array(inner, _) | TyKind::Slice(inner) => {
            collect_region_vids(inner, vids);
        }
        TyKind::Tuple(tys) => {
            for t in tys {
                collect_region_vids(t, vids);
            }
        }
        TyKind::FnPtr(sig) => {
            for t in &sig.inputs {
                collect_region_vids(t, vids);
            }
            collect_region_vids(&sig.output, vids);
        }
        _ => {}
    }
}

/// Stage 15.90/15.91: Apply lifetime elision rules to a return type.
///
/// Implements RFC 141 elision rules:
///   - Rule 2: If there's exactly one input lifetime (elided or explicit),
///     it's assigned to all elided output lifetimes.
///   - Rule 3: If there are multiple input lifetimes but one is `&self`/
///     `&mut self`, that lifetime is assigned to all elided output lifetimes.
///
/// This function replaces all `Region::Var(vid)` in the return type with
/// the selected input lifetime's vid.
///
/// Per §1.0 原則 3 "显式 > 隐式": elision rules are explicitly applied.
/// Per §23: function name follows `<verb>_<noun>_<noun>` pattern.
fn apply_elision_rules(
    return_ty: &Ty,
    input_vids: &[crate::mir::ty::RegionVid],
    self_vid: Option<crate::mir::ty::RegionVid>,
) -> Ty {
    use crate::mir::ty::{Region, RegionVid, TyKind};

    // Determine which input lifetime to use for the output.
    let target_vid = if input_vids.len() == 1 {
        // Rule 2: exactly one input lifetime → use it.
        Some(input_vids[0])
    } else if input_vids.len() > 1 {
        // Rule 3: multiple input lifetimes, but if one is &self/&mut self,
        // use the self lifetime.
        self_vid
    } else {
        // No input lifetimes → no elision (keep fresh output vids).
        None
    };

    match target_vid {
        None => return_ty.clone(),
        Some(target_vid) => {
            // Recursively replace all region vids in the return type.
            fn replace_regions(ty: &Ty, target_vid: RegionVid) -> Ty {
                let span = crate::session::Span::DUMMY;
                match &ty.kind {
                    TyKind::Ref(_, mutability, inner) => Ty::new(
                        TyKind::Ref(
                            Region::Var(target_vid),
                            *mutability,
                            Box::new(replace_regions(inner, target_vid)),
                        ),
                        span,
                    ),
                    TyKind::RawPtr(mutability, inner) => Ty::new(
                        TyKind::RawPtr(*mutability, Box::new(replace_regions(inner, target_vid))),
                        span,
                    ),
                    TyKind::Array(inner, count) => Ty::new(
                        TyKind::Array(Box::new(replace_regions(inner, target_vid)), count.clone()),
                        span,
                    ),
                    TyKind::Slice(inner) => Ty::new(
                        TyKind::Slice(Box::new(replace_regions(inner, target_vid))),
                        span,
                    ),
                    TyKind::Tuple(tys) => Ty::new(
                        TyKind::Tuple(tys.iter().map(|t| replace_regions(t, target_vid)).collect()),
                        span,
                    ),
                    TyKind::FnPtr(sig) => Ty::new(
                        TyKind::FnPtr(crate::mir::ty::Sig {
                            inputs: sig
                                .inputs
                                .iter()
                                .map(|t| replace_regions(t, target_vid))
                                .collect(),
                            output: Box::new(replace_regions(&sig.output, target_vid)),
                            abi: sig.abi,
                            is_unsafe: sig.is_unsafe,
                        }),
                        span,
                    ),
                    _ => ty.clone(),
                }
            }
            replace_regions(return_ty, target_vid)
        }
    }
}

/// Stage 15.92: Lower a HIR type to MIR type with explicit lifetime tracking.
///
/// This is a wrapper around `lower_hir_ty_to_mir_ty_with_regions` that
/// adds explicit lifetime deduplication via `lifetime_map`. When an
/// explicit lifetime is encountered (e.g., `'a`), the function looks up
/// the lifetime name in `lifetime_map`. If found, reuses the existing
/// vid; if not found, creates a fresh vid and records it in the map.
///
/// This ensures references with the same explicit lifetime name share
/// the same region vid, which is what the region inference needs to
/// enforce lifetime constraints correctly.
///
/// Per §23: `_with_lifetimes` suffix follows convention.
/// Per §1.0 原則 3 "显式 > 隐式": explicit lifetimes are tracked by name.
pub(crate) fn lower_hir_ty_to_mir_ty_with_lifetimes(
    ty: &HirTy,
    region_counter: &mut u32,
    lifetime_map: &mut std::collections::HashMap<crate::lexer::Symbol, crate::mir::ty::RegionVid>,
) -> Ty {
    // Stage 18.57: Use the HIR Ty's span instead of Span::DUMMY.
    let span = ty.span;
    match &ty.kind {
        HirTyKind::Ref(region, mutability, inner) => {
            let mir_region = match region {
                Some(lt) => {
                    // Explicit lifetime — look up or create vid.
                    let name = lt.ident.name;
                    if let Some(&existing_vid) = lifetime_map.get(&name) {
                        Region::Var(existing_vid)
                    } else {
                        let vid = *region_counter;
                        *region_counter += 1;
                        let rvid = RegionVid(vid);
                        lifetime_map.insert(name, rvid);
                        Region::Var(rvid)
                    }
                }
                None => {
                    let vid = *region_counter;
                    *region_counter += 1;
                    Region::Var(RegionVid(vid))
                }
            };
            let mir_mut = match mutability {
                ast::Mutability::Mutable => crate::mir::ty::Mutability::Mutable,
                ast::Mutability::Immutable => crate::mir::ty::Mutability::Immutable,
            };
            Ty::new(
                TyKind::Ref(
                    mir_region,
                    mir_mut,
                    Box::new(lower_hir_ty_to_mir_ty_with_lifetimes(
                        inner,
                        region_counter,
                        lifetime_map,
                    )),
                ),
                span,
            )
        }
        HirTyKind::Tuple(tys) => Ty::new(
            TyKind::Tuple(
                tys.iter()
                    .map(|t| lower_hir_ty_to_mir_ty_with_lifetimes(t, region_counter, lifetime_map))
                    .collect(),
            ),
            span,
        ),
        HirTyKind::Slice(inner) => Ty::new(
            TyKind::Slice(Box::new(lower_hir_ty_to_mir_ty_with_lifetimes(
                inner,
                region_counter,
                lifetime_map,
            ))),
            span,
        ),
        HirTyKind::Array(inner, count_expr) => {
            let len_const = const_eval_array_len(count_expr, span);
            Ty::new(
                TyKind::Array(
                    Box::new(lower_hir_ty_to_mir_ty_with_lifetimes(
                        inner,
                        region_counter,
                        lifetime_map,
                    )),
                    Box::new(len_const),
                ),
                span,
            )
        }
        // Delegate to the non-lifetime variant for types without Ref.
        _ => lower_hir_ty_to_mir_ty_with_regions(ty, region_counter),
    }
}

/// Lower a HIR type to a MIR type.
/// Stage 16.51 (Task 11 Phase 1b): Lower generic args from a HIR path into
/// a MIR `SubstsRef`.
///
/// Walks `path.segments.last().args` (if any), extracts `GenericArg::Type`
/// args, lowers each to a MIR `Ty`, and collects into `SubstsRef`.
/// Lifetime and associated type args are skipped (not yet supported).
///
/// Returns an empty `SubstsRef` if the path has no generic args.
///
/// Stage 16.56 (Task 11 Phase 4b prerequisite): Now accepts `hir` parameter
/// to resolve nested generic type paths (e.g., `Box` in `Box<Box<i32>>`).
/// When `hir` is `Some`, AST paths in generic args are resolved to DefIds
/// by scanning HIR owners for matching type names. When `hir` is `None`,
/// unresolved paths produce `Error` (same as before).
///
/// Per §23: `lower_path_generic_args` follows `<verb>_<noun>_<adj>_<noun>`
/// pattern.
/// Per §16: reads HIR (path.args) during MIR lowering.
/// Stage 18.105 (S6 fix): Lower a HIR path's generic args to MIR substs,
/// with generics context for resolving bare type parameters.
///
/// When lowering a generic arg like `T` (a bare type parameter), this function
/// checks if `T` matches one of the `generic_params` (the type parameters of
/// the item being lowered). If so, it produces `Param(N)` instead of `Error`.
///
/// # Parameters
///
/// - `path`: the HIR path (e.g., `Box<T>` — path is `Box`, args contain `T`)
/// - `_region_counter`: unused (kept for API compat)
/// - `hir`: optional HIR crate for nested type resolution
/// - `generic_params`: the type parameters of the item being lowered
///   (e.g., for `fn make_box<T>(x: T) -> Box<T>`, this is `[ParamTy{T, index:0}]`)
///
/// Per §23: `lower_path_generic_args` follows `<verb>_<noun>_<adj>_<noun>`
/// pattern.
/// Per §1.0 原則 6 "通用 > 特例": one function for all generic arg lowering.
/// Per §2.0 原則 9 "正确 > 妥协": bare type params now resolve correctly (S6 fix).
pub(crate) fn lower_path_generic_args(
    path: &crate::hir::HirPath,
    _region_counter: &mut u32,
    hir: Option<&HirCrate>,
    generic_params: &[crate::mir::ty::ParamTy],
) -> crate::mir::ty::SubstsRef {
    use crate::ast::GenericArg;

    // Get the last segment's generic args (e.g., `Vec<i32>` → args on "Vec")
    let args = match path.segments.last().and_then(|s| s.args.as_ref()) {
        Some(args) => args,
        None => return Vec::new().into(),
    };

    // Extract angle-bracketed args (e.g., `<i32, bool>`)
    let arg_list = match args {
        crate::ast::GenericArgs::AngleBracketed(args) => args,
        // Parenthesized args (fn trait syntax) not yet supported
        _ => return Vec::new().into(),
    };

    // Lower each Type arg to MIR Ty, skip Lifetime and Assoc args.
    // Stage 16.56: Pass HIR to lower_ast_ty_to_mir_ty so nested generic
    // paths can be resolved (e.g., Box<Box<i32>> → inner Box resolved).
    // Stage 18.105 (S6 fix): Pass generic_params so bare type parameters
    // (e.g., `T` in `Box<T>`) resolve to Param(N) instead of Error.
    let substs: Vec<crate::mir::ty::Ty> = arg_list
        .iter()
        .filter_map(|arg| match arg {
            GenericArg::Type(ty) => Some(lower_ast_ty_to_mir_ty_with_generics(
                ty,
                hir,
                generic_params,
            )),
            _ => None, // Skip Lifetime and Assoc args
        })
        .collect();

    substs.into()
}

/// Stage 16.56: Look up a type DefId by name from HIR owners.
///
/// Scans all HIR owners for a struct or enum with the given name.
/// Returns the first match (DefId). If multiple types share the same
/// name, the first one found is returned (this is a limitation — full
/// name resolution with module paths is future work).
///
/// Per §23: `lookup_type_def_id_by_name` follows `<verb>_<noun>_<noun>`
/// _<prep>_<noun>` pattern.
/// Per §16: reads HIR (allowed during MIR lowering).
fn lookup_type_def_id_by_name(
    hir: &HirCrate,
    name: crate::lexer::Symbol,
) -> Option<crate::hir::DefId> {
    for (def_id, owner) in &hir.owners {
        match owner {
            crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s)) if s.ident.name == name => {
                return Some(*def_id);
            }
            crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(e)) if e.ident.name == name => {
                return Some(*def_id);
            }
            _ => {}
        }
    }
    None
}

/// Stage 16.51 (Task 11 Phase 1b): Lower an AST `Ty` to a MIR `Ty`.
///
/// This is a minimal lowerer for generic type arguments (e.g., `i32` in
/// `Vec<i32>`). It handles the common cases: primitives, paths (struct/enum
/// refs), tuples, arrays, references.
///
/// Stage 16.56 (Task 11 Phase 4b prerequisite): Now accepts `hir` parameter.
/// When `hir` is `Some`, AST paths are resolved to DefIds by scanning HIR
/// owners for matching type names. This enables nested generic types like
/// `Box<Box<i32>>` where the inner `Box<i32>` is an AST path that needs
/// resolution.
///
/// Per §23: `lower_ast_ty_to_mir_ty` follows `<verb>_<noun>_<noun>_<noun>`
/// pattern.
pub(crate) fn lower_ast_ty_to_mir_ty(
    ty: &crate::ast::Ty,
    hir: Option<&HirCrate>,
) -> crate::mir::ty::Ty {
    lower_ast_ty_to_mir_ty_with_generics(ty, hir, &[])
}

/// Stage 18.105 (S6 fix): Lower an AST type to MIR type with generics context.
///
/// This is the same as `lower_ast_ty_to_mir_ty` but additionally checks if
/// a bare path name matches one of the `generic_params`. If so, it produces
/// `Param(N)` instead of `Error`.
///
/// This fixes S6: generic function return types like `Box<T>` now correctly
/// produce `Adt(Box, [Param(0)])` instead of `Adt(Box, [Error])`.
///
/// Per §23: `lower_ast_ty_to_mir_ty_with_generics` follows
/// `<verb>_<noun>_<noun>_<prep>_<noun>` pattern.
/// Per §1.0 原則 6 "通用 > 特例": one function for all AST type lowering.
pub(crate) fn lower_ast_ty_to_mir_ty_with_generics(
    ty: &crate::ast::Ty,
    hir: Option<&HirCrate>,
    generic_params: &[crate::mir::ty::ParamTy],
) -> crate::mir::ty::Ty {
    use crate::ast::Ty as ATy;
    let span = crate::session::Span::DUMMY;
    match ty {
        ATy::Bool(_) => crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Bool, span),
        ATy::Char(_) => crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Char, span),
        ATy::Int(int_ty, _) => crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Int(*int_ty), span),
        ATy::Uint(uint_ty, _) => {
            crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Uint(*uint_ty), span)
        }
        ATy::Float(float_ty, _) => {
            crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Float(*float_ty), span)
        }
        ATy::Tuple(tys, _) => {
            let mir_tys: Vec<_> = tys
                .iter()
                .map(|t| lower_ast_ty_to_mir_ty_with_generics(t, hir, generic_params))
                .collect();
            crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Tuple(mir_tys), span)
        }
        ATy::Path(_, path, _) => {
            // Stage 18.105 (S6 fix): First, check if the path is a bare type
            // parameter (single-segment, name matches a generic param).
            // Per §1.0 原則 6 "通用 > 特例": check generic params before
            // falling back to struct/enum lookup.
            if path.segments.len() == 1 {
                if let Some(last_seg) = path.segments.last() {
                    let name = last_seg.ident.name;
                    // Check if this name matches a generic type parameter.
                    for param in generic_params {
                        if param.name == name {
                            return crate::mir::ty::Ty::new(
                                crate::mir::ty::TyKind::Param(*param),
                                span,
                            );
                        }
                    }
                }
            }

            // Stage 16.56: When HIR is available, try to resolve the AST path
            // to a DefId by looking up the type name in HIR owners.
            if let Some(hir_crate) = hir {
                if let Some(last_seg) = path.segments.last() {
                    if let Some(def_id) = lookup_type_def_id_by_name(hir_crate, last_seg.ident.name)
                    {
                        // Resolve the path's generic args recursively.
                        // Stage 18.105: Pass generic_params for nested bare params.
                        let inner_substs: Vec<_> = last_seg
                            .args
                            .as_ref()
                            .map(|args| match args {
                                crate::ast::GenericArgs::AngleBracketed(args) => args
                                    .iter()
                                    .filter_map(|a| match a {
                                        crate::ast::GenericArg::Type(t) => {
                                            Some(lower_ast_ty_to_mir_ty_with_generics(
                                                t,
                                                hir,
                                                generic_params,
                                            ))
                                        }
                                        _ => None,
                                    })
                                    .collect(),
                                _ => Vec::new(),
                            })
                            .unwrap_or_default();
                        return crate::mir::ty::Ty::new(
                            crate::mir::ty::TyKind::Adt(def_id, inner_substs.into()),
                            span,
                        );
                    }
                }
            }
            // HIR not available or name not found → Error.
            crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Error, span)
        }
        // For unsupported types, return Infer (will be resolved by typeck)
        _ => crate::mir::ty::Ty::new(
            crate::mir::ty::TyKind::Infer(crate::mir::ty::InferVar::TyVar(crate::mir::ty::TyVid(
                u32::MAX,
            ))),
            span,
        ),
    }
}

pub(crate) fn lower_hir_ty_to_mir_ty(ty: &HirTy) -> Ty {
    lower_hir_ty_to_mir_ty_with_hir(ty, None)
}

/// Stage 18.105 (S6 fix): Lower a HIR type to MIR type with generics context + HIR.
///
/// This is the same as `lower_hir_ty_to_mir_ty` but additionally passes
/// `generic_params` to `lower_path_generic_args` so bare type parameters
/// (e.g., `T` in `Box<T>`) resolve to `Param(N)` instead of `Error`.
///
/// Per §23: `lower_hir_ty_to_mir_ty_with_hir_and_generics` follows
/// `<verb>_<noun>_<noun>_<prep>_<noun>_<prep>_<noun>` pattern.
pub(crate) fn lower_hir_ty_to_mir_ty_with_hir_and_generics(
    ty: &HirTy,
    hir: Option<&HirCrate>,
    generic_params: &[crate::mir::ty::ParamTy],
) -> Ty {
    let mut region_counter = 0u32;
    lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
        ty,
        &mut region_counter,
        hir,
        generic_params,
    )
}

/// Stage 16.56: Lower a HIR type to MIR type with optional HIR access.
///
/// This is the preferred entry point for callers that have HIR access.
/// When `hir` is `Some`, nested generic type paths are resolved correctly
/// (e.g., `Box<Box<i32>>` → inner `Box` resolved to its DefId).
///
/// Per §23: `lower_hir_ty_to_mir_ty_with_hir` follows
/// `<verb>_<noun>_<noun>_<prep>_<noun>` pattern.
pub(crate) fn lower_hir_ty_to_mir_ty_with_hir(ty: &HirTy, hir: Option<&HirCrate>) -> Ty {
    let mut region_counter = 0u32;
    lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(ty, &mut region_counter, hir, &[])
}

/// Stage 15.49 (HP-5 step 2): Lower a HIR type to MIR type with proper
/// region assignment.
///
/// Unlike `lower_hir_ty_to_mir_ty`, this function assigns a fresh
/// `Region::Var(RegionVid(n))` to each elided reference lifetime, where
/// `n` is obtained from `region_counter` (incremented per allocation).
/// This gives the region inference infrastructure real region variables
/// to work with, instead of `Region::Erased` (which maps to `'static`).
///
/// Per §23: function name follows `<verb>_<noun>_<noun>_<prep>_<noun>`
/// pattern with `_with_regions` suffix.
/// Per §1.0 原則 3 "显式 > 隐式": regions are explicit in the MIR.
pub(crate) fn lower_hir_ty_to_mir_ty_with_regions(ty: &HirTy, region_counter: &mut u32) -> Ty {
    lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(ty, region_counter, None, &[])
}

/// Stage 16.56: Region-aware HIR→MIR type lowering with optional HIR access.
///
/// This is the main implementation. When `hir` is `Some`, nested generic
/// type paths are resolved correctly (e.g., `Box<Box<i32>>` → inner `Box`
/// resolved to its DefId).
///
/// Stage 18.105 (S6 fix): Added `generic_params` parameter so bare type
/// parameters (e.g., `T` in `Box<T>`) resolve to `Param(N)`.
///
/// Per §23: function name follows `<verb>_<noun>_<noun>_<prep>_<noun>_<prep>_<noun>`
/// pattern with `_with_regions_and_hir_and_generics` suffix.
fn lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
    ty: &HirTy,
    region_counter: &mut u32,
    hir: Option<&HirCrate>,
    generic_params: &[crate::mir::ty::ParamTy],
) -> Ty {
    // Stage 18.57: Use the HIR Ty's span instead of Span::DUMMY.
    // Per §1.0 原則 3 "显式 > 隐式": span is explicitly propagated from HIR.
    // Per §1.0 原則 4 "报错 > 静默": accurate spans improve diagnostics.
    let span = ty.span;
    match &ty.kind {
        HirTyKind::Bool => Ty::new(TyKind::Bool, span),
        HirTyKind::Char => Ty::new(TyKind::Char, span),
        HirTyKind::Int(int_ty) => Ty::new(TyKind::Int(*int_ty), span),
        HirTyKind::Uint(uint_ty) => Ty::new(TyKind::Uint(*uint_ty), span),
        HirTyKind::Float(float_ty) => Ty::new(TyKind::Float(*float_ty), span),
        HirTyKind::Never => Ty::new(TyKind::Never, span),
        HirTyKind::Tuple(tys) => Ty::new(
            TyKind::Tuple(
                tys.iter()
                    .map(|t| {
                        lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
                            t,
                            region_counter,
                            hir,
                            generic_params,
                        )
                    })
                    .collect(),
            ),
            span,
        ),
        HirTyKind::Ref(region, mutability, inner) => {
            let mir_region = match region {
                Some(_) => {
                    let vid = *region_counter;
                    *region_counter += 1;
                    Region::Var(RegionVid(vid))
                }
                None => {
                    let vid = *region_counter;
                    *region_counter += 1;
                    Region::Var(RegionVid(vid))
                }
            };
            let mir_mut = match mutability {
                ast::Mutability::Mutable => crate::mir::ty::Mutability::Mutable,
                ast::Mutability::Immutable => crate::mir::ty::Mutability::Immutable,
            };
            Ty::new(
                TyKind::Ref(
                    mir_region,
                    mir_mut,
                    Box::new(lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
                        inner,
                        region_counter,
                        hir,
                        generic_params,
                    )),
                ),
                span,
            )
        }
        HirTyKind::Ptr(mutability, inner) => {
            let mir_mut = match mutability {
                ast::Mutability::Mutable => crate::mir::ty::Mutability::Mutable,
                ast::Mutability::Immutable => crate::mir::ty::Mutability::Immutable,
            };
            Ty::new(
                TyKind::RawPtr(
                    mir_mut,
                    Box::new(lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
                        inner,
                        region_counter,
                        hir,
                        generic_params,
                    )),
                ),
                span,
            )
        }
        HirTyKind::Slice(inner) => Ty::new(
            TyKind::Slice(Box::new(
                lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
                    inner,
                    region_counter,
                    hir,
                    generic_params,
                ),
            )),
            span,
        ),
        HirTyKind::Array(inner, count_expr) => {
            let len_const = const_eval_array_len(count_expr, span);
            Ty::new(
                TyKind::Array(
                    Box::new(lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
                        inner,
                        region_counter,
                        hir,
                        generic_params,
                    )),
                    Box::new(len_const),
                ),
                span,
            )
        }
        HirTyKind::Infer => Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(u32::MAX))), span),
        HirTyKind::Path(qself, path) => {
            // Stage 18.53 GATs Phase 2: If `qself.ty` is `Some`, this is a
            // qualified path `<T as Trait>::Item` — lower to `TyKind::Projection`
            // so `projection_resolver` can resolve it to the concrete type
            // from the impl block.
            //
            // Per §1.0 原則 3 "显式 > 隐式": projection is explicitly
            // represented as `TyKind::Projection(assoc_def_id, substs)`,
            // not implicitly folded into `TyKind::Adt`.
            // Per §1.0 原則 5 "去除兼容思维": the old code ignored qself
            // via `_` — that path is removed; qualified paths now produce
            // projections, plain paths produce Adt.
            if let Some(inner_ty) = &qself.ty {
                lower_qualified_path_to_projection(inner_ty, path, region_counter, hir, span)
            } else {
                // Plain path: existing behavior.
                match path.res {
                    Res::Def(def_id, _) => {
                        // Stage 16.56: Pass HIR to lower_path_generic_args so
                        // nested generic paths can be resolved.
                        // Stage 18.105 (S6 fix): Pass generic_params for bare type params.
                        let substs =
                            lower_path_generic_args(path, region_counter, hir, generic_params);
                        Ty::new(TyKind::Adt(def_id, substs), span)
                    }
                    Res::PrimTy(PrimTy::Str) => Ty::new(TyKind::Str, span),
                    // Stage 18.54: Generic type parameter (e.g., `T` in `fn f<T>(x: T)`).
                    // Lower to TyKind::Param so monomorphization can substitute it.
                    // Per §1.0 原則 6 "通用 > 特例": reuse existing ParamTy.
                    Res::GenericParam(name, idx) => {
                        let param = crate::mir::ty::ParamTy {
                            index: idx as u32,
                            name,
                        };
                        Ty::new(TyKind::Param(param), span)
                    }
                    // Stage 18.62: Res::Err/Res::Unknown/Res::Local/Res::SelfTy
                    // reaching here means the resolver couldn't resolve the type path.
                    // The resolver may have already pushed a ResolveError, but if not
                    // (e.g. Res::Unknown for body-local types), we return Error.
                    // Per §1.0 原則 4 "报错 > 静默": TyKind::Error is the fallback,
                    // and the resolver's scan_for_unresolved_paths will report it.
                    _ => Ty::new(TyKind::Error, span),
                }
            }
        }
        HirTyKind::FnPtr {
            inputs,
            output,
            abi,
            is_unsafe,
        } => {
            let mir_inputs: Vec<Ty> = inputs
                .iter()
                .map(|t| {
                    lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
                        t,
                        region_counter,
                        hir,
                        generic_params,
                    )
                })
                .collect();
            let mir_output = Box::new(lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
                output,
                region_counter,
                hir,
                generic_params,
            ));
            Ty::new(
                TyKind::FnPtr(crate::mir::ty::Sig {
                    inputs: mir_inputs,
                    output: mir_output,
                    abi: *abi,
                    is_unsafe: *is_unsafe,
                }),
                span,
            )
        }
        // Stage 18.62: Unsupported HirTyKind — return Error.
        _ => Ty::new(TyKind::Error, span),
    }
}

/// Stage 18.53 GATs Phase 2: Lower a qualified path `<T as Trait>::Item` to
/// `TyKind::Projection(assoc_def_id, substs)`.
///
/// ## Algorithm
///
/// 1. Lower the inner type `T` to MIR `Ty` — this becomes `substs[0]` (self type).
/// 2. Extract the trait path from `path.segments[..qself.position]` and the
///    assoc item name from `path.segments[qself.position]` (the segment after
///    the trait).
/// 3. Look up the assoc type's `DefId` by searching traits for a matching
///    `HirAssocType`. If not found, return `TyKind::Error` (graceful
///    degradation — Phase 3 will improve this).
/// 4. Lower the path's generic args (if any) to `substs[1..]`.
/// 5. Return `TyKind::Projection(assoc_def_id, substs)`.
///
/// Per §1.0 原則 3 "显式 > 隐式": projection is explicit.
/// Per §1.0 原則 4 "报错 > 静默": if assoc type not found, return `TyKind::Error`
/// (which surfaces in typeck as an error), not a silent fallback.
/// Per §10 naming: `lower_qualified_path_to_projection` follows
/// `<verb>_<noun>_<prep>_<noun>` pattern.
pub(crate) fn lower_qualified_path_to_projection(
    inner_ty: &HirTy,
    path: &crate::hir::HirPath,
    region_counter: &mut u32,
    hir: Option<&HirCrate>,
    span: Span,
) -> Ty {
    // Step 1: Lower the inner self type T.
    let self_ty = lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
        inner_ty,
        region_counter,
        hir,
        &[],
    );

    // Step 2: The last segment of the path is the assoc item name.
    let assoc_segment = path.segments.last().expect(
        "qualified path must have at least one segment after `>::` — \
         parser guarantees this",
    );

    // Step 3: Stage 18.56 — Use path.res (set by resolver) as the trait DefId.
    // The resolver now validates that the assoc type exists in the trait
    // (per §1.0 原則 4 "报错 > 静默"). If res is Res::Def, the trait is valid.
    // If res is Res::Err, the resolver already emitted an error.
    let trait_def_id = match path.res {
        crate::hir::Res::Def(def_id, _) => Some(def_id),
        _ => None,
    };

    // Step 4: Lower generic args from the assoc segment (e.g., `Item<'a, T>`).
    // Per §1.0 原則 6 "通用 > 特例": reuse `lower_ast_ty_to_mir_ty` rather
    // than duplicating AST→MIR lowering.
    let mut substs: Vec<Ty> = Vec::new();
    substs.push(self_ty);
    if let Some(crate::ast::GenericArgs::AngleBracketed(arg_list)) = &assoc_segment.args {
        for arg in arg_list {
            if let crate::ast::GenericArg::Type(ty) = arg {
                substs.push(lower_ast_ty_to_mir_ty(ty, hir));
            }
            // Lifetimes in GAT projections are erased for Stage 18.55.
            // Phase 4 will handle region-aware monomorphization.
        }
    }

    match trait_def_id {
        // Stage 18.56: Use trait_def_id from resolver (soundness fix).
        // Per §1.0 原則 9 "正确 > 妥协": trait qualifier is now respected.
        Some(def_id) => Ty::new(TyKind::Projection(def_id, substs.into()), span),
        None => {
            // Resolver already emitted an error for this case.
            // Return Error so downstream typeck doesn't crash.
            Ty::new(TyKind::Error, span)
        }
    }
}

/// Stage 16.53 (Task 11 Phase 2): Lower a HIR type to MIR type with generic
/// type parameter resolution.
///
/// This is an extension of `lower_hir_ty_to_mir_ty_with_regions` that
/// resolves generic type parameters (e.g., `T` in `struct Box<T> { val: T }`)
/// to `TyKind::Param(ParamTy { index, name })`.
///
/// ## Generic Param Resolution
///
/// When the HIR path's `Res` is `Res::Err` (unresolved by the name resolver),
/// we check if the path's single segment name matches one of the `generic_params`.
/// If it matches, we produce `TyKind::Param(ParamTy { index, name })` instead
/// of `TyKind::Error`. This is the key step that makes `substitute` useful —
/// without it, generic field types would be `Error` and substitution would
/// be a no-op.
///
/// ## When to Use
///
/// Use this function when lowering types inside a generic context (e.g.,
/// struct/enum field types, generic fn signatures). Use the plain
/// `lower_hir_ty_to_mir_ty` for non-generic contexts.
///
/// Per §23: `lower_hir_ty_to_mir_ty_with_generics` follows
/// `<verb>_<noun>_<noun>_<prep>_<noun>` pattern.
/// Per §16: reads HIR (allowed during MIR lowering).
/// Per §1.0 原則 6 "通用 > 特例": one function for all generic type lowering.
pub(crate) fn lower_hir_ty_to_mir_ty_with_generics(
    ty: &HirTy,
    generic_params: &[crate::mir::ty::ParamTy],
) -> Ty {
    let mut region_counter = 0u32;
    lower_hir_ty_to_mir_ty_with_generics_and_regions(ty, generic_params, &mut region_counter)
}

/// Stage 16.53: Region-aware variant of `lower_hir_ty_to_mir_ty_with_generics`.
///
/// Per §23: `lower_hir_ty_to_mir_ty_with_generics_and_regions` follows
/// `<verb>_<noun>_<noun>_<prep>_<noun>_<prep>_<noun>` pattern.
fn lower_hir_ty_to_mir_ty_with_generics_and_regions(
    ty: &HirTy,
    generic_params: &[crate::mir::ty::ParamTy],
    region_counter: &mut u32,
) -> Ty {
    // Stage 18.57: Use the HIR Ty's span instead of Span::DUMMY.
    let span = ty.span;
    match &ty.kind {
        // For Path types, check if it's a generic type param first.
        HirTyKind::Path(_, path) => {
            // Single-segment path with unresolved Res → might be a type param.
            if path.segments.len() == 1 && matches!(path.res, Res::Err | Res::Unknown) {
                let seg_name = path.segments[0].ident.name;
                for param in generic_params {
                    if param.name == seg_name {
                        return Ty::new(TyKind::Param(*param), span);
                    }
                }
            }
            // Not a type param — delegate to the standard lowerer.
            lower_hir_ty_to_mir_ty_with_regions(ty, region_counter)
        }
        // For recursive types (Tuple, Ref, Array, etc.), recurse with generics.
        HirTyKind::Tuple(tys) => Ty::new(
            TyKind::Tuple(
                tys.iter()
                    .map(|t| {
                        lower_hir_ty_to_mir_ty_with_generics_and_regions(
                            t,
                            generic_params,
                            region_counter,
                        )
                    })
                    .collect(),
            ),
            span,
        ),
        HirTyKind::Ref(region, mutability, inner) => {
            let mir_region = match region {
                Some(_) => {
                    let vid = *region_counter;
                    *region_counter += 1;
                    crate::mir::ty::Region::Var(crate::mir::ty::RegionVid(vid))
                }
                None => {
                    let vid = *region_counter;
                    *region_counter += 1;
                    crate::mir::ty::Region::Var(crate::mir::ty::RegionVid(vid))
                }
            };
            let mir_mut = match mutability {
                ast::Mutability::Mutable => crate::mir::ty::Mutability::Mutable,
                ast::Mutability::Immutable => crate::mir::ty::Mutability::Immutable,
            };
            Ty::new(
                TyKind::Ref(
                    mir_region,
                    mir_mut,
                    Box::new(lower_hir_ty_to_mir_ty_with_generics_and_regions(
                        inner,
                        generic_params,
                        region_counter,
                    )),
                ),
                span,
            )
        }
        HirTyKind::Ptr(mutability, inner) => {
            let mir_mut = match mutability {
                ast::Mutability::Mutable => crate::mir::ty::Mutability::Mutable,
                ast::Mutability::Immutable => crate::mir::ty::Mutability::Immutable,
            };
            Ty::new(
                TyKind::RawPtr(
                    mir_mut,
                    Box::new(lower_hir_ty_to_mir_ty_with_generics_and_regions(
                        inner,
                        generic_params,
                        region_counter,
                    )),
                ),
                span,
            )
        }
        HirTyKind::Slice(inner) => Ty::new(
            TyKind::Slice(Box::new(lower_hir_ty_to_mir_ty_with_generics_and_regions(
                inner,
                generic_params,
                region_counter,
            ))),
            span,
        ),
        HirTyKind::Array(inner, count_expr) => {
            let len_const = const_eval_array_len(count_expr, span);
            Ty::new(
                TyKind::Array(
                    Box::new(lower_hir_ty_to_mir_ty_with_generics_and_regions(
                        inner,
                        generic_params,
                        region_counter,
                    )),
                    Box::new(len_const),
                ),
                span,
            )
        }
        // All other kinds delegate to the standard lowerer (no generics needed).
        _ => lower_hir_ty_to_mir_ty_with_regions(ty, region_counter),
    }
}

/// Stage 13.18: Resolve the type of a `self` parameter from the owning impl block.
///
/// Given a `Body` (which is owned by an impl method), find the impl block in HIR
/// and return its `self_ty` as a MIR type. This allows `self.x` field access to
/// work — the self param's MIR type becomes `Adt(P, [])` instead of `Infer(TyVar)`.
///
/// Returns `None` if:
/// - HIR is not available
/// - The body's owner is not an impl method (e.g., free fn with self-like param)
/// - The impl block's self_ty can't be lowered
///
/// Per §16: this is a HIR query at MIR-lowering time. The result type is sunk
/// into `local_decls` as data, so codegen doesn't need HIR.
fn resolve_self_param_type(
    cx: &MirLowerCtxt,
    body: &Body,
    self_kind: Option<crate::ast::SelfKind>,
) -> Option<crate::mir::ty::Ty> {
    let hir = cx.hir?;
    // The body's owner DefId — for impl methods, this is the HirFn's owner.
    let _owner_def_id = body.hir_id.owner;

    // Helper: wrap an ADT type as &T/&mut T based on self_kind.
    let wrap_with_ref = |adt_ty: crate::mir::ty::Ty| -> crate::mir::ty::Ty {
        match self_kind {
            Some(crate::ast::SelfKind::Ref(mutability)) => {
                let mir_mut = match mutability {
                    crate::ast::Mutability::Mutable => crate::mir::ty::Mutability::Mutable,
                    crate::ast::Mutability::Immutable => crate::mir::ty::Mutability::Immutable,
                };
                crate::mir::ty::Ty::new(
                    crate::mir::ty::TyKind::Ref(
                        crate::mir::ty::Region::Erased,
                        mir_mut,
                        Box::new(adt_ty),
                    ),
                    body.span,
                )
            }
            // self by value — no wrapping
            _ => adt_ty,
        }
    };

    // Search all owners for an Impl block that contains this method.
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
            // Check if this impl block contains a method whose body matches.
            for impl_item in &impl_block.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    if f.body
                        == Some(crate::hir::BodyId {
                            owner: crate::hir::OwnerId(body.hir_id.owner),
                        })
                    {
                        // Found the owning impl block! Lower its self_ty.
                        // Stage 14.19 (GAP-31): For &self/&mut self, wrap the
                        // type in TyKind::Ref so the self param is a reference.
                        // This makes mutations propagate to the caller.
                        // The codegen Deref+Field handling has been fixed in
                        // mir_translation.rs to support this correctly.
                        let adt_ty = lower_hir_ty_to_mir_ty(&impl_block.self_ty);
                        return Some(wrap_with_ref(adt_ty));
                    }
                }
            }
        }
    }

    // Stage 14.97 (Bug Y1 fix): Trait default body methods.
    //
    // If no impl block owns this body, check if a Trait block owns it
    // (i.e., this is a trait default body). For trait default bodies, the
    // self type is `Self` — a type parameter that's unknown without
    // monomorphization. For v0.1, we use a single-impl heuristic: if exactly
    // one impl of the trait exists in the program, use that impl's self_ty
    // as the specialization type. This is correct for the common case of
    // `trait T { fn f(&self) {...} } impl T for Type { ... }` with one impl.
    //
    // Limitation: If multiple impls exist, we use the first impl's self_ty.
    // This is wrong for the other impls but is a v0.1 limitation (full
    // monomorphization is v0.2+ work). The alternative (returning None and
    // leaving self as Infer) causes worse failures (LLVM crashes).
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
            for trait_item in &t.items {
                if let crate::hir::HirTraitItem::Fn(f) = trait_item {
                    if f.body
                        == Some(crate::hir::BodyId {
                            owner: crate::hir::OwnerId(body.hir_id.owner),
                        })
                    {
                        // Found the owning trait! Find impls of this trait.
                        let trait_name = t.ident.name;
                        let impls: Vec<_> = hir
                            .owners
                            .iter()
                            .filter_map(|(_, o)| {
                                if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(
                                    impl_block,
                                )) = o
                                {
                                    if impl_block
                                        .of_trait
                                        .as_ref()
                                        .and_then(|p| p.segments.last().map(|s| s.ident.name))
                                        == Some(trait_name)
                                    {
                                        return Some(impl_block);
                                    }
                                }
                                None
                            })
                            .collect();

                        // Use the first impl's self_ty as the specialization type.
                        if let Some(impl_block) = impls.first() {
                            let adt_ty = lower_hir_ty_to_mir_ty(&impl_block.self_ty);
                            return Some(wrap_with_ref(adt_ty));
                        }
                        // No impls exist — fall through to return None.
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod stage15_90_tests {
    use super::*;
    use crate::mir::ty::{Mutability, Region, RegionVid, TyKind};

    /// Stage 15.90: Verify `collect_region_vids` collects vids from Ref types.
    #[test]
    fn collect_region_vids_basic() {
        // &i32 with Region::Var(5)
        let ty = Ty::new(
            TyKind::Ref(
                Region::Var(RegionVid(5)),
                Mutability::Immutable,
                Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
            ),
            Span::DUMMY,
        );
        let mut vids = Vec::new();
        collect_region_vids(&ty, &mut vids);
        assert_eq!(vids, vec![RegionVid(5)]);
    }

    /// Stage 15.90: Verify `collect_region_vids` collects from nested types.
    #[test]
    fn collect_region_vids_nested() {
        // &(&i32, &i32) with regions 1 and 2
        let inner1 = Ty::new(
            TyKind::Ref(
                Region::Var(RegionVid(1)),
                Mutability::Immutable,
                Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
            ),
            Span::DUMMY,
        );
        let inner2 = Ty::new(
            TyKind::Ref(
                Region::Var(RegionVid(2)),
                Mutability::Immutable,
                Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
            ),
            Span::DUMMY,
        );
        let tuple = Ty::new(TyKind::Tuple(vec![inner1, inner2]), Span::DUMMY);
        let mut vids = Vec::new();
        collect_region_vids(&tuple, &mut vids);
        assert_eq!(vids, vec![RegionVid(1), RegionVid(2)]);
    }

    /// Stage 15.90: Verify `apply_elision_rules` with single input lifetime (rule 2).
    #[test]
    fn apply_elision_rule_2_single_input() {
        // Return type: &i32 with Region::Var(10) (fresh output vid)
        let return_ty = Ty::new(
            TyKind::Ref(
                Region::Var(RegionVid(10)),
                Mutability::Immutable,
                Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
            ),
            Span::DUMMY,
        );
        // Input: single lifetime vid 3
        let input_vids = vec![RegionVid(3)];
        let result = apply_elision_rules(&return_ty, &input_vids, None);
        // The output lifetime should be replaced with vid 3.
        match &result.kind {
            TyKind::Ref(region, _, _) => {
                assert_eq!(region, &Region::Var(RegionVid(3)));
            }
            _ => panic!("expected Ref"),
        }
    }

    /// Stage 15.90: Verify `apply_elision_rules` with multiple input lifetimes
    /// and no self → does NOT apply (keeps original output lifetime).
    #[test]
    fn apply_elision_rule_2_multiple_inputs_no_self() {
        let return_ty = Ty::new(
            TyKind::Ref(
                Region::Var(RegionVid(10)),
                Mutability::Immutable,
                Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
            ),
            Span::DUMMY,
        );
        // Input: multiple lifetime vids, no self
        let input_vids = vec![RegionVid(1), RegionVid(2)];
        let result = apply_elision_rules(&return_ty, &input_vids, None);
        // The output lifetime should NOT be replaced (keeps vid 10).
        match &result.kind {
            TyKind::Ref(region, _, _) => {
                assert_eq!(region, &Region::Var(RegionVid(10)));
            }
            _ => panic!("expected Ref"),
        }
    }

    /// Stage 15.91: Verify `apply_elision_rules` with multiple input lifetimes
    /// AND self lifetime (rule 3) → uses self lifetime for output.
    #[test]
    fn apply_elision_rule_3_self_lifetime() {
        let return_ty = Ty::new(
            TyKind::Ref(
                Region::Var(RegionVid(10)),
                Mutability::Immutable,
                Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
            ),
            Span::DUMMY,
        );
        // Input: multiple lifetime vids (1=self, 2=other param)
        let input_vids = vec![RegionVid(1), RegionVid(2)];
        // Self lifetime is vid 1
        let self_vid = Some(RegionVid(1));
        let result = apply_elision_rules(&return_ty, &input_vids, self_vid);
        // Rule 3: the output lifetime should be replaced with self's vid 1.
        match &result.kind {
            TyKind::Ref(region, _, _) => {
                assert_eq!(region, &Region::Var(RegionVid(1)));
            }
            _ => panic!("expected Ref"),
        }
    }

    /// Stage 15.90: Verify `apply_elision_rules` with no input lifetimes
    /// does NOT apply (keeps original output lifetime).
    #[test]
    fn apply_elision_rule_2_no_inputs() {
        let return_ty = Ty::new(
            TyKind::Ref(
                Region::Var(RegionVid(10)),
                Mutability::Immutable,
                Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
            ),
            Span::DUMMY,
        );
        let input_vids: Vec<RegionVid> = vec![];
        let result = apply_elision_rules(&return_ty, &input_vids, None);
        match &result.kind {
            TyKind::Ref(region, _, _) => {
                assert_eq!(region, &Region::Var(RegionVid(10)));
            }
            _ => panic!("expected Ref"),
        }
    }
}

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
        let ty1 =
            lower_hir_ty_to_mir_ty_with_lifetimes(&ref_ty, &mut region_counter, &mut lifetime_map);
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
        let ty2 =
            lower_hir_ty_to_mir_ty_with_lifetimes(&ref_ty2, &mut region_counter, &mut lifetime_map);
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

        let ty1 =
            lower_hir_ty_to_mir_ty_with_lifetimes(&ref_ty, &mut region_counter, &mut lifetime_map);
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
        let ty2 =
            lower_hir_ty_to_mir_ty_with_lifetimes(&ref_ty2, &mut region_counter, &mut lifetime_map);
        let vid2 = match &ty2.kind {
            TyKind::Ref(Region::Var(vid), _, _) => *vid,
            _ => panic!("expected Ref with Region::Var"),
        };

        // Elided lifetimes should get different vids.
        assert_ne!(vid1, vid2, "elided lifetimes should NOT share vid");
    }
}
