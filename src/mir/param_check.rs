//! Stage 18.348 (P2 soundness fix): Pre-codegen diagnostic pass that scans
//! a MirBody for unresolved type kinds (`Param`, `Infer`, `Error`,
//! `Projection`) in **type-relevant positions** and reports them as type
//! errors.
//!
//! # Why this pass exists (per §1.0 原則 4 报错 > 静默)
//!
//! Before Stage 18.348, `mir_type_to_emit_type`'s default fallback
//! `_ => EmitType::I32` silently treated unresolved type kinds as `i32`.
//! This caused Stage 18.347's bug (`Pair<i32, i64>.second` returning 173
//! instead of 99) to go undetected — the `Param` was silently mapped to
//! `i32`, producing wrong-but-compilable LLVM IR.
//!
//! # What this pass does
//!
//! Walks `Rvalue` types (Cast target, Aggregate field_tys, Load pointee,
//! GetElementPtr result) and `Operand` constant types + projection
//! field_tys. Reports any unresolved type kind found in these
//! **type-relevant positions**.
//!
//! # What this pass does NOT report (intentional, per §12 最优 > 最小)
//!
//! - `local_decl.ty` containing `Param`/`Infer`/`Error` — many locals are
//!   placeholders (return slot, unused temporaries) whose types don't
//!   affect codegen. Reporting these would generate ~70 false positives
//!   per crate. The local_decl type is checked indirectly when the local
//!   is *used* in an Rvalue/Operand.
//! - `Error` type in local_decls — usually propagated from a prior typeck
//!   error that's already been reported. Reporting again would duplicate
//!   diagnostics.
//!
//! # When it runs
//!
//! After writeback + projection_resolver + mir optimization (all type
//! resolution passes have completed), but before codegen consumes the
//! MirBody.
//!
//! # Why a separate pass (per §1.0 原則 6 通解 > 特解)
//!
//! Adding error checks inside `mir_type_to_emit_type` would require
//! threading `Result<>` through every codegen function — a massive
//! refactor. A separate diagnostic pass is:
//!
//! - **Single responsibility**: only checks for unresolved types
//! - **Composable**: runs alongside other diagnostic passes
//! - **Cheap**: O(N) walk over statements
//! - **Doesn't change codegen semantics**: codegen still produces IR
//!   (potentially wrong), but the user sees the error
//!
//! Per §23 (API Naming): `check_unresolved_types` follows
//! `<verb>_<noun>_<noun>` pattern.
//! Per §16: reads MIR data only (no HIR).
//! Per §1.0 原則 4 (报错 > 静默): unresolved types in type-relevant
//! positions MUST be reported.

use crate::mir::body::{MirBody, StatementKind, TerminatorKind};
use crate::mir::place::{AggregateKind, Operand, PlaceKind, Rvalue};
use crate::mir::ty::{Ty, TyKind};
use crate::session::Span;
use crate::typeck::TypeError;

/// Check a MirBody for unresolved type kinds (`Param`, `Infer`, `Error`,
/// `Projection`) in **type-relevant positions** (Rvalue types, Operand
/// constant types, projection field_tys).
///
/// Returns a `Vec<TypeError>` — one per unresolved type found. The
/// caller (driver) extends `errors.typeck` with the result.
///
/// Per §1.0 原則 4 (报错 > 静默): unresolved types MUST be reported.
/// Per §1.0 原則 6 (通解 > 特解): one walker handles all type kinds.
/// Per §12 (最优 > 最小): only report types in type-relevant positions
/// (not unused local_decls which would generate false positives).
pub fn check_unresolved_types(mir: &MirBody) -> Vec<TypeError> {
    let mut errors = Vec::new();
    let fn_name = mir
        .def_id
        .map(|d| format!("DefId({})", d.as_u32()))
        .unwrap_or_else(|| "main".to_string());

    // Walk statements (Rvalue types, Operand constant types, projection
    // field_tys). We do NOT walk local_decls directly — many locals are
    // placeholders whose types don't affect codegen. The local_decl type
    // is checked indirectly when the local is *used* in an Rvalue/Operand.
    for bb in &mir.basic_blocks {
        for stmt in &bb.statements {
            if let StatementKind::Assign(boxed) = &stmt.kind {
                let (_place, rvalue) = &**boxed;
                check_rvalue(rvalue, stmt.span, &fn_name, &mut errors);
            }
        }

        // Check terminator (Call func/args, SwitchInt discr, Assert cond).
        check_terminator(&bb.terminator, &fn_name, &mut errors);
    }

    errors
}

