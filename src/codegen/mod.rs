//! LLVM IR codegen: MIR → LLVM IR via Emitter trait.
//!
//! Stage 3.2: adds variable allocation (alloca/store/load), control flow
//! (SwitchInt → br/switch), and function calls (Call → call).

pub mod emitter;
pub mod text_emitter;

pub use emitter::{EmitType, EmitValue, Emitter};
pub use text_emitter::TextEmitter;

use crate::hir::HirCrate;
use crate::mir::body::*;
use crate::mir::lvalue::*;
use crate::mir::ty::ConstVal;
use lasso::Rodeo;

/// Generate LLVM IR text for a crate.
pub fn codegen_crate(hir: &HirCrate, interner: &Rodeo) -> String {
    let mut emitter = TextEmitter::new();
    // Emit LLVM module header
    emitter.emit_header();
    codegen_crate_with_emitter(hir, interner, &mut emitter);
    emitter.output().to_string()
}

/// Generate LLVM IR for a crate using any Emitter backend.
pub fn codegen_crate_with_emitter(hir: &HirCrate, interner: &Rodeo, emitter: &mut dyn Emitter) {
    // Collect function names first (for call resolution)
    let fn_names: Vec<String> = hir
        .bodies
        .iter()
        .enumerate()
        .map(|(i, _)| format!("fn_{}", i))
        .collect();

    for (idx, (_, body)) in hir.bodies.iter().enumerate() {
        let fn_name = format!("fn_{}", idx);
        let return_ty = crate::driver::owner_return_ty_for_body(hir, body);
        let param_count = body.params.len();
        let (mut mir, unify) =
            crate::mir::lower::lower_hir_body_to_mir_full(body, interner, return_ty);
        let mut tc = crate::typeck::TypeChecker::with_unify(unify);
        tc.populate_fn_sigs(hir);
        tc.check_mir_body(&mut mir);
        codegen_function(emitter, &fn_name, &mir, &fn_names, param_count);
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
    // Determine return type
    let ret_ty = if mir.local_decls.is_empty() {
        EmitType::I32
    } else {
        emitter::mir_type_to_emit_type(&mir.local_decls[0].ty)
    };

    // Build param list: LocalId(1)..LocalId(param_count) are fn params.
    let params: Vec<(EmitType, String)> = (0..param_count)
        .map(|i| {
            let local_idx = i + 1;
            let ty = mir
                .local_decls
                .get(local_idx)
                .map(|ld| emitter::mir_type_to_emit_type(&ld.ty))
                .unwrap_or(EmitType::I32);
            (ty, format!("%arg{}", i))
        })
        .collect();

    let param_refs: Vec<(EmitType, &str)> = params.iter().map(|(t, n)| (*t, n.as_str())).collect();

    emitter.begin_function(name, &param_refs, ret_ty);

    // Emit allocas for all locals at function entry
    for (i, ld) in mir.local_decls.iter().enumerate() {
        let ty = emitter::mir_type_to_emit_type(&ld.ty);
        let ptr_name = format!("%loc_{}", i);
        let ptr = emitter.emit_alloca(ty, &ptr_name);
        emitter.set_local_ptr(i as u32, ptr);
    }

    // Store params into their alloca slots
    for (i, (ty, arg_name)) in params.iter().enumerate() {
        let local_idx = (i + 1) as u32;
        if let Some(ptr) = emitter.get_local_ptr(local_idx).cloned() {
            emitter.emit_store(*ty, arg_name, &ptr);
        }
    }

    // Walk basic blocks
    for (bb_idx, bb) in mir.basic_blocks.iter().enumerate() {
        let label = format!("bb{}", bb_idx);
        emitter.begin_block(&label);

        for stmt in &bb.statements {
            codegen_statement(emitter, mir, stmt);
        }
        codegen_terminator(emitter, mir, &bb.terminator, ret_ty, fn_names);
    }

    emitter.end_function();
}

/// Generate code for a single MIR statement.
fn codegen_statement(emitter: &mut dyn Emitter, mir: &MirBody, stmt: &Statement) {
    match &stmt.kind {
        StatementKind::Assign(boxed) => {
            let (place, rvalue) = &**boxed;
            let val = codegen_rvalue(emitter, mir, rvalue);
            // Store the result value to the local's alloca slot
            if let LvalueKind::Local(id) = &place.kind {
                let default_ty = crate::mir::ty::Ty::new(
                    crate::mir::ty::TyKind::Int(crate::ast::IntTy::I32),
                    crate::session::Span::DUMMY,
                );
                let local_ty = mir
                    .local_decls
                    .get(id.0 as usize)
                    .map(|ld| &ld.ty)
                    .unwrap_or(&default_ty);
                let ty = emitter::mir_type_to_emit_type(local_ty);
                // Also track the value directly for simple cases
                emitter.set_local(id.0, val.clone());
                // Store to alloca slot if we have one
                if let Some(ptr) = emitter.get_local_ptr(id.0).cloned() {
                    emitter.emit_store(ty, &val, &ptr);
                }
            }
        }
        StatementKind::StorageLive(id) => {
            // alloca was already emitted at function entry
            let _ = id;
        }
        StatementKind::StorageDead(_) => {
            // No-op in LLVM (memory freed at function end)
        }
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
            // Detect type from operands (float vs int, i32 vs i64)
            let ty = detect_operand_type(mir, a)
                .or(detect_operand_type(mir, b))
                .unwrap_or(EmitType::I32);
            // Comparison ops return i1, which we zext to i32 for uniformity.
            match op {
                BinOp::Eq => {
                    let cmp = if ty == EmitType::F64 || ty == EmitType::F32 {
                        emitter.emit_fcmp("oeq", ty, &a_val, &b_val)
                    } else {
                        emitter.emit_icmp("eq", ty, &a_val, &b_val)
                    };
                    emitter.emit_zext_i1_to_i32(&cmp)
                }
                BinOp::Ne => {
                    let cmp = if ty == EmitType::F64 || ty == EmitType::F32 {
                        emitter.emit_fcmp("one", ty, &a_val, &b_val)
                    } else {
                        emitter.emit_icmp("ne", ty, &a_val, &b_val)
                    };
                    emitter.emit_zext_i1_to_i32(&cmp)
                }
                BinOp::Lt => {
                    let cmp = if ty == EmitType::F64 || ty == EmitType::F32 {
                        emitter.emit_fcmp("olt", ty, &a_val, &b_val)
                    } else {
                        emitter.emit_icmp("slt", ty, &a_val, &b_val)
                    };
                    emitter.emit_zext_i1_to_i32(&cmp)
                }
                BinOp::Le => {
                    let cmp = if ty == EmitType::F64 || ty == EmitType::F32 {
                        emitter.emit_fcmp("ole", ty, &a_val, &b_val)
                    } else {
                        emitter.emit_icmp("sle", ty, &a_val, &b_val)
                    };
                    emitter.emit_zext_i1_to_i32(&cmp)
                }
                BinOp::Gt => {
                    let cmp = if ty == EmitType::F64 || ty == EmitType::F32 {
                        emitter.emit_fcmp("ogt", ty, &a_val, &b_val)
                    } else {
                        emitter.emit_icmp("sgt", ty, &a_val, &b_val)
                    };
                    emitter.emit_zext_i1_to_i32(&cmp)
                }
                BinOp::Ge => {
                    let cmp = if ty == EmitType::F64 || ty == EmitType::F32 {
                        emitter.emit_fcmp("oge", ty, &a_val, &b_val)
                    } else {
                        emitter.emit_icmp("sge", ty, &a_val, &b_val)
                    };
                    emitter.emit_zext_i1_to_i32(&cmp)
                }
                // Arithmetic ops — use detected type (fadd for float, add for int)
                _ => emitter.emit_binary_op(*op, ty, &a_val, &b_val),
            }
        }

        Rvalue::UnaryOp(op, operand) => {
            let val = codegen_operand(emitter, mir, operand);
            emitter.emit_unary_op(*op, EmitType::I32, &val)
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
            } else {
                codegen_operand(emitter, mir, &operands[0])
            }
        }
        Rvalue::Aggregate(_, _) => "0".to_string(),

        Rvalue::Cast(_, op, target_ty) => {
            // Cast: convert operand to target type.
            // For numeric casts (i32 → u64, i32 → f64, etc.), we emit
            // the appropriate LLVM conversion instruction.
            let val = codegen_operand(emitter, mir, op);
            let src_ty = EmitType::I32; // simplified: assume int source
            let dst_ty = emitter::mir_type_to_emit_type(target_ty);
            emitter.emit_cast(src_ty, dst_ty, &val)
        }

        _ => "0".to_string(),
    }
}

