//! Stage 16.76 MUV-2: Drop glue function emission.
//!
//! Per `docs/lang-design/07-codegen.md` §6 (Drop glue) + §25 (Drop elaboration).
//! Emits recursive drop glue functions for all types that need drop.
//!
//! Extraction from `codegen/mod.rs` per §13.4 J2 (single responsibility).

use crate::codegen::emitter::Emitter;
use lasso::Rodeo;

/// Stage 15.57 (HP-12): Emit drop glue functions for types that need drop.
///
/// Stage 15.63: Extended to emit drop glue for ALL types needing drop
/// (not just types with `impl Drop`). For each type `T` where
/// `ty_needs_drop(T)` is true, emit a function:
///
/// ```llvm
/// define void @drop_adt_<DefId>(ptr %self) {
///     ; If T has impl Drop: call the user's Drop::drop method.
///     call void @"landin_T_drop"(ptr %self)
///     ; Recursively drop each field that needs drop.
///     %field0 = getelementptr inbounds { i32, %struct.Inner }, ptr %self, i32 0, i32 1
///     call void @drop_adt_<InnerDefId>(ptr %field0)
///     ret void
/// }
/// ```
///
/// The function name `drop_adt_<DefId>` matches what `TerminatorKind::Drop`
/// codegen calls (Stage 15.45). The user's `Drop::drop` method is called
/// with the place pointer as `&mut self` (if the type has `impl Drop`).
/// Then each field that needs drop is recursively dropped via GEP + call.
///
/// ## Recursive drop
///
/// For a struct `Outer { inner: Inner }` where `Inner` has `impl Drop`:
/// - `Outer` does NOT have `impl Drop`, but `ty_needs_drop(Outer)` returns
///   true (because its field `inner` needs drop).
/// - `emit_drop_glue_functions` emits `drop_adt_<OuterDefId>` that GEPs to
///   `inner` and calls `drop_adt_<InnerDefId>`.
/// - `elaborate_drops` inserts `Drop { place: outer, ... }` at scope end.
/// - The `Drop` terminator calls `drop_adt_<OuterDefId>`, which calls
///   `drop_adt_<InnerDefId>`, which calls `landin_Inner_drop`.
///
/// This matches Rust's recursive drop semantics.
///
/// Per §23: function name follows `drop_<noun>_<id>` pattern.
/// Per §16: reads TraitResolver + AdtLayouts + fn_name_by_def_id (data only, no HIR).
pub(crate) fn emit_drop_glue_functions(
    resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    _fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
    adt_layouts: &crate::mir::body::AdtLayouts,
    emitter: &mut dyn Emitter,
) {
    use crate::codegen::emitter::{mir_type_to_emit_type, EmitType};
    use crate::mir::body::AdtLayout;
    use crate::mir::drop_elaboration::ty_needs_drop;
    use crate::mir::ty::{Ty, TyKind};

    // Stage 16.08: `is_drop_builtin` now uses DefId-keyed lookup internally
    // (Task 3 Step 3). No need to pre-resolve the Drop trait DefId here —
    // the method handles it. This simplifies the codegen path.
    //
    // The old Stage 16.07 code pre-resolved `drop_def_id` and called
    // `implements_by_def_ids` directly, with a Spur-based fallback. Now
    // that `is_drop_builtin` uses DefId-keyed lookup, we can just call it
    // directly — it's both cleaner and handles the fallback internally.

    // Stage 15.63: Iterate ALL types in `type_by_def_id`, not just types
    // with `impl Drop`. For each type, check `ty_needs_drop`. If it needs
    // drop, emit drop glue.
    //
    // This handles two cases:
    // 1. Types WITH `impl Drop`: call user's drop + recursively drop fields.
    // 2. Types WITHOUT `impl Drop` but with fields needing drop: recursively
    //    drop fields only.
    for (&def_id, &type_spur) in &resolver.type_by_def_id {
        let ty = Ty::new(
            TyKind::Adt(def_id, Vec::new().into()),
            crate::session::Span::DUMMY,
        );

        // Skip types that don't need drop.
        if !ty_needs_drop(&ty, resolver, adt_layouts, interner) {
            continue;
        }

        // Stage 16.08: Check if this type has `impl Drop` via `is_drop_builtin`,
        // which now uses DefId-keyed lookup (Task 3 Step 3).
        let has_drop_impl = resolver.is_drop_builtin(def_id, interner);

        // Get the type name (for the user's drop method name).
        let type_name = interner.resolve(&type_spur).to_string();

        // Emit the drop glue function: `drop_adt_<DefId>`.
        let drop_fn_name = format!("drop_adt_{}", def_id.0);

        // Declare the user's drop method (if the type has impl Drop).
        if has_drop_impl {
            let drop_method_name = format!("landin_{}_drop", type_name);
            emitter.emit_declare(&format!("void @{}(ptr %self)", drop_method_name));
        }

        // Get the AdtLayout for this type (to know field types for recursive drop).
        let layout = adt_layouts.get(&def_id);

        // Collect struct fields that need drop (for the struct case).
        // For enums, we handle variant payloads separately (SwitchInt dispatch).
        let mut fields_to_drop: Vec<(u32, Option<crate::hir::DefId>, EmitType)> = Vec::new();

        // Stage 15.66: For enums, collect per-variant payload fields that need drop.
        // Each entry: (variant_idx, field_offset_within_enum, field_def_id, field_emit_ty).
        let mut enum_variants_to_drop: Vec<Vec<(u32, Option<crate::hir::DefId>)>> = Vec::new();
        let mut enum_has_drop_variants = false;

        if let Some(layout) = &layout {
            match layout {
                AdtLayout::Struct { field_tys } => {
                    for (idx, field_ty) in field_tys.iter().enumerate() {
                        if ty_needs_drop(field_ty, resolver, adt_layouts, interner) {
                            let field_def_id = match &field_ty.kind {
                                TyKind::Adt(fid, _) => Some(*fid),
                                _ => None,
                            };
                            let field_emit_ty = mir_type_to_emit_type(field_ty);
                            fields_to_drop.push((idx as u32, field_def_id, field_emit_ty));
                        }
                    }
                }
                AdtLayout::Enum {
                    variant_payloads, ..
                } => {
                    // Stage 15.66: Recursive drop for enums.
                    //
                    // The enum layout is a flattened struct:
                    //   { discriminant, variant0_fields..., variant1_fields..., ... }
                    //
                    // To drop the active variant's payload:
                    // 1. Load the discriminant (field 0).
                    // 2. SwitchInt on the discriminant.
                    // 3. Each variant's block GEPs to its payload fields and drops them.
                    // 4. All variants branch to a merge block.
                    //
                    // The field offset for variant V's field F is:
                    //   1 (discriminant) + sum of (variant 0..V-1 payload lengths) + F
                    let mut field_offset = 1u32; // skip discriminant (field 0)
                    for payload in variant_payloads {
                        let mut variant_fields_to_drop: Vec<(u32, Option<crate::hir::DefId>)> =
                            Vec::new();
                        for (f_idx, field_ty) in payload.iter().enumerate() {
                            if ty_needs_drop(field_ty, resolver, adt_layouts, interner) {
                                let field_def_id = match &field_ty.kind {
                                    TyKind::Adt(fid, _) => Some(*fid),
                                    _ => None,
                                };
                                variant_fields_to_drop
                                    .push((field_offset + f_idx as u32, field_def_id));
                            }
                        }
                        if !variant_fields_to_drop.is_empty() {
                            enum_has_drop_variants = true;
                        }
                        enum_variants_to_drop.push(variant_fields_to_drop);
                        field_offset += payload.len() as u32;
                    }
                }
            }
        }

        // Build the struct's LLVM type string for GEP (from field types).
        let struct_llvm_ty = match &layout {
            Some(AdtLayout::Struct { field_tys }) => {
                let field_emit_tys: Vec<EmitType> =
                    field_tys.iter().map(mir_type_to_emit_type).collect();
                EmitType::Struct(field_emit_tys)
            }
            Some(AdtLayout::Enum {
                discriminant_ty,
                variant_payloads,
            }) => {
                // Flatten: { discriminant, all variant payload fields... }
                let mut field_emit_tys = vec![mir_type_to_emit_type(discriminant_ty)];
                for payload in variant_payloads {
                    for t in payload {
                        field_emit_tys.push(mir_type_to_emit_type(t));
                    }
                }
                EmitType::Struct(field_emit_tys)
            }
            None => EmitType::OpaquePtr, // fallback for missing layout
        };

        // Define the drop glue function.
        let self_str = "self".to_string();
        emitter.emit_function_begin(
            &drop_fn_name,
            &[(EmitType::OpaquePtr, &self_str)],
            &EmitType::Void,
        );

        // If the type has impl Drop, call the user's Drop::drop method.
        if has_drop_impl {
            let drop_method_name = format!("landin_{}_drop", type_name);
            emitter.emit_call(
                &drop_method_name,
                &[(EmitType::OpaquePtr, &self_str)],
                &EmitType::Void,
            );
        }

        // Stage 18.193 (TD-BOX-AUTO-DROP): For Box<T>, call __landin_dealloc
        // on the inner pointer (field 0). Box owns a heap allocation that
        // must be freed when the Box goes out of scope.
        //
        // Per §1.0 原則 6 (通解>特例): this is the canonical auto-drop for
        // owned heap types. Future: generalize via a "owns allocation" trait.
        if type_name == "Box" && !has_drop_impl {
            // Load field 0 (the *mut T pointer) from the Box struct.
            let ptr_field_addr = emitter.emit_gep_field(&self_str, &struct_llvm_ty, 0);
            let ptr_val = emitter.emit_load(&EmitType::ptr_to(EmitType::I8), &ptr_field_addr);
            // Stage 18.193: Check if pointer is null before deallocating.
            // Some locals have the same type as Box (e.g., FnDef constants
            // stored as { ptr }) but don't hold valid heap pointers. Skip
            // dealloc for null pointers (NULL-safe, matching __landin_dealloc).
            let null_val = "null".to_string();
            let is_not_null =
                emitter.emit_icmp("ne", &EmitType::ptr_to(EmitType::I8), &ptr_val, &null_val);
            let skip_bb = format!("drop_box_skip_{}", def_id.0);
            let dealloc_bb = format!("drop_box_dealloc_{}", def_id.0);
            emitter.emit_br_cond(&is_not_null, &dealloc_bb, &skip_bb);
            emitter.emit_block(&dealloc_bb);
            emitter.emit_call(
                "__landin_dealloc",
                &[(EmitType::ptr_to(EmitType::I8), &ptr_val)],
                &EmitType::Void,
            );
            emitter.emit_br(&skip_bb);
            emitter.emit_block(&skip_bb);
        }

        // Stage 15.66: For enums, emit SwitchInt dispatch to drop the active variant's payload.
        if enum_has_drop_variants {
            // Load the discriminant (field 0).
            let discr_addr = emitter.emit_gep_field(&self_str, &struct_llvm_ty, 0);
            let discr_ty = match &layout {
                Some(AdtLayout::Enum {
                    discriminant_ty, ..
                }) => mir_type_to_emit_type(discriminant_ty),
                _ => EmitType::I32,
            };
            let discr_val = emitter.emit_load(&discr_ty, &discr_addr);

            // Build switch cases: one block per variant that has drop fields.
            // Type alias for readability (avoids clippy::type-complexity).
            type VariantDropInfo = (Vec<(u32, Option<crate::hir::DefId>)>, String);
            let merge_label = format!("drop_enum_merge_{}", def_id.0);
            let mut cases: Vec<(i128, String)> = Vec::new();
            let mut variant_blocks: Vec<VariantDropInfo> = Vec::new();

            for (v_idx, variant_fields) in enum_variants_to_drop.iter().enumerate() {
                if !variant_fields.is_empty() {
                    let block_label = format!("drop_enum_v{}_{}", v_idx, def_id.0);
                    cases.push((v_idx as i128, block_label.clone()));
                    variant_blocks.push((variant_fields.clone(), block_label));
                }
            }

            // Emit the switch (default = merge block, since variants without
            // drop fields don't need any payload drop).
            emitter.emit_switch(&discr_val, &discr_ty, &cases, &merge_label);

            // Emit each variant's block: GEP + drop each payload field, then br merge.
            for (variant_fields, block_label) in &variant_blocks {
                emitter.emit_block(block_label);
                for (field_offset, field_def_id) in variant_fields {
                    let field_addr =
                        emitter.emit_gep_field(&self_str, &struct_llvm_ty, *field_offset);
                    if let Some(fid) = field_def_id {
                        let field_drop_fn = format!("drop_adt_{}", fid.0);
                        emitter.emit_call(
                            &field_drop_fn,
                            &[(EmitType::OpaquePtr, &field_addr)],
                            &EmitType::Void,
                        );
                    }
                }
                emitter.emit_br(&merge_label);
            }

            // Emit merge block.
            emitter.emit_block(&merge_label);
        } else {
            // Struct case: recursively drop each field that needs drop.
            for (field_idx, field_def_id, _field_emit_ty) in &fields_to_drop {
                let field_addr = emitter.emit_gep_field(&self_str, &struct_llvm_ty, *field_idx);
                if let Some(fid) = field_def_id {
                    let field_drop_fn = format!("drop_adt_{}", fid.0);
                    emitter.emit_call(
                        &field_drop_fn,
                        &[(EmitType::OpaquePtr, &field_addr)],
                        &EmitType::Void,
                    );
                }
                // For non-ADT fields that need drop (e.g., tuples, arrays),
                // we'd need a generic drop glue function. This is deferred to v0.3.
            }
        }

        emitter.emit_ret(&EmitType::Void, None);
        emitter.emit_function_end();
    }
}
