//! §2.3-§2.4: AdtLayout → EmitType translation.
//!
//! Per `docs/lang-design/07-codegen.md` §2.3 (Layout calculation) + §2.4
//! (Niche optimization — future extension point). Currently handles Struct
//! and Enum layouts; niche optimization is deferred to v0.4+.
//!
//! Per §16: reads MIR data (AdtLayout) — no HIR.

use crate::codegen::mir_translation::types::mir_type_to_emit_type_with_layouts_and_mono;
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
                EmitType::Struct(
                    field_tys
                        .iter()
                        .map(|t| {
                            mir_type_to_emit_type_with_layouts_and_mono(t, layouts, mono_layouts)
                        })
                        .collect(),
                )
            }
        }
        AdtLayout::Enum {
            discriminant_ty,
            variant_payloads,
        } => {
            let mut field_tys = vec![mir_type_to_emit_type_with_layouts_and_mono(
                discriminant_ty,
                layouts,
                mono_layouts,
            )];
            for payload in variant_payloads {
                for t in payload {
                    field_tys.push(mir_type_to_emit_type_with_layouts_and_mono(
                        t,
                        layouts,
                        mono_layouts,
                    ));
                }
            }
            EmitType::Struct(field_tys)
        }
    }
}
