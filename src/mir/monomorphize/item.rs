//! Stage 16.54 (Task 11 Phase 3): Monomorphization collection — walk MIR
//! bodies and collect `MonoItem { def_id, substs }` pairs for codegen.
//!
//! This module provides the `collect_mono_items` function, which walks all
//! MIR bodies in a crate and collects the set of generic instantiations
//! that need specialized codegen.
//!
//! Per §16: reads MIR only (no HIR access during collection).
//! Per §1.0 原則 6 "通用 > 特例": one collection function for all type kinds.

use crate::hir::DefId;
use crate::mir::body::{MirBody, StatementKind, TerminatorKind};
use crate::mir::place::{AggregateKind, Operand, Place, PlaceKind, ProjectionElem, Rvalue};
use crate::mir::ty::{SubstsRef, Ty, TyKind};
use std::collections::HashSet;

/// A monomorphization item: one specialization of a generic definition.
///
/// Each MonoItem represents one concrete instantiation of a generic type
/// or function. For example, `Vec<i32>` and `Vec<bool>` are two distinct
/// MonoItems with the same `def_id` (Vec) but different `substs`.
///
/// Per §23: `MonoItem` follows `<Noun>_<Noun>` pattern (data type).
/// Per §16: pure data — no behavior, just a key for codegen.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MonoItem {
    /// A monomorphized type: `Vec<i32>`, `Pair<i32, bool>`, etc.
    Type { def_id: DefId, substs: SubstsRef },
    /// A monomorphized function: `fn id<T>(x: T) -> T` called with `i32`.
    Fn { def_id: DefId, substs: SubstsRef },
    /// A monomorphized closure: `Closure<i32>` (closure with i32 captures).
    Closure { def_id: DefId, substs: SubstsRef },
}

impl MonoItem {
    /// Get the DefId of this MonoItem.
    pub fn def_id(&self) -> DefId {
        match self {
            MonoItem::Type { def_id, .. }
            | MonoItem::Fn { def_id, .. }
            | MonoItem::Closure { def_id, .. } => *def_id,
        }
    }

    /// Get the substs of this MonoItem.
    pub fn substs(&self) -> &SubstsRef {
        match self {
            MonoItem::Type { substs, .. }
            | MonoItem::Fn { substs, .. }
            | MonoItem::Closure { substs, .. } => substs,
        }
    }

    /// Format this MonoItem as a human-readable string (for debugging).
    ///
    /// Stage 16.62: Gated behind `#[cfg(test)]` — only used by unit tests.
    /// Per §1.0 原則 5 "去除兼容思维": test-only code shouldn't be in the
    /// public production API.
    #[cfg(test)]
    pub fn debug_string(&self) -> String {
        format!(
            "MonoItem::{}({:?}, {:?})",
            self.kind_str(),
            self.def_id(),
            self.substs()
        )
    }

    #[cfg(test)]
    fn kind_str(&self) -> &'static str {
        match self {
            MonoItem::Type { .. } => "Type",
            MonoItem::Fn { .. } => "Fn",
            MonoItem::Closure { .. } => "Closure",
        }
    }
}

/// Collect all MonoItems from a slice of MIR bodies.
///
/// Walks each MIR body and collects `MonoItem`s from:
/// - Local declarations (local_decls[i].ty)
/// - Statements (Rvalue::Aggregate, Rvalue::Cast)
/// - Terminators (TerminatorKind::Call)
/// - Projection elements (Field types)
///
/// Returns a deduplicated `Vec<MonoItem>`. The order is unspecified
/// (HashSet iteration order).
///
/// Per §23: `collect_mono_items` follows `<verb>_<noun>_<noun>` pattern.
/// Per §16: reads MIR only (no HIR access).
/// Per §1.0 原則 6 "通用 > 特例": one function for all MIR body kinds.
pub fn collect_mono_items(mirs: &[MirBody]) -> Vec<MonoItem> {
    let mut collected: HashSet<MonoItem> = HashSet::new();
    for mir in mirs {
        collect_from_mir_body(mir, &mut collected);
    }
    collected.into_iter().collect()
}

