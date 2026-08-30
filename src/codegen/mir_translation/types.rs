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

/// Stage 18.336 (P1 soundness fix): Filter `EmitType::Void` from a list of
/// struct/tuple/enum-payload field types.
///
/// Per §20 Round 5 audit: ZST fields (`()`) leak `EmitType::Void` into nested
/// aggregate positions (struct field, tuple element, enum payload, array
/// element). LLVM IR rejects `{ void }`, `[3 x void]`, etc. with "void type
/// only allowed for function results".
///
/// Mirrors rustc_codegen_llvm: ZST fields are elided from the LLVM struct
/// type (Rust ABI doesn't allocate space for them in the struct layout).
///
/// If ALL fields are Void (fully ZST aggregate), returns `Struct(vec![])`
/// (LLVM `{}`, valid as a struct type — the issue is only with `Void` fields
/// INSIDE a struct, not with empty structs themselves).
///
/// Per §1.0 原則 6 (通解 > 特解): one fix covers all 4 cases (A1-A4 same class
/// — struct field, tuple element, enum payload, array element).
/// Per §20 (iterative audit): same root cause as Stage 18.335 ZST param elision.
pub(crate) fn filter_void_fields(fields: Vec<EmitType>) -> EmitType {
    let filtered: Vec<EmitType> = fields
        .into_iter()
        .filter(|ty| *ty != EmitType::Void)
        .collect();
    if filtered.is_empty() {
        // All fields were ZST → represent as LLVM `{}` (valid empty struct).
        // (Not `Void` — Void is only valid as a function return type, not as
        // a struct field or array element.)
        EmitType::Struct(vec![])
    } else {
        EmitType::Struct(filtered)
    }
}

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
                    //   — VALID per LLVM Language Reference.
                    // - The alloca would be `alloca {}` — but per LLVM docs, size-0
                    //   allocas produce undef pointers (UB to dereference). So both
                    //   emitters (`text/mod.rs:48-57` + `llvm/mod.rs:618-620`) use
                    //   an `i8` fallback (1-byte placeholder) for ZST allocas.
                    //   The i8 byte is never read for true ZSTs.
                    // - The receiver `&self` is the alloca pointer (valid `ptr`).
                    //
                    // Per §1.0 原则 6 "通用 > 特例": same code path as non-empty
                    // structs, just with zero fields.
                    //
                    // Stage 18.335: Comment corrected per §20 Round 4 audit —
                    // previous comment claimed `alloca {}` is "valid, zero-size"
                    // which was misleading (LLVM docs say size-0 allocas are UB).
                    EmitType::Struct(vec![])
                } else {
                    // Stage 3.47: recurse with `layouts` so nested Adts
                    // resolve correctly (e.g., `struct Outer { i: Inner }`
                    // renders as `{ { i32 } }`, not `{ i32 }`).
                    // Stage 18.336 (P1 soundness fix): Filter Void fields
                    // (ZST fields like `()` would leak `Void` into the struct
                    // type → `llvm-as` rejects `{ void }`).
                    let fields: Vec<EmitType> = field_tys
                        .iter()
                        .map(|t| mir_type_to_emit_type_with_layouts(t, layouts))
                        .collect();
                    filter_void_fields(fields)
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
                // Stage 18.336 (P1 soundness fix): Filter Void fields (ZST
                // payloads like `()` would leak `Void` into the enum storage
                // struct → `llvm-as` rejects).
                let mut field_tys =
                    vec![mir_type_to_emit_type_with_layouts(discriminant_ty, layouts)];
                for payload in variant_payloads {
                    for t in payload {
                        field_tys.push(mir_type_to_emit_type_with_layouts(t, layouts));
                    }
                }
                filter_void_fields(field_tys)
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
                // Stage 18.336 (P1 soundness fix): Filter Void tuple elements
                // (ZST elements like `()` would leak `Void` into the struct
                // type → `llvm-as` rejects `{ i32, void }`).
                let fields: Vec<EmitType> = tys
                    .iter()
                    .map(|t| mir_type_to_emit_type_with_layouts(t, layouts))
                    .collect();
                filter_void_fields(fields)
            }
        }
        TyKind::Array(elem, len) => {
            let n = match &len.val {
                ConstVal::Int(n) | ConstVal::Uint(n) => *n as u64,
                _ => 0,
            };
            // Stage 18.336 (P1 soundness fix): ZST array element (e.g., `[(); 3]`)
            // would produce `[3 x void]` → `llvm-as` rejects. Use `Struct(vec![])`
            // (LLVM `{}`) as the element type instead → `[3 x {}]` is valid
            // (zero-size array).
            let elem_ty = mir_type_to_emit_type_with_layouts(elem, layouts);
            let elem_ty = if elem_ty == EmitType::Void {
                EmitType::Struct(vec![])
            } else {
                elem_ty
            };
            EmitType::array_of(elem_ty, n)
        }
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) => {
            // Stage 3.49 (L13 closure): `&str` and `&[T]` are fat pointers
            // `{ ptr, len }`. Other references remain thin pointers.
            //
            // Stage 18.337 (P1 soundness fix): For Ref/RawPtr to an Adt
            // (struct/enum), use opaque `ptr` WITHOUT recursing into the
            // pointee type. This breaks recursive struct cycles (e.g.,
            // `struct Node { next: *mut Node }`) that caused infinite
            // recursion + stack overflow.
            //
            // In LLVM 17+ opaque pointer mode, all pointers are `ptr` —
            // the pointee type is NOT needed for the pointer's LLVM type.
            // The pointee's layout is only needed when the pointer is
            // dereferenced (load/store/GEP), which happens separately in
            // codegen_statement/codegen_rvalue with the correct type from
            // local_decls.
            //
            // Mirrors rustc_codegen_llvm: pointers to structs are `ptr`
            // in LLVM IR; the struct type is only used at dereference sites.
            //
            // Per §1.0 原則 6 (通解 > 特解): one opaque-ptr rule for all
            // Ref/RawPtr to Adt — no special-casing per recursion depth.
            // Per §1.0 原則 9 (正确 > 妥协): correct opaque pointer semantics
            // > matching the pointee's LLVM type (which isn't needed).
            // Per §20 (iterative audit): found via §20 Round 6 audit —
            // recursive struct caused stack overflow.
            match &inner.kind {
                TyKind::Str => crate::codegen::emit_fat_ptr_type(EmitType::I8),
                TyKind::Slice(elem) => crate::codegen::emit_fat_ptr_type(
                    mir_type_to_emit_type_with_layouts(elem, layouts),
                ),
                // Stage 18.337: For Adt pointee, use opaque ptr — do NOT
                // recurse into the Adt's layout (would infinite-loop on
                // recursive types like `struct Node { next: *mut Node }`).
                TyKind::Adt(_, _) => EmitType::OpaquePtr,
                // For non-Adt, non-Slice, non-Str pointee (primitives, tuples,
                // arrays, closures): recurse is safe (no cycle possible).
                _ => EmitType::ptr_to(mir_type_to_emit_type_with_layouts(inner, layouts)),
            }
        }
        TyKind::Slice(elem) => EmitType::ptr_to(mir_type_to_emit_type_with_layouts(elem, layouts)),
        // Stage 18.176: Bare TyKind::Str (not inside Ref) — treat as fat pointer.
        // This makes `let s: String = "hello"` work (String = Str alias).
        // Per §1.0 原則 6 (通解>特例): same fat pointer as &str.
        TyKind::Str => crate::codegen::emit_fat_ptr_type(EmitType::I8),
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
        // Stage 18.336: Filter Void from closure captures (ZST captures
        // would leak `Void` into the closure struct type).
        TyKind::Closure(_, substs) => {
            let fields: Vec<EmitType> = substs
                .iter()
                .map(|ty| mir_type_to_emit_type_with_layouts(ty, layouts))
                .collect();
            filter_void_fields(fields)
        }
        // Stage 18.444 (Phase 5 Step 5): Was `_ => mir_type_to_emit_type(ty)`
        // which delegates to the unchecked variant (with I32 fallback for
        // unresolved types). Reverted to this because it caused infinite
        // recursion when `mir_type_to_emit_type_with_layouts` calls itself
        // for types like Infer/Error/Param that don't have layouts entries.
        // These types should be caught by param_check (Stage 18.348) before
        // reaching codegen — the unchecked variant's warning is defense-in-depth.
        //
        // Per §1.0 原則 9 (正确 > 妥协): can't use with_layouts for
        // non-Adt unresolved types (Infinite recursion for Infer/Error).
        // Per §13.4: incremental — this caller stays unchecked for now.
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
                // Stage 18.336 (P1 soundness fix): Filter Void tuple elements
                // (ZST elements like `()` would leak `Void` into the struct
                // type → `llvm-as` rejects).
                let fields: Vec<EmitType> = tys
                    .iter()
                    .map(|t| mir_type_to_emit_type_with_layouts_and_mono(t, layouts, mono_layouts))
                    .collect();
                filter_void_fields(fields)
            }
        }
        TyKind::Array(elem, len) => {
            let n = match &len.val {
                ConstVal::Int(n) | ConstVal::Uint(n) => *n as u64,
                _ => 0,
            };
            // Stage 18.336 (P1 soundness fix): ZST array element (e.g., `[(); 3]`)
            // would produce `[3 x void]` → `llvm-as` rejects. Use `Struct(vec![])`
            // (LLVM `{}`) as the element type instead → `[3 x {}]` is valid.
            let elem_ty = mir_type_to_emit_type_with_layouts_and_mono(elem, layouts, mono_layouts);
            let elem_ty = if elem_ty == EmitType::Void {
                EmitType::Struct(vec![])
            } else {
                elem_ty
            };
            EmitType::array_of(elem_ty, n)
        }
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) => {
            // Stage 18.337 (P1 soundness fix): Same Adt→OpaquePtr fix as
            // `_with_layouts` variant. Breaks recursive struct cycles.
            match &inner.kind {
                TyKind::Str => crate::codegen::emit_fat_ptr_type(EmitType::I8),
                TyKind::Slice(elem) => crate::codegen::emit_fat_ptr_type(
                    mir_type_to_emit_type_with_layouts_and_mono(elem, layouts, mono_layouts),
                ),
                // Stage 18.337: For Adt pointee, use opaque ptr — do NOT recurse.
                TyKind::Adt(_, _) => EmitType::OpaquePtr,
                _ => EmitType::ptr_to(mir_type_to_emit_type_with_layouts_and_mono(
                    inner,
                    layouts,
                    mono_layouts,
                )),
            }
        }
        TyKind::Slice(elem) => EmitType::ptr_to(mir_type_to_emit_type_with_layouts_and_mono(
            elem,
            layouts,
            mono_layouts,
        )),
        // Stage 18.176: Bare TyKind::Str — fat pointer (same as with_layouts).
        TyKind::Str => crate::codegen::emit_fat_ptr_type(EmitType::I8),
        TyKind::Closure(_, substs) => {
            let fields: Vec<EmitType> = substs
                .iter()
                .map(|ty| mir_type_to_emit_type_with_layouts_and_mono(ty, layouts, mono_layouts))
                .collect();
            // Stage 18.336 (P1 soundness fix): Filter Void from closure captures.
            filter_void_fields(fields)
        }
        // All other kinds — delegate to the existing function.
        _ => mir_type_to_emit_type_with_layouts(ty, layouts),
    }
}
