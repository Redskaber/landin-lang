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

/// Stage 18.185 (TD-STRING-INTRINSICS): Lower `String::from_str(s: &str) -> String`.
///
/// Generates MIR for:
///   1. len = s.len (extract from &str fat pointer field 1)
///   2. ptr = __landin_alloc(len) (allocate heap buffer)
///   3. data_ptr = s.ptr (extract from &str fat pointer field 0)
///   4. __landin_memcpy(ptr, data_ptr, len) (copy bytes)
///   5. Construct String { ptr, len, cap: len }
///
/// Per §1.0 原則 6 (通解>特例): one intrinsic for all String::from_str calls.
/// Per §2 原則 9 (正确>妥协): proper alloc+memcpy, not a stub.
/// Per §10: `lower_string_from_str_intrinsic` follows `<verb>_<noun>_<noun>_<noun>` pattern.
pub(super) fn lower_string_from_str_intrinsic(
    cx: &mut MirLowerCtxt,
    expr: &HirExpr,
    src_local: LocalId,
) -> LocalId {
    use crate::mir::place::AggregateKind;

    let usize_ty = Ty::new(TyKind::Uint(crate::ast::UintTy::Usize), expr.span);
    let u8_ptr_ty = Ty::new(
        TyKind::RawPtr(
            crate::mir::ty::Mutability::Mutable,
            Box::new(Ty::new(TyKind::Uint(crate::ast::UintTy::U8), expr.span)),
        ),
        expr.span,
    );

    // Step 1: Extract len from &str fat pointer (field 1).
    let len_local = cx.mir.new_local(usize_ty.clone(), None, expr.span);
    cx.push_assign(
        Place::local(len_local, expr.span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(src_local, expr.span)),
                ProjectionElem::Field(FieldId(1), usize_ty.clone()),
            ),
            span: expr.span,
        })),
        expr.span,
    );

    // Step 2: Extract data ptr from &str fat pointer (field 0).
    let data_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, expr.span);
    cx.push_assign(
        Place::local(data_ptr_local, expr.span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(src_local, expr.span)),
                ProjectionElem::Field(FieldId(0), u8_ptr_ty.clone()),
            ),
            span: expr.span,
        })),
        expr.span,
    );

    // Step 3: Call __landin_alloc(len) to get heap buffer.
    // Stage 18.185: Use synthetic DefIds (u32::MAX - 100, u32::MAX - 101)
    // for __landin_alloc and __landin_memcpy. These are registered in
    // driver_validations.rs::register_builtin_macros so codegen can resolve
    // them. The offsets (100, 101) are well outside the BUILTIN_MACRO_NAMES
    // range (max 28 entries) to avoid collision.
    let alloc_def_id = crate::hir::DefId::new(u32::MAX - 100);
    let alloc_fn_ty = Ty::new(
        TyKind::FnDef(alloc_def_id, std::vec::Vec::new().into()),
        expr.span,
    );
    let alloc_fn_local = cx.mir.new_local(alloc_fn_ty, None, expr.span);
    let alloc_ret_ty = u8_ptr_ty.clone();
    let alloc_dest = cx.mir.new_local(alloc_ret_ty.clone(), None, expr.span);
    let alloc_cont = cx.new_block();
    cx.terminate_kind_and_goto(
        TerminatorKind::Call {
            func: Operand::Move(Place::local(alloc_fn_local, expr.span)),
            args: vec![Operand::Copy(Place::local(len_local, expr.span))],
            destination: Place::local(alloc_dest, expr.span),
            target: Some(alloc_cont),
            dyn_trait_call: None,
        },
        alloc_cont,
    );

    // Step 4: Call __landin_memcpy(alloc_dest, data_ptr, len).
    let memcpy_def_id = crate::hir::DefId::new(u32::MAX - 101);
    let memcpy_fn_ty = Ty::new(
        TyKind::FnDef(memcpy_def_id, std::vec::Vec::new().into()),
        expr.span,
    );
    let memcpy_fn_local = cx.mir.new_local(memcpy_fn_ty, None, expr.span);
    let memcpy_dest = cx.mir.new_local(
        Ty::new(TyKind::Tuple(std::vec![]), expr.span),
        None,
        expr.span,
    );
    let memcpy_cont = cx.new_block();
    cx.terminate_kind_and_goto(
        TerminatorKind::Call {
            func: Operand::Move(Place::local(memcpy_fn_local, expr.span)),
            args: vec![
                Operand::Copy(Place::local(alloc_dest, expr.span)),
                Operand::Copy(Place::local(data_ptr_local, expr.span)),
                Operand::Copy(Place::local(len_local, expr.span)),
            ],
            destination: Place::local(memcpy_dest, expr.span),
            target: Some(memcpy_cont),
            dyn_trait_call: None,
        },
        memcpy_cont,
    );

    // Step 5: Construct String { ptr: alloc_dest, len: len_local, cap: len_local }.
    // Look up the String struct's DefId from HIR by name.
    // Per §1.0 原則 6 (通解>特例): one lookup for all String::from_str calls.
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
        Ty::new(TyKind::Adt(did, std::vec::Vec::new().into()), expr.span)
    } else {
        Ty::new(TyKind::Error, expr.span)
    };

    let dest = cx.mir.new_local(string_ty.clone(), None, expr.span);
    let cont = cx.new_block();
    cx.push_assign(
        Place::local(dest, expr.span),
        Rvalue::Aggregate(
            AggregateKind::Adt(
                string_def_id.unwrap_or(crate::hir::DefId::new(0)),
                0,
                std::vec::Vec::new().into(),
                vec![u8_ptr_ty.clone(), usize_ty.clone(), usize_ty.clone()],
            ),
            vec![
                Operand::Copy(Place::local(alloc_dest, expr.span)),
                Operand::Copy(Place::local(len_local, expr.span)),
                Operand::Copy(Place::local(len_local, expr.span)),
            ],
        ),
        expr.span,
    );
    cx.terminate_and_goto(
        Terminator {
            kind: TerminatorKind::Goto(cont),
            span: expr.span,
        },
        cont,
    );
    dest
}

