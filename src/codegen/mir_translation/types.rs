//! §2.1-§2.3: MIR Ty → EmitType translation (with layouts / mono layouts).
//!
//! Per `docs/lang-design/07-codegen.md` §2.1 (basic types) + §2.2 (composite
//! types) + §2.3 (Layout calculation). The canonical §16-compliant translation
//! ladder: `mir_type_to_emit_type_with_layouts` resolves `TyKind::Adt` via
//! `MirBody::adt_layouts` (no HIR access). The `_and_mono` variant additionally
//! consults the per-monomorphization layout map (Stage 16.59 Task 11 Phase 4c).
//!
//! Per §16: reads MIR data (Ty, MirBody.adt_layouts, MonoLayoutMap) — no HIR.

use crate::codegen::mir_translation::layouts::adt_layout_to_emit_type;
use crate::codegen::{mir_type_to_emit_type, EmitType};
use crate::mir::ty::ConstVal;

pub fn mir_type_to_emit_type_with_layouts(
    ty: &crate::mir::ty::Ty,
    layouts: &crate::mir::body::AdtLayouts,
) -> EmitType {
    use crate::mir::body::AdtLayout;
    use crate::mir::ty::TyKind;
    match &ty.kind {
        TyKind::Adt(def_id, _substs) => match layouts.get(def_id) {
            Some(AdtLayout::Struct { field_tys }) => {
                if field_tys.is_empty() {
                    // Stage 14.63: Zero-field structs (e.g. `struct Unit;`)
                    // are represented as LLVM `{}` (empty struct), NOT `void`.
                    //
                    // Previously, these were mapped to `EmitType::Void`, which
                    // caused:
                    // 1. Functions returning ZSTs to have `void` signature
                    // 2. Locals of ZST type to be skipped (no alloca)
                    // 3. Method calls on ZST receivers to pass `null` as `&self`
                    //
                    // With `Struct(vec![])` (LLVM `{}`):
                    // - The function signature is `{} @f()` (returns empty struct)
                    // - The alloca is `alloca {}` (valid, zero-size)
                    // - The receiver `&self` is the alloca pointer (valid `ptr`)
                    //
                    // Per §1.0 原则 6 "通用 > 特例": same code path as non-empty
                    // structs, just with zero fields.
                    EmitType::Struct(vec![])
                } else {
                    // Stage 3.47: recurse with `layouts` so nested Adts
                    // resolve correctly (e.g., `struct Outer { i: Inner }`
                    // renders as `{ { i32 } }`, not `{ i32 }`).
                    EmitType::Struct(
                        field_tys
                            .iter()
                            .map(|t| mir_type_to_emit_type_with_layouts(t, layouts))
                            .collect(),
                    )
                }
            }
            Some(AdtLayout::Enum {
                discriminant_ty,
                variant_payloads,
            }) => {
                // Stage 3.48 (L-ENUM-UNION): flatten ALL non-empty variants'
                // payload fields into the storage struct. This fixes the
                // soundness bug where `enum E { A, B(i32), C(i64) }` only
                // allocated i32 storage for C's i64 payload.
                let mut field_tys =
                    vec![mir_type_to_emit_type_with_layouts(discriminant_ty, layouts)];
                for payload in variant_payloads {
                    for t in payload {
                        field_tys.push(mir_type_to_emit_type_with_layouts(t, layouts));
                    }
                }
                EmitType::Struct(field_tys)
            }
            // Test-context fallback: layout not registered (MIR body constructed
            // without HIR). Falls back to I32 placeholder (preserves Stage 3.30
            // behavior for tests that don't exercise Adt codegen).
            None => EmitType::I32,
        },
        // Stage 3.48: recurse into Tuple/Array/Ref/RawPtr/Slice with `_with_layouts`
        // so nested Adts (e.g., enum inside a tuple, struct inside an array)
        // resolve their layouts correctly. Was: fell through to
        // `mir_type_to_emit_type` which doesn't know about AdtLayouts, causing
        // nested Adts to collapse to I32 (pre-existing bug exposed by the
        // e07_enum_in_tuple audit case).
        TyKind::Tuple(tys) => {
            if tys.is_empty() {
                EmitType::Void
            } else {
                EmitType::Struct(
                    tys.iter()
                        .map(|t| mir_type_to_emit_type_with_layouts(t, layouts))
                        .collect(),
                )
            }
        }
        TyKind::Array(elem, len) => {
            let n = match &len.val {
                ConstVal::Int(n) | ConstVal::Uint(n) => *n as u64,
                _ => 0,
            };
            EmitType::array_of(mir_type_to_emit_type_with_layouts(elem, layouts), n)
        }
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) => {
            // Stage 3.49 (L13 closure): `&str` and `&[T]` are fat pointers
            // `{ ptr, len }`. Other references remain thin pointers.
            // Recurse with `_with_layouts` so the pointee (if it's an Adt)
            // resolves its layout correctly.
            match &inner.kind {
                TyKind::Str => crate::codegen::emit_fat_ptr_type(EmitType::I8),
                TyKind::Slice(elem) => crate::codegen::emit_fat_ptr_type(
                    mir_type_to_emit_type_with_layouts(elem, layouts),
                ),
                _ => EmitType::ptr_to(mir_type_to_emit_type_with_layouts(inner, layouts)),
            }
        }
        TyKind::Slice(elem) => EmitType::ptr_to(mir_type_to_emit_type_with_layouts(elem, layouts)),
        // Stage 14.57: FnPtr and FnDef — emit as opaque pointer (function reference).
        TyKind::FnPtr(_) | TyKind::FnDef(_, _) => EmitType::OpaquePtr,
        // Stage 14.82 (GAP-7 partial fix): Closure type — emit as a struct
        // with one field per capture, using the layouts-aware variant
        // recursively so captured Adts resolve to their actual LLVM struct
        // type. Was: fell through to `mir_type_to_emit_type` (the legacy
        // variant), which uses itself (legacy) for substs — falling back
        // to `EmitType::I32` for any `Adt` capture. This caused closures
        // capturing structs to be typed `{ i32 }` instead of
        // `{ { i32, i32 } }`, leading to "Invalid InsertValueInst
        // operands!" in LLVM verification.
        //
        // Per §1.0 原則 5 "报错 > 静默": the legacy fallback silently
        // produced wrong LLVM types. The layouts-aware variant surfaces
        // the correct type and lets LLVM verification succeed.
        TyKind::Closure(_, substs) => {
            let fields: Vec<EmitType> = substs
                .iter()
                .map(|ty| mir_type_to_emit_type_with_layouts(ty, layouts))
                .collect();
            EmitType::Struct(fields)
        }
        _ => mir_type_to_emit_type(ty),
    }
}

