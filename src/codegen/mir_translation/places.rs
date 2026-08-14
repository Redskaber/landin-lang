//! §4.4: Place projection address computation + load.
//!
//! Per `docs/lang-design/07-codegen.md` §4.4 (Place projection mapping).
//! Contains 7 functions:
//! - `detect_place_storage_type`: classify a Place's storage (Direct/Indirect/FatPtr)
//! - `detect_place_type`: detect the EmitType of a Place's value
//! - `compute_place_address`: compute the LLVM value pointing to a Place
//! - `unwrap_fat_ptr_for_index`: extract data ptr from fat ptr for indexing
//! - `codegen_place_load_typed`: emit load of a typed Place
//! - `codegen_place_load`: convenience wrapper for codegen_place_load_typed
//! - `detect_operand_type`: detect the EmitType of an Operand
//!
//! Per §16: reads MIR data (Place, MirBody) — no HIR.

use crate::codegen::mir_translation::types::mir_type_to_emit_type_with_layouts;
use crate::codegen::{EmitType, EmitValue, Emitter};
use crate::mir::body::*;
use crate::mir::place::*;
use crate::mir::ty::*;
use lasso::Rodeo;

pub(crate) fn detect_place_storage_type(
    mir: &MirBody,
    lv: &Place,
    layouts: &crate::mir::body::AdtLayouts,
) -> EmitType {
    match &lv.kind {
        PlaceKind::Local(id) => {
            // Stage 16.23: For Closure-typed self in synthesized functions,
            // the storage type is the struct layout (with capture fields).
            // Only applies to synthesized functions (mir.def_id.is_some()).
            let ld = mir.local_decls.get(id.0 as usize);
            if let Some(ld) = ld {
                if mir.def_id.is_some()
                    && id.0 == 1
                    && matches!(ld.ty.kind, crate::mir::ty::TyKind::Closure(_, _))
                {
                    if let crate::mir::ty::TyKind::Closure(_, substs) = &ld.ty.kind {
                        let fields: Vec<EmitType> = substs
                            .iter()
                            .map(|t| mir_type_to_emit_type_with_layouts(t, layouts))
                            .collect();
                        return EmitType::Struct(fields);
                    }
                }
                mir_type_to_emit_type_with_layouts(&ld.ty, layouts)
            } else {
                EmitType::I32
            }
        }
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
                // Stage 14.61: If the base is a Ref to an array (e.g., `&[i32; 3]`),
                // detect_place_storage_type returns the Ref type (Ptr/OpaquePtr).
                // We need to check the MIR type for Ref(_, _, Array) and extract
                // the array type from the inner.
                let array_ty = if matches!(array_ty, EmitType::OpaquePtr | EmitType::Ptr(_)) {
                    // Check if the base MIR type is Ref(_, _, Array)
                    if let PlaceKind::Local(id) = &base.kind {
                        if let Some(ld) = mir.local_decls.get(id.0 as usize) {
                            if let crate::mir::ty::TyKind::Ref(_, _, inner) = &ld.ty.kind {
                                let inner_emit = mir_type_to_emit_type_with_layouts(inner, layouts);
                                if !matches!(inner_emit, EmitType::I32) {
                                    inner_emit
                                } else {
                                    array_ty
                                }
                            } else {
                                array_ty
                            }
                        } else {
                            array_ty
                        }
                    } else {
                        array_ty
                    }
                } else {
                    array_ty
                };
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
        PlaceKind::Local(id) => {
            let ld = mir.local_decls.get(id.0 as usize);
            // Stage 16.23: In synthesized closure functions, the `self`
            // parameter (LocalId(1)) has a Closure type but is stored as
            // a pointer (OpaquePtr) in the alloca. When detecting the
            // place type for loading, return OpaquePtr so the load
            // produces a pointer value (not a struct value).
            if let Some(ld) = ld {
                if id.0 == 1
                    && matches!(ld.ty.kind, crate::mir::ty::TyKind::Closure(_, _))
                    && mir.def_id.is_some()
                {
                    return EmitType::OpaquePtr;
                }
                mir_type_to_emit_type_with_layouts(&ld.ty, layouts)
            } else {
                EmitType::I32
            }
        }
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
                            // Stage 14.84 (audit fix #3): Also handle Closure
                            // base — extract the field type from the closure's
                            // substs (which were written back by the driver
                            // post-typeck). This fixes `|| p.y` codegen where
                            // the projection's field_ty was set to Infer at
                            // MIR-lower time but the closure local's substs
                            // are now resolved to [Adt(Point)].
                            if let crate::mir::ty::TyKind::Closure(_, substs) = &base_ld.ty.kind {
                                if let Some(resolved) = substs.get(field_id.0 as usize) {
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
            .local_ptr(id.0)
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
                // Stage 14.66: Handle `self.field` where `self` is `&self` (Ref).
                //
                // When the base is a Local whose type is Ref(_, _, Adt) (i.e.,
                // `&self` or `&mut self`), the alloca pointer points to a
                // POINTER (the reference), not the struct directly. We need to:
                // 1. Load the reference value from the alloca
                // 2. GEP through the loaded reference to access the field
                //
                // Without this fix, the code would GEP through the alloca
                // pointer directly, producing `getelementptr ptr, ptr %loc_1,
                // 0, 0` — invalid because `ptr` is not an aggregate.
                //
                // Per §1.0 原则 6 "通用 > 特例": one rule handles all Ref-based
                // field accesses (not just &self methods).
                let struct_ty = detect_place_storage_type(mir, base, layouts);
                // Stage 14.66: Handle `self.field` where `self` is `&self` (Ref).
                //
                // When the base is a Local whose type is Ref(_, _, Adt) (i.e.,
                // `&self` or `&mut self`), the alloca pointer points to a
                // POINTER (the reference), not the struct directly. We need to:
                // 1. Load the reference value from the alloca
                // 2. GEP through the loaded reference to access the field
                //
                // Without this fix, the code would GEP through the alloca
                // pointer directly, producing `getelementptr ptr, ptr %loc_1,
                // 0, 0` — invalid because `ptr` is not an aggregate.
                //
                // Per §1.0 原则 6 "通用 > 特例": one rule handles all Ref-based
                // field accesses (not just &self methods).
                let (base_addr, struct_ty) = if struct_ty.is_ptr() {
                    // base is a Ref — load the pointer value, then use the
                    // pointee type for GEP.
                    let pointee_ty = struct_ty.pointee();
                    // Check if pointee is an aggregate (struct/enum). If it's
                    // still OpaquePtr (unknown), try MIR type resolution.
                    let pointee_ty = if pointee_ty == EmitType::OpaquePtr {
                        // Resolve from MIR local type
                        if let PlaceKind::Local(id) = &base.kind {
                            if let Some(ld) = mir.local_decls.get(id.0 as usize) {
                                if let crate::mir::ty::TyKind::Ref(_, _, inner) = &ld.ty.kind {
                                    mir_type_to_emit_type_with_layouts(inner, layouts)
                                } else {
                                    pointee_ty
                                }
                            } else {
                                pointee_ty
                            }
                        } else {
                            pointee_ty
                        }
                    } else {
                        pointee_ty
                    };
                    // Only dereference if the pointee is a Struct (aggregate).
                    // If it's a primitive (e.g., &i32), don't dereference —
                    // the field access is on the pointer itself (rare).
                    if matches!(pointee_ty, EmitType::Struct(_)) {
                        let loaded_ptr = codegen_place_load_typed(
                            emitter,
                            mir,
                            base,
                            struct_ty.clone(),
                            _interner,
                            layouts,
                        );
                        (loaded_ptr, pointee_ty)
                    } else {
                        (base_addr, struct_ty)
                    }
                } else {
                    (base_addr, struct_ty)
                };
                emitter.emit_gep_field(&base_addr, &struct_ty, field_id.0)
            }
            // Stage 14.44: For Index projection in compute_place_address,
            // we need to GEP to the element's ADDRESS (not load the value).
            // Previously, this fell through to the `_` arm which called
            // codegen_place_load_typed (loads the value), breaking
            // `arr[i].field` patterns where Field wraps Index.
            ProjectionElem::Index(idx) => {
                // Stage 14.61: When the base is a Ref (e.g., `&[i32; 3]`),
                // we need to dereference the reference first to get the array
                // pointer, then GEP into the array.
                let base_ty = detect_place_type(mir, base, layouts);
                let base_ptr = if base_ty.is_ptr() {
                    // Ref to array — load the pointer value from the alloca
                    codegen_place_load_typed(
                        emitter,
                        mir,
                        base,
                        base_ty.clone(),
                        _interner,
                        layouts,
                    )
                } else {
                    compute_place_address(emitter, mir, base, _interner, layouts)
                };
                // Stage 14.61: For Ref to Array, extract the Array type from the
                // Ref's inner type for the GEP. detect_place_storage_type returns
                // Ptr(Array) which is wrong for GEP — we need Array itself.
                let array_ty = {
                    let raw_ty = detect_place_storage_type(mir, base, layouts);
                    match &raw_ty {
                        EmitType::Ptr(inner) => *inner.clone(),
                        EmitType::OpaquePtr => {
                            // Check MIR for Ref(_, _, Array)
                            if let PlaceKind::Local(id) = &base.kind {
                                if let Some(ld) = mir.local_decls.get(id.0 as usize) {
                                    if let crate::mir::ty::TyKind::Ref(_, _, inner) = &ld.ty.kind {
                                        mir_type_to_emit_type_with_layouts(inner, layouts)
                                    } else {
                                        raw_ty
                                    }
                                } else {
                                    raw_ty
                                }
                            } else {
                                raw_ty
                            }
                        }
                        _ => raw_ty,
                    }
                };
                let idx_val = if let Some(v) = emitter.local(idx.0).cloned() {
                    v
                } else if let Some(ptr) = emitter.local_ptr(idx.0).cloned() {
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
            if let Some(val) = emitter.local(id.0).cloned() {
                if !val.starts_with('%') {
                    return val;
                }
            }
            if let Some(ptr) = emitter.local_ptr(id.0).cloned() {
                emitter.emit_load(&ty, &ptr)
            } else {
                "0".to_string()
            }
        }
        PlaceKind::Projection(base, elem) => match elem {
            ProjectionElem::Deref => {
                // Stage 14.66: Handle `*v` where `v` is a value (not a reference).
                //
                // When matching `Some(v) => *v` on `&self`, the enum variant
                // pattern binding extracts the payload VALUE (i32) into `v`.
                // But the user wrote `*v`, expecting `v` to be a reference.
                // This produces `load i32, i32 %v14` — invalid IR.
                //
                // Fix: check if the base's MIR type is a Ref. If it's NOT a
                // Ref (i.e., it's already a value), return the value directly
                // without loading (treat `*v` as `v` for non-reference types).
                //
                // Per §1.0 原则 5 "报错 > 静默": silently treating `*value`
                // as `value` is a workaround, but it matches the semantic
                // intent (the user wants the value, and `*` on a non-reference
                // is a no-op in this context).
                let base_is_ref = if let PlaceKind::Local(id) = &base.kind {
                    mir.local_decls
                        .get(id.0 as usize)
                        .map(|ld| matches!(&ld.ty.kind, crate::mir::ty::TyKind::Ref(_, _, _)))
                        .unwrap_or(false)
                } else {
                    // For projections, check the place type
                    let base_ty = detect_place_type(mir, base, layouts);
                    base_ty.is_ptr()
                };
                if !base_is_ref {
                    // Base is not a reference — `*v` on a value is a no-op.
                    // Return the value directly.
                    let val_ty = detect_place_type(mir, base, layouts);
                    codegen_place_load_typed(emitter, mir, base, val_ty, interner, layouts)
                } else {
                    let ptr_ty = detect_place_type(mir, base, layouts);
                    let ptr_val = codegen_place_load_typed(
                        emitter,
                        mir,
                        base,
                        ptr_ty.clone(),
                        interner,
                        layouts,
                    );
                    emitter.emit_load(&ty, &ptr_val)
                }
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
                    // Stage 14.66: If the local is a Ref (e.g., `&self`),
                    // load the reference value (the pointer) — don't use
                    // the alloca pointer directly. The alloca points to a
                    // POINTER (the reference), not the struct.
                    // Stage 16.23: Also handle Closure types — the `self`
                    // parameter in synthesized closure functions is a
                    // Closure-typed local whose alloca stores a pointer
                    // (passed as OpaquePtr). We need to load the pointer
                    // value from the alloca before GEP-ing into it.
                    let local_ty = mir.local_decls.get(id.0 as usize).map(|ld| ld.ty.clone());
                    if let Some(ty) = local_ty {
                        if matches!(&ty.kind, crate::mir::ty::TyKind::Ref(_, _, _))
                            || (mir.def_id.is_some()
                                && id.0 == 1
                                && matches!(&ty.kind, crate::mir::ty::TyKind::Closure(_, _)))
                        {
                            // Ref or Closure — load the pointer value
                            let ptr_ty = detect_place_type(mir, base, layouts);
                            codegen_place_load_typed(emitter, mir, base, ptr_ty, interner, layouts)
                        } else {
                            emitter
                                .local_ptr(id.0)
                                .cloned()
                                .unwrap_or_else(|| "0".to_string())
                        }
                    } else {
                        emitter
                            .local_ptr(id.0)
                            .cloned()
                            .unwrap_or_else(|| "0".to_string())
                    }
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
                let mut struct_ty = detect_place_storage_type(mir, base, layouts);
                // Stage 14.66: If base is a Ref, the storage type is a pointer.
                // Use the pointee type (the actual struct) for GEP.
                if struct_ty.is_ptr() {
                    let pointee = struct_ty.pointee();
                    if matches!(pointee, EmitType::Struct(_)) {
                        struct_ty = pointee;
                    } else if pointee == EmitType::OpaquePtr {
                        // Try MIR resolution
                        if let PlaceKind::Local(id) = &base.kind {
                            if let Some(ld) = mir.local_decls.get(id.0 as usize) {
                                // Stage 16.28: Handle Ref types — resolve pointee
                                if let crate::mir::ty::TyKind::Ref(_, _, inner) = &ld.ty.kind {
                                    let resolved =
                                        mir_type_to_emit_type_with_layouts(inner, layouts);
                                    if matches!(resolved, EmitType::Struct(_)) {
                                        struct_ty = resolved;
                                    }
                                }
                                // Stage 16.28 (通解): Handle Closure types —
                                // resolve struct layout from substs (capture fields).
                                // This is the general solution for ALL capture types
                                // (i32, struct, nested struct, etc.), not just i32.
                                if let crate::mir::ty::TyKind::Closure(_, substs) = &ld.ty.kind {
                                    let fields: Vec<EmitType> = substs
                                        .iter()
                                        .map(|t| mir_type_to_emit_type_with_layouts(t, layouts))
                                        .collect();
                                    struct_ty = EmitType::Struct(fields);
                                }
                            }
                        }
                    }
                }
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
                //
                // Stage 14.61: When base is a Local with Ref type (e.g., `&[i32; 3]`),
                // load the reference value (the array pointer) instead of using
                // the alloca pointer directly.
                let base_ptr = if let PlaceKind::Local(id) = &base.kind {
                    let local_ty = mir.local_decls.get(id.0 as usize).map(|ld| ld.ty.clone());
                    if let Some(ty) = local_ty {
                        if matches!(&ty.kind, crate::mir::ty::TyKind::Ref(_, _, _)) {
                            // Ref to array — load the pointer value
                            let ptr_ty = detect_place_type(mir, base, layouts);
                            codegen_place_load_typed(emitter, mir, base, ptr_ty, interner, layouts)
                        } else {
                            emitter
                                .local_ptr(id.0)
                                .cloned()
                                .unwrap_or_else(|| "0".to_string())
                        }
                    } else {
                        emitter
                            .local_ptr(id.0)
                            .cloned()
                            .unwrap_or_else(|| "0".to_string())
                    }
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
                // Stage 14.61: Extract Array type from Ref for GEP.
                let array_ty = {
                    let raw_ty = detect_place_storage_type(mir, base, layouts);
                    match &raw_ty {
                        EmitType::Ptr(inner) => *inner.clone(),
                        EmitType::OpaquePtr => {
                            if let PlaceKind::Local(id) = &base.kind {
                                if let Some(ld) = mir.local_decls.get(id.0 as usize) {
                                    if let crate::mir::ty::TyKind::Ref(_, _, inner) = &ld.ty.kind {
                                        mir_type_to_emit_type_with_layouts(inner, layouts)
                                    } else {
                                        raw_ty
                                    }
                                } else {
                                    raw_ty
                                }
                            } else {
                                raw_ty
                            }
                        }
                        _ => raw_ty,
                    }
                };
                let idx_val = if let Some(v) = emitter.local(idx.0).cloned() {
                    v
                } else if let Some(ptr) = emitter.local_ptr(idx.0).cloned() {
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
                        .local_ptr(id.0)
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
                mir.local_decls.get(id.0 as usize).map(|ld| {
                    // Stage 16.27: For Closure-typed operands at call sites,
                    // use OpaquePtr. This ensures the forward declaration
                    // matches the function definition's param type.
                    // But only for Move (which is the self arg in closure calls) —
                    // Copy is used for regular closure struct values.
                    if matches!(ld.ty.kind, crate::mir::ty::TyKind::Closure(_, _))
                        && matches!(op, Operand::Move(_))
                    {
                        EmitType::OpaquePtr
                    } else {
                        mir_type_to_emit_type_with_layouts(&ld.ty, layouts)
                    }
                })
            } else {
                Some(detect_place_type(mir, lv, layouts))
            }
        }
    }
}
