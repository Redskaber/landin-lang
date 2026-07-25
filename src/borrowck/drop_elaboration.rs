//! Stage 8.4: Drop elaboration (§5).
//!
//! Per `docs/lang-design/04-ownership-borrowing.md` §5 (Drop check).
//! Per `docs/stage-committee-process.md` v3.21 §13.4 + §14.4.
//!
//! Drop elaboration inserts `Terminator::Drop` at scope ends for locals
//! that need destruction. The rules (§5.4):
//! 1. Local variables: dropped in reverse declaration order
//! 2. Struct fields: dropped in reverse declaration order
//! 3. Match arm bindings: dropped at arm block end
//!
//! This module provides the `DropElaborator` that walks MIR and identifies
//! where drops need to be inserted. The actual insertion is done by the
//! MIR lowering pass (future integration).

use crate::hir::DefId;
use crate::mir::body::{BasicBlockId, MirBody, Terminator};
use crate::mir::place::LocalId;
use crate::mir::ty::{Ty, TyKind};

/// A drop elaboration result — identifies locals that need to be dropped
/// at the end of a basic block.
#[derive(Debug, Clone)]
pub(crate) struct DropSet {
    /// Local IDs that need to be dropped, in **reverse** declaration order
    /// (per §5.4: "局部变量：按声明顺序逆序析构").
    pub locals: Vec<LocalId>,
}

/// The drop elaborator.
///
/// Walks MIR bodies and identifies where `Terminator::Drop` should be
/// inserted. Currently provides analysis only — actual drop insertion
/// is a future integration point.
///
/// Per §23: `DropElaborator` follows `<noun>_<noun>` (-er suffix) pattern.
#[derive(Debug, Clone)]
pub(crate) struct DropElaborator {
    /// Set of DefIds that have `impl Drop` — these need actual drop calls.
    /// Others (Copy types, primitives) are no-ops.
    drop_impls: Vec<DefId>,
}

impl Default for DropElaborator {
    fn default() -> Self {
        Self::new()
    }
}

impl DropElaborator {
    /// Create a new drop elaborator.
    pub(crate) fn new() -> Self {
        Self {
            drop_impls: Vec::new(),
        }
    }

    /// Register a type that has `impl Drop`.
    ///
    /// Per §16: this data is provided by TraitResolver (data flows downstream).
    pub(crate) fn register_drop_impl(&mut self, def_id: DefId) {
        self.drop_impls.push(def_id);
    }

    /// Check if a type needs destruction (has `impl Drop`).
    ///
    /// Per §5.4: only types with `impl Drop` need actual drop calls.
    /// Copy types and primitives are no-ops.
    pub(crate) fn needs_drop(&self, ty: &Ty) -> bool {
        match &ty.kind {
            // Primitives — never need drop
            TyKind::Bool | TyKind::Char | TyKind::Int(_) | TyKind::Uint(_) | TyKind::Float(_) => {
                false
            }
            TyKind::Never | TyKind::Error | TyKind::Infer(_) => false,
            // References — never need drop (the referent is owned elsewhere)
            TyKind::Ref(_, _, _) | TyKind::RawPtr(_, _) => false,
            // FnDef/FnPtr — never need drop
            TyKind::FnDef(_, _) | TyKind::FnPtr(_) => false,
            // Str/Slice — unsized, never directly owned
            TyKind::Str | TyKind::Slice(_) => false,
            // Array — needs drop if element type needs drop
            TyKind::Array(inner, _) => self.needs_drop(inner),
            // Tuple — needs drop if any element needs drop
            TyKind::Tuple(tys) => tys.iter().any(|t| self.needs_drop(t)),
            // Adt — needs drop if registered as having impl Drop
            TyKind::Adt(def_id, _) => self.drop_impls.contains(def_id),
            // Closure — needs drop if any capture needs drop
            TyKind::Closure(_, substs) => substs.iter().any(|t| self.needs_drop(t)),
            // Param — conservatively true (might be a Drop type)
            TyKind::Param(_) => true,
            // Foreign — conservatively true
            TyKind::Foreign => true,
        }
    }

    /// Compute the drop set for a basic block — locals that need to be
    /// dropped when the block exits via `Return` or `Unreachable`.
    ///
    /// Per §5.4: locals are dropped in **reverse** declaration order.
    pub(crate) fn compute_drop_set(&self, mir: &MirBody, _bb_id: BasicBlockId) -> DropSet {
        let mut locals_to_drop: Vec<LocalId> = Vec::new();

        // Walk all locals (skip LocalId(0) = return value — not dropped)
        for (idx, local_decl) in mir.local_decls.iter().enumerate() {
            if idx == 0 {
                continue; // Return local — not dropped
            }
            if self.needs_drop(&local_decl.ty) {
                locals_to_drop.push(LocalId(idx as u32));
            }
        }

        // Reverse order (§5.4: "按声明顺序逆序析构")
        locals_to_drop.reverse();

        DropSet {
            locals: locals_to_drop,
        }
    }

