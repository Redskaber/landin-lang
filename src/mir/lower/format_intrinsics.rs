//! Stage 18.273 (TD-LOC-EXPR-VARIANTS): Intrinsic lowering functions.
//!
//! Extracted from `expr_variants.rs` to separate the "intrinsic lowering"
//! responsibility from the "expression variant lowering" responsibility.
//! Per §13.4 J2 (单一职责): each module has one clear responsibility.
//!
//! This module hosts all MIR intrinsic lowering functions — special
//! methods (String::from_str, Box::new, Vec::push/get, String::push_str,
//! format! variadic) that are lowered to inline MIR rather than regular
//! function calls.
//!
//! Per §1.0 原則 6 (通解 > 特解): each intrinsic is a "特解" (special case)
//! — the long-term goal (TD-INTRINSIC-OVERUSE Phase 2) is to migrate
//! these to regular `impl` blocks in the prelude. Until then, they
//! live here as a cohesive group.
//!
//! Per §13.4 J3 (单向流动): `lower_call_expr` in `expr_variants.rs`
//! calls these functions (one direction); no back-calls.
//! Per §13.4 J4 (编译相关表达完整): all intrinsics are self-contained —
//! they take `&mut MirLowerCtxt` and produce `LocalId`.

use crate::hir::*;
use crate::mir::body::*;
use crate::mir::place::*;
use crate::mir::ty::*;

use super::MirLowerCtxt;

