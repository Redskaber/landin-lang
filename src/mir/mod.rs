//! Mid-level Intermediate Representation (MIR).
//!
//! Per 06-mir.md, MIR is the "mid-level IR" — a control flow graph
//! representation of each function body. It is the input to:
//! - Stage 2.2: Type inference (unification on MIR types)
//! - Stage 2.3: Borrow check (NLL on MIR control flow)
//! - Stage 3: LLVM codegen (MIR → LLVM IR)

pub mod body;
// Stage 15.43 (HP-12 step 2): Drop elaboration — `ty_needs_drop` analysis.
pub mod drop_elaboration;
pub mod dyn_trait;
pub mod lower;
// Stage 16.54 (Task 11 Phase 3): Monomorphization collection — walk MIR
// bodies and collect MonoItem { def_id, substs } pairs for codegen.
pub mod monomorphize;
// Stage 17.10: MIR optimization passes (DCE, const propagation).
pub mod optimization;
pub mod place;
// Stage 16.53 (Task 11 Phase 2): Type substitution — replace Param with
// concrete types from a SubstsRef slice.
pub mod substitute;
pub mod ty;
pub mod ty_interner;

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
    build_dyn_trait_call_terminator, lower_body, lower_body_full, lower_hir_body_to_mir,
    lower_hir_body_to_mir_full, lower_hir_body_to_mir_full_with_dyn_trait_plan,
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

// Stage 16.53 (Task 11 Phase 2): Type substitution re-exports.
// Stage 16.62: substitute_substs marked #[doc(hidden)] — test-only.
pub use substitute::substitute;
#[doc(hidden)]
pub use substitute::substitute_substs;

// Stage 16.54 (Task 11 Phase 3): Monomorphization collection re-exports.
// Stage 16.55 (Task 11 Phase 4a): Per-mono codegen — specialized naming.
// Stage 16.57 (Task 11 Phase 4b): Per-mono layouts.
// Stage 16.58 (Task 11 Phase 4c): Codegen integration — lookup_mono_layout.
pub use monomorphize::{
    build_mono_item_names, build_mono_layouts, collect_mono_items, lookup_mono_layout, mangle_ty,
    mangle_ty_with_interner, mono_item_name, MonoItem, MonoLayoutKey, MonoLayoutMap,
};

// Stage 5.61: dyn Trait fat pointer MIR representation
// Stage 5.62: bridge function from TraitResolver
// Stage 5.75: single-point lookup API for mir/lower integration
// Stage 5.77: fuzzy lookup variant (by method_name only)
// Stage 13.1 (TD-028): 7 emit_dyn_trait_* functions relocated to
// `codegen::dyn_trait_emit` per §16 interface isolation fix. The re-exports
// below only cover data structures + builders + lookup APIs (no emit_*).
pub use dyn_trait::{
    build_dyn_trait_fat_ptrs_from_resolver, build_dyn_trait_method_calls_from_fat_ptrs,
    build_dyn_trait_mir_plan, build_dyn_trait_mir_plan_from_resolver, build_dyn_trait_mir_summary,
    build_dyn_trait_mir_summary_from_resolver, find_dyn_trait_method_call_in_plan,
    find_dyn_trait_method_call_in_plan_by_method, DynTraitFatPtr, DynTraitMIRPlan,
    DynTraitMIRSummary, DynTraitMethodCall,
};
