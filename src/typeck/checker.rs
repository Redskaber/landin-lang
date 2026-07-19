//! Type checker: walks MIR bodies, collects constraints, unifies types.
//!
//! This module implements the full type checking pass:
//! 1. Walk each MIR body's basic blocks in order
//! 2. For each `Statement::Assign(place, rvalue)`, infer the rvalue's type
//!    and unify it with the place's declared type
//! 3. Check terminator constraints (Call args, SwitchInt discr type)
//! 4. Default unresolved int/float variables to i32/f64
//! 5. Resolve and report any type errors

use crate::ast;
use crate::hir::HirCrate;
use crate::mir::body::*;
use crate::mir::lvalue::*;
use crate::mir::ty::*;
use crate::session::Span;
use crate::typeck::error::TypeError;
use crate::typeck::unify::UnificationTable;
use lasso::Rodeo;

/// Per-body type checking results.
///
/// After running `TypeChecker::check_mir_body`, this struct holds:
/// - The resolved type of each local (keyed by LocalId)
/// - The resolved type of each HirId (keyed by HirId, for HIR writeback)
///
/// Stage 2.4d (P1-3): The driver collects these results so downstream
/// consumers (codegen, error display) can consult the resolved types
/// instead of re-running type inference.
#[derive(Debug, Default, Clone)]
pub struct TypeckResults {
    /// Map from LocalId → resolved Ty.
    pub local_types: std::collections::HashMap<LocalId, Ty>,
    /// Map from HirId → resolved Ty (for HIR nodes that have a type).
    /// Populated for local variable bindings; other HIR nodes are Stage 3+.
    pub hir_types: std::collections::HashMap<crate::hir::HirId, Ty>,
}

impl TypeckResults {
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up the resolved type of a local.
    pub fn local_type(&self, id: LocalId) -> Option<&Ty> {
        self.local_types.get(&id)
    }

    /// Look up the resolved type of a HIR node.
    pub fn hir_type(&self, id: crate::hir::HirId) -> Option<&Ty> {
        self.hir_types.get(&id)
    }
}

