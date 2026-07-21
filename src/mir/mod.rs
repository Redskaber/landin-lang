//! Mid-level Intermediate Representation (MIR).
//!
//! Per 06-mir.md, MIR is the "mid-level IR" — a control flow graph
//! representation of each function body. It is the input to:
//! - Stage 2.2: Type inference (unification on MIR types)
//! - Stage 2.3: Borrow check (NLL on MIR control flow)
//! - Stage 3: LLVM codegen (MIR → LLVM IR)

pub mod body;
pub mod lower;
pub mod lvalue;
pub mod ty;

// Stage 3.57 (P0-3 fix): explicit re-exports instead of `pub use *::*;`
// to prevent accidental leakage of internal types.
// Stage 3.63 (cross-stage naming standardization): re-export the
// `_full` variant too — it's the one the driver actually uses (returns
// the UnificationTable alongside the MirBody for typeck consumption).
pub use body::{
    AdtLayout, AdtLayouts, AssertMessage, BasicBlock, BasicBlockId, LocalDecl, MirBody, Statement,
    StatementKind, Terminator, VisibleNames,
};
pub use lower::{
    lower_hir_body_to_mir, lower_hir_body_to_mir_full, lower_hir_body_to_mir_with_return_ty,
    MirLowerCtxt,
};
pub use lvalue::{
    AggregateKind, BinOp, BorrowKind, CastKind, FieldId, LocalId, Lvalue, LvalueKind, Operand,
    ProjectionElem, RangeOp, Rvalue, UnOp,
};
pub use ty::{
    Const, ConstVal, FloatVid, InferVar, IntVid, Mutability, ParamTy, Region, RegionVid, Sig,
    SubstsRef, Ty, TyKind, TyVid,
};
