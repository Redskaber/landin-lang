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

pub use body::*;
pub use lower::{lower_hir_body_to_mir, MirLowerCtxt};
pub use lvalue::*;
pub use ty::*;
