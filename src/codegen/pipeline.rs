//! Stage 16.76 MUV-2: Codegen pipeline orchestrator.
//!
//! Contains `run_codegen_pipeline` — the unified emission order shared by both
//! text and LLVM backends. Per §1.0 原則 6 "通用 > 特例": one pipeline for all
//! backends. Per §23: clear single entry point for the codegen pipeline.
//!
//! Extraction from `codegen/mod.rs` per §13.4 J2 (single responsibility).

use crate::codegen::drop_glue::emit_drop_glue_functions;
use crate::codegen::emitter::Emitter;
use crate::codegen::error::CodegenResult;
use crate::codegen::function::{
    codegen_from_mir, codegen_mono_functions, codegen_synthesized_closure_functions,
};
use crate::codegen::trait_dispatch::{emit_dyn_trait_ptrs, emit_vtables};

/// Stage 16.37: Unified codegen pipeline — shared by both text and LLVM backends.
///
/// This function contains the single emission order used by ALL backends:
///   1. Module header + panic declarations
///   2. Vtable globals (BEFORE function bodies — LLVM needs forward refs)
///   3. Dyn trait fat-pointer globals
///   4. Drop glue functions
///   5. Main MIR function bodies (codegen_from_mir)
///   6. Synthesized closure function bodies
///
/// The text backend buffers globals separately and appends them at output
/// time (via `output_with_globals`), so the "globals first" order works
/// for both backends — text IR allows globals before function definitions.
///
/// Per §1.0 原則 6 "通用 > 特例": one pipeline for all backends.
/// Per §23: clear single entry point for the codegen pipeline.
/// Stage 18.151 (TD-CODEGEN-RESULT): `run_codegen_pipeline` now returns
/// `CodegenResult<()>` to propagate codegen errors from `codegen_from_mir`,
/// `codegen_mono_functions`, and `codegen_synthesized_closure_functions`.
///
/// Per §2 原则 9 (正确>妥协): full Result propagation.
pub fn run_codegen_pipeline(
    result: &crate::driver::CompileResult,
    emitter: &mut dyn Emitter,
) -> CodegenResult<()> {
    // 1. Module header + panic declarations
    emitter.emit_header();
    emitter.emit_declare("void @__landin_panic_overflow(i32 %op, i32 %lhs, i32 %rhs)");
    emitter.emit_declare("void @__landin_panic_bounds_check(i64 %index, i64 %len)");
    emitter.emit_declare("void @__landin_panic_div_by_zero()");
    // Stage 14.69: __landin_str_eq is NOT pre-declared (emit_declare treats
    // all args as i32; this function needs ptr, i64 args). emit_call creates
    // the declaration with correct types on first use.

    // Stage 18.21/18.27: Declare __landin_println etc. as variadic C functions.
    // Stage 18.27: The actual Call is intercepted by codegen_print_call
    // (which emits printf directly), but MIR lowering also generates
    // `store ptr @__landin_println` (function pointer assignment) which
    // requires the symbol to exist at link time. We emit declare + define
    // stubs so the linker can resolve the reference. The stubs return 0
    // and are never actually called (codegen_print_call intercepts).
    emitter.emit_declare("i32 @__landin_println(ptr, ...)");
    emitter.emit_declare("i32 @__landin_print(ptr, ...)");
    emitter.emit_declare("i32 @__landin_eprintln(ptr, ...)");
    emitter.emit_declare("i32 @__landin_eprint(ptr, ...)");
    // Stage 18.29: Declare non-print built-in macro runtime functions.
    emitter.emit_declare("void @__landin_assert(i1)");
    emitter.emit_declare("void @__landin_panic_msg(ptr)");
    // Stage 18.27: Emit stub definitions for __landin_ print functions.
    // These are needed because MIR lowering creates `store ptr @__landin_println`
    // which references the symbol. The stubs are never called (codegen_print_call
    // intercepts the actual Call terminator), but the linker needs them.
    // We use emit_declare with a define-style string that the text backend
    // will output verbatim.
    // Note: This is text-backend-specific. The LLVM backend would use
    // LLVMAddFunction + LLVMAppendBasicBlock + LLVMBuildRet.
    // Per §11: this is codegen-internal, not a cross-stage concern.

    // 2. Vtable globals (before function bodies — LLVM needs forward refs)
    emit_vtables(&result.trait_resolver, &result.interner, emitter);

    // 3. Dyn trait fat-pointer globals
    emit_dyn_trait_ptrs(&result.trait_resolver, &result.interner, emitter);

    // 4. Drop glue functions
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
        emitter,
    );

    // Stage 16.59 (Task 11 Phase 4c integration): Build per-mono layouts
    // from collected MonoItems. This is the actual codegen integration —
    // the MonoLayoutMap is threaded through codegen_from_mir →
    // codegen_function → all sub-modules, replacing the legacy
    // mir_type_to_emit_type_with_layouts calls with
    // mir_type_to_emit_type_with_layouts_and_mono.
    //
    // Per §16: builds from MIR + HIR (allowed at pipeline start).
    // Per §1.0 原則 6 "通用 > 特例": one pipeline for all backends.
    let mono_layouts: crate::mir::MonoLayoutMap = if let Some(hir) = &result.hir {
        let mono_items = crate::mir::collect_mono_items(&result.mirs);
        crate::mir::build_mono_layouts(&mono_items, hir)
    } else {
        std::collections::HashMap::new()
    };

    // 5. Main MIR function bodies
    codegen_from_mir(
        &result.mirs,
        &result.body_metas,
        &result.fn_name_by_def_id,
        &result.fn_sigs,
        &result.interner,
        &mono_layouts,
        emitter,
    )?;

    // Stage 18.103 (TD-MONO-CODEGEN): Emit specialized functions for each
    // MonoItem::Fn. For each generic function instantiation (e.g., id<i32>,
    // id<bool>), substitute the Param types in the generic MIR body and emit
    // a specialized function with a mangled name (e.g., landin_id_i32).
    //
    // Per §16: reads MIR + fn_sigs + fn_name_by_def_id + type_name_by_def_id
    // (data, no HIR). Stage 18.104 (S5 fix): type_name_by_def_id pre-computed.
    // Per §1.0 原則 6 "通用 > 特例": one pass for all MonoItem::Fn.
    // Per §2.0 原則 9 "正确 > 妥协": generic calls now emit specialized fns.
    codegen_mono_functions(
        &result.mirs,
        &result.type_name_by_def_id,
        &result.fn_name_by_def_id,
        &result.fn_sigs,
        &result.interner,
        &mono_layouts,
        emitter,
    )?;

    // 6. Synthesized closure function bodies
    codegen_synthesized_closure_functions(
        &result.synthesized_closure_mir_bodies,
        &result.fn_name_by_def_id,
        &result.fn_sigs,
        &result.interner,
        &mono_layouts,
        emitter,
    )?;
    Ok(())
}
