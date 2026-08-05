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
    map
}
