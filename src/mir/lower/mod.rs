//! HIR → MIR lowering.
//!
//! Converts each HIR body (expression tree) into a MIR body (control
//! flow graph of basic blocks + statements + terminators).
//!
//! Public entry point: [`lower_hir_body_to_mir`].

use crate::ast;
use crate::hir::*;
use crate::mir::body::*;
use crate::mir::dyn_trait::{
    find_dyn_trait_method_call_in_plan_by_method, DynTraitMIRPlan, DynTraitMethodCall,
};
use crate::mir::place::*;
use crate::mir::ty::*;
use crate::session::Span;
use crate::typeck::unify::UnificationTable;
use lasso::Rodeo;

mod adt_layout;
mod closure_capture;
mod pattern_bindings;

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
        }
    }

    /// Allocate a fresh inference type variable and return it as a Ty.
    /// Each call produces a unique TyVid — no sharing.
    pub fn fresh_infer_ty(&mut self, span: Span) -> Ty {
        let vid = self.unify.new_ty_var();
        Ty::new(TyKind::Infer(InferVar::TyVar(vid)), span)
    }

    /// Allocate a fresh integer inference variable.
    pub fn fresh_int_ty(&mut self, span: Span) -> Ty {
        let vid = self.unify.new_int_var();
        Ty::new(TyKind::Infer(InferVar::IntVar(vid)), span)
    }

    /// Allocate a fresh float inference variable.
    pub fn fresh_float_ty(&mut self, span: Span) -> Ty {
        let vid = self.unify.new_float_var();
        Ty::new(TyKind::Infer(InferVar::FloatVar(vid)), span)
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

    /// Allocate a fresh basic block and return its ID.
    pub fn new_block(&mut self) -> BasicBlockId {
        self.mir.new_block()
    }

    /// Set the terminator of the current block and switch to a new block.
    pub fn terminate(&mut self, terminator: Terminator) {
        self.mir.block_mut(self.current_block).terminator = terminator;
    }

    /// Set the terminator of the current block and switch to `next`.
    pub fn terminate_and_goto(&mut self, terminator: Terminator, next: BasicBlockId) {
        self.mir.block_mut(self.current_block).terminator = terminator;
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
                    ty: Box::new(Ty::new(TyKind::Bool, Span::DUMMY)),
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
                        ty: Box::new(ty.clone()),
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
                        ty: Box::new(ty.clone()),
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
                        ty: Box::new(ty.clone()),
                        val: ConstVal::Float(*f),
                    },
                    ty,
                )
            }
            HirLitKind::Char(c) => (
                Const {
                    ty: Box::new(Ty::new(TyKind::Char, Span::DUMMY)),
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
                        ty: Box::new(ref_str_ty.clone()),
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
                        ty: Box::new(ref_slice_ty.clone()),
                        // Reuse Str variant — codegen will interpret
                        // the symbol as bytes when the type is Ref(_, _, Slice(u8)).
                        val: ConstVal::Str(*sym),
                    },
                    ref_slice_ty,
                )
            }
            HirLitKind::Byte(b) => (
                Const {
                    ty: Box::new(Ty::new(TyKind::Uint(ast::UintTy::U8), Span::DUMMY)),
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
pub fn lower_hir_body_to_mir_full(
    body: &Body,
    interner: &Rodeo,
    hir: &HirCrate,
    return_ty: Option<HirTy>,
) -> (MirBody, UnificationTable) {
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
/// HIR `receiver.method(args)` → MIR `Terminator::Call` with Const marker
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
) -> (MirBody, UnificationTable) {
    let mut cx = MirLowerCtxt::new(interner, body.span);
    cx.hir = Some(hir);

    // Stage 5.80: attach the dyn Trait plan if provided.
    // Per §16: plan was built upstream by the driver via
    // `build_dyn_trait_mir_plan_from_resolver()`. The lower does not
    // query TraitResolver directly.
    if let Some(plan) = plan {
        cx.set_dyn_trait_plan(plan.clone());
    }

    // Allocate LocalId(0) as the return value placeholder.
    // If a return type is provided (from the fn sig), use it directly
    // instead of a fresh inference variable. This is the key fix for
    // unifying fn signatures with body value types.
    let return_mir_ty = match &return_ty {
        Some(t) => lower_hir_ty_to_mir_ty(t),
        None => cx.fresh_infer_ty(Span::DUMMY),
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
    for param in &body.params {
        let ty = match &param.ty {
            Some(t) => lower_hir_ty_to_mir_ty(t),
            None => cx.fresh_infer_ty(Span::DUMMY),
        };
        let param_local = cx.new_local(param.pat.hir_id, ty, None);
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

    // Assign the value to the return local.
    cx.push_assign(
        Place::local(return_local, Span::DUMMY),
        Rvalue::Use(Operand::Copy(Place::local(value_local, Span::DUMMY))),
        body.span,
    );

    // Emit StorageDead for all locals (except the return local) before
    // the function returns. This is a conservative approximation —
    // ideally we'd emit StorageDead at each local's scope end, but that
    // requires scope tracking (Stage 3). For now, all locals die at
    // function return.
    //
    // We skip LocalId(0) (the return local) because it's still alive
    // at the point of Return.
    let local_count = cx.mir.local_decls.len();
    for i in 1..local_count {
        cx.mir
            .block_mut(cx.current_block)
            .statements
            .push(Statement {
                kind: StatementKind::StorageDead(LocalId(i as u32)),
                span: body.span,
            });
    }

    // Terminate the current block with Return.
    cx.terminate(Terminator::Return);

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

    // Extract the unify table before consuming cx.
    let unify = std::mem::take(&mut cx.unify);
    (cx.mir, unify)
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
pub fn lower_body_full(
    body: &Body,
    interner: &Rodeo,
    hir: &HirCrate,
    return_ty: Option<HirTy>,
) -> (MirBody, UnificationTable) {
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
            ty: Box::new(Ty::new(TyKind::Uint(ast::UintTy::Usize), span)),
            val: ConstVal::Uint(*n),
        },
        HirExprKind::Lit(HirLitKind::Uint(n, _)) => Const {
            ty: Box::new(Ty::new(TyKind::Uint(ast::UintTy::Usize), span)),
            val: ConstVal::Uint(*n),
        },
        // Non-literal: emit an Error-typed const so typeck flags it.
        _ => Const {
            ty: Box::new(Ty::new(TyKind::Error, span)),
            val: ConstVal::Uint(0),
        },
    }
}

/// Lower a HIR type to a MIR type.
pub(crate) fn lower_hir_ty_to_mir_ty(ty: &HirTy) -> Ty {
    let span = ty.span;
    match &ty.kind {
        HirTyKind::Bool => Ty::new(TyKind::Bool, span),
        HirTyKind::Char => Ty::new(TyKind::Char, span),
        HirTyKind::Int(int_ty) => Ty::new(TyKind::Int(*int_ty), span),
        HirTyKind::Uint(uint_ty) => Ty::new(TyKind::Uint(*uint_ty), span),
        HirTyKind::Float(float_ty) => Ty::new(TyKind::Float(*float_ty), span),
        HirTyKind::Never => Ty::new(TyKind::Never, span),
        HirTyKind::Tuple(tys) => Ty::new(
            TyKind::Tuple(tys.iter().map(lower_hir_ty_to_mir_ty).collect()),
            span,
        ),
        HirTyKind::Ref(region, mutability, inner) => {
            let mir_region = match region {
                Some(_lt) => Region::Var(RegionVid(0)), // placeholder
                None => Region::Erased,
            };
            let mir_mut = match mutability {
                ast::Mutability::Mutable => crate::mir::ty::Mutability::Mutable,
                ast::Mutability::Immutable => crate::mir::ty::Mutability::Immutable,
            };
            Ty::new(
                TyKind::Ref(mir_region, mir_mut, Box::new(lower_hir_ty_to_mir_ty(inner))),
                span,
            )
        }
        HirTyKind::Ptr(mutability, inner) => {
            let mir_mut = match mutability {
                ast::Mutability::Mutable => crate::mir::ty::Mutability::Mutable,
                ast::Mutability::Immutable => crate::mir::ty::Mutability::Immutable,
            };
            Ty::new(
                TyKind::RawPtr(mir_mut, Box::new(lower_hir_ty_to_mir_ty(inner))),
                span,
            )
        }
        HirTyKind::Slice(inner) => {
            Ty::new(TyKind::Slice(Box::new(lower_hir_ty_to_mir_ty(inner))), span)
        }
        HirTyKind::Array(inner, count_expr) => {
            let len_const = const_eval_array_len(count_expr, span);
            Ty::new(
                TyKind::Array(Box::new(lower_hir_ty_to_mir_ty(inner)), Box::new(len_const)),
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
            Res::Def(def_id, _) => Ty::new(TyKind::Adt(def_id, Vec::new()), span),
            Res::PrimTy(PrimTy::Str) => Ty::new(TyKind::Str, span),
            _ => Ty::new(TyKind::Error, span),
        },
        _ => Ty::new(TyKind::Error, span), // complex types → Error for now
    }
}

/// Lower a HIR expression to a MIR Place (a place that can be assigned to).
///
/// Stage 3.34 (L-MUT-1 fix): used by `HirExprKind::Assign` to lower the LHS
/// into a place. Handles:
///   - `Path` (local variable) → `Place::Local`
///   - `Field { receiver, ident }` → `Place::Projection(receiver, Field(idx, ty))`
///   - `Index { receiver, index }` → `Place::Projection(receiver, Index(idx_local))`
///   - `Unary { op: Deref, expr }` → `Place::Projection(expr, Deref)`
///
/// For other expression kinds (which can't be assigned to), falls back to
/// a fresh local — typeck should catch the "assignment to non-place" error.
fn lower_expr_to_place(cx: &mut MirLowerCtxt, expr: &HirExpr) -> Place {
    match &expr.kind {
        HirExprKind::Path(path) => {
            if let Res::Local(hir_id) = path.res {
                if let Some(local_id) = cx.local_of(hir_id) {
                    return Place::local(local_id, expr.span);
                }
            }
            // Fallback: fresh local (error recovery).
            let ty = cx.fresh_infer_ty(expr.span);
            let local = cx.mir.new_local(ty, None, expr.span);
            Place::local(local, expr.span)
        }
        HirExprKind::Field { receiver, ident } => {
            let base = lower_expr_to_place(cx, receiver);
            let field_index = resolve_field_index(cx, receiver, &ident.name);
            let field_ty = resolve_field_type(cx, receiver, field_index)
                .unwrap_or_else(|| cx.fresh_infer_ty(expr.span));
            Place {
                kind: PlaceKind::Projection(
                    Box::new(base),
                    ProjectionElem::Field(FieldId(field_index), field_ty),
                ),
                span: expr.span,
            }
        }
        HirExprKind::Index {
            receiver, index, ..
        } => {
            let base = lower_expr_to_place(cx, receiver);
            let idx_local = lower_expr_to_operand(cx, index);
            Place {
                kind: PlaceKind::Projection(Box::new(base), ProjectionElem::Index(idx_local)),
                span: expr.span,
            }
        }
        HirExprKind::Unary {
            op, expr: inner, ..
        } if *op == HirUnaryOp::Deref => {
            let base = lower_expr_to_place(cx, inner);
            Place {
                kind: PlaceKind::Projection(Box::new(base), ProjectionElem::Deref),
                span: expr.span,
            }
        }
        // Other expression kinds can't be assigned to — return a fresh
        // local as error recovery. typeck should catch this.
        _ => {
            let ty = cx.fresh_infer_ty(expr.span);
            let local = cx.mir.new_local(ty, None, expr.span);
            Place::local(local, expr.span)
        }
    }
}

/// Stage 5.78: Build a `Terminator::Call` for a dyn Trait method call,
/// and register the call info in `cx.mir.dyn_trait_calls` side-table.
///
/// The function operand is a `Const` whose `ConstVal::Int` value is the
/// **index** of the call entry in `cx.mir.dyn_trait_calls`. Codegen
/// (Stage 5.79+) will detect this marker and emit a vtable indirect call
/// using the recorded (trait, type, method, slot_index, param_count) info.
///
/// # Arguments
///
/// - `cx`: the lowering context (used to push the side-table entry)
/// - `call`: the `DynTraitMethodCall` carrying trait/type/method/slot/param info
/// - `recv_local`: the MIR local holding the receiver (the `self` arg)
/// - `arg_locals`: the MIR locals holding the explicit args (excluding `self`)
/// - `dest`: the destination local where the call result is stored
/// - `span`: source span for error reporting
///
/// # Returns
///
/// A `Terminator::Call` whose:
/// - `func` is `Operand::Constant(Const { ty: Error, val: Int(index) })`
///   where `index` is the side-table entry index
/// - `args` is `[Copy(recv), Copy(arg0), Copy(arg1), ...]` — self first
///   then explicit args (matches the existing MethodCall convention)
/// - `destination` is `Place::local(dest, span)`
/// - `target` is `None` — caller sets it via `terminate_and_goto`
///
/// # §16 compliance
///
/// MIR carries the dyn Trait call info as data (`dyn_trait_calls`
/// side-table), so codegen doesn't need to query HIR or TraitResolver.
/// Data flow: `mir::dyn_trait` (DynTraitMethodCall) → `mir::lower`
/// (this helper) → `mir::body` (side-table + Terminator) → codegen
/// (Stage 5.79+). Single-directional, no circular dependency.
///
/// # §23 compliance
///
/// `build_dyn_trait_call_terminator` follows the
/// `<verb>_<noun>_<noun>_<noun>_<noun>` pattern (helper-verb `build_`
/// prefix per §8.1, mirroring `build_dyn_trait_mir_plan` from Stage 5.73).
pub fn build_dyn_trait_call_terminator(
    cx: &mut MirLowerCtxt,
    call: &DynTraitMethodCall,
    recv_local: LocalId,
    arg_locals: &[LocalId],
    dest: LocalId,
    span: Span,
) -> Terminator {
    // Push the call info into the side-table; the index becomes the marker.
    let index = cx.mir.dyn_trait_calls.len() as u128;
    cx.mir.dyn_trait_calls.push(call.clone());

    // Build the args list: self first, then explicit args.
    let mut arg_operands: Vec<Operand> = vec![Operand::Copy(Place::local(recv_local, span))];
    for local in arg_locals {
        arg_operands.push(Operand::Copy(Place::local(*local, Span::DUMMY)));
    }

    Terminator::Call {
        // The Const's Int value is the side-table index. Codegen detects
        // this marker and emits a vtable indirect call instead of a direct
        // function call.
        func: Operand::Constant(Const {
            ty: Box::new(Ty::new(TyKind::Error, Span::DUMMY)),
            val: ConstVal::Int(index),
        }),
        args: arg_operands,
        destination: Place::local(dest, span),
        target: None,
    }
}

/// Lower a HIR expression to a MIR Operand (a value that can be used
/// as an argument to a binary op, call, etc.).
///
/// Returns the LocalId of the temporary that holds the result.
fn lower_expr_to_operand(cx: &mut MirLowerCtxt, expr: &HirExpr) -> LocalId {
    match &expr.kind {
        HirExprKind::Lit(lit) => {
            let (const_val, ty) = cx.lit_to_const(lit);
            cx.eval_rvalue_to_temp(Rvalue::Use(Operand::Constant(const_val)), ty, expr.span)
        }
        HirExprKind::Path(path) => {
            // If the path resolves to a local, return that local.
            if let Res::Local(hir_id) = path.res {
                if let Some(local_id) = cx.local_of(hir_id) {
                    return local_id;
                }
            }
            // If the path resolves to a top-level Def, dispatch on DefKind:
            //   - Fn         → FnDef type (real fn item)
            //   - Struct     → Adt type (struct ctor — handled in Call lower)
            //   - Enum       → Adt type (enum variant ctor — Stage 3.31+)
            //   - Const/Static → placeholder Error type (real const-eval is Stage 3+)
            // Stage 3.30 (per §15): use DefKind from Res to dispatch, eliminating
            // the root cause of "tuple struct ctor was being lowered as Call".
            if let Res::Def(def_id, def_kind) = path.res {
                match def_kind {
                    crate::resolve::DefKind::Struct | crate::resolve::DefKind::Enum => {
                        // ADT type — produce an Adt operand. When this path is
                        // used as the `func` of a Call expression, the Call
                        // lower will check the operand's type and dispatch to
                        // Aggregate(Adt) instead of emitting a real Call.
                        //
                        // Stage 3.38 (L-ENUM): For enum variant paths like
                        // `Color::Red`, the path has 2 segments. The second
                        // segment is the variant name. We look it up in the
                        // HIR enum definition to get the variant index.
                        // For unit variants (no args), we construct the
                        // Aggregate directly here.
                        if def_kind == crate::resolve::DefKind::Enum && path.segments.len() >= 2 {
                            let variant_name = &path.segments[1].ident.name;
                            if let Some((variant_idx, field_tys)) =
                                resolve_enum_variant(cx, def_id, variant_name)
                            {
                                // Check if this is a unit variant (field_tys
                                // has only the discriminant).
                                if field_tys.len() == 1 {
                                    // Unit variant — construct directly with
                                    // discriminant operand.
                                    let adt_ty =
                                        Ty::new(TyKind::Adt(def_id, Vec::new()), expr.span);
                                    let discr = Operand::Constant(Const {
                                        ty: Box::new(Ty::new(
                                            TyKind::Int(crate::ast::IntTy::I32),
                                            Span::DUMMY,
                                        )),
                                        val: ConstVal::Uint(variant_idx as u128),
                                    });
                                    return cx.eval_rvalue_to_temp(
                                        Rvalue::Aggregate(
                                            AggregateKind::Adt(
                                                def_id,
                                                variant_idx,
                                                Vec::new(),
                                                field_tys,
                                            ),
                                            vec![discr],
                                        ),
                                        adt_ty,
                                        expr.span,
                                    );
                                }
                                // Non-unit variant — the path is the ctor,
                                // which will be used in a Call expression.
                                // Fall through to create the Adt-typed operand.
                            }
                        }
                        let adt_ty = Ty::new(TyKind::Adt(def_id, Vec::new()), expr.span);
                        return cx.eval_rvalue_to_temp(
                            Rvalue::Use(Operand::Constant(Const {
                                ty: Box::new(adt_ty.clone()),
                                val: ConstVal::Uint(def_id.as_u32() as u128),
                            })),
                            adt_ty,
                            expr.span,
                        );
                    }
                    _ => {
                        // Stage 3.44: Handle Const and Static references.
                        // Per §15: root-cause fix — dispatch on DefKind
                        // instead of treating everything as FnDef.
                        match def_kind {
                            crate::resolve::DefKind::Const | crate::resolve::DefKind::Static => {
                                // Look up the const/static's value from HIR.
                                // For Stage 3.44, we evaluate the initializer
                                // expression and produce a constant operand.
                                if let Some(hir_crate) = cx.hir {
                                    if let Some(crate::hir::OwnerNode::Item(item)) =
                                        hir_crate.owner(def_id)
                                    {
                                        match item {
                                            crate::hir::HirItem::Const(c) => {
                                                // Lower the const's body expression to get its value.
                                                if let Some(body) = hir_crate.body(c.body) {
                                                    let const_local =
                                                        lower_expr_to_operand(cx, &body.value);
                                                    let ld = cx
                                                        .mir
                                                        .local_decls
                                                        .get(const_local.0 as usize);
                                                    if let Some(ld) = ld {
                                                        return cx.eval_rvalue_to_temp(
                                                            Rvalue::Use(Operand::Copy(
                                                                Place::local(
                                                                    const_local,
                                                                    expr.span,
                                                                ),
                                                            )),
                                                            ld.ty.clone(),
                                                            expr.span,
                                                        );
                                                    }
                                                }
                                            }
                                            crate::hir::HirItem::Static(s) => {
                                                // Statics are like consts but with a fixed memory location.
                                                // For Stage 3.44, treat same as const.
                                                if let Some(body) = hir_crate.body(s.body) {
                                                    let static_local =
                                                        lower_expr_to_operand(cx, &body.value);
                                                    let ld = cx
                                                        .mir
                                                        .local_decls
                                                        .get(static_local.0 as usize);
                                                    if let Some(ld) = ld {
                                                        return cx.eval_rvalue_to_temp(
                                                            Rvalue::Use(Operand::Copy(
                                                                Place::local(
                                                                    static_local,
                                                                    expr.span,
                                                                ),
                                                            )),
                                                            ld.ty.clone(),
                                                            expr.span,
                                                        );
                                                    }
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                // Fallback: treat as FnDef (error recovery).
                                let fndef_ty =
                                    Ty::new(TyKind::FnDef(def_id, Vec::new()), expr.span);
                                return cx.eval_rvalue_to_temp(
                                    Rvalue::Use(Operand::Constant(Const {
                                        ty: Box::new(fndef_ty.clone()),
                                        val: ConstVal::Uint(def_id.as_u32() as u128),
                                    })),
                                    fndef_ty,
                                    expr.span,
                                );
                            }
                            _ => {
                                // Default: treat as FnDef (covers Fn, etc.).
                                let fndef_ty =
                                    Ty::new(TyKind::FnDef(def_id, Vec::new()), expr.span);
                                return cx.eval_rvalue_to_temp(
                                    Rvalue::Use(Operand::Constant(Const {
                                        ty: Box::new(fndef_ty.clone()),
                                        val: ConstVal::Uint(def_id.as_u32() as u128),
                                    })),
                                    fndef_ty,
                                    expr.span,
                                );
                            }
                        }
                    }
                }
            }
            // Otherwise, create an error placeholder.
            cx.eval_rvalue_to_temp(
                Rvalue::Use(Operand::Constant(Const {
                    ty: Box::new(Ty::new(TyKind::Error, Span::DUMMY)),
                    val: ConstVal::Int(0),
                })),
                Ty::new(TyKind::Error, Span::DUMMY),
                expr.span,
            )
        }
        HirExprKind::Binary { op, lhs, rhs, .. } => {
            // Short-circuit And/Or must be lowered to control flow,
            // not to BitAnd/BitOr (which would evaluate both sides).
            if *op == HirBinOp::And || *op == HirBinOp::Or {
                return lower_short_circuit(cx, *op, lhs, rhs, expr.span);
            }
            let lhs_local = lower_expr_to_operand(cx, lhs);
            let rhs_local = lower_expr_to_operand(cx, rhs);
            let mir_op = MirLowerCtxt::lower_bin_op(*op);
            let binop_ty = cx.fresh_infer_ty(expr.span);
            let lhs_operand = Operand::Copy(Place::local(lhs_local, lhs.span));
            let rhs_operand = Operand::Copy(Place::local(rhs_local, rhs.span));
            let result = cx.eval_rvalue_to_temp(
                Rvalue::BinaryOp(mir_op, lhs_operand.clone(), rhs_operand.clone()),
                binop_ty,
                expr.span,
            );
            // Stage 3.24 + 3.25: emit runtime checks for overflowable ops.
            //   - Div/Rem: emit DivisionByZero(rhs) check (divisor == 0)
            //   - Add/Sub/Mul/Shl/Shr: emit Overflow(op, lhs, rhs) check
            // Codegen turns these into real LLVM intrinsics / icmp branches.
            if is_overflowable_op(*op) {
                match *op {
                    HirBinOp::Div | HirBinOp::Rem => {
                        emit_div_by_zero_assert(cx, result, rhs_operand.clone(), expr.span);
                    }
                    _ => {
                        emit_overflow_assert(
                            cx,
                            result,
                            mir_op,
                            lhs_operand,
                            rhs_operand,
                            expr.span,
                        );
                    }
                }
            }
            result
        }
        HirExprKind::Unary {
            op, expr: inner, ..
        } => {
            // Deref is a projection, not a real unary op.
            if *op == HirUnaryOp::Deref {
                return lower_deref_expr(cx, inner, expr.span);
            }
            let inner_local = lower_expr_to_operand(cx, inner);
            let mir_op = MirLowerCtxt::lower_un_op(*op);
            let unary_ty = cx.fresh_infer_ty(expr.span);
            cx.eval_rvalue_to_temp(
                Rvalue::UnaryOp(mir_op, Operand::Copy(Place::local(inner_local, inner.span))),
                unary_ty,
                expr.span,
            )
        }
        HirExprKind::Block(block) => lower_block(cx, block),
        HirExprKind::Call { func, args, .. } => {
            // Lower func first — this determines whether the call is a real
            // function call or an ADT construction (struct/enum ctor).
            let func_local = lower_expr_to_operand(cx, func);
            let arg_locals: Vec<LocalId> =
                args.iter().map(|a| lower_expr_to_operand(cx, a)).collect();
            let arg_operands: Vec<Operand> = arg_locals
                .iter()
                .map(|l| Operand::Copy(Place::local(*l, Span::DUMMY)))
                .collect();

            // Stage 3.30 (per §15): inspect the func operand's type to decide
            //   - TyKind::Adt(def_id, _)  → Aggregate(Adt(def_id, ...)) —
            //     this is a struct/enum ctor call like `Pair(1, 2)`.
            //   - TyKind::FnDef(..)       → real Terminator::Call.
            // This dispatch eliminates the root cause of "tuple struct ctor
            // was being lowered as Call" — the type info flows naturally
            // from Path resolution through to Call lowering.
            let is_adt_ctor = {
                let func_local_decl = cx.mir.local_decls.get(func_local.0 as usize);
                func_local_decl
                    .map(|ld| matches!(&ld.ty.kind, TyKind::Adt(_, _)))
                    .unwrap_or(false)
            };

            if is_adt_ctor {
                // Struct/enum ctor: lower as Aggregate(Adt, operands).
                let func_local_decl = cx
                    .mir
                    .local_decls
                    .get(func_local.0 as usize)
                    .expect("func local must exist");
                let (adt_def_id, adt_substs) = match &func_local_decl.ty.kind {
                    TyKind::Adt(def_id, substs) => (*def_id, substs.clone()),
                    _ => unreachable!("checked is_adt_ctor above"),
                };
                // Stage 3.38 (L-ENUM): For enum variant ctors (e.g.,
                // `Opt::Some(42)`), resolve the variant index and field
                // types from the HIR enum definition. The func expression
                // is a Path like `Opt::Some` — check its HIR to find the
                // variant name.
                let (variant_idx, field_tys) = if let HirExprKind::Path(path) = &func.kind {
                    if path.segments.len() >= 2 {
                        if let Some((idx, tys)) =
                            resolve_enum_variant(cx, adt_def_id, &path.segments[1].ident.name)
                        {
                            (idx, tys)
                        } else {
                            (0, resolve_adt_field_tys(cx, adt_def_id))
                        }
                    } else {
                        (0, resolve_adt_field_tys(cx, adt_def_id))
                    }
                } else {
                    (0, resolve_adt_field_tys(cx, adt_def_id))
                };
                // For enum variants, the Aggregate operands need to include
                // the discriminant as the first element. For structs,
                // variant_idx = 0 and field_tys are the struct's fields.
                let mut all_operands = Vec::new();
                if variant_idx > 0
                    || (cx.hir.and_then(|h| h.owner(adt_def_id)).is_some_and(|o| {
                        matches!(o, crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(_)))
                    }))
                {
                    // Enum variant — prepend discriminant.
                    let discr = Operand::Constant(Const {
                        ty: Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
                        val: ConstVal::Uint(variant_idx as u128),
                    });
                    all_operands.push(discr);
                }
                all_operands.extend(arg_operands);
                let dest_ty = Ty::new(TyKind::Adt(adt_def_id, adt_substs), expr.span);
                let dest = cx.mir.new_local(dest_ty, None, expr.span);
                cx.push_assign(
                    Place::local(dest, expr.span),
                    Rvalue::Aggregate(
                        AggregateKind::Adt(adt_def_id, variant_idx, Vec::new(), field_tys),
                        all_operands,
                    ),
                    expr.span,
                );
                dest
            } else {
                // Stage 4.9: Check if func is a closure type.
                // Closures are not FnDef — they're values of type TyKind::Closure.
                // Calling a closure requires extracting the captured environment
                // and invoking the closure body. For now (simplified), we detect
                // closure calls and produce a placeholder result (unit type),
                // avoiding the incorrect Terminator::Call that would treat the
                // closure struct as a function pointer.
                let is_closure = {
                    let func_local_decl = cx.mir.local_decls.get(func_local.0 as usize);
                    func_local_decl
                        .map(|ld| matches!(&ld.ty.kind, TyKind::Closure(_, _)))
                        .unwrap_or(false)
                };

                if is_closure {
                    // Stage 4.13: Full closure call lowering — inline approach.
                    //
                    // When calling a closure, we:
                    // 1. Extract captured fields from the closure struct local
                    //    (each field is a Projection::Field on the closure local)
                    // 2. Bind captured values to fresh locals (so the inlined
                    //    body can reference them via the original local_map)
                    // 3. Bind call arguments to the closure's parameter locals
                    // 4. Lower the closure body inline at the call site
                    //
                    // This is the "inline" approach — no separate function is
                    // generated. The closure body is lowered directly at each
                    // call site. This is simple and correct, at the cost of
                    // code duplication (which LLVM's optimizer can handle).
                    //
                    // However, we don't have access to the HIR closure definition
                    // from here (we only have the func operand's type). So we
                    // use a pragmatic approach: extract captures from the closure
                    // struct, produce a fresh infer type for the result, and
                    // lower the call arguments. The actual body inlining requires
                    // HIR access which would need restructuring the lowering
                    // pipeline (deferred to Stage 5).
                    //
                    // For now (Stage 4.13): extract captures + produce result
                    // local with inferred type. This is more useful than the
                    // Stage 4.9 unit placeholder.

                    // Get the closure type's capture field types
                    let closure_ty = &cx.mir.local(func_local).ty;
                    let capture_tys: Vec<Ty> = match &closure_ty.kind {
                        TyKind::Closure(_, substs) => substs.clone(),
                        _ => vec![],
                    };

                    // Extract each captured field from the closure struct
                    for cap_ty in &capture_tys {
                        let field_ty = cap_ty.clone();
                        let _extracted_local = cx.mir.new_local(field_ty, None, expr.span);
                        // In a full implementation, we'd assign:
                        // extracted_local = Copy(Projection(closure_local, Field(i, cap_ty)))
                        // But since we can't map back to the original HirId here,
                        // we skip the binding. The inlined body would need
                        // these locals registered in local_map.
                    }

                    // Produce a result local with inferred type
                    let dest_ty = cx.fresh_infer_ty(expr.span);
                    cx.mir.new_local(dest_ty, None, expr.span)
                } else {
                    // Real function call.
                    let dest_ty = cx.fresh_infer_ty(Span::DUMMY);
                    let dest = cx.mir.new_local(dest_ty, None, expr.span);
                    let cont = cx.new_block();
                    cx.terminate_and_goto(
                        Terminator::Call {
                            func: Operand::Copy(Place::local(func_local, func.span)),
                            args: arg_operands,
                            destination: Place::local(dest, expr.span),
                            target: Some(cont),
                        },
                        cont,
                    );
                    dest
                }
            }
        }
        HirExprKind::If {
            cond, then, else_, ..
        } => lower_if(cx, cond, then, else_.as_deref(), expr.span),
        HirExprKind::Match {
            expr: scrutinee,
            arms,
            ..
        } => lower_match(cx, scrutinee, arms, expr.span),
        HirExprKind::Return { expr: ret_expr, .. } => {
            if let Some(ret) = ret_expr {
                let ret_local = lower_expr_to_operand(cx, ret);
                cx.push_assign(
                    Place::local(LocalId(0), Span::DUMMY),
                    Rvalue::Use(Operand::Copy(Place::local(ret_local, ret.span))),
                    expr.span,
                );
            }
            cx.terminate(Terminator::Return);
            // Return a dummy local (unreachable after Return)
            cx.mir
                .new_local(Ty::new(TyKind::Never, Span::DUMMY), None, Span::DUMMY)
        }
        HirExprKind::Assign { lhs, rhs, .. } => {
            let rhs_local = lower_expr_to_operand(cx, rhs);
            // Stage 3.34 (L-MUT-1 fix): handle assignment LHS that are
            // projections (field access, index, deref). Was: only handled
            // `Path` LHS — `a.v = 42` fell through to "just evaluate rhs"
            // and silently dropped the mutation.
            //
            // Per §15: root-cause fix (handle all LHS shapes in the Assign
            // lower), not a hack (e.g., special-casing field mutation in
            // codegen).
            let lhs_place = lower_expr_to_place(cx, lhs);
            cx.push_assign(
                lhs_place,
                Rvalue::Use(Operand::Copy(Place::local(rhs_local, rhs.span))),
                expr.span,
            );
            rhs_local
        }
        HirExprKind::Tuple { elems, .. } => {
            let elem_locals: Vec<LocalId> =
                elems.iter().map(|e| lower_expr_to_operand(cx, e)).collect();
            let operands: Vec<Operand> = elem_locals
                .iter()
                .map(|l| Operand::Copy(Place::local(*l, Span::DUMMY)))
                .collect();
            let tuple_ty = cx.fresh_infer_ty(expr.span);
            cx.eval_rvalue_to_temp(
                Rvalue::Aggregate(AggregateKind::Tuple, operands),
                tuple_ty,
                expr.span,
            )
        }
        HirExprKind::Unit => cx.eval_rvalue_to_temp(
            Rvalue::Aggregate(AggregateKind::Tuple, vec![]),
            Ty::new(TyKind::Tuple(vec![]), expr.span),
            expr.span,
        ),
        // === Stage 2.4b: Previously-missing expression kinds ===

        // Field access: `expr.field` → lower base, create projection
        // Stage 3.30 fix: resolve field index from the field name.
        //   - For tuple struct fields (`p.0`, `p.1`), the ident is the
        //     stringified index — parse it directly.
        //   - For named struct fields (`p.x`), look up the field index in
        //     the HIR struct definition by matching the field name.
        // Was: hardcoded `FieldId(0)` — meant `p.1`, `p.x`, etc. all
        // returned field 0.
        // Stage 3.32 fix (L-DEBT-2): resolve the field's actual type from
        // the struct definition and put it in ProjectionElem::Field (was:
        // fresh_infer_ty — typeck never resolved it, so codegen loaded
        // i32 even for i64 fields).
        HirExprKind::Field { receiver, ident } => {
            let base_local = lower_expr_to_operand(cx, receiver);
            // Resolve the field index from the ident name.
            let field_index = resolve_field_index(cx, receiver, &ident.name);
            // Stage 3.32: resolve the field's actual type from the struct def.
            let field_ty = resolve_field_type(cx, receiver, field_index)
                .unwrap_or_else(|| cx.fresh_infer_ty(expr.span));
            let field_ty_for_proj = field_ty.clone();
            let result = cx.mir.new_local(field_ty, None, expr.span);
            cx.push_assign(
                Place::local(result, expr.span),
                Rvalue::Use(Operand::Copy(Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(base_local, receiver.span)),
                        ProjectionElem::Field(FieldId(field_index), field_ty_for_proj),
                    ),
                    span: expr.span,
                })),
                expr.span,
            );
            result
        }

        // Index: `arr[idx]` → lower base + index, create projection
        HirExprKind::Index {
            receiver, index, ..
        } => {
            let base_local = lower_expr_to_operand(cx, receiver);
            let index_local = lower_expr_to_operand(cx, index);
            // Stage 3.52: compute the element type from the receiver's type,
            // instead of using a fresh infer var (which typeck defaults to
            // i32). For `&[T]` (fat pointer), elem_ty = T. For `[T; N]`,
            // elem_ty = T. Falls back to fresh infer var if the receiver's
            // type can't be resolved (preserves old behavior for test
            // contexts).
            let elem_ty = resolve_index_element_type(cx, base_local)
                .unwrap_or_else(|| cx.fresh_infer_ty(expr.span));
            let result = cx.mir.new_local(elem_ty, None, expr.span);
            cx.push_assign(
                Place::local(result, expr.span),
                Rvalue::Use(Operand::Copy(Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(base_local, receiver.span)),
                        ProjectionElem::Index(index_local),
                    ),
                    span: expr.span,
                })),
                expr.span,
            );
            result
        }

        // Address-of: `&expr` / `&mut expr` → Rvalue::Ref
        HirExprKind::AddrOf {
            mutability,
            expr: inner,
            ..
        } => {
            let inner_local = lower_expr_to_operand(cx, inner);
            let bk = match mutability {
                crate::ast::Mutability::Mutable => crate::mir::place::BorrowKind::Mut,
                crate::ast::Mutability::Immutable => crate::mir::place::BorrowKind::Shared,
            };
            let ref_ty = cx.fresh_infer_ty(expr.span);
            cx.eval_rvalue_to_temp(
                Rvalue::Ref(Region::Erased, bk, Place::local(inner_local, inner.span)),
                ref_ty,
                expr.span,
            )
        }

        // Cast: `expr as Ty` → Rvalue::Cast
        HirExprKind::Cast {
            expr: inner, ty, ..
        } => {
            let inner_local = lower_expr_to_operand(cx, inner);
            let target_ty = lower_hir_ty_to_mir_ty(ty);
            cx.eval_rvalue_to_temp(
                Rvalue::Cast(
                    CastKind::Numeric,
                    Operand::Copy(Place::local(inner_local, inner.span)),
                    target_ty.clone(),
                ),
                target_ty,
                expr.span,
            )
        }

        // Try: `expr?` → just lower inner (error propagation is Stage 3+)
        HirExprKind::Try { expr: inner, .. } => lower_expr_to_operand(cx, inner),

        // Loop: `loop { body }` → basic block loop
        HirExprKind::Loop { body, .. } => {
            let loop_header = cx.new_block();
            let loop_body_start = cx.new_block();
            let loop_exit = cx.new_block();
            let result_ty = cx.fresh_infer_ty(expr.span);
            let result = cx.mir.new_local(result_ty, None, expr.span);

            // Entry → goto loop_header
            cx.terminate(Terminator::Goto(loop_header));

            // loop_header → goto loop_body_start (placeholder for future
            // condition checking / break targeting)
            cx.current_block = loop_header;
            cx.terminate(Terminator::Goto(loop_body_start));

            // loop_body_start → lower body → goto loop_header
            cx.current_block = loop_body_start;
            let _body_result = lower_block(cx, body);
            cx.terminate(Terminator::Goto(loop_header));

            // loop_exit (reached via Break) → continuation
            cx.current_block = loop_exit;
            result
        }

        // While: `while cond { body }` → loop with SwitchInt
        HirExprKind::While { cond, body, .. } => {
            let cond_block = cx.new_block();
            let body_block = cx.new_block();
            let exit_block = cx.new_block();

            // Entry → goto cond_block
            cx.terminate(Terminator::Goto(cond_block));

            // cond_block: evaluate cond, switchInt
            cx.current_block = cond_block;
            let cond_local = lower_expr_to_operand(cx, cond);
            cx.terminate(Terminator::SwitchInt {
                discr: Operand::Copy(Place::local(cond_local, cond.span)),
                targets: vec![(ConstVal::Bool(true), body_block)],
                otherwise: exit_block,
            });

            // body_block: lower body, goto cond_block
            cx.current_block = body_block;
            lower_block(cx, body);
            cx.terminate(Terminator::Goto(cond_block));

            // exit_block: continuation
            cx.current_block = exit_block;
            cx.mir
                .new_local(Ty::new(TyKind::Tuple(vec![]), expr.span), None, expr.span)
        }

        // For: `for pat in iter { body }` → lower iter, loop with next()
        HirExprKind::For {
            pat: _, iter, body, ..
        } => {
            let iter_local = lower_expr_to_operand(cx, iter);
            let cond_block = cx.new_block();
            let body_block = cx.new_block();
            let exit_block = cx.new_block();

            // Entry → goto cond_block
            cx.terminate(Terminator::Goto(cond_block));

            // cond_block: placeholder — real impl would call iter.next()
            // For Stage 2.4b, we just check if iter is truthy
            cx.current_block = cond_block;
            cx.terminate(Terminator::SwitchInt {
                discr: Operand::Copy(Place::local(iter_local, iter.span)),
                targets: vec![(ConstVal::Bool(true), body_block)],
                otherwise: exit_block,
            });

            // body_block: lower body, goto cond_block
            cx.current_block = body_block;
            lower_block(cx, body);
            cx.terminate(Terminator::Goto(cond_block));

            // exit_block: continuation
            cx.current_block = exit_block;
            cx.mir
                .new_local(Ty::new(TyKind::Tuple(vec![]), expr.span), None, expr.span)
        }

        // Closure: `|args| body` → lower body + create closure value with captures.
        // Stage 4.4 (L3 closure codegen): creates a proper closure value.
        // Stage 4.7 (L3 capture analysis): now detects and captures external variables.
        //
        // Current implementation (Stage 4.7):
        // - Registers closure params as locals (unchanged)
        // - Collects captured locals (external variables referenced in body)
        // - Lowers the closure body
        // - Creates a closure value via `AggregateKind::Closure` with captured operands
        // - The closure type carries capture field types in substs
        //
        // Capture analysis:
        // - Walks the closure body to find `HirExprKind::Path` with `Res::Local(hir_id)`
        // - Filters out closure params (those hir_ids just registered)
        // - Remaining locals are "captured" — their values become closure env fields
        //
        // Limitations (deferred to Stage 4.8+):
        // - Closure call lowering: closure calls still go through regular Call
        // - Capture mode (move vs borrow): currently always Copy
        // - Nested closures: not yet handled
        HirExprKind::Closure { params, body, .. } => {
            // Register closure params as locals + collect their hir_ids
            let mut param_hir_ids: std::collections::HashSet<HirId> =
                std::collections::HashSet::new();
            for param in params {
                let ty = cx.fresh_infer_ty(param.pat.span);
                cx.new_local(param.pat.hir_id, ty, None);
                // Collect all hir_ids from the pattern (ident, tuple, etc.)
                pattern_bindings::collect_pat_hir_ids(&param.pat, &mut param_hir_ids);
            }

            // Stage 4.7: Collect captured locals — external variables referenced in body
            let mut captured: Vec<(HirId, LocalId)> = Vec::new();
            let mut seen: std::collections::HashSet<HirId> = std::collections::HashSet::new();
            closure_capture::collect_captured_locals(
                cx,
                body,
                &param_hir_ids,
                &mut captured,
                &mut seen,
            );

            // Lower closure body
            let _body_local = lower_expr_to_operand(cx, body);

            // Stage 4.7: Build capture field types + operands
            let mut capture_tys: Vec<Ty> = Vec::new();
            let mut capture_operands: Vec<Operand> = Vec::new();
            for (_hir_id, local_id) in &captured {
                let ty = cx.mir.local(*local_id).ty.clone();
                capture_tys.push(ty);
                capture_operands.push(Operand::Copy(Place::local(*local_id, expr.span)));
            }

            // Create closure value with captures
            let closure_def_id = cx
                .hir
                .map(|h| h.owners.first().map(|(id, _)| *id).unwrap_or_default())
                .unwrap_or_default();
            let closure_ty = Ty::new(TyKind::Closure(closure_def_id, capture_tys), expr.span);
            let closure_local = cx.mir.new_local(closure_ty, None, expr.span);
            // Assign the closure value with captured operands
            cx.mir
                .block_mut(cx.current_block)
                .statements
                .push(Statement {
                    kind: StatementKind::Assign(Box::new((
                        Place::local(closure_local, expr.span),
                        Rvalue::Aggregate(
                            AggregateKind::Closure(closure_def_id, vec![]),
                            capture_operands,
                        ),
                    ))),
                    span: expr.span,
                });
            closure_local
        }

        // Break: `break expr` → goto loop exit (simplified: just lower expr)
        HirExprKind::Break { expr: br_expr, .. } => {
            if let Some(e) = br_expr {
                let _ = lower_expr_to_operand(cx, e);
            }
            // For Stage 2.4b, Break is simplified — no loop exit targeting.
            // Full implementation requires tracking loop exit blocks.
            cx.mir
                .new_local(Ty::new(TyKind::Never, Span::DUMMY), None, Span::DUMMY)
        }

        // Continue: `continue` → goto loop header (simplified)
        HirExprKind::Continue => {
            cx.mir
                .new_local(Ty::new(TyKind::Never, Span::DUMMY), None, Span::DUMMY)
        }

        // Range: `start..end` → Aggregate (simplified)
        HirExprKind::Range { start, end, .. } => {
            let start_local = start.as_ref().map(|s| lower_expr_to_operand(cx, s));
            let end_local = end.as_ref().map(|e| lower_expr_to_operand(cx, e));
            let range_ty = cx.fresh_infer_ty(expr.span);
            // For Stage 2.4b, ranges are represented as a tuple (start, end)
            let mut operands = Vec::new();
            if let Some(s) = start_local {
                operands.push(Operand::Copy(Place::local(s, Span::DUMMY)));
            }
            if let Some(e) = end_local {
                operands.push(Operand::Copy(Place::local(e, Span::DUMMY)));
            }
            cx.eval_rvalue_to_temp(
                Rvalue::Aggregate(AggregateKind::Tuple, operands),
                range_ty,
                expr.span,
            )
        }

        // Array: `[a, b, c]` → Aggregate(Array, operands)
        HirExprKind::Array { elems, .. } => {
            let elem_locals: Vec<LocalId> =
                elems.iter().map(|e| lower_expr_to_operand(cx, e)).collect();
            let operands: Vec<Operand> = elem_locals
                .iter()
                .map(|l| Operand::Copy(Place::local(*l, Span::DUMMY)))
                .collect();
            let elem_ty = cx.fresh_infer_ty(expr.span);
            let elem_ty_for_agg = elem_ty.clone();
            let array_ty = Ty::new(
                TyKind::Array(
                    Box::new(elem_ty),
                    Box::new(Const {
                        ty: Box::new(Ty::new(TyKind::Uint(ast::UintTy::Usize), Span::DUMMY)),
                        val: ConstVal::Uint(elems.len() as u128),
                    }),
                ),
                expr.span,
            );
            cx.eval_rvalue_to_temp(
                Rvalue::Aggregate(AggregateKind::Array(elem_ty_for_agg), operands),
                array_ty,
                expr.span,
            )
        }

        // Repeat: `[val; N]` → Aggregate(Array, [val, val, ...])
        HirExprKind::Repeat { elem, count, .. } => {
            let elem_local = lower_expr_to_operand(cx, elem);
            // For Stage 2.4b, we lower repeat as a 1-element array
            // (real repeat with N requires const-eval, Stage 3+)
            let _ = count;
            let elem_ty = cx.fresh_infer_ty(expr.span);
            cx.eval_rvalue_to_temp(
                Rvalue::Aggregate(
                    AggregateKind::Array(elem_ty),
                    vec![Operand::Copy(Place::local(elem_local, elem.span))],
                ),
                Ty::new(TyKind::Error, expr.span), // simplified
                expr.span,
            )
        }

        // Struct literal: `Foo { x: 1, y: 2 }` → Aggregate(Adt, operands)
        HirExprKind::Struct { path, fields, .. } => {
            // Lower each field value
            let field_locals: Vec<LocalId> = fields
                .iter()
                .filter_map(|f| f.expr.as_ref().map(|e| lower_expr_to_operand(cx, e)))
                .collect();
            let operands: Vec<Operand> = field_locals
                .iter()
                .map(|l| Operand::Copy(Place::local(*l, Span::DUMMY)))
                .collect();
            // Stage 3.30 (per §15): if the path resolves to a known struct
            // DefId, use AggregateKind::Adt (the proper representation).
            // Stage 3.38 (L-ENUM): also handle enum struct variants
            // (e.g., `Shape::Circle { r: 1.0 }`).
            if let Res::Def(def_id, DefKind::Struct) = path.res {
                let field_tys = resolve_adt_field_tys(cx, def_id);
                let struct_ty = Ty::new(TyKind::Adt(def_id, Vec::new()), expr.span);
                return cx.eval_rvalue_to_temp(
                    Rvalue::Aggregate(
                        AggregateKind::Adt(def_id, 0, Vec::new(), field_tys),
                        operands,
                    ),
                    struct_ty,
                    expr.span,
                );
            }
            // Stage 3.38 (L-ENUM): Enum struct variant (e.g., `Shape::Circle { r: 1.0 }`).
            if let Res::Def(def_id, DefKind::Enum) = path.res {
                if path.segments.len() >= 2 {
                    let variant_name = &path.segments[1].ident.name;
                    if let Some((variant_idx, field_tys)) =
                        resolve_enum_variant(cx, def_id, variant_name)
                    {
                        // Prepend discriminant to the operands.
                        let discr = Operand::Constant(Const {
                            ty: Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
                            val: ConstVal::Uint(variant_idx as u128),
                        });
                        let mut all_operands = vec![discr];
                        all_operands.extend(operands);
                        let enum_ty = Ty::new(TyKind::Adt(def_id, Vec::new()), expr.span);
                        return cx.eval_rvalue_to_temp(
                            Rvalue::Aggregate(
                                AggregateKind::Adt(def_id, variant_idx, Vec::new(), field_tys),
                                all_operands,
                            ),
                            enum_ty,
                            expr.span,
                        );
                    }
                }
            }
            // Fallback (path didn't resolve to a struct — error recovery).
            let struct_ty = cx.fresh_infer_ty(expr.span);
            let _ = path;
            cx.eval_rvalue_to_temp(
                Rvalue::Aggregate(AggregateKind::Tuple, operands),
                struct_ty,
                expr.span,
            )
        }

        // Stage 4.10: MacroCall — expand known built-in macros.
        // Previously (Stage 3.x): all macro calls produced TyKind::Error placeholder.
        // Now: known macros (println!, stringify!, assert!) produce proper MIR.
        // Unknown macros still fall back to Error placeholder.
        HirExprKind::MacroCall { path, .. } => {
            // Get the macro name from the last path segment.
            let macro_name = path.segments.last().map(|s| s.ident.name);
            if let Some(name_spur) = macro_name {
                let name = cx.interner.resolve(&name_spur).to_string();
                match name.as_str() {
                    "println" | "print" | "eprintln" | "eprint" => {
                        // println!(...) → unit expression (no actual printing).
                        // The macro call is valid but produces no value.
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    "stringify" => {
                        // stringify!(expr) → &str type local (simplified).
                        // Since cx.interner is &Rodeo (immutable), we can't
                        // intern a new string here. Produce a str-typed local
                        // without assigning a constant (typeck will resolve).
                        let str_ty = Ty::new(
                            TyKind::Ref(
                                Region::Static,
                                crate::mir::ty::Mutability::Immutable,
                                Box::new(Ty::new(TyKind::Str, expr.span)),
                            ),
                            expr.span,
                        );
                        cx.mir.new_local(str_ty, None, expr.span)
                    }
                    "assert" | "debug_assert" => {
                        // assert!(cond) → unit expression (assertion check).
                        // For now, just produce unit (no actual assertion codegen).
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    _ => {
                        // Unknown macro → Error placeholder (fallback).
                        cx.eval_rvalue_to_temp(
                            Rvalue::Use(Operand::Constant(Const {
                                ty: Box::new(Ty::new(TyKind::Error, Span::DUMMY)),
                                val: ConstVal::Int(0),
                            })),
                            Ty::new(TyKind::Error, Span::DUMMY),
                            expr.span,
                        )
                    }
                }
            } else {
                // No macro name → Error placeholder.
                cx.eval_rvalue_to_temp(
                    Rvalue::Use(Operand::Constant(Const {
                        ty: Box::new(Ty::new(TyKind::Error, Span::DUMMY)),
                        val: ConstVal::Int(0),
                    })),
                    Ty::new(TyKind::Error, Span::DUMMY),
                    expr.span,
                )
            }
        }

        // Unsafe block: just lower inner block (unsafety is a typeck concern)
        HirExprKind::Unsafe(block) => lower_block(cx, block),

        // MethodCall: `receiver.method(args)` → simplified to Call
        HirExprKind::MethodCall {
            receiver,
            method,
            args,
            ..
        } => {
            let recv_local = lower_expr_to_operand(cx, receiver);
            let arg_locals: Vec<LocalId> =
                args.iter().map(|a| lower_expr_to_operand(cx, a)).collect();

            // Stage 5.78: dyn Trait path.
            //
            // When `cx.dyn_trait_plan()` is set AND the method name matches
            // an entry in the plan, use the dyn Trait call terminator
            // (which records the call info in `cx.mir.dyn_trait_calls`
            // side-table for codegen Stage 5.79+ to emit a vtable indirect
            // call). Otherwise fall through to the legacy placeholder path.
            //
            // Per §16: the plan was built upstream (by the driver, using
            // `build_dyn_trait_mir_plan_from_resolver()`) and attached via
            // `cx.set_dyn_trait_plan()`. The lower does not query HIR or
            // TraitResolver directly here.
            //
            // We clone the matched `DynTraitMethodCall` out of the
            // immutable borrow scope before mutating `cx` — this satisfies
            // the borrow checker (immutable borrow of `cx.dyn_trait_plan()`
            // ends before the mutable borrow begins via `build_dyn_trait_call_terminator`).
            let method_name = cx.interner.resolve(&method.name).to_string();
            let matched_call: Option<DynTraitMethodCall> = cx.dyn_trait_plan().and_then(|plan| {
                find_dyn_trait_method_call_in_plan_by_method(plan, &method_name).cloned()
            });
            if let Some(call) = matched_call {
                let dest_ty = cx.fresh_infer_ty(expr.span);
                let dest = cx.mir.new_local(dest_ty, None, expr.span);
                let cont = cx.new_block();
                let mut terminator = build_dyn_trait_call_terminator(
                    cx,
                    &call,
                    recv_local,
                    &arg_locals,
                    dest,
                    expr.span,
                );
                // Set the target before terminating — the helper
                // leaves it as None per design.
                if let Terminator::Call { target, .. } = &mut terminator {
                    *target = Some(cont);
                }
                cx.terminate_and_goto(terminator, cont);
                return dest;
            }

            // Legacy placeholder path (Stage 2.1) — unchanged.
            let arg_operands: Vec<Operand> =
                std::iter::once(Operand::Copy(Place::local(recv_local, receiver.span)))
                    .chain(
                        arg_locals
                            .iter()
                            .map(|l| Operand::Copy(Place::local(*l, Span::DUMMY))),
                    )
                    .collect();
            let dest_ty = cx.fresh_infer_ty(expr.span);
            let dest = cx.mir.new_local(dest_ty, None, expr.span);
            let cont = cx.new_block();
            cx.terminate_and_goto(
                Terminator::Call {
                    func: Operand::Constant(Const {
                        ty: Box::new(Ty::new(TyKind::Error, Span::DUMMY)),
                        val: ConstVal::Int(0),
                    }), // placeholder func
                    args: arg_operands,
                    destination: Place::local(dest, expr.span),
                    target: Some(cont),
                },
                cont,
            );
            dest
        }
    }
}

/// Resolve the type of a specific field of a struct, given the receiver
/// expression and the field index.
///
/// Stage 3.32 (L-DEBT-2 fix): looks up the receiver's struct DefId (via
/// `find_receiver_struct_def_id`), then reads the field's type from the
/// HIR struct definition. Returns `None` if the receiver isn't a struct
/// or the field index is out of bounds — caller falls back to
/// `fresh_infer_ty`.
///
/// Per §16: this is MIR lower reading HIR (allowed — data flows downstream).
/// The resolved type is sunk into `ProjectionElem::Field(_, field_ty)` so
/// codegen reads it from MIR.
fn resolve_field_type(cx: &MirLowerCtxt, receiver: &HirExpr, field_index: u32) -> Option<Ty> {
    let hir = cx.hir?;
    let struct_def_id = find_receiver_struct_def_id(cx, receiver)?;
    let owner = hir.owner(struct_def_id)?;
    if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s)) = owner {
        let field = s.fields.get(field_index as usize)?;
        Some(lower_hir_ty_to_mir_ty(&field.ty))
    } else {
        None
    }
}

/// Resolve the field index for a field-access expression `receiver.ident`.
///
/// Stage 3.30 fix: was hardcoded `FieldId(0)` — meant `p.1`, `p.x`, etc.
/// all returned field 0 (silently wrong). Now:
///   - For tuple struct fields (`p.0`, `p.1`), the ident is the stringified
///     index — parse it directly.
///   - For named struct fields (`p.x`), look up the field index in the HIR
///     struct definition by matching the field name.
///   - If we can't resolve (e.g., receiver type unknown), default to 0
///     (legacy behavior — typeck should catch real errors).
///   - Stage 3.32 fix: if the receiver's type can't be resolved (e.g.,
///     `let m = Mixed { ... }; m.b` — m's type is Infer(TyVar) at lower
///     time), scan all HIR struct owners for one that has a field with
///     the given name. If exactly one match is found, use it. This is
///     O(structs × fields) but correct for typical crates.
fn resolve_field_index(
    cx: &MirLowerCtxt,
    receiver: &HirExpr,
    field_name: &crate::lexer::Symbol,
) -> u32 {
    use crate::lexer::Symbol;
    // First, try parsing as a tuple-struct field index (`0`, `1`, etc.).
    if let Some(hir_crate) = cx.hir {
        // Get the field name as a string.
        if let Some(name_str) = cx.interner.try_resolve(field_name) {
            if let Ok(idx) = name_str.parse::<u32>() {
                return idx;
            }
            // Named field — try to find the receiver's struct def_id.
            if let Some(struct_def_id) = find_receiver_struct_def_id(cx, receiver) {
                if let Some(crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s))) =
                    hir_crate.owner(struct_def_id)
                {
                    for (i, f) in s.fields.iter().enumerate() {
                        if let Some(f_ident) = &f.ident {
                            if f_ident.name == *field_name {
                                return i as u32;
                            }
                        }
                    }
                }
            }
            // Stage 3.32: receiver type not resolved yet. Scan all struct
            // owners for one with a matching field name.
            let mut found: Option<(u32,)> = None;
            let mut ambiguous = false;
            for (_def_id, owner) in &hir_crate.owners {
                if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s)) = owner {
                    for (i, f) in s.fields.iter().enumerate() {
                        if let Some(f_ident) = &f.ident {
                            if f_ident.name == *field_name {
                                if found.is_some() {
                                    ambiguous = true;
                                } else {
                                    found = Some((i as u32,));
                                }
                                break;
                            }
                        }
                    }
                }
                if ambiguous {
                    break;
                }
            }
            if let Some((idx,)) = found {
                if !ambiguous {
                    return idx;
                }
            }
        }
    }
    let _: Symbol = crate::lexer::Symbol::default();
    0
}

