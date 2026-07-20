//! LLVM IR codegen: MIR → LLVM IR via Emitter trait.
//!
//! Stage 3.21 (v0.8.6): aggregates + call args now carry full type info.
//! - Tuples emit `{ i32, i64, ... }` (was hardcoded `{ i32 }`).
//! - Arrays emit `[N x T]` (was hardcoded `[10 x i32]`).
//! - Pointers emit `T*` (was `i32*`).
//! - Call args emit `i64 %v1, double %v2, ...` (was `i32 %v1, i32 %v2, ...`).
//! - GEP field/index now renders the actual struct/array type.

pub mod emitter;
pub mod text_emitter;

pub use emitter::{emit_type_to_llvm_str, mir_type_to_emit_type, EmitType, EmitValue, Emitter};
pub use text_emitter::TextEmitter;

use crate::hir::HirCrate;
use crate::mir::body::*;
use crate::mir::lvalue::*;
use crate::mir::ty::ConstVal;
use lasso::Rodeo;

/// Generate LLVM IR text for a crate.
pub fn codegen_crate(hir: &HirCrate, interner: &Rodeo) -> String {
    let mut emitter = TextEmitter::new();
    emitter.emit_header();
    // Emit external function declarations (per design doc §4.5, §10.2)
    emitter.emit_declare("void @__landin_panic_overflow(i32 %op, i32 %lhs, i32 %rhs)");
    emitter.emit_declare("void @__landin_panic_bounds_check(i64 %index, i64 %len)");
    emitter.emit_declare("void @__landin_panic_div_by_zero()");
    codegen_crate_with_emitter(hir, interner, &mut emitter);
    // Stage 3.27: use output_with_globals to include string-constant globals.
    emitter.output_with_globals()
}

/// Generate LLVM IR for a crate using any Emitter backend.
pub fn codegen_crate_with_emitter(hir: &HirCrate, interner: &Rodeo, emitter: &mut dyn Emitter) {
    // Collect function names from HIR.
    // Stage 3.30 fix: previously matched by `def_id.as_u32() == body_index`,
    // which was wrong because DefId and body index are different spaces
    // (struct/enum/trait owners have DefIds but no bodies, creating gaps).
    // Now we look up the fn owner whose `body` field matches the current
    // BodyId, then use that owner's ident for the function name.
    let fn_names: Vec<String> = hir
        .bodies
        .iter()
        .map(|(body_id, _)| {
            for (_def_id, owner) in &hir.owners {
                if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Fn(f)) = owner {
                    if f.body == Some(*body_id) {
                        let name = interner.try_resolve(&f.ident.name).unwrap_or("fn");
                        return format!("landin_{}", name);
                    }
                }
            }
            format!("fn_{}", body_id.owner.0.as_u32())
        })
        .collect();

    // Stage 3.30 fix: build a DefId → fn_name map so Call terminators can
    // resolve fn names by DefId (was: indexed by body index, which is a
    // different space — struct/enum/trait owners have DefIds but no bodies,
    // creating gaps that misaligned the index).
    let mut fn_name_by_def_id: std::collections::HashMap<crate::hir::DefId, String> =
        std::collections::HashMap::new();
    for (def_id, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Fn(f)) = owner {
            let name = interner.try_resolve(&f.ident.name).unwrap_or("fn");
            fn_name_by_def_id.insert(*def_id, format!("landin_{}", name));
        }
    }

    for (idx, (_, body)) in hir.bodies.iter().enumerate() {
        let fn_name = fn_names[idx].clone();
        let return_ty = crate::driver::owner_return_ty_for_body(hir, body);
        let (mut mir, unify) =
            crate::mir::lower::lower_hir_body_to_mir_full(body, interner, hir, return_ty);
        let mut tc = crate::typeck::TypeChecker::with_unify(unify);
        tc.populate_fn_sigs(hir);
        // Stage 3.32 (L-DEBT-2 fix): pass hir so typeck resolves ADT field
        // types in projections during writeback.
        tc.check_mir_body_with_hir(&mut mir, Some(hir));
        codegen_function(
            emitter,
            &fn_name,
            &mir,
            &fn_name_by_def_id,
            body.params.len(),
            interner,
            hir,
        );
    }
}

