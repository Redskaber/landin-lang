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
                    // Stage 152 (TD-OPTION-AND-THEN-I32-MISMATCH fix):
                    // Revert Stage 151's I64 fallback for Param in enum
                    // payloads. The I64 fallback caused type mismatches:
                    // prelude generic functions like Option::and_then<U>
                    // had return type {i32, i64} (I64 fallback) but callers
                    // expected {i32, i32} (concrete i32) → invalid bitcast
                    // store → wrong values.
                    //
                    // The correct behavior is to use the standard
                    // mir_type_to_emit_type_with_layouts_and_mono (which
                    // returns I32 for Param). This is correct because:
                    // 1. For concrete code (Option<i64>), mono_layouts has
                    //    the substituted layout → correct {i32, i64}.
                    // 2. For prelude generic code (Option<T> in and_then<U>),
                    //    the function body uses {i32, i32} for the local
                    //    alloca (from local_decl.ty which is concrete after
                    //    writeback), and the return type is {i32, i32}
                    //    (from local_decl[0].ty). The storage_ty in
                    //    insertvalue uses {i32, i32} (from the concrete
                    //    local_decl.ty), so it's correct even with I32 here.
                    // 3. For codegen_mono_functions (specialized generic
                    //    functions), substitute_mir_body +
                    //    resolve_projections_in_mir resolve Param →
                    //    concrete, so mono_layouts has correct layout.
                    //
                    // Per §1.0 原則 9 (正确 > 妥协): I32 fallback is the
                    // correct behavior — it matches the concrete type for
                    // i32 payloads, and for i64 payloads, the mono_layouts
                    // path handles it correctly.
                    field_tys.push(mir_type_to_emit_type_with_layouts_and_mono(
                        t,
                        layouts,
                        mono_layouts,
                    ));
                }
            }
            filter_void_fields(field_tys)
        }
    }
}