/// Stage 18.230 (v0.2.5f): Lower `String::push_str(src: &str)` via MIR intrinsics.
///
/// Replaces the previous `__landin_string_push_str` C helper Call with a pure
/// MIR sequence using MIR intrinsic ops (Load + GetElementPtr + Store) plus
/// `__landin_realloc` (primitive, per §16.5) and `__landin_memcpy` (primitive,
/// per §16.5). The growth while loop is expressed via a MIR back-edge.
///
/// Generates MIR for (10 basic blocks):
///   1. bb0: Extract str fields + src fields; compute new_len; need_grow check
///   2. grow_init_bb: is_zero = (cap == 0); SwitchInt
///   3. zero_cap_bb: new_cap = 4; goto grow_loop_bb
///   4. nonzero_cap_bb: new_cap = cap; goto grow_loop_bb
///   5. grow_loop_bb: cond = (new_cap < new_len); SwitchInt ← BACK-EDGE TARGET
///   6. grow_body_bb: new_cap = new_cap + new_cap; goto grow_loop_bb ← BACK-EDGE
///   7. alloc_bb: realloc + Store str.ptr + Store str.cap; goto copy_bb
///   8. copy_bb: reload str.ptr; GEP(dest, len); Call memcpy; Store str.len
///
/// Per §1.0 原則 6 (通解>特解): one MIR sequence for all String::push_str calls.
/// Per §1.0 原則 4 (报错>静默): OOM panics via `__landin_realloc` (visible).
/// Per §10 DRY: reuses `__landin_realloc` + `__landin_memcpy` (primitive helpers),
/// `MemoryEmitter` methods, `push_statement` API (Stage 18.229).
/// Per §11 接口隔离: MIR lowering emits MIR intrinsics; codegen only translates MIR.
/// Per §12 (最优 > 最小): typed Load + Store + memcpy replaces byte-by-byte C loop.
///
/// **MVP scope (§17.6 record)**:
/// - **Always realloc**: libc `realloc(NULL, size) == malloc(size)` per C standard.
/// - **No OOM check**: `__landin_realloc` itself panics on OOM (runtime.rs:185).
/// - **PHI avoidance**: Reload `str.ptr` in copy_bb via `Projection(recv, Field(0))`.
/// - **memcpy via C helper**: `__landin_memcpy` is a primitive C helper (per §16.5).
/// - **Growth while loop**: Expressed via MIR back-edge (first MIR loop in an intrinsic).
///
/// Stage 31.6c (v0.19): This function is now DEAD CODE — `push_str` has been
/// migrated to prelude impl using `.ptr`/`.len`/`.cap` + extern C. Kept for
/// reference per §1.0 原則 13 (architecture limit recording). Will be removed
/// in Stage 31.7 (whitelist cleanup).
#[allow(dead_code)]
pub(super) fn lower_string_push_str_intrinsic(
    cx: &mut MirLowerCtxt,
    expr: &HirExpr,
    recv_local: LocalId,
    src_local: LocalId,
) -> LocalId {
    use crate::mir::ty::ConstVal;

    let span = expr.span;
    let usize_ty = Ty::new(TyKind::Uint(crate::ast::UintTy::Usize), span);
    let bool_ty = Ty::new(TyKind::Bool, span);
    let u8_ty = Ty::new(TyKind::Uint(crate::ast::UintTy::U8), span);
    let u8_ptr_ty = Ty::new(
        TyKind::RawPtr(crate::mir::ty::Mutability::Mutable, Box::new(u8_ty.clone())),
        span,
    );

    // Step 1: Extract str.ptr (field 0, *mut u8).
    let data_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(data_ptr_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(0), u8_ptr_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 2: Extract str.len (field 1, i64).
    let len_local = cx.mir.new_local(usize_ty.clone(), None, span);
    cx.push_assign(
        Place::local(len_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(1), usize_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 3: Extract str.cap (field 2, i64).
    let cap_local = cx.mir.new_local(usize_ty.clone(), None, span);
    cx.push_assign(
        Place::local(cap_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(2), usize_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 4: Extract src.ptr (field 0) from &str fat pointer.
    let src_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(src_ptr_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(src_local, span)),
                ProjectionElem::Field(FieldId(0), u8_ptr_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 5: Extract src.len (field 1) from &str fat pointer.
    let src_len_local = cx.mir.new_local(usize_ty.clone(), None, span);
    cx.push_assign(
        Place::local(src_len_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(src_local, span)),
                ProjectionElem::Field(FieldId(1), usize_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 6: new_len = len + src_len.
    let new_len_local = cx.mir.new_local(usize_ty.clone(), None, span);
    cx.push_assign(
        Place::local(new_len_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Add,
            Operand::Copy(Place::local(len_local, span)),
            Operand::Copy(Place::local(src_len_local, span)),
        ),
        span,
    );

    // Step 7: need_grow = (new_len > cap).
    let need_grow_local = cx.mir.new_local(bool_ty.clone(), None, span);
    cx.push_assign(
        Place::local(need_grow_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Gt,
            Operand::Copy(Place::local(new_len_local, span)),
            Operand::Copy(Place::local(cap_local, span)),
        ),
        span,
    );

    // Step 8: SwitchInt(need_grow, [(0, copy_bb)], otherwise=grow_init_bb).
    let grow_init_bb = cx.new_block();
    let copy_bb = cx.new_block();
    cx.terminate_kind_span(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::local(need_grow_local, span)),
            targets: vec![(ConstVal::Bool(false), copy_bb)],
            otherwise: grow_init_bb,
        },
        span,
    );

    // === grow_init_bb: is_zero = (cap == 0) ===
    cx.current_block = grow_init_bb;
    let is_zero_local = cx.mir.new_local(bool_ty.clone(), None, span);
    cx.push_assign(
        Place::local(is_zero_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Eq,
            Operand::Copy(Place::local(cap_local, span)),
            Operand::Constant(Const {
                val: ConstVal::Int(0),
                ty: usize_ty.clone(),
            }),
        ),
        span,
    );

    let zero_cap_bb = cx.new_block();
    let nonzero_cap_bb = cx.new_block();
    cx.terminate_kind_span(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::local(is_zero_local, span)),
            targets: vec![(ConstVal::Bool(true), zero_cap_bb)],
            otherwise: nonzero_cap_bb,
        },
        span,
    );

    // === zero_cap_bb: new_cap = 4 (initial capacity) ===
    cx.current_block = zero_cap_bb;
    // Mutable because assigned in zero_cap_bb, nonzero_cap_bb, and grow_body_bb.
    let new_cap_local = cx.mir.new_local_with_mut(
        usize_ty.clone(),
        None,
        span,
        crate::mir::ty::Mutability::Mutable,
    );
    cx.push_assign(
        Place::local(new_cap_local, span),
        Rvalue::Use(Operand::Constant(Const {
            val: ConstVal::Int(4),
            ty: usize_ty.clone(),
        })),
        span,
    );
    let grow_loop_bb = cx.new_block();
    cx.terminate_kind_span(TerminatorKind::Goto(grow_loop_bb), span);

    // === nonzero_cap_bb: new_cap = cap ===
    cx.current_block = nonzero_cap_bb;
    cx.push_assign(
        Place::local(new_cap_local, span),
        Rvalue::Use(Operand::Copy(Place::local(cap_local, span))),
        span,
    );
    cx.terminate_kind_span(TerminatorKind::Goto(grow_loop_bb), span);

    // === grow_loop_bb: while (new_cap < new_len) new_cap *= 2  ← BACK-EDGE TARGET ===
    cx.current_block = grow_loop_bb;
    let loop_cond_local = cx.mir.new_local(bool_ty.clone(), None, span);
    cx.push_assign(
        Place::local(loop_cond_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Lt,
            Operand::Copy(Place::local(new_cap_local, span)),
            Operand::Copy(Place::local(new_len_local, span)),
        ),
        span,
    );
    let alloc_bb = cx.new_block();
    let grow_body_bb = cx.new_block();
    cx.terminate_kind_span(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::local(loop_cond_local, span)),
            targets: vec![(ConstVal::Bool(false), alloc_bb)],
            otherwise: grow_body_bb,
        },
        span,
    );

    // === grow_body_bb: new_cap = new_cap + new_cap (2x)  ← BACK-EDGE ===
    cx.current_block = grow_body_bb;
    let doubled_local = cx.mir.new_local(usize_ty.clone(), None, span);
    cx.push_assign(
        Place::local(doubled_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Add,
            Operand::Copy(Place::local(new_cap_local, span)),
            Operand::Copy(Place::local(new_cap_local, span)),
        ),
        span,
    );
    cx.push_assign(
        Place::local(new_cap_local, span),
        Rvalue::Use(Operand::Copy(Place::local(doubled_local, span))),
        span,
    );
    cx.terminate_kind_span(TerminatorKind::Goto(grow_loop_bb), span);

    // === alloc_bb: realloc + update str.ptr + str.cap ===
    cx.current_block = alloc_bb;

    // new_bytes = new_cap (String stores bytes, elem_size = 1).
    // old_bytes = cap.
    // Call __landin_realloc(data_ptr, old_bytes, new_bytes) → new_ptr.
    let realloc_def_id = crate::hir::DefId::new(u32::MAX - 102);
    let realloc_fn_ty = Ty::new(
        TyKind::FnDef(realloc_def_id, std::vec::Vec::new().into()),
        span,
    );
    let realloc_fn_local = cx.mir.new_local(realloc_fn_ty, None, span);
    let new_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
    let realloc_cont = cx.new_block();
    cx.terminate_kind_and_goto(
        TerminatorKind::Call {
            func: Operand::Move(Place::local(realloc_fn_local, span)),
            args: vec![
                Operand::Copy(Place::local(data_ptr_local, span)),
                Operand::Copy(Place::local(cap_local, span)),
                Operand::Copy(Place::local(new_cap_local, span)),
            ],
            destination: Place::local(new_ptr_local, span),
            target: Some(realloc_cont),
            dyn_trait_call: None,
        },
        realloc_cont,
    );

    // Store new_ptr to str.ptr (field 0).
    cx.push_statement(
        Statement {
            kind: StatementKind::Store {
                ptr: Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(recv_local, span)),
                        ProjectionElem::Field(FieldId(0), u8_ptr_ty.clone()),
                    ),
                    span,
                },
                val: Operand::Copy(Place::local(new_ptr_local, span)),
                val_ty: u8_ptr_ty.clone(),
            },
            span,
        },
        span,
    );

    // Store new_cap to str.cap (field 2).
    cx.push_statement(
        Statement {
            kind: StatementKind::Store {
                ptr: Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(recv_local, span)),
                        ProjectionElem::Field(FieldId(2), usize_ty.clone()),
                    ),
                    span,
                },
                val: Operand::Copy(Place::local(new_cap_local, span)),
                val_ty: usize_ty.clone(),
            },
            span,
        },
        span,
    );
    cx.terminate_kind_span(TerminatorKind::Goto(copy_bb), span);

    // === copy_bb: reload str.ptr + GEP(dest, len) + memcpy + update len ===
    cx.current_block = copy_bb;

    // Reload str.ptr (handles both growth and no-growth cases).
    let current_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(current_ptr_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(0), u8_ptr_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // dest_ptr = GetElementPtr(current_ptr, [len], *mut u8).
    let dest_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(dest_ptr_local, span),
        Rvalue::GetElementPtr {
            base: Operand::Copy(Place::local(current_ptr_local, span)),
            indices: vec![Operand::Copy(Place::local(len_local, span))],
            result_ty: u8_ptr_ty.clone(),
        },
        span,
    );

    // Call __landin_memcpy(dest_ptr, src_ptr, src_len).
    // Per §16.5: __landin_memcpy is a primitive C helper (not migrated).
    let memcpy_def_id = crate::hir::DefId::new(u32::MAX - 101);
    let memcpy_fn_ty = Ty::new(
        TyKind::FnDef(memcpy_def_id, std::vec::Vec::new().into()),
        span,
    );
    let memcpy_fn_local = cx.mir.new_local(memcpy_fn_ty, None, span);
    let memcpy_dest = cx
        .mir
        .new_local(Ty::new(TyKind::Tuple(vec![]), span), None, span);
    let memcpy_cont = cx.new_block();
    cx.terminate_kind_and_goto(
        TerminatorKind::Call {
            func: Operand::Move(Place::local(memcpy_fn_local, span)),
            args: vec![
                Operand::Copy(Place::local(dest_ptr_local, span)),
                Operand::Copy(Place::local(src_ptr_local, span)),
                Operand::Copy(Place::local(src_len_local, span)),
            ],
            destination: Place::local(memcpy_dest, span),
            target: Some(memcpy_cont),
            dyn_trait_call: None,
        },
        memcpy_cont,
    );

    // Store new_len to str.len (field 1).
    cx.push_statement(
        Statement {
            kind: StatementKind::Store {
                ptr: Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(recv_local, span)),
                        ProjectionElem::Field(FieldId(1), usize_ty.clone()),
                    ),
                    span,
                },
                val: Operand::Copy(Place::local(new_len_local, span)),
                val_ty: usize_ty.clone(),
            },
            span,
        },
        span,
    );

    // Return unit.
    let unit_ty = Ty::new(TyKind::Tuple(vec![]), span);
    let dest = cx.mir.new_local(unit_ty, None, span);
    let after = cx.new_block();
    cx.terminate_and_goto(
        Terminator {
            kind: TerminatorKind::Goto(after),
            span,
        },
        after,
    );
    dest
}