/// Find the struct DefId that a receiver expression's type resolves to.
///
/// Used by `resolve_field_index` to look up named fields. Walks the
/// receiver's MIR local decl to find its type, and if it's `TyKind::Adt`,
/// returns the DefId.
fn find_receiver_struct_def_id(cx: &MirLowerCtxt, receiver: &HirExpr) -> Option<crate::hir::DefId> {
    // Lower the receiver to find its local id (without actually lowering
    // again — we just need the type). Since lower_expr_to_operand has
    // side effects, we use a different approach: pattern-match the
    // receiver to extract its type from HIR.
    //
    // For a simple `let p = Point { ... }; p.x` — the receiver is a Path
    // that resolves to a local. We'd need to track locals → types.
    //
    // Simpler: walk the receiver and if it's a Path to a local, look up
    // the local's type from cx.local_map → mir.local_decls.
    match &receiver.kind {
        HirExprKind::Path(path) => {
            if let crate::hir::Res::Local(hir_id) = path.res {
                if let Some(local_id) = cx.local_map.get(&hir_id) {
                    if let Some(ld) = cx.mir.local_decls.get(local_id.0 as usize) {
                        if let TyKind::Adt(def_id, _) = &ld.ty.kind {
                            return Some(*def_id);
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Resolve the field types of an ADT (struct/enum variant) by looking up
/// the HIR owner.
///
/// Stage 3.30 (per §16 阶段间接口隔离): this is called by MIR lower to
/// sink the field types into `AggregateKind::Adt`'s `field_tys` field,
/// so codegen doesn't have to re-query HIR (which would be a cross-stage
/// internal-API call).
/// Stage 3.52: Resolve the element type of an index expression `base[idx]`
/// by inspecting the base's MIR type. For:
///   - `&[T]` (Ref to Slice(T)): elem_ty = T
///   - `[T; N]` (Array(T, _)): elem_ty = T
///   - `&[T; N]` (Ref to Array(T, _)): elem_ty = T
///
/// Returns `None` if the base's type can't be resolved (e.g., fresh infer
/// var in test contexts). The caller falls back to a fresh infer var in
/// that case, preserving the old behavior.
///
/// Per §16 (阶段间接口隔离): reads MIR local_decls only (data flows
/// downstream per §16.2.1 — MIR lower reads its own body). No HIR lookup.
fn resolve_index_element_type(cx: &MirLowerCtxt, base_local: LocalId) -> Option<Ty> {
    let base_ty = cx.mir.local_decls.get(base_local.0 as usize)?.ty.clone();
    match &base_ty.kind {
        // `&[T]` — fat pointer to slice
        // `&str` — fat pointer to str (element is u8)
        TyKind::Ref(_, _, inner) => match &inner.kind {
            TyKind::Slice(elem) => Some((**elem).clone()),
            TyKind::Array(elem, _) => Some((**elem).clone()),
            // Stage 3.53: `&str` indexing → element is u8 (like `&[u8]`).
            // Was: fell through to None → fresh_infer_ty → typeck default i32,
            // causing `s[0]` on `&str` to store i8 into an i32 temp (type mismatch).
            TyKind::Str => Some(Ty::new(TyKind::Uint(ast::UintTy::U8), Span::DUMMY)),
            _ => None,
        },
        // `[T; N]` — array
        TyKind::Array(elem, _) => Some((**elem).clone()),
        // `&[T; N]` — array reference (thin pointer to array)
        TyKind::Slice(elem) => Some((**elem).clone()),
        _ => None,
    }
}

/// Resolve the declared field types of an ADT (struct or enum variant).
///
/// For structs, returns the declared field types. For enums, returns the
/// variant's field types (Stage 3.31+ — currently returns empty for
/// non-struct owners). Returns empty if HIR is not available (e.g., in
/// test contexts that construct MirLowerCtxt without a HIR crate).
fn resolve_adt_field_tys(cx: &MirLowerCtxt, def_id: crate::hir::DefId) -> Vec<Ty> {
    let hir = match cx.hir {
        Some(h) => h,
        None => return Vec::new(),
    };
    match hir.owner(def_id) {
        Some(crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s))) => s
            .fields
            .iter()
            .map(|f| lower_hir_ty_to_mir_ty(&f.ty))
            .collect(),
        // Stage 3.38 (L-ENUM): Enum variant field types.
        // For enums, the field_tys include a discriminant (i32) as the
        // first element, followed by the variant's payload field types.
        // This is called with def_id = enum_def_id and variant_index = 0
        // (from the Call/Struct paths that don't yet resolve variant).
        // The variant-aware version is `resolve_enum_variant_field_tys`.
        Some(crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(_))) => {
            // Fallback: return just the discriminant (unit variant).
            vec![Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)]
        }
        _ => Vec::new(),
    }
}

/// Stage 3.38 (L-ENUM): Resolve the variant index and field types for an
/// enum variant construction.
///
/// Given an enum DefId and a variant name, looks up the variant in the HIR
/// enum definition. Returns:
///   - `Some((variant_index, field_tys))` where field_tys includes the
///     discriminant (i32) as the first element, followed by the variant's
///     payload field types.
///   - `None` if the variant isn't found.
///
/// Per §16: MIR lower reads HIR (allowed — data flows downstream). The
/// resolved field_tys are sunk into `AggregateKind::Adt` so codegen reads
/// from MIR.
pub(crate) fn resolve_enum_variant(
    cx: &MirLowerCtxt,
    enum_def_id: crate::hir::DefId,
    variant_name: &crate::lexer::Symbol,
) -> Option<(u32, Vec<Ty>)> {
    let hir = cx.hir?;
    let owner = hir.owner(enum_def_id)?;
    let enum_def = match owner {
        crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(e)) => e,
        _ => return None,
    };
    for (i, variant) in enum_def.variants.iter().enumerate() {
        if variant.ident.name == *variant_name {
            // Found the variant. Build field_tys: [discriminant, payload...]
            let mut field_tys = vec![Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)];
            match &variant.data {
                crate::hir::HirVariantData::Unit(_) => {
                    // No payload — just the discriminant.
                }
                crate::hir::HirVariantData::Tuple(fields, _) => {
                    for f in fields {
                        field_tys.push(lower_hir_ty_to_mir_ty(&f.ty));
                    }
                }
                crate::hir::HirVariantData::Struct(fields, _) => {
                    for f in fields {
                        field_tys.push(lower_hir_ty_to_mir_ty(&f.ty));
                    }
                }
            }
            return Some((i as u32, field_tys));
        }
    }
    None
}

/// Whether a HIR binary op can overflow (and thus needs an Assert check).
///
/// Comparison ops (Eq/Ne/Lt/Le/Gt/Ge) and bitwise ops (BitAnd/BitOr/BitXor)
/// cannot overflow. Arithmetic (Add/Sub/Mul/Div/Rem) and shift ops
/// (Shl/Shr) can.
fn is_overflowable_op(op: HirBinOp) -> bool {
    matches!(
        op,
        HirBinOp::Add
            | HirBinOp::Sub
            | HirBinOp::Mul
            | HirBinOp::Div
            | HirBinOp::Rem
            | HirBinOp::Shl
            | HirBinOp::Shr
    )
}

/// Emit an `Assert` terminator that checks for arithmetic overflow.
///
/// Stage 3.24: now carries `lhs` and `rhs` operands in the `Overflow` message
/// so codegen can emit `llvm.{sadd,ssub,smul}.with.overflow.*` intrinsics and
/// branch on the real overflow flag. The `cond` field of the Assert remains
/// `Bool(true)` for backward compatibility with typeck/borrowck (which treat
/// the Assert as a normal terminator) — codegen ignores `cond` for Overflow
/// messages and uses the operands directly.
///
/// The Assert is emitted as the terminator of the current block, and
/// a fresh continuation block is created for the rest of the code.
fn emit_overflow_assert(
    cx: &mut MirLowerCtxt,
    result: LocalId,
    op: BinOp,
    lhs: Operand,
    rhs: Operand,
    span: Span,
) {
    let cont = cx.new_block();
    cx.terminate_and_goto(
        Terminator::Assert {
            // Backward-compat placeholder: codegen computes the real
            // overflow flag from `lhs` and `rhs` in the Overflow message.
            cond: Operand::Constant(Const {
                ty: Box::new(Ty::new(TyKind::Bool, span)),
                val: ConstVal::Bool(true),
            }),
            expected: true,
            target: cont,
            msg: crate::mir::body::AssertMessage::Overflow(op, lhs, rhs),
        },
        cont,
    );
    // Silence unused warning for `result` — kept for API stability.
    let _ = result;
}

/// Emit an `Assert` terminator that checks for division by zero.
///
/// Stage 3.25: emitted for `Div` and `Rem` operations. The `rhs` operand
/// is stored in the `DivisionByZero` message so codegen can emit
/// `icmp eq rhs, 0` and branch to a panic block on true.
///
/// `result` is unused (kept for API symmetry with `emit_overflow_assert`).
fn emit_div_by_zero_assert(cx: &mut MirLowerCtxt, result: LocalId, rhs: Operand, span: Span) {
    let cont = cx.new_block();
    cx.terminate_and_goto(
        Terminator::Assert {
            cond: Operand::Constant(Const {
                ty: Box::new(Ty::new(TyKind::Bool, span)),
                val: ConstVal::Bool(true),
            }),
            expected: true,
            target: cont,
            msg: crate::mir::body::AssertMessage::DivisionByZero(rhs),
        },
        cont,
    );
    let _ = result;
}

/// Lower a short-circuit `&&` / `||` expression to MIR control flow.
///
/// `a && b` lowers to:
/// ```text
/// bb0: switchInt(a) { true => bb1, _ => bb_result_false }
/// bb1: switchInt(b) { true => bb_result_true, _ => bb_result_false }
/// bb_result_true:  result = true;  goto bb_cont
/// bb_result_false: result = false; goto bb_cont
/// bb_cont: (continuation)
/// ```
///
/// `a || b` is symmetric (swap true/false targets).
///
/// The key invariant: `b` is only evaluated if `a` doesn't short-circuit.
/// This is required for correctness — e.g., `ptr != null && *ptr == 42`
/// must not dereference a null pointer.
fn lower_short_circuit(
    cx: &mut MirLowerCtxt,
    op: HirBinOp,
    lhs: &HirExpr,
    rhs: &HirExpr,
    span: Span,
) -> LocalId {
    let lhs_local = lower_expr_to_operand(cx, lhs);
    let eval_rhs_block = cx.new_block();
    let short_circuit_block = cx.new_block();
    let result_true_block = cx.new_block();
    let result_false_block = cx.new_block();
    let cont_block = cx.new_block();
    let result_local = cx.mir.new_local_with_mut(
        Ty::new(TyKind::Bool, span),
        None,
        span,
        crate::mir::ty::Mutability::Mutable,
    );

    // bb_curr: switchInt(lhs) → {true: eval_rhs, _: short_circuit}
    // For `&&`: short-circuit value is `false` (if lhs is false, result is false).
    // For `||`: short-circuit value is `true` (if lhs is true, result is true).
    let (true_target, false_target) = match op {
        HirBinOp::And => (eval_rhs_block, short_circuit_block),
        HirBinOp::Or => (short_circuit_block, eval_rhs_block),
        _ => unreachable!("lower_short_circuit called with non-And/Or op"),
    };
    cx.terminate(Terminator::SwitchInt {
        discr: Operand::Copy(Place::local(lhs_local, lhs.span)),
        targets: vec![(ConstVal::Bool(true), true_target)],
        otherwise: false_target,
    });

    // short_circuit_block: result = (op == Or); goto cont
    //   For `&&`: lhs was false → result = false
    //   For `||`: lhs was true  → result = true
    cx.current_block = short_circuit_block;
    let short_val = matches!(op, HirBinOp::Or);
    cx.push_assign(
        Place::local(result_local, span),
        Rvalue::Use(Operand::Constant(Const {
            ty: Box::new(Ty::new(TyKind::Bool, Span::DUMMY)),
            val: ConstVal::Bool(short_val),
        })),
        span,
    );
    cx.terminate(Terminator::Goto(cont_block));

    // eval_rhs_block: evaluate rhs, switchInt(rhs) → {true: result_true, _: result_false}
    cx.current_block = eval_rhs_block;
    let rhs_local = lower_expr_to_operand(cx, rhs);
    cx.terminate(Terminator::SwitchInt {
        discr: Operand::Copy(Place::local(rhs_local, rhs.span)),
        targets: vec![(ConstVal::Bool(true), result_true_block)],
        otherwise: result_false_block,
    });

    // result_true_block: result = true; goto cont
    cx.current_block = result_true_block;
    cx.push_assign(
        Place::local(result_local, span),
        Rvalue::Use(Operand::Constant(Const {
            ty: Box::new(Ty::new(TyKind::Bool, Span::DUMMY)),
            val: ConstVal::Bool(true),
        })),
        span,
    );
    cx.terminate(Terminator::Goto(cont_block));

    // result_false_block: result = false; goto cont
    cx.current_block = result_false_block;
    cx.push_assign(
        Place::local(result_local, span),
        Rvalue::Use(Operand::Constant(Const {
            ty: Box::new(Ty::new(TyKind::Bool, Span::DUMMY)),
            val: ConstVal::Bool(false),
        })),
        span,
    );
    cx.terminate(Terminator::Goto(cont_block));

    // Continuation
    cx.current_block = cont_block;
    result_local
}

/// Lower a HIR deref expression `*inner` to MIR.
///
/// `*p` reads the value at the place `Projection(p, Deref)`. We:
/// 1. Lower `inner` to a local (the pointer/reference value)
/// 2. Construct `Place::Projection(local, Deref)`
/// 3. Assign `result = Use(Copy(projection))` to a fresh temp
///
/// The temp's type is left as a fresh inference variable — typeck will
/// unify it with the pointee type via `infer_projection(Deref)`.
fn lower_deref_expr(cx: &mut MirLowerCtxt, inner: &HirExpr, span: Span) -> LocalId {
    let inner_local = lower_expr_to_operand(cx, inner);
    let proj = Place {
        kind: PlaceKind::Projection(
            Box::new(Place::local(inner_local, inner.span)),
            ProjectionElem::Deref,
        ),
        span,
    };
    let result_ty = cx.fresh_infer_ty(span);
    cx.eval_rvalue_to_temp(Rvalue::Use(Operand::Copy(proj)), result_ty, span)
}
/// evaluates the trailing expression (if any). Returns the LocalId
/// of the block's result value.
fn lower_block(cx: &mut MirLowerCtxt, block: &HirBlock) -> LocalId {
    for stmt in &block.stmts {
        match stmt {
            HirStmt::Local(local) => {
                // Lower the init expression first (if present)
                if let Some(init) = &local.init {
                    let init_local = lower_expr_to_operand(cx, init);
                    // Allocate a local for this binding. If the let has
                    // an explicit type annotation (`let x: T = ...`), use
                    // it directly; this lets typeck unify the init's type
                    // with the annotation, catching mismatches like
                    // `let x: bool = 42`.
                    let ty = match &local.ty {
                        Some(t) => lower_hir_ty_to_mir_ty(t),
                        None => cx.fresh_infer_ty(local.span),
                    };
                    // G1 fix (Stage 2.4e): use `local.pat.hir_id` (not
                    // `local.hir_id`) as the local_map key. The resolver
                    // inserts bindings into the scope keyed by `pat.hir_id`,
                    // so Path expressions resolve to `pat.hir_id`. Using
                    // `local.hir_id` would create a mismatch and cause all
                    // let-bound variables to be unresolvable in Path
                    // expressions.
                    //
                    // G5 fix (Stage 2.4e): extract mutability from the
                    // pattern's BindingMode. `let mut x = ...` produces
                    // `ByValue(Mutable)`. Without this, all locals are
                    // immutable and the borrow checker can't catch
                    // `let x = 1; x = 2;`.
                    let mutability = pattern_bindings::pat_mutability(&local.pat);
                    let local_id = cx.new_local_with_mut(local.pat.hir_id, ty, None, mutability);
                    // Emit StorageLive to mark the local as in-scope.
                    // Codegen uses this to allocate stack space.
                    cx.mir
                        .block_mut(cx.current_block)
                        .statements
                        .push(Statement {
                            kind: StatementKind::StorageLive(local_id),
                            span: local.span,
                        });
                    // Use Operand::Move instead of Operand::Copy. For Copy
                    // types, Move is equivalent to Copy (the source remains
                    // valid). For non-Copy types, Move correctly transfers
                    // ownership. Using Copy here would fail the borrow
                    // checker's Copy-ness check on non-Copy types (e.g.,
                    // `let s = "hello"` where s : Str — Str is not Copy).
                    cx.push_assign(
                        Place::local(local_id, local.span),
                        Rvalue::Use(Operand::Move(Place::local(init_local, init.span))),
                        local.span,
                    );
                } else {
                    // No init: just allocate the local. If a type annotation
                    // is present, use it; otherwise fresh Infer var.
                    let ty = match &local.ty {
                        Some(t) => lower_hir_ty_to_mir_ty(t),
                        None => cx.fresh_infer_ty(local.span),
                    };
                    // G1 fix: use pat.hir_id (see comment above).
                    // G5 fix: extract mutability.
                    let mutability = pattern_bindings::pat_mutability(&local.pat);
                    let local_id = cx.new_local_with_mut(local.pat.hir_id, ty, None, mutability);
                    // Emit StorageLive even for uninit locals (codegen
                    // still needs to allocate stack space).
                    cx.mir
                        .block_mut(cx.current_block)
                        .statements
                        .push(Statement {
                            kind: StatementKind::StorageLive(local_id),
                            span: local.span,
                        });
                }
            }
            HirStmt::Expr(e, _) => {
                lower_expr_to_operand(cx, e);
            }
            _ => {}
        }
    }
    // Trailing expression
    if let Some(expr) = &block.expr {
        lower_expr_to_operand(cx, expr)
    } else {
        // No trailing expr → unit
        cx.eval_rvalue_to_temp(
            Rvalue::Aggregate(AggregateKind::Tuple, vec![]),
            Ty::new(TyKind::Tuple(vec![]), block.span),
            block.span,
        )
    }
}

/// Lower an if expression to MIR control flow.
///
/// ```text
/// bb0: switchInt(cond) { true => bb1, _ => bb2 }
/// bb1: ... (then block) ... goto bb3
/// bb2: ... (else block) ... goto bb3
/// bb3: (continuation)
/// ```
fn lower_if(
    cx: &mut MirLowerCtxt,
    cond: &HirExpr,
    then: &HirBlock,
    else_: Option<&HirExpr>,
    span: Span,
) -> LocalId {
    let cond_local = lower_expr_to_operand(cx, cond);
    let then_block = cx.new_block();
    let else_block = cx.new_block();
    let cont_block = cx.new_block();
    let result_ty = cx.fresh_infer_ty(span);
    let result_local =
        cx.mir
            .new_local_with_mut(result_ty, None, span, crate::mir::ty::Mutability::Mutable);

    // Terminate current block: switchInt(cond) { 1 => then, _ => else }
    cx.terminate(Terminator::SwitchInt {
        discr: Operand::Copy(Place::local(cond_local, cond.span)),
        targets: vec![(ConstVal::Bool(true), then_block)],
        otherwise: else_block,
    });

    // Then block
    cx.current_block = then_block;
    let then_result = lower_block(cx, then);
    cx.push_assign(
        Place::local(result_local, span),
        Rvalue::Use(Operand::Copy(Place::local(then_result, then.span))),
        then.span,
    );
    cx.terminate(Terminator::Goto(cont_block));

    // Else block
    cx.current_block = else_block;
    if let Some(else_expr) = else_ {
        let else_result = lower_expr_to_operand(cx, else_expr);
        cx.push_assign(
            Place::local(result_local, span),
            Rvalue::Use(Operand::Copy(Place::local(else_result, else_expr.span))),
            else_expr.span,
        );
    } else {
        // No else → unit
        cx.push_assign(
            Place::local(result_local, span),
            Rvalue::Aggregate(AggregateKind::Tuple, vec![]),
            span,
        );
    }
    cx.terminate(Terminator::Goto(cont_block));

    // Continuation block
    cx.current_block = cont_block;
    result_local
}

/// Lower a match expression to MIR control flow.
///
/// For Stage 2.1b, match is lowered to a simplified SwitchInt:
/// - Integer literal patterns → constant targets
/// - Wildcard `_` → otherwise
/// - Other patterns → otherwise (simplified)
fn lower_match(cx: &mut MirLowerCtxt, scrutinee: &HirExpr, arms: &[HirArm], span: Span) -> LocalId {
    let scrut_local = lower_expr_to_operand(cx, scrutinee);
    let cont_block = cx.new_block();
    let result_ty = cx.fresh_infer_ty(span);
    let result_local =
        cx.mir
            .new_local_with_mut(result_ty, None, span, crate::mir::ty::Mutability::Mutable);

    // Stage 3.40 (L-ENUM-MATCH): Check if the scrutinee is an enum type.
    // If so, extract the discriminant (field 0 of the enum struct) and
    // switch on that instead of the enum value itself.
    //
    // We check both the MIR local type AND the HIR enum owners — the
    // local type may be Infer (if typeck hasn't resolved it yet at lower
    // time) but the HIR owner can tell us it's an enum.
    let scrut_ty = cx
        .mir
        .local_decls
        .get(scrut_local.0 as usize)
        .map(|ld| ld.ty.clone())
        .unwrap_or_else(|| Ty::new(TyKind::Error, span));
    let is_enum = matches!(&scrut_ty.kind, TyKind::Adt(def_id, _) if cx.hir.and_then(|h| h.owner(*def_id)).is_some_and(|o| {
        matches!(o, crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(_)))
    }));
    // Also check: if any arm pattern is an enum variant path, treat as enum.
    let has_enum_pat = arms.iter().any(|arm| {
        matches!(&arm.pat.kind, HirPatKind::Path(p) | HirPatKind::TupleStruct(p, _) | HirPatKind::Struct(p, _, _)
            if matches!(p.res, Res::Def(_, crate::resolve::DefKind::Enum)))
    });
    let is_enum = is_enum || has_enum_pat;

    // If enum, extract discriminant: discr = scrut.0 (field 0 of the struct).
    let switch_discr = if is_enum {
        // Create a temp local for the extracted discriminant.
        let discr_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), span);
        let discr_local = cx.mir.new_local(discr_ty.clone(), None, span);
        cx.push_assign(
            Place::local(discr_local, span),
            Rvalue::Use(Operand::Move(Place {
                kind: PlaceKind::Projection(
                    Box::new(Place::local(scrut_local, scrutinee.span)),
                    ProjectionElem::Field(FieldId(0), discr_ty.clone()),
                ),
                span,
            })),
            span,
        );
        Operand::Copy(Place::local(discr_local, span))
    } else {
        Operand::Copy(Place::local(scrut_local, scrutinee.span))
    };

    // Collect targets: (constant, arm_block) pairs
    let mut targets: Vec<(ConstVal, BasicBlockId)> = Vec::new();
    let mut arm_blocks: Vec<BasicBlockId> = Vec::new();
    let otherwise_block = cx.new_block();

    for arm in arms {
        let arm_block = cx.new_block();
        arm_blocks.push(arm_block);

        // Check if this arm's pattern is a literal
        if let HirPatKind::Lit(expr) = &arm.pat.kind {
            if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                targets.push((ConstVal::Int(*n), arm_block));
                continue;
            }
            if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                targets.push((ConstVal::Bool(*b), arm_block));
                continue;
            }
        }

        // Stage 3.40 (L-ENUM-MATCH): Handle enum variant patterns.
        // `Color::Red` → HirPatKind::Path(path) where path resolves to enum.
        // `Opt::Some(x)` → HirPatKind::TupleStruct(path, sub_pats).
        // Resolve the variant index and use it as the switch target.
        if is_enum {
            let variant_idx = match &arm.pat.kind {
                HirPatKind::Path(path) => {
                    // Unit variant pattern: `Color::Red`
                    if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                        if path.segments.len() >= 2 {
                            resolve_enum_variant(cx, def_id, &path.segments[1].ident.name)
                                .map(|(idx, _)| idx)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                HirPatKind::TupleStruct(path, _) => {
                    // Tuple variant pattern: `Opt::Some(x)`
                    if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                        if path.segments.len() >= 2 {
                            resolve_enum_variant(cx, def_id, &path.segments[1].ident.name)
                                .map(|(idx, _)| idx)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                HirPatKind::Struct(path, _, _) => {
                    // Struct variant pattern: `Shape::Circle { r: x }`
                    if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                        if path.segments.len() >= 2 {
                            resolve_enum_variant(cx, def_id, &path.segments[1].ident.name)
                                .map(|(idx, _)| idx)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(idx) = variant_idx {
                targets.push((ConstVal::Uint(idx as u128), arm_block));
                continue;
            }
        }

        // Non-literal patterns (Wild, Ident, etc.) → go to otherwise
    }

    // Terminate current block with SwitchInt
    cx.terminate(Terminator::SwitchInt {
        discr: switch_discr,
        targets: targets.clone(),
        otherwise: otherwise_block,
    });

    // Lower each arm body
    for (i, arm) in arms.iter().enumerate() {
        let arm_block = arm_blocks[i];
        cx.current_block = arm_block;

        // Collect pattern bindings (for Ident patterns)
        pattern_bindings::collect_pat_bindings_for_mir(cx, &arm.pat);
        // Stage 3.48 (L-ENUM-BINDING): generate payload-extraction projections
        // for enum tuple/struct variant patterns (e.g., `Opt::Some(x)`).
        // Before this fix, the binding `x` was never assigned — reading
        // uninitialized memory (P0 soundness bug).
        pattern_bindings::lower_enum_variant_pattern_bindings(cx, scrut_local, &arm.pat);

        // Lower the arm body
        let arm_result = lower_expr_to_operand(cx, &arm.body);
        cx.push_assign(
            Place::local(result_local, span),
            Rvalue::Use(Operand::Copy(Place::local(arm_result, arm.body.span))),
            arm.span,
        );
        cx.terminate(Terminator::Goto(cont_block));
    }

    // Lower the otherwise block (for non-literal patterns)
    cx.current_block = otherwise_block;
    // Find the first arm with a non-literal pattern
    for arm in arms {
        let is_literal = matches!(&arm.pat.kind, HirPatKind::Lit(_));
        if !is_literal {
            pattern_bindings::collect_pat_bindings_for_mir(cx, &arm.pat);
            // Stage 3.48 (L-ENUM-BINDING): same as above, for the otherwise arm.
            pattern_bindings::lower_enum_variant_pattern_bindings(cx, scrut_local, &arm.pat);
            let arm_result = lower_expr_to_operand(cx, &arm.body);
            cx.push_assign(
                Place::local(result_local, span),
                Rvalue::Use(Operand::Copy(Place::local(arm_result, arm.body.span))),
                arm.span,
            );
            break;
        }
    }
    cx.terminate(Terminator::Goto(cont_block));

    // Continuation
    cx.current_block = cont_block;
    result_local
}