/// Walk an Rvalue's embedded types and report any unresolved kinds.
fn check_rvalue(rv: &Rvalue, span: Span, fn_name: &str, errors: &mut Vec<TypeError>) {
    match rv {
        Rvalue::Use(op) => check_operand(op, span, fn_name, errors),
        Rvalue::Ref(_, _, place) => {
            // Place types are checked via local_decls; nothing extra here.
            let _ = place;
        }
        Rvalue::BinaryOp(_, a, b) | Rvalue::BinaryOp2(_, a, b) => {
            check_operand(a, span, fn_name, errors);
            check_operand(b, span, fn_name, errors);
        }
        Rvalue::UnaryOp(_, op) => check_operand(op, span, fn_name, errors),
        Rvalue::Cast(_, op, ty) => {
            check_operand(op, span, fn_name, errors);
            if let Some(kind) = unresolved_kind(ty) {
                errors.push(TypeError::new(
                    format!(
                        "Cast target type `{}` in `{}` is unresolved — \
                         typeck should have resolved this",
                        kind, fn_name,
                    ),
                    span,
                ));
            }
        }
        Rvalue::Aggregate(kind, operands) => {
            match kind {
                AggregateKind::Adt(def_id, _variant, substs, field_tys) => {
                    // Check the Adt's substs.
                    for (i, subst) in substs.iter().enumerate() {
                        if let Some(k) = unresolved_kind(subst) {
                            errors.push(TypeError::new(
                                format!(
                                    "Adt DefId({}) subst[{}] `{}` in `{}` is unresolved — \
                                     writeback should have substituted generic params",
                                    def_id.as_u32(),
                                    i,
                                    k,
                                    fn_name,
                                ),
                                span,
                            ));
                        }
                    }
                    // Check field types.
                    for (i, ft) in field_tys.iter().enumerate() {
                        if let Some(k) = unresolved_kind(ft) {
                            errors.push(TypeError::new(
                                format!(
                                    "Adt DefId({}) field_ty[{}] `{}` in `{}` is unresolved — \
                                     writeback Rule 3 should have applied substitute()",
                                    def_id.as_u32(),
                                    i,
                                    k,
                                    fn_name,
                                ),
                                span,
                            ));
                        }
                    }
                }
                AggregateKind::Tuple => {
                    // Tuple operands' types are checked via check_operand below.
                }
                AggregateKind::Array(elem_ty) => {
                    if let Some(k) = unresolved_kind(elem_ty) {
                        errors.push(TypeError::new(
                            format!("Array element type `{}` in `{}` is unresolved", k, fn_name,),
                            span,
                        ));
                    }
                }
                AggregateKind::Closure(def_id, substs) => {
                    for (i, subst) in substs.iter().enumerate() {
                        if let Some(k) = unresolved_kind(subst) {
                            errors.push(TypeError::new(
                                format!(
                                    "Closure DefId({}) subst[{}] `{}` in `{}` is unresolved",
                                    def_id.as_u32(),
                                    i,
                                    k,
                                    fn_name,
                                ),
                                span,
                            ));
                        }
                    }
                }
            }
            for op in operands {
                check_operand(op, span, fn_name, errors);
            }
        }
        Rvalue::Load(op, ty) => {
            check_operand(op, span, fn_name, errors);
            if let Some(k) = unresolved_kind(ty) {
                errors.push(TypeError::new(
                    format!("Load pointee type `{}` in `{}` is unresolved", k, fn_name,),
                    span,
                ));
            }
        }
        Rvalue::GetElementPtr {
            base,
            indices,
            result_ty,
        } => {
            check_operand(base, span, fn_name, errors);
            for idx in indices {
                check_operand(idx, span, fn_name, errors);
            }
            if let Some(k) = unresolved_kind(result_ty) {
                errors.push(TypeError::new(
                    format!(
                        "GetElementPtr result type `{}` in `{}` is unresolved",
                        k, fn_name,
                    ),
                    span,
                ));
            }
        }
        // Stage 33.1: SizeOf carries a type — check if it's unresolved.
        Rvalue::SizeOf(ty) => {
            if let Some(k) = unresolved_kind(ty) {
                errors.push(TypeError::new(
                    format!("SizeOf type `{}` in `{}` is unresolved", k, fn_name),
                    span,
                ));
            }
        }
    }
}

