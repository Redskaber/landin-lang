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
    emitter.output().to_string()
}

/// Generate LLVM IR for a crate using any Emitter backend.
pub fn codegen_crate_with_emitter(hir: &HirCrate, interner: &Rodeo, emitter: &mut dyn Emitter) {
    // Collect function names from HIR (use actual fn names, not fn_0/fn_1)
    let fn_names: Vec<String> = hir
        .bodies
        .iter()
        .enumerate()
        .map(|(i, _)| {
            for (def_id, owner) in &hir.owners {
                if def_id.as_u32() as usize == i {
                    if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Fn(f)) = owner {
                        let name = interner.try_resolve(&f.ident.name).unwrap_or("fn");
                        return format!("landin_{}", name);
                    }
                }
            }
            format!("fn_{}", i)
        })
        .collect();

    for (idx, (_, body)) in hir.bodies.iter().enumerate() {
        let fn_name = fn_names[idx].clone();
        let return_ty = crate::driver::owner_return_ty_for_body(hir, body);
        let (mut mir, unify) =
            crate::mir::lower::lower_hir_body_to_mir_full(body, interner, return_ty);
        let mut tc = crate::typeck::TypeChecker::with_unify(unify);
        tc.populate_fn_sigs(hir);
        tc.check_mir_body(&mut mir);
        codegen_function(emitter, &fn_name, &mir, &fn_names, body.params.len());
    }
}

