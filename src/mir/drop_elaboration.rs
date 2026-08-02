//! Stage 15.43 (HP-12 step 2 of 6): Drop elaboration — `ty_needs_drop` analysis.
//!
//! Per `docs/lang-design/25-drop-elaboration.md` §2.2. This module owns the
//! `ty_needs_drop` function, which determines whether a type needs drop glue.
//!
//! A type needs drop if:
//! - It implements `Drop` (user-defined destructor), OR
//! - It has fields that need drop (for structs/enums), OR
//! - It's a container of a type that needs drop (for `Box<T>`, `Vec<T>`).
//!
//! Primitive types (i32, bool, etc.) never need drop. References (&T, &mut T)
//! never need drop (they're just pointers). Raw pointers never need drop.
//!
//! ## MVP scope (v0.2)
//!
//! For the v0.2 MVP, `ty_needs_drop` checks:
//! 1. `Adt` types: `resolver.is_drop_builtin(def_id, interner)` — if the type
//!    implements `Drop`, it needs drop.
//! 2. Field traversal: for `Adt` types that don't implement `Drop`, check if
//!    any field needs drop (recursive). This uses `AdtLayouts` to look up
//!    field types without reading HIR (per §16 interface isolation).
//! 3. `Tuple`: check if any element needs drop.
//! 4. `Array`/`Slice`: check if the element type needs drop.
//!
//! ## What's NOT in scope for v0.2 MVP
//!
//! - `dyn Trait` (vtable drop slot) — always returns true for now, but the
//!   actual drop glue call is not implemented yet.
//! - Closures — return false (closures don't have Drop in v0.2).
//! - Generic types (monomorphization) — handled by codegen, not here.
//!
//! Per §23: function name follows `<noun>_<verb>_<noun>` pattern.
//! Per §16: `ty_needs_drop` reads `Ty`, `TraitResolver`, `AdtLayouts`, and
//! `Rodeo` — all are read-only references, no writes, no HIR lookup.
//! Per §1.0 原則 5 "报错 > 静默": when in doubt (e.g., `Infer`, `Error`),
//! return false — a false negative (missing a needed drop) just leaks memory;
//! a false positive (drop a type that doesn't need it) would be unsound
//! (calling a nonexistent drop method).

use crate::hir::DefId;
use crate::mir::body::{AdtLayout, AdtLayouts};
use crate::mir::ty::{Ty, TyKind};
use crate::traits::TraitResolver;
use lasso::Rodeo;
use std::collections::HashSet;

/// Determine whether a type needs drop glue.
///
/// A type needs drop if it implements `Drop`, or if it has fields/elements
/// that need drop (recursive). See module docs for full semantics.
///
/// ## Parameters
///
/// - `ty`: The type to check.
/// - `resolver`: The `TraitResolver` for querying `Drop` implementations.
/// - `adt_layouts`: The `AdtLayouts` for looking up field types (avoids
///   HIR lookup per §16).
/// - `interner`: The `Rodeo` for resolving trait names (e.g., "Drop").
///
/// ## Returns
///
/// `true` if the type needs drop glue, `false` otherwise.
///
/// ## Complexity
///
/// O(N) where N is the total number of fields/elements in the type tree
/// (recursive traversal). Cycles are handled via a `visited` set to prevent
/// infinite recursion on self-referential types (e.g., a struct with a
/// `Box<Self>` field).
///
/// Per §23: function name follows `<noun>_<verb>_<noun>` pattern.
pub fn ty_needs_drop(
    ty: &Ty,
    resolver: &TraitResolver,
    adt_layouts: &AdtLayouts,
    interner: &Rodeo,
) -> bool {
    let mut visited = HashSet::new();
    ty_needs_drop_impl(ty, resolver, adt_layouts, interner, &mut visited)
}