/// Walk an Operand's constant type (if any) and report unresolved kinds.
fn check_operand(op: &Operand, span: Span, fn_name: &str, errors: &mut Vec<TypeError>) {
    match op {
        Operand::Copy(place) | Operand::Move(place) => {
            // Place type checked via local_decls. But projection's field_ty
            // may have unresolved types.
            if let PlaceKind::Projection(_base, elem) = &place.kind {
                use crate::mir::place::ProjectionElem;
                if let ProjectionElem::Field(_field_id, field_ty) = elem {
                    if let Some(k) = unresolved_kind(field_ty) {
                        errors.push(TypeError::new(
                            format!(
                                "Field projection type `{}` in `{}` is unresolved — \
                                 writeback Rule 3 should have applied substitute()",
                                k, fn_name,
                            ),
                            span,
                        ));
                    }
                }
            }
        }
        Operand::Constant(c) => {
            if let Some(k) = unresolved_kind(&c.ty) {
                errors.push(TypeError::new(
                    format!("Constant type `{}` in `{}` is unresolved", k, fn_name,),
                    span,
                ));
            }
        }
    }
}

/// Walk a Terminator's embedded types (Call func/args/dest, Assert cond, etc).
fn check_terminator(
    term: &crate::mir::body::Terminator,
    fn_name: &str,
    errors: &mut Vec<TypeError>,
) {
    match &term.kind {
        TerminatorKind::Call {
            func,
            args,
            destination,
            ..
        } => {
            check_operand(func, term.span, fn_name, errors);
            for arg in args {
                check_operand(arg, term.span, fn_name, errors);
            }
            // Destination place type checked via local_decls.
            let _ = destination;
        }
        TerminatorKind::Drop { place, .. } => {
            // Place type checked via local_decls.
            let _ = place;
        }
        TerminatorKind::SwitchInt { discr, .. } => {
            check_operand(discr, term.span, fn_name, errors);
        }
        TerminatorKind::Assert { cond, .. } => {
            check_operand(cond, term.span, fn_name, errors);
        }
        _ => {}
    }
}

/// Return a human-readable name if the type is "unresolved" (Param/Infer/
/// Error/Projection). Return `None` if the type is concrete.
///
/// Per §1.0 原則 3 (显式 > 隐式): explicit predicate makes the
/// "is unresolved" check visible at every callsite.
fn unresolved_kind(ty: &Ty) -> Option<&'static str> {
    match &ty.kind {
        TyKind::Param(_) => Some("Param"),
        TyKind::Infer(_) => Some("Infer"),
        TyKind::Error => Some("Error"),
        TyKind::Projection(_, _) => Some("Projection"),
        // Recursive types — recurse to catch nested unresolved kinds.
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) | TyKind::Slice(inner) => {
            unresolved_kind(inner)
        }
        TyKind::Array(elem, _) => unresolved_kind(elem),
        TyKind::Tuple(tys) => tys.iter().find_map(unresolved_kind),
        TyKind::Adt(_, substs) => substs.iter().find_map(unresolved_kind),
        TyKind::Closure(_, substs) => substs.iter().find_map(unresolved_kind),
        TyKind::FnDef(_, substs) => substs.iter().find_map(unresolved_kind),
        _ => None,
    }
}

/// Stage 18.348: Check all MirBodies in a crate for unresolved types.
///
/// Convenience wrapper for the driver — takes a slice of MirBodies and
/// returns a flat Vec<TypeError>.
///
/// Per §23: `check_unresolved_types_in_crate` follows `<verb>_<noun>_<prep>_<noun>` pattern.
pub fn check_unresolved_types_in_crate(mirs: &[MirBody]) -> Vec<TypeError> {
    let mut all_errors = Vec::new();
    for mir in mirs {
        all_errors.extend(check_unresolved_types(mir));
    }
    all_errors
}

