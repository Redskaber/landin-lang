//! Mid-level Intermediate Representation (MIR).
//!
//! Per 06-mir.md, MIR is the "mid-level IR" — a control flow graph
//! representation of each function body. It is the input to:
//! - Stage 2.2: Type inference (unification on MIR types)
//! - Stage 2.3: Borrow check (NLL on MIR control flow)
//! - Stage 3: LLVM codegen (MIR → LLVM IR)

pub mod body;
pub mod dyn_trait;
pub mod lower;
pub mod place;
pub mod ty;

// Stage 3.57 (P0-3 fix): explicit re-exports instead of `pub use *::*;`
// to prevent accidental leakage of internal types.
// Stage 3.63 (cross-stage naming standardization): re-export the
// `_full` variant too — it's the one the driver actually uses (returns
// the UnificationTable alongside the MirBody for typeck consumption).
// Stage 3.65: re-export the short-form `lower_body` / `lower_body_full`
// aliases per api-naming-standard.md §2.2 verb_noun convention.
// Stage 3.66: `lvalue` module renamed to `place` per 06-mir.md §4 design
// doc + borrowck internal vocabulary (PlacePath, PlaceRoot).
pub use body::{
    AdtLayout, AdtLayouts, AssertMessage, BasicBlock, BasicBlockId, LocalDecl, MirBody, Statement,
    StatementKind, Terminator, VisibleNames,
};
pub use lower::{
    lower_body, lower_body_full, lower_hir_body_to_mir, lower_hir_body_to_mir_full,
    lower_hir_body_to_mir_with_return_ty, MirLowerCtxt,
};
pub use place::{
    AggregateKind, BinOp, BorrowKind, CastKind, FieldId, LocalId, Operand, Place, PlaceKind,
    ProjectionElem, RangeOp, Rvalue, UnOp,
};
pub use ty::{
    Const, ConstVal, FloatVid, InferVar, IntVid, Mutability, ParamTy, Region, RegionVid, Sig,
    SubstsRef, Ty, TyKind, TyVid,
};

// Stage 5.61: dyn Trait fat pointer MIR representation
// Stage 5.62: bridge function from TraitResolver
pub use dyn_trait::{
    build_dyn_trait_fat_ptrs_from_resolver, build_dyn_trait_method_calls_from_fat_ptrs,
    build_dyn_trait_mir_plan, build_dyn_trait_mir_plan_from_resolver, build_dyn_trait_mir_summary,
    build_dyn_trait_mir_summary_from_resolver, emit_dyn_trait_fat_ptr_text,
    emit_dyn_trait_fat_ptrs_text_batch, emit_dyn_trait_fat_ptrs_text_batch_from_resolver,
    emit_dyn_trait_method_call_text, emit_dyn_trait_method_calls_text_batch,
    emit_dyn_trait_method_calls_text_batch_from_resolver, emit_dyn_trait_mir_plan_text,
    DynTraitFatPtr, DynTraitMIRPlan, DynTraitMIRSummary, DynTraitMethodCall,
};
