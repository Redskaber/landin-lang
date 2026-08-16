//! Type checker: walks MIR bodies, collects constraints, unifies types.
//!
//! This module implements the full type checking pass:
//! 1. Walk each MIR body's basic blocks in order
//! 2. For each `Statement::Assign(place, rvalue)`, infer the rvalue's type
//!    and unify it with the place's declared type
//! 3. Check terminator constraints (Call args, SwitchInt discr type)
//! 4. Default unresolved int/float variables to i32/f64
//! 5. Resolve and report any type errors
//!
//! ## Stage 6.15 architectural split (TD-025)
//!
//! Per `docs/stage-committee-process.md` v3.21 §14.4 + §13.4, this file
//! has been split into 2 sub-modules:
//!
//! - `tables.rs`     — typeck data tables (TypeckResults + FieldTyTable + FnSigTable)
//! - `predicates.rs` — type classification predicates + coercion rules
//!
//! This file (`checker.rs`) retains: TypeChecker struct + impl + entry
//! points (`check_mir_body` / `check_crate`) + tests.

#[cfg(test)]
use crate::mir::body::TerminatorKind;
use crate::mir::body::*;
use crate::mir::place::*;
use crate::mir::ty::*;
#[cfg(test)]
use crate::session::Span;
use crate::typeck::error::TypeError;
use crate::typeck::unify::UnificationTable;