/// Generate LLVM IR for a single function.
fn codegen_function(
    emitter: &mut dyn Emitter,
    name: &str,
    mir: &MirBody,
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
    param_count: usize,
    interner: &Rodeo,
    hir: &HirCrate,
) {
    // Determine return type. Unit type (empty tuple) → void return.
    let ret_ty = if mir.local_decls.is_empty() {
        EmitType::Void
    } else {
        match &mir.local_decls[0].ty.kind {
            crate::mir::ty::TyKind::Tuple(tys) if tys.is_empty() => EmitType::Void,
            // Stage 3.30: use _with_hir so ADT-typed return values get
            // their full struct type.
            _ => mir_type_to_emit_type_with_hir(&mir.local_decls[0].ty, hir),
        }
    };

    // Build param list: LocalId(1)..LocalId(param_count) are fn params.
    let params: Vec<(EmitType, String)> = (0..param_count)
        .map(|i| {
            let local_idx = i + 1;
            let ty = mir
                .local_decls
                .get(local_idx)
                .map(|ld| mir_type_to_emit_type_with_hir(&ld.ty, hir))
                .unwrap_or(EmitType::I32);
            (ty, format!("%arg{}", i))
        })
        .collect();

    let param_refs: Vec<(EmitType, &str)> = params
        .iter()
        .map(|(t, n)| (t.clone(), n.as_str()))
        .collect();

    emitter.emit_function_begin(name, &param_refs, &ret_ty);

    // Emit allocas for all locals at function entry (per design doc §4.1)
    // Stage 3.27: skip void-typed locals — LLVM doesn't allow `alloca void`.
    // Unit-typed locals are dead values from MIR lowering (temp slots for
    // expressions that produce `()`); they don't need storage.
    // Stage 3.30: use mir_type_to_emit_type_with_hir so ADT-typed locals
    // get their full struct type (was: fell through to I32, producing
    // `alloca i32` for a struct-typed local — type mismatch on store).
    for (i, ld) in mir.local_decls.iter().enumerate() {
        let ty = mir_type_to_emit_type_with_hir(&ld.ty, hir);
        if ty == EmitType::Void {
            continue;
        }
        let ptr_name = format!("%loc_{}", i);
        let ptr = emitter.emit_alloca(&ty, &ptr_name);
        emitter.set_local_ptr(i as u32, ptr);
    }

    // Store params into their alloca slots
    for (i, (ty, arg_name)) in params.iter().enumerate() {
        let local_idx = (i + 1) as u32;
        if let Some(ptr) = emitter.get_local_ptr(local_idx).cloned() {
            emitter.emit_store(ty, arg_name, &ptr);
        }
    }

    // Walk basic blocks
    for (bb_idx, bb) in mir.basic_blocks.iter().enumerate() {
        let label = format!("bb{}", bb_idx);
        emitter.emit_block(&label);
        for stmt in &bb.statements {
            codegen_statement(emitter, mir, stmt, interner, hir);
        }
        codegen_terminator(
            emitter,
            mir,
            &bb.terminator,
            &ret_ty,
            fn_name_by_def_id,
            interner,
            hir,
        );
    }

    emitter.emit_function_end();
}

