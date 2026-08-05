//! Stage 16.76 MUV-2: Per-function codegen orchestrator.
//!
//! Contains:
//! - `codegen_from_mir`: iterate over MirBody list and call codegen_function
//! - `codegen_synthesized_closure_functions`: emit synthesized closure call fns
//! - `codegen_function`: emit a single LLVM function from a MirBody
//! - `get_call_dest_type`: helper to override local type for Call destinations
//!
//! Extraction from `codegen/mod.rs` per §13.4 J2 (single responsibility).
//! Per `docs/lang-design/07-codegen.md` §4 (MIR → LLVM IR mapping).

use crate::codegen::emitter::{EmitType, Emitter};
use crate::codegen::mir_translation::types::mir_type_to_emit_type_with_layouts_and_mono;
use crate::codegen::statement::codegen_statement;
use crate::codegen::terminator::codegen_terminator;
use crate::mir::body::MirBody;
use lasso::Rodeo;

/// (no HIR, no re-lowering, no re-typeck).
pub fn codegen_from_mir(
    mirs: &[MirBody],
    body_metas: &[crate::driver::BodyMeta],
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
    fn_sigs: &std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
    interner: &Rodeo,
    mono_layouts: &crate::mir::MonoLayoutMap,
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
            Some(mono_layouts),
            meta.is_void,
            meta.abi,
        );
    }
}