/// Generate code for an operand.
fn codegen_operand(emitter: &mut dyn Emitter, _mir: &MirBody, op: &Operand) -> EmitValue {
    match op {
        Operand::Constant(c) => emitter.emit_constant(&c.val),
        Operand::Copy(lv) | Operand::Move(lv) => codegen_lvalue_load(emitter, lv),
    }
}

/// Load a value from an lvalue (place expression).
///
/// For Local(id): load from the alloca slot.
/// For Projection(base, Deref): load the pointer from base, then load through it.
/// For other projections: simplified to 0.
fn codegen_lvalue_load(emitter: &mut dyn Emitter, lv: &Lvalue) -> EmitValue {
    match &lv.kind {
        LvalueKind::Local(id) => {
            // Try direct value first (from set_local)
            if let Some(val) = emitter.get_local(id.0).cloned() {
                // Check if it's a simple constant (no load needed)
                if !val.starts_with('%') {
                    return val;
                }
            }
            // Load from alloca slot
            if let Some(ptr) = emitter.get_local_ptr(id.0).cloned() {
                return emitter.emit_load(EmitType::I32, &ptr);
            }
            "0".to_string()
        }
        LvalueKind::Projection(base, elem) => {
            match elem {
                ProjectionElem::Deref => {
                    // *ptr: first load the pointer value from base,
                    // then load the value through that pointer.
                    let ptr_val = codegen_lvalue_load(emitter, base);
                    emitter.emit_load(EmitType::I32, &ptr_val)
                }
                ProjectionElem::Field(field_id, _) => {
                    // struct field access: getelementptr + load
                    let base_ptr = if let LvalueKind::Local(id) = &base.kind {
                        emitter
                            .get_local_ptr(id.0)
                            .cloned()
                            .unwrap_or_else(|| "0".to_string())
                    } else {
                        codegen_lvalue_load(emitter, base)
                    };
                    let field_ptr = emitter.emit_gep_field(&base_ptr, field_id.0);
                    emitter.emit_load(EmitType::I32, &field_ptr)
                }
                ProjectionElem::Index(idx) => {
                    // array index: getelementptr + load
                    let base_ptr = if let LvalueKind::Local(id) = &base.kind {
                        emitter
                            .get_local_ptr(id.0)
                            .cloned()
                            .unwrap_or_else(|| "0".to_string())
                    } else {
                        codegen_lvalue_load(emitter, base)
                    };
                    let idx_val = if let Some(v) = emitter.get_local(idx.0).cloned() {
                        v
                    } else if let Some(ptr) = emitter.get_local_ptr(idx.0).cloned() {
                        emitter.emit_load(EmitType::I32, &ptr)
                    } else {
                        "0".to_string()
                    };
                    let elem_ptr = emitter.emit_gep_index(&base_ptr, &idx_val);
                    emitter.emit_load(EmitType::I32, &elem_ptr)
                }
                ProjectionElem::ConstantIndex { offset, .. } => {
                    // constant index: getelementptr with constant
                    let base_ptr = if let LvalueKind::Local(id) = &base.kind {
                        emitter
                            .get_local_ptr(id.0)
                            .cloned()
                            .unwrap_or_else(|| "0".to_string())
                    } else {
                        codegen_lvalue_load(emitter, base)
                    };
                    let elem_ptr = emitter.emit_gep_index(&base_ptr, &offset.to_string());
                    emitter.emit_load(EmitType::I32, &elem_ptr)
                }
                _ => "0".to_string(),
            }
        }
        LvalueKind::Static(_) => "0".to_string(),
    }
}