/// Collect MonoItems from a single MIR body.
fn collect_from_mir_body(mir: &MirBody, collected: &mut HashSet<MonoItem>) {
    // 1. Collect from local declarations.
    for local_decl in &mir.local_decls {
        collect_from_ty(&local_decl.ty, collected);
    }

    // 2. Collect from basic blocks (statements + terminators).
    for block in &mir.basic_blocks {
        for stmt in &block.statements {
            collect_from_statement(&stmt.kind, collected);
        }
        collect_from_terminator(&block.terminator.kind, collected);
    }
}

/// Collect MonoItems from a statement.
fn collect_from_statement(stmt: &StatementKind, collected: &mut HashSet<MonoItem>) {
    if let StatementKind::Assign(boxed) = stmt {
        let (_, rvalue) = &**boxed;
        collect_from_rvalue(rvalue, collected);
    }
}

/// Collect MonoItems from a rvalue.
fn collect_from_rvalue(rvalue: &Rvalue, collected: &mut HashSet<MonoItem>) {
    match rvalue {
        Rvalue::Use(operand) | Rvalue::UnaryOp(_, operand) => {
            collect_from_operand(operand, collected);
        }
        Rvalue::BinaryOp(_, a, b) | Rvalue::BinaryOp2(_, a, b) => {
            collect_from_operand(a, collected);
            collect_from_operand(b, collected);
        }
        Rvalue::Ref(_, _, place) => {
            collect_from_place(place, collected);
        }
        Rvalue::Cast(_, operand, ty) => {
            collect_from_operand(operand, collected);
            collect_from_ty(ty, collected);
        }
        Rvalue::Aggregate(kind, operands) => {
            collect_from_aggregate_kind(kind, collected);
            for operand in operands {
                collect_from_operand(operand, collected);
            }
        }
    }
}

/// Collect MonoItems from an aggregate kind.
fn collect_from_aggregate_kind(kind: &AggregateKind, collected: &mut HashSet<MonoItem>) {
    match kind {
        AggregateKind::Array(ty) => collect_from_ty(ty, collected),
        AggregateKind::Adt(def_id, _, substs, field_tys) => {
            // Collect the Adt itself if it has non-empty substs.
            if !substs.is_empty() {
                collected.insert(MonoItem::Type {
                    def_id: *def_id,
                    substs: substs.clone(),
                });
            }
            // Collect from inner substs (e.g., Vec<Vec<i32>> → inner Vec<i32>).
            for inner_ty in substs.iter() {
                collect_from_ty(inner_ty, collected);
            }
            // Collect from field types.
            for field_ty in field_tys {
                collect_from_ty(field_ty, collected);
            }
        }
        AggregateKind::Closure(def_id, substs) => {
            if !substs.is_empty() {
                collected.insert(MonoItem::Closure {
                    def_id: *def_id,
                    substs: substs.clone(),
                });
            }
            for inner_ty in substs.iter() {
                collect_from_ty(inner_ty, collected);
            }
        }
        AggregateKind::Tuple => {}
    }
}

/// Collect MonoItems from an operand.
fn collect_from_operand(operand: &Operand, collected: &mut HashSet<MonoItem>) {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            collect_from_place(place, collected);
        }
        Operand::Constant(const_val) => {
            collect_from_ty(&const_val.ty, collected);
        }
    }
}

/// Collect MonoItems from a place.
fn collect_from_place(place: &Place, collected: &mut HashSet<MonoItem>) {
    match &place.kind {
        PlaceKind::Local(_) | PlaceKind::Static(_) => {}
        PlaceKind::Projection(base, elem) => {
            collect_from_place(base, collected);
            collect_from_projection_elem(elem, collected);
        }
    }
}

