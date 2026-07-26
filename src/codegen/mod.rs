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

pub mod emitter;
pub mod text_emitter;

// Stage 13.5 MUV-2: LLVMSysEmitter — LLVM C-API emitter via llvm-sys.
// Only available behind the `llvm-backend` feature.
#[cfg(feature = "llvm-backend")]
pub mod llvm_sys_emitter;

pub use emitter::{
    emit_dyn_trait_ptr_type, emit_fat_ptr_type, emit_type_to_llvm_str, mir_type_to_emit_type,
    EmitType, EmitValue, Emitter,
};
#[cfg(feature = "llvm-backend")]
pub use llvm_sys_emitter::LLVMSysEmitter;
pub use text_emitter::TextEmitter;

use crate::mir::body::*;
use crate::mir::place::*;
use crate::mir::ty::ConstVal;
use lasso::Rodeo;

mod mir_translation;
mod trait_dispatch;

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
    codegen_from_mir(
        &result.mirs,
        &result.body_metas,
        &result.fn_name_by_def_id,
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
    emitter.output_with_globals()
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
    codegen_from_mir(
        &result.mirs,
        &result.body_metas,
        &result.fn_name_by_def_id,
        &result.interner,
        &mut emitter,
    );
    emit_vtables(&result.trait_resolver, &result.interner, &mut emitter);
    emit_dyn_trait_ptrs(&result.trait_resolver, &result.interner, &mut emitter);
    emitter
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
    interner: &Rodeo,
    emitter: &mut dyn Emitter,
) {
    for (mir, meta) in mirs.iter().zip(body_metas.iter()) {
        codegen_function(
            emitter,
            &meta.fn_name,
            mir,
            fn_name_by_def_id,
            meta.param_count,
            interner,
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
    param_count: usize,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
    is_void: bool,
    abi: crate::ast::Abi,
) {
    let ret_ty = if is_void || mir.local_decls.is_empty() {
        EmitType::Void
    } else {
        match &mir.local_decls[0].ty.kind {
            crate::mir::ty::TyKind::Tuple(tys) if tys.is_empty() => EmitType::Void,
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
            codegen_statement(emitter, mir, stmt, interner, layouts);
        }
        codegen_terminator(
            emitter,
            mir,
            &bb.terminator,
            &ret_ty,
            fn_name_by_def_id,
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
    // The MirBody println_messages field is retained (Vec::new()) for
    // backward compatibility with any external tooling that reads MIR
    // side-tables, but is no longer populated by MIR lower.
    emitter.emit_function_end();
}
fn codegen_statement(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    stmt: &Statement,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
) {
    match &stmt.kind {
        StatementKind::Assign(boxed) => {
            let (place, rvalue) = &**boxed;
            let val = codegen_rvalue(emitter, mir, rvalue, interner, layouts);
            match &place.kind {
                PlaceKind::Local(id) => {
                    let default_ty = crate::mir::ty::Ty::new(
                        crate::mir::ty::TyKind::Int(crate::ast::IntTy::I32),
                        crate::session::Span::DUMMY,
                    );
                    let local_ty = mir
                        .local_decls
                        .get(id.0 as usize)
                        .map(|ld| &ld.ty)
                        .unwrap_or(&default_ty);
                    let ty = mir_type_to_emit_type_with_layouts(local_ty, layouts);
                    emitter.set_local(id.0, val.clone());
                    if ty != EmitType::Void {
                        if let Some(ptr) = emitter.get_local_ptr(id.0).cloned() {
                            emitter.emit_store(&ty, &val, &ptr);
                        }
                    }
                }
                PlaceKind::Projection(base, elem) => {
                    let ty = detect_operand_type(mir, &Operand::Copy(place.clone()), layouts)
                        .unwrap_or(EmitType::I32);
                    match elem {
                        ProjectionElem::Deref => {
                            let ptr_val = codegen_place_load(emitter, mir, base, interner, layouts);
                            emitter.emit_store(&ty, &val, &ptr_val);
                        }
                        ProjectionElem::Field(field_id, _) => {
                            let base_ptr = if let PlaceKind::Local(id) = &base.kind {
                                emitter
                                    .get_local_ptr(id.0)
                                    .cloned()
                                    .unwrap_or_else(|| "0".to_string())
                            } else {
                                codegen_place_load(emitter, mir, base, interner, layouts)
                            };
                            let struct_ty = detect_place_storage_type(mir, base, layouts);
                            let field_ptr =
                                emitter.emit_gep_field(&base_ptr, &struct_ty, field_id.0);
                            emitter.emit_store(&ty, &val, &field_ptr);
                        }
                        ProjectionElem::Index(idx) => {
                            // Stage 3.54: for the store path, when the base
                            // is a projection (e.g., `s.data` where `data` is
                            // a field), we need the ADDRESS of the base, not
                            // its loaded value. Was: `codegen_place_load`
                            // returned the loaded fat pointer value, then
                            // `unwrap_fat_ptr_for_index` tried to GEP into
                            // the value (treating it as a pointer) — invalid.
                            //
                            // Fix: if the base is a Local, use its alloca
                            // pointer (address). If the base is a Projection,
                            // compute the address by GEP-ing to the field
                            // (without loading). This matches the load path's
                            // behavior where `base_ptr` is always an address.
                            let base_ptr =
                                compute_place_address(emitter, mir, base, interner, layouts);
                            let array_ty = detect_place_storage_type(mir, base, layouts);
                            let idx_val = if let Some(v) = emitter.get_local(idx.0).cloned() {
                                v
                            } else if let Some(ptr) = emitter.get_local_ptr(idx.0).cloned() {
                                emitter.emit_load(&EmitType::I32, &ptr)
                            } else {
                                "0".to_string()
                            };
                            // Stage 3.51: same fat pointer unwrap as the load path.
                            let (gep_base, pointee_opt) =
                                unwrap_fat_ptr_for_index(emitter, &base_ptr, &array_ty);
                            let elem_ptr = match pointee_opt {
                                Some(elem_ty) => {
                                    emitter.emit_gep_index_ptr(&gep_base, &elem_ty, &idx_val)
                                }
                                None => emitter.emit_gep_index(&gep_base, &array_ty, &idx_val),
                            };
                            emitter.emit_store(&ty, &val, &elem_ptr);
                        }
                        _ => {}
                    }
                }
                PlaceKind::Static(_) => {}
            }
        }
        StatementKind::StorageLive(id) => {
            let _ = id;
        }
        StatementKind::StorageDead(_) => {}
        StatementKind::Nop | StatementKind::Deinit(_) => {}
        // Stage 13.13: Inline println! / print! / eprintln! / eprint!
        // statement — emits `printf("%s", <msg_global>)` at this exact
        // position in the basic block (the §16-compliant source-of-truth
        // for execution order).
        //
        // For `eprintln!`/`eprint!` (stderr == true), we currently still
        // call `printf` (deferred to Stage 13.14 to switch to
        // `fprintf(stderr, ...)`).
        //
        // The `msg` field already includes the trailing "\n" if
        // `newline == true` (set at MIR-lowering time). The `newline`
        // and `stderr` flags are kept here for forward-compatibility
        // (e.g., Stage 13.14 will use `stderr` to switch to `fprintf`).
        StatementKind::Println {
            msg,
            newline,
            stderr,
        } => {
            let _ = newline; // already encoded in `msg` (trailing "\n")
            let _ = stderr; // Stage 13.14: switch to fprintf(stderr, ...) when true

            // Emit format string "%s\0" (null-terminated for printf).
            // The emitter deduplicates identical globals, so this is
            // emitted once per module.
            let fmt = emitter.emit_string_global(b"%s\0");

            // Emit message string (null-terminated for printf).
            let mut msg_bytes = msg.as_bytes().to_vec();
            msg_bytes.push(0); // null terminator
            let str_global = emitter.emit_string_global(&msg_bytes);

            // Call printf("%s", str_global) inline at this position.
            // printf returns i32 (number of chars printed); we discard it.
            emitter.emit_call(
                "printf",
                &[
                    (EmitType::OpaquePtr, &fmt),
                    (EmitType::OpaquePtr, &str_global),
                ],
                &EmitType::I32,
            );
        }
    }
}

fn codegen_rvalue(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    rv: &Rvalue,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
) -> EmitValue {
    match rv {
        Rvalue::Use(op) => codegen_operand(emitter, mir, op, interner, layouts),
        Rvalue::BinaryOp(op, a, b) => {
            let a_val = codegen_operand(emitter, mir, a, interner, layouts);
            let b_val = codegen_operand(emitter, mir, b, interner, layouts);
            let ty = detect_operand_type(mir, a, layouts)
                .or(detect_operand_type(mir, b, layouts))
                .unwrap_or(EmitType::I32);

            // Stage 3.49 (L13 closure): fat pointers (`{ ptr, len }`) cannot
            // be compared with a single `icmp` — LLVM icmp doesn't support
            // aggregate types. For `==`/`!=`, we compare both fields:
            //   eq = (a.ptr == b.ptr) & (a.len == b.len)
            //   ne = (a.ptr != b.ptr) | (a.len != b.len)
            // This is a bitwise comparison, not content comparison —
            // `"abc" == "abc"` returns true only if they're the same
            // global (deduped) or same allocation. Content comparison
            // (memcmp) is deferred to a future stage (requires a runtime
            // function).
            //
            // Stage 3.50: extract the actual pointee type from the fat
            // pointer's field 0 (was: hardcoded `i8*` in Stage 3.49, which
            // was technically valid for `&str` but wrong for `&[T]` where
            // T ≠ u8 — would produce `icmp eq i8*` for an `i32*` value,
            // which is a type mismatch in typed-pointer LLVM).
            let (is_fat_ptr, ptr_field_ty) = match &ty {
                EmitType::Struct(fields) if fields.len() == 2 => {
                    let is_fp = fields[0].is_ptr() && fields[1] == EmitType::I64;
                    (is_fp, fields[0].clone())
                }
                _ => (false, EmitType::I32),
            };

            match op {
                BinOp::Eq => {
                    let cmp = if is_fat_ptr {
                        // Extract ptr (field 0) and len (field 1) from both,
                        // compare each, AND the results.
                        let a_ptr = emitter.emit_extractvalue(&ty, &a_val, 0);
                        let a_len = emitter.emit_extractvalue(&ty, &a_val, 1);
                        let b_ptr = emitter.emit_extractvalue(&ty, &b_val, 0);
                        let b_len = emitter.emit_extractvalue(&ty, &b_val, 1);
                        let ptr_eq = emitter.emit_icmp("eq", &ptr_field_ty, &a_ptr, &b_ptr);
                        let len_eq = emitter.emit_icmp("eq", &EmitType::I64, &a_len, &b_len);
                        emitter.emit_and(&EmitType::I1, &ptr_eq, &len_eq)
                    } else if ty == EmitType::F64 || ty == EmitType::F32 {
                        emitter.emit_fcmp("oeq", &ty, &a_val, &b_val)
                    } else {
                        emitter.emit_icmp("eq", &ty, &a_val, &b_val)
                    };
                    emitter.emit_zext(&EmitType::I1, &EmitType::I32, &cmp)
                }
                BinOp::Ne => {
                    let cmp = if is_fat_ptr {
                        let a_ptr = emitter.emit_extractvalue(&ty, &a_val, 0);
                        let a_len = emitter.emit_extractvalue(&ty, &a_val, 1);
                        let b_ptr = emitter.emit_extractvalue(&ty, &b_val, 0);
                        let b_len = emitter.emit_extractvalue(&ty, &b_val, 1);
                        let ptr_ne = emitter.emit_icmp("ne", &ptr_field_ty, &a_ptr, &b_ptr);
                        let len_ne = emitter.emit_icmp("ne", &EmitType::I64, &a_len, &b_len);
                        emitter.emit_or(&EmitType::I1, &ptr_ne, &len_ne)
                    } else if ty == EmitType::F64 || ty == EmitType::F32 {
                        emitter.emit_fcmp("one", &ty, &a_val, &b_val)
                    } else {
                        emitter.emit_icmp("ne", &ty, &a_val, &b_val)
                    };
                    emitter.emit_zext(&EmitType::I1, &EmitType::I32, &cmp)
                }
                BinOp::Lt => {
                    let cmp = if ty == EmitType::F64 || ty == EmitType::F32 {
                        emitter.emit_fcmp("olt", &ty, &a_val, &b_val)
                    } else {
                        emitter.emit_icmp("slt", &ty, &a_val, &b_val)
                    };
                    emitter.emit_zext(&EmitType::I1, &EmitType::I32, &cmp)
                }
                BinOp::Le => {
                    let cmp = if ty == EmitType::F64 || ty == EmitType::F32 {
                        emitter.emit_fcmp("ole", &ty, &a_val, &b_val)
                    } else {
                        emitter.emit_icmp("sle", &ty, &a_val, &b_val)
                    };
                    emitter.emit_zext(&EmitType::I1, &EmitType::I32, &cmp)
                }
                BinOp::Gt => {
                    let cmp = if ty == EmitType::F64 || ty == EmitType::F32 {
                        emitter.emit_fcmp("ogt", &ty, &a_val, &b_val)
                    } else {
                        emitter.emit_icmp("sgt", &ty, &a_val, &b_val)
                    };
                    emitter.emit_zext(&EmitType::I1, &EmitType::I32, &cmp)
                }
                BinOp::Ge => {
                    let cmp = if ty == EmitType::F64 || ty == EmitType::F32 {
                        emitter.emit_fcmp("oge", &ty, &a_val, &b_val)
                    } else {
                        emitter.emit_icmp("sge", &ty, &a_val, &b_val)
                    };
                    emitter.emit_zext(&EmitType::I1, &EmitType::I32, &cmp)
                }
                BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor
                    if ty == EmitType::F64 || ty == EmitType::F32 =>
                {
                    let int_ty = if ty == EmitType::F64 {
                        EmitType::I64
                    } else {
                        EmitType::I32
                    };
                    let a_int = emitter.emit_cast(&ty, &int_ty, &a_val);
                    let b_int = emitter.emit_cast(&ty, &int_ty, &b_val);
                    let result_int = emitter.emit_binop(*op, &int_ty, &a_int, &b_int);
                    emitter.emit_cast(&int_ty, &ty, &result_int)
                }
                _ => emitter.emit_binop(*op, &ty, &a_val, &b_val),
            }
        }
        Rvalue::UnaryOp(op, operand) => {
            let val = codegen_operand(emitter, mir, operand, interner, layouts);
            let ty = detect_operand_type(mir, operand, layouts).unwrap_or(EmitType::I32);
            emitter.emit_unop(*op, &ty, &val)
        }
        Rvalue::Ref(_, _borrow_kind, lv) => {
            if let PlaceKind::Local(id) = &lv.kind {
                if let Some(ptr) = emitter.get_local_ptr(id.0).cloned() {
                    return ptr;
                }
            }
            "0".to_string()
        }
        Rvalue::Aggregate(AggregateKind::Tuple, operands) => {
            if operands.is_empty() {
                "0".to_string()
            } else if operands.len() == 1 {
                codegen_operand(emitter, mir, &operands[0], interner, layouts)
            } else {
                let field_tys: Vec<EmitType> = operands
                    .iter()
                    .map(|op| detect_operand_type(mir, op, layouts).unwrap_or(EmitType::I32))
                    .collect();
                let agg_ty = EmitType::Struct(field_tys.clone());
                let mut agg = "undef".to_string();
                for (i, op) in operands.iter().enumerate() {
                    let val = codegen_operand(emitter, mir, op, interner, layouts);
                    let val_ty = &field_tys[i];
                    agg = emitter.emit_insertvalue(&agg_ty, &agg, val_ty, &val, i as u32);
                }
                agg
            }
        }
        Rvalue::Aggregate(AggregateKind::Array(elem_ty), operands) => {
            if operands.is_empty() {
                return "0".to_string();
            }
            let elem_emit_ty = mir_type_to_emit_type(elem_ty);
            let n = operands.len() as u64;
            let agg_ty = EmitType::array_of(elem_emit_ty.clone(), n);
            let mut agg = "undef".to_string();
            for (i, op) in operands.iter().enumerate() {
                let val = codegen_operand(emitter, mir, op, interner, layouts);
                agg = emitter.emit_insertvalue(&agg_ty, &agg, &elem_emit_ty, &val, i as u32);
            }
            agg
        }
        Rvalue::Aggregate(AggregateKind::Adt(def_id, variant, _substs, field_tys), operands) => {
            if operands.is_empty() {
                return "0".to_string();
            }
            // Stage 3.48 (L-ENUM-UNION): for enum variants, compute the
            // correct starting field_idx in the flat storage layout.
            // The storage is `{ discr, variant_0_fields..., variant_1_fields..., ... }`
            // (flattened — unit variants contribute no fields). The starting
            // field_idx for variant V's payload = 1 + sum(field_counts of
            // variants 0..V-1) — but only counting this variant's own fields
            // starting from that offset. (See `mir_type_to_emit_type_with_layouts`
            // for the layout definition.)
            //
            // For struct (AdtLayout::Struct), variant_idx is always 0 and the
            // storage is just the struct's fields (no discriminant). The legacy
            // path (no AdtLayout) also treats it as a flat struct.
            use crate::mir::body::AdtLayout;

            let layout = layouts.get(def_id);
            let is_enum = matches!(layout, Some(AdtLayout::Enum { .. }));

            if is_enum {
                // Enum variant construction.
                // Look up the full storage type from the Adt layout.
                let storage_ty = mir_type_to_emit_type_with_layouts(
                    &crate::mir::ty::Ty::new(
                        crate::mir::ty::TyKind::Adt(*def_id, Vec::new()),
                        crate::session::Span::DUMMY,
                    ),
                    layouts,
                );
                // Compute the starting field_idx for this variant's payload.
                // = 1 (for discriminant) + sum(field_counts of variants 0..V-1)
                let variant_idx = *variant;
                let starting_field_idx = if let Some(AdtLayout::Enum {
                    variant_payloads, ..
                }) = layout
                {
                    let mut idx = 1u32; // skip discriminant
                    for (i, payload) in variant_payloads.iter().enumerate() {
                        if i as u32 >= variant_idx {
                            break;
                        }
                        idx += payload.len() as u32;
                    }
                    idx
                } else {
                    1 // fallback (shouldn't reach here for enum)
                };

                let mut agg = "undef".to_string();
                // Operand 0 is always the discriminant (prepended by MIR lower
                // for enum variants — see `lower_expr_to_operand`'s Call path).
                // Insert it at field 0 of the storage.
                let discr_op = &operands[0];
                let discr_val = codegen_operand(emitter, mir, discr_op, interner, layouts);
                let discr_ty = detect_operand_type(mir, discr_op, layouts).unwrap_or(EmitType::I32);
                agg = emitter.emit_insertvalue(&storage_ty, &agg, &discr_ty, &discr_val, 0);

                // Remaining operands are the variant's payload fields, inserted
                // starting at `starting_field_idx`.
                // `field_tys` from AggregateKind includes the discriminant as
                // element 0 (per `resolve_enum_variant`), so payload field i
                // is at `field_tys[i+1]`.
                for (i, op) in operands.iter().enumerate().skip(1) {
                    let val = codegen_operand(emitter, mir, op, interner, layouts);
                    // field_tys[i] is this operand's type (field_tys[0]=discr,
                    // field_tys[1]=payload_field_0, ...).
                    let val_ty = field_tys
                        .get(i)
                        .map(mir_type_to_emit_type)
                        .unwrap_or_else(|| {
                            detect_operand_type(mir, op, layouts).unwrap_or(EmitType::I32)
                        });
                    let target_idx = starting_field_idx + (i as u32 - 1);
                    agg = emitter.emit_insertvalue(&storage_ty, &agg, &val_ty, &val, target_idx);
                }
                agg
            } else {
                // Struct construction (or test-context fallback without layout).
                // Legacy path: flat struct, operands at 0..N.
                let field_tys: Vec<EmitType> = if field_tys.is_empty() {
                    operands
                        .iter()
                        .map(|op| detect_operand_type(mir, op, layouts).unwrap_or(EmitType::I32))
                        .collect()
                } else {
                    field_tys.iter().map(mir_type_to_emit_type).collect()
                };
                let agg_ty = EmitType::Struct(field_tys.clone());
                let mut agg = "undef".to_string();
                for (i, op) in operands.iter().enumerate() {
                    let val = codegen_operand(emitter, mir, op, interner, layouts);
                    let val_ty = field_tys.get(i).cloned().unwrap_or(EmitType::I32);
                    agg = emitter.emit_insertvalue(&agg_ty, &agg, &val_ty, &val, i as u32);
                }
                agg
            }
        }
        // Stage 13.3a (TD-030): Closure struct construction.
        // Aggregate(AggregateKind::Closure(def_id, substs), operands) constructs
        // a closure struct value with one field per capture. The `substs`
        // vector carries the capture field types (matching `TyKind::Closure`).
        // The `operands` vector carries the capture values (in field order).
        //
        // Codegen: emit the closure struct type as `{ field_tys... }`, then
        // `insertvalue` each capture operand at its field index. Mirrors the
        // `AggregateKind::Adt` struct path (lines 603-622 above).
        //
        // Per `07-codegen.md` §8.1: "每个闭包字面量生成一个唯一的匿名 struct" —
        // the closure struct is an anonymous struct with one field per capture.
        // The struct type is computed by `mir_type_to_emit_type` from
        // `TyKind::Closure(_, substs)` (see `emitter.rs:487-490`).
        Rvalue::Aggregate(AggregateKind::Closure(_def_id, substs), operands) => {
            if operands.is_empty() {
                // Empty closure (no captures) — emit an empty struct value.
                return "0".to_string();
            }
            // Build the closure struct type from the capture field types.
            let field_tys: Vec<EmitType> = substs.iter().map(mir_type_to_emit_type).collect();
            let agg_ty = EmitType::Struct(field_tys.clone());
            let mut agg = "undef".to_string();
            for (i, op) in operands.iter().enumerate() {
                let val = codegen_operand(emitter, mir, op, interner, layouts);
                let val_ty = field_tys.get(i).cloned().unwrap_or(EmitType::I32);
                agg = emitter.emit_insertvalue(&agg_ty, &agg, &val_ty, &val, i as u32);
            }
            agg
        }
        Rvalue::Cast(_, op, target_ty) => {
            let val = codegen_operand(emitter, mir, op, interner, layouts);
            let src_ty = detect_operand_type(mir, op, layouts).unwrap_or(EmitType::I32);
            let dst_ty = mir_type_to_emit_type(target_ty);
            emitter.emit_cast(&src_ty, &dst_ty, &val)
        }
        _ => "0".to_string(),
    }
}

fn codegen_operand(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    op: &Operand,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
) -> EmitValue {
    match op {
        Operand::Constant(c) => match c.val {
            ConstVal::Str(sym) => {
                let bytes = interner
                    .try_resolve(&sym)
                    .map(|s| s.as_bytes())
                    .unwrap_or(b"\0");
                let global_name = emitter.emit_string_global(bytes);
                let n = bytes.len();
                // Stage 3.49 (L13 closure): emit a fat pointer
                // `{ i8*, i64 }` (or `{ T*, i64 }` for byte strings) instead
                // of just the thin `i8*` pointer. The fat pointer carries
                // the length, so callees can recover it.
                //
                // The constant's type (`c.ty`) tells us whether this is a
                // `&str` (Ref to Str) or a `&[u8]` (Slice(u8) — actually
                // MIR lower produces Slice(u8) directly for byte strings,
                // not a Ref to Slice, but codegen treats both as fat ptrs).
                //
                // For `&str`: fat_ptr = `{ i8*, i64 }`, ptr = GEP to global,
                //   len = byte count.
                // For `&[u8]` (Slice(u8) const): same layout, same emission.
                //
                // We compute the fat pointer's EmitType from the constant's
                // declared type, then `insertvalue` the ptr and len.
                let fat_ty = mir_type_to_emit_type_with_layouts(&c.ty, layouts);
                let ptr_val = format!(
                    "getelementptr inbounds ([{} x i8], [{} x i8]* @{}, i32 0, i32 0)",
                    n, n, global_name
                );
                // insertvalue { i8*, i64 } undef, i8* %ptr, 0
                let with_ptr = emitter.emit_insertvalue(
                    &fat_ty,
                    &"undef".to_string(),
                    &EmitType::ptr_to(EmitType::I8),
                    &ptr_val,
                    0,
                );
                // insertvalue { i8*, i64 } %with_ptr, i64 N, 1
                emitter.emit_insertvalue(&fat_ty, &with_ptr, &EmitType::I64, &n.to_string(), 1)
            }
            _ => emitter.emit_const(&c.val),
        },
        Operand::Copy(lv) | Operand::Move(lv) => {
            let ty = detect_place_type(mir, lv, layouts);
            codegen_place_load_typed(emitter, mir, lv, ty, interner, layouts)
        }
    }
}

#[allow(clippy::only_used_in_recursion)]
/// Stage 5.79: Codegen a dyn Trait method call.
///
/// Reads `mir.dyn_trait_calls[index]` to get the
/// `(trait, type, method, slot_index, param_count)` info, computes the
/// dynptr symbol (`.dynptr.<trait>.<type>`), and calls
/// `emitter.emit_dyn_trait_method_call()` with the slot_index + args.
///
/// # Arguments
///
/// - `emitter`: the codegen emitter (text or other backend)
/// - `mir`: the MIR body (carries `dyn_trait_calls` side-table)
/// - `index`: the side-table index (from the `Const{val: Int(index)}` marker)
/// - `args`: the call's argument operands (self first, then explicit args)
/// - `interner`: the string interner (for type detection)
/// - `layouts`: ADT layouts (for type detection)
///
/// # Returns
///
/// The `EmitValue` (LLVM register name) holding the call result.
///
/// # Panics
///
/// Panics if `index` is out of bounds for `mir.dyn_trait_calls`. The
/// caller (`codegen_terminator`'s `Terminator::Call` branch) is
/// responsible for bounds-checking before invoking this function.
///
/// # §16 compliance
///
/// Reads `mir::body::MirBody.dyn_trait_calls` (side-table populated by
/// Stage 5.78's `build_dyn_trait_call_terminator`). No HIR or
/// TraitResolver queries — MIR carries all dyn Trait info as data.
/// Data flow: `mir::body` → `codegen` → LLVM IR text. Single-directional.
///
/// # §23 compliance
///
/// `codegen_dyn_trait_call` follows the `<verb>_<noun>_<noun>_<noun>`
/// pattern (helper-verb `codegen_` prefix per §8.1, mirrors
/// `codegen_terminator` / `codegen_operand`).
pub fn codegen_dyn_trait_call(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    index: u128,
    args: &[Operand],
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
) -> EmitValue {
    let call_info = &mir.dyn_trait_calls[index as usize];
    let dynptr_symbol = format!(".dynptr.{}.{}", call_info.trait_name, call_info.type_name);

    // Codegen the args (self first, then explicit args — already ordered
    // by `build_dyn_trait_call_terminator`).
    //
    // Stage 5.84: use param_kinds for precise arg types. The args list is
    // [self, arg0, arg1, ...] — self is at index 0 (always OpaquePtr since
    // it's a fat pointer), explicit args start at index 1 and use
    // param_kinds[i-1] for their EmitType. Falls back to detect_operand_type
    // when param_kinds is exhausted or unavailable.
    let arg_pairs: Vec<(EmitType, EmitValue)> = args
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let ty = if i == 0 {
                // self — always a fat pointer (OpaquePtr)
                EmitType::OpaquePtr
            } else {
                // Explicit arg — use param_kinds[i-1] if available
                let param_idx = i - 1;
                if param_idx < call_info.param_kinds.len() {
                    stdlib_type_kind_to_emit_type(call_info.param_kinds[param_idx])
                } else {
                    // Fallback to operand type detection
                    detect_operand_type(mir, a, layouts).unwrap_or(EmitType::I32)
                }
            };
            let val = codegen_operand(emitter, mir, a, interner, layouts);
            (ty, val)
        })
        .collect();
    let arg_refs: Vec<(EmitType, &EmitValue)> =
        arg_pairs.iter().map(|(t, v)| (t.clone(), v)).collect();

    // Stage 5.82: use return_kind for precise return type (TD-016 closure).
    // Previously (Stage 5.79), this used EmitType::I32 as a placeholder.
    // Now we convert call_info.return_kind (populated from
    // StdlibTraitMethod.return_kind via build_dyn_trait_method_calls_from_fat_ptrs)
    // to the correct EmitType.
    let ret_ty = stdlib_type_kind_to_emit_type(call_info.return_kind);
    emitter.emit_dyn_trait_method_call(&dynptr_symbol, call_info.slot_index, &arg_refs, &ret_ty)
}

fn codegen_terminator(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    term: &Terminator,
    ret_ty: &EmitType,
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
) {
    match term {
        Terminator::Return => {
            if *ret_ty == EmitType::Void {
                emitter.emit_ret(ret_ty, None);
            } else {
                let ret_val = if let Some(ptr) = emitter.get_local_ptr(0).cloned() {
                    Some(emitter.emit_load(ret_ty, &ptr))
                } else {
                    emitter.get_local(0).cloned()
                };
                emitter.emit_ret(ret_ty, ret_val.as_ref());
            }
        }
        Terminator::Unreachable => {
            emitter.emit_unreachable();
        }
        Terminator::Goto(target) => {
            emitter.emit_br(&format!("bb{}", target.0));
        }
        Terminator::SwitchInt {
            discr,
            targets,
            otherwise,
        } => {
            let discr_val = codegen_operand(emitter, mir, discr, interner, layouts);
            let is_bool_switch = targets
                .iter()
                .any(|(val, _)| matches!(val, ConstVal::Bool(_)));
            if is_bool_switch {
                let true_bb = targets
                    .iter()
                    .find(|(val, _)| matches!(val, ConstVal::Bool(true)))
                    .map(|(_, bb)| bb.0)
                    .unwrap_or(otherwise.0);
                let false_bb = otherwise.0;
                emitter.emit_br_cond(
                    &discr_val,
                    &format!("bb{}", true_bb),
                    &format!("bb{}", false_bb),
                );
            } else {
                let discr_ty = detect_operand_type(mir, discr, layouts).unwrap_or(EmitType::I32);
                let cases: Vec<(i128, String)> = targets
                    .iter()
                    .filter_map(|(val, bb)| match val {
                        ConstVal::Int(n) => Some((*n as i128, format!("bb{}", bb.0))),
                        ConstVal::Uint(n) => Some((*n as i128, format!("bb{}", bb.0))),
                        _ => None,
                    })
                    .collect();
                emitter.emit_switch(&discr_val, &discr_ty, &cases, &format!("bb{}", otherwise.0));
            }
        }
        Terminator::Call {
            func,
            args,
            destination,
            target,
        } => {
            // Stage 5.79: dyn Trait vtable indirect call path.
            //
            // Detect the marker `Operand::Constant(Const { ty: Error,
            // val: Int(index) })` where `index < mir.dyn_trait_calls.len()`.
            // If matched, dispatch to `codegen_dyn_trait_call` which emits
            // the vtable indirect call (getelementptr + load + call).
            //
            // Otherwise fall through to the legacy direct-call path.
            //
            // Per §16: MIR carries the dyn Trait info as data on
            // `mir.dyn_trait_calls` (populated by Stage 5.78's
            // `build_dyn_trait_call_terminator`). Codegen doesn't query
            // HIR or TraitResolver.
            if let Operand::Constant(c) = func {
                if matches!(c.ty.kind, crate::mir::ty::TyKind::Error) {
                    if let ConstVal::Int(idx) = c.val {
                        if (idx as usize) < mir.dyn_trait_calls.len() {
                            let ret_val =
                                codegen_dyn_trait_call(emitter, mir, idx, args, interner, layouts);
                            // Store the result to destination local.
                            if let PlaceKind::Local(id) = &destination.kind {
                                let dest_ty = mir
                                    .local_decls
                                    .get(id.0 as usize)
                                    .map(|ld| mir_type_to_emit_type_with_layouts(&ld.ty, layouts))
                                    .unwrap_or(EmitType::I32);
                                emitter.set_local(id.0, ret_val.clone());
                                if let Some(ptr) = emitter.get_local_ptr(id.0).cloned() {
                                    emitter.emit_store(&dest_ty, &ret_val, &ptr);
                                }
                            }
                            if let Some(cont) = target {
                                emitter.emit_br(&format!("bb{}", cont.0));
                            }
                            return;
                        }
                    }
                }
            }

            let fn_name = if let Operand::Copy(lv) | Operand::Move(lv) = func {
                if let PlaceKind::Local(id) = &lv.kind {
                    let local_ty = mir.local_decls.get(id.0 as usize).map(|ld| &ld.ty);
                    if let Some(ty) = local_ty {
                        if let crate::mir::ty::TyKind::FnDef(def_id, _) = &ty.kind {
                            fn_name_by_def_id.get(def_id).cloned()
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else if let Operand::Constant(c) = func {
                match &c.val {
                    ConstVal::Uint(n) => fn_name_by_def_id
                        .get(&crate::hir::DefId(*n as u32))
                        .cloned(),
                    ConstVal::Int(n) => fn_name_by_def_id
                        .get(&crate::hir::DefId(*n as u32))
                        .cloned(),
                    _ => None,
                }
            } else {
                None
            };

            let arg_pairs: Vec<(EmitType, EmitValue)> = args
                .iter()
                .map(|a| {
                    let ty = detect_operand_type(mir, a, layouts).unwrap_or(EmitType::I32);
                    let val = codegen_operand(emitter, mir, a, interner, layouts);
                    (ty, val)
                })
                .collect();
            let arg_refs: Vec<(EmitType, &EmitValue)> =
                arg_pairs.iter().map(|(t, v)| (t.clone(), v)).collect();

            let ret_val = if let Some(ref name) = fn_name {
                let call_ret_ty = if let PlaceKind::Local(id) = &destination.kind {
                    mir.local_decls
                        .get(id.0 as usize)
                        .map(|ld| mir_type_to_emit_type_with_layouts(&ld.ty, layouts))
                        .unwrap_or(EmitType::I32)
                } else {
                    EmitType::I32
                };
                emitter.emit_call(name, &arg_refs, &call_ret_ty)
            } else {
                "0".to_string()
            };

            if let PlaceKind::Local(id) = &destination.kind {
                let dest_ty = mir
                    .local_decls
                    .get(id.0 as usize)
                    .map(|ld| mir_type_to_emit_type_with_layouts(&ld.ty, layouts))
                    .unwrap_or(EmitType::I32);
                emitter.set_local(id.0, ret_val.clone());
                if let Some(ptr) = emitter.get_local_ptr(id.0).cloned() {
                    emitter.emit_store(&dest_ty, &ret_val, &ptr);
                }
            }
            if let Some(cont) = target {
                emitter.emit_br(&format!("bb{}", cont.0));
            }
        }
        Terminator::Assert {
            cond, target, msg, ..
        } => {
            let panic_label = format!("panic_assert_{}", target.0);
            match msg {
                crate::mir::body::AssertMessage::Overflow(op, lhs, rhs) => {
                    let op_ty = detect_operand_type(mir, lhs, layouts)
                        .or(detect_operand_type(mir, rhs, layouts))
                        .unwrap_or(EmitType::I32);
                    let lhs_val = codegen_operand(emitter, mir, lhs, interner, layouts);
                    let rhs_val = codegen_operand(emitter, mir, rhs, interner, layouts);
                    match op {
                        crate::mir::place::BinOp::Shl | crate::mir::place::BinOp::Shr => {
                            let bit_width: u32 = match op_ty {
                                EmitType::I8 => 8,
                                EmitType::I16 => 16,
                                EmitType::I32 => 32,
                                EmitType::I64 => 64,
                                EmitType::I128 => 128,
                                _ => 32,
                            };
                            let width_str = bit_width.to_string();
                            let is_overflow =
                                emitter.emit_icmp("uge", &op_ty, &rhs_val, &width_str);
                            emitter.emit_br_cond(
                                &is_overflow,
                                &panic_label,
                                &format!("bb{}", target.0),
                            );
                        }
                        _ => {
                            let agg = emitter.emit_checked_binop(*op, &op_ty, &lhs_val, &rhs_val);
                            let agg_ty = EmitType::struct_of(vec![op_ty.clone(), EmitType::I1]);
                            let overflow_flag = emitter.emit_extractvalue(&agg_ty, &agg, 1);
                            let inverted = emitter.emit_unop(
                                crate::mir::place::UnOp::Not,
                                &EmitType::I1,
                                &overflow_flag,
                            );
                            emitter.emit_br_cond(
                                &inverted,
                                &format!("bb{}", target.0),
                                &panic_label,
                            );
                        }
                    }
                }
                crate::mir::body::AssertMessage::DivisionByZero(rhs) => {
                    let rhs_val = codegen_operand(emitter, mir, rhs, interner, layouts);
                    let rhs_ty = detect_operand_type(mir, rhs, layouts).unwrap_or(EmitType::I32);
                    let is_zero = emitter.emit_icmp("eq", &rhs_ty, &rhs_val, &"0".to_string());
                    emitter.emit_br_cond(&is_zero, &panic_label, &format!("bb{}", target.0));
                }
                crate::mir::body::AssertMessage::BoundsCheck => {
                    let cond_val = codegen_operand(emitter, mir, cond, interner, layouts);
                    emitter.emit_br_cond(&cond_val, &format!("bb{}", target.0), &panic_label);
                }
            }
            emitter.emit_block(&panic_label);
            match msg {
                crate::mir::body::AssertMessage::Overflow(op, _, _) => {
                    let op_code: i32 = match op {
                        BinOp::Add => 0,
                        BinOp::Sub => 1,
                        BinOp::Mul => 2,
                        BinOp::Div => 3,
                        BinOp::Rem => 4,
                        BinOp::Shl => 5,
                        BinOp::Shr => 6,
                        _ => 99,
                    };
                    let op_str = op_code.to_string();
                    let z1 = "0".to_string();
                    let z2 = "0".to_string();
                    let _ = emitter.emit_call(
                        "__landin_panic_overflow",
                        &[
                            (EmitType::I32, &op_str),
                            (EmitType::I32, &z1),
                            (EmitType::I32, &z2),
                        ],
                        &EmitType::Void,
                    );
                }
                crate::mir::body::AssertMessage::DivisionByZero(_) => {
                    let _ = emitter.emit_call("__landin_panic_div_by_zero", &[], &EmitType::Void);
                }
                crate::mir::body::AssertMessage::BoundsCheck => {
                    let z1 = "0".to_string();
                    let z2 = "0".to_string();
                    let _ = emitter.emit_call(
                        "__landin_panic_bounds_check",
                        &[(EmitType::I64, &z1), (EmitType::I64, &z2)],
                        &EmitType::Void,
                    );
                }
            }
            emitter.emit_unreachable();
        }
        Terminator::Drop { place, target, .. } => {
            let _ = place;
            emitter.emit_br(&format!("bb{}", target.0));
        }
    }
}

// Stage 6.8: Type translation + place/operand helpers moved to mir_translation module.
pub(crate) use mir_translation::{
    codegen_place_load, codegen_place_load_typed, compute_place_address, detect_operand_type,
    detect_place_storage_type, detect_place_type, unwrap_fat_ptr_for_index,
};
pub use mir_translation::{mir_type_to_emit_type_with_layouts, stdlib_type_kind_to_emit_type};