/// Stage 16.58 (Task 11 Phase 4c): Map a MIR Ty to an EmitType, resolving
/// generic ADT instantiations via per-mono layouts.
///
/// This is the codegen integration point for monomorphization. It first
/// checks `lookup_mono_layout` for generic types (non-empty substs). If
/// found, it uses the specialized layout (with substituted field types).
/// Otherwise, it falls back to the existing `AdtLayouts` map (non-generic
/// types) or `mir_type_to_emit_type_with_layouts` (primitives, etc.).
///
/// ## Data Flow
///
/// ```text
/// TyKind::Adt(def_id, substs)
///     │
///     ├─ if !substs.is_empty():
///     │   └─ lookup_mono_layout(def_id, substs, mono_layouts)
///     │       ├─ Some(layout) → use specialized layout (substituted fields)
///     │       └─ None → fall through to AdtLayouts (legacy)
///     │
///     └─ else (empty substs):
///         └─ AdtLayouts.get(def_id) (legacy non-generic path)
/// ```
///
/// Per §23: `mir_type_to_emit_type_with_layouts_and_mono` follows
/// `<verb>_<noun>_<noun>_<prep>_<noun>_<prep>_<noun>` pattern.
/// Per §16: reads MonoLayoutMap + AdtLayouts (data only, no HIR).
/// Per §1.0 原則 6 "通用 > 特例": one function for generic + non-generic.
pub fn mir_type_to_emit_type_with_layouts_and_mono(
    ty: &crate::mir::ty::Ty,
    layouts: &crate::mir::body::AdtLayouts,
    mono_layouts: Option<&crate::mir::monomorphize::MonoLayoutMap>,
) -> EmitType {
    use crate::mir::ty::TyKind;

    match &ty.kind {
        TyKind::Adt(def_id, substs) => {
            // Stage 16.58: First, try the per-mono layout (for generic types).
            if let Some(mono_layout) =
                crate::mir::monomorphize::lookup_mono_layout(*def_id, substs, mono_layouts)
            {
                return adt_layout_to_emit_type(mono_layout, layouts, mono_layouts);
            }
            // Fall back to the legacy AdtLayouts map (non-generic types).
            match layouts.get(def_id) {
                Some(layout) => adt_layout_to_emit_type(layout, layouts, mono_layouts),
                None => EmitType::I32, // test-context fallback
            }
        }
        // Recursive types — recurse with mono_layouts so nested generic
        // Adts resolve their specialized layouts.
        TyKind::Tuple(tys) => {
            if tys.is_empty() {
                EmitType::Void
            } else {
                EmitType::Struct(
                    tys.iter()
                        .map(|t| {
                            mir_type_to_emit_type_with_layouts_and_mono(t, layouts, mono_layouts)
                        })
                        .collect(),
                )
            }
        }
        TyKind::Array(elem, len) => {
            let n = match &len.val {
                ConstVal::Int(n) | ConstVal::Uint(n) => *n as u64,
                _ => 0,
            };
            EmitType::array_of(
                mir_type_to_emit_type_with_layouts_and_mono(elem, layouts, mono_layouts),
                n,
            )
        }
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) => match &inner.kind {
            TyKind::Str => crate::codegen::emit_fat_ptr_type(EmitType::I8),
            TyKind::Slice(elem) => crate::codegen::emit_fat_ptr_type(
                mir_type_to_emit_type_with_layouts_and_mono(elem, layouts, mono_layouts),
            ),
            _ => EmitType::ptr_to(mir_type_to_emit_type_with_layouts_and_mono(
                inner,
                layouts,
                mono_layouts,
            )),
        },
        TyKind::Slice(elem) => EmitType::ptr_to(mir_type_to_emit_type_with_layouts_and_mono(
            elem,
            layouts,
            mono_layouts,
        )),
        TyKind::Closure(_, substs) => {
            let fields: Vec<EmitType> = substs
                .iter()
                .map(|ty| mir_type_to_emit_type_with_layouts_and_mono(ty, layouts, mono_layouts))
                .collect();
            EmitType::Struct(fields)
        }
        // All other kinds — delegate to the existing function.
        _ => mir_type_to_emit_type_with_layouts(ty, layouts),
    }
}
