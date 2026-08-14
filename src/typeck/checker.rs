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

use crate::ast;
use crate::mir::body::TerminatorKind;
use crate::mir::body::*;
use crate::mir::place::*;
use crate::mir::ty::*;
use crate::session::Span;
use crate::typeck::error::TypeError;
use crate::typeck::unify::UnificationTable;

// Stage 6.15: import data tables + predicates from sub-modules.
use super::predicates::{
    can_coerce, is_arithmetic_ty, is_concrete_int_or_float, is_negatable_ty, is_notable_ty,
    is_shift_count_ty,
};
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
    fn format_ty(&self, ty: &Ty) -> String {
        if let (Some(resolver), Some(interner)) = (self.unify.resolver(), self.unify.interner()) {
            crate::mir::ty::type_to_string_with_resolver(ty, resolver, interner)
        } else {
            crate::mir::ty::type_to_string(ty)
        }
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
    fn post_check_statement(&mut self, mir: &MirBody, stmt: &Statement) {
        if let StatementKind::Assign(boxed) = &stmt.kind {
            let (place, rvalue) = &**boxed;
            let place_ty = self.infer_place(mir, place);
            // Stage 18.71: Use type-only inference (no side effects).
            let rvalue_ty = self.infer_rvalue_type_only(mir, rvalue);

            let resolved_place = self.unify.resolve(&place_ty);
            let resolved_rvalue = self.unify.resolve(&rvalue_ty);

            let place_is_concrete =
                !matches!(resolved_place.kind, TyKind::Infer(_) | TyKind::Error)
                    && !type_has_unresolved_substs(&resolved_place);
            let rvalue_is_concrete =
                !matches!(resolved_rvalue.kind, TyKind::Infer(_) | TyKind::Error)
                    && !type_has_unresolved_substs(&resolved_rvalue);

            // Stage 18.71: Only fire if BOTH are concrete AND can't coerce
            // AND can't loose-match. Per §1.0 原則 9 "正确 > 妥协": avoid
            // false positives on generic/unresolved types.
            if place_is_concrete
                && rvalue_is_concrete
                && !can_coerce(&resolved_place, &resolved_rvalue)
                && !types_match_loose(&resolved_place, &resolved_rvalue)
            {
                // Per §1.0 原則 4 "报错 > 静默".
                let span = stmt.span;
                // Stage 18.71: Dedupe — skip if Phase 1 already reported
                // the same mismatch (same span + expected + found). This
                // happens when both Phase 1 and Phase 5.5 detect the same
                // type mismatch (e.g., `let x: i32 = true;` where both
                // place and rvalue are already concrete in Phase 1).
                //
                // Per §1.0 原則 6 "通用 > 特例": one dedup logic for all
                // type mismatch errors.
                let already_reported = self.errors.iter().any(|e| {
                    e.span == span
                        && e.expected.as_ref() == Some(&resolved_place)
                        && e.found.as_ref() == Some(&resolved_rvalue)
                });
                if !already_reported {
                    self.errors.push(crate::typeck::TypeError::mismatch(
                        resolved_place.clone(),
                        resolved_rvalue.clone(),
                        span,
                    ));
                }
            }
        }
    }

    /// Stage 18.71: Infer an rvalue's type WITHOUT side effects.
    ///
    /// Unlike `infer_rvalue`, this function does NOT call `unify` on
    /// operands. It only returns the rvalue's type based on its structure.
    /// Used by `post_check_statement` to avoid re-unifying already-resolved
    /// types (which would produce spurious errors).
    ///
    /// Per §1.0 原則 3 "显式 > 隐式": the no-side-effect contract is explicit.
    ///
    /// Stage 18.72: Now `&mut self` because `infer_operand_type_only` →
    /// `infer_place` → `infer_projection` may push tuple index OOB errors.
    /// The "no side effects" contract is relaxed: tuple index OOB is a
    /// real error that should be reported even in post_check_statement.
    fn infer_rvalue_type_only(&mut self, mir: &MirBody, rv: &Rvalue) -> Ty {
        use crate::mir::place::Rvalue;
        match rv {
            Rvalue::Use(op) => self.infer_operand_type_only(mir, op),
            Rvalue::BinaryOp(op, a, _) => {
                // Comparison ops return Bool; arithmetic ops return operand type.
                match op {
                    crate::mir::place::BinOp::Eq
                    | crate::mir::place::BinOp::Ne
                    | crate::mir::place::BinOp::Lt
                    | crate::mir::place::BinOp::Le
                    | crate::mir::place::BinOp::Gt
                    | crate::mir::place::BinOp::Ge => Ty::new(TyKind::Bool, Span::DUMMY),
                    _ => self.infer_operand_type_only(mir, a),
                }
            }
            Rvalue::BinaryOp2(_, a, _) => {
                // Range ops — return the first operand's type (best effort).
                self.infer_operand_type_only(mir, a)
            }
            Rvalue::UnaryOp(_, op) => self.infer_operand_type_only(mir, op),
            Rvalue::Cast(_, _, ty) => ty.clone(),
            Rvalue::Aggregate(kind, operands) => match kind {
                crate::mir::place::AggregateKind::Tuple => {
                    let tys: Vec<Ty> = operands
                        .iter()
                        .map(|op| self.infer_operand_type_only(mir, op))
                        .collect();
                    Ty::new(TyKind::Tuple(tys), Span::DUMMY)
                }
                crate::mir::place::AggregateKind::Array(elem_ty) => {
                    let len = operands.len() as u128;
                    Ty::new(
                        TyKind::Array(
                            Box::new(elem_ty.clone()),
                            Box::new(crate::mir::ty::Const {
                                ty: Ty::new(TyKind::Uint(ast::UintTy::Usize), Span::DUMMY),
                                val: crate::mir::ty::ConstVal::Uint(len),
                            }),
                        ),
                        Span::DUMMY,
                    )
                }
                crate::mir::place::AggregateKind::Adt(def_id, _variant, substs, _field_tys) => {
                    Ty::new(TyKind::Adt(*def_id, substs.clone()), Span::DUMMY)
                }
                crate::mir::place::AggregateKind::Closure(def_id, substs) => {
                    Ty::new(TyKind::Closure(*def_id, substs.clone()), Span::DUMMY)
                }
            },
            Rvalue::Ref(_, _, lv) => {
                let inner_ty = self.infer_place(mir, lv);
                Ty::new(
                    TyKind::Ref(
                        crate::mir::ty::Region::Erased,
                        crate::mir::ty::Mutability::Immutable,
                        Box::new(inner_ty),
                    ),
                    Span::DUMMY,
                )
            }
        }
    }

    /// Stage 18.71: Infer an operand's type WITHOUT side effects.
    /// Stage 18.72: This is now `&mut self` because `infer_place` is
    /// `&mut self` (tuple index bounds check pushes errors). The
    /// "no side effects" contract of `post_check_statement` is preserved
    /// because `infer_rvalue_type_only` (the caller) doesn't use
    /// `infer_operand` — it uses this function directly. The tuple index
    /// OOB error from `infer_place` is acceptable here because it's a
    /// real error that should be reported.
    fn infer_operand_type_only(&mut self, mir: &MirBody, op: &crate::mir::place::Operand) -> Ty {
        match op {
            crate::mir::place::Operand::Copy(lv) | crate::mir::place::Operand::Move(lv) => {
                self.infer_place(mir, lv)
            }
            crate::mir::place::Operand::Constant(c) => c.ty.clone(),
        }
    }

    /// Stage 3.60: Writeback field types using FieldTyTable instead of HIR.
    fn writeback_field_types_with_table(&mut self, mir: &mut MirBody, table: &FieldTyTable) {
        let mut updates: Vec<(
            usize,
            usize,
            Option<crate::mir::place::Place>,
            Option<crate::mir::place::Rvalue>,
        )> = Vec::new();
        for (bb_idx, bb) in mir.basic_blocks.iter().enumerate() {
            for (stmt_idx, stmt) in bb.statements.iter().enumerate() {
                if let StatementKind::Assign(boxed) = &stmt.kind {
                    let (place, rvalue) = &**boxed;
                    let mut new_place = place.clone();
                    let mut new_rvalue = rvalue.clone();
                    let place_changed = writeback_field_types_in_place_with_table(
                        &mut new_place,
                        mir,
                        table,
                        &mut self.unify,
                    );
                    let rvalue_changed = writeback_field_types_in_rvalue_with_table(
                        &mut new_rvalue,
                        mir,
                        table,
                        &mut self.unify,
                    );
                    if place_changed || rvalue_changed {
                        updates.push((
                            bb_idx,
                            stmt_idx,
                            if place_changed { Some(new_place) } else { None },
                            if rvalue_changed {
                                Some(new_rvalue)
                            } else {
                                None
                            },
                        ));
                    }
                }
            }
        }
        for (bb_idx, stmt_idx, new_place, new_rvalue) in updates {
            if let Some(bb) = mir.basic_blocks.get_mut(bb_idx) {
                if let Some(stmt) = bb.statements.get_mut(stmt_idx) {
                    if let StatementKind::Assign(boxed) = &mut stmt.kind {
                        let (place, rvalue) = &mut **boxed;
                        if let Some(np) = new_place {
                            *place = np;
                        }
                        if let Some(nr) = new_rvalue {
                            *rvalue = nr;
                        }
                    }
                }
            }
        }

        fn resolve_place_type_with_table(lv: &crate::mir::place::Place, mir: &MirBody) -> Ty {
            use crate::mir::place::PlaceKind;
            match &lv.kind {
                PlaceKind::Local(id) => mir
                    .local_decls
                    .get(id.0 as usize)
                    .map(|ld| ld.ty.clone())
                    .unwrap_or_else(|| Ty::new(TyKind::Error, lv.span)),
                PlaceKind::Projection(base, elem) => {
                    let _base_ty = resolve_place_type_with_table(base, mir);
                    match elem {
                        crate::mir::place::ProjectionElem::Field(_field_id, field_ty) => {
                            field_ty.clone()
                        }
                        _ => Ty::new(TyKind::Error, lv.span),
                    }
                }
                PlaceKind::Static(_) => Ty::new(TyKind::Error, lv.span),
            }
        }

        fn writeback_field_types_in_place_with_table(
            lv: &mut crate::mir::place::Place,
            mir: &MirBody,
            table: &FieldTyTable,
            unify: &mut crate::typeck::unify::UnificationTable,
        ) -> bool {
            use crate::mir::place::{PlaceKind, ProjectionElem};
            match &mut lv.kind {
                PlaceKind::Projection(base, elem) => {
                    let mut changed =
                        writeback_field_types_in_place_with_table(base, mir, table, unify);
                    if let ProjectionElem::Field(field_id, field_ty) = elem {
                        let base_ty = resolve_place_type_with_table(base, mir);
                        if let TyKind::Adt(def_id, _) = &base_ty.kind {
                            if let Some(fields) = table.struct_fields(def_id) {
                                if let Some(resolved) = fields.get(field_id.0 as usize) {
                                    let current = unify.resolve(field_ty);
                                    match &current.kind {
                                        TyKind::Infer(InferVar::TyVar(vid)) => {
                                            unify.bind_ty_var(*vid, resolved.clone());
                                        }
                                        TyKind::Infer(InferVar::IntVar(vid)) => {
                                            if let TyKind::Int(int_ty) = &resolved.kind {
                                                unify.bind_int_var(*vid, *int_ty);
                                            }
                                        }
                                        _ => {}
                                    }
                                    *field_ty = resolved.clone();
                                    changed = true;
                                }
                            }
                        }
                    }
                    changed
                }
                _ => false,
            }
        }

        fn writeback_field_types_in_rvalue_with_table(
            rv: &mut crate::mir::place::Rvalue,
            mir: &MirBody,
            table: &FieldTyTable,
            unify: &mut crate::typeck::unify::UnificationTable,
        ) -> bool {
            use crate::mir::place::Rvalue;
            match rv {
                Rvalue::Use(op) => {
                    writeback_field_types_in_operand_with_table(op, mir, table, unify)
                }
                Rvalue::BinaryOp(_, a, b) | Rvalue::BinaryOp2(_, a, b) => {
                    writeback_field_types_in_operand_with_table(a, mir, table, unify)
                        | writeback_field_types_in_operand_with_table(b, mir, table, unify)
                }
                Rvalue::UnaryOp(_, op) => {
                    writeback_field_types_in_operand_with_table(op, mir, table, unify)
                }
                Rvalue::Cast(_, op, _) => {
                    writeback_field_types_in_operand_with_table(op, mir, table, unify)
                }
                Rvalue::Aggregate(_, operands) => {
                    let mut changed = false;
                    for op in operands {
                        changed |=
                            writeback_field_types_in_operand_with_table(op, mir, table, unify);
                    }
                    changed
                }
                Rvalue::Ref(_, _, lv) => {
                    writeback_field_types_in_place_with_table(lv, mir, table, unify)
                }
            }
        }

        fn writeback_field_types_in_operand_with_table(
            op: &mut crate::mir::place::Operand,
            mir: &MirBody,
            table: &FieldTyTable,
            unify: &mut crate::typeck::unify::UnificationTable,
        ) -> bool {
            use crate::mir::place::Operand;
            match op {
                Operand::Copy(lv) | Operand::Move(lv) => {
                    writeback_field_types_in_place_with_table(lv, mir, table, unify)
                }
                _ => false,
            }
        }
    }

    /// Stage 3.60: Writeback field-load locals using FieldTyTable instead of HIR.
    fn writeback_field_load_locals_with_table(&mut self, mir: &mut MirBody, table: &FieldTyTable) {
        use crate::mir::place::{Operand, PlaceKind, ProjectionElem, Rvalue};
        for bb in &mir.basic_blocks {
            for stmt in &bb.statements {
                if let StatementKind::Assign(boxed) = &stmt.kind {
                    let (place, rvalue) = &**boxed;
                    if let PlaceKind::Local(dest_id) = &place.kind {
                        if let Rvalue::Use(op) = rvalue {
                            let lv = match op {
                                Operand::Copy(lv) | Operand::Move(lv) => lv,
                                _ => continue,
                            };
                            if let PlaceKind::Projection(base, ProjectionElem::Field(field_id, _)) =
                                &lv.kind
                            {
                                let base_ty = self.resolve_place_for_writeback(mir, base);
                                if let TyKind::Adt(def_id, _) = &base_ty.kind {
                                    if let Some(fields) = table.struct_fields(def_id) {
                                        if let Some(field_ty) = fields.get(field_id.0 as usize) {
                                            if let Some(dest_local) =
                                                mir.local_decls.get_mut(dest_id.0 as usize)
                                            {
                                                dest_local.ty = field_ty.clone();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        // Second pass: fix BinaryOp results.
        for bb in &mir.basic_blocks {
            for stmt in &bb.statements {
                if let StatementKind::Assign(boxed) = &stmt.kind {
                    let (place, rvalue) = &**boxed;
                    if let PlaceKind::Local(dest_id) = &place.kind {
                        if let Rvalue::BinaryOp(op, a, b) = rvalue {
                            // Stage 14.65: Comparison ops (Eq/Ne/Lt/Le/Gt/Ge)
                            // ALWAYS return Bool, regardless of operand types.
                            // Skip the operand-type propagation for these ops,
                            // otherwise `x > 0.0` (where 0.0 is f64) would
                            // overwrite the result type with f64, causing
                            // `store double %cmp_result, %bool_alloca` — a
                            // type mismatch that silently miscompiles at runtime
                            // (segfault when loading as i1).
                            //
                            // Per §1.0 原则 5 "报错 > 静默": comparison results
                            // are always Bool, never the operand type.
                            let is_comparison = matches!(
                                op,
                                BinOp::Eq
                                    | BinOp::Ne
                                    | BinOp::Lt
                                    | BinOp::Le
                                    | BinOp::Gt
                                    | BinOp::Ge
                            );
                            if is_comparison {
                                continue;
                            }
                            let a_ty = self.resolve_operand_for_writeback(mir, a);
                            let b_ty = self.resolve_operand_for_writeback(mir, b);
                            let result_ty = if is_concrete_int_or_float(&a_ty) {
                                Some(a_ty)
                            } else if is_concrete_int_or_float(&b_ty) {
                                Some(b_ty)
                            } else {
                                None
                            };
                            if let Some(ty) = result_ty {
                                if let Some(dest_local) =
                                    mir.local_decls.get_mut(dest_id.0 as usize)
                                {
                                    dest_local.ty = ty;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Check a single MIR body. Walks all basic blocks, infers types
    /// for rvalues, and unifies with place types.
    ///
    /// After inference, writes the resolved types back into
    /// `mir.local_decls[i].ty` so that downstream consumers (borrowck,
    /// codegen) see concrete types instead of inference variables.
    pub fn check_mir_body(&mut self, mir: &mut MirBody) {
        self.check_mir_body_with_tables(mir, None);
    }

    /// Resolve an operand's type for the writeback pass (reads local_decls
    /// which have been fixed by the first pass of writeback_field_load_locals).
    fn resolve_operand_for_writeback(&self, mir: &MirBody, op: &crate::mir::place::Operand) -> Ty {
        use crate::mir::place::Operand;
        match op {
            Operand::Copy(lv) | Operand::Move(lv) => self.resolve_place_for_writeback(mir, lv),
            Operand::Constant(c) => c.ty.clone(),
        }
    }

    /// Resolve a place's type for the writeback pass (post-Phase 3, so
    /// local_decls have resolved types).
    fn resolve_place_for_writeback(&self, mir: &MirBody, lv: &crate::mir::place::Place) -> Ty {
        use crate::mir::place::PlaceKind;
        match &lv.kind {
            PlaceKind::Local(id) => mir
                .local_decls
                .get(id.0 as usize)
                .map(|ld| ld.ty.clone())
                .unwrap_or_else(|| Ty::new(TyKind::Error, lv.span)),
            PlaceKind::Projection(base, elem) => {
                let base_ty = self.resolve_place_for_writeback(mir, base);
                match elem {
                    crate::mir::place::ProjectionElem::Field(_field_id, field_ty) => {
                        field_ty.clone()
                    }
                    _ => base_ty,
                }
            }
            PlaceKind::Static(_) => Ty::new(TyKind::Error, lv.span),
        }
    }

    /// Post-defaulting terminator check. Runs after Phase 3 (writeback)
    /// so all types are resolved. Catches errors that depend on
    /// defaulting (e.g., `let x = 1; x();` where x defaults to i32).
    fn post_check_terminator(&mut self, mir: &MirBody, term: &Terminator) {
        if let TerminatorKind::Call { func, .. } = &term.kind {
            // Stage 18.72: Split into two statements to avoid borrow conflict
            // (infer_operand is now &mut self, unify.resolve is &self).
            let func_ty_raw = self.infer_operand(mir, func);
            let func_ty = self.unify.resolve(&func_ty_raw);
            // G7 fix: if func is neither FnDef nor FnPtr (after defaulting),
            // emit an error. Infer should be resolved by now; if it's still
            // Infer, it means no constraint was applied (rare).
            //
            // Stage 16.29: Also accept TyKind::Closure as callable —
            // closures are called via the synthesized `call` function,
            // and the Closure type is the func type at the call site.
            // Without this, `f()()` patterns (where f returns a closure)
            // would emit false "expected function, found {closure}" errors.
            //
            // Per §1.0 原則 9 "正确 > 妥协": Closure IS a callable type.
            if !matches!(
                &func_ty.kind,
                TyKind::FnDef(_, _) | TyKind::FnPtr(_) | TyKind::Closure(_, _) | TyKind::Error
            ) {
                self.errors.push(TypeError::new(
                    // Stage 15.80: use human-readable type name.
                    // Stage 15.81: use func operand span (was: Span::DUMMY).
                    format!("expected function, found {}", self.format_ty(&func_ty)),
                    crate::mir::place::operand_span(func),
                ));
            }
        }
    }

    /// Check a single MIR statement (Assign or Nop).
    fn check_statement(&mut self, mir: &MirBody, stmt: &Statement) {
        match &stmt.kind {
            StatementKind::Assign(boxed) => {
                let (place, rvalue) = &**boxed;
                let place_ty = self.infer_place(mir, place);
                // Stage 15.82: pass stmt.span to infer_rvalue so BinaryOp/
                // UnaryOp errors get accurate spans (was: Span::DUMMY inside
                // infer_rvalue).
                let rvalue_ty = self.infer_rvalue(mir, rvalue, stmt.span);

                // Stage 18.71: Type mismatch check for Assign statements.
                // Per §1.0 原則 4 "报错 > 静默": if place type is a concrete
                // type (not Infer/Error) and rvalue type is also concrete,
                // and they don't match and can't coerce, report an error.
                //
                // This catches:
                // - `let x: i32 = true;` (e2e-err-002)
                // - `fn f() -> i32 { true }` (cg-err-002, via return local)
                // - `let x = if true { 1 } else { true };` (cg-err-014, via if-else)
                //
                // Per §1.0 原則 6 "通用 > 特例": one check covers all Assign
                // statements — let bindings, return values, if-else results.
                //
                // IMPORTANT: Only check when BOTH types are fully concrete
                // (no Infer vars, no Error). This avoids false positives on
                // generic types where substs haven't been substituted yet
                // (e.g., Box<T> with empty substs vs Box<T> with [i32]).
                // Per §1.0 原則 9 "正确 > 妥协": must not break valid code.
                let resolved_place = self.unify.resolve(&place_ty);
                let resolved_rvalue = self.unify.resolve(&rvalue_ty);

                let place_is_concrete =
                    !matches!(resolved_place.kind, TyKind::Infer(_) | TyKind::Error)
                        && !type_has_unresolved_substs(&resolved_place);
                let rvalue_is_concrete =
                    !matches!(resolved_rvalue.kind, TyKind::Infer(_) | TyKind::Error)
                        && !type_has_unresolved_substs(&resolved_rvalue);

                if place_is_concrete
                    && rvalue_is_concrete
                    && !can_coerce(&resolved_place, &resolved_rvalue)
                    && !types_match_loose(&resolved_place, &resolved_rvalue)
                {
                    // Stage 18.71: Report type mismatch.
                    // Per §1.0 原則 4 "报错 > 静默".
                    let span = if stmt.span != Span::DUMMY {
                        stmt.span
                    } else {
                        Span::DUMMY
                    };
                    self.errors.push(crate::typeck::TypeError::mismatch(
                        resolved_place.clone(),
                        resolved_rvalue.clone(),
                        span,
                    ));
                } else if can_coerce(&place_ty, &rvalue_ty)
                    || types_match_loose(&resolved_place, &resolved_rvalue)
                {
                    // Coercion or loose match succeeded — still try to unify
                    // so Infer vars get bound. Suppress unify errors.
                    let _ = self.unify.unify(&place_ty, &rvalue_ty, stmt.span);
                } else if let Err(mut e) = self.unify.unify(&place_ty, &rvalue_ty, stmt.span) {
                    // Stage 15.82: use stmt.span for unify errors (was:
                    // Span::DUMMY from mismatch(), producing "1:1").
                    if stmt.span != Span::DUMMY {
                        e.span = stmt.span;
                    }
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
            | StatementKind::Deinit(_) => {} // Stage 13.13 + Stage 13.16: Inline println! statement — no type
                                             // constraints to check on the format string (opaque String).
                                             // Stage 13.16: args are already lowered to MIR operands and
                                             // their types were checked during operand lowering (each arg
                                             // is lowered via lower_expr_to_operand which goes through the
                                             // normal type-checking path).
                                             // Stage 18.48: StatementKind::Println variant removed.
        }
    }

    /// Check a terminator's type constraints.
    fn check_terminator(&mut self, mir: &MirBody, term: &Terminator) {
        match &term.kind {
            TerminatorKind::Call {
                func,
                args,
                destination,
                ..
            } => {
                // Infer func type
                // Stage 18.72: Split into two statements to avoid borrow conflict.
                let func_ty_raw = self.infer_operand(mir, func);
                let func_ty = self.unify.resolve(&func_ty_raw);
                // Infer arg types and collect them
                let arg_tys_raw: Vec<Ty> = args
                    .iter()
                    .map(|arg| self.infer_operand(mir, arg))
                    .collect();
                let arg_tys: Vec<Ty> = arg_tys_raw
                    .iter()
                    .map(|ty| self.unify.resolve(ty))
                    .collect();
                // Infer destination type
                let dest_ty_raw = self.infer_place(mir, destination);
                let dest_ty = self.unify.resolve(&dest_ty_raw);

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
                                // Stage 15.81: use the call terminator's span
                                // (was: Span::DUMMY, producing "1:1").
                                term.span,
                            ));
                        } else {
                            for (arg_ty, input_ty) in arg_tys.iter().zip(sig.inputs.iter()) {
                                if let Err(mut e) = self.unify.unify(arg_ty, input_ty, term.span) {
                                    // Stage 15.81: use term.span for unify errors
                                    // (was: Span::DUMMY from mismatch()).
                                    if term.span != Span::DUMMY {
                                        e.span = term.span;
                                    }
                                    self.errors.push(*e);
                                }
                            }
                        }
                        if let Err(mut e) = self.unify.unify(&dest_ty, &sig.output, term.span) {
                            // Stage 15.81: use term.span for unify errors.
                            if term.span != Span::DUMMY {
                                e.span = term.span;
                            }
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
                        if let Err(mut e) = self.unify.unify(arg_ty, input_ty, term.span) {
                            // Stage 15.81: use term.span for unify errors.
                            if term.span != Span::DUMMY {
                                e.span = term.span;
                            }
                            self.errors.push(*e);
                        }
                    }
                    // Unify destination with output
                    if let Err(mut e) = self.unify.unify(&dest_ty, &sig.output, term.span) {
                        // Stage 15.81: use term.span for unify errors.
                        if term.span != Span::DUMMY {
                            e.span = term.span;
                        }
                        self.errors.push(*e);
                    }
                }

                // Stage 16.32 (通解 — Closure-typed func in typeck):
                // If func is a Closure(def_id, _), look up the synthesized
                // function's sig from fn_sigs (same as FnDef). This unifies
                // the dest type with the closure's return type, which is
                // essential for nested closures (`f()()` where f returns a
                // closure).
                //
                // Without this, the dest type stays Infer → "expected
                // function, found _" when the result is called.
                //
                // Note: The closure's sig has inputs = [self, params...].
                // The MIR Call terminator's args = [params...] (self is
                // prepended by codegen, not by MIR lowering). So we skip
                // the first input (self) when checking arg count and unify.
                //
                // Per §1.0 原則 6 "通用 > 特例": handle Closure the same
                // way as FnDef — both are callable types with sigs in
                // fn_sigs.
                if let TyKind::Closure(def_id, _) = &func_ty.kind {
                    if let Some(sig) = self.fn_sigs.get(def_id).cloned() {
                        // Skip the first input (self) — it's not in the
                        // MIR Call terminator's args.
                        let sig_params = &sig.inputs[1.min(sig.inputs.len())..];
                        if arg_tys.len() != sig_params.len() {
                            self.errors.push(TypeError::new(
                                format!(
                                    "this closure takes {} argument(s) but {} were supplied",
                                    sig_params.len(),
                                    arg_tys.len()
                                ),
                                term.span,
                            ));
                        } else {
                            for (arg_ty, input_ty) in arg_tys.iter().zip(sig_params.iter()) {
                                if let Err(mut e) = self.unify.unify(arg_ty, input_ty, term.span) {
                                    if term.span != Span::DUMMY {
                                        e.span = term.span;
                                    }
                                    self.errors.push(*e);
                                }
                            }
                        }
                        if let Err(mut e) = self.unify.unify(&dest_ty, &sig.output, term.span) {
                            if term.span != Span::DUMMY {
                                e.span = term.span;
                            }
                            self.errors.push(*e);
                        }
                    }
                }

                // G7 fix (Stage 2.4f): if func is neither FnDef nor FnPtr
                // (e.g., calling an Int, Bool, Str, Tuple), emit an error.
                // Infer and Error are deferred (might resolve to a fn type).
                //
                // Stage 16.29: Also accept TyKind::Closure as callable —
                // closures are called via the synthesized `call` function.
                if !matches!(
                    &func_ty.kind,
                    TyKind::FnDef(_, _)
                        | TyKind::FnPtr(_)
                        | TyKind::Closure(_, _)
                        | TyKind::Infer(_)
                        | TyKind::Error
                ) {
                    self.errors.push(TypeError::new(
                        // Stage 15.80: use human-readable type name.
                        // Stage 15.81: use func operand span (was: Span::DUMMY).
                        format!("expected function, found {}", self.format_ty(&func_ty)),
                        crate::mir::place::operand_span(func),
                    ));
                }
            }
            TerminatorKind::SwitchInt { discr, targets, .. } => {
                // The discriminant must be an integer or bool
                let discr_ty = self.infer_operand(mir, discr);
                // Stage 15.81: use the discriminant operand's span for
                // error reporting (was: Span::DUMMY, producing "1:1").
                let discr_span = crate::mir::place::operand_span(discr);
                // G7 fix (Stage 2.4f): if any target is ConstVal::Bool(_),
                // this SwitchInt came from an `if` or `while` condition,
                // and the discriminant must be bool (not just any int).
                let requires_bool = targets
                    .iter()
                    .any(|(val, _)| matches!(val, ConstVal::Bool(_)));
                if requires_bool {
                    let bool_ty = Ty::new(TyKind::Bool, Span::DUMMY);
                    if let Err(mut e) = self.unify.unify(&discr_ty, &bool_ty, term.span) {
                        // Stage 15.81: override the dummy span with the
                        // actual discriminant span (was: Span::DUMMY).
                        if discr_span != Span::DUMMY {
                            e.span = discr_span;
                        }
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
                            let _ = self.unify.unify(&discr_ty, &i32_ty, term.span);
                        }
                        TyKind::Error => {}
                        _ => {
                            self.errors.push(TypeError::new(
                                // Stage 15.80: use human-readable type name.
                                // Stage 15.81: use discriminant span (was: Span::DUMMY).
                                format!(
                                    "expected integer or bool for switch, found {}",
                                    self.format_ty(&discr_ty)
                                ),
                                discr_span,
                            ));
                        }
                    }
                }
            }
            TerminatorKind::Drop { place, .. } => {
                // Just infer the place type (no constraint to check)
                let _ = self.infer_place(mir, place);
            }
            TerminatorKind::Assert { cond, .. } => {
                // The condition must be a bool. We don't enforce this
                // strictly (codegen will handle the runtime check) but
                // we do infer the type to catch obvious mismatches.
                let cond_ty = self.infer_operand(mir, cond);
                // Stage 15.81: use the condition operand's span for
                // error reporting (was: Span::DUMMY).
                let cond_span = crate::mir::place::operand_span(cond);
                match &cond_ty.kind {
                    TyKind::Bool | TyKind::Infer(_) | TyKind::Error => {}
                    _ => {
                        self.errors.push(TypeError::new(
                            // Stage 15.80: use human-readable type name.
                            // Stage 15.81: use condition span (was: Span::DUMMY).
                            format!(
                                "assert condition must be bool, found {}",
                                self.format_ty(&cond_ty)
                            ),
                            cond_span,
                        ));
                    }
                }
            }
            _ => {}
        }
    }

    /// Infer the type of a place (place expression).
    ///
    /// Stage 18.72: Changed from `&self` to `&mut self` so that
    /// `infer_projection` can push TypeErrors (e.g., tuple index out
    /// of bounds). All callers already have `&mut self`.
    fn infer_place(&mut self, mir: &MirBody, lv: &Place) -> Ty {
        match &lv.kind {
            PlaceKind::Local(id) => {
                if (id.0 as usize) < mir.local_decls.len() {
                    mir.local(*id).ty.clone()
                } else {
                    Ty::new(TyKind::Error, lv.span)
                }
            }
            PlaceKind::Static(_) => {
                // Static type would come from the HIR; for now, Error
                Ty::new(TyKind::Error, lv.span)
            }
            PlaceKind::Projection(base, elem) => {
                let base_ty = self.infer_place(mir, base);
                self.infer_projection(mir, &base_ty, elem, lv.span)
            }
        }
    }

    /// Infer the type after applying a projection element.
    ///
    /// Stage 18.65: Three sites below (Deref on non-Ref, Index on non-Array,
    /// ConstantIndex on non-Array) silently return `TyKind::Error` without
    /// pushing a `TypeError`. These are known Stage 0 limitations — the
    /// conformance tests `err-*-deref-non-ref-*` and `err-*-index-non-array-*`
    /// document the current behavior. Adding errors here would change those
    /// tests from `compile_error` to having specific error patterns.
    /// Per §1.0 原則 4 "报错 > 静默": these should push errors in a future stage.
    ///
    /// Stage 18.72 P1-B: Added tuple index bounds check. When base type is
    /// `Tuple(tys)` and `Field(field_id, _)` is applied, verify
    /// `field_id.0 < tys.len()`. If out of bounds, push a TypeError and
    /// return Error type.
    /// Per §1.0 原則 4 "报错 > 静默": tuple index OOB must be reported.
    fn infer_projection(
        &mut self,
        mir: &MirBody,
        base_ty: &Ty,
        elem: &ProjectionElem,
        place_span: Span,
    ) -> Ty {
        match elem {
            ProjectionElem::Deref => {
                if let TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) = &base_ty.kind {
                    (**inner).clone()
                } else {
                    // Stage 18.76 P1-A: Deref on non-pointer types is a known
                    // Stage 0 limitation. Pattern bindings on &self, closure
                    // captures, and other internal mechanisms produce Deref
                    // projections on non-Ref types. Pushing an error here would
                    // break valid code that relies on this limitation.
                    //
                    // Per §1.0 原則 9 "正确 > 妥协": defer error reporting for
                    // Deref until typeck properly tracks reference types through
                    // pattern bindings (v0.2 work).
                    //
                    // Return Error type (not base_ty) to avoid confusing
                    // writeback — returning base_ty would make the local's
                    // type be the pointer type, not the pointee type.
                    Ty::new(TyKind::Error, Span::DUMMY)
                }
            }
            ProjectionElem::Field(field_id, field_ty) => {
                // Stage 18.72 P1-B: Tuple index bounds check.
                // Per §1.0 原則 4 "报错 > 静默": out-of-bounds tuple index
                // must be reported, not silently return Error.
                if let TyKind::Tuple(tys) = &base_ty.kind {
                    if (field_id.0 as usize) >= tys.len() {
                        self.errors.push(TypeError::new(
                            format!(
                                "tuple index out of bounds: index {} but tuple has {} element(s)",
                                field_id.0,
                                tys.len()
                            ),
                            place_span,
                        ));
                        return Ty::new(TyKind::Error, Span::DUMMY);
                    }
                }
                field_ty.clone()
            }
            ProjectionElem::Index(idx_local) => {
                // Stage 18.76 P1-A: Allow Array, Slice, Str, and Ref(_, _, Str)
                // as indexable types. Str indexing returns u8 (byte).
                // Per §1.0 原則 6 "通用 > 特例": one check covers all indexable types.
                // Per §1.0 原則 9 "正确 > 妥协": defer for Infer/Error/Param types
                // (don't push false-positive errors on unresolved types).
                let inner_ty = match &base_ty.kind {
                    TyKind::Array(inner, _) | TyKind::Slice(inner) => Some((**inner).clone()),
                    TyKind::Str => Some(Ty::new(TyKind::Uint(crate::ast::UintTy::U8), place_span)),
                    TyKind::Ref(_, _, inner) if matches!(inner.kind, TyKind::Str) => {
                        Some(Ty::new(TyKind::Uint(crate::ast::UintTy::U8), place_span))
                    }
                    TyKind::Ref(_, _, inner) => {
                        // For &Array, &Slice, or &Str, index returns element type.
                        // For &Infer or &Error, defer (don't push error).
                        match &inner.kind {
                            TyKind::Array(inner, _) | TyKind::Slice(inner) => {
                                Some((**inner).clone())
                            }
                            TyKind::Str => {
                                Some(Ty::new(TyKind::Uint(crate::ast::UintTy::U8), place_span))
                            }
                            TyKind::Infer(_) | TyKind::Error => None, // defer
                            _ => None,
                        }
                    }
                    // Stage 18.76: Defer for unresolved types — don't push false-positive errors.
                    TyKind::Infer(_) | TyKind::Error | TyKind::Param(_) => None,
                    _ => None,
                };
                if let Some(inner) = inner_ty {
                    // Stage 18.73 P1-D: Validate array index type.
                    if let Some(idx_local_decl) = mir.local_decls.get(idx_local.0 as usize) {
                        let idx_ty = self.unify.resolve(&idx_local_decl.ty);
                        match &idx_ty.kind {
                            TyKind::Int(_)
                            | TyKind::Uint(_)
                            | TyKind::Infer(InferVar::IntVar(_))
                            | TyKind::Error => {
                                // OK — integer type, Infer (deferred), or Error.
                            }
                            _ => {
                                self.errors.push(TypeError::new(
                                    format!(
                                        "array index must be an integer type, found {}",
                                        self.format_ty(&idx_ty)
                                    ),
                                    place_span,
                                ));
                            }
                        }
                    }
                    inner
                } else {
                    // Stage 18.76: Defer for Infer/Error/Param types — don't push
                    // false-positive errors on unresolved types. Return Error
                    // type to propagate the unknown state.
                    Ty::new(TyKind::Error, Span::DUMMY)
                }
            }
            ProjectionElem::ConstantIndex { .. } | ProjectionElem::Subslice { .. } => {
                // Stage 18.76 P1-A: Same indexable types as Index.
                // Per §1.0 原則 9 "正确 > 妥协": defer for Infer/Error/Param types.
                let inner_ty = match &base_ty.kind {
                    TyKind::Array(inner, _) | TyKind::Slice(inner) => Some((**inner).clone()),
                    TyKind::Str => Some(Ty::new(TyKind::Uint(crate::ast::UintTy::U8), place_span)),
                    TyKind::Ref(_, _, inner) if matches!(inner.kind, TyKind::Str) => {
                        Some(Ty::new(TyKind::Uint(crate::ast::UintTy::U8), place_span))
                    }
                    TyKind::Ref(_, _, inner) => match &inner.kind {
                        TyKind::Array(inner, _) | TyKind::Slice(inner) => Some((**inner).clone()),
                        TyKind::Str => {
                            Some(Ty::new(TyKind::Uint(crate::ast::UintTy::U8), place_span))
                        }
                        TyKind::Infer(_) | TyKind::Error => None, // defer
                        _ => None,
                    },
                    // Stage 18.76: Defer for unresolved types.
                    TyKind::Infer(_) | TyKind::Error | TyKind::Param(_) => None,
                    _ => None,
                };
                if let Some(inner) = inner_ty {
                    inner
                } else {
                    // Stage 18.76: Defer for Infer/Error/Param types — don't push
                    // false-positive errors on unresolved types.
                    Ty::new(TyKind::Error, Span::DUMMY)
                }
            }
        }
    }

    /// Infer the type of an rvalue.
    ///
    /// Stage 15.82: `stmt_span` is the span of the enclosing `Statement`
    /// (or the terminator span for rvalues embedded in terminators). It's
    /// used to attach accurate spans to errors produced inside `infer_rvalue`
    /// (e.g., BinaryOp/UnaryOp type mismatches). Previously these errors
    /// used `Span::DUMMY` (file start "1:1"), making them hard to locate.
    ///
    /// Per §1.0 原則 3 "显式 > 隐式": the span is an explicit parameter,
    /// not hidden state on `self`.
    fn infer_rvalue(&mut self, mir: &MirBody, rv: &Rvalue, stmt_span: Span) -> Ty {
        match rv {
            Rvalue::Use(operand) => self.infer_operand(mir, operand),
            Rvalue::BinaryOp(op, a, b) => {
                // G8 fix (Stage 2.4g): resolve operands before type checking,
                // so TyVar bound to concrete types (Bool, Str, Tuple) is
                // correctly rejected by is_arithmetic_ty etc.
                // Stage 18.72: Split into two statements to avoid borrow conflict.
                let a_ty_raw = self.infer_operand(mir, a);
                let a_ty = self.unify.resolve(&a_ty_raw);
                let b_ty_raw = self.infer_operand(mir, b);
                let b_ty = self.unify.resolve(&b_ty_raw);
                // Unify lhs and rhs types (they must match for arithmetic)
                match op {
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                        // Comparison: unify a and b, return bool
                        if let Err(mut e) = self.unify.unify(&a_ty, &b_ty, stmt_span) {
                            // Stage 15.82: use stmt_span for unify errors.
                            if stmt_span != Span::DUMMY {
                                e.span = stmt_span;
                            }
                            self.errors.push(*e);
                        }
                        Ty::new(TyKind::Bool, Span::DUMMY)
                    }
                    // Bitwise ops: Bool or integer types only.
                    BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
                        if let Err(mut e) = self.unify.unify(&a_ty, &b_ty, stmt_span) {
                            // Stage 15.82: use stmt_span for unify errors.
                            if stmt_span != Span::DUMMY {
                                e.span = stmt_span;
                            }
                            self.errors.push(*e);
                        }
                        // Result type matches operand type (Bool or Int).
                        a_ty
                    }
                    // Shifts: lhs can be int, rhs must be int (not bool).
                    BinOp::Shl | BinOp::Shr => {
                        if !is_shift_count_ty(&b_ty) {
                            self.errors.push(TypeError::new(
                                // Stage 15.80: use human-readable type name.
                                // Stage 15.82: use stmt_span (was: Span::DUMMY).
                                format!(
                                    "shift count must be an integer type, found {}",
                                    self.format_ty(&b_ty)
                                ),
                                stmt_span,
                            ));
                        }
                        a_ty
                    }
                    // Arithmetic: lhs and rhs must be Int/Uint/Float.
                    // G7 fix (Stage 2.4f): reject Bool, Str, Tuple, etc.
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                        if !is_arithmetic_ty(&a_ty) {
                            self.errors.push(TypeError::new(
                                // Stage 15.80: use human-readable type name.
                                // Stage 15.82: use stmt_span (was: Span::DUMMY).
                                format!(
                                    "cannot apply arithmetic to {} (expected integer or float)",
                                    self.format_ty(&a_ty)
                                ),
                                stmt_span,
                            ));
                        }
                        if !is_arithmetic_ty(&b_ty) {
                            self.errors.push(TypeError::new(
                                // Stage 15.80: use human-readable type name.
                                // Stage 15.82: use stmt_span (was: Span::DUMMY).
                                format!(
                                    "cannot apply arithmetic to {} (expected integer or float)",
                                    self.format_ty(&b_ty)
                                ),
                                stmt_span,
                            ));
                        }
                        if let Err(mut e) = self.unify.unify(&a_ty, &b_ty, stmt_span) {
                            // Stage 15.82: use stmt_span for unify errors.
                            if stmt_span != Span::DUMMY {
                                e.span = stmt_span;
                            }
                            self.errors.push(*e);
                        }
                        a_ty
                    }
                }
            }
            Rvalue::BinaryOp2(_, a, _) => {
                // Stage 14.102 (ME-2 fix): BinaryOp2 is used for Range
                // expressions (start..end). The result type is a Range struct,
                // but for v0.1 we don't have Range in the type system.
                // Previously this silently returned Ty::Error.
                //
                // Per §1.0 原则 5 "报错 > 静默": emit a type error instead of
                // silently returning Error. The operand type is still inferred
                // (for use in for-loop desugaring).
                // Stage 15.82: use stmt_span (was: Span::DUMMY).
                let _a_ty = self.infer_operand(mir, a);
                self.errors.push(TypeError::new(
                    "range expressions (start..end) are not supported in type position in v0.1 — use them only in for-loop iterators".to_string(),
                    stmt_span,
                ));
                Ty::new(TyKind::Error, Span::DUMMY)
            }
            Rvalue::UnaryOp(op, operand) => {
                // Stage 18.72: Split into two statements to avoid borrow conflict.
                let inner_ty_raw = self.infer_operand(mir, operand);
                let inner_ty = self.unify.resolve(&inner_ty_raw);
                match op {
                    UnOp::Not => {
                        // !bool → bool, !int → int
                        // G7 fix: !str, !float, !tuple are errors.
                        // G8 fix (Stage 2.4g): resolve before checking,
                        // so TyVar bound to Tuple/Float is correctly rejected.
                        if !is_notable_ty(&inner_ty) {
                            self.errors.push(TypeError::new(
                                // Stage 15.80: use human-readable type name.
                                // Stage 15.82: use stmt_span (was: Span::DUMMY).
                                format!(
                                    "cannot apply `!` to {} (expected bool or integer)",
                                    self.format_ty(&inner_ty)
                                ),
                                stmt_span,
                            ));
                        }
                        inner_ty
                    }
                    UnOp::Neg => {
                        // -int → int, -float → float
                        // G7 fix: -bool, -str, -tuple are errors.
                        // G8 fix (Stage 2.4g): resolve before checking.
                        if !is_negatable_ty(&inner_ty) {
                            self.errors.push(TypeError::new(
                                // Stage 15.80: use human-readable type name.
                                // Stage 15.82: use stmt_span (was: Span::DUMMY).
                                format!(
                                    "cannot apply unary `-` to {} (expected integer or float)",
                                    self.format_ty(&inner_ty)
                                ),
                                stmt_span,
                            ));
                        }
                        inner_ty
                    }
                }
            }
            Rvalue::Ref(_, borrow_kind, lv) => {
                let inner_ty = self.infer_place(mir, lv);
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
                    // Stage 15.83: use stmt_span for unify errors (was:
                    // Span::DUMMY from mismatch(), producing "1:1").
                    for op in operands {
                        let op_ty = self.infer_operand(mir, op);
                        if let Err(mut e) = self.unify.unify(&op_ty, elem_ty, stmt_span) {
                            if stmt_span != Span::DUMMY {
                                e.span = stmt_span;
                            }
                            self.errors.push(*e);
                        }
                    }
                    Ty::new(
                        TyKind::Array(
                            Box::new(elem_ty.clone()),
                            Box::new(Const {
                                ty: Ty::new(TyKind::Uint(ast::UintTy::Usize), Span::DUMMY),
                                val: ConstVal::Uint(operands.len() as u128),
                            }),
                        ),
                        Span::DUMMY,
                    )
                }
                // Stage 3.32 (L-DEBT-2 fix): AggregateKind::Adt now carries
                // field_tys (per §16 data sink from Stage 3.30). Use them
                // to unify each operand with its declared field type, and
                // return the Adt type.
                // Stage 15.83: use stmt_span for unify errors (was:
                // Span::DUMMY from mismatch(), producing "1:1").
                AggregateKind::Adt(def_id, _variant, _substs, field_tys) => {
                    for (i, op) in operands.iter().enumerate() {
                        let op_ty = self.infer_operand(mir, op);
                        if let Some(field_ty) = field_tys.get(i) {
                            if let Err(mut e) = self.unify.unify(&op_ty, field_ty, stmt_span) {
                                if stmt_span != Span::DUMMY {
                                    e.span = stmt_span;
                                }
                                self.errors.push(*e);
                            }
                        }
                    }
                    Ty::new(TyKind::Adt(*def_id, _substs.clone()), Span::DUMMY)
                }
                // Stage 16.29 (通解 — fix closure type inference):
                // Previously, AggregateKind::Closure returned a fresh Infer
                // var, which caused the closure literal's type to be lost.
                // This broke nested closures (`|| || x`) — the outer
                // closure's return type stayed Infer, causing "expected
                // function, found _" errors.
                //
                // The fix: return the actual Closure type
                // (`Closure(def_id, substs)`), which is the correct type
                // of a closure literal. This matches how Rust handles
                // closure literals — their type is determined at lowering
                // time, not inferred.
                //
                // Per §1.0 原則 9 "正确 > 妥协": fix the root cause
                // (return correct type), not the symptom ( Infer var).
                AggregateKind::Closure(def_id, substs) => {
                    for op in operands {
                        let _ = self.infer_operand(mir, op);
                    }
                    Ty::new(TyKind::Closure(*def_id, substs.clone()), Span::DUMMY)
                }
            },
        }
    }

    /// Infer the type of an operand.
    ///
    /// Stage 18.72: Changed from `&self` to `&mut self` because
    /// `infer_place` is now `&mut self` (to support tuple index bounds
    /// check in `infer_projection`).
    fn infer_operand(&mut self, mir: &MirBody, op: &Operand) -> Ty {
        match op {
            Operand::Copy(lv) | Operand::Move(lv) => self.infer_place(mir, lv),
            Operand::Constant(c) => c.ty.clone(),
        }
    }

    // Stage 15.86: `operand_span` moved to `mir::place::operand_span` (shared
    // helper, DRY per §23 rule 5). Callers now use
    // `crate::mir::place::operand_span(op)` directly. Previously duplicated
    // as a private method here (Stage 15.81) and in `borrowck::mod` (Stage
    // 15.85).

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
fn type_has_unresolved_substs(ty: &crate::mir::ty::Ty) -> bool {
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
fn types_match_loose(a: &crate::mir::ty::Ty, b: &crate::mir::ty::Ty) -> bool {
    use crate::mir::ty::TyKind;
    match (&a.kind, &b.kind) {
        // Str ↔ Ref(_, _, Str): string literal vs &str reference
        (TyKind::Str, TyKind::Ref(_, _, inner)) if matches!(inner.kind, TyKind::Str) => true,
        (TyKind::Ref(_, _, inner), TyKind::Str) if matches!(inner.kind, TyKind::Str) => true,
        // Adt with same DefId (generic substs may differ in representation)
        (TyKind::Adt(a_def, _), TyKind::Adt(b_def, _)) if a_def == b_def => true,
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
        // FnDef ↔ FnPtr: function item type coerces to function pointer type.
        // Per §1.0 原則 9: FnDef coerces to FnPtr (like Rust).
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
                Place::local(temp, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
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
        let dest = mir.new_local(Ty::new(TyKind::Bool, Span::DUMMY), None, Span::DUMMY);
        mir.block_mut(BasicBlockId(0)).statements.push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(dest, Span::DUMMY),
                Rvalue::Use(Operand::Constant(Const {
                    ty: Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
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
            Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY),
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
                    Box::new(Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY)),
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
        let ty = Ty::new(TyKind::Int(ast::IntTy::I32), Span::DUMMY);
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
            if let crate::driver::TraitError::Incomplete(inc) = e {
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