/// Collect MonoItems from a projection element.
fn collect_from_projection_elem(elem: &ProjectionElem, collected: &mut HashSet<MonoItem>) {
    match elem {
        ProjectionElem::Field(_, ty) => collect_from_ty(ty, collected),
        ProjectionElem::Deref
        | ProjectionElem::Index(_)
        | ProjectionElem::ConstantIndex { .. }
        | ProjectionElem::Subslice { .. } => {}
    }
}

/// Collect MonoItems from a terminator.
fn collect_from_terminator(term: &TerminatorKind, collected: &mut HashSet<MonoItem>) {
    match term {
        TerminatorKind::Call { func, args, .. } => {
            collect_from_operand(func, collected);
            for arg in args {
                collect_from_operand(arg, collected);
            }
        }
        TerminatorKind::SwitchInt { discr, .. } => {
            collect_from_operand(discr, collected);
        }
        TerminatorKind::Drop { place, .. } => {
            collect_from_place(place, collected);
        }
        TerminatorKind::Assert { cond, .. } => {
            collect_from_operand(cond, collected);
        }
        TerminatorKind::Goto(_) | TerminatorKind::Return | TerminatorKind::Unreachable => {}
    }
}

/// Collect MonoItems from a type.
///
/// This is the core type-walking function. It extracts MonoItems from:
/// - `TyKind::Adt(def_id, substs)` where `!substs.is_empty()` → `MonoItem::Type`
/// - `TyKind::FnDef(def_id, substs)` where `!substs.is_empty()` → `MonoItem::Fn`
/// - `TyKind::Closure(def_id, substs)` where `!substs.is_empty()` → `MonoItem::Closure`
///
/// It also recursively walks inner types (substs, Ref inner, Tuple elements,
/// Array/Slice element, etc.) to find nested MonoItems.
///
/// Per §23: `collect_from_ty` follows `<verb>_<prep>_<noun>` pattern.
/// Per §1.0 原則 6 "通用 > 特例": one function for all type kinds.
pub(crate) fn collect_from_ty(ty: &Ty, collected: &mut HashSet<MonoItem>) {
    match &ty.kind {
        // Generic-capable types — collect if substs are non-empty AND concrete.
        // Stage 18.106 (S7 fix): Skip substs containing Param — those are
        // generic definitions (e.g., `Box<T>` in `fn make_box<T>() -> Box<T>`),
        // not concrete instantiations. Only collect fully-concrete substs
        // (e.g., `Box<i32>` from a call site).
        // Per §1.0 原則 6 "通用 > 特例": one check for all generic-capable types.
        // Per §2.0 原則 9 "正确 > 妥协": don't collect generic definitions.
        TyKind::Adt(def_id, substs) => {
            if !substs.is_empty() && substs_are_concrete(substs) {
                collected.insert(MonoItem::Type {
                    def_id: *def_id,
                    substs: substs.clone(),
                });
            }
            // Recursively collect from inner substs (e.g., Vec<Vec<i32>>).
            for inner_ty in substs.iter() {
                collect_from_ty(inner_ty, collected);
            }
        }
        TyKind::Projection(_def_id, substs) => {
            // Stage 16.67: Collect from projection substs.
            for inner_ty in substs.iter() {
                collect_from_ty(inner_ty, collected);
            }
        }
        TyKind::FnDef(def_id, substs) => {
            if !substs.is_empty() && substs_are_concrete(substs) {
                collected.insert(MonoItem::Fn {
                    def_id: *def_id,
                    substs: substs.clone(),
                });
            }
            for inner_ty in substs.iter() {
                collect_from_ty(inner_ty, collected);
            }
        }
        TyKind::Closure(def_id, substs) => {
            if !substs.is_empty() && substs_are_concrete(substs) {
                collected.insert(MonoItem::Closure {
                    def_id: *def_id,
                    substs: substs.clone(),
                });
            }
            for inner_ty in substs.iter() {
                collect_from_ty(inner_ty, collected);
            }
        }

        // Recursive types — walk inner.
        TyKind::Ref(_, _, inner) => collect_from_ty(inner, collected),
        TyKind::RawPtr(_, inner) => collect_from_ty(inner, collected),
        TyKind::Array(inner, _) => collect_from_ty(inner, collected),
        TyKind::Slice(inner) => collect_from_ty(inner, collected),
        TyKind::Tuple(tys) => {
            for inner_ty in tys {
                collect_from_ty(inner_ty, collected);
            }
        }
        TyKind::FnPtr(sig) => {
            for input_ty in &sig.inputs {
                collect_from_ty(input_ty, collected);
            }
            collect_from_ty(&sig.output, collected);
        }

        // Leaf types — no MonoItems to collect.
        TyKind::Bool
        | TyKind::Char
        | TyKind::Int(_)
        | TyKind::Uint(_)
        | TyKind::Float(_)
        | TyKind::Str
        | TyKind::Never
        | TyKind::Foreign
        | TyKind::Error
        | TyKind::Param(_)
        | TyKind::Infer(_) => {}
    }
}