/// The type checker. Holds the unification table and collects errors.
pub struct TypeChecker {
    pub unify: UnificationTable,
    pub errors: Vec<TypeError>,
    /// Per-body results, populated during check_mir_body.
    pub results: TypeckResults,
    /// Map from HirId → LocalId (built during MIR lowering, used to
    /// write resolved types back to HIR nodes).
    /// Note: this is currently empty because MIR lower doesn't expose
    /// its local_map. Stage 3 will wire this up properly.
    hir_to_local: std::collections::HashMap<crate::hir::HirId, LocalId>,
    /// G3 fix (Stage 2.4e): Map from DefId → fn signature.
    /// Populated by `populate_fn_sigs` from the HIR crate before
    /// type-checking bodies. Used by Call type checking to verify
    /// arg count and arg/return types.
    fn_sigs: std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            unify: UnificationTable::new(),
            errors: Vec::new(),
            results: TypeckResults::new(),
            hir_to_local: std::collections::HashMap::new(),
            fn_sigs: std::collections::HashMap::new(),
        }
    }

    /// Construct a TypeChecker with a pre-populated unify table.
    ///
    /// This is used when MIR lowering has already allocated IntVar/FloatVar
    /// for unsuffixed integer/float literals. By passing the lowering's
    /// unify table, the type checker can resolve those variables during
    /// `default_unresolved` (defaulting unresolved int vars to i32, etc).
    pub fn with_unify(unify: UnificationTable) -> Self {
        Self {
            unify,
            errors: Vec::new(),
            results: TypeckResults::new(),
            hir_to_local: std::collections::HashMap::new(),
            fn_sigs: std::collections::HashMap::new(),
        }
    }

    /// G3 fix: Populate the fn_sigs map from a HIR crate.
    ///
    /// Walks all owner nodes; for each `HirItem::Fn`, extracts the fn
    /// signature (param types + return type) and stores it keyed by
    /// the fn's DefId. The Call type checker then looks up the sig
    /// to verify arg count and types.
    pub fn populate_fn_sigs(&mut self, hir: &HirCrate) {
        use crate::hir::{HirFnRetTy, HirItem, OwnerNode};
        for (def_id, owner) in &hir.owners {
            if let OwnerNode::Item(HirItem::Fn(f)) = owner {
                let inputs: Vec<Ty> = f
                    .sig
                    .inputs
                    .iter()
                    .map(|p| {
                        p.ty.as_ref()
                            .map(crate::mir::lower::lower_hir_ty_to_mir_ty)
                            .unwrap_or_else(|| Ty::new(TyKind::Error, p.span))
                    })
                    .collect();
                let output = match &f.sig.output {
                    HirFnRetTy::Ty(t) => crate::mir::lower::lower_hir_ty_to_mir_ty(t),
                    HirFnRetTy::Default(_) => Ty::new(TyKind::Tuple(vec![]), f.span),
                };
                self.fn_sigs.insert(
                    *def_id,
                    crate::mir::ty::Sig {
                        inputs,
                        output: Box::new(output),
                        abi: f.sig.abi,
                        is_unsafe: f.sig.is_unsafe,
                    },
                );
            }
        }
    }

    /// Register a HirId → LocalId mapping. Called by the driver after
    /// MIR lowering (which produces the local_map) so that typeck can
    /// write resolved types back to HIR nodes via HirId lookup.
    pub fn register_hir_to_local(&mut self, hir_id: crate::hir::HirId, local_id: LocalId) {
        self.hir_to_local.insert(hir_id, local_id);
    }

    /// Check a single MIR body. Walks all basic blocks, infers types
    /// for rvalues, and unifies with lvalue types.
    ///
    /// After inference, writes the resolved types back into
    /// `mir.local_decls[i].ty` so that downstream consumers (borrowck,
    /// codegen) see concrete types instead of inference variables.
    pub fn check_mir_body(&mut self, mir: &mut MirBody) {
        // Phase 1: Walk basic blocks in order, collecting constraints.
        let bb_count = mir.basic_blocks.len();
        for bb_id in 0..bb_count {
            let bb_id = BasicBlockId(bb_id as u32);
            // Snapshot the statements (clones) so we can mutate `mir`
            // while iterating. Statements are small (an Assign is two
            // boxes + a span); cloning is cheap enough.
            let statements: Vec<Statement> = mir.block(bb_id).statements.to_vec();
            for stmt in &statements {
                self.check_statement(mir, stmt);
            }
            let terminator = mir.block(bb_id).terminator.clone();
            self.check_terminator(mir, &terminator);
        }

        // Phase 2: Default unresolved int/float variables.
        self.unify.default_unresolved();

        // Phase 3: Write resolved types back into local_decls.
        for local in mir.local_decls.iter_mut() {
            local.ty = self.unify.resolve(&local.ty);
        }

        // Phase 4: Populate TypeckResults for downstream consumers.
        // Walk all locals and record their resolved types.
        for (idx, local) in mir.local_decls.iter().enumerate() {
            self.results
                .local_types
                .insert(LocalId(idx as u32), local.ty.clone());
        }
        // Walk the hir_to_local map and record hir_id → resolved Ty.
        // This lets HIR consumers see the resolved type without re-running
        // typeck.
        for (hir_id, local_id) in &self.hir_to_local {
            if let Some(ty) = mir.local_decls.get(local_id.0 as usize) {
                self.results.hir_types.insert(*hir_id, ty.ty.clone());
            }
        }

        // Phase 5 (G7 fix, Stage 2.4f): Post-defaulting Call check.
        // After default_unresolved + writeback, re-scan Call terminators
        // to catch "call non-function" errors that couldn't be detected
        // in Phase 1 (when func_ty was still an unresolved Infer var).
        // Example: `let x = 1; x();` — x resolves to Int(I32) after
        // defaulting, so the Call check can now reject it.
        for bb in &mir.basic_blocks {
            let term = bb.terminator.clone();
            self.post_check_terminator(mir, &term);
        }
    }

    /// Post-defaulting terminator check. Runs after Phase 3 (writeback)
    /// so all types are resolved. Catches errors that depend on
    /// defaulting (e.g., `let x = 1; x();` where x defaults to i32).
    fn post_check_terminator(&mut self, mir: &MirBody, term: &Terminator) {
        if let Terminator::Call { func, .. } = term {
            let func_ty = self.unify.resolve(&self.infer_operand(mir, func));
            // G7 fix: if func is neither FnDef nor FnPtr (after defaulting),
            // emit an error. Infer should be resolved by now; if it's still
            // Infer, it means no constraint was applied (rare).
            if !matches!(
                &func_ty.kind,
                TyKind::FnDef(_, _) | TyKind::FnPtr(_) | TyKind::Error
            ) {
                self.errors.push(TypeError::new(
                    format!("expected function, found {:?}", func_ty.kind),
                    Span::DUMMY,
                ));
            }
        }
    }

    /// Check a single MIR statement (Assign or Nop).
    fn check_statement(&mut self, mir: &MirBody, stmt: &Statement) {
        match &stmt.kind {
            StatementKind::Assign(boxed) => {
                let (place, rvalue) = &**boxed;
                let place_ty = self.infer_lvalue(mir, place);
                let rvalue_ty = self.infer_rvalue(mir, rvalue);
                if let Err(e) = self.unify.unify(&place_ty, &rvalue_ty) {
                    self.errors.push(*e);
                }
                // The resolved type will be written back to local_decls
                // in Phase 3 of check_mir_body (after all constraints
                // are collected and defaults are applied).
            }
            // StorageLive/StorageDead/Deinit are scope/lifetime markers.
            // They don't introduce type constraints.
            StatementKind::Nop
            | StatementKind::StorageLive(_)
            | StatementKind::StorageDead(_)
            | StatementKind::Deinit(_) => {}
        }
    }

    /// Check a terminator's type constraints.
    fn check_terminator(&mut self, mir: &MirBody, term: &Terminator) {
        match term {
            Terminator::Call {
                func,
                args,
                destination,
                ..
            } => {
                // Infer func type
                let func_ty = self.unify.resolve(&self.infer_operand(mir, func));
                // Infer arg types and collect them
                let arg_tys: Vec<Ty> = args
                    .iter()
                    .map(|arg| self.unify.resolve(&self.infer_operand(mir, arg)))
                    .collect();
                // Infer destination type
                let dest_ty = self.unify.resolve(&self.infer_lvalue(mir, destination));

                // G3 fix (Stage 2.4e): If func is a FnDef(def_id, _),
                // look up the fn signature from fn_sigs and verify:
                //   1. arg count matches
                //   2. each arg type unifies with the corresponding input
                //   3. destination type unifies with the return type
                if let TyKind::FnDef(def_id, _) = &func_ty.kind {
                    if let Some(sig) = self.fn_sigs.get(def_id).cloned() {
                        if arg_tys.len() != sig.inputs.len() {
                            self.errors.push(TypeError::new(
                                format!(
                                    "this function takes {} argument(s) but {} were supplied",
                                    sig.inputs.len(),
                                    arg_tys.len()
                                ),
                                Span::DUMMY,
                            ));
                        } else {
                            for (arg_ty, input_ty) in arg_tys.iter().zip(sig.inputs.iter()) {
                                if let Err(e) = self.unify.unify(arg_ty, input_ty) {
                                    self.errors.push(*e);
                                }
                            }
                        }
                        if let Err(e) = self.unify.unify(&dest_ty, &sig.output) {
                            self.errors.push(*e);
                        }
                    }
                    // If fn_sigs doesn't have the DefId (e.g., external fn),
                    // skip type checking — codegen will handle it.
                }

                // If func is a FnPtr, unify args with inputs and dest with output.
                if let TyKind::FnPtr(sig) = &func_ty.kind {
                    // Unify each arg with the corresponding input
                    for (arg_ty, input_ty) in arg_tys.iter().zip(sig.inputs.iter()) {
                        if let Err(e) = self.unify.unify(arg_ty, input_ty) {
                            self.errors.push(*e);
                        }
                    }
                    // Unify destination with output
                    if let Err(e) = self.unify.unify(&dest_ty, &sig.output) {
                        self.errors.push(*e);
                    }
                }

                // G7 fix (Stage 2.4f): if func is neither FnDef nor FnPtr
                // (e.g., calling an Int, Bool, Str, Tuple), emit an error.
                // Infer and Error are deferred (might resolve to a fn type).
                if !matches!(
                    &func_ty.kind,
                    TyKind::FnDef(_, _) | TyKind::FnPtr(_) | TyKind::Infer(_) | TyKind::Error
                ) {
                    self.errors.push(TypeError::new(
                        format!("expected function, found {:?}", func_ty.kind),
                        Span::DUMMY,
                    ));
                }
            }
            Terminator::SwitchInt { discr, targets, .. } => {
                // The discriminant must be an integer or bool
                let discr_ty = self.infer_operand(mir, discr);
                // G7 fix (Stage 2.4f): if any target is ConstVal::Bool(_),
                // this SwitchInt came from an `if` or `while` condition,
                // and the discriminant must be bool (not just any int).
                let requires_bool = targets
                    .iter()
                    .any(|(val, _)| matches!(val, ConstVal::Bool(_)));
                if requires_bool {
                    let bool_ty = Ty::new(TyKind::Bool, Span::DUMMY);
                    if let Err(e) = self.unify.unify(&discr_ty, &bool_ty) {
                        self.errors.push(*e);
                    }
                } else {
                    // Match on integer — check that it's int-like.
                    match &discr_ty.kind {
                        TyKind::Int(_) | TyKind::Uint(_) | TyKind::Bool => {}
                        TyKind::Infer(InferVar::IntVar(_)) => {}
                        TyKind::Infer(InferVar::TyVar(_)) => {
                            // Unbound variable — unify with i32 as default
                            let i32_ty = Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY);
                            let _ = self.unify.unify(&discr_ty, &i32_ty);
                        }
                        TyKind::Error => {}
                        _ => {
                            self.errors.push(TypeError::new(
                                format!(
                                    "expected integer or bool for switch, found {:?}",
                                    discr_ty.kind
                                ),
                                Span::DUMMY,
                            ));
                        }
                    }
                }
            }
            Terminator::Drop { place, .. } => {
                // Just infer the place type (no constraint to check)
                let _ = self.infer_lvalue(mir, place);
            }
            Terminator::Assert { cond, .. } => {
                // The condition must be a bool. We don't enforce this
                // strictly (codegen will handle the runtime check) but
                // we do infer the type to catch obvious mismatches.
                let cond_ty = self.infer_operand(mir, cond);
                match &cond_ty.kind {
                    TyKind::Bool | TyKind::Infer(_) | TyKind::Error => {}
                    _ => {
                        self.errors.push(TypeError::new(
                            format!("assert condition must be bool, found {:?}", cond_ty.kind),
                            Span::DUMMY,
                        ));
                    }
                }
            }
            _ => {}
        }
    }

    /// Infer the type of an lvalue (place expression).
    fn infer_lvalue(&self, mir: &MirBody, lv: &Lvalue) -> Ty {
        match &lv.kind {
            LvalueKind::Local(id) => {
                if (id.0 as usize) < mir.local_decls.len() {
                    mir.local(*id).ty.clone()
                } else {
                    Ty::new(TyKind::Error, lv.span)
                }
            }
            LvalueKind::Static(_) => {
                // Static type would come from the HIR; for now, Error
                Ty::new(TyKind::Error, lv.span)
            }
            LvalueKind::Projection(base, elem) => {
                let base_ty = self.infer_lvalue(mir, base);
                self.infer_projection(&base_ty, elem)
            }
        }
    }

    /// Infer the type after applying a projection element.
    fn infer_projection(&self, base_ty: &Ty, elem: &ProjectionElem) -> Ty {
        match elem {
            ProjectionElem::Deref => {
                if let TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) = &base_ty.kind {
                    (**inner).clone()
                } else {
                    Ty::new(TyKind::Error, Span::DUMMY)
                }
            }
            ProjectionElem::Field(_, field_ty) => field_ty.clone(),
            ProjectionElem::Index(_) => {
                if let TyKind::Array(inner, _) | TyKind::Slice(inner) = &base_ty.kind {
                    (**inner).clone()
                } else {
                    Ty::new(TyKind::Error, Span::DUMMY)
                }
            }
            ProjectionElem::ConstantIndex { .. } | ProjectionElem::Subslice { .. } => {
                if let TyKind::Array(inner, _) | TyKind::Slice(inner) = &base_ty.kind {
                    (**inner).clone()
                } else {
                    Ty::new(TyKind::Error, Span::DUMMY)
                }
            }
        }
    }

    /// Infer the type of an rvalue.
    fn infer_rvalue(&mut self, mir: &MirBody, rv: &Rvalue) -> Ty {
        match rv {
            Rvalue::Use(operand) => self.infer_operand(mir, operand),
            Rvalue::BinaryOp(op, a, b) => {
                let a_ty = self.infer_operand(mir, a);
                let b_ty = self.infer_operand(mir, b);
                // Unify lhs and rhs types (they must match for arithmetic)
                match op {
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                        // Comparison: unify a and b, return bool
                        if let Err(e) = self.unify.unify(&a_ty, &b_ty) {
                            self.errors.push(*e);
                        }
                        Ty::new(TyKind::Bool, Span::DUMMY)
                    }
                    // Bitwise ops: Bool or integer types only.
                    BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
                        if let Err(e) = self.unify.unify(&a_ty, &b_ty) {
                            self.errors.push(*e);
                        }
                        // Result type matches operand type (Bool or Int).
                        a_ty
                    }
                    // Shifts: lhs can be int, rhs must be int (not bool).
                    BinOp::Shl | BinOp::Shr => {
                        if !is_shift_count_ty(&b_ty) {
                            self.errors.push(TypeError::new(
                                format!(
                                    "shift count must be an integer type, found {:?}",
                                    b_ty.kind
                                ),
                                Span::DUMMY,
                            ));
                        }
                        a_ty
                    }
                    // Arithmetic: lhs and rhs must be Int/Uint/Float.
                    // G7 fix (Stage 2.4f): reject Bool, Str, Tuple, etc.
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                        if !is_arithmetic_ty(&a_ty) {
                            self.errors.push(TypeError::new(
                                format!(
                                    "cannot apply arithmetic to {:?} (expected integer or float)",
                                    a_ty.kind
                                ),
                                Span::DUMMY,
                            ));
                        }
                        if !is_arithmetic_ty(&b_ty) {
                            self.errors.push(TypeError::new(
                                format!(
                                    "cannot apply arithmetic to {:?} (expected integer or float)",
                                    b_ty.kind
                                ),
                                Span::DUMMY,
                            ));
                        }
                        if let Err(e) = self.unify.unify(&a_ty, &b_ty) {
                            self.errors.push(*e);
                        }
                        a_ty
                    }
                }
            }
            Rvalue::BinaryOp2(_, a, _) => {
                let _a_ty = self.infer_operand(mir, a);
                Ty::new(TyKind::Error, Span::DUMMY) // Range type (Stage 3)
            }
            Rvalue::UnaryOp(op, operand) => {
                let inner_ty = self.infer_operand(mir, operand);
                match op {
                    UnOp::Not => {
                        // !bool → bool, !int → int
                        // G7 fix: !str, !float, !tuple are errors.
                        if !is_notable_ty(&inner_ty) {
                            self.errors.push(TypeError::new(
                                format!(
                                    "cannot apply `!` to {:?} (expected bool or integer)",
                                    inner_ty.kind
                                ),
                                Span::DUMMY,
                            ));
                        }
                        inner_ty
                    }
                    UnOp::Neg => {
                        // -int → int, -float → float
                        // G7 fix: -bool, -str, -tuple are errors.
                        if !is_negatable_ty(&inner_ty) {
                            self.errors.push(TypeError::new(
                                format!(
                                    "cannot apply unary `-` to {:?} (expected integer or float)",
                                    inner_ty.kind
                                ),
                                Span::DUMMY,
                            ));
                        }
                        inner_ty
                    }
                }
            }
            Rvalue::Ref(_, borrow_kind, lv) => {
                let inner_ty = self.infer_lvalue(mir, lv);
                let mutability = match borrow_kind {
                    BorrowKind::Shared => Mutability::Immutable,
                    BorrowKind::Mut => Mutability::Mutable,
                    BorrowKind::Raw => Mutability::Immutable,
                };
                Ty::new(
                    TyKind::Ref(Region::Erased, mutability, Box::new(inner_ty)),
                    Span::DUMMY,
                )
            }
            Rvalue::Cast(_, _, target_ty) => target_ty.clone(),
            Rvalue::Aggregate(kind, operands) => match kind {
                AggregateKind::Tuple => {
                    let elem_tys: Vec<Ty> = operands
                        .iter()
                        .map(|o| self.infer_operand(mir, o))
                        .collect();
                    Ty::new(TyKind::Tuple(elem_tys), Span::DUMMY)
                }
                AggregateKind::Array(elem_ty) => {
                    // G7 fix (Stage 2.4f): unify each element's type with
                    // the array's declared element type. This catches
                    // `[1, true]` (Int + Bool mismatch).
                    for op in operands {
                        let op_ty = self.infer_operand(mir, op);
                        if let Err(e) = self.unify.unify(&op_ty, elem_ty) {
                            self.errors.push(*e);
                        }
                    }
                    Ty::new(
                        TyKind::Array(
                            Box::new(elem_ty.clone()),
                            Box::new(Const {
                                ty: Box::new(Ty::new(
                                    TyKind::Uint(ast::UintTy::Usize),
                                    Span::DUMMY,
                                )),
                                val: ConstVal::Uint(operands.len() as u128),
                            }),
                        ),
                        Span::DUMMY,
                    )
                }
                _ => Ty::new(TyKind::Error, Span::DUMMY),
            },
        }
    }

    /// Infer the type of an operand.
    fn infer_operand(&self, mir: &MirBody, op: &Operand) -> Ty {
        match op {
            Operand::Copy(lv) | Operand::Move(lv) => self.infer_lvalue(mir, lv),
            Operand::Constant(c) => c.ty.as_ref().clone(),
        }
    }

    /// Consume the type checker and return all errors.
    pub fn into_errors(mut self) -> Vec<TypeError> {
        let mut errors = self.errors;
        errors.extend(self.unify.take_errors());
        errors
    }

    /// Consume the type checker and return (errors, results).
    pub fn into_results(mut self) -> (Vec<TypeError>, TypeckResults) {
        let mut errors = self.errors;
        errors.extend(self.unify.take_errors());
        (errors, std::mem::take(&mut self.results))
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether a type can be used in arithmetic ops (Add/Sub/Mul/Div/Rem).
///
/// G7 fix (Stage 2.4f): Bool, Str, Tuple, Array, etc. are NOT arithmetic.
/// Int, Uint, Float, and Infer (deferred) are.
fn is_arithmetic_ty(ty: &Ty) -> bool {
    matches!(
        &ty.kind,
        TyKind::Int(_) | TyKind::Uint(_) | TyKind::Float(_) | TyKind::Infer(_) | TyKind::Error
    )
}

/// Whether a type can be negated with unary `-`.
///
/// Int, Uint, Float, Infer, Error are negatable. Bool, Str, Tuple are not.
fn is_negatable_ty(ty: &Ty) -> bool {
    is_arithmetic_ty(ty)
}

/// Whether a type can be used with `!` (bitwise NOT).
///
/// Bool, Int, Uint, Infer, Error are notable. Float, Str, Tuple are not.
fn is_notable_ty(ty: &Ty) -> bool {
    matches!(
        &ty.kind,
        TyKind::Bool | TyKind::Int(_) | TyKind::Uint(_) | TyKind::Infer(_) | TyKind::Error
    )
}

/// Whether a type can be used as a shift count (rhs of Shl/Shr).
///
/// Int, Uint, Infer, Error are valid. Bool, Float, Str are not.
fn is_shift_count_ty(ty: &Ty) -> bool {
    matches!(
        &ty.kind,
        TyKind::Int(_) | TyKind::Uint(_) | TyKind::Infer(_) | TyKind::Error
    )
}

/// Check a single MIR body for type errors.
/// Returns a list of type errors (non-fatal; the MIR is still usable).
///
/// As a side effect, writes resolved types back into `mir.local_decls[i].ty`.
pub fn check_mir_body(mir: &mut MirBody) -> Vec<TypeError> {
    let mut tc = TypeChecker::new();
    tc.check_mir_body(mir);
    tc.into_errors()
}

/// Check all MIR bodies in a HIR crate for type errors.
///
/// This is the main entry point for the type checking pass. It:
/// 1. Lowers each HIR body to MIR (if not already done)
/// 2. Runs the type checker on each MIR body
/// 3. Collects all type errors
///
/// Returns a list of all type errors found across all bodies.
pub fn check_crate(hir: &HirCrate, interner: &Rodeo) -> Vec<TypeError> {
    let mut all_errors = Vec::new();
    for (_, body) in &hir.bodies {
        let mut mir = crate::mir::lower::lower_hir_body_to_mir(body, interner);
        let mut tc = TypeChecker::new();
        tc.check_mir_body(&mut mir);
        all_errors.extend(tc.into_errors());
    }
    all_errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;

    fn make_mir_with_return_i32() -> MirBody {
        let mut mir = MirBody::new(Span::DUMMY);
        let _bb0 = mir.new_block(); // create entry block
        let return_local = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let temp = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(temp, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Box::new(Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)),
                    val: ConstVal::Int(42),
                })),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(return_local, Span::DUMMY),
                Rvalue::Use(Operand::Copy(Lvalue::local(temp, Span::DUMMY))),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::Return;
        mir
    }

    #[test]
    fn check_valid_i32_assignment() {
        let mut mir = make_mir_with_return_i32();
        let errors = check_mir_body(&mut mir);
        assert!(
            errors.is_empty(),
            "expected no type errors, got {:?}",
            errors
        );
    }

    #[test]
    fn check_type_mismatch_detected() {
        let mut mir = MirBody::new(Span::DUMMY);
        let _ = mir.new_block(); // entry block
        let dest = mir.new_local(Ty::new(TyKind::Bool, Span::DUMMY), None, Span::DUMMY);
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(dest, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Box::new(Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)),
                    val: ConstVal::Int(42),
                })),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::Return;
        let errors = check_mir_body(&mut mir);
        assert!(!errors.is_empty(), "expected type mismatch error");
    }

    #[test]
    fn check_comparison_returns_bool() {
        let mut mir = MirBody::new(Span::DUMMY);
        let _ = mir.new_block(); // entry block
        let result = mir.new_local(Ty::new(TyKind::Bool, Span::DUMMY), None, Span::DUMMY);
        let a = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let b = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(result, Span::DUMMY),
                Rvalue::BinaryOp(
                    BinOp::Eq,
                    Operand::Copy(Lvalue::local(a, Span::DUMMY)),
                    Operand::Copy(Lvalue::local(b, Span::DUMMY)),
                ),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::Return;
        let errors = check_mir_body(&mut mir);
        assert!(
            errors.is_empty(),
            "expected no errors for comparison, got {:?}",
            errors
        );
    }

    #[test]
    fn check_ref_type_inference() {
        let mut mir = MirBody::new(Span::DUMMY);
        let _ = mir.new_block(); // entry block
        let dest = mir.new_local(
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
        let src = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(dest, Span::DUMMY),
                Rvalue::Ref(
                    Region::Erased,
                    BorrowKind::Shared,
                    Lvalue::local(src, Span::DUMMY),
                ),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::Return;
        let errors = check_mir_body(&mut mir);
        assert!(
            errors.is_empty(),
            "expected no errors for ref, got {:?}",
            errors
        );
    }

    #[test]
    fn check_tuple_aggregate() {
        let mut mir = MirBody::new(Span::DUMMY);
        let _ = mir.new_block(); // entry block
        let dest = mir.new_local(
            Ty::new(
                TyKind::Tuple(vec![
                    Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
                    Ty::new(TyKind::Bool, Span::DUMMY),
                ]),
                Span::DUMMY,
            ),
            None,
            Span::DUMMY,
        );
        let a = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let b = mir.new_local(Ty::new(TyKind::Bool, Span::DUMMY), None, Span::DUMMY);
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Lvalue::local(dest, Span::DUMMY),
                Rvalue::Aggregate(
                    AggregateKind::Tuple,
                    vec![
                        Operand::Copy(Lvalue::local(a, Span::DUMMY)),
                        Operand::Copy(Lvalue::local(b, Span::DUMMY)),
                    ],
                ),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::Return;
        let errors = check_mir_body(&mut mir);
        assert!(
            errors.is_empty(),
            "expected no errors for tuple, got {:?}",
            errors
        );
    }

    #[test]
    fn check_switch_int_valid() {
        let mut mir = MirBody::new(Span::DUMMY);
        let _ = mir.new_block(); // entry block
        let discr = mir.new_local(
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        let bb1 = mir.new_block();
        let bb2 = mir.new_block();
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::SwitchInt {
            discr: Operand::Copy(Lvalue::local(discr, Span::DUMMY)),
            targets: vec![(ConstVal::Int(1), bb1)],
            otherwise: bb2,
        };
        mir.block_mut(bb1).terminator = Terminator::Return;
        mir.block_mut(bb2).terminator = Terminator::Return;
        let errors = check_mir_body(&mut mir);
        assert!(
            errors.is_empty(),
            "expected no errors for switch, got {:?}",
            errors
        );
    }

    #[test]
    fn check_switch_int_invalid_type() {
        let mut mir = MirBody::new(Span::DUMMY);
        let _ = mir.new_block(); // entry block
        let discr = mir.new_local(
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
        let bb1 = mir.new_block();
        let bb2 = mir.new_block();
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::SwitchInt {
            discr: Operand::Copy(Lvalue::local(discr, Span::DUMMY)),
            targets: vec![(ConstVal::Int(1), bb1)],
            otherwise: bb2,
        };
        mir.block_mut(bb1).terminator = Terminator::Return;
        mir.block_mut(bb2).terminator = Terminator::Return;
        let errors = check_mir_body(&mut mir);
        assert!(!errors.is_empty(), "expected error for switch on ref type");
    }
}
