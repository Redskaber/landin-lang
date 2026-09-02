//! Method-call expression lowering.
//!
//! Stage 18.309 (P3 LOC refactor): extracted from
//! `expr_variants.rs` per §13.4 J1-J6. Single
//! responsibility — lowers `HirExprKind::MethodCall`
//! to a MIR `LocalId`.

use crate::ast::Ident;
use crate::hir::*;
use crate::mir::body::*;
use crate::mir::dyn_trait::{find_dyn_trait_method_call_in_plan_by_method, DynTraitMethodCall};
use crate::mir::place::*;
use crate::mir::ty::*;
use crate::session::Span;

use super::call_lower::build_dyn_trait_call_terminator;
use super::lower_expr_to_operand;
use super::method_resolution::{
    find_local_init_expr, find_local_init_type, query_method_self_kind, resolve_inherent_method,
    resolve_inherent_method_from_hir_expr, resolve_trait_method,
};
use super::MirLowerCtxt;
// Stage 33.1: Vec::push/get intrinsics removed — now in prelude impl.
use super::primitive_intrinsics::{emit_primitive_intrinsic, lookup_primitive_intrinsic};

/// Lower a MethodCall expression to a MIR operand (Stage 18.133: extracted from lower_expr_to_operand).
pub(super) fn lower_method_call_expr(
    cx: &mut MirLowerCtxt,
    expr: &HirExpr,
    receiver: &HirExpr,
    method: &Ident,
    args: &[HirExpr],
) -> LocalId {
    let recv_local = lower_expr_to_operand(cx, receiver, None);
    let arg_locals: Vec<LocalId> = args
        .iter()
        .map(|a| lower_expr_to_operand(cx, a, None))
        .collect();

    // Stage 5.78: dyn Trait path.
    //
    // When `cx.dyn_trait_plan()` is set AND the method name matches
    // an entry in the plan, use the dyn Trait call terminator
    // (which records the call info in `cx.mir.dyn_trait_calls`
    // side-table for codegen Stage 5.79+ to emit a vtable indirect
    // call). Otherwise fall through to the legacy placeholder path.
    //
    // Per §16: the plan was built upstream (by the driver, using
    // `build_dyn_trait_mir_plan_from_resolver()`) and attached via
    // `cx.set_dyn_trait_plan()`. The lower does not query HIR or
    // TraitResolver directly here.
    //
    // We clone the matched `DynTraitMethodCall` out of the
    // immutable borrow scope before mutating `cx` — this satisfies
    // the borrow checker (immutable borrow of `cx.dyn_trait_plan()`
    // ends before the mutable borrow begins via `build_dyn_trait_call_terminator`).
    // Stage 14.91 (Bug X3 fix): Before using the dyn Trait vtable
    // indirect call path, check if the method can be resolved via
    // static dispatch (inherent method or trait impl method). If so,
    // skip the dyn Trait path and use static dispatch instead.
    //
    // The dyn Trait path is for actual `dyn Trait` receivers (fat
    // pointers with vtable). For concrete types like `Square`, we
    // should use static dispatch — the vtable indirect call crashes
    // because the receiver is passed as a value, not a fat pointer.
    //
    // Per §1.0 原則 5 "报错 > 静默": the dyn Trait path silently
    // produced wrong code for concrete types, causing LLVM crashes.
    let method_name_str = cx.interner.resolve(&method.name).to_string();
    let matched_call: Option<DynTraitMethodCall> = cx.dyn_trait_plan().and_then(|plan| {
        find_dyn_trait_method_call_in_plan_by_method(plan, &method_name_str).cloned()
    });

    // Check if static dispatch is possible before using dyn Trait
    let can_static_dispatch = cx.hir.is_some_and(|hir| {
        let recv_ty = cx.mir.local(recv_local).ty.clone();
        if resolve_inherent_method(hir, cx.interner, &recv_ty, &method.name).is_some() {
            return true;
        }
        if resolve_inherent_method_from_hir_expr(cx, hir, receiver, &method.name).is_some() {
            return true;
        }
        // Stage 32.3: Pass owner_def_id for Param(N) trait method resolution.
        if resolve_trait_method(hir, cx.interner, &recv_ty, &method.name, cx.owner_def_id).is_some()
        {
            return true;
        }
        // Stage 14.91: Also try HIR-traced type for trait method resolution.
        // The MIR type may be Infer, but HIR tracing can find the ADT type.
        if let Some(init_ty) = find_local_init_type(cx, hir, {
            // Get the hir_id from the receiver Path
            if let HirExprKind::Path(path) = &receiver.kind {
                if let crate::hir::Res::Local(hir_id) = path.res {
                    hir_id
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }) {
            // Stage 32.3: Pass owner_def_id for Param(N) trait method resolution.
            return resolve_trait_method(hir, cx.interner, &init_ty, &method.name, cx.owner_def_id)
                .is_some();
        }
        false
    });

    if let Some(call) = matched_call {
        if !can_static_dispatch {
            // Stage 18.297 (typeck gap fix): Before using dyn Trait dispatch,
            // verify the receiver type matches the type in the dyn_trait_plan.
            // If the receiver is i32 but the plan entry is for bool (because
            // `impl T for bool` exists but not `impl T for i32`), the dyn
            // Trait path would produce wrong code (type mismatch crash).
            //
            // Per §2 原則 4 (报错>静默): report "no method found" instead of
            // silently generating wrong dyn Trait call.
            // Per §12 (最优>最小): root cause fix — check type match at the
            // dispatch decision point, not at codegen.
            let recv_ty = cx.mir.local(recv_local).ty.clone();
            let recv_type_name: String = match &recv_ty.kind {
                crate::mir::ty::TyKind::Adt(def_id, _) => cx
                    .hir
                    .and_then(|hir| {
                        hir.find_owner(*def_id).and_then(|owner| match owner {
                            crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s)) => {
                                Some(cx.interner.resolve(&s.ident.name).to_string())
                            }
                            crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(e)) => {
                                Some(cx.interner.resolve(&e.ident.name).to_string())
                            }
                            _ => None,
                        })
                    })
                    .unwrap_or_default(),
                _ => {
                    // Stage 18.297: For Infer types (e.g., unsuffixed float
                    // literal 3.14 → Infer(FloatVar)), name_of_primitive_ty
                    // returns None → recv_type_name = "".
                    // Per §2 原則 4 (报错>静默): empty recv_type_name means we
                    // can't verify the type match. For Infer types, fall through
                    // to error path (don't use dyn Trait dispatch) — typeck
                    // post-defaulting will handle it.
                    super::method_resolution::name_of_primitive_ty(&recv_ty)
                        .unwrap_or("")
                        .to_string()
                }
            };
            // Check if the receiver type matches the dyn_trait_plan's type_name.
            // If not, the trait is not implemented for this type — fall through
            // to the "no method found" error path below.
            // Stage 18.297: Empty recv_type_name (e.g. Infer type) also falls
            // through to error path — don't use dyn Trait dispatch for unknown
            // types.
            if !recv_type_name.is_empty() && recv_type_name == call.type_name {
                let dest_ty = cx.fresh_infer_ty(expr.span);
                let dest = cx.mir.new_local(dest_ty, None, expr.span);
                let cont = cx.new_block();
                let mut terminator = build_dyn_trait_call_terminator(
                    cx,
                    &call,
                    recv_local,
                    &arg_locals,
                    dest,
                    expr.span,
                );
                // Set the target before terminating — the helper
                // leaves it as None per design.
                if let TerminatorKind::Call { target, .. } = &mut terminator.kind {
                    *target = Some(cont);
                }
                cx.terminate_and_goto(terminator, cont);
                return dest;
            } // end type matches → dyn Trait dispatch
        } // end if !can_static_dispatch
          // If can_static_dispatch, fall through to the static dispatch path below
    }

    // Stage 13.17: Inherent method call resolution.
    //
    // Before Stage 13.17, this path emitted a placeholder
    // `Const{ty: Error, val: Int(0)}` func, which codegen dropped
    // (producing wrong results — method calls always returned 0).
    //
    // Stage 13.17: resolve the method to a real DefId by querying HIR
    // for an impl block on the receiver's type. If found, emit a real
    // `TerminatorKind::Call` with `func: Const{ty: FnDef(def_id), val: Uint(def_id)}`.
    // If not found (unknown method or non-ADT receiver), fall back to
    // the Error placeholder (graceful degradation).

    // Stage 18.343 (P1 soundness fix): String::as_str() intrinsic —
    // intercept BEFORE method_def_id resolution.
    //
    // Root cause (per §2.2 根因思维): as_str is declared in prelude as
    // `fn as_str(&self) -> &str { loop {} }`. `resolve_inherent_method`
    // finds this DefId → method_def_id = Some(DefId(19)). Then
    // `lookup_primitive_intrinsic(DefId(19), "as_str")` returns None
    // (as_str is NOT in the str/i32/bool intrinsic table — it's a String
    // method). So codegen falls through to the normal method call path
    // → calls `landin_String_as_str` → runs `loop {}` → infinite loop.
    //
    // Fix: intercept as_str BEFORE method_def_id resolution. When
    // `method_name == "as_str"` AND the receiver is a String, construct
    // the &str fat pointer directly and return — never reaching the
    // method_def_id path. This mirrors the pre-Stage-18.284 pattern for
    // str::len/is_empty/as_bytes (they were early-intercepted before
    // being migrated to `lookup_primitive_intrinsic`).
    //
    // Per §1.0 原則 6 (通解 > 特解): one early-interception pattern for all
    // methods needing MIR-level construction (as_str constructs a fat pointer).
    // Per §1.0 原則 4 (报错 > 静默): prelude declaration provides typeck
    // visibility; intrinsic provides the real implementation.
    // Per §12 (最优 > 最小): root-cause fix = intercept before resolution.
    // Per §20 (iterative audit): found by tracing IR — `landin_String_as_str`
    // was called with `loop {}` body instead of being intercepted.
    {
        let early_method_name = cx.interner.resolve(&method.name);
        if early_method_name == "as_str" && args.is_empty() {
            let early_recv_ty = cx.mir.local(recv_local).ty.clone();
            let early_is_string = cx.hir.is_some_and(|hir| {
                if let crate::mir::ty::TyKind::Adt(did, _) = &early_recv_ty.kind {
                    if let Some(crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s))) =
                        hir.find_owner(*did)
                    {
                        return cx.interner.resolve(&s.ident.name) == "String";
                    }
                }
                false
            });
            if early_is_string {
                // Construct &str fat pointer { ptr, len } from String fields.
                use crate::mir::place::AggregateKind;
                let u8_ptr_ty = Ty::new(
                    TyKind::RawPtr(
                        crate::mir::ty::Mutability::Mutable,
                        Box::new(Ty::new(TyKind::Uint(crate::ast::UintTy::U8), expr.span)),
                    ),
                    expr.span,
                );
                let usize_ty = Ty::new(TyKind::Uint(crate::ast::UintTy::Usize), expr.span);
                let str_ty = Ty::new(
                    TyKind::Ref(
                        crate::mir::ty::Region::Erased,
                        crate::mir::ty::Mutability::Immutable,
                        Box::new(Ty::new(TyKind::Str, expr.span)),
                    ),
                    expr.span,
                );

                // Extract ptr (field 0) and len (field 1) from String.
                let ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, expr.span);
                cx.push_assign(
                    Place::local(ptr_local, expr.span),
                    Rvalue::Use(Operand::Copy(Place {
                        kind: PlaceKind::Projection(
                            Box::new(Place::local(recv_local, receiver.span)),
                            ProjectionElem::Field(FieldId(0), u8_ptr_ty.clone()),
                        ),
                        span: expr.span,
                    })),
                    expr.span,
                );
                let len_local = cx.mir.new_local(usize_ty.clone(), None, expr.span);
                cx.push_assign(
                    Place::local(len_local, expr.span),
                    Rvalue::Use(Operand::Copy(Place {
                        kind: PlaceKind::Projection(
                            Box::new(Place::local(recv_local, receiver.span)),
                            ProjectionElem::Field(FieldId(1), usize_ty.clone()),
                        ),
                        span: expr.span,
                    })),
                    expr.span,
                );

                // Build Tuple { ptr, len } then Cast to &str.
                let tuple_ty = Ty::new(
                    TyKind::Tuple(vec![u8_ptr_ty.clone(), usize_ty.clone()]),
                    expr.span,
                );
                let tuple_local = cx.mir.new_local(tuple_ty.clone(), None, expr.span);
                cx.push_assign(
                    Place::local(tuple_local, expr.span),
                    Rvalue::Aggregate(
                        AggregateKind::Tuple,
                        vec![
                            Operand::Copy(Place::local(ptr_local, expr.span)),
                            Operand::Copy(Place::local(len_local, expr.span)),
                        ],
                    ),
                    expr.span,
                );
                let str_local = cx.mir.new_local(str_ty.clone(), None, expr.span);
                cx.push_assign(
                    Place::local(str_local, expr.span),
                    Rvalue::Cast(
                        crate::mir::place::CastKind::Unsize,
                        Operand::Copy(Place::local(tuple_local, expr.span)),
                        str_ty.clone(),
                    ),
                    expr.span,
                );
                return str_local;
            }
        }
    }

    // Stage 36.1 (v0.24 — TD-SLICE-LEN-MISSING): Early interception for
    // `slice::len()`. Slices (`&[T]`, `[T]`) don't have a prelude impl
    // block (Landin parser doesn't support `impl [T]` syntax yet), so
    // we intercept the `len` method call directly here when the receiver
    // is a slice-typed value. The MIR is identical to `str::len` — both
    // are fat pointers `{ ptr, len: usize }`, and `len()` projects Field(1).
    //
    // Per §1.0 原則 6 (通解 > 特解): one early-interception for all slice
    // types (regardless of element type T). Also handles sized arrays
    // (`[T; N]`) — they coerce to `&[T]` via the receiver being a reference.
    // Per §1.0 原則 4 (报错 > 静默): the early-interception only fires when
    // the receiver is actually a slice — other `len` calls fall through to
    // normal method resolution.
    // Per §12 (最优 > 最小): root-cause fix = primitive intrinsic dispatch
    // (mirrors `str::len` pattern, centralizes in primitive_intrinsics.rs).
    {
        let early_method_name = cx.interner.resolve(&method.name);
        if early_method_name == "len" && args.is_empty() {
            let early_recv_ty = cx.mir.local(recv_local).ty.clone();
            // Detect slice receiver: `&[T]` (Ref to Slice) or `[T]` (Slice directly)
            // or `&[T; N]` (Ref to Array — sized arrays also support .len()).
            let is_slice_receiver = match &early_recv_ty.kind {
                crate::mir::ty::TyKind::Ref(_, _, inner) => matches!(
                    inner.kind,
                    crate::mir::ty::TyKind::Slice(_)
                        | crate::mir::ty::TyKind::Array(_, _)
                        | crate::mir::ty::TyKind::Str
                ),
                crate::mir::ty::TyKind::Slice(_) | crate::mir::ty::TyKind::Array(_, _) => true,
                _ => false,
            };
            if is_slice_receiver {
                // Emit slice::len intrinsic (same MIR as str::len).
                return super::primitive_intrinsics::emit_primitive_intrinsic(
                    cx,
                    super::primitive_intrinsics::PrimitiveIntrinsic::SliceLen,
                    recv_local,
                    expr,
                );
            }
        }
    }

    // Try to resolve the method to a DefId via HIR impl lookup.
    // Stage 13.17: We try multiple strategies to find the receiver's ADT type:
    //   1. Check the MIR local's type (works if typeck has resolved it)
    //   2. Check the HIR receiver expression directly (works for struct
    //      literals like `P { x: 1 }.get()`)
    //   3. If the receiver is a Path (local variable), trace back to
    //      the let binding's initializer type
    let method_def_id: Option<crate::hir::DefId> = cx.hir.and_then(|hir| {
        // Strategy 1: Check MIR local type.
        let recv_ty = cx.mir.local(recv_local).ty.clone();
        if let Some(did) = resolve_inherent_method(hir, cx.interner, &recv_ty, &method.name) {
            return Some(did);
        }
        // Strategy 2: Check HIR receiver expression for ADT construction.
        if let Some(did) = resolve_inherent_method_from_hir_expr(cx, hir, receiver, &method.name) {
            return Some(did);
        }
        // Stage 14.91 (Bug X3 fix): Strategy 3 — Try trait impl method
        // resolution. If the receiver's ADT type has a trait impl that
        // provides the method, resolve to that trait impl method's DefId.
        // This enables static trait dispatch (`impl Trait for Type`).
        // Stage 32.3: Pass owner_def_id for Param(N) trait method resolution.
        if let Some(did) =
            resolve_trait_method(hir, cx.interner, &recv_ty, &method.name, cx.owner_def_id)
        {
            return Some(did);
        }
        // Stage 14.91: Also try HIR-traced type for trait method resolution.
        // The MIR type may be Infer, but HIR tracing can find the ADT type.
        if let HirExprKind::Path(path) = &receiver.kind {
            if let crate::hir::Res::Local(hir_id) = path.res {
                if let Some(init_ty) = find_local_init_type(cx, hir, hir_id) {
                    if let Some(did) = resolve_trait_method(
                        hir,
                        cx.interner,
                        &init_ty,
                        &method.name,
                        cx.owner_def_id,
                    ) {
                        return Some(did);
                    }
                }
            }
        }
        None
    });

    // Stage 18.284 (TD-INTRINSIC-OVERUSE Phase 2-A): Primitive intrinsic
    // dispatch — AFTER method resolution succeeds, check if the resolved
    // method is a primitive intrinsic (str::len, str::is_empty, str::as_bytes).
    // If yes, emit the appropriate MIR directly via `emit_primitive_intrinsic`
    // and return early. Otherwise, fall through to normal call lowering.
    //
    // This replaces the previous early-interception code (3+ scattered
    // `if method_name_str == "len" && is_str { ... }` blocks in the else
    // branch below). The prelude now declares `impl str { fn len(...) ... }`
    // with real signatures, so method resolution finds the prelude impl's
    // DefId. We intercept here, AFTER resolution, to emit the intrinsic MIR.
    //
    // Per §1.0 原則 6 (通解>特解): one dispatch path for all primitive intrinsics.
    // Per §12 (最优>最小): infrastructure for ALL future primitive impls.
    // Per §17.6 (整体性修复): removes scattered str special-casing.
    if let Some(def_id) = method_def_id {
        let intrinsic = cx
            .hir
            .and_then(|hir| lookup_primitive_intrinsic(hir, cx.interner, def_id));
        if let Some(intrinsic) = intrinsic {
            // Stage 56 (v0.7 — TD-STR-INTRINSIC-MARKER-BODIES): str::len
            // now has a real body (`self.len`). Skip intrinsic interception
            // for StrLen — let the real body be lowered normally via
            // fat pointer field access.
            //
            // Per §12 (最优 > 最小): root-cause fix — real body replaces intrinsic.
            // Per §1.0 原則 6 (通解 > 特解): standard method resolution, no intrinsic.
            //
            // NOTE: StrIsEmpty and StrAsBytes still have marker bodies —
            // they keep using intrinsic interception until migrated to real bodies.
            let skip_interception = matches!(
                intrinsic,
                super::primitive_intrinsics::PrimitiveIntrinsic::StrLen
                    | super::primitive_intrinsics::PrimitiveIntrinsic::StrIsEmpty
            );
            if !skip_interception {
                if args.len() == intrinsic.expected_arg_count() {
                    return emit_primitive_intrinsic(cx, intrinsic, recv_local, expr);
                } else {
                    cx.type_errors.push(crate::typeck::TypeError::new(
                        format!(
                            "method `{}` expects {} argument{}, got {}",
                            cx.interner.resolve(&method.name),
                            intrinsic.expected_arg_count(),
                            if intrinsic.expected_arg_count() == 1 {
                                ""
                            } else {
                                "s"
                            },
                            args.len()
                        ),
                        expr.span,
                    ));
                    // Fall through to emit Error placeholder (below).
                }
            }
        }
    }

    // Stage 14.90 (Bug X2 fix): Check if the receiver is a local whose
    // init is a reference expression (e.g., `let r = &p; r.method()`).
    // If so, the receiver is already a reference — don't create a new
    // reference for &self methods. This prevents &&T double-referencing.
    let receiver_is_ref_init = cx.hir.is_some_and(|hir| {
        if let HirExprKind::Path(path) = &receiver.kind {
            if let crate::hir::Res::Local(hir_id) = path.res {
                // Find the init expression for this local
                if let Some(init_expr) = find_local_init_expr(hir, hir_id) {
                    return matches!(&init_expr.kind, HirExprKind::AddrOf { .. });
                }
            }
        }
        false
    });

    // Stage 14.19 (GAP-31): Check if the method takes &self/&mut self.
    // If so, pass the receiver as a reference (Rvalue::Ref) instead of
    // by value (Operand::Copy). This makes mutations propagate to the caller.
    // The codegen Deref+Field handling has been fixed to support this.
    let method_self_kind: Option<crate::ast::SelfKind> =
        method_def_id.and_then(|did| query_method_self_kind(cx.hir?, did));

    let (first_arg_operand, remaining_arg_operands): (Operand, Vec<Operand>) =
        if let Some(crate::ast::SelfKind::Ref(_)) = method_self_kind {
            // &self or &mut self — create a reference to the receiver.
            //
            // Stage 14.73: If the receiver is ALREADY a reference
            // (e.g., `self` inside a &mut self method), pass it
            // directly without creating a new reference. Creating
            // `&self` when self is already `&mut T` produces `&&mut T`,
            // which causes a type mismatch.
            //
            // Per §1.0 原则 6 "通用 > 特例": one rule handles both
            // by-value receivers (create new ref) and by-ref receivers
            // (pass existing ref).
            let recv_ty = cx.mir.local(recv_local).ty.clone();
            let is_already_ref = matches!(&recv_ty.kind, crate::mir::ty::TyKind::Ref(_, _, _))
                || receiver_is_ref_init;

            if is_already_ref {
                // Receiver is already a reference — pass it directly.
                (
                    Operand::Copy(Place::local(recv_local, receiver.span)),
                    arg_locals
                        .iter()
                        .map(|l| Operand::Copy(Place::local(*l, Span::DUMMY)))
                        .collect(),
                )
            } else {
                // Receiver is by-value — create a new reference.
                let bk = match method_self_kind {
                    Some(crate::ast::SelfKind::Ref(crate::ast::Mutability::Mutable)) => {
                        crate::mir::place::BorrowKind::Mut
                    }
                    _ => crate::mir::place::BorrowKind::Shared,
                };
                // Stage 33.1 (TD-VEC-PUSH-GET-MIGRATION): Set ref_ty to the
                // actual reference type (Ref(_, mutability, recv_ty)) instead
                // of a fresh Infer. Was: fresh_infer_ty — caused writeback_
                // fndef_substs to read Infer for the self arg type, preventing
                // T inference for Vec::get(&self) -> T.
                //
                // Per §1.0 原則 3 (显式 > 隐式): ref type is explicit.
                // Per §1.0 原則 6 (通解 > 特解): one path for all &self methods.
                let ref_ty = Ty::new(
                    TyKind::Ref(
                        crate::mir::ty::Region::Erased,
                        match bk {
                            crate::mir::place::BorrowKind::Mut => {
                                crate::mir::ty::Mutability::Mutable
                            }
                            _ => crate::mir::ty::Mutability::Immutable,
                        },
                        Box::new(recv_ty.clone()),
                    ),
                    receiver.span,
                );
                let ref_local = cx.eval_rvalue_to_temp(
                    Rvalue::Ref(
                        crate::mir::ty::Region::Erased,
                        bk,
                        Place::local(recv_local, receiver.span),
                    ),
                    ref_ty,
                    receiver.span,
                );
                (
                    Operand::Copy(Place::local(ref_local, receiver.span)),
                    arg_locals
                        .iter()
                        .map(|l| Operand::Copy(Place::local(*l, Span::DUMMY)))
                        .collect(),
                )
            }
        } else {
            // self by value — pass as Copy (original behavior).
            (
                Operand::Copy(Place::local(recv_local, receiver.span)),
                arg_locals
                    .iter()
                    .map(|l| Operand::Copy(Place::local(*l, Span::DUMMY)))
                    .collect(),
            )
        };

    // Rebuild arg_operands with the correct first arg (ref or copy).
    let arg_operands: Vec<Operand> = std::iter::once(first_arg_operand)
        .chain(remaining_arg_operands)
        .collect();

    // Stage 18.342 (P2 soundness fix): String::as_str intrinsic — moved
    // BEFORE the method_def_id check. Was: only intercepted when method_def_id
    // was None (method not found in prelude). Now that as_str is declared in
    // prelude (Stage 18.342), method_def_id is Some — the normal path would
    // call the `loop {}` body → infinite loop.
    //
    // Fix: intercept as_str BEFORE checking method_def_id. The intrinsic
    // constructs the &str fat pointer from String's fields; the prelude
    // declaration is only for typeck visibility (so users can call s.as_str()).
    //
    // Per §1.0 原則 6 (通解 > 特解): one early-interception point for all
    // methods that need intrinsic implementation (as_str, from_str, push_str).
    // Per §1.0 原則 4 (报错 > 静默): the prelude declaration makes the method
    // visible; the intrinsic provides the real implementation.
    // Per §18 (依赖审查): unblocks TD-INTRINSIC-OVERUSE Phase 2-B condition 2.
    // Stage 31.5 (v0.19): String::as_str() intrinsic REMOVED.
    //
    // The as_str method is now implemented in the prelude using the FatPtrLit
    // syntax: `&str { ptr: self.ptr, len: self.len }`. This replaces the
    // hardcoded intrinsic dispatch (Stage 18.189) — the same MIR pattern
    // (Aggregate(Tuple, [ptr, len]) + Cast(Unsize, &str)) is now triggered
    // from Landin source via `lower_fat_ptr_lit` in expr_operand.rs.
    //
    // Per §1.0 原則 6 (通解 > 特解): standard method resolution handles as_str,
    // no per-method intrinsic dispatch.
    // Per §1.0 原則 5 (去除兼容思维): dead intrinsic code removed.
    // Per §12 (最优 > 最小): root-cause fix via language feature (FatPtrLit).

    // Stage 14.29: Resolve the method's return type from HIR so that
    // chained method calls can resolve methods on the result type.
    // Was: fresh_infer_ty (which meant resolve_inherent_method couldn't
    // find methods on the result — chaining always returned 0).
    let dest_ty = if let Some(did) = method_def_id {
        // Stage 15.6 (perf): use cached lookup — O(1) amortized
        // vs O(n) HIR scan per call. Per §1.0 原则 6 "通用 > 特例":
        // one cache handles all owner kinds.
        cx.query_method_return_type(did)
            .unwrap_or_else(|| cx.fresh_infer_ty(expr.span))
    } else {
        cx.fresh_infer_ty(expr.span)
    };
    let dest = cx.mir.new_local(dest_ty, None, expr.span);
    let cont = cx.new_block();

    if let Some(def_id) = method_def_id {
        // Stage 13.17: Real inherent method call.
        // Emit `TerminatorKind::Call` with `func: Const{ty: FnDef(def_id), val: Uint(def_id)}`.
        // Codegen resolves this via `fn_name_by_def_id` (which maps to
        // `landin_<Type>_<method>` per the driver's naming convention).
        //
        // Stage 47 (v0.6 — TD-METHOD-LEVEL-GENERICS): Infer substs from the
        // receiver type. Previously, substs was always empty (Vec::new()),
        // which meant monomorphization couldn't collect methods with generic
        // params beyond the impl block's own params (e.g., map_err's F).
        //
        // The fix: look up the impl block's generic params, then substitute
        // them from the receiver's type. For `impl<T, E> Result<T, E>`, the
        // receiver type `Result<i32, i32>` produces substs=[i32, i32].
        //
        // For extra method-level generic params (e.g., F in map_err), we use
        // Infer as placeholder — typeck will resolve them later.
        //
        // Per §12 (最优 > 最小): root-cause fix — populate FnDef substs.
        // Per §1.0 原則 6 (通解 > 特解): one inference path for all methods.
        let method_substs = infer_method_substs(cx, def_id, &cx.mir.local(recv_local).ty.clone());
        cx.terminate_kind_and_goto(
            TerminatorKind::Call {
                func: Operand::Constant(Const {
                    ty: Ty::new(
                        TyKind::FnDef(def_id, method_substs.into()),
                        expr.span,
                    ),
                    val: ConstVal::Uint(def_id.as_u32() as u128),
                }),
                args: arg_operands,
                destination: Place::local(dest, expr.span),
                target: Some(cont),
                dyn_trait_call: None,
            },
            cont,
        );
    } else {
        // Stage 33.1: Vec::push/get now in prelude impl. All intrinsic dispatch removed.
        let method_name_str = cx.interner.resolve(&method.name);
        let recv_ty = cx.mir.local(recv_local).ty.clone();
        let is_known_unsupported = matches!(
            &recv_ty.kind,
            crate::mir::ty::TyKind::Error | crate::mir::ty::TyKind::Infer(_)
        );
        if !is_known_unsupported {
            cx.type_errors.push(crate::typeck::TypeError::new(
                format!(
                    "no method `{}` found for type `{}`",
                    method_name_str,
                    cx.format_ty(&recv_ty)
                ),
                expr.span,
            ));
        } else if matches!(&recv_ty.kind, crate::mir::ty::TyKind::Infer(_)) {
            cx.mir
                .deferred_method_calls
                .push(crate::mir::body::DeferredMethodCall {
                    recv_local,
                    method_name: method.name,
                    span: expr.span,
                });
        }
        cx.terminate_kind_and_goto(
            TerminatorKind::Call {
                func: Operand::Constant(Const {
                    ty: Ty::new(TyKind::Error, Span::DUMMY),
                    val: ConstVal::Int(0),
                }),
                args: arg_operands,
                destination: Place::local(dest, expr.span),
                target: Some(cont),
                dyn_trait_call: None,
            },
            cont,
        );
    }
    dest
}

