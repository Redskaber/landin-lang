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

pub use emitter::{
    emit_dyn_trait_ptr_type, emit_fat_ptr_type, emit_type_to_llvm_str, mir_type_to_emit_type,
    EmitType, EmitValue, Emitter,
};
pub use text_emitter::TextEmitter;

use crate::mir::body::*;
use crate::mir::place::*;
use crate::mir::ty::ConstVal;
use lasso::Rodeo;

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

/// Stage 5.6: Emit LLVM IR vtable globals for every (trait, type) pair in
/// `TraitResolver.vtables`.
///
/// Each vtable becomes a module-level global:
///
/// ```text
/// @.vtable.<trait>.<type> = private unnamed_addr constant
///     [N x ptr] [ptr @landin_<Type>_<m1>, ...]
/// ```
///
/// Per API-naming-standard §3: `emit_` prefix consistent with the rest of
/// the codegen module (`emit_fat_ptr_type`, `emit_type_to_llvm_str`, etc.).
///
/// Per §16: takes `&TraitResolver` (pre-built data) + `&Rodeo` (interner,
/// for symbol resolution) — no HIR access.
pub fn emit_vtables(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    emitter: &mut dyn Emitter,
) {
    for ((trait_name, self_ty_name), vtable) in &trait_resolver.vtables {
        // Build the global name: `.vtable.<trait>.<type>`.
        // LLVM global names use `.` as a private-name separator.
        let trait_str = interner.try_resolve(trait_name).unwrap_or("Trait");
        let type_str = interner.try_resolve(self_ty_name).unwrap_or("Type");
        let global_name = format!(".vtable.{}.{}", trait_str, type_str);

        // Collect the resolved method symbol names from VtableEntry.
        let method_symbols: Vec<String> =
            vtable.entries.iter().map(|e| e.fn_name.clone()).collect();

        emitter.emit_vtable_global(&global_name, &method_symbols);
    }
}

/// Stage 5.7: Emit `dyn Trait` fat-pointer constant globals for every
/// (trait, type) pair in `TraitResolver.vtables`.
///
/// Each `dyn Trait` fat pointer becomes a module-level global:
///
/// ```text
/// @.dynptr.<trait>.<type> = private unnamed_addr constant
///     { ptr, ptr } { ptr @.data.<type>, ptr @.vtable.<trait>.<type> }
/// ```
///
/// The fat pointer is `{ ptr (data), ptr (vtable) }` — the data pointer
/// references a per-type data global (`@.data.<type>`), and the vtable
/// pointer references the vtable global emitted by `emit_vtables` (Stage 5.6).
///
/// Per API-naming-standard §3: `emit_` prefix consistent with
/// `emit_vtables`, `emit_fat_ptr_type`, etc.
///
/// Per §16: takes `&TraitResolver` (pre-built data) + `&Rodeo` (interner)
/// — no HIR access.
pub fn emit_dyn_trait_ptrs(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    emitter: &mut dyn Emitter,
) {
    for (trait_name, self_ty_name) in trait_resolver.vtables.keys() {
        let trait_str = interner.try_resolve(trait_name).unwrap_or("Trait");
        let type_str = interner.try_resolve(self_ty_name).unwrap_or("Type");

        // Global names: matches the naming convention from emit_vtables.
        let dynptr_name = format!(".dynptr.{}.{}", trait_str, type_str);
        let data_symbol = format!(".data.{}", type_str);
        let vtable_symbol = format!(".vtable.{}.{}", trait_str, type_str);

        emitter.emit_dyn_trait_const(&dynptr_name, &data_symbol, &vtable_symbol);
    }
}

/// Stage 3.56: Generate LLVM IR from pre-built MIR + metadata.
/// This is the §16-compliant codegen entry point: takes only MIR data
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

    emitter.emit_function_end();
}

/// Stage 3.47 (L-PIPE-1 closure per §16): Map a MIR Ty to an EmitType,
/// resolving `TyKind::Adt(def_id, _)` via the `adt_layouts` side-table
/// on `MirBody` — **without reading HIR**.
///
/// Per §16 (阶段间接口隔离): MIR lower has sunk the ADT layouts into
/// `MirBody::adt_layouts` during lowering. Codegen now reads the layouts
/// from MIR, eliminating the cross-stage HIR lookup that was carried as
/// L-PIPE-1 debt since Stage 3.30.
///
/// Stage 3.48 (L-ENUM-UNION closure): enum storage layout now flattens
/// ALL non-empty variants' payload fields into the storage struct (was:
/// only the first non-empty variant's payload, a soundness bug for enums
/// with ≥2 non-empty variants of different widths). Layout is now:
///   - Case A (all unit variants): `{ discr }`
///   - Case B (exactly one non-empty variant): `{ discr, payload_fields... }`
///   - Case C (≥2 non-empty variants): `{ discr, variant_0_fields..., variant_1_fields..., ... }`
///     (unit variants contribute no fields; this is the soundness fix)
///
/// The flat layout means `variant_idx` does NOT directly map to `field_idx`
/// in the storage. The mapping is:
///   `field_idx(variant_V, field_F) = 1 + sum(field_counts of variants 0..V-1) + F`
/// This is computed in the `Aggregate(Adt(...))` codegen and in MIR lower's
/// pattern-binding projection generation.
/// **Stage 3.65 (P2 fix)**: This is the **canonical** §16-compliant
/// MIR→EmitType translation. It resolves `TyKind::Adt` via
/// `MirBody::adt_layouts` (the side-table populated by MIR lower per
/// §16 — no HIR access). Use this everywhere a `MirBody` is available.
///
/// The legacy `mir_type_to_emit_type` (no layouts) is kept only for
/// tests/standalone helpers where the type is known to be primitive.
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
                    EmitType::Void
                } else {
                    // Stage 3.47: recurse with `layouts` so nested Adts
                    // resolve correctly (e.g., `struct Outer { i: Inner }`
                    // renders as `{ { i32 } }`, not `{ i32 }`).
                    EmitType::Struct(
                        field_tys
                            .iter()
                            .map(|t| mir_type_to_emit_type_with_layouts(t, layouts))
                            .collect(),
                    )
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
                let mut field_tys =
                    vec![mir_type_to_emit_type_with_layouts(discriminant_ty, layouts)];
                for payload in variant_payloads {
                    for t in payload {
                        field_tys.push(mir_type_to_emit_type_with_layouts(t, layouts));
                    }
                }
                EmitType::Struct(field_tys)
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
                EmitType::Struct(
                    tys.iter()
                        .map(|t| mir_type_to_emit_type_with_layouts(t, layouts))
                        .collect(),
                )
            }
        }
        TyKind::Array(elem, len) => {
            let n = match &len.val {
                ConstVal::Int(n) | ConstVal::Uint(n) => *n as u64,
                _ => 0,
            };
            EmitType::array_of(mir_type_to_emit_type_with_layouts(elem, layouts), n)
        }
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) => {
            // Stage 3.49 (L13 closure): `&str` and `&[T]` are fat pointers
            // `{ ptr, len }`. Other references remain thin pointers.
            // Recurse with `_with_layouts` so the pointee (if it's an Adt)
            // resolves its layout correctly.
            match &inner.kind {
                TyKind::Str => crate::codegen::emit_fat_ptr_type(EmitType::I8),
                TyKind::Slice(elem) => crate::codegen::emit_fat_ptr_type(
                    mir_type_to_emit_type_with_layouts(elem, layouts),
                ),
                _ => EmitType::ptr_to(mir_type_to_emit_type_with_layouts(inner, layouts)),
            }
        }
        TyKind::Slice(elem) => EmitType::ptr_to(mir_type_to_emit_type_with_layouts(elem, layouts)),
        _ => mir_type_to_emit_type(ty),
    }
}

