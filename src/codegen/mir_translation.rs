//! Stage 6.8: MIR type/place/operand translation helpers.
//!
//! Architectural extraction from `codegen/mod.rs` (TD-017 step 2).
//! Contains the type translation ladder (MIR Ty → EmitType) and
//! type detection helpers that bridge MIR data structures to codegen's
//! EmitType system.
//!
//! Per §8.2 (translation function ladder): `mir_type_to_emit_type_with_layouts`
//! is the canonical §16-compliant MIR→EmitType translation.
//! `stdlib_type_kind_to_emit_type` bridges stdlib→codegen.
//!
//! Per §16: reads MIR data (Ty, MirBody.adt_layouts) — no HIR access.

use crate::codegen::{mir_type_to_emit_type, EmitType, EmitValue, Emitter};
use crate::mir::body::*;
use crate::mir::place::*;
use crate::mir::ty::*;
use lasso::Rodeo;

/// Stage 3.47 (L-PIPE-1 closure per §16): Map a MIR Ty to an EmitType,
/// resolving `TyKind::Adt(def_id, _)` via the `adt_layouts` side-table
/// on `MirBody` — **without reading HIR**.
///
/// Per §16 (阶段间接口隔离): MIR lower has sunk the ADT layouts into
/// `MirBody::adt_layouts` during lowering. Codegen now reads the layouts
/// from MIR, eliminating the cross-stage HIR lookup that was carried as
/// L-PIPE-1 debt since Stage 3.30.
///
/// Stage 3.48 (L-ENUM-UNION closure): enum storage layout now flattens
/// ALL non-empty variants' payload fields into the storage struct (was:
/// only the first non-empty variant's payload, a soundness bug for enums
/// with ≥2 non-empty variants of different widths). Layout is now:
///   - Case A (all unit variants): `{ discr }`
///   - Case B (exactly one non-empty variant): `{ discr, payload_fields... }`
///   - Case C (≥2 non-empty variants): `{ discr, variant_0_fields..., variant_1_fields..., ... }`
///     (unit variants contribute no fields; this is the soundness fix)
///
/// The flat layout means `variant_idx` does NOT directly map to `field_idx`
/// in the storage. The mapping is:
///   `field_idx(variant_V, field_F) = 1 + sum(field_counts of variants 0..V-1) + F`
/// This is computed in the `Aggregate(Adt(...))` codegen and in MIR lower's
/// pattern-binding projection generation.
/// **Stage 3.65 (P2 fix)**: This is the **canonical** §16-compliant
/// MIR→EmitType translation. It resolves `TyKind::Adt` via
/// `MirBody::adt_layouts` (the side-table populated by MIR lower per
/// §16 — no HIR access). Use this everywhere a `MirBody` is available.
///
/// The legacy `mir_type_to_emit_type` (no layouts) is kept only for
/// tests/standalone helpers where the type is known to be primitive.
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
                    EmitType::Void
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
        _ => mir_type_to_emit_type(ty),
    }
}

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

pub(crate) fn detect_place_storage_type(
    mir: &MirBody,
    lv: &Place,
    layouts: &crate::mir::body::AdtLayouts,
) -> EmitType {
    match &lv.kind {
        PlaceKind::Local(id) => mir
            .local_decls
            .get(id.0 as usize)
            .map(|ld| mir_type_to_emit_type_with_layouts(&ld.ty, layouts))
            .unwrap_or(EmitType::I32),
        // Stage 3.54: for Field projection, return the FIELD's type (not the
        // base's type). Was: returned detect_place_storage_type(base), which
        // gave the struct type instead of the field type — causing
        // unwrap_fat_ptr_for_index to see the struct layout instead of the
        // fat pointer layout when indexing a struct field that contains a
        // slice/array.
        PlaceKind::Projection(base, elem) => match elem {
            ProjectionElem::Field(_, field_ty) => {
                mir_type_to_emit_type_with_layouts(field_ty, layouts)
            }
            // Stage 14.19 (GAP-31): For Deref, the storage type is the
            // pointee type (what the reference points to), not the reference
            // type itself. This matches detect_place_type's Deref handling.
            // Was: recursed into base, returning the Ref type (pointer) instead
            // of the pointee, causing GEP to use the wrong type.
            ProjectionElem::Deref => {
                let base_ty = detect_place_storage_type(mir, base, layouts);
                if base_ty.is_ptr() {
                    base_ty.pointee()
                } else {
                    base_ty
                }
            }
            // Stage 14.44: For Index/ConstantIndex, the storage type is the
            // ELEMENT type (what's stored at that index), not the array type.
            // Was: returned detect_place_storage_type(base) which gives the
            // array type [N x T] instead of the element type T. This caused
            // emit_gep_field to use the array type instead of the element type
            // for `arr[i].field` patterns → GEP with wrong indices.
            ProjectionElem::Index(_) | ProjectionElem::ConstantIndex { .. } => {
                let array_ty = detect_place_storage_type(mir, base, layouts);
                // If the base is an array, return the element type.
                if let EmitType::Array(elem_ty, _) = &array_ty {
                    elem_ty.as_ref().clone()
                } else {
                    // Fat pointer { ptr, len } — element type is ptr's pointee.
                    // unwrap_fat_ptr handles this; return the base type for now.
                    array_ty
                }
            }
            ProjectionElem::Subslice { .. } => detect_place_storage_type(mir, base, layouts),
        },
        PlaceKind::Static(_) => EmitType::I32,
    }
}

