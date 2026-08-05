//! Cross-§ helper: StdlibTypeKind → EmitType bridge.
//!
//! Per `docs/lang-design/09-stdlib.md`: StdlibTypeKind enumerates the type
//! kinds exposed by the Landin stdlib (primitives, strings, vec, etc.). This
//! function bridges stdlib type kinds to codegen's EmitType.
//!
//! Per §16: reads stdlib type kind enum only — no HIR/MIR access.

use crate::codegen::EmitType;

/// Stage 5.82: Convert `StdlibTypeKind` to `EmitType` for codegen.
///
/// Used by `codegen_dyn_trait_call` to emit the correct LLVM return type
/// for dyn Trait method calls (TD-016 closure). Previously (Stage 5.79),
/// all dyn Trait calls used `EmitType::I32` as a placeholder — this
/// function enables precise return type emission based on
/// `StdlibTraitMethod.return_kind`.
///
/// # Mapping
///
/// - Integer types (I8/U8/Bool/Char → I8, I16/U16 → I16, etc.) — width-based
/// - Float types (F32 → F32, F64 → F64) — direct
/// - Unit/Never → Void
/// - AllocType/StdType/Str/Unknown → OpaquePtr (dyn Trait receivers are
///   fat pointers; method returns of these types are ptr-sized)
///
/// Per API-naming-standard §3 + §8.2: `stdlib_type_kind_to_emit_type`
/// follows the `<noun>_<noun>_<noun>_<prep>_<noun>_<noun>` pattern,
/// matching the translation ladder convention of `mir_type_to_emit_type`
/// and `emit_type_to_llvm_str`.
pub fn stdlib_type_kind_to_emit_type(kind: crate::stdlib::StdlibTypeKind) -> EmitType {
    use crate::stdlib::StdlibTypeKind;
    match kind {
        StdlibTypeKind::I8 | StdlibTypeKind::U8 | StdlibTypeKind::Bool | StdlibTypeKind::Char => {
            EmitType::I8
        }
        StdlibTypeKind::I16 | StdlibTypeKind::U16 => EmitType::I16,
        StdlibTypeKind::I32 | StdlibTypeKind::U32 => EmitType::I32,
        StdlibTypeKind::I64 | StdlibTypeKind::U64 => EmitType::I64,
        StdlibTypeKind::I128 | StdlibTypeKind::U128 => EmitType::I128,
        StdlibTypeKind::F32 => EmitType::F32,
        StdlibTypeKind::F64 => EmitType::F64,
        StdlibTypeKind::Unit | StdlibTypeKind::Never => EmitType::Void,
        // AllocType/StdType/Str/Unknown → opaque pointer (dyn Trait
        // receivers are fat pointers; method returns of these types are
        // ptr-sized).
        StdlibTypeKind::AllocType
        | StdlibTypeKind::StdType
        | StdlibTypeKind::Str
        | StdlibTypeKind::Unknown => EmitType::OpaquePtr,
    }
}
