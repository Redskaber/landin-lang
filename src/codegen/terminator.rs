//! MIR terminator → LLVM IR codegen.
//!
//! Extracted from codegen/mod.rs per Stage 13.28 codegen reorganization.
//! Handles `TerminatorKind::Return`, `Goto`, `SwitchInt`, `Call`, `Drop`,
//! `Assert`, `Unreachable`.

#![allow(unused_imports)]
use super::mir_translation::{
    codegen_place_load, codegen_place_load_typed, compute_place_address, detect_operand_type,
    detect_place_storage_type, detect_place_type, unwrap_fat_ptr_for_index,
};
use super::*;
#[allow(unused_imports)]
use crate::mir::body::TerminatorKind;
use crate::mir::body::*;
use crate::mir::place::*;
use crate::mir::ty::ConstVal;
#[allow(clippy::too_many_arguments)]
pub(crate) fn codegen_terminator(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    term: &Terminator,
    ret_ty: &EmitType,
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
    fn_sigs: &std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
) {
    match &term.kind {
        TerminatorKind::Return => {
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
        TerminatorKind::Unreachable => {
            emitter.emit_unreachable();
        }
        TerminatorKind::Goto(target) => {
            emitter.emit_br(&format!("bb{}", target.0));
        }
        TerminatorKind::SwitchInt {
            discr,
            targets,
            otherwise,
        } => {
            let discr_val =
                codegen_operand(emitter, mir, discr, interner, layouts, fn_name_by_def_id);
            let is_bool_switch = targets
                .iter()
                .any(|(val, _)| matches!(val, ConstVal::Bool(_)));
            if is_bool_switch {
                // Stage 14.65: Fix bool match with both true AND false arms.
                //
                // Previously, the codegen assumed "false goes to otherwise"
                // and only checked for the `true` target. This was WRONG when
                // the match had BOTH a `true => ...` and a `false => ...` arm:
                //
                //   match b { true => 1, false => 0 }
                //
                // The `false` arm's body was NEVER executed — the otherwise
                // block was empty (or had stale content), so the result was
                // uninitialized garbage (e.g., -976284176).
                //
                // Fix: check for BOTH `true` and `false` targets. If both are
                // present (as separate arms), branch to each. If only one is
                // present, the other goes to `otherwise` (legacy behavior).
                //
                // Per §1.0 原则 5 "报错 > 静默": both arms now execute their
                // proper bodies, rather than silently skipping the false arm.
                let true_bb = targets
                    .iter()
                    .find(|(val, _)| matches!(val, ConstVal::Bool(true)))
                    .map(|(_, bb)| bb.0)
                    .unwrap_or(otherwise.0);
                let false_bb = targets
                    .iter()
                    .find(|(val, _)| matches!(val, ConstVal::Bool(false)))
                    .map(|(_, bb)| bb.0)
                    .unwrap_or(otherwise.0);
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
        TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            dyn_trait_call,
        } => {
            // Stage 15.30 (HP-22): Check dyn_trait_call field FIRST.
            // If Some, this is a dyn Trait vtable indirect call — use the
            // DynTraitMethodCall info directly from the terminator.
            // Was: decode magic `Error + Int(index)` marker from func operand.
            if let Some(call_info) = dyn_trait_call {
                let ret_val = codegen_dyn_trait_call_direct(
                    emitter,
                    call_info,
                    args,
                    interner,
                    layouts,
                    fn_name_by_def_id,
                );
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

            // Stage 5.79: Legacy dyn Trait vtable indirect call path.
            // (Kept for backward compat — will be removed in Stage 15.31.)
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
                            let ret_val = codegen_dyn_trait_call(
                                emitter,
                                mir,
                                idx,
                                args,
                                interner,
                                layouts,
                                fn_name_by_def_id,
                            );
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
                    let val =
                        codegen_operand(emitter, mir, a, interner, layouts, fn_name_by_def_id);
                    (ty, val)
                })
                .collect();
            let arg_refs: Vec<(EmitType, &EmitValue)> =
                arg_pairs.iter().map(|(t, v)| (t.clone(), v)).collect();

            // Stage 14.35: Extract the callee's DefId from the func operand
            // so we can look up its return type in fn_sigs. This fixes
            // struct-returning method calls where dest local type defaults to i32.
            let callee_def_id: Option<crate::hir::DefId> = if let Operand::Constant(c) = func {
                match &c.val {
                    ConstVal::Uint(n) => Some(crate::hir::DefId(*n as u32)),
                    ConstVal::Int(n) => Some(crate::hir::DefId(*n as u32)),
                    _ => None,
                }
            } else if let Operand::Copy(lv) | Operand::Move(lv) = func {
                if let PlaceKind::Local(id) = &lv.kind {
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

            let ret_val = if let Some(ref name) = fn_name {
                // Stage 14.35: Use the callee's actual return type from fn_sigs
                let call_ret_ty = callee_def_id
                    .and_then(|did| fn_sigs.get(&did))
                    .map(|sig| mir_type_to_emit_type_with_layouts(&sig.output, layouts))
                    .unwrap_or_else(|| {
                        if let PlaceKind::Local(id) = &destination.kind {
                            mir.local_decls
                                .get(id.0 as usize)
                                .map(|ld| mir_type_to_emit_type_with_layouts(&ld.ty, layouts))
                                .unwrap_or(EmitType::I32)
                        } else {
                            EmitType::I32
                        }
                    });
                emitter.emit_call(name, &arg_refs, &call_ret_ty)
            } else {
                // Stage 14.58: Indirect call through function pointer.
                // When fn_name is None but the func operand is a FnPtr-typed
                // local, we need to call through the pointer value.
                // Get the function pointer value from the operand.
                let fn_ptr_val =
                    codegen_operand(emitter, mir, func, interner, layouts, fn_name_by_def_id);
                // Determine return type from fn_sigs or dest local
                let call_ret_ty = if let PlaceKind::Local(id) = &destination.kind {
                    mir.local_decls
                        .get(id.0 as usize)
                        .map(|ld| mir_type_to_emit_type_with_layouts(&ld.ty, layouts))
                        .unwrap_or(EmitType::I32)
                } else {
                    EmitType::I32
                };
                emitter.emit_call(&fn_ptr_val, &arg_refs, &call_ret_ty)
            };

            if let PlaceKind::Local(id) = &destination.kind {
                // Stage 14.35: Use the callee's return type for the store too
                let dest_ty = callee_def_id
                    .and_then(|did| fn_sigs.get(&did))
                    .map(|sig| mir_type_to_emit_type_with_layouts(&sig.output, layouts))
                    .unwrap_or_else(|| {
                        mir.local_decls
                            .get(id.0 as usize)
                            .map(|ld| mir_type_to_emit_type_with_layouts(&ld.ty, layouts))
                            .unwrap_or(EmitType::I32)
                    });
                emitter.set_local(id.0, ret_val.clone());
                if let Some(ptr) = emitter.get_local_ptr(id.0).cloned() {
                    emitter.emit_store(&dest_ty, &ret_val, &ptr);
                }
            }
            if let Some(cont) = target {
                emitter.emit_br(&format!("bb{}", cont.0));
            }
        }
        TerminatorKind::Assert {
            cond, target, msg, ..
        } => {
            let panic_label = format!("panic_assert_{}", target.0);
            match msg {
                crate::mir::body::AssertMessage::Overflow(op, lhs, rhs) => {
                    let op_ty = detect_operand_type(mir, lhs, layouts)
                        .or(detect_operand_type(mir, rhs, layouts))
                        .unwrap_or(EmitType::I32);
                    let lhs_val =
                        codegen_operand(emitter, mir, lhs, interner, layouts, fn_name_by_def_id);
                    let rhs_val =
                        codegen_operand(emitter, mir, rhs, interner, layouts, fn_name_by_def_id);
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
                    let rhs_val =
                        codegen_operand(emitter, mir, rhs, interner, layouts, fn_name_by_def_id);
                    let rhs_ty = detect_operand_type(mir, rhs, layouts).unwrap_or(EmitType::I32);
                    let is_zero = emitter.emit_icmp("eq", &rhs_ty, &rhs_val, &"0".to_string());
                    emitter.emit_br_cond(&is_zero, &panic_label, &format!("bb{}", target.0));
                }
                crate::mir::body::AssertMessage::BoundsCheck => {
                    let cond_val =
                        codegen_operand(emitter, mir, cond, interner, layouts, fn_name_by_def_id);
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
        // Stage 14.103 (SH-8 documentation): TerminatorKind::Drop is a no-op in v0.1.
        //
        // In Rust, `Drop` would call the type's `Drop::drop` method here.
        // In v0.1, user-defined `Drop::drop` is not supported (GAP-3 —
        // `drop_elaboration.rs` is dead code). So there's nothing to call.
        //
        // Per §1.0 原则 3 "显式 > 隐式": the no-op is now explicitly documented.
        // When v0.2 adds Drop support, this arm will need to:
        //   1. Look up the type's Drop impl (if any)
        //   2. Call the drop method with the place as receiver
        //   3. Then branch to target
        TerminatorKind::Drop { place, target, .. } => {
            let _ = place; // v0.1: no Drop impls exist, so nothing to call
            emitter.emit_br(&format!("bb{}", target.0));
        }
    }
}
