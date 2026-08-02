//! LLVM IR codegen: MIR → LLVM IR via Emitter trait.
//!
//! ## Status
//!
//! Stage 3 (v0.8.x) is COMPLETE. `codegen_crate` is §16-compliant —
//! it takes a `&CompileResult` (pre-built MIR + pre-computed metadata)
//! and makes zero upstream function calls (no `crate::mir::lower`,
//! no `crate::typeck`, no `crate::driver` beyond type-only references).
//!
//! Stage 3.46 (v0.8.6): full integer type support (i8/i16/i32/i64/i128).
//! Stage 3.63 (cross-stage naming standardization): `fat_ptr_type` →
//! `emit_fat_ptr_type` for prefix consistency with the
//! `mir_type_to_emit_type` / `emit_type_to_llvm_str` translation ladder.
//!
//! ## Open limitations (deferred to Stage 4+)
//!
//! All soundness-critical limitations are CLOSED. The 5 remaining open
//! limitations are soundness-non-critical and explicitly deferred:
//!
//! | ID | Description | Target |
//! |----|-------------|--------|
//! | L1 | PHI node optimization — **CLOSED in Stage 4.2** (design decision: rely on LLVM `mem2reg` rather than emitting PHI directly; documented below) | ✅ |
//! | L3 | Closure codegen — **IN PROGRESS (Stage 4.9)**: closure type lowering + capture analysis + closure call detection. Full call lowering (extract captures + invoke body) deferred to Stage 4.10. | Stage 4.10 |
//! | L5 | Trait dispatch (vtable generation, dyn fat pointers) | Stage 5 |
//! | L8 | `lli` execution verification (env constraint — no `lli` in test sandbox) | Stage 4 |
//! | L-COPY-ADT | Proper Copy trait (current borrowck pragmatically treats Adt as Copy) | Stage 5 |
//!
//! ## L1 PHI optimization — design decision (Stage 4.2)
//!
//! **Decision**: Landin codegen emits `alloca` + `load` + `store` for all
//! locals, and relies on LLVM's `mem2reg` optimization pass to produce SSA
//! form with PHI nodes. This is the **standard approach** used by Clang,
//! rustc, and most LLVM frontends.
//!
//! **Rationale**:
//! 1. `mem2reg` is a well-tested LLVM pass that produces optimal SSA form
//! 2. Implementing PHI emission manually would duplicate `mem2reg` logic
//!    and risk correctness bugs
//! 3. The current `alloca`-based IR is **correct** — it produces valid
//!    LLVM IR that any LLVM toolchain can optimize
//! 4. The IR quality concern is **non-blocking** — `opt -mem2reg` or
//!    `lli` (which runs default passes) produces optimal code
//!
//! **What was considered and rejected**: Emitting PHI nodes directly in
//! `codegen_function` by tracking SSA values per basic block. This would
//! require:
//! - A per-block value mapping (local → SSA value)
//! - PHI node insertion at block joins
//! - Dominance frontier computation
//! - Handling of partially-defined variables
//!
//! This is essentially reimplementing `mem2reg` in Rust — high effort,
//! high risk, low benefit over just running `opt -mem2reg`.
//!
//! **Conclusion**: L1 is **CLOSED** as a design decision. The `alloca`-
//! based IR is the intended design, not a limitation to be fixed.
//!
//! ## Architectural debt (tracked, not blocking)
//!
//! - **Emitter trait bloat**: 36 methods, 1 implementation (`TextEmitter`).
//!   Decompose into sub-traits (`EmitterArith`, `EmitterMemory`, etc.)
//!   when adding a second backend. Stage 3.59 Issue #5.

use crate::mir::body::*;
use lasso::Rodeo;

pub mod emitter;
pub use emitter::{
    emit_dyn_trait_ptr_type, emit_fat_ptr_type, emit_type_to_llvm_str, mir_type_to_emit_type,
    EmitType, EmitValue, Emitter,
};

pub mod text;
pub use text::TextEmitter;

