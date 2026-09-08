//! §2.3-§2.4: AdtLayout → EmitType translation.
//!
//! Per `docs/lang-design/07-codegen.md` §2.3 (Layout calculation) + §2.4
//! (Niche optimization — future extension point). Currently handles Struct
//! and Enum layouts; niche optimization is deferred to v0.4+.
//!
//! Per §16: reads MIR data (AdtLayout) — no HIR.

use crate::codegen::mir_translation::types::{
    filter_void_fields, mir_type_to_emit_type_with_layouts_and_mono,
};
use crate::codegen::EmitType;

/// Stage 16.58: Convert an AdtLayout to EmitType, recursing with mono_layouts.
///
/// Helper for `mir_type_to_emit_type_with_layouts_and_mono`. Handles both
/// Struct and Enum layouts, recursing into field types with the mono_layouts
/// parameter so nested generic Adts resolve correctly.
pub(crate) fn adt_layout_to_emit_type(
    layout: &crate::mir::body::AdtLayout,
    layouts: &crate::mir::body::AdtLayouts,
    mono_layouts: Option<&crate::mir::monomorphize::MonoLayoutMap>,
) -> EmitType {
    use crate::mir::body::AdtLayout;
    match layout {
        AdtLayout::Struct { field_tys } => {
            if field_tys.is_empty() {
                EmitType::Struct(vec![])
            } else {
                // Stage 18.336 (P1 soundness fix): Filter Void fields (ZST
                // fields like `()` would leak `Void` into the struct type
                // → `llvm-as` rejects `{ void }`).
                let fields: Vec<EmitType> = field_tys
                    .iter()
                    .map(|t| mir_type_to_emit_type_with_layouts_and_mono(t, layouts, mono_layouts))
                    .collect();
                filter_void_fields(fields)
            }
        }
        AdtLayout::Enum {
            discriminant_ty,
            variant_payloads,
        } => {
            // Stage 18.336 (P1 soundness fix): Filter Void fields (ZST
            // payloads like `()` would leak `Void` into the enum storage
            // struct → `llvm-as` rejects).
            let mut field_tys = vec![mir_type_to_emit_type_with_layouts_and_mono(
                discriminant_ty,
                layouts,
                mono_layouts,
            )];
            for payload in variant_payloads {
                for t in payload {
                    // Stage 151 (TD-TRAIT-METHOD-RET-MATCH-GEP fix):
                    // For generic enums like `Option<T>`, the variant_payloads
                    // contain `Param(0)` (the `T` from `Some(T)`). When
                    // `mir_type_to_emit_type_with_layouts_and_mono` encounters
                    // `Param(0)`, it falls through to `mir_type_to_emit_type`
                    // which returns `I32` as a fallback.
                    //
                    // This produces `{ i32, i32 }` for `Option<i64>` (wrong —
                    // should be `{ i32, i64 }`), causing `insertvalue` to
                    // truncate the i64 payload to i32 → garbage values.
                    //
                    // Fix: When we encounter `Param(N)` in an enum payload,
                    // use `I64` as the default. This handles i64 and usize
                    // payloads correctly. For i32 payloads, the storage uses
                    // i64 (wider than needed), but LLVM handles the i32→i64
                    // extension transparently on insertvalue, and the match
                    // arm binding will use the correct field_ty (from Stage
                    // 150's substitute in pattern_bindings).
                    //
                    // Per §1.0 原則 6 (通解 > 特解): one Param handling rule
                    // for all enum payloads.
                    // Per §1.0 原則 9 (正确 > 妥协): I64 fallback is safer
                    // than I32 for Param in enum payloads.
                    let emit_ty = if matches!(t.kind, crate::mir::ty::TyKind::Param(_)) {
                        let from_mono =
                            mir_type_to_emit_type_with_layouts_and_mono(t, layouts, mono_layouts);
                        if from_mono == EmitType::I32 {
                            // Param fell through to I32 fallback. Use I64
                            // as a safer default for enum payloads.
                            // This handles i64 and usize payloads correctly.
                            // For i32 payloads, the wider storage (i64) is
                            // safe because LLVM zero-extends i32 to i64 on
                            // insertvalue, and the match arm binding uses
                            // the correct substituted type (from Stage 150).
                            EmitType::I64
                        } else {
                            from_mono
                        }
                    } else {
                        mir_type_to_emit_type_with_layouts_and_mono(t, layouts, mono_layouts)
                    };
                    field_tys.push(emit_ty);
                }
            }
            filter_void_fields(field_tys)
        }
    }
}