/// Stage 16.16 (Task 10 Steps 3+4): Emit LLVM functions for synthesized
/// closure `call` functions.
///
/// Each MirBody in `synthesized_closure_mir_bodies` represents a closure's
/// synthesized `call` function. The function name is resolved from
/// `fn_name_by_def_id` by matching the MirBody's DefId (stored in the
/// closure struct's type).
///
/// Since the synthesized MIR bodies don't have BodyMeta entries, we
/// synthesize the metadata here: param_count = captures + params + 1 (self),
/// is_void = false (closures return a value), abi = Landin.
///
/// Per §16: codegen reads MirBody + fn_name_by_def_id (data only, no HIR).
/// Per §23: `codegen_synthesized_closure_functions` follows
/// `<verb>_<adj>_<noun>_<noun>` pattern.
///
/// Stage 16.35: Removed incorrect `#[cfg(feature = "llvm-backend")]` gate.
/// This function is fully backend-agnostic (operates on `&mut dyn Emitter`),
/// so it must be available for the text-only build too. The gate was a bug
/// that broke `cargo check` without `--features llvm-backend`.
pub(crate) fn codegen_synthesized_closure_functions(
    synthesized_mirs: &[MirBody],
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
    fn_sigs: &std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
    interner: &Rodeo,
    mono_layouts: &crate::mir::MonoLayoutMap,
    emitter: &mut dyn Emitter,
) {
    for mir in synthesized_mirs {
        // Stage 16.17: Use the DefId stored on MirBody (set during
        // build_synthesized_closure_mir_body) to resolve the function name.
        // This replaces the fragile string-pattern search from Stage 16.16.
        let def_id = match mir.def_id {
            Some(id) => id,
            None => continue, // Skip MIR bodies without DefId (shouldn't happen)
        };

        let fn_name = match fn_name_by_def_id.get(&def_id) {
            Some(name) => name.clone(),
            None => continue, // Skip if name not registered (shouldn't happen)
        };

        // Stage 16.29 (通解 — fix hardcoded param_count):
        // The previous code hardcoded `param_count = 2` (self + 1 param)
        // — a 特解 (special-case) that breaks for closures with 0 params
        // (e.g., `|| 42`) or 2+ params (e.g., `|x, y| x + y`).
        //
        // The 通解 (general solution) is to read the actual param_count
        // from `fn_sigs[def_id].inputs.len()`. The driver now populates
        // `fn_sig_table` with the resolved sig (after closure typeck),
        // so `inputs.len()` = 1 (self) + N (closure params) = correct
        // param_count for codegen.
        //
        // Per §1.0 原則 6 "通用 > 特例": one source of truth (fn_sigs)
        // for the param_count, not a hardcoded constant.
        // Per §16: codegen reads MirBody + fn_sigs (data only, no HIR).
        let param_count = fn_sigs
            .get(&def_id)
            .map(|sig| sig.inputs.len())
            .unwrap_or(1); // Defensive: self only (shouldn't happen)

        let meta = crate::driver::BodyMeta {
            fn_name: fn_name.clone(),
            is_void: false, // Closures return a value
            param_count,
            abi: crate::ast::Abi::Landin,
        };
        codegen_function(
            emitter,
            &meta.fn_name,
            mir,
            fn_name_by_def_id,
            fn_sigs,
            meta.param_count,
            interner,
            &mir.adt_layouts,
            Some(mono_layouts),
            meta.is_void,
            meta.abi,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn codegen_function(
    emitter: &mut dyn Emitter,
    name: &str,
    mir: &MirBody,
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
    fn_sigs: &std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
    param_count: usize,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
    mono_layouts: Option<&crate::mir::MonoLayoutMap>,
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
            _ => mir_type_to_emit_type_with_layouts_and_mono(
                &mir.local_decls[0].ty,
                layouts,
                mono_layouts,
            ),
        }
    };

    let params: Vec<(EmitType, String)> = (0..param_count)
        .map(|i| {
            let local_idx = i + 1;
            let ty = mir
                .local_decls
                .get(local_idx)
                .map(|ld| {
                    // Stage 16.21: For synthesized closure functions,
                    // the `self` parameter (local_idx=1) has a Closure
                    // type. Codegen should pass it as OpaquePtr (pointer)
                    // instead of by-value struct, because:
                    // 1. The call site passes the closure struct by address
                    // 2. The function body accesses captures via GEP from
                    //    the pointer
                    // This matches how regular struct params are handled
                    // (Adt types are passed as OpaquePtr in codegen).
                    if local_idx == 1 && matches!(ld.ty.kind, crate::mir::ty::TyKind::Closure(_, _))
                    {
                        EmitType::OpaquePtr
                    } else {
                        mir_type_to_emit_type_with_layouts_and_mono(&ld.ty, layouts, mono_layouts)
                    }
                })
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
        // Stage 16.21: For the `self` parameter (local_idx=1) in synthesized
        // closure functions, use OpaquePtr for the alloca type. This matches
        // how the parameter is passed (as ptr). Other Closure-typed locals
        // (e.g., the closure struct in the caller) keep their original type.
        let is_self_param = i == 1
            && matches!(ld.ty.kind, crate::mir::ty::TyKind::Closure(_, _))
            && mir.def_id.is_some();
        let ty = if is_self_param {
            EmitType::OpaquePtr
        } else {
            mir_type_to_emit_type_with_layouts_and_mono(&ld.ty, layouts, mono_layouts)
        };
        if ty == EmitType::Void {
            continue;
        }
        // Stage 14.36: If this local is the destination of a Call terminator,
        // override its type with the callee's return type from fn_sigs. This
        // fixes struct-returning method calls where the local's type is
        // Infer→i32 after typeck writeback but the actual value is a struct.
        let ty =
            if let Some(override_ty) = get_call_dest_type(mir, i, fn_sigs, layouts, mono_layouts) {
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
            codegen_statement(
                emitter,
                mir,
                stmt,
                interner,
                layouts,
                mono_layouts,
                fn_name_by_def_id,
            );
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
            mono_layouts,
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
pub(crate) fn get_call_dest_type(
    mir: &MirBody,
    local_idx: usize,
    fn_sigs: &std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
    layouts: &crate::mir::body::AdtLayouts,
    mono_layouts: Option<&crate::mir::MonoLayoutMap>,
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
                            return Some(mir_type_to_emit_type_with_layouts_and_mono(
                                &sig.output,
                                layouts,
                                mono_layouts,
                            ));
                        }
                    }
                }
            }
        }
    }
    None
}