pub(crate) fn detect_place_type(
    mir: &MirBody,
    lv: &Place,
    layouts: &crate::mir::body::AdtLayouts,
) -> EmitType {
    match &lv.kind {
        PlaceKind::Local(id) => mir
            .local_decls
            .get(id.0 as usize)
            .map(|ld| mir_type_to_emit_type_with_layouts(&ld.ty, layouts))
            .unwrap_or(EmitType::I32),
        PlaceKind::Projection(base, elem) => match elem {
            ProjectionElem::Deref => {
                let base_ty = detect_place_type(mir, base, layouts);
                if base_ty.is_ptr() {
                    base_ty.pointee()
                } else {
                    base_ty
                }
            }
            ProjectionElem::Field(field_id, field_ty) => {
                // Stage 14.49: If field_ty is Infer, try to resolve it from
                // the base's type (e.g., Tuple field types). This handles
                // nested tuple destructure where the projection's field_ty
                // was set to Infer at MIR-lower time but the base's type was
                // resolved by the post-typeck writeback.
                let emit_ty = mir_type_to_emit_type_with_layouts(field_ty, layouts);
                if matches!(emit_ty, EmitType::I32)
                    && matches!(&field_ty.kind, crate::mir::ty::TyKind::Infer(_))
                {
                    // Try to get the field type from the base's Tuple type
                    if let PlaceKind::Local(base_id) = &base.kind {
                        if let Some(base_ld) = mir.local_decls.get(base_id.0 as usize) {
                            if let crate::mir::ty::TyKind::Tuple(field_tys) = &base_ld.ty.kind {
                                if let Some(resolved) = field_tys.get(field_id.0 as usize) {
                                    return mir_type_to_emit_type_with_layouts(resolved, layouts);
                                }
                            }
                        }
                    }
                }
                emit_ty
            }
            ProjectionElem::Index(_) | ProjectionElem::ConstantIndex { .. } => {
                let storage = detect_place_storage_type(mir, base, layouts);
                match storage {
                    EmitType::Array(elem, _) => *elem,
                    // Stage 3.52: fat pointer (&[T] slice) — extract the
                    // pointee type from field 0 of the fat pointer struct.
                    // Was (Stage 3.51 bug): fell through to I32 fallback,
                    // causing `s[0]` on `&[i64]` to `load i32` instead of
                    // `load i64` — type mismatch in typed-pointer LLVM.
                    EmitType::Struct(fields)
                        if fields.len() == 2
                            && fields[0].is_ptr()
                            && fields[1] == EmitType::I64 =>
                    {
                        fields[0].pointee()
                    }
                    _ => EmitType::I32,
                }
            }
            _ => EmitType::I32,
        },
        PlaceKind::Static(_) => EmitType::I32,
    }
}

