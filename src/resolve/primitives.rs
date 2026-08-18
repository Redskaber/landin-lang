//! Stage 6.16 (TD-026): Primitive type lookup table.
//!
//! Per 01-language-specification.md §6.2 (resolve order) — primitive type
//! names are resolved during late resolve via this lookup table.
//! Extracted from `resolver.rs` per `docs/stage-committee-process.md`
//! v3.21 §14.4 + §13.4.

use crate::hir::PrimTy;

/// Look up a primitive type by name string.
///
/// Stage 18.180 (TD-STRING-AS-STR-ALIAS fix): "String" is NO LONGER mapped
/// to PrimTy::Str. It's now a real struct type defined in the prelude
/// (`struct String { ptr: *mut u8, len: i64, cap: i64 }`). The resolver
/// finds it via the normal module tree lookup, not via this primitive table.
///
/// Previously (Stage 18.176), String was an alias for &str — a stack-
/// allocated fat pointer. This violated the design doc (09-stdlib.md §3.4)
/// which defines String as an owned heap type backed by Vec<u8>. The alias
/// was a temporary MVP compromise (TD-STRING-AS-STR-ALIAS), now removed.
///
/// Per §1.0 原則 6 (通解>特例): one lookup for all primitive types.
/// Per §2 原則 9 (正确>妥协): the alias compromise is removed — real String
/// is the correct design. Ergonomic intrinsics (from_str/push_str/len)
/// are deferred to Stage 18.181 (TD-STRING-INTRINSICS).
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
        // Stage 18.180: "String" is NOT here — it's a prelude struct now.
        _ => return None,
    })
}