/// Stage 18.106 (S7 fix): Check if substs are fully concrete (no Param, no Error).
///
/// Returns `true` if no subst contains `TyKind::Param` or `TyKind::Error` —
/// i.e., all substs are concrete types (Int, Bool, Adt with concrete substs, etc.).
/// Returns `false` if any subst contains `Param` (generic definition) or
/// `Error` (unresolved type).
///
/// This prevents collecting generic definitions like `Box<T>` (where substs
/// = `[Param(0)]`) or unresolved types like `Box<Error>` as MonoItems — only
/// concrete instantiations like `Box<i32>` should be collected.
///
/// Per §23: `substs_are_concrete` follows `<noun>_<verb>_<adj>` pattern.
/// Per §1.0 原則 6 "通用 > 特例": one check for all generic-capable types.
fn substs_are_concrete(substs: &SubstsRef) -> bool {
    substs.iter().all(|ty| !type_contains_param_or_error(ty))
}

/// Helper: check if a type contains any `TyKind::Param` or `TyKind::Error` (recursively).
fn type_contains_param_or_error(ty: &Ty) -> bool {
    match &ty.kind {
        TyKind::Param(_) | TyKind::Error => true,
        TyKind::Adt(_, substs) => substs.iter().any(type_contains_param_or_error),
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) | TyKind::Slice(inner) => {
            type_contains_param_or_error(inner)
        }
        TyKind::Array(inner, _) => type_contains_param_or_error(inner),
        TyKind::Tuple(tys) => tys.iter().any(type_contains_param_or_error),
        TyKind::FnDef(_, substs) | TyKind::Closure(_, substs) => {
            substs.iter().any(type_contains_param_or_error)
        }
        TyKind::FnPtr(sig) => {
            sig.inputs.iter().any(type_contains_param_or_error)
                || type_contains_param_or_error(&sig.output)
        }
        TyKind::Projection(_, substs) => substs.iter().any(type_contains_param_or_error),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::IntTy;
    use crate::ast::UintTy;
    use crate::mir::body::{MirBody, Statement, Terminator};
    use crate::mir::place::{LocalId, Place};
    use crate::mir::ty::{ConstVal, Mutability, Region};
    use crate::mir::LocalDecl;
    use crate::session::Span;

    /// Helper: create a Ty of the given kind.
    fn ty(kind: TyKind) -> Ty {
        Ty::new(kind, Span::DUMMY)
    }

    /// Helper: create an i32 Ty.
    fn i32_ty() -> Ty {
        ty(TyKind::Int(IntTy::I32))
    }

    /// Helper: create a bool Ty.
    fn bool_ty() -> Ty {
        ty(TyKind::Bool)
    }

    /// Helper: create an Adt Ty with substs.
    fn adt_ty(def_id: u32, substs: Vec<Ty>) -> Ty {
        ty(TyKind::Adt(DefId::new(def_id), substs.into()))
    }

    /// Helper: create an empty MirBody.
    fn empty_mir() -> MirBody {
        MirBody::new(Span::DUMMY)
    }

    // =================================================================
    // §1. MonoItem struct tests
    // =================================================================

    #[test]
    fn stage16_54_mono_item_type_def_id() {
        let item = MonoItem::Type {
            def_id: DefId::new(1),
            substs: vec![i32_ty()].into(),
        };
        assert_eq!(item.def_id(), DefId::new(1));
    }

    #[test]
    fn stage16_54_mono_item_fn_def_id() {
        let item = MonoItem::Fn {
            def_id: DefId::new(2),
            substs: vec![i32_ty()].into(),
        };
        assert_eq!(item.def_id(), DefId::new(2));
    }

    #[test]
    fn stage16_54_mono_item_closure_def_id() {
        let item = MonoItem::Closure {
            def_id: DefId::new(3),
            substs: vec![i32_ty()].into(),
        };
        assert_eq!(item.def_id(), DefId::new(3));
    }

    #[test]
    fn stage16_54_mono_item_equality() {
        let item1 = MonoItem::Type {
            def_id: DefId::new(1),
            substs: vec![i32_ty()].into(),
        };
        let item2 = MonoItem::Type {
            def_id: DefId::new(1),
            substs: vec![i32_ty()].into(),
        };
        assert_eq!(item1, item2);
    }

    #[test]
    fn stage16_54_mono_item_inequality_different_substs() {
        let item1 = MonoItem::Type {
            def_id: DefId::new(1),
            substs: vec![i32_ty()].into(),
        };
        let item2 = MonoItem::Type {
            def_id: DefId::new(1),
            substs: vec![bool_ty()].into(),
        };
        assert_ne!(item1, item2);
    }

    // =================================================================
    // §2. collect_from_ty tests
    // =================================================================

    #[test]
    fn stage16_54_collect_from_adt_with_substs() {
        let mut collected = HashSet::new();
        let t = adt_ty(1, vec![i32_ty()]);
        collect_from_ty(&t, &mut collected);
        assert_eq!(collected.len(), 1);
        assert!(collected.iter().any(|m| matches!(
            m,
            MonoItem::Type { def_id, substs } if *def_id == DefId::new(1) && substs.len() == 1
        )));
    }

    #[test]
    fn stage16_54_collect_from_adt_empty_substs() {
        let mut collected = HashSet::new();
        let t = adt_ty(1, vec![]);
        collect_from_ty(&t, &mut collected);
        assert!(collected.is_empty());
    }

    #[test]
    fn stage16_54_collect_from_nested_adt() {
        let mut collected = HashSet::new();
        // Vec<Vec<i32>> — outer Adt(1, [Adt(1, [i32])])
        let inner = adt_ty(1, vec![i32_ty()]);
        let outer = adt_ty(1, vec![inner]);
        collect_from_ty(&outer, &mut collected);
        // Should collect both outer and inner
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn stage16_54_collect_from_ref_adt() {
        let mut collected = HashSet::new();
        let inner = adt_ty(1, vec![i32_ty()]);
        let ref_ty = ty(TyKind::Ref(
            Region::Static,
            Mutability::Immutable,
            Box::new(inner),
        ));
        collect_from_ty(&ref_ty, &mut collected);
        assert_eq!(collected.len(), 1);
    }

    #[test]
    fn stage16_54_collect_from_tuple_of_adts() {
        let mut collected = HashSet::new();
        let adt1 = adt_ty(1, vec![i32_ty()]);
        let adt2 = adt_ty(2, vec![bool_ty()]);
        let tuple = ty(TyKind::Tuple(vec![adt1, adt2]));
        collect_from_ty(&tuple, &mut collected);
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn stage16_54_collect_from_fn_def() {
        let mut collected = HashSet::new();
        let t = ty(TyKind::FnDef(DefId::new(5), vec![i32_ty()].into()));
        collect_from_ty(&t, &mut collected);
        assert_eq!(collected.len(), 1);
        assert!(collected.iter().any(|m| matches!(m, MonoItem::Fn { .. })));
    }

    #[test]
    fn stage16_54_collect_from_closure() {
        let mut collected = HashSet::new();
        let t = ty(TyKind::Closure(DefId::new(7), vec![i32_ty()].into()));
        collect_from_ty(&t, &mut collected);
        assert_eq!(collected.len(), 1);
        assert!(collected
            .iter()
            .any(|m| matches!(m, MonoItem::Closure { .. })));
    }

    #[test]
    fn stage16_54_collect_from_leaf_types() {
        let mut collected = HashSet::new();
        collect_from_ty(&i32_ty(), &mut collected);
        collect_from_ty(&bool_ty(), &mut collected);
        collect_from_ty(&ty(TyKind::Str), &mut collected);
        collect_from_ty(&ty(TyKind::Never), &mut collected);
        collect_from_ty(&ty(TyKind::Error), &mut collected);
        assert!(collected.is_empty());
    }

    // =================================================================
    // §3. collect_mono_items tests
    // =================================================================

    #[test]
    fn stage16_54_collect_empty_mirs() {
        let items = collect_mono_items(&[]);
        assert!(items.is_empty());
    }

    #[test]
    fn stage16_54_collect_from_local_decls() {
        let mut mir = empty_mir();
        // Add two locals with Adt types.
        mir.local_decls.push(LocalDecl {
            ty: adt_ty(1, vec![i32_ty()]),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });
        mir.local_decls.push(LocalDecl {
            ty: adt_ty(2, vec![bool_ty()]),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });

        let items = collect_mono_items(std::slice::from_ref(&mir));
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn stage16_54_collect_dedup() {
        let mut mir = empty_mir();
        // Add two locals with the SAME Adt type — should dedup to 1.
        mir.local_decls.push(LocalDecl {
            ty: adt_ty(1, vec![i32_ty()]),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });
        mir.local_decls.push(LocalDecl {
            ty: adt_ty(1, vec![i32_ty()]),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });

        let items = collect_mono_items(std::slice::from_ref(&mir));
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn stage16_54_collect_multiple_mirs() {
        let mut mir1 = empty_mir();
        mir1.local_decls.push(LocalDecl {
            ty: adt_ty(1, vec![i32_ty()]),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });
        let mut mir2 = empty_mir();
        mir2.local_decls.push(LocalDecl {
            ty: adt_ty(2, vec![bool_ty()]),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });

        let items = collect_mono_items(&[mir1, mir2]);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn stage16_54_collect_across_mirs_dedup() {
        let mut mir1 = empty_mir();
        mir1.local_decls.push(LocalDecl {
            ty: adt_ty(1, vec![i32_ty()]),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });
        let mut mir2 = empty_mir();
        mir2.local_decls.push(LocalDecl {
            ty: adt_ty(1, vec![i32_ty()]),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });

        let items = collect_mono_items(&[mir1, mir2]);
        assert_eq!(items.len(), 1);
    }

    // =================================================================
    // §4. collect_from_statement / rvalue / terminator tests
    // =================================================================

    #[test]
    fn stage16_54_collect_from_aggregate_statement() {
        let mut mir = empty_mir();
        let block_id = mir.new_block();
        // Add a statement: Assign(local_0, Aggregate(Adt(1, [i32]), []))
        mir.basic_blocks[block_id.0 as usize]
            .statements
            .push(Statement {
                kind: StatementKind::Assign(Box::new((
                    Place::local(LocalId(0), Span::DUMMY),
                    Rvalue::Aggregate(
                        AggregateKind::Adt(DefId::new(1), 0, vec![i32_ty()].into(), vec![]),
                        vec![],
                    ),
                ))),
                span: Span::DUMMY,
            });

        let items = collect_mono_items(std::slice::from_ref(&mir));
        assert_eq!(items.len(), 1);
        assert!(items.iter().any(|m| matches!(
            m,
            MonoItem::Type { def_id, .. } if *def_id == DefId::new(1)
        )));
    }

    #[test]
    fn stage16_54_collect_from_cast_statement() {
        let mut mir = empty_mir();
        let block_id = mir.new_block();
        let const_val = crate::mir::ty::Const {
            ty: i32_ty(),
            val: ConstVal::Int(42),
        };
        mir.basic_blocks[block_id.0 as usize]
            .statements
            .push(Statement {
                kind: StatementKind::Assign(Box::new((
                    Place::local(LocalId(0), Span::DUMMY),
                    Rvalue::Cast(
                        crate::mir::place::CastKind::Numeric,
                        Operand::Constant(const_val),
                        adt_ty(1, vec![i32_ty()]),
                    ),
                ))),
                span: Span::DUMMY,
            });

        let items = collect_mono_items(std::slice::from_ref(&mir));
        // Should collect from both the operand's type (i32 — leaf, no item)
        // and the cast target type (Adt(1, [i32])).
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn stage16_54_collect_from_call_terminator() {
        let mut mir = empty_mir();
        let block_id = mir.new_block();
        // Add a Call terminator with a FnDef func.
        let func_ty = ty(TyKind::FnDef(DefId::new(5), vec![i32_ty()].into()));
        let func_operand = Operand::Constant(crate::mir::ty::Const {
            ty: func_ty,
            val: ConstVal::Uint(5),
        });
        mir.basic_blocks[block_id.0 as usize].terminator = Terminator {
            kind: TerminatorKind::Call {
                func: func_operand,
                args: vec![],
                destination: Place::local(LocalId(0), Span::DUMMY),
                target: None,
                dyn_trait_call: None,
            },
            span: Span::DUMMY,
        };

        let items = collect_mono_items(std::slice::from_ref(&mir));
        assert_eq!(items.len(), 1);
        assert!(items.iter().any(|m| matches!(m, MonoItem::Fn { .. })));
    }

    #[test]
    fn stage16_54_collect_from_array_aggregate() {
        let mut mir = empty_mir();
        let block_id = mir.new_block();
        // Add a statement: Assign(local_0, Aggregate(Array(Adt(1, [i32])), []))
        mir.basic_blocks[block_id.0 as usize]
            .statements
            .push(Statement {
                kind: StatementKind::Assign(Box::new((
                    Place::local(LocalId(0), Span::DUMMY),
                    Rvalue::Aggregate(AggregateKind::Array(adt_ty(1, vec![i32_ty()])), vec![]),
                ))),
                span: Span::DUMMY,
            });

        let items = collect_mono_items(std::slice::from_ref(&mir));
        assert_eq!(items.len(), 1);
    }

    // =================================================================
    // §5. Complex scenarios
    // =================================================================

    #[test]
    fn stage16_54_collect_mixed_types() {
        let mut mir = empty_mir();
        // Local 1: Adt(1, [i32])
        mir.local_decls.push(LocalDecl {
            ty: adt_ty(1, vec![i32_ty()]),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });
        // Local 2: Adt(2, [bool, Adt(1, [u64])])  — nested
        mir.local_decls.push(LocalDecl {
            ty: adt_ty(
                2,
                vec![bool_ty(), adt_ty(1, vec![ty(TyKind::Uint(UintTy::U64))])],
            ),
            name: None,
            mutability: Mutability::Immutable,
            source_info: Span::DUMMY,
        });

        let items = collect_mono_items(std::slice::from_ref(&mir));
        // Should collect: Adt(1, [i32]), Adt(2, [bool, Adt(1, [u64])]), Adt(1, [u64])
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn stage16_54_debug_string() {
        let item = MonoItem::Type {
            def_id: DefId::new(1),
            substs: vec![i32_ty()].into(),
        };
        let s = item.debug_string();
        assert!(s.contains("Type"));
        assert!(s.contains("DefId"));
    }
}