/// Stage 3.54: Compute the ADDRESS of a place (without loading its value).
/// Used by the store path's Index projection to get a pointer to the base
/// storage (e.g., the address of a struct field containing a fat pointer),
/// so that `unwrap_fat_ptr_for_index` can GEP into the storage correctly.
///
/// For a Local: returns the alloca pointer.
/// For a Projection(Local, Field): GEPs to the field, returns the field's address.
/// For deeper projections: recurses (best-effort — complex cases may fall back
/// to loading, which is the old behavior).
pub(crate) fn compute_place_address(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    lv: &Place,
    _interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
) -> String {
    match &lv.kind {
        PlaceKind::Local(id) => emitter
            .get_local_ptr(id.0)
            .cloned()
            .unwrap_or_else(|| "0".to_string()),
        PlaceKind::Projection(base, elem) => match elem {
            ProjectionElem::Field(field_id, _) => {
                // Stage 14.19 (GAP-31): Handle Deref+Field for store path.
                // When base is a Deref (e.g. `(*self).field`), load the pointer
                // from the inner base, then GEP through it.
                let base_addr = if let PlaceKind::Projection(inner_base, ProjectionElem::Deref) =
                    &base.kind
                {
                    let ptr_ty = detect_place_type(mir, inner_base, layouts);
                    codegen_place_load_typed(emitter, mir, inner_base, ptr_ty, _interner, layouts)
                } else {
                    compute_place_address(emitter, mir, base, _interner, layouts)
                };
                let struct_ty = detect_place_storage_type(mir, base, layouts);
                emitter.emit_gep_field(&base_addr, &struct_ty, field_id.0)
            }
            // Stage 14.44: For Index projection in compute_place_address,
            // we need to GEP to the element's ADDRESS (not load the value).
            // Previously, this fell through to the `_` arm which called
            // codegen_place_load_typed (loads the value), breaking
            // `arr[i].field` patterns where Field wraps Index.
            ProjectionElem::Index(idx) => {
                let base_ptr = compute_place_address(emitter, mir, base, _interner, layouts);
                let array_ty = detect_place_storage_type(mir, base, layouts);
                let idx_val = if let Some(v) = emitter.get_local(idx.0).cloned() {
                    v
                } else if let Some(ptr) = emitter.get_local_ptr(idx.0).cloned() {
                    emitter.emit_load(&EmitType::I32, &ptr)
                } else {
                    "0".to_string()
                };
                let (gep_base, pointee_opt) =
                    unwrap_fat_ptr_for_index(emitter, &base_ptr, &array_ty);
                match pointee_opt {
                    Some(elem_ty) => emitter.emit_gep_index_ptr(&gep_base, &elem_ty, &idx_val),
                    None => emitter.emit_gep_index(&gep_base, &array_ty, &idx_val),
                }
            }
            ProjectionElem::ConstantIndex { offset, .. } => {
                let base_ptr = compute_place_address(emitter, mir, base, _interner, layouts);
                let array_ty = detect_place_storage_type(mir, base, layouts);
                let (gep_base, pointee_opt) =
                    unwrap_fat_ptr_for_index(emitter, &base_ptr, &array_ty);
                match pointee_opt {
                    Some(elem_ty) => {
                        emitter.emit_gep_index_ptr(&gep_base, &elem_ty, &offset.to_string())
                    }
                    None => emitter.emit_gep_index(&gep_base, &array_ty, &offset.to_string()),
                }
            }
            // For other projection types, fall back to the load path
            // (loads the value — old behavior, may not work for fat pointers
            // in store position, but preserves existing behavior for non-fat-ptr cases).
            _ => {
                let ptr_ty = detect_place_type(mir, lv, layouts);
                codegen_place_load_typed(emitter, mir, lv, ptr_ty, _interner, layouts)
            }
        },
        PlaceKind::Static(_) => "0".to_string(),
    }
}

/// Stage 3.51: If `storage_ty` is a fat pointer (`{ ptr, len }` struct),
/// load the data pointer from field 0 and return `(data_ptr, pointee_ty)`.
/// Otherwise, return `(base_ptr, None)` (array case — caller uses
/// `emit_gep_index` with the array type directly).
///
/// This is used by `Index`/`ConstantIndex` projections to handle the
/// difference between:
///   - `[T; N]` array: `storage_ty = Array(T, N)`, GEP directly into the
///     array storage at `base_ptr` using `emit_gep_index`.
///   - `&[T]` slice (fat pointer): `storage_ty = Struct([Ptr(T), I64])`,
///     must first load `Ptr(T)` from field 0 of the fat pointer, then GEP
///     into the data pointer using `emit_gep_index_ptr`.
///
/// Returns `(gep_base, pointee_ty_opt)`:
/// - For arrays: `(base_ptr, None)` — caller uses `emit_gep_index`.
/// - For fat pointers: `(data_ptr, Some(pointee_ty))` — caller uses
///   `emit_gep_index_ptr`.
pub(crate) fn unwrap_fat_ptr_for_index(
    emitter: &mut dyn Emitter,
    base_ptr: &str,
    storage_ty: &EmitType,
) -> (String, Option<EmitType>) {
    match storage_ty {
        EmitType::Struct(fields) if fields.len() == 2 => {
            let is_fat_ptr = fields[0].is_ptr() && fields[1] == EmitType::I64;
            if is_fat_ptr {
                // Fat pointer: load the data pointer from field 0.
                let base_ptr_owned = base_ptr.to_string();
                let data_ptr = emitter.emit_gep_field(&base_ptr_owned, storage_ty, 0);
                let pointee_ty = fields[0].pointee();
                (data_ptr, Some(pointee_ty))
            } else {
                (base_ptr.to_string(), None)
            }
        }
        _ => (base_ptr.to_string(), None),
    }
}

