//! High-level Intermediate Representation (HIR).
//!
//! Per 06-mir.md §3, HIR is the "high-level IR" — the AST plus per-node
//! `HirId`s and resolution placeholders. It is the input to:
//! - Stage 1.2: AST → HIR lowering (constructs the HIR from AST) — DONE
//! - Stage 1.3: Name resolution (populates `Res` in `HirPath`) — NEXT
//! - Stage 1.4: Scope-based name resolution (locals within bodies)
//! - Stage 2: Type inference (populates `InferTy` in `HirTy`)
//! - Stage 2: Borrow check (uses `HirId` to key borrow info)

pub mod id;
pub mod kinds;
pub mod lower;
pub mod map;

// Re-export the most-used types at the module root for convenience.
// Stage 3.57 (P0-3 fix): explicit list instead of `pub use kinds::*;`
// to prevent accidental leakage of internal types.
pub use id::{DefId, DefIdCounter, HirId, ItemLocalId, ItemLocalIdCounter, OwnerId};
pub use kinds::{
    Body, BodyId, DefKind, HirArm, HirAssocConst, HirAssocType, HirBinOp, HirBlock, HirConst,
    HirCrate, HirEnum, HirExpr, HirExprField, HirExprKind, HirExternBlock, HirFieldDef, HirFn,
    HirFnRetTy, HirFnSig, HirForeignItem, HirGenericParam, HirGenerics, HirImpl, HirImplItem,
    HirItem, HirLifetimeParam, HirLitKind, HirLocal, HirMod, HirModKind, HirParam, HirPat,
    HirPatField, HirPatKind, HirPath, HirPathSegment, HirQSelf, HirStatic, HirStmt, HirStruct,
    HirTrait, HirTraitBound, HirTraitItem, HirTy, HirTyKind, HirTypeAlias, HirTypeBound,
    HirTypeParam, HirUnaryOp, HirUse, HirUseTree, HirVariant, HirVariantData, HirWherePredicate,
    InferTy, InferTyCounter, OwnerNode, PrimTy, Res,
};
pub use lower::{lower_crate, HirLowerCtxt, LowerError};
pub use map::{DefIdMap, DefIdSet, HirIdMap, HirIdSet};
