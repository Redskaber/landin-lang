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
        let (mut mir, unify) =
            crate::mir::lower::lower_hir_body_to_mir_full(body, interner, return_ty);
        let mut tc = crate::typeck::TypeChecker::with_unify(unify);
        tc.populate_fn_sigs(hir);
        tc.check_mir_body(&mut mir);
        codegen_function(emitter, &fn_name, &mir, &fn_names, hir);
    }
}

/// Generate LLVM IR for a single function.
fn codegen_function(
    emitter: &mut dyn Emitter,
    name: &str,
    mir: &MirBody,
    fn_names: &[String],
    hir: &HirCrate,
) {
    // Determine return type
    let ret_ty = if mir.local_decls.is_empty() {
        EmitType::I32
    } else {
        emitter::mir_type_to_emit_type(&mir.local_decls[0].ty)
    };

    // Determine params: locals 1..N where N = number of fn params.
    // We detect params by checking which locals have StorageLive at entry
    // AND are not the return local (LocalId(0)).
    let params: Vec<(EmitType, String)> = Vec::new();
    for (i, _ld) in mir.local_decls.iter().enumerate().skip(1) {
        // Heuristic: if the local decl has a name or is early, treat as param.
        // A proper impl would track which locals are params from MIR lower.
        // For now, we check if StorageLive appears at entry for this local.
        let is_param = mir
            .basic_blocks
            .first()
            .map(|bb| {
                bb.statements.iter().any(|s| {
                matches!(&s.kind, StatementKind::StorageLive(LocalId(n)) if *n as usize == i)
            })
            })
            .unwrap_or(false);
        if is_param && i <= 10 {
            // Only treat first few as params (avoid treating all StorageLive as params)
            // This is a simplification — proper param detection needs MIR lower changes.
        }
        // For Stage 3.2: skip param detection, use no params.
        // Params will be properly handled when we wire the fn sig.
        let _ = is_param;
    }

    let param_refs: Vec<(EmitType, &str)> = params.iter().map(|(t, n)| (*t, n.as_str())).collect();

    emitter.begin_function(name, &param_refs, ret_ty);

    // Emit allocas for all locals at function entry (Stage 3.2)
    for (i, ld) in mir.local_decls.iter().enumerate() {
        let ty = emitter::mir_type_to_emit_type(&ld.ty);
        let ptr_name = format!("%loc_{}", i);
        let ptr = emitter.emit_alloca(ty, &ptr_name);
        emitter.set_local_ptr(i as u32, ptr);
    }

    // Walk basic blocks
    for (bb_idx, bb) in mir.basic_blocks.iter().enumerate() {
        let label = format!("bb{}", bb_idx);
        emitter.begin_block(&label);

        for stmt in &bb.statements {
            codegen_statement(emitter, mir, stmt);
        }
        codegen_terminator(emitter, mir, &bb.terminator, ret_ty, fn_names, hir);
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
            let ty = EmitType::I32;
            emitter.emit_binary_op(*op, ty, &a_val, &b_val)
        }

        Rvalue::UnaryOp(op, operand) => {
            let val = codegen_operand(emitter, mir, operand);
            let ty = EmitType::I32;
            emitter.emit_unary_op(*op, ty, &val)
        }

        Rvalue::Ref(_, _borrow_kind, lv) => {
            // &x → get the alloca pointer of x
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

        _ => "0".to_string(),
    }
}

/// Generate code for an operand.
fn codegen_operand(emitter: &mut dyn Emitter, _mir: &MirBody, op: &Operand) -> EmitValue {
    match op {
        Operand::Constant(c) => emitter.emit_constant(&c.val),
        Operand::Copy(lv) | Operand::Move(lv) => {
            if let LvalueKind::Local(id) = &lv.kind {
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
            } else if let LvalueKind::Projection(base, elem) = &lv.kind {
                // *ptr → load from the pointer
                if matches!(elem, ProjectionElem::Deref) {
                    if let LvalueKind::Local(id) = &base.kind {
                        if let Some(ptr) = emitter.get_local_ptr(id.0).cloned() {
                            return emitter.emit_load(EmitType::I32, &ptr);
                        }
                    }
                }
                "0".to_string()
            } else {
                "0".to_string()
            }
        }
    }
}

/// Generate code for a terminator.
fn codegen_terminator(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    term: &Terminator,
    ret_ty: EmitType,
    fn_names: &[String],
    _hir: &HirCrate,
) {
    match term {
        Terminator::Return => {
            // Load the return value from LocalId(0)'s alloca
            let ret_val = if let Some(ptr) = emitter.get_local_ptr(0).cloned() {
                Some(emitter.emit_load(ret_ty, &ptr))
            } else {
                emitter.get_local(0).cloned()
            };
            emitter.emit_return(ret_ty, ret_val.as_ref());
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
                // Integer switch: use LLVM switch instruction
                // For simplicity, emit a series of conditional branches
                // (a real impl would use LLVM's switch instruction)
                let current = discr_val.clone();
                let _ = current;
                // Simplified: just branch to otherwise for now
                // (proper switch implementation is Stage 3.3)
                emitter.emit_branch(&format!("bb{}", otherwise.0));
            }
        }

        Terminator::Call {
            func,
            args,
            destination,
            target,
        } => {
            // Resolve the function name from the func operand
            let fn_name = if let Operand::Constant(c) = func {
                if let ConstVal::Uint(n) = &c.val {
                    let idx = *n as usize;
                    fn_names.get(idx).cloned()
                } else {
                    None
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
            // Simplified: just branch to target (overflow check is Stage 3.5)
            let cond_val = codegen_operand(emitter, mir, cond);
            emitter.emit_cond_branch(
                &cond_val,
                &format!("bb{}", target.0),
                &format!("bb{}", target.0),
            );
        }

        _ => {
            // Unsupported terminators
        }
    }
}
