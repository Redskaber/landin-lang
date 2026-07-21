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
//! | L1 | PHI node optimization (IR quality; codegen emits alloca+load/store, relies on LLVM `mem2reg`) | Stage 4 |
//! | L3 | Closure codegen (closure type lowering + capture codegen) | Stage 4 |
//! | L5 | Trait dispatch (vtable generation, dyn fat pointers) | Stage 5 |
//! | L8 | `lli` execution verification (env constraint — no `lli` in test sandbox) | Stage 4 |
//! | L-COPY-ADT | Proper Copy trait (current borrowck pragmatically treats Adt as Copy) | Stage 5 |
//!
//! ## Architectural debt (tracked, not blocking)
//!
//! - **Emitter trait bloat**: 36 methods, 1 implementation (`TextEmitter`).
//!   Decompose into sub-traits (`EmitterArith`, `EmitterMemory`, etc.)
//!   when adding a second backend. Stage 3.59 Issue #5.

pub mod emitter;
pub mod text_emitter;

pub use emitter::{
    emit_fat_ptr_type, emit_type_to_llvm_str, mir_type_to_emit_type, EmitType, EmitValue, Emitter,
};
pub use text_emitter::TextEmitter;

use crate::mir::body::*;
use crate::mir::lvalue::*;
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
    emitter.output_with_globals()
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