/// Generate code for a single MIR statement.
fn codegen_statement(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    stmt: &Statement,
    interner: &Rodeo,
    hir: &HirCrate,
) {
    match &stmt.kind {
        StatementKind::Assign(boxed) => {
            let (place, rvalue) = &**boxed;
            let val = codegen_rvalue(emitter, mir, rvalue, interner, hir);
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
                    // Stage 3.30: use _with_hir so ADT-typed locals get
                    // their full struct type for the store.
                    let ty = mir_type_to_emit_type_with_hir(local_ty, hir);
                    emitter.set_local(id.0, val.clone());
                    // Stage 3.27: skip store for void-typed locals — LLVM
                    // doesn't allow `store void`. The value is dead anyway.
                    if ty != EmitType::Void {
                        if let Some(ptr) = emitter.get_local_ptr(id.0).cloned() {
                            emitter.emit_store(&ty, &val, &ptr);
                        }
                    }
                }
                LvalueKind::Projection(base, elem) => {
                    let ty = detect_operand_type(mir, &Operand::Copy(place.clone()), hir)
                        .unwrap_or(EmitType::I32);
                    match elem {
                        ProjectionElem::Deref => {
                            let ptr_val = codegen_lvalue_load(emitter, base, interner, hir);
                            emitter.emit_store(&ty, &val, &ptr_val);
                        }
                        ProjectionElem::Field(field_id, _) => {
                            let base_ptr = if let LvalueKind::Local(id) = &base.kind {
                                emitter
                                    .get_local_ptr(id.0)
                                    .cloned()
                                    .unwrap_or_else(|| "0".to_string())
                            } else {
                                codegen_lvalue_load(emitter, base, interner, hir)
                            };
                            let struct_ty = detect_lvalue_storage_type_with_hir(mir, base, hir);
                            let field_ptr =
                                emitter.emit_gep_field(&base_ptr, &struct_ty, field_id.0);
                            emitter.emit_store(&ty, &val, &field_ptr);
                        }
                        ProjectionElem::Index(idx) => {
                            let base_ptr = if let LvalueKind::Local(id) = &base.kind {
                                emitter
                                    .get_local_ptr(id.0)
                                    .cloned()
                                    .unwrap_or_else(|| "0".to_string())
                            } else {
                                codegen_lvalue_load(emitter, base, interner, hir)
                            };
                            let array_ty = detect_lvalue_storage_type_with_hir(mir, base, hir);
                            let idx_val = if let Some(v) = emitter.get_local(idx.0).cloned() {
                                v
                            } else if let Some(ptr) = emitter.get_local_ptr(idx.0).cloned() {
                                emitter.emit_load(&EmitType::I32, &ptr)
                            } else {
                                "0".to_string()
                            };
                            let elem_ptr = emitter.emit_gep_index(&base_ptr, &array_ty, &idx_val);
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

/// Generate code for an rvalue.
fn codegen_rvalue(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    rv: &Rvalue,
    interner: &Rodeo,
    hir: &HirCrate,
) -> EmitValue {
    match rv {
        Rvalue::Use(op) => codegen_operand(emitter, mir, op, interner, hir),

        Rvalue::BinaryOp(op, a, b) => {
            let a_val = codegen_operand(emitter, mir, a, interner, hir);
            let b_val = codegen_operand(emitter, mir, b, interner, hir);
            let ty = detect_operand_type(mir, a, hir)
                .or(detect_operand_type(mir, b, hir))
                .unwrap_or(EmitType::I32);
            match op {
                BinOp::Eq => {
                    let cmp = if ty == EmitType::F64 || ty == EmitType::F32 {
                        emitter.emit_fcmp("oeq", &ty, &a_val, &b_val)
                    } else {
                        emitter.emit_icmp("eq", &ty, &a_val, &b_val)
                    };
                    emitter.emit_zext(&EmitType::I1, &EmitType::I32, &cmp)
                }
                BinOp::Ne => {
                    let cmp = if ty == EmitType::F64 || ty == EmitType::F32 {
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
                _ => emitter.emit_binop(*op, &ty, &a_val, &b_val),
            }
        }

        Rvalue::UnaryOp(op, operand) => {
            let val = codegen_operand(emitter, mir, operand, interner, hir);
            let ty = detect_operand_type(mir, operand, hir).unwrap_or(EmitType::I32);
            emitter.emit_unop(*op, &ty, &val)
        }

        Rvalue::Ref(_, _borrow_kind, lv) => {
            // &x → return the alloca pointer of x (the address)
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
                codegen_operand(emitter, mir, &operands[0], interner, hir)
            } else {
                let field_tys: Vec<EmitType> = operands
                    .iter()
                    .map(|op| detect_operand_type(mir, op, hir).unwrap_or(EmitType::I32))
                    .collect();
                let agg_ty = EmitType::Struct(field_tys.clone());
                let mut agg = "undef".to_string();
                for (i, op) in operands.iter().enumerate() {
                    let val = codegen_operand(emitter, mir, op, interner, hir);
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
                let val = codegen_operand(emitter, mir, op, interner, hir);
                agg = emitter.emit_insertvalue(&agg_ty, &agg, &elem_emit_ty, &val, i as u32);
            }
            agg
        }

        // Stage 3.30: ADT construction (struct/enum ctor).
        // Per §16 阶段间接口隔离: field types are sunk into
        // `AggregateKind::Adt(def_id, variant, substs, field_tys)` by MIR
        // lower. Codegen reads them from MIR — does NOT re-query HIR or
        // call `lower_hir_ty_to_mir_ty` (which would be a cross-stage
        // internal-API call).
        Rvalue::Aggregate(AggregateKind::Adt(_def_id, _variant, _substs, field_tys), operands) => {
            if operands.is_empty() {
                return "0".to_string();
            }
            // Use the field types from MIR (sunk by MIR lower per §16).
            // Fallback to operand inference if MIR lower couldn't resolve
            // them (e.g., enum variants not yet supported).
            let field_tys: Vec<EmitType> = if field_tys.is_empty() {
                operands
                    .iter()
                    .map(|op| detect_operand_type(mir, op, hir).unwrap_or(EmitType::I32))
                    .collect()
            } else {
                field_tys.iter().map(mir_type_to_emit_type).collect()
            };
            let agg_ty = EmitType::Struct(field_tys.clone());
            let mut agg = "undef".to_string();
            for (i, op) in operands.iter().enumerate() {
                let val = codegen_operand(emitter, mir, op, interner, hir);
                let val_ty = field_tys.get(i).cloned().unwrap_or(EmitType::I32);
                agg = emitter.emit_insertvalue(&agg_ty, &agg, &val_ty, &val, i as u32);
            }
            agg
        }

        Rvalue::Cast(_, op, target_ty) => {
            let val = codegen_operand(emitter, mir, op, interner, hir);
            let src_ty = detect_operand_type(mir, op, hir).unwrap_or(EmitType::I32);
            let dst_ty = mir_type_to_emit_type(target_ty);
            emitter.emit_cast(&src_ty, &dst_ty, &val)
        }

        _ => "0".to_string(),
    }
}

/// Generate code for an operand.
fn codegen_operand(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    op: &Operand,
    interner: &Rodeo,
    hir: &HirCrate,
) -> EmitValue {
    match op {
        Operand::Constant(c) => match c.val {
            // Stage 3.27: string literals → module-level global + GEP to i8*.
            // The Str symbol is looked up in the interner to get the bytes.
            // The returned value is an i8* pointing at the first byte of the
            // global. Full &str (ptr+len) fat-pointer representation is
            // deferred — Stage 3.27 just gives the pointer.
            ConstVal::Str(sym) => {
                let bytes = interner
                    .try_resolve(&sym)
                    .map(|s| s.as_bytes())
                    .unwrap_or(b"\0");
                let global_name = emitter.emit_string_global(bytes);
                // Emit `getelementptr inbounds [N x i8], [N x i8]* @.str.N, i32 0, i32 0`
                // to get an i8*. We don't know N from here, but LLVM accepts
                // `[N x i8]` only if N matches the global's actual length.
                // The TextEmitter's emit_string_global stored the length, but
                // the Emitter trait doesn't expose it back. As a workaround,
                // we emit a typed GEP using the actual byte count.
                //
                // To keep the Emitter trait clean, we return the global name
                // directly and let the TextEmitter's emit_string_global emit
                // the GEP instruction as part of the value (deferred). For
                // now, return the name and the codegen layer treats it as
                // pointer-typed. When stored/loaded the type is `Ptr(I8)`.
                let n = bytes.len();
                // Synthesize a GEP value (text-only — this matches what a
                // real LLVM frontend emits for `&str` literals' pointer part).
                format!(
                    "getelementptr inbounds ([{} x i8], [{} x i8]* @{}, i32 0, i32 0)",
                    n, n, global_name
                )
            }
            _ => emitter.emit_const(&c.val),
        },
        Operand::Copy(lv) | Operand::Move(lv) => {
            let ty = detect_lvalue_type(mir, lv, hir);
            codegen_lvalue_load_typed(emitter, mir, lv, ty, interner, hir)
        }
    }
}

/// Detect the EmitType of an lvalue by examining its source.
#[allow(clippy::only_used_in_recursion)]
fn detect_lvalue_type(mir: &MirBody, lv: &Lvalue, hir: &HirCrate) -> EmitType {
    match &lv.kind {
        LvalueKind::Local(id) => mir
            .local_decls
            .get(id.0 as usize)
            .map(|ld| mir_type_to_emit_type_with_hir(&ld.ty, hir))
            .unwrap_or(EmitType::I32),
        LvalueKind::Projection(base, elem) => match elem {
            ProjectionElem::Deref => {
                let base_ty = detect_lvalue_type(mir, base, hir);
                if base_ty.is_ptr() {
                    base_ty.pointee()
                } else {
                    base_ty
                }
            }
            ProjectionElem::Field(_, field_ty) => mir_type_to_emit_type_with_hir(field_ty, hir),
            ProjectionElem::Index(_) | ProjectionElem::ConstantIndex { .. } => {
                // Element type: detect from the array's element type via storage.
                let storage = detect_lvalue_storage_type_with_hir(mir, base, hir);
                match storage {
                    EmitType::Array(elem, _) => *elem,
                    _ => EmitType::I32,
                }
            }
            _ => EmitType::I32,
        },
        LvalueKind::Static(_) => EmitType::I32,
    }
}

/// Map a MIR Ty to an EmitType, resolving `TyKind::Adt` via HIR lookup.
///
/// Stage 3.30 (per §16): `mir_type_to_emit_type` (in emitter.rs) doesn't have
/// access to HIR, so it falls back to `I32` for `TyKind::Adt`. This helper
/// takes a HIR crate reference and resolves ADTs to their struct field types,
/// producing correct `{ i32, i64, ... }` LLVM struct types.
///
/// §16 compliance note: this function reads HIR (allowed per §16.2.1 — reading
/// upstream data structures) but does NOT call `crate::mir::lower::`
/// functions. The HirTy → EmitType conversion is done locally via
/// `hir_ty_to_emit_type` to avoid cross-stage internal-API calls.
/// Marked L-PIPE-1: the root-cause fix would be to sink field types into
/// `TyKind::Adt` itself (or have typeck write them back to `LocalDecl.ty`),
/// eliminating the HIR lookup entirely.
pub fn mir_type_to_emit_type_with_hir(ty: &crate::mir::ty::Ty, hir: &HirCrate) -> EmitType {
    use crate::mir::ty::TyKind;
    match &ty.kind {
        TyKind::Adt(def_id, _substs) => match hir.owner(*def_id) {
            Some(crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s))) => {
                let field_tys: Vec<EmitType> = s
                    .fields
                    .iter()
                    .map(|f| hir_ty_to_emit_type(&f.ty, hir))
                    .collect();
                if field_tys.is_empty() {
                    EmitType::Void
                } else {
                    EmitType::Struct(field_tys)
                }
            }
            // Stage 3.38 (L-ENUM): Enum type → { i32 discriminant, <payload> }.
            // The payload is the field types of the first non-unit variant.
            // For unit-only enums, the type is just { i32 }.
            // This is a simplification — a proper implementation would use
            // a union of all variant payloads (L-ENUM-UNION).
            Some(crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(e))) => {
                let mut field_tys = vec![EmitType::I32]; // discriminant
                                                         // Find the first variant with payload fields.
                for variant in &e.variants {
                    let payload: Vec<EmitType> = match &variant.data {
                        crate::hir::HirVariantData::Unit(_) => vec![],
                        crate::hir::HirVariantData::Tuple(fields, _) => fields
                            .iter()
                            .map(|f| hir_ty_to_emit_type(&f.ty, hir))
                            .collect(),
                        crate::hir::HirVariantData::Struct(fields, _) => fields
                            .iter()
                            .map(|f| hir_ty_to_emit_type(&f.ty, hir))
                            .collect(),
                    };
                    if !payload.is_empty() {
                        field_tys.extend(payload);
                        break;
                    }
                }
                EmitType::Struct(field_tys)
            }
            _ => EmitType::I32,
        },
        // For non-Adt types, delegate to the HIR-agnostic version.
        _ => mir_type_to_emit_type(ty),
    }
}

/// Map a HIR Ty directly to an EmitType (local to codegen, no MIR lower call).
///
/// Stage 3.30 (per §16): this is the codegen-local HirTy → EmitType
/// conversion. It mirrors `lower_hir_ty_to_mir_ty` + `mir_type_to_emit_type`
/// but does so without calling into MIR lower (which would be a cross-stage
/// internal-API call per §16.3).
fn hir_ty_to_emit_type(ty: &crate::hir::HirTy, hir: &HirCrate) -> EmitType {
    use crate::hir::HirTyKind;
    match &ty.kind {
        HirTyKind::Bool => EmitType::I1,
        HirTyKind::Char => EmitType::I8,
        HirTyKind::Int(crate::ast::IntTy::I8) | HirTyKind::Uint(crate::ast::UintTy::U8) => {
            EmitType::I8
        }
        HirTyKind::Int(crate::ast::IntTy::I64) | HirTyKind::Uint(crate::ast::UintTy::U64) => {
            EmitType::I64
        }
        HirTyKind::Int(_) | HirTyKind::Uint(_) => EmitType::I32,
        HirTyKind::Float(crate::ast::FloatTy::F32) => EmitType::F32,
        HirTyKind::Float(_) => EmitType::F64,
        HirTyKind::Tuple(tys) => {
            if tys.is_empty() {
                EmitType::Void
            } else {
                EmitType::Struct(tys.iter().map(|t| hir_ty_to_emit_type(t, hir)).collect())
            }
        }
        HirTyKind::Array(elem, _) => {
            // Array length not needed for EmitType::Array if we use a
            // placeholder; but emit_type_to_llvm_str needs the length.
            // For now, fall back to the element type (caller should handle
            // arrays via mir_type_to_emit_type which has the length).
            hir_ty_to_emit_type(elem, hir)
        }
        HirTyKind::Slice(elem) => EmitType::ptr_to(hir_ty_to_emit_type(elem, hir)),
        HirTyKind::Ref(_, _, inner) | HirTyKind::Ptr(_, inner) => {
            EmitType::ptr_to(hir_ty_to_emit_type(inner, hir))
        }
        HirTyKind::Path(_, path) => {
            // Resolve path to a struct DefId and recurse via
            // mir_type_to_emit_type_with_hir.
            if let crate::hir::Res::Def(def_id, _) = path.res {
                let mir_ty = crate::mir::ty::Ty::new(
                    crate::mir::ty::TyKind::Adt(def_id, Vec::new()),
                    ty.span,
                );
                return mir_type_to_emit_type_with_hir(&mir_ty, hir);
            }
            EmitType::I32
        }
        _ => EmitType::I32,
    }
}

/// Detect the *storage* type of an lvalue, resolving ADTs via HIR.
/// Used to render GEP base types correctly for struct field access.
fn detect_lvalue_storage_type_with_hir(mir: &MirBody, lv: &Lvalue, hir: &HirCrate) -> EmitType {
    match &lv.kind {
        LvalueKind::Local(id) => mir
            .local_decls
            .get(id.0 as usize)
            .map(|ld| mir_type_to_emit_type_with_hir(&ld.ty, hir))
            .unwrap_or(EmitType::I32),
        LvalueKind::Projection(base, _) => detect_lvalue_storage_type_with_hir(mir, base, hir),
        LvalueKind::Static(_) => EmitType::I32,
    }
}

/// Load a value from an lvalue with a known type.
#[allow(clippy::only_used_in_recursion)]
fn codegen_lvalue_load_typed(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    lv: &Lvalue,
    ty: EmitType,
    interner: &Rodeo,
    hir: &HirCrate,
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
                let ptr_ty = detect_lvalue_type(mir, base, hir);
                let ptr_val =
                    codegen_lvalue_load_typed(emitter, mir, base, ptr_ty.clone(), interner, hir);
                emitter.emit_load(&ty, &ptr_val)
            }
            ProjectionElem::Field(field_id, _) => {
                let base_ptr = if let LvalueKind::Local(id) = &base.kind {
                    emitter
                        .get_local_ptr(id.0)
                        .cloned()
                        .unwrap_or_else(|| "0".to_string())
                } else {
                    let ptr_ty = detect_lvalue_type(mir, base, hir);
                    codegen_lvalue_load_typed(emitter, mir, base, ptr_ty, interner, hir)
                };
                let struct_ty = detect_lvalue_storage_type_with_hir(mir, base, hir);
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
                    let ptr_ty = detect_lvalue_type(mir, base, hir);
                    codegen_lvalue_load_typed(emitter, mir, base, ptr_ty, interner, hir)
                };
                let array_ty = detect_lvalue_storage_type_with_hir(mir, base, hir);
                let idx_val = if let Some(v) = emitter.get_local(idx.0).cloned() {
                    v
                } else if let Some(ptr) = emitter.get_local_ptr(idx.0).cloned() {
                    emitter.emit_load(&EmitType::I32, &ptr)
                } else {
                    "0".to_string()
                };
                let elem_ptr = emitter.emit_gep_index(&base_ptr, &array_ty, &idx_val);
                emitter.emit_load(&ty, &elem_ptr)
            }
            ProjectionElem::ConstantIndex { offset, .. } => {
                let base_ptr = if let LvalueKind::Local(id) = &base.kind {
                    emitter
                        .get_local_ptr(id.0)
                        .cloned()
                        .unwrap_or_else(|| "0".to_string())
                } else {
                    let ptr_ty = detect_lvalue_type(mir, base, hir);
                    codegen_lvalue_load_typed(emitter, mir, base, ptr_ty, interner, hir)
                };
                let array_ty = detect_lvalue_storage_type_with_hir(mir, base, hir);
                let elem_ptr = emitter.emit_gep_index(&base_ptr, &array_ty, &offset.to_string());
                emitter.emit_load(&ty, &elem_ptr)
            }
            _ => "0".to_string(),
        },
        LvalueKind::Static(_) => "0".to_string(),
    }
}

