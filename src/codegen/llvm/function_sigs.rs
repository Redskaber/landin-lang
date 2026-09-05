//! Stage 16.76 MUV-2: LLVM-only function signature map builder.
//!
//! Per `docs/lang-design/07-codegen.md` §3 (Function signature mapping).
//! Builds a map from function name → (return type, param types) for the
//! LLVMSysEmitter's forward-reference resolution.
//!
//! Extraction from `codegen/mod.rs` per §13.4 J2 (single responsibility).
//! Per §11: LLVM-only — text backend does not need this (text IR allows
//! forward references without explicit declarations).

#![cfg(feature = "llvm-backend")]

use crate::codegen::emitter::EmitType;
use crate::codegen::mir_translation::types::mir_type_to_emit_type_with_layouts;
use crate::hir::DefId;
use crate::mir::body::AdtLayouts;
use crate::mir::ty::Sig;
use std::collections::HashMap;

/// Stage 14.65: Build a map from function name → (return type, param types)
/// for the LLVMSysEmitter's forward-reference resolution.
///
/// Stage 18.442 (v0.5+ Phase 5 Step 4): Migrated from `mir_type_to_emit_type`
/// to `mir_type_to_emit_type_with_layouts` so that Adt types (e.g., `Big`
/// struct return types) are correctly resolved via AdtLayouts instead of
/// falling back to `EmitType::I32`.
///
/// Per §1.0 原則 4 (报错 > 静默): Adt types should be correctly resolved,
/// not silently mapped to I32.
/// Per §1.0 原則 6 (通解 > 特解): one layouts variant handles all types.
/// Per §13.4: incremental migration — this unblocks Phase 5 Step 3 (panic).
pub(crate) fn build_fn_sigs_map(
    fn_name_by_def_id: &HashMap<DefId, String>,
    fn_sigs: &HashMap<DefId, Sig>,
    adt_layouts: &AdtLayouts,
    // Stage 113: mono_names — MonoItem → specialized name map, used to add
    // specialized function sigs to the fn_sigs map. Without this, forward
    // declarations for specialized functions (e.g., `process_i32`) use a
    // variadic fallback → type mismatch → SIGSEGV in LLVMTargetMachineEmitToFile.
    mono_names_opt: Option<&HashMap<crate::mir::MonoItem, String>>,
) -> HashMap<String, (EmitType, Vec<EmitType>)> {
    let mut map = HashMap::new();
    for (def_id, name) in fn_name_by_def_id {
        if let Some(sig) = fn_sigs.get(def_id) {
            let ret_ty = mir_type_to_emit_type_with_layouts(&sig.output, adt_layouts);
            // Stage 85 (v0.8 — TD-FN-UNIT-ARGS): Filter out ZST params
            // (EmitType::Void) from the signature map. Without this filter,
            // functions with `()` params (e.g., `Fn<()>` trait impl's
            // `fn call(&self, args: ())`) get a forward declaration with
            // `void` as a param type — LLVM rejects this with
            // "Function arguments must have first-class types! void %0".
            //
            // This mirrors the ZST elision already done in:
            // - codegen_function.rs (function definition side, Stage 18.335)
            // - codegen/terminator.rs (call site side, Stage 18.335)
            //
            // The signature map is used by `declare_function` (forward
            // declarations) and `emit_call` (call site type lookup). Both
            // need to agree with the function definition's signature, which
            // elides ZST params. Without this filter, the forward decl has
            // a different signature than the definition → LLVM module
            // verification fails.
            //
            // Per §1.0 原則 6 (通解 > 特解): same ZST elision pattern for
            // all three sites (definition, call site, forward decl).
            // Per §12 (最优 > 最小): root-cause fix at the sig map layer,
            // not a workaround in declare_function or emit_call.
            // Per §20 (iterative audit): same root cause as Stage 18.335;
            // this is the third site that needed the same fix.
            let param_tys: Vec<EmitType> = sig
                .inputs
                .iter()
                .map(|t| {
                    // Stage 16.27: For Closure-typed params, use OpaquePtr
                    // (not Struct). This ensures the forward declaration
                    // created by get_or_declare_function matches the
                    // function definition's param type (OpaquePtr).
                    if matches!(t.kind, crate::mir::ty::TyKind::Closure(_, _)) {
                        EmitType::OpaquePtr
                    } else {
                        mir_type_to_emit_type_with_layouts(t, adt_layouts)
                    }
                })
                .filter(|ty| *ty != EmitType::Void)
                .collect();
            map.insert(name.clone(), (ret_ty, param_tys));
        }
    }

    // Stage 113 (TD-LLVM-OBJ-EMIT-CRASH fix): Add specialized (monomorphized)
    // function sigs to the fn_sigs map.
    //
    // Without this, when codegen_operand returns a specialized name like
    // `@process_i32` (from writeback_fndef_substs secondary pass), the
    // LLVMSysEmitter's `interpret_adhoc` looks up `process_i32` in fn_sigs,
    // doesn't find it → falls back to variadic `i32 ()` forward declaration.
    // Later, codegen_mono_functions emits the actual `process_i32` with
    // correct sig `i32 (i32)` → type mismatch → old decl deleted + re-added
    // → dangling references → SIGSEGV in LLVMTargetMachineEmitToFile.
    //
    // Fix: for each MonoItem::Fn with non-empty substs, compute the
    // specialized name via mono_item_name and add it to the map with the
    // substituted signature (inputs/output substituted with substs).
    //
    // Per §1.0 原則 6 (通解 > 特解): one pass for all MonoItem::Fn instances.
    // Per §1.0 原則 9 (正确 > 妥协): correct forward declarations, not variadic fallback.
    // Per §12 (最优 > 最小): root-cause fix at the sig map layer, not in interpret_adhoc.
    // Per §17.6 (直到审查不出问题为止): found by Stage 113 RCA investigation.
    if let Some(mono_names) = mono_names_opt {
        for (mono_item, specialized_name) in mono_names.iter() {
            if let crate::mir::MonoItem::Fn { def_id, substs } = mono_item {
                if substs.is_empty() {
                    continue;
                }
                // Look up the base function's sig.
                if let Some(sig) = fn_sigs.get(def_id) {
                    // Substitute Param types with concrete substs.
                    let substituted_output = crate::mir::substitute(&sig.output, substs);
                    let ret_ty =
                        mir_type_to_emit_type_with_layouts(&substituted_output, adt_layouts);
                    let param_tys: Vec<EmitType> = sig
                        .inputs
                        .iter()
                        .map(|t| {
                            let substituted = crate::mir::substitute(t, substs);
                            if matches!(t.kind, crate::mir::ty::TyKind::Closure(_, _)) {
                                EmitType::OpaquePtr
                            } else {
                                mir_type_to_emit_type_with_layouts(&substituted, adt_layouts)
                            }
                        })
                        .filter(|ty| *ty != EmitType::Void)
                        .collect();
                    map.insert(specialized_name.clone(), (ret_ty, param_tys));
                }
            }
        }
    }
    // Stage 18.202 (TD-FORMAT-VARIADIC fix): Add runtime helper signatures
    let runtime_sigs: &[(&str, EmitType, &[EmitType])] = &[
        ("__landin_alloc", EmitType::OpaquePtr, &[EmitType::I64]),
        ("__landin_dealloc", EmitType::Void, &[EmitType::OpaquePtr]),
        (
            "__landin_memcpy",
            EmitType::Void,
            &[EmitType::OpaquePtr, EmitType::OpaquePtr, EmitType::I64],
        ),
        (
            "__landin_realloc",
            EmitType::OpaquePtr,
            &[EmitType::OpaquePtr, EmitType::I64, EmitType::I64],
        ),
        (
            "__landin_i64_to_str",
            EmitType::I64,
            &[EmitType::OpaquePtr, EmitType::I64, EmitType::I64],
        ),
    ];
    for (name, ret, params) in runtime_sigs {
        map.insert(name.to_string(), (ret.clone(), params.to_vec()));
    }
    map
}
