//! HIR → MIR lowering.
//!
//! Converts each HIR body (expression tree) into a MIR body (control
//! flow graph of basic blocks + statements + terminators).
//!
//! Public entry point: [`lower_hir_body_to_mir`].

use crate::ast;
use crate::hir::*;
use crate::mir::body::*;
use crate::mir::lvalue::*;
use crate::mir::ty::*;
use crate::session::Span;
use crate::typeck::unify::UnificationTable;
use lasso::Rodeo;

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
    pub fn push_assign(&mut self, place: Lvalue, rvalue: Rvalue, span: Span) {
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
        self.push_assign(Lvalue::local(temp, span), rvalue, span);
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
            HirLitKind::Str(sym) => (
                Const {
                    ty: Box::new(Ty::new(TyKind::Str, Span::DUMMY)),
                    val: ConstVal::Str(*sym),
                },
                Ty::new(TyKind::Str, Span::DUMMY),
            ),
            HirLitKind::ByteStr(sym) => {
                // `[u8; N]` — we don't know N at this point without
                // computing the byte length. For Stage 2.4d, represent
                // as Slice(u8) which is close enough for typeck
                // (codegen will need the real array type — Stage 3).
                let elem_ty = Ty::new(TyKind::Uint(ast::UintTy::U8), Span::DUMMY);
                let slice_ty = Ty::new(TyKind::Slice(Box::new(elem_ty)), Span::DUMMY);
                (
                    Const {
                        ty: Box::new(slice_ty.clone()),
                        // Reuse Str variant — codegen will interpret
                        // the symbol as bytes when the type is Slice(u8).
                        val: ConstVal::Str(*sym),
                    },
                    slice_ty,
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
    let mut cx = MirLowerCtxt::new(interner, body.span);
    cx.hir = Some(hir);

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
        Lvalue::local(return_local, Span::DUMMY),
        Rvalue::Use(Operand::Copy(Lvalue::local(value_local, Span::DUMMY))),
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

    // Extract the unify table before consuming cx.
    let unify = std::mem::take(&mut cx.unify);
    (cx.mir, unify)
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
pub fn lower_hir_ty_to_mir_ty(ty: &HirTy) -> Ty {
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
        HirTyKind::Path(_, path) => {
            if let Res::Def(def_id, _) = path.res {
                Ty::new(TyKind::Adt(def_id, Vec::new()), span)
            } else {
                Ty::new(TyKind::Error, span)
            }
        }
        _ => Ty::new(TyKind::Error, span), // complex types → Error for now
    }
}

/// Lower a HIR expression to a MIR Lvalue (a place that can be assigned to).
///
/// Stage 3.34 (L-MUT-1 fix): used by `HirExprKind::Assign` to lower the LHS
/// into a place. Handles:
///   - `Path` (local variable) → `Lvalue::Local`
///   - `Field { receiver, ident }` → `Lvalue::Projection(receiver, Field(idx, ty))`
///   - `Index { receiver, index }` → `Lvalue::Projection(receiver, Index(idx_local))`
///   - `Unary { op: Deref, expr }` → `Lvalue::Projection(expr, Deref)`
///
/// For other expression kinds (which can't be assigned to), falls back to
/// a fresh local — typeck should catch the "assignment to non-place" error.
fn lower_expr_to_lvalue(cx: &mut MirLowerCtxt, expr: &HirExpr) -> Lvalue {
    match &expr.kind {
        HirExprKind::Path(path) => {
            if let Res::Local(hir_id) = path.res {
                if let Some(local_id) = cx.local_of(hir_id) {
                    return Lvalue::local(local_id, expr.span);
                }
            }
            // Fallback: fresh local (error recovery).
            let ty = cx.fresh_infer_ty(expr.span);
            let local = cx.mir.new_local(ty, None, expr.span);
            Lvalue::local(local, expr.span)
        }
        HirExprKind::Field { receiver, ident } => {
            let base = lower_expr_to_lvalue(cx, receiver);
            let field_index = resolve_field_index(cx, receiver, &ident.name);
            let field_ty = resolve_field_type(cx, receiver, field_index)
                .unwrap_or_else(|| cx.fresh_infer_ty(expr.span));
            Lvalue {
                kind: LvalueKind::Projection(
                    Box::new(base),
                    ProjectionElem::Field(FieldId(field_index), field_ty),
                ),
                span: expr.span,
            }
        }
        HirExprKind::Index {
            receiver, index, ..
        } => {
            let base = lower_expr_to_lvalue(cx, receiver);
            let idx_local = lower_expr_to_operand(cx, index);
            Lvalue {
                kind: LvalueKind::Projection(Box::new(base), ProjectionElem::Index(idx_local)),
                span: expr.span,
            }
        }
        HirExprKind::Unary {
            op, expr: inner, ..
        } if *op == HirUnaryOp::Deref => {
            let base = lower_expr_to_lvalue(cx, inner);
            Lvalue {
                kind: LvalueKind::Projection(Box::new(base), ProjectionElem::Deref),
                span: expr.span,
            }
        }
        // Other expression kinds can't be assigned to — return a fresh
        // local as error recovery. typeck should catch this.
        _ => {
            let ty = cx.fresh_infer_ty(expr.span);
            let local = cx.mir.new_local(ty, None, expr.span);
            Lvalue::local(local, expr.span)
        }
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
                        // Default: treat as FnDef (covers Fn, Const, Static,
                        // etc. — the typeck/codegen layers handle the
                        // FnDef-typed operand correctly for fn calls;
                        // Const/Static are still placeholders).
                        let fndef_ty = Ty::new(TyKind::FnDef(def_id, Vec::new()), expr.span);
                        return cx.eval_rvalue_to_temp(
                            Rvalue::Use(Operand::Constant(Const {
                                ty: Box::new(fndef_ty.clone()),
                                // ConstVal::Unit doesn't exist; use Uint(0) as a
                                // placeholder. The actual fn pointer is resolved
                                // at codegen time.
                                val: ConstVal::Uint(def_id.as_u32() as u128),
                            })),
                            fndef_ty,
                            expr.span,
                        );
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
            let lhs_operand = Operand::Copy(Lvalue::local(lhs_local, lhs.span));
            let rhs_operand = Operand::Copy(Lvalue::local(rhs_local, rhs.span));
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
                Rvalue::UnaryOp(
                    mir_op,
                    Operand::Copy(Lvalue::local(inner_local, inner.span)),
                ),
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
                .map(|l| Operand::Copy(Lvalue::local(*l, Span::DUMMY)))
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
                    Lvalue::local(dest, expr.span),
                    Rvalue::Aggregate(
                        AggregateKind::Adt(adt_def_id, variant_idx, Vec::new(), field_tys),
                        all_operands,
                    ),
                    expr.span,
                );
                dest
            } else {
                // Real function call.
                let dest_ty = cx.fresh_infer_ty(Span::DUMMY);
                let dest = cx.mir.new_local(dest_ty, None, expr.span);
                let cont = cx.new_block();
                cx.terminate_and_goto(
                    Terminator::Call {
                        func: Operand::Copy(Lvalue::local(func_local, func.span)),
                        args: arg_operands,
                        destination: Lvalue::local(dest, expr.span),
                        target: Some(cont),
                    },
                    cont,
                );
                dest
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
                    Lvalue::local(LocalId(0), Span::DUMMY),
                    Rvalue::Use(Operand::Copy(Lvalue::local(ret_local, ret.span))),
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
            let lhs_lvalue = lower_expr_to_lvalue(cx, lhs);
            cx.push_assign(
                lhs_lvalue,
                Rvalue::Use(Operand::Copy(Lvalue::local(rhs_local, rhs.span))),
                expr.span,
            );
            rhs_local
        }
        HirExprKind::Tuple { elems, .. } => {
            let elem_locals: Vec<LocalId> =
                elems.iter().map(|e| lower_expr_to_operand(cx, e)).collect();
            let operands: Vec<Operand> = elem_locals
                .iter()
                .map(|l| Operand::Copy(Lvalue::local(*l, Span::DUMMY)))
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
                Lvalue::local(result, expr.span),
                Rvalue::Use(Operand::Copy(Lvalue {
                    kind: LvalueKind::Projection(
                        Box::new(Lvalue::local(base_local, receiver.span)),
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
            let elem_ty = cx.fresh_infer_ty(expr.span);
            let result = cx.mir.new_local(elem_ty, None, expr.span);
            cx.push_assign(
                Lvalue::local(result, expr.span),
                Rvalue::Use(Operand::Copy(Lvalue {
                    kind: LvalueKind::Projection(
                        Box::new(Lvalue::local(base_local, receiver.span)),
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
                crate::ast::Mutability::Mutable => crate::mir::lvalue::BorrowKind::Mut,
                crate::ast::Mutability::Immutable => crate::mir::lvalue::BorrowKind::Shared,
            };
            let ref_ty = cx.fresh_infer_ty(expr.span);
            cx.eval_rvalue_to_temp(
                Rvalue::Ref(Region::Erased, bk, Lvalue::local(inner_local, inner.span)),
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
                    Operand::Copy(Lvalue::local(inner_local, inner.span)),
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
                discr: Operand::Copy(Lvalue::local(cond_local, cond.span)),
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
                discr: Operand::Copy(Lvalue::local(iter_local, iter.span)),
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

        // Closure: `|args| body` → lower body (closure capture is Stage 3+)
        HirExprKind::Closure { params, body, .. } => {
            // Register closure params as locals
            for param in params {
                let ty = cx.fresh_infer_ty(param.pat.span);
                cx.new_local(param.pat.hir_id, ty, None);
            }
            // Lower closure body
            lower_expr_to_operand(cx, body)
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
                operands.push(Operand::Copy(Lvalue::local(s, Span::DUMMY)));
            }
            if let Some(e) = end_local {
                operands.push(Operand::Copy(Lvalue::local(e, Span::DUMMY)));
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
                .map(|l| Operand::Copy(Lvalue::local(*l, Span::DUMMY)))
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
                    vec![Operand::Copy(Lvalue::local(elem_local, elem.span))],
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
                .map(|l| Operand::Copy(Lvalue::local(*l, Span::DUMMY)))
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

        // MacroCall: `foo!(...)` → placeholder (macro expansion is Stage 4)
        HirExprKind::MacroCall { .. } => cx.eval_rvalue_to_temp(
            Rvalue::Use(Operand::Constant(Const {
                ty: Box::new(Ty::new(TyKind::Error, Span::DUMMY)),
                val: ConstVal::Int(0),
            })),
            Ty::new(TyKind::Error, Span::DUMMY),
            expr.span,
        ),

        // Unsafe block: just lower inner block (unsafety is a typeck concern)
        HirExprKind::Unsafe(block) => lower_block(cx, block),

        // MethodCall: `receiver.method(args)` → simplified to Call
        HirExprKind::MethodCall {
            receiver,
            method: _,
            args,
            ..
        } => {
            let recv_local = lower_expr_to_operand(cx, receiver);
            let arg_locals: Vec<LocalId> =
                args.iter().map(|a| lower_expr_to_operand(cx, a)).collect();
            let arg_operands: Vec<Operand> =
                std::iter::once(Operand::Copy(Lvalue::local(recv_local, receiver.span)))
                    .chain(
                        arg_locals
                            .iter()
                            .map(|l| Operand::Copy(Lvalue::local(*l, Span::DUMMY))),
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
                    destination: Lvalue::local(dest, expr.span),
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
fn resolve_enum_variant(
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
        discr: Operand::Copy(Lvalue::local(lhs_local, lhs.span)),
        targets: vec![(ConstVal::Bool(true), true_target)],
        otherwise: false_target,
    });

    // short_circuit_block: result = (op == Or); goto cont
    //   For `&&`: lhs was false → result = false
    //   For `||`: lhs was true  → result = true
    cx.current_block = short_circuit_block;
    let short_val = matches!(op, HirBinOp::Or);
    cx.push_assign(
        Lvalue::local(result_local, span),
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
        discr: Operand::Copy(Lvalue::local(rhs_local, rhs.span)),
        targets: vec![(ConstVal::Bool(true), result_true_block)],
        otherwise: result_false_block,
    });

    // result_true_block: result = true; goto cont
    cx.current_block = result_true_block;
    cx.push_assign(
        Lvalue::local(result_local, span),
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
        Lvalue::local(result_local, span),
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
/// 2. Construct `Lvalue::Projection(local, Deref)`
/// 3. Assign `result = Use(Copy(projection))` to a fresh temp
///
/// The temp's type is left as a fresh inference variable — typeck will
/// unify it with the pointee type via `infer_projection(Deref)`.
fn lower_deref_expr(cx: &mut MirLowerCtxt, inner: &HirExpr, span: Span) -> LocalId {
    let inner_local = lower_expr_to_operand(cx, inner);
    let proj = Lvalue {
        kind: LvalueKind::Projection(
            Box::new(Lvalue::local(inner_local, inner.span)),
            ProjectionElem::Deref,
        ),
        span,
    };
    let result_ty = cx.fresh_infer_ty(span);
    cx.eval_rvalue_to_temp(Rvalue::Use(Operand::Copy(proj)), result_ty, span)
}

/// Extract the mutability from a pattern's BindingMode.
///
/// G5 fix (Stage 2.4e): For `let mut x = ...`, the pattern is
/// `HirPatKind::Ident(ByValue(Mutable), ...)`. This helper extracts
/// the `Mutable` and returns it as a MIR `Mutability`.
///
/// For non-Ident patterns (Wild, Tuple, Struct, etc.), returns Immutable
/// (the default — these patterns don't directly bind a single local).
fn pat_mutability(pat: &HirPat) -> crate::mir::ty::Mutability {
    use crate::ast::BindingMode;
    use crate::hir::HirPatKind;
    use crate::mir::ty::Mutability;
    match &pat.kind {
        HirPatKind::Ident(
            BindingMode::ByValue(ast::Mutability::Mutable)
            | BindingMode::ByRef(ast::Mutability::Mutable),
            _,
            _,
        ) => Mutability::Mutable,
        _ => Mutability::Immutable,
    }
}

/// Lower a HIR block to MIR. Processes statements in order and
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
                    let mutability = pat_mutability(&local.pat);
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
                        Lvalue::local(local_id, local.span),
                        Rvalue::Use(Operand::Move(Lvalue::local(init_local, init.span))),
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
                    let mutability = pat_mutability(&local.pat);
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
        discr: Operand::Copy(Lvalue::local(cond_local, cond.span)),
        targets: vec![(ConstVal::Bool(true), then_block)],
        otherwise: else_block,
    });

    // Then block
    cx.current_block = then_block;
    let then_result = lower_block(cx, then);
    cx.push_assign(
        Lvalue::local(result_local, span),
        Rvalue::Use(Operand::Copy(Lvalue::local(then_result, then.span))),
        then.span,
    );
    cx.terminate(Terminator::Goto(cont_block));

    // Else block
    cx.current_block = else_block;
    if let Some(else_expr) = else_ {
        let else_result = lower_expr_to_operand(cx, else_expr);
        cx.push_assign(
            Lvalue::local(result_local, span),
            Rvalue::Use(Operand::Copy(Lvalue::local(else_result, else_expr.span))),
            else_expr.span,
        );
    } else {
        // No else → unit
        cx.push_assign(
            Lvalue::local(result_local, span),
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

        // Non-literal patterns (Wild, Ident, etc.) → go to otherwise
        // For Stage 2.1b, we treat them all as "otherwise"
        // The first non-literal arm becomes the otherwise target
        if targets.is_empty() {
            // If no literal targets yet, this arm is the otherwise
            // We'll set it as the otherwise block
        }
    }

    // Terminate current block with SwitchInt
    cx.terminate(Terminator::SwitchInt {
        discr: Operand::Copy(Lvalue::local(scrut_local, scrutinee.span)),
        targets: targets.clone(),
        otherwise: otherwise_block,
    });

    // Lower each arm body
    for (i, arm) in arms.iter().enumerate() {
        let arm_block = arm_blocks[i];
        cx.current_block = arm_block;

        // Collect pattern bindings (for Ident patterns)
        collect_pat_bindings_for_mir(cx, &arm.pat);

        // Lower the arm body
        let arm_result = lower_expr_to_operand(cx, &arm.body);
        cx.push_assign(
            Lvalue::local(result_local, span),
            Rvalue::Use(Operand::Copy(Lvalue::local(arm_result, arm.body.span))),
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
            collect_pat_bindings_for_mir(cx, &arm.pat);
            let arm_result = lower_expr_to_operand(cx, &arm.body);
            cx.push_assign(
                Lvalue::local(result_local, span),
                Rvalue::Use(Operand::Copy(Lvalue::local(arm_result, arm.body.span))),
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

/// Collect pattern bindings into the MIR local map.
fn collect_pat_bindings_for_mir(cx: &mut MirLowerCtxt, pat: &HirPat) {
    match &pat.kind {
        HirPatKind::Ident(_mode, _ident, sub) => {
            // Allocate a local for this binding
            let ty = cx.fresh_infer_ty(pat.span);
            cx.new_local(pat.hir_id, ty, None);
            if let Some(s) = sub {
                collect_pat_bindings_for_mir(cx, s);
            }
        }
        HirPatKind::TupleStruct(_, pats) => {
            for p in pats {
                collect_pat_bindings_for_mir(cx, p);
            }
        }
        HirPatKind::Tuple(pats) => {
            for p in pats {
                collect_pat_bindings_for_mir(cx, p);
            }
        }
        HirPatKind::Struct(_, fields, _) => {
            for f in fields {
                collect_pat_bindings_for_mir(cx, &f.pat);
            }
        }
        _ => {}
    }
}