/// Stage 18.231 (v0.2.5g): Lower `format!("x={}", x, ...)` via MIR intrinsics.
///
/// Replaces the previous `__landin_format_variadic` C helper Call with a pure
/// MIR sequence that walks the format string byte-by-byte and builds the
/// output String using MIR intrinsic ops (Load + GetElementPtr + Store) plus
/// `__landin_alloc` + `__landin_i64_to_str` (primitive, per §16.5).
///
/// Generates MIR for:
/// 1. Allocate a fixed-size output buffer (4096 bytes, matching C helper MVP)
/// 2. Extract fmt.ptr and fmt.len from the &str format string (arg[0])
/// 3. Initialize: out_len = 0, fmt_idx = 0, arg_idx = 1
/// 4. Loop (fmt_loop_bb): while (fmt_idx < fmt_len)
///    - Load byte at fmt_ptr[fmt_idx] via GEP + Load
///    - If byte == '{': dispatch on arg_idx (SwitchInt per arg)
///      - Call __landin_i64_to_str, advance fmt_idx by 2, arg_idx by 1
///    - Else: Store byte to out_ptr[out_len], out_len++, fmt_idx++
/// 5. Construct String { ptr: out_ptr, len: out_len, cap: out_len + 1 }
/// 6. Return the output String
///
/// Per §1.0 原則 6 (通解>特解): one MIR sequence for all format! calls.
/// Per §1.0 原則 4 (报错>静默): OOM panics via `__landin_alloc` (visible).
/// Per §10 DRY: reuses `__landin_alloc` + `__landin_i64_to_str` (primitives),
/// `MemoryEmitter` methods, `push_statement` API (Stage 18.229).
/// Per §11 接口隔离: MIR lowering emits MIR intrinsics; codegen only translates MIR.
/// Per §12 (最优 > 最小): MIR-level format walker replaces C's snprintf + buffer walk.
///
/// **MVP scope (§17.6 record)**:
/// - **Fixed-size buffer (4096 bytes)**: matches C helper MVP (runtime.rs:351).
///   Dynamic growth deferred — same limitation as the C helper.
/// - **i64 args only**: The C helper supports &str args via `%s`, but the MIR
///   migration supports i64 only (the most common case). All format args are
///   cast to i64 and formatted via `__landin_i64_to_str`. &str arg support
///   deferred to v0.3 (requires fat pointer handling in the format walker).
/// - **No arg_types array**: Type dispatch inferred from MIR (all i64).
pub(super) fn lower_format_variadic_intrinsic(
    cx: &mut MirLowerCtxt,
    expr: &HirExpr,
    arg_locals: &[LocalId],
) -> LocalId {
    use crate::mir::ty::ConstVal;

    let span = expr.span;
    let usize_ty = Ty::new(TyKind::Uint(crate::ast::UintTy::Usize), span);
    let u8_ty = Ty::new(TyKind::Uint(crate::ast::UintTy::U8), span);
    let u8_ptr_ty = Ty::new(
        TyKind::RawPtr(crate::mir::ty::Mutability::Mutable, Box::new(u8_ty.clone())),
        span,
    );
    let bool_ty = Ty::new(TyKind::Bool, span);

    // arg_locals[0] = format string (&str fat pointer)
    // arg_locals[1..] = format arguments (all cast to i64 for MVP)
    let fmt_local = arg_locals[0];

    // Look up String struct's DefId from HIR by name.
    let string_def_id = {
        let mut found = None;
        if let Some(hir) = cx.hir {
            let string_spur = cx.interner.get("String");
            if let Some(target_name) = string_spur {
                for (def_id, owner) in &hir.owners {
                    if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s)) = owner {
                        if s.ident.name == target_name {
                            found = Some(*def_id);
                            break;
                        }
                    }
                }
            }
        }
        found
    };

    let string_ty = if let Some(did) = string_def_id {
        Ty::new(TyKind::Adt(did, std::vec::Vec::new().into()), span)
    } else {
        Ty::new(TyKind::Error, span)
    };

    // Step 1: Allocate a fixed-size output buffer (4096 bytes).
    // Matches C helper MVP (runtime.rs:351: char buffer[4096]).
    let buf_size: i64 = 4096;
    let buf_size_local = cx.mir.new_local(usize_ty.clone(), None, span);
    cx.push_assign(
        Place::local(buf_size_local, span),
        Rvalue::Use(Operand::Constant(Const {
            val: ConstVal::Int(buf_size as u128),
            ty: usize_ty.clone(),
        })),
        span,
    );

    let alloc_def_id = crate::hir::DefId::new(u32::MAX - 100);
    let alloc_fn_ty = Ty::new(
        TyKind::FnDef(alloc_def_id, std::vec::Vec::new().into()),
        span,
    );
    let alloc_fn_local = cx.mir.new_local(alloc_fn_ty, None, span);
    let out_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
    let alloc_cont = cx.new_block();
    cx.terminate_kind_and_goto(
        TerminatorKind::Call {
            func: Operand::Move(Place::local(alloc_fn_local, span)),
            args: vec![Operand::Copy(Place::local(buf_size_local, span))],
            destination: Place::local(out_ptr_local, span),
            target: Some(alloc_cont),
            dyn_trait_call: None,
        },
        alloc_cont,
    );

    // Step 2: Extract fmt.ptr (field 0) from &str fat pointer.
    let fmt_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(fmt_ptr_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(fmt_local, span)),
                ProjectionElem::Field(FieldId(0), u8_ptr_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 3: Extract fmt.len (field 1) from &str fat pointer.
    let fmt_len_local = cx.mir.new_local(usize_ty.clone(), None, span);
    cx.push_assign(
        Place::local(fmt_len_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(fmt_local, span)),
                ProjectionElem::Field(FieldId(1), usize_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 4: Initialize loop variables.
    // out_len = 0 (current write position in output buffer)
    // fmt_idx = 0 (current read position in format string)
    // arg_idx = 1 (next arg to consume, 1-based; arg_locals[0] is fmt)
    let out_len_local = cx.mir.new_local_with_mut(
        usize_ty.clone(),
        None,
        span,
        crate::mir::ty::Mutability::Mutable,
    );
    cx.push_assign(
        Place::local(out_len_local, span),
        Rvalue::Use(Operand::Constant(Const {
            val: ConstVal::Int(0),
            ty: usize_ty.clone(),
        })),
        span,
    );

    let fmt_idx_local = cx.mir.new_local_with_mut(
        usize_ty.clone(),
        None,
        span,
        crate::mir::ty::Mutability::Mutable,
    );
    cx.push_assign(
        Place::local(fmt_idx_local, span),
        Rvalue::Use(Operand::Constant(Const {
            val: ConstVal::Int(0),
            ty: usize_ty.clone(),
        })),
        span,
    );

    let arg_idx_local = cx.mir.new_local_with_mut(
        usize_ty.clone(),
        None,
        span,
        crate::mir::ty::Mutability::Mutable,
    );
    cx.push_assign(
        Place::local(arg_idx_local, span),
        Rvalue::Use(Operand::Constant(Const {
            val: ConstVal::Int(1),
            ty: usize_ty.clone(),
        })),
        span,
    );

    // Step 5: fmt_loop_bb: while (fmt_idx < fmt_len)  ← BACK-EDGE TARGET
    let fmt_loop_bb = cx.new_block();
    cx.terminate_kind_span(TerminatorKind::Goto(fmt_loop_bb), span);
    cx.current_block = fmt_loop_bb;

    // Loop condition: fmt_idx < fmt_len
    let loop_cond_local = cx.mir.new_local(bool_ty.clone(), None, span);
    cx.push_assign(
        Place::local(loop_cond_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Lt,
            Operand::Copy(Place::local(fmt_idx_local, span)),
            Operand::Copy(Place::local(fmt_len_local, span)),
        ),
        span,
    );

    let loop_body_bb = cx.new_block();
    let loop_exit_bb = cx.new_block();
    cx.terminate_kind_span(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::local(loop_cond_local, span)),
            targets: vec![(ConstVal::Bool(false), loop_exit_bb)],
            otherwise: loop_body_bb,
        },
        span,
    );

    // === loop_body_bb: load byte at fmt_ptr[fmt_idx] and dispatch ===
    cx.current_block = loop_body_bb;

    // Compute byte address: fmt_ptr + fmt_idx via GEP.
    let byte_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(byte_ptr_local, span),
        Rvalue::GetElementPtr {
            base: Operand::Copy(Place::local(fmt_ptr_local, span)),
            indices: vec![Operand::Copy(Place::local(fmt_idx_local, span))],
            result_ty: u8_ptr_ty.clone(),
        },
        span,
    );

    // Load the byte.
    let byte_local = cx.mir.new_local(u8_ty.clone(), None, span);
    cx.push_assign(
        Place::local(byte_local, span),
        Rvalue::Load(
            Operand::Copy(Place::local(byte_ptr_local, span)),
            u8_ty.clone(),
        ),
        span,
    );

    // Cast byte to i64 for comparison (BinaryOp needs matching types).
    let byte_usize_local = cx.mir.new_local(usize_ty.clone(), None, span);
    cx.push_assign(
        Place::local(byte_usize_local, span),
        Rvalue::Cast(
            crate::mir::place::CastKind::Numeric,
            Operand::Copy(Place::local(byte_local, span)),
            usize_ty.clone(),
        ),
        span,
    );

    // Check if byte == '{' (ASCII 123).
    let is_open_brace_local = cx.mir.new_local(bool_ty.clone(), None, span);
    cx.push_assign(
        Place::local(is_open_brace_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Eq,
            Operand::Copy(Place::local(byte_usize_local, span)),
            Operand::Constant(Const {
                val: ConstVal::Int(123), // '{'
                ty: usize_ty.clone(),
            }),
        ),
        span,
    );

    let placeholder_bb = cx.new_block();
    let literal_bb = cx.new_block();
    cx.terminate_kind_span(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::local(is_open_brace_local, span)),
            targets: vec![(ConstVal::Bool(true), placeholder_bb)],
            otherwise: literal_bb,
        },
        span,
    );

    // === placeholder_bb: handle {} placeholder ===
    // For MVP: assume it's "{}" (open + close brace). We don't check the next
    // byte — the format string is validated at parse time. Just consume 2 bytes
    // and format the next arg as i64.
    cx.current_block = placeholder_bb;

    // Get the arg value at arg_idx (1-based → arg_locals[arg_idx - 1 + 1] = arg_locals[arg_idx]).
    // For MVP, we cast all args to i64.
    // Since MIR can't dynamically index arg_locals, we emit a SwitchInt to
    // select the arg based on arg_idx. For MVP with ≤4 args, this is feasible.
    //
    // Actually, for MVP, let's handle the common case: format! with args known
    // at compile time. We emit a specialized block per arg position.
    //
    // Simpler MVP approach: since arg_locals is known at lower time, we can
    // emit a switch on arg_idx_local with one case per known arg.
    let mut arg_switch_targets: Vec<(ConstVal, BasicBlockId)> = Vec::new();
    let mut arg_format_blocks: Vec<BasicBlockId> = Vec::new();
    for (i, _arg_local) in arg_locals.iter().enumerate().skip(1) {
        let arg_block = cx.new_block();
        arg_switch_targets.push((ConstVal::Int(i as u128), arg_block));
        arg_format_blocks.push(arg_block);
    }
    let no_arg_bb = cx.new_block(); // arg_idx > n_args → no more args
    cx.terminate_kind_span(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::local(arg_idx_local, span)),
            targets: arg_switch_targets,
            otherwise: no_arg_bb,
        },
        span,
    );

    // Emit per-arg format blocks.
    for (i, arg_local) in arg_locals.iter().enumerate().skip(1) {
        let arg_block = arg_format_blocks[i - 1];
        cx.current_block = arg_block;

        // Cast arg to i64.
        let arg_i64_local = cx.mir.new_local(usize_ty.clone(), None, span);
        cx.push_assign(
            Place::local(arg_i64_local, span),
            Rvalue::Cast(
                crate::mir::place::CastKind::Numeric,
                Operand::Copy(Place::local(*arg_local, span)),
                usize_ty.clone(),
            ),
            span,
        );

        // Compute dest pointer: out_ptr + out_len via GEP.
        let dest_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
        cx.push_assign(
            Place::local(dest_ptr_local, span),
            Rvalue::GetElementPtr {
                base: Operand::Copy(Place::local(out_ptr_local, span)),
                indices: vec![Operand::Copy(Place::local(out_len_local, span))],
                result_ty: u8_ptr_ty.clone(),
            },
            span,
        );

        // Compute remaining capacity: buf_size - out_len.
        let remaining_local = cx.mir.new_local(usize_ty.clone(), None, span);
        cx.push_assign(
            Place::local(remaining_local, span),
            Rvalue::BinaryOp(
                crate::mir::place::BinOp::Sub,
                Operand::Copy(Place::local(buf_size_local, span)),
                Operand::Copy(Place::local(out_len_local, span)),
            ),
            span,
        );

        // Call __landin_i64_to_str(dest_ptr, remaining, arg_i64) → written_len.
        let i64_to_str_def_id = crate::hir::DefId::new(u32::MAX - 107);
        let i64_to_str_fn_ty = Ty::new(
            TyKind::FnDef(i64_to_str_def_id, std::vec::Vec::new().into()),
            span,
        );
        let i64_to_str_fn_local = cx.mir.new_local(i64_to_str_fn_ty, None, span);
        let written_len_local = cx.mir.new_local(usize_ty.clone(), None, span);
        let i64_to_str_cont = cx.new_block();
        cx.terminate_kind_and_goto(
            TerminatorKind::Call {
                func: Operand::Move(Place::local(i64_to_str_fn_local, span)),
                args: vec![
                    Operand::Copy(Place::local(dest_ptr_local, span)),
                    Operand::Copy(Place::local(remaining_local, span)),
                    Operand::Copy(Place::local(arg_i64_local, span)),
                ],
                destination: Place::local(written_len_local, span),
                target: Some(i64_to_str_cont),
                dyn_trait_call: None,
            },
            i64_to_str_cont,
        );

        // out_len += written_len
        let new_out_len_local = cx.mir.new_local(usize_ty.clone(), None, span);
        cx.push_assign(
            Place::local(new_out_len_local, span),
            Rvalue::BinaryOp(
                crate::mir::place::BinOp::Add,
                Operand::Copy(Place::local(out_len_local, span)),
                Operand::Copy(Place::local(written_len_local, span)),
            ),
            span,
        );
        cx.push_assign(
            Place::local(out_len_local, span),
            Rvalue::Use(Operand::Copy(Place::local(new_out_len_local, span))),
            span,
        );

        // fmt_idx += 2 (skip "{}")
        let new_fmt_idx_local = cx.mir.new_local(usize_ty.clone(), None, span);
        cx.push_assign(
            Place::local(new_fmt_idx_local, span),
            Rvalue::BinaryOp(
                crate::mir::place::BinOp::Add,
                Operand::Copy(Place::local(fmt_idx_local, span)),
                Operand::Constant(Const {
                    val: ConstVal::Int(2),
                    ty: usize_ty.clone(),
                }),
            ),
            span,
        );
        cx.push_assign(
            Place::local(fmt_idx_local, span),
            Rvalue::Use(Operand::Copy(Place::local(new_fmt_idx_local, span))),
            span,
        );

        // arg_idx += 1
        let new_arg_idx_local = cx.mir.new_local(usize_ty.clone(), None, span);
        cx.push_assign(
            Place::local(new_arg_idx_local, span),
            Rvalue::BinaryOp(
                crate::mir::place::BinOp::Add,
                Operand::Copy(Place::local(arg_idx_local, span)),
                Operand::Constant(Const {
                    val: ConstVal::Int(1),
                    ty: usize_ty.clone(),
                }),
            ),
            span,
        );
        cx.push_assign(
            Place::local(arg_idx_local, span),
            Rvalue::Use(Operand::Copy(Place::local(new_arg_idx_local, span))),
            span,
        );

        // Back-edge to loop.
        cx.terminate_kind_span(TerminatorKind::Goto(fmt_loop_bb), span);
    }

    // === no_arg_bb: no more args to format — treat {} as literal ===
    cx.current_block = no_arg_bb;
    // Fall through to literal_bb (store '{' as literal byte).
    cx.terminate_kind_span(TerminatorKind::Goto(literal_bb), span);

    // === literal_bb: store byte to out_ptr[out_len], out_len++, fmt_idx++ ===
    cx.current_block = literal_bb;

    // Compute dest pointer: out_ptr + out_len via GEP.
    let lit_dest_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(lit_dest_ptr_local, span),
        Rvalue::GetElementPtr {
            base: Operand::Copy(Place::local(out_ptr_local, span)),
            indices: vec![Operand::Copy(Place::local(out_len_local, span))],
            result_ty: u8_ptr_ty.clone(),
        },
        span,
    );

    // Store the byte via Projection(Deref).
    cx.push_statement(
        Statement {
            kind: StatementKind::Store {
                ptr: Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(lit_dest_ptr_local, span)),
                        ProjectionElem::Deref,
                    ),
                    span,
                },
                val: Operand::Copy(Place::local(byte_local, span)),
                val_ty: u8_ty.clone(),
            },
            span,
        },
        span,
    );

    // out_len += 1
    let lit_new_out_len_local = cx.mir.new_local(usize_ty.clone(), None, span);
    cx.push_assign(
        Place::local(lit_new_out_len_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Add,
            Operand::Copy(Place::local(out_len_local, span)),
            Operand::Constant(Const {
                val: ConstVal::Int(1),
                ty: usize_ty.clone(),
            }),
        ),
        span,
    );
    cx.push_assign(
        Place::local(out_len_local, span),
        Rvalue::Use(Operand::Copy(Place::local(lit_new_out_len_local, span))),
        span,
    );

    // fmt_idx += 1
    let lit_new_fmt_idx_local = cx.mir.new_local(usize_ty.clone(), None, span);
    cx.push_assign(
        Place::local(lit_new_fmt_idx_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Add,
            Operand::Copy(Place::local(fmt_idx_local, span)),
            Operand::Constant(Const {
                val: ConstVal::Int(1),
                ty: usize_ty.clone(),
            }),
        ),
        span,
    );
    cx.push_assign(
        Place::local(fmt_idx_local, span),
        Rvalue::Use(Operand::Copy(Place::local(lit_new_fmt_idx_local, span))),
        span,
    );

    // Back-edge to loop.
    cx.terminate_kind_span(TerminatorKind::Goto(fmt_loop_bb), span);

    // === loop_exit_bb: construct String { ptr, len, cap } ===
    cx.current_block = loop_exit_bb;

    // Stage 18.231: cap = out_len + 1 (matches C helper's `result_len + 1`
    // convention — the +1 accounts for the null terminator byte).
    let cap_val_local = cx.mir.new_local(usize_ty.clone(), None, span);
    cx.push_assign(
        Place::local(cap_val_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Add,
            Operand::Copy(Place::local(out_len_local, span)),
            Operand::Constant(Const {
                val: ConstVal::Int(1),
                ty: usize_ty.clone(),
            }),
        ),
        span,
    );

    // Construct the String struct via Aggregate.
    // Stage 18.231: field_tys are [u8_ptr_ty, usize_ty, usize_ty] (ptr, len, cap).
    let dest = cx.mir.new_local(string_ty.clone(), None, span);
    let after = cx.new_block();
    cx.push_assign(
        Place::local(dest, span),
        Rvalue::Aggregate(
            crate::mir::place::AggregateKind::Adt(
                string_def_id.unwrap_or(crate::hir::DefId::new(0)),
                0,
                std::vec::Vec::new().into(),
                vec![u8_ptr_ty.clone(), usize_ty.clone(), usize_ty.clone()],
            ),
            vec![
                Operand::Copy(Place::local(out_ptr_local, span)),
                Operand::Copy(Place::local(out_len_local, span)),
                Operand::Copy(Place::local(cap_val_local, span)),
            ],
        ),
        span,
    );
    cx.terminate_and_goto(
        Terminator {
            kind: TerminatorKind::Goto(after),
            span,
        },
        after,
    );
    dest
}