fn detect_place_storage_type(
    mir: &MirBody,
    lv: &Place,
    layouts: &crate::mir::body::AdtLayouts,
) -> EmitType {
    match &lv.kind {
        PlaceKind::Local(id) => mir
            .local_decls
            .get(id.0 as usize)
            .map(|ld| mir_type_to_emit_type_with_layouts(&ld.ty, layouts))
            .unwrap_or(EmitType::I32),
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
            // For Index/ConstantIndex/Deref, the storage type is the base's
            // storage type (we're indexing INTO the base's storage).
            _ => detect_place_storage_type(mir, base, layouts),
        },
        PlaceKind::Static(_) => EmitType::I32,
    }
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
fn detect_place_type(
    mir: &MirBody,
    lv: &Place,
    layouts: &crate::mir::body::AdtLayouts,
) -> EmitType {
    match &lv.kind {
        PlaceKind::Local(id) => mir
            .local_decls
            .get(id.0 as usize)
            .map(|ld| mir_type_to_emit_type_with_layouts(&ld.ty, layouts))
            .unwrap_or(EmitType::I32),
        PlaceKind::Projection(base, elem) => match elem {
            ProjectionElem::Deref => {
                let base_ty = detect_place_type(mir, base, layouts);
                if base_ty.is_ptr() {
                    base_ty.pointee()
                } else {
                    base_ty
                }
            }
            ProjectionElem::Field(_, field_ty) => {
                mir_type_to_emit_type_with_layouts(field_ty, layouts)
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
fn compute_place_address(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    lv: &Place,
    _interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
) -> String {
    match &lv.kind {
        PlaceKind::Local(id) => emitter
            .get_local_ptr(id.0)
            .cloned()
            .unwrap_or_else(|| "0".to_string()),
        PlaceKind::Projection(base, elem) => match elem {
            ProjectionElem::Field(field_id, _) => {
                let base_addr = compute_place_address(emitter, mir, base, _interner, layouts);
                let struct_ty = detect_place_storage_type(mir, base, layouts);
                emitter.emit_gep_field(&base_addr, &struct_ty, field_id.0)
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
fn unwrap_fat_ptr_for_index(
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
fn codegen_place_load_typed(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    lv: &Place,
    ty: EmitType,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
) -> EmitValue {
    match &lv.kind {
        PlaceKind::Local(id) => {
            if let Some(val) = emitter.get_local(id.0).cloned() {
                if !val.starts_with('%') {
                    return val;
                }
            }
            if let Some(ptr) = emitter.get_local_ptr(id.0).cloned() {
                emitter.emit_load(&ty, &ptr)
            } else {
                "0".to_string()
            }
        }
        PlaceKind::Projection(base, elem) => match elem {
            ProjectionElem::Deref => {
                let ptr_ty = detect_place_type(mir, base, layouts);
                let ptr_val =
                    codegen_place_load_typed(emitter, mir, base, ptr_ty.clone(), interner, layouts);
                emitter.emit_load(&ty, &ptr_val)
            }
            ProjectionElem::Field(field_id, _) => {
                let base_ptr = if let PlaceKind::Local(id) = &base.kind {
                    emitter
                        .get_local_ptr(id.0)
                        .cloned()
                        .unwrap_or_else(|| "0".to_string())
                } else {
                    let ptr_ty = detect_place_type(mir, base, layouts);
                    codegen_place_load_typed(emitter, mir, base, ptr_ty, interner, layouts)
                };
                let struct_ty = detect_place_storage_type(mir, base, layouts);
                let field_ptr = emitter.emit_gep_field(&base_ptr, &struct_ty, field_id.0);
                emitter.emit_load(&ty, &field_ptr)
            }
            ProjectionElem::Index(idx) => {
                let base_ptr = if let PlaceKind::Local(id) = &base.kind {
                    emitter
                        .get_local_ptr(id.0)
                        .cloned()
                        .unwrap_or_else(|| "0".to_string())
                } else {
                    let ptr_ty = detect_place_type(mir, base, layouts);
                    codegen_place_load_typed(emitter, mir, base, ptr_ty, interner, layouts)
                };
                let array_ty = detect_place_storage_type(mir, base, layouts);
                let idx_val = if let Some(v) = emitter.get_local(idx.0).cloned() {
                    v
                } else if let Some(ptr) = emitter.get_local_ptr(idx.0).cloned() {
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
                        .get_local_ptr(id.0)
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

fn codegen_place_load(
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

fn detect_operand_type(
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
                mir.local_decls
                    .get(id.0 as usize)
                    .map(|ld| mir_type_to_emit_type_with_layouts(&ld.ty, layouts))
            } else {
                Some(detect_place_type(mir, lv, layouts))
            }
        }
    }
}

// ============================================================================
// Stage 5.43: Codegen vtable emission helper (pure free function)
//
// New free function `emit_vtable_global_from_emission()` that takes a
// `&StdlibVtableEmission` and returns the LLVM IR text for one vtable global.
// This is the **pure-function counterpart** of
// `TextEmitter::emit_vtable_global()` — produces byte-for-byte identical
// IR, but doesn't require an `Emitter` trait object.
//
// This is the first Stage 5 sub-stage that modifies `src/codegen/`, but
// it does NOT modify the existing emission path:
//   - `emit_vtables()` (Stage 5.6) continues to iterate TraitResolver.vtables
//   - `TextEmitter::emit_vtable_global()` (Stage 5.6) continues to push to
//     `self.globals`
//
// The new function is parallel — Stage 5.44+ will refactor
// `TextEmitter::emit_vtable_global()` to delegate here, eliminating the
// duplicated LLVM IR formatting logic.
//
// Per API-naming-standard §3: `emit_vtable_global_from_emission` follows
// `<verb>_<noun>_<adj>_<prep>_<noun>` pattern. The `emit_` prefix is
// consistent with the rest of the codegen module (`emit_vtables`,
// `emit_dyn_trait_ptrs`, `emit_fat_ptr_type`).
//
// Per §16: takes `&StdlibVtableEmission` (stdlib-internal type) and returns
// `String`. No `mir::ty` / `traits::TraitResolver` / `Emitter` reference,
// no circular dependency.
// ============================================================================

/// Stage 5.43: Build the LLVM IR text for one vtable global from a
/// `StdlibVtableEmission`.
///
/// Produces a line like:
/// ```text
/// @.vtable.<trait>.<type> = private unnamed_addr constant [N x ptr] [ptr @sym1, ptr @sym2, ...]
/// ```
///
/// Edge cases:
/// - `method_symbols.is_empty()` (marker trait) → `... constant zeroinitializer`
/// - `method_symbols = ["null", ...]` → `ptr null` literal in the initializer
///
/// The output is **byte-for-byte identical** to what
/// `TextEmitter::emit_vtable_global()` produces — verified by
/// `test_emit_vtable_global_from_emission_match_text_emitter` in
/// `tests/v0/stage5/plan/codegen_vtable_emission_helper_tests.rs`.
///
/// Per API-naming-standard §3: `emit_vtable_global_from_emission` follows
/// `<verb>_<noun>_<adj>_<prep>_<noun>` pattern.
pub fn emit_vtable_global_from_emission(emission: &crate::stdlib::StdlibVtableEmission) -> String {
    // Build the LLVM initializer expression — mirrors
    // TextEmitter::emit_vtable_global (text_emitter.rs:538-546).
    //
    // Stage 5.43: the `method_symbols` entries may be either:
    //   - a real symbol name like "landin_S_clone" → emit as `ptr @landin_S_clone`
    //   - the literal string "null" (from `stdlib_vtable_method_symbols` when
    //     a slot is not provided) → emit as `ptr null` (no `@` prefix)
    let init = if emission.method_symbols.is_empty() {
        "zeroinitializer".to_string()
    } else {
        let entries: Vec<String> = emission
            .method_symbols
            .iter()
            .map(|sym| {
                if sym == "null" {
                    "ptr null".to_string()
                } else {
                    format!("ptr @{}", sym)
                }
            })
            .collect();
        format!(
            "[{} x ptr] [{}]",
            emission.method_symbols.len(),
            entries.join(", ")
        )
    };

    format!(
        "@{} = private unnamed_addr constant {}",
        emission.global_name, init
    )
}

/// Stage 5.44: Build the LLVM IR text for one vtable global from raw
/// `(global_name, method_symbols)` parameters.
///
/// This is the **bridge function** between Stage 5.43's
/// `emit_vtable_global_from_emission()` (high-level, takes
/// `StdlibVtableEmission`) and the future Stage 5.45 refactor where
/// `TextEmitter::emit_vtable_global()` will delegate here.
///
/// Parameter signature matches `TextEmitter::emit_vtable_global()` exactly
/// — `(global_name: &str, method_symbols: &[String])` — so the Stage 5.45
/// delegation is a trivial body change.
///
/// Produces a line like:
/// ```text
/// @<global_name> = private unnamed_addr constant [N x ptr] [ptr @sym1, ptr @sym2, ...]
/// ```
///
/// Edge cases:
/// - `method_symbols.is_empty()` → `... constant zeroinitializer`
/// - `method_symbols = ["null", ...]` → `ptr null` literal (no `@` prefix)
///
/// Per API-naming-standard §3: `emit_vtable_global_text` follows
/// `<verb>_<noun>_<adj>_<noun>` pattern. The `_text` suffix indicates the
/// function returns LLVM IR text (String), distinguishing it from the
/// trait method's side-effect version.
///
/// Per §16: pure function, input `(&str, &[String])`, output `String`. No
/// `mir::ty` / `traits::TraitResolver` / `Emitter` / `StdlibVtableEmission`
/// reference, no circular dependency.
pub fn emit_vtable_global_text(global_name: &str, method_symbols: &[String]) -> String {
    // Build the LLVM initializer expression.
    //
    // Stage 5.44: handles `"null"` strings in `method_symbols` → `ptr null`
    // literal (no `@` prefix). This matches the behavior of
    // `emit_vtable_global_from_emission()` (Stage 5.43) and prepares for
    // Stage 5.45 where `TextEmitter::emit_vtable_global()` will delegate
    // here.
    let init = if method_symbols.is_empty() {
        "zeroinitializer".to_string()
    } else {
        let entries: Vec<String> = method_symbols
            .iter()
            .map(|sym| {
                if sym == "null" {
                    "ptr null".to_string()
                } else {
                    format!("ptr @{}", sym)
                }
            })
            .collect();
        format!("[{} x ptr] [{}]", method_symbols.len(), entries.join(", "))
    };

    format!("@{} = private unnamed_addr constant {}", global_name, init)
}

// ============================================================================
// Stage 5.45: Codegen vtable emission batch helper
//
// Batch version of `emit_vtable_global_text()` (Stage 5.44). Takes a slice
// of `StdlibVtableGlobalSpec` and returns `Vec<String>` — one LLVM IR line
// per spec. Prepares for Stage 5.46 refactor where `emit_vtables()` will
// construct the spec list once, call this batch helper, and push all IR
// lines to the emitter in one pass.
//
// Per API-naming-standard §3:
//   - `StdlibVtableGlobalSpec` follows `<Noun><Noun><Noun><Noun>` pattern.
//   - `emit_vtable_globals_batch` follows `<verb>_<noun>_<adj>_<noun>`
//     pattern. `_batch` suffix indicates batch version; `_globals` (plural)
//     distinguishes from Stage 5.44's `emit_vtable_global_text` (singular).
//
// Per §16: uses only String + Vec<String> — no `mir::ty` /
// `traits::TraitResolver` / `Emitter` / `StdlibVtableEmission` reference,
// no circular dependency.
// ============================================================================

/// Stage 5.45: Specification for one vtable global — the inputs needed by
/// `emit_vtable_global_text()` packaged as a struct for batch processing.
///
/// Codegen constructs a `Vec<StdlibVtableGlobalSpec>` (one per (trait, type)
/// pair in `TraitResolver.vtables`), then calls
/// `emit_vtable_globals_batch()` to generate all IR lines in one pass.
///
/// Per API-naming-standard §3: `StdlibVtableGlobalSpec` follows
/// `<Noun><Noun><Noun><Noun>` pattern. Field names follow `<noun>_<noun>`
/// pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdlibVtableGlobalSpec {
    /// LLVM global name (e.g. `.vtable.Clone.S` — without leading `@`).
    pub global_name: String,
    /// Ordered method symbol list — each entry is either a real symbol
    /// (e.g. `landin_S_clone`) or the literal `"null"` for missing slots.
    pub method_symbols: Vec<String>,
}

/// Stage 5.45: Build LLVM IR text for multiple vtable globals in one call.
///
/// Given a slice of `StdlibVtableGlobalSpec`, returns `Vec<String>` where
/// each element is one vtable global definition (one LLVM IR line). The
/// output order matches the input order — no sorting or deduplication.
///
/// Each output line is identical to what `emit_vtable_global_text()` (Stage
/// 5.44) produces for the corresponding spec — verified by
/// `test_emit_vtable_globals_batch_matches_individual`.
///
/// Empty input returns an empty Vec.
///
/// Per API-naming-standard §3: `emit_vtable_globals_batch` follows
/// `<verb>_<noun>_<adj>_<noun>` pattern.
pub fn emit_vtable_globals_batch(specs: &[StdlibVtableGlobalSpec]) -> Vec<String> {
    specs
        .iter()
        .map(|spec| emit_vtable_global_text(&spec.global_name, &spec.method_symbols))
        .collect()
}

// ============================================================================
// Stage 5.46: Codegen vtable spec builder
//
// Pure free function that extracts the "construct spec list" logic from
// `emit_vtables()` into a standalone function. Takes `&TraitResolver` +
// `&Rodeo` (same inputs as `emit_vtables()`), returns
// `Vec<StdlibVtableGlobalSpec>`.
//
// Stage 5.47 will refactor `emit_vtables()` to call this builder +
// `emit_vtable_globals_batch()` + push all IR lines to emitter in one pass.
//
// Per API-naming-standard §3: `build_vtable_global_specs` follows
// `<verb>_<noun>_<adj>_<noun>` pattern. The `build_` prefix indicates a
// constructor function (input data → output data, no side effects).
//
// Per §16: takes `&TraitResolver` + `&Rodeo` (same as `emit_vtables()`),
// returns `Vec<StdlibVtableGlobalSpec>`. No `mir::ty` / `Emitter` reference,
// no circular dependency.
// ============================================================================

/// Stage 5.46: Build the list of `StdlibVtableGlobalSpec` from
/// `TraitResolver.vtables`.
///
/// For each `((trait_name, self_ty_name), vtable)` entry in
/// `trait_resolver.vtables`, constructs a `StdlibVtableGlobalSpec` with:
/// - `global_name = format!(".vtable.{trait_str}.{type_str}")`
///   where `trait_str = interner.try_resolve(trait_name).unwrap_or("Trait")`
///   and `type_str = interner.try_resolve(self_ty_name).unwrap_or("Type")`
/// - `method_symbols = vtable.entries.iter().map(|e| e.fn_name.clone()).collect()`
///
/// This is the **pure-function extraction** of the spec-construction logic
/// currently inlined in `emit_vtables()` (Stage 5.6). Stage 5.47 will
/// refactor `emit_vtables()` to call this builder + `emit_vtable_globals_batch()`
/// + push all IR lines to emitter in one pass.
///
/// Per API-naming-standard §3: `build_vtable_global_specs` follows
/// `<verb>_<noun>_<adj>_<noun>` pattern.
pub fn build_vtable_global_specs(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
) -> Vec<StdlibVtableGlobalSpec> {
    let mut specs: Vec<StdlibVtableGlobalSpec> = Vec::new();
    for ((trait_name, self_ty_name), vtable) in &trait_resolver.vtables {
        // Build the global name: `.vtable.<trait>.<type>`.
        // LLVM global names use `.` as a private-name separator.
        let trait_str = interner.try_resolve(trait_name).unwrap_or("Trait");
        let type_str = interner.try_resolve(self_ty_name).unwrap_or("Type");
        let global_name = format!(".vtable.{trait_str}.{type_str}");

        // Collect the resolved method symbol names from VtableEntry.
        let method_symbols: Vec<String> =
            vtable.entries.iter().map(|e| e.fn_name.clone()).collect();

        specs.push(StdlibVtableGlobalSpec {
            global_name,
            method_symbols,
        });
    }
    specs
}

// ============================================================================
// Stage 5.47: Codegen vtable emission orchestrator
//
// Composes Stage 5.46's `build_vtable_global_specs()` + per-spec
// `Emitter::emit_vtable_global()` calls. This is the "pure-function +
// side-effect" combination version of `emit_vtables()` current inline loop.
//
// Stage 5.48 will refactor `emit_vtables()` to delegate to this orchestrator
// — its body becomes a one-liner.
//
// Per API-naming-standard §3: `emit_vtables_from_resolver` follows
// `<verb>_<noun>_<prep>_<noun>` pattern. The `emit_` prefix indicates
// side-effect (push to emitter). `_from_resolver` indicates the input source.
//
// Per §16: takes `&TraitResolver` + `&Rodeo` + `&mut dyn Emitter` (same as
// `emit_vtables()`). No `mir::ty` reference, no circular dependency.
// ============================================================================

/// Stage 5.47: Emit vtable globals by composing `build_vtable_global_specs()`
/// + per-spec `Emitter::emit_vtable_global()` calls.
///
/// This is the **orchestrator** that combines:
/// 1. Stage 5.46's `build_vtable_global_specs()` — construct spec list
/// 2. Per-spec `Emitter::emit_vtable_global()` — push IR to emitter
///
/// Behavior is **identical** to `emit_vtables()` (Stage 5.6) current inline
/// loop — verified by `test_emit_vtables_from_resolver_match_emit_vtables`.
///
/// Stage 5.48 will refactor `emit_vtables()` to delegate to this orchestrator:
/// ```text
/// pub fn emit_vtables(resolver, interner, emitter) {
///     emit_vtables_from_resolver(resolver, interner, emitter)
/// }
/// ```
///
/// Per API-naming-standard §3: `emit_vtables_from_resolver` follows
/// `<verb>_<noun>_<prep>_<noun>` pattern.
pub fn emit_vtables_from_resolver(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    emitter: &mut dyn Emitter,
) {
    let specs = build_vtable_global_specs(trait_resolver, interner);
    for spec in &specs {
        emitter.emit_vtable_global(&spec.global_name, &spec.method_symbols);
    }
}

// ============================================================================
// Stage 5.48: Codegen dynptr global text helper
//
// Pure free function `emit_dynptr_global_text()` that takes
// `(global_name, data_symbol, vtable_symbol)` — the **exact same parameter
// signature** as `TextEmitter::emit_dyn_trait_const()` — and returns the
// LLVM IR text for one dyn Trait fat-pointer global.
//
// This is the **dynptr counterpart** of Stage 5.44's
// `emit_vtable_global_text()`. Stage 5.49 will refactor
// `TextEmitter::emit_dyn_trait_const()` to delegate here (trivial body
// change, same signature).
//
// Per API-naming-standard §3: `emit_dynptr_global_text` follows
// `<verb>_<noun>_<adj>_<noun>` pattern. The `_text` suffix indicates the
// function returns LLVM IR text (String), distinguishing it from the trait
// method's side-effect version. Naming symmetric with Stage 5.44's
// `emit_vtable_global_text` (vtable → dynptr).
//
// Per §16: pure function, input `(&str, &str, &str)`, output `String`. No
// `mir::ty` / `traits::TraitResolver` / `Emitter` / `StdlibVtableEmission`
// reference, no circular dependency.
// ============================================================================

/// Stage 5.48: Build the LLVM IR text for one `dyn Trait` fat-pointer global
/// from raw `(global_name, data_symbol, vtable_symbol)` parameters.
///
/// This is the **dynptr counterpart** of Stage 5.44's
/// `emit_vtable_global_text()`. Parameter signature matches
/// `TextEmitter::emit_dyn_trait_const()` exactly — Stage 5.49 delegation
/// is a trivial body change.
///
/// Produces a line like:
/// ```text
/// @<global_name> = private unnamed_addr constant
///     { ptr, ptr } { ptr @<data_symbol>, ptr @<vtable_symbol> }
/// ```
///
/// Example:
/// ```text
/// @.dynptr.Foo.S = private unnamed_addr constant
///     { ptr, ptr } { ptr @.data.S, ptr @.vtable.Foo.S }
/// ```
///
/// The output is **byte-for-byte identical** to what
/// `TextEmitter::emit_dyn_trait_const()` produces — verified by
/// `test_emit_dynptr_global_text_match_text_emitter` in
/// `tests/v0/stage5/plan/codegen_dynptr_text_tests.rs`.
///
/// Per API-naming-standard §3: `emit_dynptr_global_text` follows
/// `<verb>_<noun>_<adj>_<noun>` pattern.
pub fn emit_dynptr_global_text(
    global_name: &str,
    data_symbol: &str,
    vtable_symbol: &str,
) -> String {
    // Build the LLVM initializer — mirrors
    // TextEmitter::emit_dyn_trait_const (text_emitter.rs:569-572).
    let init = format!(
        "{{ ptr, ptr }} {{ ptr @{}, ptr @{} }}",
        data_symbol, vtable_symbol
    );
    format!("@{} = private unnamed_addr constant {}", global_name, init)
}

// ============================================================================
// Stage 5.49: Codegen dynptr spec builder
//
// Pure free function that extracts the "construct dynptr spec list" logic
// from `emit_dyn_trait_ptrs()` into a standalone function. Takes
// `&TraitResolver` + `&Rodeo` (same inputs as `emit_dyn_trait_ptrs()`),
// returns `Vec<StdlibDynptrGlobalSpec>`.
//
// This is the **dynptr counterpart** of Stage 5.46's
// `build_vtable_global_specs()`. Stage 5.50 will refactor
// `emit_dyn_trait_ptrs()` to call this builder + per-spec
// `Emitter::emit_dyn_trait_const()` calls.
//
// Per API-naming-standard §3: `build_dynptr_global_specs` follows
// `<verb>_<noun>_<adj>_<noun>` pattern. The `build_` prefix indicates a
// constructor function (input data → output data, no side effects).
// Naming symmetric with Stage 5.46's `build_vtable_global_specs` (vtable → dynptr).
//
// Per §16: takes `&TraitResolver` + `&Rodeo` (same as `emit_dyn_trait_ptrs()`),
// returns `Vec<StdlibDynptrGlobalSpec>`. No `mir::ty` / `Emitter` reference,
// no circular dependency.
// ============================================================================

/// Stage 5.49: Specification for one `dyn Trait` fat-pointer global — the
/// inputs needed by `emit_dynptr_global_text()` (Stage 5.48) packaged as a
/// struct for batch processing.
///
/// This is the **dynptr counterpart** of Stage 5.45's
/// `StdlibVtableGlobalSpec`. Codegen constructs a
/// `Vec<StdlibDynptrGlobalSpec>` (one per (trait, type) pair in
/// `TraitResolver.vtables`), then in Stage 5.50 will call
/// `emit_dynptr_global_text()` per spec to generate all IR lines.
///
/// Per API-naming-standard §3: `StdlibDynptrGlobalSpec` follows
/// `<Noun><Noun><Noun><Noun>` pattern. Naming symmetric with
/// `StdlibVtableGlobalSpec` (vtable → dynptr). Field names follow
/// `<noun>_<noun>` pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdlibDynptrGlobalSpec {
    /// LLVM global name (e.g. `.dynptr.Foo.S` — without leading `@`).
    pub global_name: String,
    /// Data symbol (e.g. `.data.S` — references the per-type data global).
    pub data_symbol: String,
    /// Vtable symbol (e.g. `.vtable.Foo.S` — references the vtable global).
    pub vtable_symbol: String,
}

/// Stage 5.49: Build the list of `StdlibDynptrGlobalSpec` from
/// `TraitResolver.vtables`.
///
/// For each `(trait_name, self_ty_name)` key in `trait_resolver.vtables`,
/// constructs a `StdlibDynptrGlobalSpec` with:
/// - `global_name = format!(".dynptr.{trait_str}.{type_str}")`
/// - `data_symbol = format!(".data.{type_str}")`
/// - `vtable_symbol = format!(".vtable.{trait_str}.{type_str}")`
///
/// where `trait_str = interner.try_resolve(trait_name).unwrap_or("Trait")`
/// and `type_str = interner.try_resolve(self_ty_name).unwrap_or("Type")`.
///
/// This is the **pure-function extraction** of the spec-construction logic
/// currently inlined in `emit_dyn_trait_ptrs()` (Stage 5.7). Stage 5.50
/// will refactor `emit_dyn_trait_ptrs()` to call this builder + per-spec
/// `Emitter::emit_dyn_trait_const()` calls.
///
/// Per API-naming-standard §3: `build_dynptr_global_specs` follows
/// `<verb>_<noun>_<adj>_<noun>` pattern. Naming symmetric with Stage 5.46's
/// `build_vtable_global_specs` (vtable → dynptr).
pub fn build_dynptr_global_specs(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
) -> Vec<StdlibDynptrGlobalSpec> {
    let mut specs: Vec<StdlibDynptrGlobalSpec> = Vec::new();
    for (trait_name, self_ty_name) in trait_resolver.vtables.keys() {
        // Global names: matches the naming convention from emit_vtables.
        let trait_str = interner.try_resolve(trait_name).unwrap_or("Trait");
        let type_str = interner.try_resolve(self_ty_name).unwrap_or("Type");

        let global_name = format!(".dynptr.{trait_str}.{type_str}");
        let data_symbol = format!(".data.{type_str}");
        let vtable_symbol = format!(".vtable.{trait_str}.{type_str}");

        specs.push(StdlibDynptrGlobalSpec {
            global_name,
            data_symbol,
            vtable_symbol,
        });
    }
    specs
}

// ============================================================================
// Stage 5.50: Codegen dynptr emission orchestrator
//
// Composes Stage 5.49's `build_dynptr_global_specs()` + per-spec
// `Emitter::emit_dyn_trait_const()` calls. This is the "pure-function +
// side-effect" combination version of `emit_dyn_trait_ptrs()` current
// inline loop.
//
// This is the **dynptr counterpart** of Stage 5.47's
// `emit_vtables_from_resolver()`. Stage 5.51 will refactor
// `emit_dyn_trait_ptrs()` to delegate to this orchestrator — its body
// becomes a one-liner.
//
// Per API-naming-standard §3: `emit_dynptrs_from_resolver` follows
// `<verb>_<noun>_<prep>_<noun>` pattern. The `emit_` prefix indicates
// side-effect (push to emitter). `_from_resolver` indicates the input source.
// Naming symmetric with Stage 5.47's `emit_vtables_from_resolver`
// (vtables → dynptrs).
//
// Per §16: takes `&TraitResolver` + `&Rodeo` + `&mut dyn Emitter` (same as
// `emit_dyn_trait_ptrs()`). No `mir::ty` reference, no circular dependency.
// ============================================================================

/// Stage 5.50: Emit `dyn Trait` fat-pointer globals by composing
/// `build_dynptr_global_specs()` + per-spec `Emitter::emit_dyn_trait_const()`
/// calls.
///
/// This is the **orchestrator** that combines:
/// 1. Stage 5.49's `build_dynptr_global_specs()` — construct spec list
/// 2. Per-spec `Emitter::emit_dyn_trait_const()` — push IR to emitter
///
/// Behavior is **identical** to `emit_dyn_trait_ptrs()` (Stage 5.7) current
/// inline loop — verified by `test_emit_dynptrs_from_resolver_match_emit_dyn_trait_ptrs`.
///
/// Stage 5.51 will refactor `emit_dyn_trait_ptrs()` to delegate to this
/// orchestrator:
/// ```text
/// pub fn emit_dyn_trait_ptrs(resolver, interner, emitter) {
///     emit_dynptrs_from_resolver(resolver, interner, emitter)
/// }
/// ```
///
/// Per API-naming-standard §3: `emit_dynptrs_from_resolver` follows
/// `<verb>_<noun>_<prep>_<noun>` pattern. Naming symmetric with Stage 5.47's
/// `emit_vtables_from_resolver` (vtables → dynptrs).
pub fn emit_dynptrs_from_resolver(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    emitter: &mut dyn Emitter,
) {
    let specs = build_dynptr_global_specs(trait_resolver, interner);
    for spec in &specs {
        emitter.emit_dyn_trait_const(&spec.global_name, &spec.data_symbol, &spec.vtable_symbol);
    }
}

// ============================================================================
// Stage 5.51: Codegen vtable + dynptr combined emission orchestrator
//
// Single entry point that composes Stage 5.47's `emit_vtables_from_resolver()`
// + Stage 5.50's `emit_dynptrs_from_resolver()`. Emits ALL trait-dispatch
// globals (vtable + dynptr) in one call.
//
// Stage 5.52 will refactor driver/codegen to call this combined orchestrator
// instead of separately calling `emit_vtables()` + `emit_dyn_trait_ptrs()`.
//
// Per API-naming-standard §3: `emit_vtables_and_dynptrs_from_resolver`
// follows `<verb>_<noun>_<conj>_<noun>_<prep>_<noun>` pattern. The `_and_`
// conjunction connects the two noun phrases (vtables + dynptrs).
//
// Per §16: takes `&TraitResolver` + `&Rodeo` + `&mut dyn Emitter` (same as
// `emit_vtables()` + `emit_dyn_trait_ptrs()`). No `mir::ty` reference, no
// circular dependency.
// ============================================================================

/// Stage 5.51: Emit ALL trait-dispatch globals (vtable + dynptr) by composing
/// `emit_vtables_from_resolver()` (Stage 5.47) + `emit_dynptrs_from_resolver()`
/// (Stage 5.50).
///
/// This is the **single entry point** for codegen to emit all trait-dispatch
/// globals. Stage 5.52 will refactor driver/codegen to call this combined
/// orchestrator instead of separately calling `emit_vtables()` +
/// `emit_dyn_trait_ptrs()`.
///
/// Behavior is **identical** to calling `emit_vtables()` + `emit_dyn_trait_ptrs()`
/// separately — verified by `test_emit_vtables_and_dynptrs_match_separate_calls`.
///
/// Per API-naming-standard §3: `emit_vtables_and_dynptrs_from_resolver`
/// follows `<verb>_<noun>_<conj>_<noun>_<prep>_<noun>` pattern. The `_and_`
/// conjunction connects the two noun phrases (vtables + dynptrs).
pub fn emit_vtables_and_dynptrs_from_resolver(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    emitter: &mut dyn Emitter,
) {
    // Emit vtable globals first (Stage 5.47 orchestrator).
    emit_vtables_from_resolver(trait_resolver, interner, emitter);
    // Then emit dynptr globals (Stage 5.50 orchestrator).
    emit_dynptrs_from_resolver(trait_resolver, interner, emitter);
}

// ============================================================================
// Stage 5.52: Codegen trait-dispatch emission summary
//
// Project-level aggregate statistics for trait-dispatch global emission.
// Counts vtable + dynptr globals, collects deduplicated trait/type names,
// sums total method slots. This is the **codegen counterpart** of Stage
// 5.42's `stdlib_vtable_emission_summary()`, but computed from
// `TraitResolver` rather than from a list of `StdlibVtableEmission`.
//
// Stage 5.53 will use this for codegen diagnostic output ("emit N vtable
// globals, M dynptr globals, K total method slots").
//
// Per API-naming-standard §3:
//   - `CodegenTraitDispatchEmissionSummary` follows
//     `<Noun><Noun><Noun><Noun><Noun>` pattern.
//   - `build_trait_dispatch_emission_summary` follows
//     `<verb>_<noun>_<noun>_<noun>_<noun>` pattern.
//
// Per §16: takes `&TraitResolver` + `&Rodeo` (same as `emit_vtables()`),
// returns `CodegenTraitDispatchEmissionSummary`. No `mir::ty` / `Emitter`
// reference, no circular dependency.
// ============================================================================

/// Stage 5.52: Project-level trait-dispatch emission statistics.
///
/// Aggregates vtable + dynptr global counts, deduplicated trait/type names,
/// and total method slots from `TraitResolver.vtables`. Useful for:
/// - Codegen diagnostics ("emit N vtable globals, M dynptr globals")
/// - Detecting trait-dispatch bloat (large `total_method_slots`)
/// - Identifying trait-heavy code (many distinct `trait_names`)
///
/// Per API-naming-standard §3: `CodegenTraitDispatchEmissionSummary` follows
/// `<Noun><Noun><Noun><Noun><Noun>` pattern. Field names follow
/// `<noun>_<noun>` / `<adj>_<noun>_<noun>` patterns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodegenTraitDispatchEmissionSummary {
    /// Number of vtable globals to emit (= `TraitResolver.vtables.len()`).
    pub vtable_count: u32,
    /// Number of dynptr globals to emit (= `TraitResolver.vtables.len()`,
    /// one dynptr per (trait, type) pair).
    pub dynptr_count: u32,
    /// Total global count (`vtable_count + dynptr_count`).
    pub total_global_count: u32,
    /// Deduplicated list of trait names involved (resolved via interner,
    /// or "Trait" default if unresolved).
    pub trait_names: Vec<String>,
    /// Deduplicated list of type names involved (resolved via interner,
    /// or "Type" default if unresolved).
    pub type_names: Vec<String>,
    /// Sum of `vtable.entries.len()` across all vtables — total method
    /// slots across all vtable globals.
    pub total_method_slots: u32,
}

/// Stage 5.52: Build a project-level trait-dispatch emission summary from
/// `TraitResolver.vtables`.
///
/// Given a `&TraitResolver` + `&Rodeo`, returns a
/// `CodegenTraitDispatchEmissionSummary` aggregating vtable + dynptr global
/// counts, deduplicated trait/type names, and total method slots.
///
/// Empty `TraitResolver.vtables` returns a summary with all-zero counts and
/// empty name lists.
///
/// Per API-naming-standard §3: `build_trait_dispatch_emission_summary`
/// follows `<verb>_<noun>_<noun>_<noun>_<noun>` pattern.
pub fn build_trait_dispatch_emission_summary(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
) -> CodegenTraitDispatchEmissionSummary {
    let vtable_count = trait_resolver.vtables.len() as u32;
    let dynptr_count = vtable_count; // one dynptr per (trait, type) pair
    let total_global_count = vtable_count + dynptr_count;

    let mut trait_names: Vec<String> = Vec::new();
    let mut type_names: Vec<String> = Vec::new();
    let mut total_method_slots: u32 = 0;

    for ((trait_name, self_ty_name), vtable) in &trait_resolver.vtables {
        let trait_str = interner
            .try_resolve(trait_name)
            .unwrap_or("Trait")
            .to_string();
        let type_str = interner
            .try_resolve(self_ty_name)
            .unwrap_or("Type")
            .to_string();

        if !trait_names.contains(&trait_str) {
            trait_names.push(trait_str);
        }
        if !type_names.contains(&type_str) {
            type_names.push(type_str);
        }
        total_method_slots += vtable.entries.len() as u32;
    }

    CodegenTraitDispatchEmissionSummary {
        vtable_count,
        dynptr_count,
        total_global_count,
        trait_names,
        type_names,
        total_method_slots,
    }
}

// ============================================================================
// Stage 5.53: Codegen trait-dispatch emission plan (final aggregate)
//
// Single-call API that returns EVERYTHING codegen needs to emit all
// trait-dispatch globals:
//   - vtable_specs (from Stage 5.46 build_vtable_global_specs)
//   - dynptr_specs (from Stage 5.49 build_dynptr_global_specs)
//   - summary (from Stage 5.52 build_trait_dispatch_emission_summary)
//
// This is the **final aggregate API** — Stage 5.54 driver refactor will call
// this plan once, then iterate vtable_specs + dynptr_specs to emit globals,
// and use summary for diagnostic output.
//
// Per API-naming-standard §3:
//   - `CodegenTraitDispatchEmissionPlan` follows
//     `<Noun><Noun><Noun><Noun><Noun>` pattern.
//   - `build_trait_dispatch_emission_plan` follows
//     `<verb>_<noun>_<noun>_<noun>_<noun>` pattern.
//
// Per §16: takes `&TraitResolver` + `&Rodeo` (same as `emit_vtables()`),
// returns `CodegenTraitDispatchEmissionPlan`. No `mir::ty` / `Emitter`
// reference, no circular dependency.
// ============================================================================

/// Stage 5.53: Everything codegen needs to emit all trait-dispatch globals
/// in one struct.
///
/// Combines:
/// - `vtable_specs` (from Stage 5.46 `build_vtable_global_specs()`)
/// - `dynptr_specs` (from Stage 5.49 `build_dynptr_global_specs()`)
/// - `summary` (from Stage 5.52 `build_trait_dispatch_emission_summary()`)
///
/// Stage 5.54 driver refactor will call `build_trait_dispatch_emission_plan()`
/// once, then iterate `vtable_specs` + `dynptr_specs` to emit globals, and
/// use `summary` for diagnostic output.
///
/// Per API-naming-standard §3: `CodegenTraitDispatchEmissionPlan` follows
/// `<Noun><Noun><Noun><Noun><Noun>` pattern. Field names follow
/// `<noun>_<noun>` / `<noun>` patterns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodegenTraitDispatchEmissionPlan {
    /// Vtable global specs — one per (trait, type) pair in
    /// `TraitResolver.vtables`. Used by codegen to emit `@.vtable.*` globals.
    pub vtable_specs: Vec<StdlibVtableGlobalSpec>,
    /// Dynptr global specs — one per (trait, type) pair. Used by codegen to
    /// emit `@.dynptr.*` globals.
    pub dynptr_specs: Vec<StdlibDynptrGlobalSpec>,
    /// Project-level summary — counts + deduplicated names + total slots.
    /// Used by codegen for diagnostic output.
    pub summary: CodegenTraitDispatchEmissionSummary,
}

/// Stage 5.53: Build a complete trait-dispatch emission plan from
/// `TraitResolver.vtables`.
///
/// Given a `&TraitResolver` + `&Rodeo`, returns a
/// `CodegenTraitDispatchEmissionPlan` containing:
/// - `vtable_specs` (from `build_vtable_global_specs()` — Stage 5.46)
/// - `dynptr_specs` (from `build_dynptr_global_specs()` — Stage 5.49)
/// - `summary` (from `build_trait_dispatch_emission_summary()` — Stage 5.52)
///
/// This is the **final aggregate API** — one call returns everything codegen
/// needs to emit all trait-dispatch globals.
///
/// Per API-naming-standard §3: `build_trait_dispatch_emission_plan` follows
/// `<verb>_<noun>_<noun>_<noun>_<noun>` pattern.
pub fn build_trait_dispatch_emission_plan(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
) -> CodegenTraitDispatchEmissionPlan {
    CodegenTraitDispatchEmissionPlan {
        vtable_specs: build_vtable_global_specs(trait_resolver, interner),
        dynptr_specs: build_dynptr_global_specs(trait_resolver, interner),
        summary: build_trait_dispatch_emission_summary(trait_resolver, interner),
    }
}

// ============================================================================
// Stage 5.54: Codegen trait-dispatch emission orchestrator (plan-based)
//
// First **plan-based orchestrator** — takes a `&CodegenTraitDispatchEmissionPlan`
// (from Stage 5.53) + `&mut dyn Emitter`, emits all trait-dispatch globals
// (vtable + dynptr) by iterating the plan's specs.
//
// Stage 5.55 driver refactor will call `build_trait_dispatch_emission_plan()`
// + this orchestrator, replacing separate `emit_vtables()` +
// `emit_dyn_trait_ptrs()` calls.
//
// Per API-naming-standard §3: `emit_trait_dispatch_globals_from_plan`
// follows `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>` pattern. The `_from_plan`
// suffix indicates the input source (plan, not resolver).
//
// Per §16: takes `&CodegenTraitDispatchEmissionPlan` + `&mut dyn Emitter`. No
// `mir::ty` / `TraitResolver` / `Rodeo` reference, no circular dependency.
// ============================================================================

/// Stage 5.54: Emit all trait-dispatch globals (vtable + dynptr) from a
/// pre-built `CodegenTraitDispatchEmissionPlan`.
///
/// This is the **first plan-based orchestrator**. Stage 5.55 driver refactor
/// will call `build_trait_dispatch_emission_plan()` (Stage 5.53) + this
/// orchestrator, replacing separate `emit_vtables()` + `emit_dyn_trait_ptrs()`
/// calls.
///
/// Behavior is **identical** to `emit_vtables_and_dynptrs_from_resolver()`
/// (Stage 5.51) when given the plan from the same resolver — verified by
/// `test_emit_trait_dispatch_globals_from_plan_match_resolver_orchestrator`.
///
/// Per API-naming-standard §3: `emit_trait_dispatch_globals_from_plan`
/// follows `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>` pattern.
pub fn emit_trait_dispatch_globals_from_plan(
    plan: &CodegenTraitDispatchEmissionPlan,
    emitter: &mut dyn Emitter,
) {
    // Emit vtable globals first (matching emit_vtables order).
    for spec in &plan.vtable_specs {
        emitter.emit_vtable_global(&spec.global_name, &spec.method_symbols);
    }
    // Then emit dynptr globals (matching emit_dyn_trait_ptrs order).
    for spec in &plan.dynptr_specs {
        emitter.emit_dyn_trait_const(&spec.global_name, &spec.data_symbol, &spec.vtable_symbol);
    }
}

// ============================================================================
// Stage 5.55: Codegen trait-dispatch emission text batch (plan-based)
//
// Text-based batch generation of all trait-dispatch globals (vtable + dynptr)
// WITHOUT needing an `Emitter` trait object. This is the **plan-based
// counterpart** of Stage 5.45's `emit_vtable_globals_batch()`, extended to
// both vtable + dynptr globals.
//
// Useful for:
// - Testing (assert IR text directly, no Emitter construction needed)
// - Future codegen paths that push pre-formatted text to emitter.globals
// - Diagnostics (inspect the IR lines before emission)
//
// Per API-naming-standard §3: `emit_trait_dispatch_globals_text_batch`
// follows `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>` pattern. The
// `_text_batch` suffix indicates LLVM IR text batch (no Emitter).
//
// Per §16: takes `&CodegenTraitDispatchEmissionPlan`, returns `Vec<String>`.
// No `mir::ty` / `Emitter` / `TraitResolver` / `Rodeo` reference, no
// circular dependency.
// ============================================================================

/// Stage 5.55: Generate LLVM IR text for all trait-dispatch globals (vtable
/// and dynptr) from a pre-built `CodegenTraitDispatchEmissionPlan`, WITHOUT
/// needing an `Emitter` trait object.
///
/// This is the **plan-based counterpart** of Stage 5.45's
/// `emit_vtable_globals_batch()`, extended to both vtable + dynptr globals.
/// Each output element is one LLVM IR line (one global definition).
///
/// Returns `Vec<String>` where:
/// - First N elements are vtable global definitions (from `plan.vtable_specs`)
/// - Next M elements are dynptr global definitions (from `plan.dynptr_specs`)
/// - N == M == `plan.vtable_specs.len()` (one vtable + one dynptr per spec)
///
/// Each vtable line is identical to what `emit_vtable_global_text()` (Stage
/// 5.44) produces. Each dynptr line is identical to what
/// `emit_dynptr_global_text()` (Stage 5.48) produces.
///
/// Per API-naming-standard §3: `emit_trait_dispatch_globals_text_batch`
/// follows `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>` pattern.
pub fn emit_trait_dispatch_globals_text_batch(
    plan: &CodegenTraitDispatchEmissionPlan,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    // Vtable global IR text (Stage 5.44)
    for spec in &plan.vtable_specs {
        lines.push(emit_vtable_global_text(
            &spec.global_name,
            &spec.method_symbols,
        ));
    }
    // Dynptr global IR text (Stage 5.48)
    for spec in &plan.dynptr_specs {
        lines.push(emit_dynptr_global_text(
            &spec.global_name,
            &spec.data_symbol,
            &spec.vtable_symbol,
        ));
    }
    lines
}