#[allow(clippy::only_used_in_recursion)]
pub(crate) fn codegen_place_load_typed(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    lv: &Place,
    ty: EmitType,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
) -> EmitValue {
    match &lv.kind {
        PlaceKind::Local(id) => {
            if let Some(val) = emitter.get_local(id.0).cloned() {
                if !val.starts_with('%') {
                    return val;
                }
            }
            if let Some(ptr) = emitter.get_local_ptr(id.0).cloned() {
                emitter.emit_load(&ty, &ptr)
            } else {
                "0".to_string()
            }
        }
        PlaceKind::Projection(base, elem) => match elem {
            ProjectionElem::Deref => {
                let ptr_ty = detect_place_type(mir, base, layouts);
                let ptr_val =
                    codegen_place_load_typed(emitter, mir, base, ptr_ty.clone(), interner, layouts);
                emitter.emit_load(&ty, &ptr_val)
            }
            ProjectionElem::Field(field_id, _) => {
                // Stage 14.19 (GAP-31): Handle Deref+Field projection correctly.
                // When the base is a Deref (e.g. `(*self).field` for &self/&mut self
                // methods), we need to load the POINTER from the base (the &self
                // reference), then GEP through that pointer to get the field.
                // Previously, this fell through to codegen_place_load_typed which
                // loaded the entire struct VALUE and then tried to GEP the value
                // (invalid LLVM IR — caused segfaults).
                //
                // Stage 14.43: Handle nested Field projection (e.g., `self.inner.val`).
                // When the base is itself a Field projection (e.g., `self.inner`),
                // we need the ADDRESS of the inner field, not its loaded value.
                // Was: codegen_place_load_typed loaded the inner struct value, then
                // GEP-ed into it as if it were a pointer → invalid IR + LLVM error.
                // Fix: use compute_place_address for nested Field projections.
                let base_ptr = if let PlaceKind::Local(id) = &base.kind {
                    emitter
                        .get_local_ptr(id.0)
                        .cloned()
                        .unwrap_or_else(|| "0".to_string())
                } else if let PlaceKind::Projection(inner_base, ProjectionElem::Deref) = &base.kind
                {
                    // base is (*inner).field — load the pointer from inner_base,
                    // then GEP through it.
                    let ptr_ty = detect_place_type(mir, inner_base, layouts);
                    let ptr_val = codegen_place_load_typed(
                        emitter,
                        mir,
                        inner_base,
                        ptr_ty.clone(),
                        interner,
                        layouts,
                    );
                    // The pointer value IS the base address for GEP
                    ptr_val
                } else if let PlaceKind::Projection(_, ProjectionElem::Field(_, _)) = &base.kind {
                    // Stage 14.43: base is a nested Field projection (e.g., self.inner).
                    // Compute its ADDRESS (GEP to the inner field), don't load the value.
                    compute_place_address(emitter, mir, base, interner, layouts)
                } else {
                    let ptr_ty = detect_place_type(mir, base, layouts);
                    codegen_place_load_typed(emitter, mir, base, ptr_ty, interner, layouts)
                };
                let struct_ty = detect_place_storage_type(mir, base, layouts);
                let field_ptr = emitter.emit_gep_field(&base_ptr, &struct_ty, field_id.0);
                emitter.emit_load(&ty, &field_ptr)
            }
            ProjectionElem::Index(idx) => {
                // Stage 14.21 (GAP-31): Handle Deref+Field+Index and Field+Index
                // projections correctly. When the base is a Projection (e.g.
                // `(*self).data` for &self methods, or `self.data` for by-value),
                // we need the ADDRESS of the array, not its loaded value.
                // Was: codegen_place_load_typed loaded the value, then GEP tried
                // to index into the value (invalid for arrays — caused segfault).
                let base_ptr = if let PlaceKind::Local(id) = &base.kind {
                    emitter
                        .get_local_ptr(id.0)
                        .cloned()
                        .unwrap_or_else(|| "0".to_string())
                } else if let PlaceKind::Projection(inner_base, ProjectionElem::Deref) = &base.kind
                {
                    // base is (*inner) — load the pointer from inner_base
                    let ptr_ty = detect_place_type(mir, inner_base, layouts);
                    codegen_place_load_typed(emitter, mir, inner_base, ptr_ty, interner, layouts)
                } else {
                    // base is a Field projection (e.g. self.data) — compute its
                    // ADDRESS (GEP to the field), don't load the value.
                    compute_place_address(emitter, mir, base, interner, layouts)
                };
                let array_ty = detect_place_storage_type(mir, base, layouts);
                let idx_val = if let Some(v) = emitter.get_local(idx.0).cloned() {
                    v
                } else if let Some(ptr) = emitter.get_local_ptr(idx.0).cloned() {
                    emitter.emit_load(&EmitType::I32, &ptr)
                } else {
                    "0".to_string()
                };
                // Stage 3.51: if the storage type is a fat pointer ({ ptr, len }),
                // we need to load the data pointer from field 0 first, then GEP
                // into the data pointer (not the fat pointer struct). Was: GEP
                // directly into the fat pointer struct, which loaded the pointer
                // field instead of the element.
                let (gep_base, pointee_opt) =
                    unwrap_fat_ptr_for_index(emitter, &base_ptr, &array_ty);
                let elem_ptr = match pointee_opt {
                    Some(elem_ty) => emitter.emit_gep_index_ptr(&gep_base, &elem_ty, &idx_val),
                    None => emitter.emit_gep_index(&gep_base, &array_ty, &idx_val),
                };
                emitter.emit_load(&ty, &elem_ptr)
            }
            ProjectionElem::ConstantIndex { offset, .. } => {
                let base_ptr = if let PlaceKind::Local(id) = &base.kind {
                    emitter
                        .get_local_ptr(id.0)
                        .cloned()
                        .unwrap_or_else(|| "0".to_string())
                } else {
                    let ptr_ty = detect_place_type(mir, base, layouts);
                    codegen_place_load_typed(emitter, mir, base, ptr_ty, interner, layouts)
                };
                let array_ty = detect_place_storage_type(mir, base, layouts);
                // Stage 3.51: same fat pointer unwrap as Index.
                let (gep_base, pointee_opt) =
                    unwrap_fat_ptr_for_index(emitter, &base_ptr, &array_ty);
                let elem_ptr = match pointee_opt {
                    Some(elem_ty) => {
                        emitter.emit_gep_index_ptr(&gep_base, &elem_ty, &offset.to_string())
                    }
                    None => emitter.emit_gep_index(&gep_base, &array_ty, &offset.to_string()),
                };
                emitter.emit_load(&ty, &elem_ptr)
            }
            _ => "0".to_string(),
        },
        PlaceKind::Static(_) => "0".to_string(),
    }
}

