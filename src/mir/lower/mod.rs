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
pub use writeback::{writeback_closures, writeback_type_propagation};
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
    /// Per `stage-13.3-design-alignment.md` §4, the long-term plan is
    /// Strategy A (synthesized `call` function per closure); Stage 13.3a
    /// implements the inline approach as a pragmatic subset to make the
    /// common case (`let f = |x| ...; f(5);`) work without the full
    /// synthesized-MirBody infrastructure.
    pub closure_bodies: std::collections::HashMap<LocalId, ClosureBodyInfo>,
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
}

/// Stage 13.3a (TD-030): Information about a closure literal, stored in
/// `MirLowerCtxt.closure_bodies` keyed by the LocalId holding the closure
/// struct value. Used by `HirExprKind::Call` to inline the closure body at
/// the call site.
///
/// Fields:
/// - `params`: the closure's declared parameters (HIR). At the call site,
///   each param is bound to the corresponding call argument local.
/// - `body`: the closure's body expression (HIR). Lowered inline at each
///   call site.
/// - `captures`: list of (HirId of the captured binding, capture field type).
///   The capture field index in the closure struct = the vec index. At the
///   call site, each capture is extracted via
///   `Place::Projection(closure_local, Field(i, cap_ty))`.
///
/// Per §16: this side-table carries HIR-derived data downstream to the call
/// site. The lowering context reads HIR (allowed — MIR lower is downstream
/// of HIR). No HIR access from codegen.
#[derive(Clone, Debug)]
pub struct ClosureBodyInfo {
    /// The closure's declared parameters (HIR).
    pub params: Vec<HirParam>,
    /// The closure's body expression (HIR).
    pub body: Box<HirExpr>,
    /// Captured locals: (HirId of the captured binding, capture field type).
    /// The capture field index in the closure struct = the vec index.
    pub captures: Vec<(HirId, Ty)>,
}

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
            closure_bodies: std::collections::HashMap::new(),
            loop_stack: Vec::new(),
            loop_result_locals: Vec::new(),
            type_errors: Vec::new(),
            method_return_type_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
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
    pub fn local_of(&self, hir_id: HirId) -> Option<LocalId> {
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
    /// flow (short-circuit evaluation) via `lower_short_circuit`. This
    /// function panics if called with `And` or `Or` to force callers to
    /// route them correctly.
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
            // Logical and/or must be lowered to control flow, not BitOp.
            // Caller must route them through lower_short_circuit.
            HirBinOp::And | HirBinOp::Or => panic!(
                "lower_bin_op called with {:?} — caller must route And/Or to lower_short_circuit",
                op
            ),
        }
    }

    /// Convert a HIR UnaryOp to a MIR UnOp.
    ///
    /// **Note**: `HirUnaryOp::Deref` is NOT a real unary op in MIR —
    /// it is a projection (`*p` reads the place `Projection(p, Deref)`).
    /// Callers must handle `Deref` separately before calling this function.
    /// This function only converts `Neg`/`Not`; if passed `Deref` it will
    /// panic (signaling a caller bug — the caller should have routed Deref
    /// to `lower_deref_expr` instead).
    pub fn lower_un_op(op: HirUnaryOp) -> UnOp {
        match op {
            HirUnaryOp::Neg => UnOp::Neg,
            HirUnaryOp::Not => UnOp::Not,
            HirUnaryOp::Deref => panic!(
                "lower_un_op called with Deref — caller must route Deref to lower_deref_expr"
            ),
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
    // Stage 15.12: lower_full now returns 3-tuple (mir, unify, type_errors).
    // The convenience wrappers discard unify + type_errors for callers that
    // only need the MirBody (e.g., tests).
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
) -> (MirBody, UnificationTable, Vec<crate::typeck::TypeError>) {
    // Stage 5.80: delegate to the new entry point with plan = None.
    // Backward-compatible: all existing callers see identical behavior.
    lower_hir_body_to_mir_full_with_dyn_trait_plan(body, interner, hir, return_ty, None)
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
) -> (MirBody, UnificationTable, Vec<crate::typeck::TypeError>) {
    let mut cx = MirLowerCtxt::new(interner, body.span);
    cx.hir = Some(hir);

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
    // Stage 15.90: We need to collect the first region vid from params
    // for elision rule 2 (output lifetime = input lifetime when there's
    // exactly one input lifetime).
    let mut param_region_vids_collected: Vec<crate::mir::ty::RegionVid> = Vec::new();
    // Stage 15.90: Store lowered param types so we don't lower them twice
    // (once for elision collection, once for local allocation). Reusing
    // ensures the region vids match.
    let mut lowered_param_types: Vec<Option<Ty>> = Vec::with_capacity(body.params.len());

    // Allocate LocalId(0) as the return value placeholder.
    // We lower the return type AFTER params so elision rule 2 can apply.
    let return_mir_ty = {
        // First, lower all param types to collect region vids.
        for param in &body.params {
            if let Some(t) = &param.ty {
                if param.self_kind.is_some() {
                    // Self params are resolved separately — skip for elision.
                    lowered_param_types.push(None);
                } else {
                    let mir_ty = lower_hir_ty_to_mir_ty_with_regions(t, &mut region_counter);
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
                let raw_return_ty = lower_hir_ty_to_mir_ty_with_regions(t, &mut region_counter);
                // Stage 15.90: Apply elision rule 2.
                apply_elision_rule_2(&raw_return_ty, &param_region_vids_collected)
            }
            None => cx.fresh_infer_ty(Span::DUMMY),
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
    if !cx.is_terminated() {
        // Assign the value to the return local.
        cx.push_assign(
            Place::local(return_local, Span::DUMMY),
            Rvalue::Use(Operand::Copy(Place::local(value_local, Span::DUMMY))),
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
    let unify = std::mem::take(&mut cx.unify);
    let type_errors = std::mem::take(&mut cx.type_errors);
    (cx.mir, unify, type_errors)
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
) -> (MirBody, UnificationTable, Vec<crate::typeck::TypeError>) {
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

/// Stage 15.90: Apply lifetime elision rule 2 to a return type.
///
/// Rule 2: If there's exactly one input lifetime (elided or explicit),
/// it's assigned to all elided output lifetimes.
///
/// This function replaces all `Region::Var(vid)` in the return type with
/// the single input lifetime's vid, if `input_vids` has exactly one entry.
/// If `input_vids` is empty or has multiple entries, the return type is
/// returned unchanged (each output lifetime keeps its own fresh vid, per
/// elision rule 1).
fn apply_elision_rule_2(return_ty: &Ty, input_vids: &[crate::mir::ty::RegionVid]) -> Ty {
    use crate::mir::ty::{Region, RegionVid, TyKind};
    // Rule 2 applies only when there's exactly one input lifetime.
    if input_vids.len() != 1 {
        return return_ty.clone();
    }
    let target_vid = input_vids[0];
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

/// Lower a HIR type to a MIR type.
pub(crate) fn lower_hir_ty_to_mir_ty(ty: &HirTy) -> Ty {
    // Stage 15.49: delegate to the region-aware variant with a throwaway
    // counter. The legacy callers that don't need region tracking get
    // `Region::Erased` for elided lifetimes (same as before).
    let mut region_counter = 0u32;
    lower_hir_ty_to_mir_ty_with_regions(ty, &mut region_counter)
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
    let span = Span::DUMMY;
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
                    .map(|t| lower_hir_ty_to_mir_ty_with_regions(t, region_counter))
                    .collect(),
            ),
            span,
        ),
        HirTyKind::Ref(region, mutability, inner) => {
            // Stage 15.49: Assign a fresh Region::Var for each reference.
            // - Explicit lifetimes: assign a fresh vid (we don't yet track
            //   the source-level lifetime name, but the vid is unique per
            //   reference, which is what the region inference needs).
            // - Elided lifetimes: assign a fresh vid (lifetime elision rule 1:
            //   each elided input lifetime gets its own fresh lifetime).
            let mir_region = match region {
                Some(_lt) => {
                    // Explicit lifetime — assign a fresh vid.
                    // TODO (future): track the lifetime name so we can unify
                    // references with the same explicit lifetime.
                    let vid = *region_counter;
                    *region_counter += 1;
                    Region::Var(RegionVid(vid))
                }
                None => {
                    // Elided lifetime — assign a fresh vid (elision rule 1).
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
                    Box::new(lower_hir_ty_to_mir_ty_with_regions(inner, region_counter)),
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
                    Box::new(lower_hir_ty_to_mir_ty_with_regions(inner, region_counter)),
                ),
                span,
            )
        }
        HirTyKind::Slice(inner) => Ty::new(
            TyKind::Slice(Box::new(lower_hir_ty_to_mir_ty_with_regions(
                inner,
                region_counter,
            ))),
            span,
        ),
        HirTyKind::Array(inner, count_expr) => {
            let len_const = const_eval_array_len(count_expr, span);
            Ty::new(
                TyKind::Array(
                    Box::new(lower_hir_ty_to_mir_ty_with_regions(inner, region_counter)),
                    Box::new(len_const),
                ),
                span,
            )
        }
        HirTyKind::Infer => {
            // For type `_` (inference placeholder), we use a special
            // sentinel TyVid(u32::MAX) that the type checker will
            // replace with a fresh variable. This avoids borrowing cx
            // in the free function lower_hir_ty_to_mir_ty.
            Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(u32::MAX))), span)
        }
        // Stage 3.30: resolve named types (struct/enum/etc.) via the path's
        // Res. Was: fell through to `Ty::Error`, which made `Point`-typed
        // params/locals lose their type info and codegen treat them as i32.
        // Stage 3.42: also handle PrimTy::Str → TyKind::Str (was: fell
        // through to Error, breaking `&str` type annotations).
        HirTyKind::Path(_, path) => match path.res {
            Res::Def(def_id, _) => Ty::new(
                TyKind::Adt(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
                span,
            ),
            Res::PrimTy(PrimTy::Str) => Ty::new(TyKind::Str, span),
            _ => Ty::new(TyKind::Error, span),
        },
        // Stage 14.57: Handle fn pointer type annotations (e.g., `fn(i32) -> i32`).
        // Previously fell through to Error, causing fn pointer params to be
        // treated as i32 — function references were passed as `0` instead of
        // the actual function pointer.
        HirTyKind::FnPtr {
            inputs,
            output,
            abi,
            is_unsafe,
        } => {
            let mir_inputs: Vec<Ty> = inputs
                .iter()
                .map(|t| lower_hir_ty_to_mir_ty_with_regions(t, region_counter))
                .collect();
            let mir_output = Box::new(lower_hir_ty_to_mir_ty_with_regions(output, region_counter));
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
        _ => Ty::new(TyKind::Error, span), // complex types → Error for now
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

    /// Stage 15.90: Verify `apply_elision_rule_2` with single input lifetime.
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
        let result = apply_elision_rule_2(&return_ty, &input_vids);
        // The output lifetime should be replaced with vid 3.
        match &result.kind {
            TyKind::Ref(region, _, _) => {
                assert_eq!(*region, Region::Var(RegionVid(3)));
            }
            _ => panic!("expected Ref"),
        }
    }

    /// Stage 15.90: Verify `apply_elision_rule_2` with multiple input lifetimes
    /// does NOT apply (keeps original output lifetime).
    #[test]
    fn apply_elision_rule_2_multiple_inputs() {
        // Return type: &i32 with Region::Var(10) (fresh output vid)
        let return_ty = Ty::new(
            TyKind::Ref(
                Region::Var(RegionVid(10)),
                Mutability::Immutable,
                Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
            ),
            Span::DUMMY,
        );
        // Input: multiple lifetime vids
        let input_vids = vec![RegionVid(1), RegionVid(2)];
        let result = apply_elision_rule_2(&return_ty, &input_vids);
        // The output lifetime should NOT be replaced (keeps vid 10).
        match &result.kind {
            TyKind::Ref(region, _, _) => {
                assert_eq!(*region, Region::Var(RegionVid(10)));
            }
            _ => panic!("expected Ref"),
        }
    }

    /// Stage 15.90: Verify `apply_elision_rule_2` with no input lifetimes
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
        let result = apply_elision_rule_2(&return_ty, &input_vids);
        match &result.kind {
            TyKind::Ref(region, _, _) => {
                assert_eq!(*region, Region::Var(RegionVid(10)));
            }
            _ => panic!("expected Ref"),
        }
    }
}