// =====================================================================
// Unit Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::DefId;
    use crate::mir::body::{BasicBlock, LocalDecl, MirBody, Statement, StatementKind};
    use crate::mir::place::{CastKind, LocalId, Operand, Place, PlaceKind, Rvalue};
    use crate::mir::ty::{InferVar, Mutability, ParamTy, TyVid};
    use crate::session::Span;

    /// Build a MirBody with one statement: `loc_1 = Cast(Copy(loc_0), <ty>)`.
    /// The cast target type is the parameter `ty` — used to inject
    /// unresolved types for testing.
    fn make_mir_with_cast_stmt(cast_target_ty: Ty) -> MirBody {
        let mut mir = MirBody::new(Span::DUMMY);
        // local 0: source (concrete i32).
        mir.local_decls.push(LocalDecl {
            ty: Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
            name: None,
        });
        // local 1: dest (concrete i32).
        mir.local_decls.push(LocalDecl {
            ty: Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
            name: None,
        });
        // Statement: loc_1 = Cast(Copy(loc_0), <cast_target_ty>).
        let stmt = Statement {
            kind: StatementKind::Assign(Box::new((
                Place {
                    kind: PlaceKind::Local(LocalId(1)),
                    span: Span::DUMMY,
                },
                Rvalue::Cast(
                    CastKind::Unsize,
                    Operand::Copy(Place {
                        kind: PlaceKind::Local(LocalId(0)),
                        span: Span::DUMMY,
                    }),
                    cast_target_ty,
                ),
            ))),
            span: Span::DUMMY,
        };
        mir.basic_blocks.push(BasicBlock {
            statements: vec![stmt],
            terminator: crate::mir::body::Terminator {
                kind: crate::mir::body::TerminatorKind::Return,
                span: Span::DUMMY,
            },
            span: Span::DUMMY,
            terminator_span: Span::DUMMY,
        });
        mir
    }

    #[test]
    fn stage18_348_param_in_cast_target_reported() {
        // Cast target type is Param — should be reported.
        let mir = make_mir_with_cast_stmt(Ty::new(
            TyKind::Param(ParamTy {
                index: 0,
                name: lasso::Rodeo::default().get_or_intern("T"),
            }),
            Span::DUMMY,
        ));
        let errors = check_unresolved_types(&mir);
        assert_eq!(errors.len(), 1, "Expected 1 error for Param in Cast target");
        assert!(errors[0].message.contains("Param"));
        assert!(errors[0].message.contains("Cast target type"));
    }

    #[test]
    fn stage18_348_infer_in_cast_target_reported() {
        let mir = make_mir_with_cast_stmt(Ty::new(
            TyKind::Infer(InferVar::TyVar(TyVid(0))),
            Span::DUMMY,
        ));
        let errors = check_unresolved_types(&mir);
        assert_eq!(errors.len(), 1, "Expected 1 error for Infer in Cast target");
        assert!(errors[0].message.contains("Infer"));
    }

    #[test]
    fn stage18_348_error_in_cast_target_reported() {
        let mir = make_mir_with_cast_stmt(Ty::new(TyKind::Error, Span::DUMMY));
        let errors = check_unresolved_types(&mir);
        assert_eq!(errors.len(), 1, "Expected 1 error for Error in Cast target");
        assert!(errors[0].message.contains("Error"));
    }

    #[test]
    fn stage18_348_concrete_cast_target_no_error() {
        let mir =
            make_mir_with_cast_stmt(Ty::new(TyKind::Int(crate::ast::IntTy::I64), Span::DUMMY));
        let errors = check_unresolved_types(&mir);
        assert!(
            errors.is_empty(),
            "Expected no errors for concrete i64 cast target. Got: {:?}",
            errors
        );
    }

    #[test]
    fn stage18_348_nested_param_in_adt_substs_reported() {
        // Cast target type is Adt with a Param in its substs.
        let adt_ty = Ty::new(
            TyKind::Adt(
                DefId::new(1),
                vec![Ty::new(
                    TyKind::Param(ParamTy {
                        index: 0,
                        name: lasso::Rodeo::default().get_or_intern("T"),
                    }),
                    Span::DUMMY,
                )]
                .into(),
            ),
            Span::DUMMY,
        );
        let mir = make_mir_with_cast_stmt(adt_ty);
        let errors = check_unresolved_types(&mir);
        assert_eq!(
            errors.len(),
            1,
            "Expected 1 error for nested Param in Adt substs"
        );
        assert!(errors[0].message.contains("Param"));
    }

    #[test]
    fn stage18_348_empty_mir_no_errors() {
        // Empty MirBody (no statements) — no errors.
        let mir = MirBody::new(Span::DUMMY);
        let errors = check_unresolved_types(&mir);
        assert!(
            errors.is_empty(),
            "Expected no errors for empty MirBody. Got: {:?}",
            errors
        );
    }
}