// Stage 13.5 MUV-2: LLVMSysEmitter — LLVM C-API emitter via llvm-sys.
// Only available behind the `llvm-backend` feature.
#[cfg(feature = "llvm-backend")]
pub mod llvm;
#[cfg(feature = "llvm-backend")]
pub use llvm::LLVMSysEmitter;

// Stage 13.28: Codegen sub-modules (extracted from mod.rs for better
// organization and maintainability).
mod operand;
mod rvalue;
mod statement;
mod terminator;
mod trait_dispatch;

// Re-export functions from sub-modules for use within codegen.
// Stage 15.65: `codegen_dyn_trait_call` (legacy) removed; use `_direct` variant.
pub use operand::codegen_dyn_trait_call_direct;
pub(crate) use operand::codegen_operand;
pub(crate) use rvalue::codegen_rvalue;
pub(crate) use statement::codegen_statement;
pub(crate) use terminator::codegen_terminator;

// mir_translation helpers — pub so lib.rs can re-export; pub(crate) for
// sub-module access via super::*
pub(crate) mod mir_translation;
pub use mir_translation::{mir_type_to_emit_type_with_layouts, stdlib_type_kind_to_emit_type};

// Stage 13.1 (TD-028): dyn Trait LLVM IR text emission relocated from
// `mir::dyn_trait` per §16 interface isolation fix. These 7 emit_*
// functions are pure "MIR data → LLVM IR text" converters and belong
// in codegen, not MIR.
pub mod dyn_trait_emit;
pub use dyn_trait_emit::{
    emit_dyn_trait_fat_ptr_text, emit_dyn_trait_fat_ptrs_text_batch,
    emit_dyn_trait_fat_ptrs_text_batch_from_resolver, emit_dyn_trait_method_call_text,
    emit_dyn_trait_method_calls_text_batch, emit_dyn_trait_method_calls_text_batch_from_resolver,
    emit_dyn_trait_mir_plan_text,
};

