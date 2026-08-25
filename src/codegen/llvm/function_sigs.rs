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

use crate::codegen::emitter::{mir_type_to_emit_type, EmitType};
use crate::hir::DefId;
use crate::mir::ty::Sig;
use std::collections::HashMap;

/// Stage 14.65: Build a map from function name → (return type, param types)
/// for the LLVMSysEmitter's forward-reference resolution.
///
/// Uses empty ADT layouts (forward declarations don't need precise ADT
/// types — they only need the right primitive signature so that
/// `emit_function_begin` can reuse the declaration).
pub(crate) fn build_fn_sigs_map(
    fn_name_by_def_id: &HashMap<DefId, String>,
    fn_sigs: &HashMap<DefId, Sig>,
) -> HashMap<String, (EmitType, Vec<EmitType>)> {
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
    let mut map = HashMap::new();
    for (def_id, name) in fn_name_by_def_id {
        if let Some(sig) = fn_sigs.get(def_id) {
            let ret_ty = mir_type_to_emit_type(&sig.output);
            let param_tys: Vec<EmitType> = sig
                .inputs
                .iter()
                .map(|t| {
                    // Stage 16.27: For Closure-typed params, use OpaquePtr
                    // (not Struct). This ensures the forward declaration
                    // created by get_or_declare_function matches the
                    // function definition's param type (OpaquePtr).
                    // Without this, the forward declaration uses Struct
                    // type from mir_type_to_emit_type, but the function
                    // definition uses OpaquePtr from the is_self_param
                    // check → type mismatch → segfault.
                    if matches!(t.kind, crate::mir::ty::TyKind::Closure(_, _)) {
                        EmitType::OpaquePtr
                    } else {
                        mir_type_to_emit_type(t)
                    }
                })
                .collect();
            map.insert(name.clone(), (ret_ty, param_tys));
        }
    }
    // Stage 18.202 (TD-FORMAT-VARIADIC fix): Add runtime helper signatures
    // so get_or_declare_function creates correct forward declarations.
    // Without this, the fallback creates i32 (...) which mismatches the
    // actual void return type and fixed param count, causing ABI issues.
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
        // Stage 18.231: __landin_i64_to_str primitive (§16.5).
        (
            "__landin_i64_to_str",
            EmitType::I64,
            &[EmitType::OpaquePtr, EmitType::I64, EmitType::I64],
        ),
        // Stage 18.232: The 4 compound C helpers (vec_push, string_push_str,
        // vec_get, format_variadic) have been migrated to MIR intrinsics
        // (Stages 18.228-18.231) and are NO LONGER called. Their sigs are
        // removed. Per §1.0 原則 5 (去除兼容思维): dead code removed.
    ];
    for (name, ret, params) in runtime_sigs {
        map.insert(name.to_string(), (ret.clone(), params.to_vec()));
    }
    map
}