/// Load a value from an lvalue (legacy, uses I32 default).
fn codegen_lvalue_load(
    emitter: &mut dyn Emitter,
    lv: &Lvalue,
    interner: &Rodeo,
    hir: &HirCrate,
) -> EmitValue {
    codegen_lvalue_load_typed(
        emitter,
        &MirBody::new(crate::session::Span::DUMMY),
        lv,
        EmitType::I32,
        interner,
        hir,
    )
}

/// Generate code for a terminator.
fn codegen_terminator(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    term: &Terminator,
    ret_ty: &EmitType,
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
    interner: &Rodeo,
    hir: &HirCrate,
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
            let discr_val = codegen_operand(emitter, mir, discr, interner, hir);
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
                let discr_ty = detect_operand_type(mir, discr, hir).unwrap_or(EmitType::I32);
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
            // Stage 3.30 fix: resolve fn name by DefId via the DefId→name map
            // (was: indexed by body index, which was wrong when struct/enum
            // owners created gaps in the DefId space).
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
                    ConstVal::Uint(n) => {
                        let def_id = crate::hir::DefId(*n as u32);
                        fn_name_by_def_id.get(&def_id).cloned()
                    }
                    ConstVal::Int(n) => {
                        let def_id = crate::hir::DefId(*n as u32);
                        fn_name_by_def_id.get(&def_id).cloned()
                    }
                    _ => None,
                }
            } else {
                None
            };

            // Build (type, value) pairs for call args.
            let arg_pairs: Vec<(EmitType, EmitValue)> = args
                .iter()
                .map(|a| {
                    let ty = detect_operand_type(mir, a, hir).unwrap_or(EmitType::I32);
                    let val = codegen_operand(emitter, mir, a, interner, hir);
                    (ty, val)
                })
                .collect();
            let arg_refs: Vec<(EmitType, &EmitValue)> =
                arg_pairs.iter().map(|(t, v)| (t.clone(), v)).collect();

            let ret_val = if let Some(ref name) = fn_name {
                let call_ret_ty = if let LvalueKind::Local(id) = &destination.kind {
                    mir.local_decls
                        .get(id.0 as usize)
                        .map(|ld| mir_type_to_emit_type_with_hir(&ld.ty, hir))
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
                    .map(|ld| mir_type_to_emit_type_with_hir(&ld.ty, hir))
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
            // Stage 3.24: real overflow check via llvm.{sadd,ssub,smul}.with.overflow.
            // The MIR's `cond` field is a placeholder (Bool(true)) — for Overflow
            // messages we ignore it and compute the real overflow flag from the
            // lhs/rhs operands stored in the message.
            let panic_label = format!("panic_assert_{}", target.0);
            match msg {
                crate::mir::body::AssertMessage::Overflow(op, lhs, rhs) => {
                    // Detect operand type (must be int — only ints overflow-check).
                    let op_ty = detect_operand_type(mir, lhs, hir)
                        .or(detect_operand_type(mir, rhs, hir))
                        .unwrap_or(EmitType::I32);
                    let lhs_val = codegen_operand(emitter, mir, lhs, interner, hir);
                    let rhs_val = codegen_operand(emitter, mir, rhs, interner, hir);
                    let agg = emitter.emit_checked_binop(*op, &op_ty, &lhs_val, &rhs_val);
                    let agg_ty = EmitType::struct_of(vec![op_ty.clone(), EmitType::I1]);
                    let overflow_flag = emitter.emit_extractvalue(&agg_ty, &agg, 1);
                    // If overflow_flag is FALSE (no overflow), branch to target.
                    // If TRUE (overflow), branch to panic block.
                    // i.e. `br i1 (xor overflow_flag, true), label %target, label %panic`
                    let inverted = emitter.emit_unop(
                        crate::mir::lvalue::UnOp::Not,
                        &EmitType::I1,
                        &overflow_flag,
                    );
                    emitter.emit_br_cond(&inverted, &format!("bb{}", target.0), &panic_label);
                }
                crate::mir::body::AssertMessage::DivisionByZero(rhs) => {
                    // Stage 3.25: emit `icmp eq rhs, 0`; if true → panic,
                    // if false → continue to target.
                    let rhs_val = codegen_operand(emitter, mir, rhs, interner, hir);
                    let rhs_ty = detect_operand_type(mir, rhs, hir).unwrap_or(EmitType::I32);
                    let is_zero = emitter.emit_icmp("eq", &rhs_ty, &rhs_val, &"0".to_string());
                    // is_zero == true means divisor is zero → panic.
                    // br i1 is_zero, label %panic, label %target
                    emitter.emit_br_cond(&is_zero, &panic_label, &format!("bb{}", target.0));
                }
                crate::mir::body::AssertMessage::BoundsCheck => {
                    let cond_val = codegen_operand(emitter, mir, cond, interner, hir);
                    emitter.emit_br_cond(&cond_val, &format!("bb{}", target.0), &panic_label);
                }
            }
            // Emit the panic block (unconditional — only reached on overflow).
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
                    let zero_str = "0".to_string();
                    let zero_str2 = "0".to_string();
                    let _ = emitter.emit_call(
                        "__landin_panic_overflow",
                        &[
                            (EmitType::I32, &op_str),
                            (EmitType::I32, &zero_str),
                            (EmitType::I32, &zero_str2),
                        ],
                        &EmitType::Void,
                    );
                }
                crate::mir::body::AssertMessage::DivisionByZero(_) => {
                    let _ = emitter.emit_call("__landin_panic_div_by_zero", &[], &EmitType::Void);
                }
                crate::mir::body::AssertMessage::BoundsCheck => {
                    let zero_str = "0".to_string();
                    let zero_str2 = "0".to_string();
                    let _ = emitter.emit_call(
                        "__landin_panic_bounds_check",
                        &[(EmitType::I64, &zero_str), (EmitType::I64, &zero_str2)],
                        &EmitType::Void,
                    );
                }
            }
            emitter.emit_unreachable();
        }

        Terminator::Drop { place, target, .. } => {
            // Drop: primitives don't need drop glue — just branch.
            // Full drop glue generation is a future stage.
            let _ = place;
            emitter.emit_br(&format!("bb{}", target.0));
        }
    }
}

/// Detect the EmitType of an operand by examining its source.
fn detect_operand_type(mir: &MirBody, op: &Operand, hir: &HirCrate) -> Option<EmitType> {
    match op {
        Operand::Constant(c) => match &c.val {
            ConstVal::Float(_) => Some(EmitType::F64),
            ConstVal::Bool(_) => Some(EmitType::I1),
            ConstVal::Char(_) => Some(EmitType::I8),
            _ => Some(EmitType::I32),
        },
        Operand::Copy(lv) | Operand::Move(lv) => {
            if let LvalueKind::Local(id) = &lv.kind {
                mir.local_decls
                    .get(id.0 as usize)
                    .map(|ld| mir_type_to_emit_type_with_hir(&ld.ty, hir))
            } else {
                Some(detect_lvalue_type(mir, lv, hir))
            }
        }
    }
}
