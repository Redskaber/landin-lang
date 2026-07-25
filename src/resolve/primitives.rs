//! Stage 6.16 (TD-026): Primitive type lookup table.
//!
//! Per 01-language-specification.md §6.2 (resolve order) — primitive type
//! names are resolved during late resolve via this lookup table.
//! Extracted from `resolver.rs` per `docs/stage-committee-process.md`
//! v3.21 §14.4 + §13.4.

use crate::hir::PrimTy;

/// Look up a primitive type by name string.
pub(super) fn lookup_prim_ty(name: &str) -> Option<PrimTy> {
    Some(match name {
        "bool" => PrimTy::Bool,
        "char" => PrimTy::Char,
        "i8" => PrimTy::I8,
        "i16" => PrimTy::I16,
        "i32" => PrimTy::I32,
        "i64" => PrimTy::I64,
        "i128" => PrimTy::I128,
        "isize" => PrimTy::Isize,
        "u8" => PrimTy::U8,
        "u16" => PrimTy::U16,
        "u32" => PrimTy::U32,
        "u64" => PrimTy::U64,
        "u128" => PrimTy::U128,
        "usize" => PrimTy::Usize,
        "f32" => PrimTy::F32,
        "f64" => PrimTy::F64,
        "str" => PrimTy::Str,
        _ => return None,
    })
}