/// Stage 3.56 (Phase A §16 refactoring): Generate LLVM IR from a
/// `CompileResult` — codegen is now a **pure MIR consumer**.
///
/// Was (Stage 3.1-3.55): codegen re-lowered HIR to MIR + re-ran typeck
/// inside codegen, violating section 16. Also silently skipped borrowck
/// and dropped type errors.
///
/// Now: codegen reads pre-built MIR + pre-computed metadata. Zero
/// calls to upstream stage functions.
pub fn codegen_crate(result: &crate::driver::CompileResult) -> String {
    let mut emitter = TextEmitter::new();
    emitter.emit_header();
    emitter.emit_declare("void @__landin_panic_overflow(i32 %op, i32 %lhs, i32 %rhs)");
    emitter.emit_declare("void @__landin_panic_bounds_check(i64 %index, i64 %len)");
    emitter.emit_declare("void @__landin_panic_div_by_zero()");
    // Stage 14.69: __landin_str_eq is NOT pre-declared here because emit_declare
    // treats all args as i32 (heuristic). The function needs (ptr, i64, ptr, i64)
    // args. emit_call will create the declaration with correct types on first use.
    codegen_from_mir(
        &result.mirs,
        &result.body_metas,
        &result.fn_name_by_def_id,
        &result.fn_sigs,
        &result.interner,
        &mut emitter,
    );

    // Stage 5.6: emit vtable globals for all (trait, type) pairs collected
    // by TraitResolver. Each vtable is a constant array of opaque function
    // pointers, one per trait method, pointing at the concrete impl method
    // symbol (`landin_<Type>_<method>`). This is the L5 trait dispatch
    // foundation — future stages will use these globals when constructing
    // `dyn Trait` fat pointers.
    //
    // Per §16: codegen reads the pre-built TraitResolver (data only, no
    // HIR access). The fn_name strings are already resolved at collect
    // time, so no further upstream lookup is needed here.
    emit_vtables(&result.trait_resolver, &result.interner, &mut emitter);
    // Stage 5.7: emit `dyn Trait` fat-pointer constant globals for every
    // (trait, type) pair. Each fat pointer is `{ ptr, ptr }` (data + vtable),
    // referencing the vtable globals emitted above. This is the foundation
    // for `dyn Trait` value construction — future stages will use these
    // globals when lowering `dyn Trait` locals.
    emit_dyn_trait_ptrs(&result.trait_resolver, &result.interner, &mut emitter);

    // Stage 15.57 (HP-12 step 7): Emit drop glue functions for types that
    // need drop. Stage 15.63: Extended to emit drop glue for ALL types
    // needing drop (not just types with `impl Drop`). For types with
    // `impl Drop`, calls the user's `Drop::drop` method then recursively
    // drops each field. For types without `impl Drop` but with fields
    // needing drop, recursively drops each field.
    //
    // Per §16: codegen reads the pre-built TraitResolver + AdtLayouts (data only).
    // Per §23: function name follows `drop_<noun>_<id>` pattern.
    let adt_layouts = result
        .mirs
        .first()
        .map(|m| m.adt_layouts.clone())
        .unwrap_or_default();
    emit_drop_glue_functions(
        &result.trait_resolver,
        &result.interner,
        &result.fn_name_by_def_id,
        &adt_layouts,
        &mut emitter,
    );

    emitter.output_with_globals()
}

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
fn emit_drop_glue_functions(
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

    // Get the "Drop" trait name from the interner (if any types implement Drop).
    let drop_name = interner.get("Drop");

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

        // Check if this type has `impl Drop`.
        let has_drop_impl = drop_name
            .map(|dn| resolver.implements(dn, type_spur))
            .unwrap_or(false);

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

/// Stage 13.5 MUV-2: Generate LLVM IR via the LLVM C API (`llvm-sys`).
///
/// Mirrors `codegen_crate` but uses `LLVMSysEmitter` instead of
/// `TextEmitter`. The returned `LLVMModuleRef` is owned by the
/// `LLVMSysEmitter` instance returned alongside it (so callers can
/// drop them together). Use `LLVMSysEmitter::to_module()` to access
/// the module and `LLVMSysEmitter::to_object_file()` to emit an
/// object file.
///
/// Per §16: same MIR-only consumer contract as `codegen_crate` —
/// zero upstream calls to `crate::mir::lower` / `crate::typeck`.
#[cfg(feature = "llvm-backend")]
pub fn codegen_crate_to_module(result: &crate::driver::CompileResult) -> LLVMSysEmitter {
    let mut emitter = LLVMSysEmitter::new();
    emitter.emit_header();
    emitter.emit_declare("void @__landin_panic_overflow(i32 %op, i32 %lhs, i32 %rhs)");
    emitter.emit_declare("void @__landin_panic_bounds_check(i64 %index, i64 %len)");
    emitter.emit_declare("void @__landin_panic_div_by_zero()");
    // Stage 14.69: __landin_str_eq is NOT pre-declared (emit_declare treats
    // all args as i32; this function needs ptr, i64 args). emit_call creates
    // the declaration with correct types on first use.
    // Stage 14.91 (Bug X3 fix): Populate fn_sigs BEFORE vtable emission.
    // The vtable globals reference function symbols (e.g., `ptr @landin_Square_area`),
    // which causes LLVM to auto-create declarations with 0 args (wrong). By
    // setting fn_sigs first, get_or_declare_function (if called during vtable
    // emission) will create the correct declaration. And even if LLVM auto-creates
    // a declaration, emit_function_begin will find the fn_sigs entry and create
    // a matching declaration that gets reused.
    //
    // Previously, set_fn_sigs was called AFTER vtable emission (line 222),
    // so the vtable's auto-created declaration had 0 args, causing a signature
    // mismatch in emit_function_begin → duplicate function (.1 suffix) →
    // "undefined reference" link error.
    let fn_sigs_map = build_fn_sigs_map(&result.fn_name_by_def_id, &result.fn_sigs);
    emitter.set_fn_sigs(fn_sigs_map);
    // Stage 14.13 (GAP-30): Emit vtable + dynptr globals BEFORE function
    // bodies, so that `emit_dyn_trait_method_call` can look up the dynptr
    // global by name via LLVMGetNamedGlobal when generating indirect calls.
    // Previously this came AFTER codegen_from_mir, causing the dynptr
    // global to not exist yet when functions referenced it.
    emit_vtables(&result.trait_resolver, &result.interner, &mut emitter);
    emit_dyn_trait_ptrs(&result.trait_resolver, &result.interner, &mut emitter);
    // Stage 15.61: Emit drop glue functions in the LLVM backend too.
    // Previously this was only called in `codegen_crate` (text backend),
    // causing "undefined reference to `drop_adt_<N>`" link errors when
    // using `--emit-obj` / `--emit-bin` / `--run` with `impl Drop` programs.
    // The LLVM backend must emit the same drop glue functions as the text
    // backend so that `TerminatorKind::Drop` codegen can resolve them.
    // Stage 15.63: Now passes AdtLayouts for recursive drop support.
    //
    // Per §16: codegen reads the pre-built TraitResolver + AdtLayouts (data only).
    // Per §23: function name follows `drop_<noun>_<id>` pattern.
    let adt_layouts = result
        .mirs
        .first()
        .map(|m| m.adt_layouts.clone())
        .unwrap_or_default();
    emit_drop_glue_functions(
        &result.trait_resolver,
        &result.interner,
        &result.fn_name_by_def_id,
        &adt_layouts,
        &mut emitter,
    );
    codegen_from_mir(
        &result.mirs,
        &result.body_metas,
        &result.fn_name_by_def_id,
        &result.fn_sigs,
        &result.interner,
        &mut emitter,
    );
    emitter
}

/// Stage 14.65: Build a map from function name → (return type, param types)
/// for the LLVMSysEmitter's forward-reference resolution.
///
/// Uses empty ADT layouts (forward declarations don't need precise ADT
/// types — they only need the right primitive signature so that
/// `emit_function_begin` can reuse the declaration).
#[cfg(feature = "llvm-backend")]
fn build_fn_sigs_map(
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
    fn_sigs: &std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
) -> std::collections::HashMap<String, (EmitType, Vec<EmitType>)> {
    use crate::codegen::emitter::mir_type_to_emit_type;
    use crate::codegen::emitter::EmitType;

    // Stage 14.91 (Bug X3 fix): Use the legacy mir_type_to_emit_type (without
    // layouts) for fn_sig_map. This correctly maps Ref types to ptr (OpaquePtr),
    // while the _with_layouts variant would fall back to I32 for Adt types
    // when layouts are empty (which they are here — build_fn_sigs_map runs
    // before MIR bodies are available).
    //
    // For Ref types (which is what &self/&mut self params are), both variants
    // produce the same result (ptr). For Adt types, the legacy variant produces
    // I32 (wrong for structs) — but this is only used for forward declarations,
    // and the actual function definition uses the correct type from the MIR
    // body's local_decls. The forward declaration is just a placeholder that
    // gets reused if the signature matches.
    let mut map = std::collections::HashMap::new();
    for (def_id, name) in fn_name_by_def_id {
        if let Some(sig) = fn_sigs.get(def_id) {
            let ret_ty = mir_type_to_emit_type(&sig.output);
            let param_tys: Vec<EmitType> = sig.inputs.iter().map(mir_type_to_emit_type).collect();
            map.insert(name.clone(), (ret_ty, param_tys));
        }
    }
    map
}

// Stage 6.7: emit_vtables and emit_dyn_trait_ptrs moved to trait_dispatch module.
// Re-exported here for backward compatibility.
pub use trait_dispatch::{
    build_dynptr_global_specs, build_trait_dispatch_emission_plan,
    build_trait_dispatch_emission_summary, build_vtable_global_specs, emit_dyn_trait_ptrs,
    emit_dynptr_global_text, emit_dynptrs_from_resolver, emit_trait_dispatch_globals_from_plan,
    emit_trait_dispatch_globals_text_batch, emit_trait_dispatch_globals_text_batch_from_resolver,
    emit_vtable_global_from_emission, emit_vtable_global_text, emit_vtable_globals_batch,
    emit_vtables, emit_vtables_and_dynptrs_from_resolver, emit_vtables_from_resolver,
    CodegenTraitDispatchEmissionPlan, CodegenTraitDispatchEmissionSummary, StdlibDynptrGlobalSpec,
    StdlibVtableGlobalSpec,
};

/// (no HIR, no re-lowering, no re-typeck).
pub fn codegen_from_mir(
    mirs: &[MirBody],
    body_metas: &[crate::driver::BodyMeta],
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
    fn_sigs: &std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
    interner: &Rodeo,
    emitter: &mut dyn Emitter,
) {
    for (mir, meta) in mirs.iter().zip(body_metas.iter()) {
        codegen_function(
            emitter,
            &meta.fn_name,
            mir,
            fn_name_by_def_id,
            fn_sigs,
            meta.param_count,
            interner,
            // Stage 15.8: Arc<AdtLayouts> auto-derefs to &AdtLayouts.
            // Per clippy::explicit_auto_deref, use &mir.adt_layouts (auto-deref).
            &mir.adt_layouts,
            meta.is_void,
            meta.abi,
        );
    }
}

/// Generate LLVM IR for a single function.
#[allow(clippy::too_many_arguments)]
fn codegen_function(
    emitter: &mut dyn Emitter,
    name: &str,
    mir: &MirBody,
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
    fn_sigs: &std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
    param_count: usize,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
    is_void: bool,
    abi: crate::ast::Abi,
) {
    // The entry point `fn main()` is codegen'd as `landin_main` and is called
    // by the C wrapper which declares `extern int landin_main(void)`.
    // Per Rust convention: `fn main()` without explicit return type returns `()`.
    // The C wrapper reads the return value, so for `()` return we emit `ret i32 0`.
    // For `fn main() -> i32 { N }` we emit `ret i32 N`.
    //
    // The `is_entry` flag is set by the driver for the `fn main()` function.
    // This replaces the old `name == "landin_main"` string comparison (Stage 13.26).
    let is_entry = name == "landin_main";

    let ret_ty = if is_void {
        if is_entry {
            // Entry point with `()` return → emit i32 (C wrapper reads it as 0)
            EmitType::I32
        } else {
            EmitType::Void
        }
    } else if mir.local_decls.is_empty() {
        if is_entry {
            EmitType::I32
        } else {
            EmitType::Void
        }
    } else {
        match &mir.local_decls[0].ty.kind {
            crate::mir::ty::TyKind::Tuple(tys) if tys.is_empty() => {
                if is_entry {
                    EmitType::I32
                } else {
                    EmitType::Void
                }
            }
            _ => mir_type_to_emit_type_with_layouts(&mir.local_decls[0].ty, layouts),
        }
    };

    let params: Vec<(EmitType, String)> = (0..param_count)
        .map(|i| {
            let local_idx = i + 1;
            let ty = mir
                .local_decls
                .get(local_idx)
                .map(|ld| mir_type_to_emit_type_with_layouts(&ld.ty, layouts))
                .unwrap_or(EmitType::I32);
            (ty, format!("%arg{}", i))
        })
        .collect();

    let param_refs: Vec<(EmitType, &str)> = params
        .iter()
        .map(|(t, n)| (t.clone(), n.as_str()))
        .collect();

    // Stage 8.3: Add ABI attributes after the function definition.
    // For C ABI: no special attribute needed (C is the default in LLVM).
    // For Landin ABI: add `cc 64` (custom calling convention placeholder).
    // In MVP, both ABIs use the same LLVM calling convention (C), so no
    // attribute is emitted. Future: Landin ABI could use a custom CC.
    let _ = abi; // ABI is tracked but not yet differentiated in codegen
    emitter.emit_function_begin(name, &param_refs, &ret_ty);

    for (i, ld) in mir.local_decls.iter().enumerate() {
        let ty = mir_type_to_emit_type_with_layouts(&ld.ty, layouts);
        if ty == EmitType::Void {
            continue;
        }
        // Stage 14.36: If this local is the destination of a Call terminator,
        // override its type with the callee's return type from fn_sigs. This
        // fixes struct-returning method calls where the local's type is
        // Infer→i32 after typeck writeback but the actual value is a struct.
        let ty = if let Some(override_ty) = get_call_dest_type(mir, i, fn_sigs, layouts) {
            override_ty
        } else {
            ty
        };
        let ptr_name = format!("%loc_{}", i);
        let ptr = emitter.emit_alloca(&ty, &ptr_name);
        emitter.set_local_ptr(i as u32, ptr);
    }

    for (i, (ty, arg_name)) in params.iter().enumerate() {
        let local_idx = (i + 1) as u32;
        if let Some(ptr) = emitter.get_local_ptr(local_idx).cloned() {
            emitter.emit_store(ty, arg_name, &ptr);
        }
    }

    for (bb_idx, bb) in mir.basic_blocks.iter().enumerate() {
        let label = format!("bb{}", bb_idx);
        emitter.emit_block(&label);
        for stmt in &bb.statements {
            codegen_statement(emitter, mir, stmt, interner, layouts, fn_name_by_def_id);
        }
        codegen_terminator(
            emitter,
            mir,
            &bb.terminator,
            &ret_ty,
            fn_name_by_def_id,
            fn_sigs,
            interner,
            layouts,
        );
    }

    // Stage 13.12 + Stage 13.13: println! output is now emitted INLINE
    // via StatementKind::Println in codegen_statement (see Println arm
    // below). The Stage 13.12 side-table approach (a Vec<String> field
    // on MirBody + a separate helper function emitted after
    // emit_function_end + a weak-symbol trick in the C wrapper) was
    // REMOVED in Stage 13.13 because it broke output ordering for loops
    // and conditionals — the helper ran BEFORE landin_main(), so all
    // prints appeared before the program body.
    //
    // Stage 15.6 (cleanup): The MirBody.println_messages field itself
    // was removed in Stage 14.x (no longer declared on MirBody). This
    // comment retained as historical context for the inline-emission
    // design decision.
    emitter.emit_function_end();
}

/// Stage 14.36: Check if a local is the destination of a Call terminator,
/// and if so, return the callee's return type from fn_sigs. This overrides
/// the local's declared type (which may be Infer→i32 after typeck writeback)
/// with the actual return type (e.g. struct { i32, i32 }), ensuring the
/// alloca has the correct size for struct-returning method calls.
fn get_call_dest_type(
    mir: &MirBody,
    local_idx: usize,
    fn_sigs: &std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
    layouts: &crate::mir::body::AdtLayouts,
) -> Option<EmitType> {
    for bb in &mir.basic_blocks {
        if let crate::mir::body::TerminatorKind::Call {
            func, destination, ..
        } = &bb.terminator.kind
        {
            if let crate::mir::place::PlaceKind::Local(id) = &destination.kind {
                if id.0 as usize == local_idx {
                    // This local is a Call destination — get callee's DefId
                    let callee_def_id = if let crate::mir::place::Operand::Constant(c) = func {
                        match &c.val {
                            crate::mir::ty::ConstVal::Uint(n) => Some(crate::hir::DefId(*n as u32)),
                            crate::mir::ty::ConstVal::Int(n) => Some(crate::hir::DefId(*n as u32)),
                            _ => None,
                        }
                    } else if let crate::mir::place::Operand::Copy(lv)
                    | crate::mir::place::Operand::Move(lv) = func
                    {
                        if let crate::mir::place::PlaceKind::Local(id) = &lv.kind {
                            mir.local_decls.get(id.0 as usize).and_then(|ld| {
                                if let crate::mir::ty::TyKind::FnDef(did, _) = &ld.ty.kind {
                                    Some(*did)
                                } else {
                                    None
                                }
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if let Some(did) = callee_def_id {
                        if let Some(sig) = fn_sigs.get(&did) {
                            return Some(mir_type_to_emit_type_with_layouts(&sig.output, layouts));
                        }
                    }
                }
            }
        }
    }
    None
}