    /// Elaborate drops for a MIR body.
    ///
    /// This is the main entry point. It walks all basic blocks and identifies
    /// where `Terminator::Drop` should be inserted.
    ///
    /// Currently analysis-only: returns the drop sets without modifying MIR.
    /// Future integration will insert actual `Terminator::Drop` terminators.
    pub(crate) fn elaborate(&self, mir: &MirBody) -> Vec<(BasicBlockId, DropSet)> {
        let mut results = Vec::new();

        for (bb_idx, _bb) in mir.basic_blocks.iter().enumerate() {
            let bb_id = BasicBlockId(bb_idx as u32);
            let drop_set = self.compute_drop_set(mir, bb_id);

            // Only include blocks that have drops
            if !drop_set.locals.is_empty() {
                // Check if the block's terminator is Return (where drops happen)
                let terminator = &mir.basic_blocks[bb_idx].terminator;
                if matches!(terminator, Terminator::Return) {
                    results.push((bb_id, drop_set));
                }
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::body::MirBody;
    use crate::mir::ty::{Ty, TyKind};
    use crate::session::Span;

    #[test]
    fn test_needs_drop_primitive() {
        let elaborator = DropElaborator::new();
        let ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY);
        assert!(!elaborator.needs_drop(&ty));
    }

    #[test]
    fn test_needs_drop_bool() {
        let elaborator = DropElaborator::new();
        let ty = Ty::new(TyKind::Bool, Span::DUMMY);
        assert!(!elaborator.needs_drop(&ty));
    }

    #[test]
    fn test_needs_drop_ref() {
        let elaborator = DropElaborator::new();
        let ty = Ty::new(
            TyKind::Ref(
                crate::mir::ty::Region::Erased,
                crate::mir::ty::Mutability::Immutable,
                Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
            ),
            Span::DUMMY,
        );
        assert!(!elaborator.needs_drop(&ty));
    }

    #[test]
    fn test_needs_drop_adt_without_impl() {
        let elaborator = DropElaborator::new();
        let ty = Ty::new(TyKind::Adt(crate::hir::DefId(42), vec![]), Span::DUMMY);
        // No Drop impl registered → doesn't need drop
        assert!(!elaborator.needs_drop(&ty));
    }

    #[test]
    fn test_needs_drop_adt_with_impl() {
        let mut elaborator = DropElaborator::new();
        elaborator.register_drop_impl(crate::hir::DefId(42));
        let ty = Ty::new(TyKind::Adt(crate::hir::DefId(42), vec![]), Span::DUMMY);
        // Drop impl registered → needs drop
        assert!(elaborator.needs_drop(&ty));
    }

    #[test]
    fn test_needs_drop_tuple_with_drop() {
        let mut elaborator = DropElaborator::new();
        elaborator.register_drop_impl(crate::hir::DefId(42));
        let ty = Ty::new(
            TyKind::Tuple(vec![
                Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
                Ty::new(TyKind::Adt(crate::hir::DefId(42), vec![]), Span::DUMMY),
            ]),
            Span::DUMMY,
        );
        // Tuple contains a Drop type → needs drop
        assert!(elaborator.needs_drop(&ty));
    }

    #[test]
    fn test_needs_drop_tuple_without_drop() {
        let elaborator = DropElaborator::new();
        let ty = Ty::new(
            TyKind::Tuple(vec![
                Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
                Ty::new(TyKind::Bool, Span::DUMMY),
            ]),
            Span::DUMMY,
        );
        // Tuple of primitives → doesn't need drop
        assert!(!elaborator.needs_drop(&ty));
    }

    #[test]
    fn test_compute_drop_set_reverse_order() {
        let mut elaborator = DropElaborator::new();
        elaborator.register_drop_impl(crate::hir::DefId(1));

        let mut mir = MirBody::new(Span::DUMMY);
        let _bb0 = mir.new_block();

        // Local 0 = return (i32, not dropped)
        mir.new_local(
            Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        // Local 1 = Drop type (Adt with DefId 1)
        mir.new_local(
            Ty::new(TyKind::Adt(crate::hir::DefId(1), vec![]), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        // Local 2 = i32 (not dropped)
        mir.new_local(
            Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
            None,
            Span::DUMMY,
        );
        // Local 3 = Drop type (Adt with DefId 1)
        mir.new_local(
            Ty::new(TyKind::Adt(crate::hir::DefId(1), vec![]), Span::DUMMY),
            None,
            Span::DUMMY,
        );

        let drop_set = elaborator.compute_drop_set(&mir, BasicBlockId(0));
        // Should have locals 3 and 1 (in reverse order: 3 first, then 1)
        assert_eq!(drop_set.locals.len(), 2);
        assert_eq!(drop_set.locals[0], LocalId(3)); // Reverse order
        assert_eq!(drop_set.locals[1], LocalId(1));
    }

    #[test]
    fn test_elaborate_empty_body() {
        let elaborator = DropElaborator::new();
        let mut mir = MirBody::new(Span::DUMMY);
        mir.new_block();
        let results = elaborator.elaborate(&mir);
        assert!(
            results.is_empty(),
            "empty body with no Drop types should have no drops"
        );
    }
}