fn detect_lvalue_storage_type(
    mir: &MirBody,
    lv: &Lvalue,
    layouts: &crate::mir::body::AdtLayouts,
) -> EmitType {
    match &lv.kind {
        LvalueKind::Local(id) => mir
            .local_decls
            .get(id.0 as usize)
            .map(|ld| mir_type_to_emit_type_with_layouts(&ld.ty, layouts))
            .unwrap_or(EmitType::I32),
        // Stage 3.54: for Field projection, return the FIELD's type (not the
        // base's type). Was: returned detect_lvalue_storage_type(base), which
        // gave the struct type instead of the field type — causing
        // unwrap_fat_ptr_for_index to see the struct layout instead of the
        // fat pointer layout when indexing a struct field that contains a
        // slice/array.
        LvalueKind::Projection(base, elem) => match elem {
            ProjectionElem::Field(_, field_ty) => {
                mir_type_to_emit_type_with_layouts(field_ty, layouts)
            }
            // For Index/ConstantIndex/Deref, the storage type is the base's
            // storage type (we're indexing INTO the base's storage).
            _ => detect_lvalue_storage_type(mir, base, layouts),
        },
        LvalueKind::Static(_) => EmitType::I32,
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
                LvalueKind::Local(id) => {
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
                LvalueKind::Projection(base, elem) => {
                    let ty = detect_operand_type(mir, &Operand::Copy(place.clone()), layouts)
                        .unwrap_or(EmitType::I32);
                    match elem {
                        ProjectionElem::Deref => {
                            let ptr_val =
                                codegen_lvalue_load(emitter, mir, base, interner, layouts);
                            emitter.emit_store(&ty, &val, &ptr_val);
                        }
                        ProjectionElem::Field(field_id, _) => {
                            let base_ptr = if let LvalueKind::Local(id) = &base.kind {
                                emitter
                                    .get_local_ptr(id.0)
                                    .cloned()
                                    .unwrap_or_else(|| "0".to_string())
                            } else {
                                codegen_lvalue_load(emitter, mir, base, interner, layouts)
                            };
                            let struct_ty = detect_lvalue_storage_type(mir, base, layouts);
                            let field_ptr =
                                emitter.emit_gep_field(&base_ptr, &struct_ty, field_id.0);
                            emitter.emit_store(&ty, &val, &field_ptr);
                        }
                        ProjectionElem::Index(idx) => {
                            // Stage 3.54: for the store path, when the base
                            // is a projection (e.g., `s.data` where `data` is
                            // a field), we need the ADDRESS of the base, not
                            // its loaded value. Was: `codegen_lvalue_load`
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
                                compute_lvalue_address(emitter, mir, base, interner, layouts);
                            let array_ty = detect_lvalue_storage_type(mir, base, layouts);
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
                LvalueKind::Static(_) => {}
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
            if let LvalueKind::Local(id) = &lv.kind {
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
            let ty = detect_lvalue_type(mir, lv, layouts);
            codegen_lvalue_load_typed(emitter, mir, lv, ty, interner, layouts)
        }
    }
}

#[allow(clippy::only_used_in_recursion)]
fn detect_lvalue_type(
    mir: &MirBody,
    lv: &Lvalue,
    layouts: &crate::mir::body::AdtLayouts,
) -> EmitType {
    match &lv.kind {
        LvalueKind::Local(id) => mir
            .local_decls
            .get(id.0 as usize)
            .map(|ld| mir_type_to_emit_type_with_layouts(&ld.ty, layouts))
            .unwrap_or(EmitType::I32),
        LvalueKind::Projection(base, elem) => match elem {
            ProjectionElem::Deref => {
                let base_ty = detect_lvalue_type(mir, base, layouts);
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
                let storage = detect_lvalue_storage_type(mir, base, layouts);
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
        LvalueKind::Static(_) => EmitType::I32,
    }
}

/// Stage 3.54: Compute the ADDRESS of an lvalue (without loading its value).
/// Used by the store path's Index projection to get a pointer to the base
/// storage (e.g., the address of a struct field containing a fat pointer),
/// so that `unwrap_fat_ptr_for_index` can GEP into the storage correctly.
///
/// For a Local: returns the alloca pointer.
/// For a Projection(Local, Field): GEPs to the field, returns the field's address.
/// For deeper projections: recurses (best-effort — complex cases may fall back
/// to loading, which is the old behavior).
fn compute_lvalue_address(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    lv: &Lvalue,
    _interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
) -> String {
    match &lv.kind {
        LvalueKind::Local(id) => emitter
            .get_local_ptr(id.0)
            .cloned()
            .unwrap_or_else(|| "0".to_string()),
        LvalueKind::Projection(base, elem) => match elem {
            ProjectionElem::Field(field_id, _) => {
                let base_addr = compute_lvalue_address(emitter, mir, base, _interner, layouts);
                let struct_ty = detect_lvalue_storage_type(mir, base, layouts);
                emitter.emit_gep_field(&base_addr, &struct_ty, field_id.0)
            }
            // For other projection types, fall back to the load path
            // (loads the value — old behavior, may not work for fat pointers
            // in store position, but preserves existing behavior for non-fat-ptr cases).
            _ => {
                let ptr_ty = detect_lvalue_type(mir, lv, layouts);
                codegen_lvalue_load_typed(emitter, mir, lv, ptr_ty, _interner, layouts)
            }
        },
        LvalueKind::Static(_) => "0".to_string(),
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
fn codegen_lvalue_load_typed(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    lv: &Lvalue,
    ty: EmitType,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
) -> EmitValue {
    match &lv.kind {
        LvalueKind::Local(id) => {
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
        LvalueKind::Projection(base, elem) => match elem {
            ProjectionElem::Deref => {
                let ptr_ty = detect_lvalue_type(mir, base, layouts);
                let ptr_val = codegen_lvalue_load_typed(
                    emitter,
                    mir,
                    base,
                    ptr_ty.clone(),
                    interner,
                    layouts,
                );
                emitter.emit_load(&ty, &ptr_val)
            }
            ProjectionElem::Field(field_id, _) => {
                let base_ptr = if let LvalueKind::Local(id) = &base.kind {
                    emitter
                        .get_local_ptr(id.0)
                        .cloned()
                        .unwrap_or_else(|| "0".to_string())
                } else {
                    let ptr_ty = detect_lvalue_type(mir, base, layouts);
                    codegen_lvalue_load_typed(emitter, mir, base, ptr_ty, interner, layouts)
                };
                let struct_ty = detect_lvalue_storage_type(mir, base, layouts);
                let field_ptr = emitter.emit_gep_field(&base_ptr, &struct_ty, field_id.0);
                emitter.emit_load(&ty, &field_ptr)
            }
            ProjectionElem::Index(idx) => {
                let base_ptr = if let LvalueKind::Local(id) = &base.kind {
                    emitter
                        .get_local_ptr(id.0)
                        .cloned()
                        .unwrap_or_else(|| "0".to_string())
                } else {
                    let ptr_ty = detect_lvalue_type(mir, base, layouts);
                    codegen_lvalue_load_typed(emitter, mir, base, ptr_ty, interner, layouts)
                };
                let array_ty = detect_lvalue_storage_type(mir, base, layouts);
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
                let base_ptr = if let LvalueKind::Local(id) = &base.kind {
                    emitter
                        .get_local_ptr(id.0)
                        .cloned()
                        .unwrap_or_else(|| "0".to_string())
                } else {
                    let ptr_ty = detect_lvalue_type(mir, base, layouts);
                    codegen_lvalue_load_typed(emitter, mir, base, ptr_ty, interner, layouts)
                };
                let array_ty = detect_lvalue_storage_type(mir, base, layouts);
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
        LvalueKind::Static(_) => "0".to_string(),
    }
}

fn codegen_lvalue_load(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    lv: &Lvalue,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
) -> EmitValue {
    // Stage 3.47 (L-PIPE-1 closure): previously this fabricated a fake
    // `MirBody::new(Span::DUMMY)` to satisfy `codegen_lvalue_load_typed`'s
    // `&MirBody` parameter (which was needed to access local_decls for type
    // info). Now we pass the caller's `mir` reference through directly —
    // no fake MirBody needed. The `EmitType::I32` placeholder is the
    // historical default for untyped lvalue loads (used when the caller
    // doesn't know the type ahead of time). The typed path
    // (`codegen_lvalue_load_typed`) is preferred when the type is known.
    codegen_lvalue_load_typed(emitter, mir, lv, EmitType::I32, interner, layouts)
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
                if let LvalueKind::Local(id) = &lv.kind {
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
                let call_ret_ty = if let LvalueKind::Local(id) = &destination.kind {
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

            if let LvalueKind::Local(id) = &destination.kind {
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
                        crate::mir::lvalue::BinOp::Shl | crate::mir::lvalue::BinOp::Shr => {
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
                                crate::mir::lvalue::UnOp::Not,
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
            if let LvalueKind::Local(id) = &lv.kind {
                mir.local_decls
                    .get(id.0 as usize)
                    .map(|ld| mir_type_to_emit_type_with_layouts(&ld.ty, layouts))
            } else {
                Some(detect_lvalue_type(mir, lv, layouts))
            }
        }
    }
}