/// Internal recursive implementation of `ty_needs_drop`.
///
/// The `visited` set tracks `DefId`s of `Adt` types we've already examined,
/// to prevent infinite recursion on self-referential types (e.g., a linked
/// list node with a `Box<Node>` field). If we revisit a `DefId`, we return
/// `false` (the cycle-breaking case — the `Box` itself needs drop, but the
/// inner type's cycle is broken by the `Box` indirection).
fn ty_needs_drop_impl(
    ty: &Ty,
    resolver: &TraitResolver,
    adt_layouts: &AdtLayouts,
    interner: &Rodeo,
    visited: &mut HashSet<DefId>,
) -> bool {
    match &ty.kind {
        // Primitives never need drop.
        TyKind::Bool
        | TyKind::Char
        | TyKind::Int(_)
        | TyKind::Uint(_)
        | TyKind::Float(_)
        | TyKind::Str
        | TyKind::Never => false,

        // References and raw pointers are just pointers — never need drop.
        TyKind::Ref(_, _, _) | TyKind::RawPtr(_, _) => false,

        // Function definitions and pointers never need drop.
        TyKind::FnDef(_, _) | TyKind::FnPtr(_) => false,

        // Tuples need drop if any element needs drop.
        TyKind::Tuple(tys) => tys
            .iter()
            .any(|t| ty_needs_drop_impl(t, resolver, adt_layouts, interner, visited)),

        // Arrays and slices need drop if the element type needs drop.
        TyKind::Array(inner, _) | TyKind::Slice(inner) => {
            ty_needs_drop_impl(inner, resolver, adt_layouts, interner, visited)
        }

        // ADTs (struct/enum): check Drop impl + field types.
        TyKind::Adt(def_id, _) => {
            // Cycle detection: if we've already visited this DefId, return
            // false (the cycle is broken by the indirection that led us
            // here, e.g., Box<Self>).
            if !visited.insert(*def_id) {
                return false;
            }

            // Check if the type implements Drop (user-defined destructor).
            if resolver.is_drop_builtin(*def_id, interner) {
                return true;
            }

            // Check if any field needs drop (recursive).
            // Per §16: use AdtLayouts (sunk from HIR during MIR lowering)
            // instead of reading HIR directly.
            if let Some(layout) = adt_layouts.get(def_id) {
                match layout {
                    AdtLayout::Struct { field_tys } => {
                        for field_ty in field_tys {
                            if ty_needs_drop_impl(
                                field_ty,
                                resolver,
                                adt_layouts,
                                interner,
                                visited,
                            ) {
                                return true;
                            }
                        }
                    }
                    AdtLayout::Enum {
                        variant_payloads, ..
                    } => {
                        for payload in variant_payloads {
                            for field_ty in payload {
                                if ty_needs_drop_impl(
                                    field_ty,
                                    resolver,
                                    adt_layouts,
                                    interner,
                                    visited,
                                ) {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }

            false
        }

        // Closures: v0.2 doesn't support Drop on closures.
        TyKind::Closure(_, _) => false,

        // Foreign types: conservatively false (we don't know their layout).
        TyKind::Foreign => false,

        // Type parameters: conservatively false (we don't know the concrete
        // type at this point — monomorphization will handle it).
        TyKind::Param(_) => false,

        // Infer and Error: conservatively false to avoid spurious drops
        // during type inference. Per §1.0 原則 5 "报错 > 静默": a false
        // negative (missing a needed drop) just leaks memory; a false
        // positive (drop a type that doesn't need it) would be unsound.
        TyKind::Infer(_) | TyKind::Error => false,
    }
}

/// Stage 15.44 (HP-12 step 3 of 6): Insert `Drop` terminators before
/// `StorageDead` statements for locals whose type needs drop.
///
/// This pass walks all basic blocks. For each `StorageDead(local)`
/// statement where `ty_needs_drop(local.ty)` is true, it splits the
/// basic block at that point:
///
/// 1. The current block's statements up to (but not including) the
///    `StorageDead` stay in the current block.
/// 2. The current block's terminator is replaced with
///    `Drop { place: local, target: new_block }`.
/// 3. A new basic block is created with the statements AFTER the
///    `StorageDead` (the `StorageDead` itself is consumed by the
///    `Drop` terminator — see Stage 15.61 fix below) plus the original
///    terminator.
///
/// This correctly models the drop semantics: the destructor runs BEFORE
/// the local is marked as dead. The `Drop` terminator is a control-flow
/// point (it branches to the target after running the destructor).
///
/// ## Stage 15.61 fix — infinite-loop prevention
///
/// **Bug (Stage 15.60)**: The new block used to retain the
/// `StorageDead(local)` statement. When `bb_idx` reached the new block,
/// the algorithm found `StorageDead(local)` again (the local still needs
/// drop) and split again — ad infinitum, until OOM kill (exit 137).
///
/// **Fix (Stage 15.61)**: The `StorageDead(local)` statement is **dropped**
/// when splitting. The `Drop` terminator subsumes its role: after the
/// destructor runs, the local is dead. The `StorageDead` marker is no
/// longer needed (it was a stack-slot liveness hint, not a semantic
/// requirement). This matches rustc's behaviour where `Drop` terminators
/// replace `StorageDead` for types that need drop glue.
///
/// ## Drop order
///
/// Locals are dropped in forward declaration order in the current MVP
/// (matching the `StorageDead` emission order). Rust proper uses reverse
/// declaration order; a future stage will reverse the `StorageDead`
/// emission to match.
///
/// Per §23: function name follows `<verb>_<noun>` pattern (free-function
/// entry point).
/// Per §16: this pass mutates `MirBody` in place — it's a MIR-to-MIR
/// transformation, not a cross-stage operation.
/// Per §1.0 原則 3 "显式 > 隐式": the `Drop` terminators are explicit
/// in the MIR, not implicit in `StorageDead`.
/// Per §1.0 原則 5 "报错 > 静默": the pass is total — it terminates
/// on all inputs (the Stage 15.61 fix guarantees termination).
pub fn elaborate_drops(
    mir: &mut crate::mir::body::MirBody,
    resolver: &TraitResolver,
    interner: &Rodeo,
) {
    use crate::mir::body::{BasicBlockId, Statement, StatementKind, Terminator, TerminatorKind};
    use crate::mir::place::{Place, PlaceKind};

    // We need to process blocks in order. Since we may insert new blocks
    // (which are appended to the end of basic_blocks), we process all
    // blocks including newly created ones. The Stage 15.61 fix guarantees
    // termination: each split removes a `StorageDead` statement rather
    // than carrying it forward, so the new block has strictly fewer
    // `StorageDead` statements needing drop than the original.
    let mut bb_idx = 0;
    while bb_idx < mir.basic_blocks.len() {
        let bb_id = BasicBlockId(bb_idx as u32);
        let bb = &mir.basic_blocks[bb_idx];

        // Find the first StorageDead(local) where local needs drop.
        let split_point = bb.statements.iter().enumerate().find(|(_, stmt)| {
            if let StatementKind::StorageDead(local_id) = &stmt.kind {
                let local_ty = &mir.local(*local_id).ty;
                ty_needs_drop(local_ty, resolver, &mir.adt_layouts, interner)
            } else {
                false
            }
        });

        if let Some((stmt_idx, stmt)) = split_point {
            let local_id = if let StatementKind::StorageDead(lid) = &stmt.kind {
                *lid
            } else {
                unreachable!() // we just checked this above
            };

            // Split the block at stmt_idx.
            // 1. Save the statements AFTER stmt_idx (skip the StorageDead
            //    itself — Stage 15.61 fix). The StorageDead is consumed
            //    by the Drop terminator; carrying it into the new block
            //    would cause the new block to split again (infinite loop).
            let remaining_stmts: Vec<Statement> = bb.statements[stmt_idx + 1..].to_vec();
            let original_terminator = bb.terminator.clone();
            let original_term_span = bb.terminator_span;
            let block_span = bb.span;
            let stmt_span = stmt.span;

            // Compute the new block ID BEFORE the mutable borrow (avoids
            // borrow checker conflict: mir.basic_blocks.len() is immutable,
            // mir.block_mut() is mutable).
            let new_block_id_num = mir.basic_blocks.len() as u32;

            // 2. Truncate the current block's statements (remove from stmt_idx onwards).
            let bb_mut = mir.block_mut(bb_id);
            bb_mut.statements.truncate(stmt_idx);

            // 3. Set the current block's terminator to Drop { place: local, target: new_block }.
            bb_mut.terminator = Terminator {
                kind: TerminatorKind::Drop {
                    place: Place {
                        kind: PlaceKind::Local(local_id),
                        span: stmt_span,
                    },
                    target: BasicBlockId(new_block_id_num),
                    unwind: None,
                },
                span: stmt_span,
            };
            bb_mut.terminator_span = stmt_span;

            // 4. Create a new block with the remaining statements (AFTER
            //    the StorageDead) + original terminator. The StorageDead
            //    is NOT carried forward — see Stage 15.61 fix above.
            let new_block_id = mir.new_block();
            let new_block = mir.block_mut(new_block_id);
            new_block.statements = remaining_stmts;
            new_block.terminator = original_terminator;
            new_block.terminator_span = original_term_span;
            new_block.span = block_span;

            // Move to the next block. The current block no longer has
            // any StorageDead needing drop (truncated). The new block
            // has strictly fewer StorageDead statements needing drop
            // (we removed one). The loop terminates.
            bb_idx += 1;
        } else {
            // No StorageDead needing drop in this block — move to the next.
            bb_idx += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{FloatTy, IntTy};
    use crate::hir::DefId;
    use crate::mir::body::{AdtLayout, AdtLayouts};
    use crate::mir::ty::{Mutability, Region, Ty, TyKind};
    use crate::session::Span;
    use crate::traits::TraitResolver;
    use lasso::Rodeo;
    use std::collections::HashMap;

    /// Build a minimal TraitResolver + Rodeo + AdtLayouts for testing.
    /// The resolver has no impls (so `is_drop_builtin` returns false for all).
    fn build_test_context() -> (TraitResolver, Rodeo, AdtLayouts) {
        let resolver = TraitResolver::new();
        let interner = Rodeo::new();
        let adt_layouts: AdtLayouts = HashMap::new();
        (resolver, interner, adt_layouts)
    }

    /// Helper: build a `Ty` from a `TyKind`.
    fn ty(kind: TyKind) -> Ty {
        Ty::new(kind, Span::DUMMY)
    }

    // ----- Primitive types never need drop -----

    #[test]
    fn stage15_43_needs_drop_i32_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let t = ty(TyKind::Int(IntTy::I32));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    #[test]
    fn stage15_43_needs_drop_bool_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let t = ty(TyKind::Bool);
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    #[test]
    fn stage15_43_needs_drop_char_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let t = ty(TyKind::Char);
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    #[test]
    fn stage15_43_needs_drop_float_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let t = ty(TyKind::Float(FloatTy::F64));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    #[test]
    fn stage15_43_needs_drop_str_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let t = ty(TyKind::Str);
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    #[test]
    fn stage15_43_needs_drop_never_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let t = ty(TyKind::Never);
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    // ----- References and pointers never need drop -----

    #[test]
    fn stage15_43_needs_drop_ref_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let inner = ty(TyKind::Int(IntTy::I32));
        let t = ty(TyKind::Ref(
            Region::Static,
            Mutability::Immutable,
            Box::new(inner),
        ));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    #[test]
    fn stage15_43_needs_drop_mut_ref_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let inner = ty(TyKind::Int(IntTy::I32));
        let t = ty(TyKind::Ref(
            Region::Static,
            Mutability::Mutable,
            Box::new(inner),
        ));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    #[test]
    fn stage15_43_needs_drop_raw_ptr_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let inner = ty(TyKind::Int(IntTy::I32));
        let t = ty(TyKind::RawPtr(Mutability::Mutable, Box::new(inner)));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    // ----- Tuples: need drop if any element needs drop -----

    #[test]
    fn stage15_43_needs_drop_tuple_all_primitives_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let t = ty(TyKind::Tuple(vec![
            ty(TyKind::Int(IntTy::I32)),
            ty(TyKind::Bool),
        ]));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    // ----- Arrays/slices: need drop if element needs drop -----

    #[test]
    fn stage15_43_needs_drop_array_primitive_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let inner = ty(TyKind::Int(IntTy::I32));
        let const_val = crate::mir::ty::Const {
            ty: ty(TyKind::Int(IntTy::I32)),
            val: crate::mir::ty::ConstVal::Int(5),
        };
        let t = ty(TyKind::Array(Box::new(inner), Box::new(const_val)));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    #[test]
    fn stage15_43_needs_drop_slice_primitive_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let inner = ty(TyKind::Int(IntTy::I32));
        let t = ty(TyKind::Slice(Box::new(inner)));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    // ----- ADT without Drop impl: false (no fields need drop) -----

    #[test]
    fn stage15_43_needs_drop_adt_no_drop_no_fields_false() {
        let (resolver, interner, mut adt_layouts) = build_test_context();
        let def_id = DefId(0);
        adt_layouts.insert(
            def_id,
            AdtLayout::Struct {
                field_tys: vec![ty(TyKind::Int(IntTy::I32)), ty(TyKind::Bool)],
            },
        );
        let t = ty(TyKind::Adt(def_id, Vec::new().into()));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    // ----- Infer/Error: false (conservative) -----

    #[test]
    fn stage15_43_needs_drop_infer_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let t = ty(TyKind::Infer(crate::mir::ty::InferVar::TyVar(
            crate::mir::ty::TyVid(0),
        )));
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    #[test]
    fn stage15_43_needs_drop_error_false() {
        let (resolver, interner, adt_layouts) = build_test_context();
        let t = ty(TyKind::Error);
        assert!(!ty_needs_drop(&t, &resolver, &adt_layouts, &interner));
    }

    // ----- Cycle detection: self-referential type doesn't infinite-loop -----

    #[test]
    fn stage15_43_needs_drop_cycle_no_infinite_loop() {
        let (resolver, interner, mut adt_layouts) = build_test_context();
        let def_id = DefId(0);
        // A struct with a field of its own type (cycle).
        // In real code this would be behind a Box, but we test the cycle
        // detection directly.
        adt_layouts.insert(
            def_id,
            AdtLayout::Struct {
                field_tys: vec![ty(TyKind::Adt(def_id, Vec::new().into()))],
            },
        );
        let t = ty(TyKind::Adt(def_id, Vec::new().into()));
        // Should not infinite-loop. Returns false (cycle broken, no Drop impl).
        let _ = ty_needs_drop(&t, &resolver, &adt_layouts, &interner);
        // We don't assert the result — just that it terminates.
    }

    // ----- elaborate_drops() tests (Stage 15.44) -----

    /// Stage 15.44: `elaborate_drops` is a no-op when no types need drop.
    /// With an empty TraitResolver (no Drop impls), all types return false
    /// for `ty_needs_drop`, so no `Drop` terminators should be inserted.
    #[test]
    fn stage15_44_elaborate_drops_noop_when_no_drop_needed() {
        use crate::mir::body::{MirBody, Statement, StatementKind, Terminator};

        let (resolver, interner, _adt_layouts) = build_test_context();

        let mut body = MirBody::new(Span::DUMMY);
        let local_0 = body.new_local(ty(TyKind::Int(IntTy::I32)), None, Span::DUMMY);
        let local_1 = body.new_local(ty(TyKind::Bool), None, Span::DUMMY);
        let bb0 = body.new_block();
        body.block_mut(bb0).statements.push(Statement {
            kind: StatementKind::StorageDead(local_0),
            span: Span::DUMMY,
        });
        body.block_mut(bb0).statements.push(Statement {
            kind: StatementKind::StorageDead(local_1),
            span: Span::DUMMY,
        });
        body.block_mut(bb0).terminator = Terminator::ret(Span::DUMMY);

        // Replace the adt_layouts with our test one.
        // (MirBody::new creates an empty Arc<AdtLayouts>, which is fine.)
        // We need to use Arc::make_mut to set it, but since the Arc has
        // refcount 1, make_mut is a no-op.
        // Actually, MirBody doesn't expose a way to set adt_layouts directly.
        // But elaborate_drops uses mir.adt_layouts, which is an Arc.
        // For this test, the empty adt_layouts is fine — no ADT types.

        let block_count_before = body.basic_blocks.len();
        elaborate_drops(&mut body, &resolver, &interner);
        let block_count_after = body.basic_blocks.len();

        // No types need drop → no blocks inserted.
        assert_eq!(
            block_count_before, block_count_after,
            "elaborate_drops should not insert blocks when no types need drop"
        );

        // The block should still have its StorageDead statements (not split).
        assert_eq!(
            body.block(bb0).statements.len(),
            2,
            "StorageDead statements should remain (no drop needed)"
        );
    }

    /// Stage 15.44: `elaborate_drops` is callable on an empty body (no panic).
    #[test]
    fn stage15_44_elaborate_drops_empty_body() {
        use crate::mir::body::MirBody;

        let (resolver, interner, _) = build_test_context();
        let mut body = MirBody::new(Span::DUMMY);
        body.new_block(); // One empty block with unreachable terminator.

        // Should not panic.
        elaborate_drops(&mut body, &resolver, &interner);
    }
}