/// Generate LLVM IR for a single function.
fn codegen_function(
    emitter: &mut dyn Emitter,
    name: &str,
    mir: &MirBody,
    fn_names: &[String],
    param_count: usize,
) {
    // Determine return type. Unit type (empty tuple) → void return.
    let ret_ty = if mir.local_decls.is_empty() {
        EmitType::Void
    } else {
        match &mir.local_decls[0].ty.kind {
            crate::mir::ty::TyKind::Tuple(tys) if tys.is_empty() => EmitType::Void,
            _ => mir_type_to_emit_type(&mir.local_decls[0].ty),
        }
    };

    // Build param list: LocalId(1)..LocalId(param_count) are fn params.
    let params: Vec<(EmitType, String)> = (0..param_count)
        .map(|i| {
            let local_idx = i + 1;
            let ty = mir
                .local_decls
                .get(local_idx)
                .map(|ld| mir_type_to_emit_type(&ld.ty))
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
    for (i, ld) in mir.local_decls.iter().enumerate() {
        let ty = mir_type_to_emit_type(&ld.ty);
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
            codegen_statement(emitter, mir, stmt);
        }
        codegen_terminator(emitter, mir, &bb.terminator, &ret_ty, fn_names);
    }

    emitter.emit_function_end();
}

/// Generate code for a single MIR statement.
fn codegen_statement(emitter: &mut dyn Emitter, mir: &MirBody, stmt: &Statement) {
    match &stmt.kind {
        StatementKind::Assign(boxed) => {
            let (place, rvalue) = &**boxed;
            let val = codegen_rvalue(emitter, mir, rvalue);
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
                    let ty = mir_type_to_emit_type(local_ty);
                    emitter.set_local(id.0, val.clone());
                    if let Some(ptr) = emitter.get_local_ptr(id.0).cloned() {
                        emitter.emit_store(&ty, &val, &ptr);
                    }
                }
                LvalueKind::Projection(base, elem) => {
                    let ty = detect_operand_type(mir, &Operand::Copy(place.clone()))
                        .unwrap_or(EmitType::I32);
                    match elem {
                        ProjectionElem::Deref => {
                            let ptr_val = codegen_lvalue_load(emitter, base);
                            emitter.emit_store(&ty, &val, &ptr_val);
                        }
                        ProjectionElem::Field(field_id, _) => {
                            let base_ptr = if let LvalueKind::Local(id) = &base.kind {
                                emitter
                                    .get_local_ptr(id.0)
                                    .cloned()
                                    .unwrap_or_else(|| "0".to_string())
                            } else {
                                codegen_lvalue_load(emitter, base)
                            };
                            let struct_ty = detect_lvalue_storage_type(mir, base);
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
                                codegen_lvalue_load(emitter, base)
                            };
                            let array_ty = detect_lvalue_storage_type(mir, base);
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
fn codegen_rvalue(emitter: &mut dyn Emitter, mir: &MirBody, rv: &Rvalue) -> EmitValue {
    match rv {
        Rvalue::Use(op) => codegen_operand(emitter, mir, op),

        Rvalue::BinaryOp(op, a, b) => {
            let a_val = codegen_operand(emitter, mir, a);
            let b_val = codegen_operand(emitter, mir, b);
            let ty = detect_operand_type(mir, a)
                .or(detect_operand_type(mir, b))
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
            let val = codegen_operand(emitter, mir, operand);
            let ty = detect_operand_type(mir, operand).unwrap_or(EmitType::I32);
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
                codegen_operand(emitter, mir, &operands[0])
            } else {
                let field_tys: Vec<EmitType> = operands
                    .iter()
                    .map(|op| detect_operand_type(mir, op).unwrap_or(EmitType::I32))
                    .collect();
                let agg_ty = EmitType::Struct(field_tys.clone());
                let mut agg = "undef".to_string();
                for (i, op) in operands.iter().enumerate() {
                    let val = codegen_operand(emitter, mir, op);
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
                let val = codegen_operand(emitter, mir, op);
                agg = emitter.emit_insertvalue(&agg_ty, &agg, &elem_emit_ty, &val, i as u32);
            }
            agg
        }

        Rvalue::Cast(_, op, target_ty) => {
            let val = codegen_operand(emitter, mir, op);
            let src_ty = detect_operand_type(mir, op).unwrap_or(EmitType::I32);
            let dst_ty = mir_type_to_emit_type(target_ty);
            emitter.emit_cast(&src_ty, &dst_ty, &val)
        }

        _ => "0".to_string(),
    }
}

/// Generate code for an operand.
fn codegen_operand(emitter: &mut dyn Emitter, mir: &MirBody, op: &Operand) -> EmitValue {
    match op {
        Operand::Constant(c) => emitter.emit_const(&c.val),
        Operand::Copy(lv) | Operand::Move(lv) => {
            let ty = detect_lvalue_type(mir, lv);
            codegen_lvalue_load_typed(emitter, mir, lv, ty)
        }
    }
}

/// Detect the EmitType of an lvalue by examining its source.
#[allow(clippy::only_used_in_recursion)]
fn detect_lvalue_type(mir: &MirBody, lv: &Lvalue) -> EmitType {
    match &lv.kind {
        LvalueKind::Local(id) => mir
            .local_decls
            .get(id.0 as usize)
            .map(|ld| mir_type_to_emit_type(&ld.ty))
            .unwrap_or(EmitType::I32),
        LvalueKind::Projection(base, elem) => match elem {
            ProjectionElem::Deref => {
                let base_ty = detect_lvalue_type(mir, base);
                if base_ty.is_ptr() {
                    base_ty.pointee()
                } else {
                    base_ty
                }
            }
            ProjectionElem::Field(_, field_ty) => mir_type_to_emit_type(field_ty),
            ProjectionElem::Index(_) | ProjectionElem::ConstantIndex { .. } => {
                // Element type: detect from the array's element type via storage.
                let storage = detect_lvalue_storage_type(mir, base);
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

/// Detect the *storage* type of an lvalue — i.e., the type of the alloca
/// it ultimately reads from. Used to render GEP base types correctly.
fn detect_lvalue_storage_type(mir: &MirBody, lv: &Lvalue) -> EmitType {
    match &lv.kind {
        LvalueKind::Local(id) => mir
            .local_decls
            .get(id.0 as usize)
            .map(|ld| mir_type_to_emit_type(&ld.ty))
            .unwrap_or(EmitType::I32),
        LvalueKind::Projection(base, _) => detect_lvalue_storage_type(mir, base),
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
                let ptr_ty = detect_lvalue_type(mir, base);
                let ptr_val = codegen_lvalue_load_typed(emitter, mir, base, ptr_ty.clone());
                emitter.emit_load(&ty, &ptr_val)
            }
            ProjectionElem::Field(field_id, _) => {
                let base_ptr = if let LvalueKind::Local(id) = &base.kind {
                    emitter
                        .get_local_ptr(id.0)
                        .cloned()
                        .unwrap_or_else(|| "0".to_string())
                } else {
                    let ptr_ty = detect_lvalue_type(mir, base);
                    codegen_lvalue_load_typed(emitter, mir, base, ptr_ty)
                };
                let struct_ty = detect_lvalue_storage_type(mir, base);
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
                    let ptr_ty = detect_lvalue_type(mir, base);
                    codegen_lvalue_load_typed(emitter, mir, base, ptr_ty)
                };
                let array_ty = detect_lvalue_storage_type(mir, base);
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
                    let ptr_ty = detect_lvalue_type(mir, base);
                    codegen_lvalue_load_typed(emitter, mir, base, ptr_ty)
                };
                let array_ty = detect_lvalue_storage_type(mir, base);
                let elem_ptr = emitter.emit_gep_index(&base_ptr, &array_ty, &offset.to_string());
                emitter.emit_load(&ty, &elem_ptr)
            }
            _ => "0".to_string(),
        },
        LvalueKind::Static(_) => "0".to_string(),
    }
}

/// Load a value from an lvalue (legacy, uses I32 default).
fn codegen_lvalue_load(emitter: &mut dyn Emitter, lv: &Lvalue) -> EmitValue {
    codegen_lvalue_load_typed(
        emitter,
        &MirBody::new(crate::session::Span::DUMMY),
        lv,
        EmitType::I32,
    )
}

/// Generate code for a terminator.
fn codegen_terminator(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    term: &Terminator,
    ret_ty: &EmitType,
    fn_names: &[String],
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
            let discr_val = codegen_operand(emitter, mir, discr);
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
                let discr_ty = detect_operand_type(mir, discr).unwrap_or(EmitType::I32);
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
                            let idx = def_id.as_u32() as usize;
                            fn_names.get(idx).cloned()
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
                        let idx = *n as usize;
                        fn_names.get(idx).cloned()
                    }
                    ConstVal::Int(n) => {
                        let idx = *n as usize;
                        fn_names.get(idx).cloned()
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
                    let ty = detect_operand_type(mir, a).unwrap_or(EmitType::I32);
                    let val = codegen_operand(emitter, mir, a);
                    (ty, val)
                })
                .collect();
            let arg_refs: Vec<(EmitType, &EmitValue)> =
                arg_pairs.iter().map(|(t, v)| (t.clone(), v)).collect();

            let ret_val = if let Some(ref name) = fn_name {
                let call_ret_ty = if let LvalueKind::Local(id) = &destination.kind {
                    mir.local_decls
                        .get(id.0 as usize)
                        .map(|ld| mir_type_to_emit_type(&ld.ty))
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
                    .map(|ld| mir_type_to_emit_type(&ld.ty))
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
            let cond_val = codegen_operand(emitter, mir, cond);
            let panic_label = format!("panic_assert_{}", target.0);
            emitter.emit_br_cond(&cond_val, &format!("bb{}", target.0), &panic_label);
            emitter.emit_block(&panic_label);
            match msg {
                crate::mir::body::AssertMessage::Overflow(op) => {
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
                crate::mir::body::AssertMessage::DivisionByZero => {
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
fn detect_operand_type(mir: &MirBody, op: &Operand) -> Option<EmitType> {
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
                    .map(|ld| mir_type_to_emit_type(&ld.ty))
            } else {
                Some(detect_lvalue_type(mir, lv))
            }
        }
    }
}