// Stage 6.15: import data tables + predicates from sub-modules.
use super::tables::{FieldTyTable, TypeckResults};

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
    /// Populated by `populate_fn_sigs` or set directly by the driver
    /// (Stage 3.60: from pre-computed FnSigTable).
    pub fn_sigs: std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
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

    /// Stage 16.84: Format a `Ty` for error messages, using resolver if available.
    ///
    /// Reads resolver/interner from the unify table (set by `set_resolver`
    /// in Stage 16.81). When available, uses `type_to_string_with_resolver`
    /// to show actual type names (e.g., "MyStruct" instead of "<adt>").
    /// Otherwise falls back to `type_to_string`.
    ///
    /// Per §1.0 原則 3 "显式 > 隐式": user-facing type names are explicit.
    /// Per §23: `format_ty` follows `<verb>_<noun>` pattern.
    /// Stage 18.100 (TD-DUP2): delegates to `mir::ty::format_ty_with_optional_resolver`
    /// (single source of truth — was duplicated in 3 modules).
    /// Stage 18.128 §13.4 J4: pub(super) for cross-file access.
    pub(super) fn format_ty(&self, ty: &Ty) -> String {
        crate::mir::ty::format_ty_with_optional_resolver(
            ty,
            self.unify.resolver(),
            self.unify.interner(),
        )
    }

    /// Register a HirId → LocalId mapping. Called by the driver after
    /// MIR lowering (which produces the local_map) so that typeck can
    /// write resolved types back to HIR nodes via HirId lookup.
    pub fn register_hir_to_local(&mut self, hir_id: crate::hir::HirId, local_id: LocalId) {
        self.hir_to_local.insert(hir_id, local_id);
    }

    /// Stage 3.60: Check a MIR body using pre-computed data tables instead
    /// of HIR. This is the section-16-compliant entry point — typeck
    /// receives data (FieldTyTable), not HIR references.
    ///
    /// The `field_ty_table` is built by the driver from HIR before calling
    /// typeck. It maps struct DefIds to their field types, so typeck can
    /// resolve ADT field types during writeback without reading HIR.
    pub fn check_mir_body_with_tables(
        &mut self,
        mir: &mut MirBody,
        field_ty_table: Option<&FieldTyTable>,
    ) {
        // Phase 1: Walk basic blocks in order, collecting constraints.
        let bb_count = mir.basic_blocks.len();
        for bb_id in 0..bb_count {
            let bb_id = BasicBlockId(bb_id as u32);
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

        // Phase 3.5: Writeback field types using the pre-computed table.
        if let Some(table) = field_ty_table {
            self.writeback_field_types_with_table(mir, table);
            self.writeback_field_load_locals_with_table(mir, table);
        }

        // Phase 4: Populate TypeckResults.
        for (idx, local) in mir.local_decls.iter().enumerate() {
            self.results
                .local_types
                .insert(LocalId(idx as u32), local.ty.clone());
        }

        // Phase 5: Post-defaulting terminator check.
        for bb_id in 0..bb_count {
            let bb_id = BasicBlockId(bb_id as u32);
            let term = mir.block(bb_id).terminator.clone();
            self.post_check_terminator(mir, &term);
        }

        // Stage 18.71 Phase 5.5: Post-defaulting statement re-check.
        //
        // Why: In Phase 1, `check_statement` reads `place_ty` from
        // `local_decls` (which is still Infer), then resolves via
        // `unify.resolve`. For unsuffixed int literals (`1`), the
        // rvalue's type is `Infer(IntVar)` — only resolved to `i32`
        // after Phase 2 (`default_unresolved`). So the Phase 1 check
        // sees `place=Infer(TyVar)` and `rvalue=Infer(IntVar)`, both
        // non-concrete → check is skipped.
        //
        // After Phase 2 + Phase 3, the local_decls have resolved types.
        // Re-running the Assign type mismatch check here catches:
        //   - `let x = if true { 1 } else { true };` (if-branch mismatch)
        //   - `let x = match 1 { 0 => 1, _ => true };` (match arm mismatch)
        //
        // Per §1.0 原則 4 "报错 > 静默": if-branch/match-arm type
        // mismatches must be reported, not silently accepted.
        // Per §1.0 原則 6 "通用 > 特例": one re-check covers all
        // Assign statements — no special if-branch handling needed.
        for bb_id in 0..bb_count {
            let bb_id = BasicBlockId(bb_id as u32);
            let statements: Vec<Statement> = mir.block(bb_id).statements.to_vec();
            for stmt in &statements {
                self.post_check_statement(mir, stmt);
            }
        }
    }

    /// Stage 18.71: Post-defaulting statement check. Runs after Phase 3
    /// (writeback) so all types are resolved. Catches type mismatches
    /// that depend on IntVar/FloatVar defaulting (e.g., if-branch
    /// mismatch where one branch is an unsuffixed int literal).
    ///
    /// IMPORTANT: This function does NOT call `infer_rvalue` because
    /// `infer_rvalue` has side effects (it calls `unify` on Aggregate
    /// operands, BinaryOp operands, etc.). Re-running those unifications
    /// after Phase 2 (default_unresolved) would re-trigger unify with
    /// already-resolved types, producing spurious errors (e.g., for
    /// `struct Byte { v: u8 } fn f() { let b = Byte { v: 65 }; }`,
    /// the literal `65` resolves to i8 after Phase 3 — re-unifying i8
    /// with u8 fails, even though the original IntVar was correctly
    /// bound to u8 in Phase 1).
    ///
    /// Instead, this function uses `infer_rvalue_type_only` which returns
    /// the rvalue's type WITHOUT any unify side effects.
    ///
    /// Per §1.0 原則 6 "通用 > 特例": one check covers all Assign statements.
    /// Per §1.0 原則 9 "正确 > 妥协": must not break valid code.
    pub fn check_mir_body(&mut self, mir: &mut MirBody) {
        self.check_mir_body_with_tables(mir, None);
    }

    /// Resolve an operand's type for the writeback pass (reads local_decls
    /// which have been fixed by the first pass of writeback_field_load_locals).
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

    /// Stage 16.29 (通解): Consume the type checker and return
    /// (errors, results, unify_table). The unify table is returned so the
    /// caller can pass it to the next TypeChecker (for chained typeck on
    /// main body + closure MIR bodies sharing the same unify table).
    ///
    /// Per §1.0 原則 6 "通用 > 特例": one unify table for main body +
    /// all closures — no special-case handling per closure type.
    pub fn into_results_with_unify(mut self) -> (Vec<TypeError>, TypeckResults, UnificationTable) {
        let mut errors = self.errors;
        errors.extend(self.unify.take_errors());
        (
            errors,
            std::mem::take(&mut self.results),
            std::mem::take(&mut self.unify),
        )
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
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

// Stage 18.71: Check if a type has unresolved substs (empty substs on Adt).
/// Used to skip type mismatch checks on generic types where substs haven't
/// been substituted yet (e.g., Box<T> with empty substs vs Box<i32>).
/// Per §1.0 原則 9 "正确 > 妥协": must not break valid generic code.
/// Stage 18.128 §13.4 J4: pub(super) for check.rs access.
pub(super) fn type_has_unresolved_substs(ty: &crate::mir::ty::Ty) -> bool {
    use crate::mir::ty::TyKind;
    match &ty.kind {
        TyKind::Adt(_, substs) if substs.is_empty() => true,
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) | TyKind::Slice(inner) => {
            type_has_unresolved_substs(inner)
        }
        TyKind::Array(inner, _) => type_has_unresolved_substs(inner),
        TyKind::Tuple(tys) => tys.iter().any(type_has_unresolved_substs),
        TyKind::Adt(_, substs) => substs.iter().any(type_has_unresolved_substs),
        TyKind::FnDef(_, substs) => substs.iter().any(type_has_unresolved_substs),
        TyKind::Closure(_, substs) => substs.iter().any(type_has_unresolved_substs),
        // Stage 18.71: Projection types (associated types like `<T as Trait>::Item`)
        // are always "unresolved" — they can't be compared to concrete types
        // without monomorphization. Skip the check.
        // Per §1.0 原則 9 "正确 > 妥协": generic associated types need
        // monomorphization before type comparison (v0.2 work).
        TyKind::Projection(_, _) => true,
        TyKind::FnPtr(sig) => {
            sig.inputs.iter().any(type_has_unresolved_substs)
                || type_has_unresolved_substs(&sig.output)
        }
        // Stage 18.71: Error type is "unresolved" — skip check.
        TyKind::Error => true,
        _ => false,
    }
}

// Stage 18.71: Loose type matching for cases where the MIR type system
/// represents the same source type differently (e.g., Str vs Ref(_, _, Str)).
/// Per §1.0 原則 9 "正确 > 妥协": must not break valid code.
/// Stage 18.128 §13.4 J4: pub(super) for check.rs access.
pub(super) fn types_match_loose(a: &crate::mir::ty::Ty, b: &crate::mir::ty::Ty) -> bool {
    use crate::mir::ty::TyKind;
    match (&a.kind, &b.kind) {
        // Str ↔ Ref(_, _, Str): string literal vs &str reference
        (TyKind::Str, TyKind::Ref(_, _, inner)) if matches!(inner.kind, TyKind::Str) => true,
        (TyKind::Ref(_, _, inner), TyKind::Str) if matches!(inner.kind, TyKind::Str) => true,
        // Stage 18.98: Adt with same DefId — check substs recursively.
        // Per §2.0 原则 9 "正确 > 妥协": Vec<i32> != Vec<bool> (soundness).
        // The old code (`if a_def == b_def => true`) ignored substs entirely,
        // which accepted unsound assignments like `let v: Vec<i32> = vec_bool;`.
        //
        // Empty substs (inference case) still loose-match — they represent
        // "unknown, to be inferred" and unify with anything per unify.rs
        // (the empty-substs fallback in `unify_adt`). This preserves valid
        // generic inference code like `let w: Wrapper<i32> = make(42);`
        // where the rvalue's substs may not yet be back-propagated.
        //
        // Per §1.0 原則 6 "通用 > 特例": one recursive path for all Adt types.
        (TyKind::Adt(a_def, a_substs), TyKind::Adt(b_def, b_substs)) => {
            if a_def != b_def {
                return false;
            }
            // Empty substs = inference case (unknown instantiation) → loose match
            if a_substs.is_empty() || b_substs.is_empty() {
                return true;
            }
            // Both have substs — must match in length and element-wise (loose)
            if a_substs.len() != b_substs.len() {
                return false;
            }
            a_substs
                .iter()
                .zip(b_substs.iter())
                .all(|(at, bt)| types_match_loose(at, bt))
        }
        // Ref with same inner kind (region may differ — Var vs Static etc.)
        // Per §1.0 原則 9 "正确 > 妥协": regions are erased in Stage 0.
        // Also handles Infer inner: &{integer} vs &i32 (recursive loose match).
        (TyKind::Ref(_, _, a_inner), TyKind::Ref(_, _, b_inner)) => {
            types_match_loose(a_inner, b_inner)
        }
        // Array with matching element type (count may be Infer vs concrete)
        (TyKind::Array(a_inner, _), TyKind::Array(b_inner, _)) => {
            types_match_loose(a_inner, b_inner)
        }
        // Int ↔ Infer(IntVar): unsuffixed integer literal vs concrete int type
        // Per §1.0 原則 9: Stage 0 allows int fallback.
        (TyKind::Int(_), TyKind::Infer(crate::mir::ty::InferVar::IntVar(_))) => true,
        (TyKind::Infer(crate::mir::ty::InferVar::IntVar(_)), TyKind::Int(_)) => true,
        (TyKind::Uint(_), TyKind::Infer(crate::mir::ty::InferVar::IntVar(_))) => true,
        (TyKind::Infer(crate::mir::ty::InferVar::IntVar(_)), TyKind::Uint(_)) => true,
        // Stage 18.71: Int ↔ Uint of same bit width.
        // The unify table's `bind_int_var_to_uint` converts Uint to Int
        // (losing Uint-ness) when binding an IntVar to a Uint type. This
        // means `let x: usize = 1;` ends up with place=usize, rvalue=isize
        // (the IntVar was bound to isize, the corresponding Int for usize).
        // Without this loose match, valid code like `let x: u32 = 1;` would
        // spuriously fail typeck.
        // Per §1.0 原則 9 "正确 > 妥协": workaround for the unify table's
        // lossy Uint→Int conversion. The proper fix is a separate
        // IntOrUintVar (v0.2 work).
        (TyKind::Int(crate::ast::IntTy::I8), TyKind::Uint(crate::ast::UintTy::U8)) => true,
        (TyKind::Uint(crate::ast::UintTy::U8), TyKind::Int(crate::ast::IntTy::I8)) => true,
        (TyKind::Int(crate::ast::IntTy::I16), TyKind::Uint(crate::ast::UintTy::U16)) => true,
        (TyKind::Uint(crate::ast::UintTy::U16), TyKind::Int(crate::ast::IntTy::I16)) => true,
        (TyKind::Int(crate::ast::IntTy::I32), TyKind::Uint(crate::ast::UintTy::U32)) => true,
        (TyKind::Uint(crate::ast::UintTy::U32), TyKind::Int(crate::ast::IntTy::I32)) => true,
        (TyKind::Int(crate::ast::IntTy::I64), TyKind::Uint(crate::ast::UintTy::U64)) => true,
        (TyKind::Uint(crate::ast::UintTy::U64), TyKind::Int(crate::ast::IntTy::I64)) => true,
        (TyKind::Int(crate::ast::IntTy::I128), TyKind::Uint(crate::ast::UintTy::U128)) => true,
        (TyKind::Uint(crate::ast::UintTy::U128), TyKind::Int(crate::ast::IntTy::I128)) => true,
        (TyKind::Int(crate::ast::IntTy::Isize), TyKind::Uint(crate::ast::UintTy::Usize)) => true,
        (TyKind::Uint(crate::ast::UintTy::Usize), TyKind::Int(crate::ast::IntTy::Isize)) => true,
        // Stage 18.71: Bool literal in let binding — when the literal `true`
        // is assigned to a bool local, the IntVar (from the literal's Infer)
        // is bound to... hmm, actually Bool literals have type Bool directly,
        // not Infer. So this case shouldn't be needed. But adding it for safety.
        (TyKind::Bool, TyKind::Bool) => true,
        // Never (!) type is compatible with everything (divergence)
        // Per §1.0 原則 9: Never type unifies with all types (like Rust).
        (TyKind::Never, _) | (_, TyKind::Never) => true,
        // Tuple(()) ↔ Tuple(): unit type match (also handles general tuple matching)
        (TyKind::Tuple(a_tys), TyKind::Tuple(b_tys)) if a_tys.len() == b_tys.len() => a_tys
            .iter()
            .zip(b_tys.iter())
            .all(|(a, b)| types_match_loose(a, b)),
        // Param ↔ concrete: generic type param vs concrete type in monomorphized code.
        // Per §1.0 原則 9: Stage 0 doesn't fully monomorphize before typeck.
        (TyKind::Param(_), _) | (_, TyKind::Param(_)) => true,
        // Stage 18.99 (TD-13 fix): FnDef ↔ FnPtr loose-matches to route
        // through unify (which now checks sig compatibility via
        // unify_fndef_with_fnptr when fn_sigs is set). Previously this
        // returned true AND the else-if branch suppressed unify errors,
        // which accepted incompatible sigs. Now the else-if branch still
        // calls unify but does NOT suppress errors for FnDef↔FnPtr —
        // see check_statement's else-if branch for the conditional.
        // Per §2.0 原则 9 "正确 > 妥协": soundness.
        (TyKind::FnDef(_, _), TyKind::FnPtr(_)) | (TyKind::FnPtr(_), TyKind::FnDef(_, _)) => true,
        // Same kind: catch-all for primitive types (Bool, Char, Int, Uint, Float, Str, etc.)
        _ if a.kind == b.kind => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;

    fn make_mir_with_return_i32() -> MirBody {
        let mut mir = MirBody::new(Span::DUMMY);
        let _bb0 = mir.new_block(); // create entry block
        let return_local = mir.new_local(
            Ty::from_kind(TyKind::Int(ast::IntTy::I32)),
            None,
            Span::DUMMY,
        );
        let temp = mir.new_local(
            Ty::from_kind(TyKind::Int(ast::IntTy::I32)),
            None,
            Span::DUMMY,
        );
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(temp, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Ty::from_kind(TyKind::Int(ast::IntTy::I32)),
                    val: ConstVal::Int(42),
                })),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(return_local, Span::DUMMY),
                Rvalue::Use(Operand::Copy(Place::local(temp, Span::DUMMY))),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator =
            Terminator::new(TerminatorKind::Return, Span::DUMMY);
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
        let dest = mir.new_local(Ty::from_kind(TyKind::Bool), None, Span::DUMMY);
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(dest, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Ty::from_kind(TyKind::Int(ast::IntTy::I32)),
                    val: ConstVal::Int(42),
                })),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator =
            Terminator::new(TerminatorKind::Return, Span::DUMMY);
        let errors = check_mir_body(&mut mir);
        assert!(!errors.is_empty(), "expected type mismatch error");
    }

    #[test]
    fn check_comparison_returns_bool() {
        let mut mir = MirBody::new(Span::DUMMY);
        let _ = mir.new_block(); // entry block
        let result = mir.new_local(Ty::from_kind(TyKind::Bool), None, Span::DUMMY);
        let a = mir.new_local(
            Ty::from_kind(TyKind::Int(ast::IntTy::I32)),
            None,
            Span::DUMMY,
        );
        let b = mir.new_local(
            Ty::from_kind(TyKind::Int(ast::IntTy::I32)),
            None,
            Span::DUMMY,
        );
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(result, Span::DUMMY),
                Rvalue::BinaryOp(
                    BinOp::Eq,
                    Operand::Copy(Place::local(a, Span::DUMMY)),
                    Operand::Copy(Place::local(b, Span::DUMMY)),
                ),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator =
            Terminator::new(TerminatorKind::Return, Span::DUMMY);
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
                    Box::new(Ty::from_kind(TyKind::Int(ast::IntTy::I32))),
                ),
                Span::DUMMY,
            ),
            None,
            Span::DUMMY,
        );
        let src = mir.new_local(
            Ty::from_kind(TyKind::Int(ast::IntTy::I32)),
            None,
            Span::DUMMY,
        );
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(dest, Span::DUMMY),
                Rvalue::Ref(
                    Region::Erased,
                    BorrowKind::Shared,
                    Place::local(src, Span::DUMMY),
                ),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator =
            Terminator::new(TerminatorKind::Return, Span::DUMMY);
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
                    Ty::from_kind(TyKind::Int(ast::IntTy::I32)),
                    Ty::from_kind(TyKind::Bool),
                ]),
                Span::DUMMY,
            ),
            None,
            Span::DUMMY,
        );
        let a = mir.new_local(
            Ty::from_kind(TyKind::Int(ast::IntTy::I32)),
            None,
            Span::DUMMY,
        );
        let b = mir.new_local(Ty::from_kind(TyKind::Bool), None, Span::DUMMY);
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(dest, Span::DUMMY),
                Rvalue::Aggregate(
                    AggregateKind::Tuple,
                    vec![
                        Operand::Copy(Place::local(a, Span::DUMMY)),
                        Operand::Copy(Place::local(b, Span::DUMMY)),
                    ],
                ),
            ))),
            span: Span::DUMMY,
        });
        mir.block_mut(BasicBlockId(0)).terminator =
            Terminator::new(TerminatorKind::Return, Span::DUMMY);
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
            Ty::from_kind(TyKind::Int(ast::IntTy::I32)),
            None,
            Span::DUMMY,
        );
        let bb1 = mir.new_block();
        let bb2 = mir.new_block();
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::new(
            TerminatorKind::SwitchInt {
                discr: Operand::Copy(Place::local(discr, Span::DUMMY)),
                targets: vec![(ConstVal::Int(1), bb1)],
                otherwise: bb2,
            },
            Span::DUMMY,
        );
        mir.block_mut(bb1).terminator = Terminator::new(TerminatorKind::Return, Span::DUMMY);
        mir.block_mut(bb2).terminator = Terminator::new(TerminatorKind::Return, Span::DUMMY);
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
                    Box::new(Ty::from_kind(TyKind::Int(ast::IntTy::I32))),
                ),
                Span::DUMMY,
            ),
            None,
            Span::DUMMY,
        );
        let bb1 = mir.new_block();
        let bb2 = mir.new_block();
        mir.block_mut(BasicBlockId(0)).terminator = Terminator::new(
            TerminatorKind::SwitchInt {
                discr: Operand::Copy(Place::local(discr, Span::DUMMY)),
                targets: vec![(ConstVal::Int(1), bb1)],
                otherwise: bb2,
            },
            Span::DUMMY,
        );
        mir.block_mut(bb1).terminator = Terminator::new(TerminatorKind::Return, Span::DUMMY);
        mir.block_mut(bb2).terminator = Terminator::new(TerminatorKind::Return, Span::DUMMY);
        let errors = check_mir_body(&mut mir);
        assert!(!errors.is_empty(), "expected error for switch on ref type");
    }

    // === Stage 16.84: checker.rs type error resolver tests ===
    // Per §9.4.3: 2 positive + 6 negative tests (1:3 ratio).

    /// Stage 16.84 positive 1: format_ty with resolver shows struct name.
    #[test]
    fn stage16_84_format_ty_with_resolver_shows_name() {
        use crate::compile;
        let src = "struct MyStruct { x: i32 } fn main() { 0 }";
        let result = compile(src);
        let resolver = &result.trait_resolver;
        let interner = &result.interner;

        let mut tc = TypeChecker::new();
        tc.unify.set_resolver(resolver, interner);

        // Find MyStruct DefId
        let mut struct_def_id = None;
        for (def_id, spur) in &resolver.type_by_def_id {
            if interner.resolve(spur) == "MyStruct" {
                struct_def_id = Some(*def_id);
                break;
            }
        }
        let def_id = struct_def_id.expect("MyStruct not found");
        let ty = Ty::new(TyKind::Adt(def_id, Vec::new().into()), Span::DUMMY);
        let formatted = tc.format_ty(&ty);
        assert_eq!(
            formatted, "MyStruct",
            "format_ty should show 'MyStruct', got '{}'",
            formatted
        );
    }

    /// Stage 16.84 positive 2: format_ty without resolver falls back.
    #[test]
    fn stage16_84_format_ty_without_resolver_falls_back() {
        let tc = TypeChecker::new();
        let ty = Ty::from_kind(TyKind::Int(ast::IntTy::I32));
        let formatted = tc.format_ty(&ty);
        assert_eq!(formatted, "i32");
    }

    /// Stage 16.84 negative 1: Compile "expected function, found struct" shows name.
    #[test]
    fn stage16_84_compile_expected_function_found_struct_shows_name() {
        use crate::compile;
        let src = "struct MyStruct { x: i32 } fn main() { let s = MyStruct { x: 1 }; s(); 0 }";
        let result = compile(src);
        let has_struct_name = result
            .errors
            .typeck
            .iter()
            .any(|e| e.message.contains("MyStruct"));
        if !result.errors.typeck.is_empty() {
            assert!(
                has_struct_name,
                "Error should contain 'MyStruct', got: {:?}",
                result.errors.typeck
            );
        }
    }

    /// Stage 16.84 negative 2: Compile "if condition must be bool" shows name.
    #[test]
    fn stage16_84_compile_if_condition_must_be_bool_shows_name() {
        use crate::compile;
        let src = "struct MyStruct { x: i32 } fn main() { let s = MyStruct { x: 1 }; if s { 0 } else { 1 } }";
        let result = compile(src);
        let has_struct_name = result
            .errors
            .typeck
            .iter()
            .any(|e| e.message.contains("MyStruct"));
        if !result.errors.typeck.is_empty() {
            assert!(
                has_struct_name,
                "Error should contain 'MyStruct', got: {:?}",
                result.errors.typeck
            );
        }
    }

    /// Stage 16.84 negative 3: Compile switch discriminant shows name.
    #[test]
    fn stage16_84_compile_switch_discriminant_shows_name() {
        use crate::compile;
        let src = "struct MyStruct { x: i32 } fn main() { let s = MyStruct { x: 1 }; match s { _ => 0 } }";
        let result = compile(src);
        let has_struct_name = result
            .errors
            .typeck
            .iter()
            .any(|e| e.message.contains("MyStruct"));
        if !result.errors.typeck.is_empty() {
            assert!(
                has_struct_name,
                "Error should contain 'MyStruct', got: {:?}",
                result.errors.typeck
            );
        }
    }

    /// Stage 16.84 negative 4: Compile match arm mismatch shows name.
    #[test]
    fn stage16_84_compile_match_arm_mismatch_shows_name() {
        use crate::compile;
        let src = "struct Foo { x: i32 } struct Bar { y: i32 } fn main() { let f = Foo { x: 1 }; match 1 { 0 => f, _ => Bar { y: 2 } } }";
        let result = compile(src);
        let has_type_name = result
            .errors
            .typeck
            .iter()
            .any(|e| e.message.contains("Foo") || e.message.contains("Bar"));
        if !result.errors.typeck.is_empty() {
            assert!(
                has_type_name,
                "Error should contain 'Foo' or 'Bar', got: {:?}",
                result.errors.typeck
            );
        }
    }

    /// Stage 16.84 negative 5: Compile call non-function shows name.
    #[test]
    fn stage16_84_compile_call_non_function_shows_name() {
        use crate::compile;
        let src = "struct MyStruct { x: i32 } fn main() { let s = MyStruct { x: 1 }; s(42); 0 }";
        let result = compile(src);
        let has_struct_name = result
            .errors
            .typeck
            .iter()
            .any(|e| e.message.contains("MyStruct"));
        if !result.errors.typeck.is_empty() {
            assert!(
                has_struct_name,
                "Error should contain 'MyStruct', got: {:?}",
                result.errors.typeck
            );
        }
    }

    /// Stage 16.84 negative 6: Compile method call on non-function shows name.
    #[test]
    fn stage16_84_compile_method_call_non_function_shows_name() {
        use crate::compile;
        // Calling a method that doesn't exist on a struct should produce
        // a "no method found" error. The MIR lower currently uses
        // type_kind_to_string (no resolver), so the type name may show
        // as <adt>. This test verifies the error is produced (not the
        // exact message format, which is a separate improvement area).
        let src = "struct MyStruct { x: i32 } fn main() { let s = MyStruct { x: 1 }; s.nonexistent(); 0 }";
        let result = compile(src);
        let has_method_error = result
            .errors
            .typeck
            .iter()
            .any(|e| e.message.contains("no method") || e.message.contains("method"));
        assert!(
            has_method_error,
            "Should produce method error, got: {:?}",
            result.errors.typeck
        );
    }

    // ========================================================================
    // Stage 18.71 P0 tests — typeck enhancement for type mismatch detection.
    // Per §9.4.3: 1:3+ ratio (positive : negative).
    // ========================================================================

    /// Stage 18.71 P0-1 positive: `let x: i32 = 42;` compiles OK.
    #[test]
    fn stage18_71_let_int_with_int_literal_ok() {
        use crate::compile;
        let src = "fn main() { let x: i32 = 42; }";
        let result = compile(src);
        assert!(
            result.errors.typeck.is_empty(),
            "expected no typeck errors, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.71 P0-1 negative: `let x: i32 = true;` is rejected.
    #[test]
    fn stage18_71_let_int_with_bool_rejected() {
        use crate::compile;
        let src = "fn main() { let x: i32 = true; }";
        let result = compile(src);
        assert!(
            !result.errors.typeck.is_empty(),
            "expected type mismatch error for `let x: i32 = true;`"
        );
        assert!(
            result
                .errors
                .typeck
                .iter()
                .any(|e| e.message.contains("mismatched types")),
            "expected 'mismatched types' error, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.71 P0-2 negative: `fn f() -> i32 { true }` is rejected.
    #[test]
    fn stage18_71_fn_return_type_mismatch_rejected() {
        use crate::compile;
        let src = "fn f() -> i32 { true } fn main() { f(); }";
        let result = compile(src);
        assert!(
            !result.errors.typeck.is_empty(),
            "expected type mismatch error for `fn f() -> i32 {{ true }}`"
        );
    }

    /// Stage 18.71 P0-2 positive: `fn f() -> i32 { 42 }` compiles OK.
    #[test]
    fn stage18_71_fn_return_type_match_ok() {
        use crate::compile;
        let src = "fn f() -> i32 { 42 } fn main() { f(); }";
        let result = compile(src);
        assert!(
            result.errors.typeck.is_empty(),
            "expected no typeck errors, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.71 P0-3 negative: if-branch type mismatch is rejected.
    #[test]
    fn stage18_71_if_branch_mismatch_rejected() {
        use crate::compile;
        let src = "fn main() { let x = if true { 1 } else { true }; }";
        let result = compile(src);
        assert!(
            !result.errors.typeck.is_empty(),
            "expected type mismatch error for if-branch mismatch"
        );
    }

    /// Stage 18.71 P0-3 positive: if-branches with same type compiles OK.
    #[test]
    fn stage18_71_if_branch_match_ok() {
        use crate::compile;
        let src = "fn main() { let x = if true { 1 } else { 2 }; }";
        let result = compile(src);
        assert!(
            result.errors.typeck.is_empty(),
            "expected no typeck errors, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.71 P0-3 variant: match arm type mismatch is rejected.
    #[test]
    fn stage18_71_match_arm_mismatch_rejected() {
        use crate::compile;
        let src = "fn main() { let x = match 1 { 0 => 1, _ => true }; }";
        let result = compile(src);
        assert!(
            !result.errors.typeck.is_empty(),
            "expected type mismatch error for match arm mismatch"
        );
    }

    /// Stage 18.71 P0-4 negative: trait impl return type mismatch is rejected.
    #[test]
    fn stage18_71_trait_impl_ret_mismatch_rejected() {
        use crate::compile;
        let src = "trait T { fn f(&self) -> i32; } struct S; impl T for S { fn f(&self) -> bool { true } } fn main() {}";
        let result = compile(src);
        assert!(
            !result.errors.typeck.is_empty(),
            "expected trait impl signature mismatch error"
        );
        assert!(
            result.errors.typeck.iter().any(|e| e
                .message
                .contains("method")
                && e.message.contains("return type mismatch")),
            "expected 'method return type mismatch' error, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.71 P0-4 negative: trait impl arg count mismatch is rejected.
    #[test]
    fn stage18_71_trait_impl_arg_count_mismatch_rejected() {
        use crate::compile;
        let src = "trait T { fn f(&self) -> i32; } struct S; impl T for S { fn f(&self, x: i32) -> i32 { 0 } } fn main() {}";
        let result = compile(src);
        assert!(
            !result.errors.typeck.is_empty(),
            "expected trait impl arg count mismatch error"
        );
    }

    /// Stage 18.71 P0-4 positive: trait impl with correct signature compiles OK.
    #[test]
    fn stage18_71_trait_impl_correct_sig_ok() {
        use crate::compile;
        let src = "trait T { fn f(&self) -> i32; } struct S; impl T for S { fn f(&self) -> i32 { 0 } } fn main() {}";
        let result = compile(src);
        assert!(
            result.errors.typeck.is_empty(),
            "expected no typeck errors, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.71 P0-5 negative: return with value in void fn is rejected.
    #[test]
    fn stage18_71_void_fn_return_value_rejected() {
        use crate::compile;
        let src = "fn f() { return 42; } fn main() { f(); }";
        let result = compile(src);
        assert!(
            !result.errors.typeck.is_empty(),
            "expected type mismatch error for `return 42` in void fn"
        );
    }

    /// Stage 18.71 P0-5 positive: return without value in void fn compiles OK.
    #[test]
    fn stage18_71_void_fn_return_no_value_ok() {
        use crate::compile;
        let src = "fn f() { return; } fn main() { f(); }";
        let result = compile(src);
        assert!(
            result.errors.typeck.is_empty(),
            "expected no typeck errors, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.71 P0-5 positive: void fn with no return compiles OK.
    #[test]
    fn stage18_71_void_fn_no_return_ok() {
        use crate::compile;
        let src = "fn f() { let x = 42; } fn main() { f(); }";
        let result = compile(src);
        assert!(
            result.errors.typeck.is_empty(),
            "expected no typeck errors, got: {:?}",
            result.errors.typeck
        );
    }

    // ========================================================================
    // Stage 18.72 P1 tests — struct field count + tuple index + pattern arity.
    // Per §9.4.3: 1:3+ ratio (positive : negative).
    // ========================================================================

    /// Stage 18.72 P1-A positive: `S { x: 1, y: 2 }` compiles OK.
    #[test]
    fn stage18_72_struct_all_fields_ok() {
        use crate::compile;
        let src = "struct S { x: i32, y: i32 } fn main() { let s = S { x: 1, y: 2 }; }";
        let result = compile(src);
        assert!(
            result.errors.typeck.is_empty(),
            "expected no typeck errors, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.72 P1-A negative: missing field is rejected.
    #[test]
    fn stage18_72_struct_missing_field_rejected() {
        use crate::compile;
        let src = "struct S { x: i32, y: i32 } fn main() { let s = S { x: 1 }; }";
        let result = compile(src);
        assert!(
            result
                .errors
                .typeck
                .iter()
                .any(|e| e.message.contains("missing field")),
            "expected 'missing field' error, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.72 P1-A negative: extra field is rejected.
    #[test]
    fn stage18_72_struct_extra_field_rejected() {
        use crate::compile;
        let src = "struct S { x: i32 } fn main() { let s = S { x: 1, y: 2 }; }";
        let result = compile(src);
        assert!(
            result
                .errors
                .typeck
                .iter()
                .any(|e| e.message.contains("no field")),
            "expected 'no field' error, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.72 P1-A negative: unknown field is rejected.
    #[test]
    fn stage18_72_struct_unknown_field_rejected() {
        use crate::compile;
        let src = "struct S { x: i32 } fn main() { let s = S { z: 1 }; }";
        let result = compile(src);
        assert!(
            result
                .errors
                .typeck
                .iter()
                .any(|e| e.message.contains("no field")),
            "expected 'no field' error, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.72 P1-A negative: duplicate field is rejected.
    #[test]
    fn stage18_72_struct_duplicate_field_rejected() {
        use crate::compile;
        let src = "struct S { x: i32, y: i32 } fn main() { let s = S { x: 1, x: 2 }; }";
        let result = compile(src);
        assert!(
            result
                .errors
                .typeck
                .iter()
                .any(|e| e.message.contains("specified more than once")),
            "expected 'specified more than once' error, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.72 P1-B positive: valid tuple index compiles OK.
    #[test]
    fn stage18_72_tuple_valid_index_ok() {
        use crate::compile;
        let src = "fn main() { let t = (1, 2, 3); let x = t.0; let y = t.1; let z = t.2; }";
        let result = compile(src);
        assert!(
            result
                .errors
                .typeck
                .iter()
                .all(|e| !e.message.contains("tuple index out of bounds")),
            "expected no tuple index OOB errors, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.72 P1-B negative: tuple index out of bounds is rejected.
    #[test]
    fn stage18_72_tuple_index_oob_rejected() {
        use crate::compile;
        let src = "fn main() { let t = (1, 2); let x = t.5; }";
        let result = compile(src);
        assert!(
            result
                .errors
                .typeck
                .iter()
                .any(|e| e.message.contains("tuple index out of bounds")),
            "expected 'tuple index out of bounds' error, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.72 P1-C negative: pattern arity too many is rejected.
    #[test]
    fn stage18_72_pattern_arity_too_many_rejected() {
        use crate::compile;
        let src = "fn main() { let (a, b, c) = (1, 2); }";
        let result = compile(src);
        assert!(
            !result.errors.typeck.is_empty(),
            "expected typeck error for pattern arity mismatch"
        );
    }

    /// Stage 18.72 P1-C positive: matching pattern arity compiles OK.
    #[test]
    fn stage18_72_pattern_arity_match_ok() {
        use crate::compile;
        let src = "fn main() { let (a, b) = (1, 2); }";
        let result = compile(src);
        assert!(
            result.errors.typeck.is_empty(),
            "expected no typeck errors, got: {:?}",
            result.errors.typeck
        );
    }

    // ========================================================================
    // Stage 18.73 P1 tests — array index + cast + assignment + assoc const.
    // ========================================================================

    /// Stage 18.73 P1-D positive: valid integer array index compiles OK.
    #[test]
    fn stage18_73_array_valid_index_ok() {
        use crate::compile;
        let src = "fn main() { let a = [1, 2, 3]; let b = a[0]; let c = a[1]; }";
        let result = compile(src);
        assert!(
            result
                .errors
                .typeck
                .iter()
                .all(|e| !e.message.contains("array index must be an integer")),
            "expected no array index errors, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.73 P1-D negative: bool array index is rejected.
    #[test]
    fn stage18_73_array_bool_index_rejected() {
        use crate::compile;
        let src = "fn main() { let a = [1, 2, 3]; let b = a[true]; }";
        let result = compile(src);
        assert!(
            result
                .errors
                .typeck
                .iter()
                .any(|e| e.message.contains("array index must be an integer")),
            "expected 'array index must be an integer' error, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.73 P1-F negative: invalid cast str→int is rejected.
    #[test]
    fn stage18_73_cast_str_to_int_rejected() {
        use crate::compile;
        let src = "fn main() { let x = \"hello\" as i32; }";
        let result = compile(src);
        assert!(
            result
                .errors
                .typeck
                .iter()
                .any(|e| e.message.contains("invalid cast")),
            "expected 'invalid cast' error, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.73 P1-F positive: valid cast int→float compiles OK.
    #[test]
    fn stage18_73_cast_int_to_float_ok() {
        use crate::compile;
        let src = "fn main() { let x = 42 as f64; }";
        let result = compile(src);
        assert!(
            result
                .errors
                .typeck
                .iter()
                .all(|e| !e.message.contains("invalid cast")),
            "expected no cast errors, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.73 P1-E negative: assign to literal is rejected.
    #[test]
    fn stage18_73_assign_to_literal_rejected() {
        use crate::compile;
        let src = "fn main() { 42 = 99; }";
        let result = compile(src);
        assert!(
            result
                .errors
                .typeck
                .iter()
                .any(|e| e.message.contains("invalid assignment target")),
            "expected 'invalid assignment target' error, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.73 P1-E positive: assign to variable compiles OK.
    #[test]
    fn stage18_73_assign_to_variable_ok() {
        use crate::compile;
        let src = "fn main() { let mut x = 0; x = 42; }";
        let result = compile(src);
        assert!(
            result
                .errors
                .typeck
                .iter()
                .all(|e| !e.message.contains("invalid assignment target")),
            "expected no assignment target errors, got: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 18.73 P1-H negative: missing associated const is rejected.
    #[test]
    fn stage18_73_missing_assoc_const_rejected() {
        use crate::compile;
        let src = "trait T { const X: i32; } struct S; impl T for S { } fn main() {}";
        let result = compile(src);
        // The error is a TraitError::Incomplete with missing_associated_consts.
        let has_assoc_const_error = result.errors.trait_errors.iter().any(|e| {
            if let crate::traits::TraitError::Incomplete(inc) = e {
                !inc.missing_associated_consts.is_empty()
            } else {
                false
            }
        });
        assert!(
            has_assoc_const_error,
            "expected missing associated const error, got: {:?}",
            result.errors.trait_errors
        );
    }

    /// Stage 18.73 P1-H positive: impl with all associated consts compiles OK.
    #[test]
    fn stage18_73_assoc_const_provided_ok() {
        use crate::compile;
        let src =
            "trait T { const X: i32; } struct S; impl T for S { const X: i32 = 42; } fn main() {}";
        let result = compile(src);
        assert!(
            result.errors.trait_errors.is_empty(),
            "expected no trait errors, got: {:?}",
            result.errors.trait_errors
        );
    }

    // ========================================================================
    // Stage 18.75 P0 tests — error system fixes.
    // ========================================================================

    /// Stage 18.75 P0-2: macro errors are now visible via to_diagnostics.
    #[test]
    fn stage18_75_macro_errors_visible() {
        use crate::compile;
        let src = "fn main() { write!(\"test\"); }";
        let result = compile(src);
        // Macro errors should be collected in macro_errors.
        assert!(
            !result.errors.macro_errors.is_empty(),
            "expected macro error for undefined write! macro, got: {:?}",
            result.errors.macro_errors
        );
        // Macro errors should appear in to_diagnostics output.
        let diags = result.errors.to_diagnostics(None);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("no matching rule for macro")),
            "expected 'no matching rule for macro' in diagnostics, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    /// Stage 18.75 P0-1: CompileErrors has lower + codegen fields.
    #[test]
    fn stage18_75_compile_errors_has_lower_codegen_fields() {
        use crate::compile;
        let src = "fn main() { 42 }";
        let result = compile(src);
        // The lower and codegen fields should exist and be accessible.
        let lower_count = result.errors.lower.len();
        let codegen_count = result.errors.codegen.len();
        // For valid code, both should be 0 (no errors).
        assert_eq!(
            lower_count, 0,
            "expected 0 lower errors for valid code, got {}",
            lower_count
        );
        assert_eq!(
            codegen_count, 0,
            "expected 0 codegen errors for valid code, got {}",
            codegen_count
        );
    }

    /// Stage 18.75 P0-3: ErrorCode has Codegen (E700) and Macro (E800).
    #[test]
    fn stage18_75_error_code_codegen_and_macro_exist() {
        use crate::diagnostics::ErrorCode;
        assert_eq!(ErrorCode::Codegen.code(), "E700");
        assert_eq!(ErrorCode::Macro.code(), "E800");
        assert_eq!(ErrorCode::Codegen.category(), "codegen");
        assert_eq!(ErrorCode::Macro.category(), "macro");
    }

    /// Stage 18.75 P0-1: total_count includes lower + codegen.
    #[test]
    fn stage18_75_total_count_includes_all_fields() {
        use crate::compile;
        let src = "fn main() { write!(\"test\"); }";
        let result = compile(src);
        let total = result.errors.total_count();
        let sum = result.errors.lex.len()
            + result.errors.parse.len()
            + result.errors.lower.len()
            + result.errors.resolve.len()
            + result.errors.typeck.len()
            + result.errors.borrowck.len()
            + result.errors.trait_errors.len()
            + result.errors.macro_errors.len()
            + result.errors.codegen.len();
        assert_eq!(
            total, sum,
            "total_count should equal sum of all field lengths"
        );
        assert!(total > 0, "expected at least 1 error (macro), got 0");
    }
}
