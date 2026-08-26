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
use crate::session::Span;

use super::compute_type_size_with_fallback;
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

/// Stage 18.189 (TD-BOX-AUTO-DROP partial): Lower `Box::new(x) -> Box<T>`.
///
/// Generates MIR for:
///   1. size = sizeof(T) (hardcoded per primitive type for MVP)
///   2. ptr = __landin_alloc(size) (allocate heap buffer)
///   3. *ptr = x (store x into the heap buffer via Deref projection)
///   4. Construct Box { ptr } via Aggregate
///
/// Per §1.0 原則 6 (通解>特例): one intrinsic for all Box::new calls.
/// Per §2 原則 9 (正确>妥协): proper alloc+store, not a stub.
/// Per §10: `lower_box_new_intrinsic` follows `<verb>_<noun>_<noun>_<noun>` pattern.
pub(super) fn lower_box_new_intrinsic(
    cx: &mut MirLowerCtxt,
    expr: &HirExpr,
    val_local: LocalId,
) -> LocalId {
    use crate::mir::place::AggregateKind;
    use crate::mir::ty::ConstVal;

    // Step 1: Determine sizeof(T) from the value's type.
    // Stage 18.203: delegates to compute_type_size (single source of truth)
    // - handles primitives, Adt (struct/enum via HIR walk), Tuple, Array.
    // Fallback 8 for Infer/Param/Error (TD-TYPECK-GENERIC-INST, v0.2 P2+).
    let val_ty = cx.mir.local(val_local).ty.clone();
    // Box::new: fallback 8 (safe over-allocation - extra bytes unused by Deref load).
    let size: i64 = compute_type_size_with_fallback(&val_ty, cx.hir, 8);

    let usize_ty = Ty::new(TyKind::Uint(crate::ast::UintTy::Usize), expr.span);

    // Step 2: Create size constant and call __landin_alloc(size).
    let size_local = cx.mir.new_local(usize_ty.clone(), None, expr.span);
    cx.push_assign(
        Place::local(size_local, expr.span),
        Rvalue::Use(Operand::Constant(Const {
            val: ConstVal::Int(size as u128),
            ty: usize_ty.clone(),
        })),
        expr.span,
    );

    let alloc_def_id = crate::hir::DefId::new(u32::MAX - 100);
    let alloc_fn_ty = Ty::new(
        TyKind::FnDef(alloc_def_id, std::vec::Vec::new().into()),
        expr.span,
    );
    let alloc_fn_local = cx.mir.new_local(alloc_fn_ty, None, expr.span);
    // Stage 18.212: alloc_dest receives the return from __landin_alloc
    // (which is *mut u8), but for typeck purposes we want it to be *mut T
    // so that the store `*alloc_dest = x` type-checks correctly.
    // The actual LLVM codegen handles the bitcast from *mut u8 to *mut T
    // via emit_store (Stage 18.190 TD-BOX-NEW-TYPE-COERCE fix).
    let val_ptr_ty = Ty::new(
        TyKind::RawPtr(
            crate::mir::ty::Mutability::Mutable,
            Box::new(val_ty.clone()),
        ),
        expr.span,
    );
    let alloc_dest = cx.mir.new_local(val_ptr_ty, None, expr.span);
    let alloc_cont = cx.new_block();
    cx.terminate_kind_and_goto(
        TerminatorKind::Call {
            func: Operand::Move(Place::local(alloc_fn_local, expr.span)),
            args: vec![Operand::Copy(Place::local(size_local, expr.span))],
            destination: Place::local(alloc_dest, expr.span),
            target: Some(alloc_cont),
            dyn_trait_call: None,
        },
        alloc_cont,
    );

    // Step 3: Store x into the heap buffer (*alloc_dest = x).
    // Stage 18.212: alloc_dest is *mut u8 (from __landin_alloc return type),
    // but we store val_ty through it. The codegen emit_store handles the
    // pointer bitcast (Stage 18.190 TD-BOX-NEW-TYPE-COERCE fix).
    // Per §1.0 原則 9 (正确>妥协): store the actual value type, not u8.
    cx.push_assign(
        Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(alloc_dest, expr.span)),
                ProjectionElem::Deref,
            ),
            span: expr.span,
        },
        Rvalue::Use(Operand::Move(Place::local(val_local, expr.span))),
        expr.span,
    );

    // Step 4: Construct Box { ptr: alloc_dest }.
    // Look up Box struct's DefId from HIR by name.
    let box_def_id = {
        let mut found = None;
        if let Some(hir) = cx.hir {
            let box_spur = cx.interner.get("Box");
            if let Some(target_name) = box_spur {
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

    // Stage 18.212 (TD-TUPLE-CTOR-TYPECK fix): Construct Box<T> with the
    // correct element type substs. Previously hardcoded `Vec::new().into()`
    // (empty substs) and `u8_ptr_ty` as the field type — causing typeck to
    // see Box<u8> regardless of the actual T.
    //
    // Now we extract the element type from the value's type and:
    // 1. Set substs = [val_ty] so Box<Point> has substs [Point]
    // 2. Set field_ty = *mut T (matching the prelude `struct Box<T>(*mut T)`)
    //
    // Per §1.0 原則 6 (通解>特例): one path for all Box<T> types.
    // Per §12 (最优 > 最小): root-cause fix — use actual substs, not empty.
    let box_ty = if let Some(did) = box_def_id {
        Ty::new(TyKind::Adt(did, vec![val_ty.clone()].into()), expr.span)
    } else {
        Ty::new(TyKind::Error, expr.span)
    };

    // The field type is *mut T (matching `struct Box<T>(*mut T)`).
    let box_field_ty = Ty::new(
        TyKind::RawPtr(
            crate::mir::ty::Mutability::Mutable,
            Box::new(val_ty.clone()),
        ),
        expr.span,
    );

    let dest = cx.mir.new_local(box_ty.clone(), None, expr.span);
    let cont = cx.new_block();
    cx.push_assign(
        Place::local(dest, expr.span),
        Rvalue::Aggregate(
            AggregateKind::Adt(
                box_def_id.unwrap_or(crate::hir::DefId::new(0)),
                0,
                vec![val_ty.clone()].into(),
                vec![box_field_ty.clone()],
            ),
            vec![Operand::Copy(Place::local(alloc_dest, expr.span))],
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

// Stage 18.238 (TD-INTRINSIC-OVERUSE Phase 1): lower_vec_new_intrinsic REMOVED.
// Vec::new() is now handled by the prelude impl block:
//   impl<T> Vec<T> { fn new() -> Vec<T> { Vec { ptr: 0 as *mut T, len: 0, cap: 0 } } }
// Per §1.0 原則 5 (去除兼容思维): dead code removed.
// Per §1.0 原則 6 (通解 > 特解): standard method resolution, not hardcoded.

/// Stage 18.229 (v0.2.5e): Lower `Vec::push(x)` via MIR intrinsics.
///
/// Replaces the previous `__landin_vec_push` C helper Call with a pure MIR
/// sequence using the MIR intrinsic ops (Load + GetElementPtr + Store) added
/// in Stage 18.226 and codegen-enabled in Stage 18.227. The growth logic
/// (conditional realloc) is expressed via `SwitchInt` + `Call(__landin_realloc)`.
///
/// Generates MIR for:
///   1. Extract `vec.ptr` (field 0), `vec.len` (field 1), `vec.cap` (field 2)
///   2. `need_grow = BinaryOp(Ge, len, cap)` — len >= cap → must grow
///   3. `SwitchInt(need_grow, [(0, store_bb)], otherwise=grow_bb)` — branch
///   4. grow_bb: `is_zero = BinaryOp(Eq, cap, 0)`; `SwitchInt(is_zero, [(1, zero_cap_bb)], otherwise=nonzero_cap_bb)`
///   5. zero_cap_bb: `new_cap = 4` (initial capacity); goto alloc_bb
///   6. nonzero_cap_bb: `new_cap = cap + cap` (2x growth); goto alloc_bb
///   7. alloc_bb: `new_bytes = new_cap * elem_size`; `old_bytes = cap * elem_size`;
///      `new_ptr = Call(__landin_realloc, [data_ptr, old_bytes, new_bytes])`;
///      `Store(vec.ptr, new_ptr)`; `Store(vec.cap, new_cap)`; goto store_bb
///   8. store_bb: `current_ptr = Use(Projection(recv, Field(0)))` (reload — handles growth);
///      `elem_ptr = GetElementPtr(current_ptr, [len], *mut T)`;
///      `Store(Projection(elem_ptr, Deref), val)` — `*elem_ptr = val`;
///      `new_len = BinaryOp(Add, len, 1)`; `Store(vec.len, new_len)`; goto after
///
/// Per §1.0 原則 6 (通解>特解): one MIR sequence for all Vec<T> types —
/// the element type T flows through `extract_vec_element_type` (Stage 18.208).
/// Per §1.0 原則 4 (报错>静默): OOM panics via `__landin_realloc` (visible).
/// Per §10 DRY: reuses `extract_vec_element_type`, `compute_type_size_with_fallback`,
/// `MemoryEmitter` methods, `Place::Projection(Deref)` pattern (Stage 14.66).
/// Per §11 接口隔离: MIR lowering emits MIR intrinsics; codegen only translates MIR.
/// Per §12 (最优 > 最小): typed Load + Store replaces byte-by-byte memcpy loop.
///
/// **MVP scope (§17.6 record)**:
/// - **Always realloc**: libc `realloc(NULL, size) == malloc(size)` per C standard.
///   When `cap == 0`, `vec.ptr` is NULL, so `__landin_realloc(NULL, 0, new_bytes)`
///   is equivalent to `malloc(new_bytes)`. One Call path instead of two.
/// - **No OOM check**: `__landin_realloc` itself panics on OOM (runtime.rs:185).
/// - **PHI avoidance**: Reload `vec.ptr` in store_bb via `Projection(recv, Field(0))`.
///   Handles both growth (field updated) and no-growth (field unchanged) cases.
///   Simpler MIR — no PHI support needed.
pub(super) fn lower_vec_push_intrinsic(
    cx: &mut MirLowerCtxt,
    expr: &HirExpr,
    recv_local: LocalId,
    val_local: LocalId,
) -> LocalId {
    use crate::mir::ty::ConstVal;

    let span = expr.span;
    let usize_ty = Ty::new(TyKind::Uint(crate::ast::UintTy::Usize), span);
    let bool_ty = Ty::new(TyKind::Bool, span);

    // Stage 18.208: Extract the element type T from `Vec<T>` (or `&Vec<T>`).
    let recv_ty = cx.mir.local(recv_local).ty.clone();
    let val_ty = {
        let raw_val_ty = cx.mir.local(val_local).ty.clone();
        if matches!(raw_val_ty.kind, TyKind::Infer(_)) {
            extract_vec_element_type(&recv_ty, span)
        } else {
            raw_val_ty
        }
    };

    // The Vec.ptr field has type `*mut T`. Construct it explicitly so the
    // Projection carries the right field type (per §1.0 原則 3 显式 > 隐式).
    let elem_ptr_ty = Ty::new(
        TyKind::RawPtr(
            crate::mir::ty::Mutability::Mutable,
            Box::new(val_ty.clone()),
        ),
        span,
    );

    // Step 1: Extract vec.ptr (field 0, *mut T) via Place::Projection.
    let data_ptr_local = cx.mir.new_local(elem_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(data_ptr_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(0), elem_ptr_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 2: Extract vec.len (field 1, i64).
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

    // Step 3: Extract vec.cap (field 2, i64).
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

    // Step 4: Compute elem_size from val type (single source of truth).
    let elem_size: i64 = compute_type_size_with_fallback(&val_ty, cx.hir, 4);
    let elem_size_local = cx.mir.new_local(usize_ty.clone(), None, span);
    cx.push_assign(
        Place::local(elem_size_local, span),
        Rvalue::Use(Operand::Constant(Const {
            val: ConstVal::Int(elem_size as u128),
            ty: usize_ty.clone(),
        })),
        span,
    );

    // Step 5: need_grow = BinaryOp(Ge, len, cap) — len >= cap → must grow.
    let need_grow_local = cx.mir.new_local(bool_ty.clone(), None, span);
    cx.push_assign(
        Place::local(need_grow_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Ge,
            Operand::Copy(Place::local(len_local, span)),
            Operand::Copy(Place::local(cap_local, span)),
        ),
        span,
    );

    // Step 6: SwitchInt(need_grow, [(0, store_bb)], otherwise=grow_bb).
    // need_grow is bool: 1 (true) → grow_bb; 0 (false) → store_bb.
    let grow_bb = cx.new_block();
    let store_bb = cx.new_block();
    cx.terminate_kind_span(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::local(need_grow_local, span)),
            targets: vec![(ConstVal::Bool(false), store_bb)],
            otherwise: grow_bb,
        },
        span,
    );

    // === grow_bb: compute new_cap (4 if cap==0, else cap*2) ===
    cx.current_block = grow_bb;

    // Step 7: is_zero = BinaryOp(Eq, cap, 0).
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

    // Step 8: SwitchInt(is_zero, [(1, zero_cap_bb)], otherwise=nonzero_cap_bb).
    let zero_cap_bb = cx.new_block();
    let nonzero_cap_bb = cx.new_block();
    let alloc_bb = cx.new_block();
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
    // Stage 18.229: new_cap_local is Mutable because it's assigned in both
    // zero_cap_bb (= 4) and nonzero_cap_bb (= cap * 2). Without Mutable,
    // the borrowck flags the second assignment as "assign twice to immutable".
    // Per §1.0 原則 6 (通解>特例): same pattern as if/else result locals
    // (control_flow.rs:31 uses new_local_with_mut for PHI-like assignments).
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
    cx.terminate_kind_span(TerminatorKind::Goto(alloc_bb), span);

    // === nonzero_cap_bb: new_cap = cap + cap (2x growth) ===
    cx.current_block = nonzero_cap_bb;
    let doubled_local = cx.mir.new_local(usize_ty.clone(), None, span);
    cx.push_assign(
        Place::local(doubled_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Add,
            Operand::Copy(Place::local(cap_local, span)),
            Operand::Copy(Place::local(cap_local, span)),
        ),
        span,
    );
    cx.push_assign(
        Place::local(new_cap_local, span),
        Rvalue::Use(Operand::Copy(Place::local(doubled_local, span))),
        span,
    );
    cx.terminate_kind_span(TerminatorKind::Goto(alloc_bb), span);

    // === alloc_bb: realloc + update vec.ptr + vec.cap ===
    cx.current_block = alloc_bb;

    // new_bytes = new_cap * elem_size
    let new_bytes_local = cx.mir.new_local(usize_ty.clone(), None, span);
    cx.push_assign(
        Place::local(new_bytes_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Mul,
            Operand::Copy(Place::local(new_cap_local, span)),
            Operand::Copy(Place::local(elem_size_local, span)),
        ),
        span,
    );

    // old_bytes = cap * elem_size (passed to __landin_realloc for diagnostics)
    let old_bytes_local = cx.mir.new_local(usize_ty.clone(), None, span);
    cx.push_assign(
        Place::local(old_bytes_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Mul,
            Operand::Copy(Place::local(cap_local, span)),
            Operand::Copy(Place::local(elem_size_local, span)),
        ),
        span,
    );

    // Call __landin_realloc(data_ptr, old_bytes, new_bytes) → new_ptr.
    // libc realloc(NULL, size) == malloc(size), so this handles cap==0 case.
    // Per §16.5 (06-mir.md): __landin_realloc is a primitive C helper (not migrated).
    let realloc_def_id = crate::hir::DefId::new(u32::MAX - 102);
    let realloc_fn_ty = Ty::new(
        TyKind::FnDef(realloc_def_id, std::vec::Vec::new().into()),
        span,
    );
    let realloc_fn_local = cx.mir.new_local(realloc_fn_ty, None, span);
    let new_ptr_local = cx.mir.new_local(elem_ptr_ty.clone(), None, span);
    let realloc_cont = cx.new_block();
    cx.terminate_kind_and_goto(
        TerminatorKind::Call {
            func: Operand::Move(Place::local(realloc_fn_local, span)),
            args: vec![
                Operand::Copy(Place::local(data_ptr_local, span)),
                Operand::Copy(Place::local(old_bytes_local, span)),
                Operand::Copy(Place::local(new_bytes_local, span)),
            ],
            destination: Place::local(new_ptr_local, span),
            target: Some(realloc_cont),
            dyn_trait_call: None,
        },
        realloc_cont,
    );

    // Store new_ptr to vec.ptr (field 0).
    // Per §10 DRY: reuses StatementKind::Store + Field projection pattern.
    cx.push_statement(
        Statement {
            kind: StatementKind::Store {
                ptr: Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(recv_local, span)),
                        ProjectionElem::Field(FieldId(0), elem_ptr_ty.clone()),
                    ),
                    span,
                },
                val: Operand::Copy(Place::local(new_ptr_local, span)),
                val_ty: elem_ptr_ty.clone(),
            },
            span,
        },
        span,
    );

    // Store new_cap to vec.cap (field 2).
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
    cx.terminate_kind_span(TerminatorKind::Goto(store_bb), span);

    // === store_bb: store val + increment len ===
    cx.current_block = store_bb;

    // Reload vec.ptr (handles both growth and no-growth cases via Field projection).
    let current_ptr_local = cx.mir.new_local(elem_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(current_ptr_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(0), elem_ptr_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // elem_ptr = GetElementPtr(current_ptr, [len], *mut T)
    let elem_ptr_local = cx.mir.new_local(elem_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(elem_ptr_local, span),
        Rvalue::GetElementPtr {
            base: Operand::Copy(Place::local(current_ptr_local, span)),
            indices: vec![Operand::Copy(Place::local(len_local, span))],
            result_ty: elem_ptr_ty.clone(),
        },
        span,
    );

    // *elem_ptr = val — Store through the pointer via Projection(Deref).
    // Reuses the Box::new pattern (Stage 14.66 Deref + RawPtr handling).
    cx.push_statement(
        Statement {
            kind: StatementKind::Store {
                ptr: Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(elem_ptr_local, span)),
                        ProjectionElem::Deref,
                    ),
                    span,
                },
                val: Operand::Copy(Place::local(val_local, span)),
                val_ty: val_ty.clone(),
            },
            span,
        },
        span,
    );

    // new_len = BinaryOp(Add, len, 1)
    let new_len_local = cx.mir.new_local(usize_ty.clone(), None, span);
    cx.push_assign(
        Place::local(new_len_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Add,
            Operand::Copy(Place::local(len_local, span)),
            Operand::Constant(Const {
                val: ConstVal::Int(1),
                ty: usize_ty.clone(),
            }),
        ),
        span,
    );

    // Store new_len to vec.len (field 1).
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

/// Stage 18.208 (TD-VEC-GET-TYPE-INFERENCE fix): Extract the element type
/// from a `Vec<T>` receiver type.
///
/// Given the receiver's type (e.g., `Adt(Vec_def_id, [Point])`), returns
/// `substs[0]` (the element type `T`). If the receiver type is not a
/// generic Adt with at least one substitution, falls back to `i32`
/// (the canonical `Vec<i32>` case).
///
/// Per §1.0 原則 6 (通解>特例): one extraction path for all Vec<T> types.
/// Per §12 (最优 > 最小): root-cause fix — read substs[0] from the type.
/// Per §10 (DRY): single helper, used by `lower_vec_get_intrinsic`.
///
/// Stage 18.208 addendum: The receiver type may be wrapped in a `Ref`
/// (e.g., `&Vec<T>` for by-ref method calls). We unwrap one level of Ref.
pub(super) fn extract_vec_element_type(recv_ty: &Ty, span: Span) -> Ty {
    // Unwrap one level of Ref (&Vec<T> → Vec<T>).
    let inner_ty = match &recv_ty.kind {
        TyKind::Ref(_, _, inner) => inner.as_ref(),
        _ => recv_ty,
    };
    match &inner_ty.kind {
        TyKind::Adt(_def_id, substs) => {
            if let Some(elem_ty) = substs.first() {
                elem_ty.clone()
            } else {
                // substs is empty — fallback to i32 (canonical Vec<i32> case).
                Ty::new(TyKind::Int(crate::ast::IntTy::I32), span)
            }
        }
        _ => Ty::new(TyKind::Int(crate::ast::IntTy::I32), span),
    }
}

/// Stage 18.228 (v0.2.5d): Lower `Vec::get(index) -> T` via MIR intrinsics.
///
/// Replaces the previous `__landin_vec_get` C helper Call with a pure MIR
/// sequence using the new MIR intrinsic ops (Load + GetElementPtr) added
/// in Stage 18.226 and codegen-enabled in Stage 18.227.
///
/// Generates MIR for:
///   1. Extract `vec.ptr` (field 0, `*mut T`) via `Place::Projection(Field(0))`
///   2. Extract `vec.len` (field 1, `i64`) via `Place::Projection(Field(1))`
///   3. Cast `index` to `i64` (if needed)
///   4. Compute `cond = (idx < len)` via `BinaryOp(Lt)`
///   5. `Assert(cond, expected=true, target=ok_bb, msg=BoundsCheck)` —
///      branches to panic block (calls `__landin_panic_bounds_check`) on
///      OOB. Reuses existing Assert infra (Stage 3.24).
///   6. ok_bb: `elem_ptr = GetElementPtr(data_ptr, [idx])` (Stage 18.226)
///   7. `dest = Load(elem_ptr, T)` (Stage 18.226) — typed load, no memcpy
///
/// Per §1.0 原則 6 (通解>特解): one MIR sequence for all Vec<T> types —
/// the element type T flows through `extract_vec_element_type` (Stage 18.208).
/// Per §1.0 原則 4 (报错>静默): bounds check via `Assert(BoundsCheck)` —
/// OOB panics with `__landin_panic_bounds_check` (visible, not silent).
/// Per §10 DRY: reuses `extract_vec_element_type`, `MemoryEmitter` methods,
/// `AssertMessage::BoundsCheck` — no new infrastructure.
/// Per §11 接口隔离: MIR lowering emits MIR intrinsics; codegen only
/// translates MIR (no C helper Call).
/// Per §12 (最优 > 最小): typed `Load` replaces byte-by-byte `memcpy` loop.
///
/// **MVP scope (§17.6 record)**: only checks `idx < len` (upper bound).
/// The `idx < 0` check is deferred — Landin's `Vec::get` index is `usize`-like
/// in idiomatic usage (negative indices impossible in Rust convention).
/// Recorded in task-review §2.5; will be revisited if a test exercises
/// negative index behavior.
pub(super) fn lower_vec_get_intrinsic(
    cx: &mut MirLowerCtxt,
    expr: &HirExpr,
    recv_local: LocalId,
    idx_local: LocalId,
) -> LocalId {
    let span = expr.span;
    let usize_ty = Ty::new(TyKind::Uint(crate::ast::UintTy::Usize), span);

    // Stage 18.208: Extract the element type T from `Vec<T>` (or `&Vec<T>`).
    // Per §10 DRY: single source of truth for element-type extraction.
    let recv_ty = cx.mir.local(recv_local).ty.clone();
    let elem_ty = extract_vec_element_type(&recv_ty, span);

    // The Vec.ptr field has type `*mut T`. Construct it explicitly so the
    // Projection carries the right field type (per §1.0 原則 3 显式 > 隐式).
    let elem_ptr_ty = Ty::new(
        TyKind::RawPtr(
            crate::mir::ty::Mutability::Mutable,
            Box::new(elem_ty.clone()),
        ),
        span,
    );

    // Step 1: Extract `vec.ptr` (field 0, `*mut T`) via Place::Projection.
    // Reuses the AdtLayout system (Stage 18.200 lower_vec_push_intrinsic pattern).
    let data_ptr_local = cx.mir.new_local(elem_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(data_ptr_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(0), elem_ptr_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 2: Extract `vec.len` (field 1, `i64`) via Place::Projection.
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

    // Step 3: Cast `index` to `i64` (if needed). Numeric cast handles
    // i32→i64, u32→i64, etc. Per §1.0 原則 6 (通解>特例): one cast path
    // for all integer index types.
    let idx_usize = cx.mir.new_local(usize_ty.clone(), None, span);
    cx.push_assign(
        Place::local(idx_usize, span),
        Rvalue::Cast(
            crate::mir::place::CastKind::Numeric,
            Operand::Copy(Place::local(idx_local, span)),
            usize_ty.clone(),
        ),
        span,
    );

    // Step 4: Compute `cond = (idx < len)` via BinaryOp(Lt).
    // Per §1.0 原則 6 (通解>特例): one BinaryOp for all bounds checks.
    let cond_local = cx.mir.new_local(Ty::new(TyKind::Bool, span), None, span);
    cx.push_assign(
        Place::local(cond_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Lt,
            Operand::Copy(Place::local(idx_usize, span)),
            Operand::Copy(Place::local(len_local, span)),
        ),
        span,
    );

    // Step 5: `Assert(cond, expected=true, target=ok_bb, msg=BoundsCheck)`.
    // Reuses existing Assert infra (Stage 3.24) + `__landin_panic_bounds_check`
    // C helper. On OOB, codegen emits a panic block that calls the helper
    // and emits `unreachable`.
    //
    // Per §1.0 原則 4 (报错>静默): OOB panics visibly, not silent skip.
    // Per §10 DRY: reuses AssertMessage::BoundsCheck (no new panic path).
    let ok_bb = cx.new_block();
    cx.terminate_kind_span(
        TerminatorKind::Assert {
            cond: Operand::Copy(Place::local(cond_local, span)),
            expected: true,
            target: ok_bb,
            msg: crate::mir::body::AssertMessage::BoundsCheck,
        },
        span,
    );
    cx.current_block = ok_bb;

    // Step 6: `elem_ptr = GetElementPtr(data_ptr, [idx])` (Stage 18.226).
    // Computes `&data_ptr[idx]` as `*mut T`. Codegen (Stage 18.227) emits
    // `getelementptr inbounds` via `MemoryEmitter::emit_gep_index_ptr`.
    //
    // Per §1.0 原則 6 (通解>特例): one GEP for all element types — the
    // element type is encoded in the LLVM IR GEP instruction's source type.
    // Per §16.2 (06-mir.md): MIR intrinsic ops design.
    let elem_ptr_local = cx.mir.new_local(elem_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(elem_ptr_local, span),
        Rvalue::GetElementPtr {
            base: Operand::Copy(Place::local(data_ptr_local, span)),
            indices: vec![Operand::Copy(Place::local(idx_usize, span))],
            result_ty: elem_ptr_ty.clone(),
        },
        span,
    );

    // Step 7: `dest = Load(elem_ptr, T)` (Stage 18.226). Typed load — no
    // byte-by-byte memcpy needed (unlike the C helper). Codegen (Stage
    // 18.227) emits `load T, ptr %elem_ptr` via `MemoryEmitter::emit_load`.
    //
    // Per §1.0 原則 6 (通解>特例): one Load for all element types.
    // Per §12 (最优 > 最小): typed load, not memcpy loop.
    let dest = cx.mir.new_local(elem_ty.clone(), None, span);
    let after = cx.new_block();
    cx.push_assign(
        Place::local(dest, span),
        Rvalue::Load(Operand::Copy(Place::local(elem_ptr_local, span)), elem_ty),
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
