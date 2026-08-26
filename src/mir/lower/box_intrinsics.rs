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

use super::compute_type_size_with_fallback;
use super::MirLowerCtxt;

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