/// Generate code for a terminator.
fn codegen_terminator(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    term: &Terminator,
    ret_ty: EmitType,
    fn_names: &[String],
) {
    match term {
        Terminator::Return => {
            // Load the return value from LocalId(0)'s alloca.
            // Per design doc §4.3: %ret = load i32, i32* %_0; ret i32 %ret
            // Handle different return types:
            // - i32/i64: ret iN %val
            // - bool: ret i1 %val
            // - unit (): ret void
            if ret_ty == EmitType::Void {
                emitter.emit_return(ret_ty, None);
            } else {
                let ret_val = if let Some(ptr) = emitter.get_local_ptr(0).cloned() {
                    Some(emitter.emit_load(ret_ty, &ptr))
                } else {
                    emitter.get_local(0).cloned()
                };
                emitter.emit_return(ret_ty, ret_val.as_ref());
            }
        }

        Terminator::Unreachable => {
            emitter.emit_unreachable();
        }

        Terminator::Goto(target) => {
            emitter.emit_branch(&format!("bb{}", target.0));
        }

        Terminator::SwitchInt {
            discr,
            targets,
            otherwise,
        } => {
            let discr_val = codegen_operand(emitter, mir, discr);

            // Check if this is a bool switch (from if/while)
            let is_bool_switch = targets
                .iter()
                .any(|(val, _)| matches!(val, ConstVal::Bool(_)));

            if is_bool_switch {
                // Simple conditional branch: br i1 %discr, label %true, label %false
                let true_bb = targets
                    .iter()
                    .find(|(val, _)| matches!(val, ConstVal::Bool(true)))
                    .map(|(_, bb)| bb.0)
                    .unwrap_or(otherwise.0);
                let false_bb = otherwise.0;
                emitter.emit_cond_branch(
                    &discr_val,
                    &format!("bb{}", true_bb),
                    &format!("bb{}", false_bb),
                );
            } else {
                // Integer switch: emit LLVM switch instruction
                let cases: Vec<(i128, String)> = targets
                    .iter()
                    .filter_map(|(val, bb)| match val {
                        ConstVal::Int(n) => Some((*n as i128, format!("bb{}", bb.0))),
                        ConstVal::Uint(n) => Some((*n as i128, format!("bb{}", bb.0))),
                        _ => None,
                    })
                    .collect();
                emitter.emit_switch(&discr_val, &cases, &format!("bb{}", otherwise.0));
            }
        }

        Terminator::Call {
            func,
            args,
            destination,
            target,
        } => {
            // Resolve the function name from the func operand.
            // The func operand is a Constant with FnDef type.
            // Its ConstVal::Uint(n) holds the DefId.
            // We match it against the fn_names list (which is indexed
            // by body index, and body index == def_id for top-level fns).
            let fn_name = if let Operand::Copy(lv) | Operand::Move(lv) = func {
                // The func is loaded from a local that holds a FnDef value.
                // We need to check the local's type to find the DefId.
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

            let arg_vals: Vec<EmitValue> = args
                .iter()
                .map(|a| codegen_operand(emitter, mir, a))
                .collect();

            let ret_val = if let Some(ref name) = fn_name {
                emitter.emit_call(name, &arg_vals, EmitType::I32)
            } else {
                // Unresolved call — emit a placeholder
                "0".to_string()
            };

            // Store result to destination
            if let LvalueKind::Local(id) = &destination.kind {
                let ty = EmitType::I32;
                emitter.set_local(id.0, ret_val.clone());
                if let Some(ptr) = emitter.get_local_ptr(id.0).cloned() {
                    emitter.emit_store(ty, &ret_val, &ptr);
                }
            }

            // Branch to continuation block
            if let Some(cont) = target {
                emitter.emit_branch(&format!("bb{}", cont.0));
            }
        }

        Terminator::Assert { cond, target, .. } => {
            // Assert: check condition, branch to target or panic.
            // Per design doc §4.3: in debug mode, emit actual check.
            // For now, just branch to target (simplified — no real panic).
            let cond_val = codegen_operand(emitter, mir, cond);
            emitter.emit_cond_branch(
                &cond_val,
                &format!("bb{}", target.0),
                &format!("bb{}", target.0),
            );
        }

        Terminator::Drop { place, target, .. } => {
            // Drop: per design doc §6.2, check if type needs drop.
            // For MVP, primitives don't need drop — just branch to target.
            // Full drop glue generation is a future stage.
            let _ = place;
            emitter.emit_branch(&format!("bb{}", target.0));
        }
    }
}

/// Detect the EmitType of an operand by examining its source.
///
/// For constants: checks ConstVal variant (Float → F64, Int → I32).
/// For locals: looks up local_decls and maps the Ty.
/// Returns None if the type can't be determined (defaults to I32).
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
                    .map(|ld| emitter::mir_type_to_emit_type(&ld.ty))
            } else {
                None
            }
        }
    }
}