/// Stage 47 (v0.6 — TD-METHOD-LEVEL-GENERICS): Infer the type substitutions
/// for a method call from the receiver type and the impl block's generic params.
///
/// For `impl<T, E> Result<T, E>` with receiver `Result<i32, i32>`:
/// - T → i32, E → i32 → substs = [i32, i32]
///
/// For `impl<T, E, F> Result<T, E>` with receiver `Result<i32, i32>`:
/// - T → i32, E → i32, F → Infer (extra method-level generic, inferred later)
/// - substs = [i32, i32, Infer]
///
/// Per §12 (最优 > 最小): root-cause fix — populate FnDef substs.
/// Per §1.0 原則 6 (通解 > 特解): one inference path for all methods.
fn infer_method_substs(
    cx: &MirLowerCtxt,
    method_def_id: crate::hir::DefId,
    recv_ty: &Ty,
) -> Vec<crate::mir::ty::Ty> {
    let hir = match cx.hir {
        Some(h) => h,
        None => return Vec::new(),
    };

    let impl_generics = find_impl_generics_for_method(hir, method_def_id);

    if impl_generics.is_empty() {
        return Vec::new();
    }

    let recv_substs: Vec<crate::mir::ty::Ty> = match &recv_ty.kind {
        TyKind::Adt(_, substs) => substs.iter().cloned().collect(),
        _ => Vec::new(),
    };

    let mut result = Vec::with_capacity(impl_generics.len());
    for (i, _param) in impl_generics.iter().enumerate() {
        if i < recv_substs.len() {
            result.push(recv_substs[i].clone());
        } else {
            // Extra method-level generic — use a simple Infer placeholder.
            result.push(Ty::new(TyKind::Infer(crate::mir::ty::InferVar::TyVar(crate::mir::ty::TyVid(0))), crate::session::Span::DUMMY));
        }
    }
    result
}

/// Stage 47: Find the impl block's generic params that own the given method.
fn find_impl_generics_for_method(
    hir: &crate::hir::HirCrate,
    method_def_id: crate::hir::DefId,
) -> Vec<crate::lexer::Symbol> {
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
            for impl_item in &impl_block.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    if f.hir_id.owner == method_def_id {
                        // Found the impl block — return its generic params.
                        let generics =
                            crate::hir::generics::find_generics(impl_block.hir_id.owner, hir);
                        return generics
                            .iter()
                            .map(|p| p.name)
                            .collect();
                    }
                }
            }
        }
    }
    Vec::new()
}
