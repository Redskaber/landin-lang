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