pub(crate) fn codegen_place_load(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    lv: &Place,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
) -> EmitValue {
    // Stage 3.47 (L-PIPE-1 closure): previously this fabricated a fake
    // `MirBody::new(Span::DUMMY)` to satisfy `codegen_place_load_typed`'s
    // `&MirBody` parameter (which was needed to access local_decls for type
    // info). Now we pass the caller's `mir` reference through directly —
    // no fake MirBody needed. The `EmitType::I32` placeholder is the
    // historical default for untyped place loads (used when the caller
    // doesn't know the type ahead of time). The typed path
    // (`codegen_place_load_typed`) is preferred when the type is known.
    codegen_place_load_typed(emitter, mir, lv, EmitType::I32, interner, layouts)
}

pub(crate) fn detect_operand_type(
    mir: &MirBody,
    op: &Operand,
    layouts: &crate::mir::body::AdtLayouts,
) -> Option<EmitType> {
    match op {
        Operand::Constant(c) => {
            // Stage 3.46: use the constant's declared type if it's concrete
            // (not Infer). This ensures e.g. i16 constants use i16 ops.
            let from_ty = mir_type_to_emit_type_with_layouts(&c.ty, layouts);
            if from_ty != EmitType::I32 {
                Some(from_ty)
            } else {
                // Fallback: infer from value kind.
                match &c.val {
                    ConstVal::Float(_) => Some(EmitType::F64),
                    ConstVal::Bool(_) => Some(EmitType::I1),
                    ConstVal::Char(_) => Some(EmitType::I8),
                    _ => Some(EmitType::I32),
                }
            }
        }
        Operand::Copy(lv) | Operand::Move(lv) => {
            if let PlaceKind::Local(id) = &lv.kind {
                mir.local_decls
                    .get(id.0 as usize)
                    .map(|ld| mir_type_to_emit_type_with_layouts(&ld.ty, layouts))
            } else {
                Some(detect_place_type(mir, lv, layouts))
            }
        }
    }
}
