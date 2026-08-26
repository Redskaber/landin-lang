//! Primitive type intrinsic dispatch (Stage 18.284, TD-INTRINSIC-OVERUSE Phase 2-A).
//!
//! After `resolve_inherent_method` succeeds (returns a `DefId`), check if the
//! resolved method is a primitive intrinsic (e.g., `str::len`). If yes, emit
//! the appropriate MIR directly; otherwise, the caller proceeds with normal
//! call lowering.
//!
//! ## Why post-resolution dispatch?
//!
//! Before Stage 18.284, primitive str methods (`str::len`, `str::is_empty`,
//! `str::as_bytes`) were hardcoded as early interception in `expr_variants.rs`
//! (3+ scattered sites checking `method_name == "len"` + `is_str`). This
//! violated §1.0 原則 6 (通解 > 特解) and §17.6 (整体性修复) — each new primitive
//! method required touching multiple files.
//!
//! The post-resolution architecture centralizes dispatch:
//! 1. Prelude declares `impl str { fn len(&self) -> i64 { loop {} } ... }` —
//!    provides real signatures for typeck + user introspection.
//! 2. `resolve_inherent_method` finds the prelude impl's `DefId` via standard
//!    impl-block lookup (extended in Stage 18.284 to recognize primitive
//!    types via `name_of_primitive_ty`).
//! 3. THIS FILE: `lookup_primitive_intrinsic` checks if the resolved `DefId`
//!    is a known primitive intrinsic. If yes, `emit_primitive_intrinsic`
//!    emits the appropriate MIR. If no, the caller falls through to normal
//!    call lowering.
//!
//! ## Identification strategy
//!
//! Identification is by `(impl_block.self_ty name, method_name)` pair — NOT
//! by raw `DefId` (which is unstable across compilations). This is the same
//! identifier pair the early-interception code used, but centralized here.
//!
//! Per §1.0 原則 6 (通解>特例): one lookup for all primitive intrinsics.
//! Per §10 (api-naming): `lookup_primitive_intrinsic` follows `<verb>_<noun>`.
//!
//! ## J1-J6 compliance
//! - J1: mir::lower design unchanged (new sub-responsibility, follows existing
//!   pattern of `intrinsic_lower.rs`).
//! - J2: single responsibility — primitive intrinsic dispatch.
//! - J3: no circular deps (called by `expr_variants.rs` one-way; reads HIR
//!   via `hir.owners` — allowed per §16).
//! - J4: complete primitive intrinsic dispatch in this file. Future primitive
//!   intrinsics (i32::abs, bool::then, char::is_ascii, etc.) also belong here.
//! - J5: stays within mir::lower stage.
//! - J6: LOC driven by responsibility (~180 LOC for table + emit fns).

use crate::hir::*;
use crate::mir::body::*;
use crate::mir::place::*;
use crate::mir::ty::*;
use crate::session::Span;

use super::MirLowerCtxt;

/// A primitive intrinsic kind — identifies which MIR emit path to use.
///
/// Per §1.0 原則 6 (通解>特例): one enum for all primitive intrinsic kinds.
/// Adding a new intrinsic = adding a new variant + an emit branch.
///
/// Variant naming: avoid common prefix (clippy::enum_variant_names).
/// Use descriptive names without the redundant `Str` prefix.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum PrimitiveIntrinsic {
    /// `str::len()` → Field(1) projection of the fat pointer (returns usize).
    StrLen,
    /// `str::is_empty()` → Field(1) + BinOp::Eq with 0 (returns bool).
    StrIsEmpty,
    /// `str::as_bytes()` → no-op (return receiver, same fat pointer layout).
    StrAsBytes,
}

impl PrimitiveIntrinsic {
    /// Expected number of user-supplied arguments (NOT counting `self`).
    ///
    /// Per §1.0 原則 4 (报错>静默): used to validate arg count at dispatch
    /// time. If the caller passes a different number of args, report an
    /// error instead of silently ignoring extra args or missing required ones.
    pub(crate) fn expected_arg_count(&self) -> usize {
        match self {
            PrimitiveIntrinsic::StrLen => 0,
            PrimitiveIntrinsic::StrIsEmpty => 0,
            PrimitiveIntrinsic::StrAsBytes => 0,
        }
    }
}

/// Check if a resolved method call is a primitive intrinsic.
///
/// Walks HIR owners to find the method's owning impl block, then checks
/// if the impl's `self_ty` is a primitive type name (`str`, `i32`, etc.)
/// AND the method name matches a known intrinsic for that type.
///
/// Returns `Some(PrimitiveIntrinsic)` if the method is a primitive
/// intrinsic (caller should emit MIR via `emit_primitive_intrinsic`).
/// Returns `None` if the method is a normal user/prelude method (caller
/// should lower the call body normally).
///
/// Per §1.0 原則 6 (通解>特例): one lookup function for all primitive intrinsics.
/// Per §16: HIR query at MIR-lowering time — sunk via the resolved DefId.
pub(crate) fn lookup_primitive_intrinsic(
    hir: &HirCrate,
    interner: &lasso::Rodeo,
    method_def_id: crate::hir::DefId,
) -> Option<PrimitiveIntrinsic> {
    // Find the method's owning impl block by searching HIR owners.
    // This is O(N) per call — same pattern as `query_method_self_kind`
    // (method_resolution.rs:126). Acceptable cost: one walk per method
    // call, and method resolution is already O(N) in impl blocks.
    for (_, owner) in &hir.owners {
        if let OwnerNode::Item(HirItem::Impl(impl_block)) = owner {
            // Skip trait impls — only inherent impls are primitive intrinsics.
            if impl_block.of_trait.is_some() {
                continue;
            }
            // Check if this impl block contains our method.
            for impl_item in &impl_block.items {
                if let HirImplItem::Fn(f) = impl_item {
                    if f.hir_id.owner == method_def_id {
                        // Found the method. Get the impl block's self_ty name
                        // as a source-language string, then check if (self_ty_name,
                        // method_name) is a known primitive intrinsic.
                        //
                        // Stage 18.285: Two paths for getting self_ty name:
                        //   (1) `impl str { ... }` — self_ty is `HirTyKind::Path("str")`.
                        //       Resolve the path's single segment as the name.
                        //   (2) `impl i32 { ... }` — self_ty is `HirTyKind::Int(I32)`.
                        //       Use `name_of_primitive_hir_ty` to get "i32".
                        //
                        // Per §1.0 原則 6 (通解>特解): one `identify_intrinsic`
                        // call works for both paths — it just needs the type name
                        // as a `&str`.
                        let self_ty_name: Option<&str> = match &impl_block.self_ty.kind {
                            HirTyKind::Path(_, path) if path.segments.len() == 1 => {
                                Some(interner.resolve(&path.segments[0].ident.name))
                            }
                            hir_kind => {
                                super::method_resolution::name_of_primitive_hir_ty(hir_kind)
                            }
                        };
                        if let Some(self_ty_name) = self_ty_name {
                            let method_name = interner.resolve(&f.ident.name);
                            return identify_intrinsic(self_ty_name, method_name);
                        }
                        // self_ty is neither a single-segment Path nor a
                        // primitive variant — not a primitive intrinsic.
                        return None;
                    }
                }
            }
        }
    }
    None
}

/// Map `(self_ty_name, method_name)` to a `PrimitiveIntrinsic`.
///
/// Per §1.0 原則 6 (通解>特例): one match table, not scattered checks.
/// Adding a new intrinsic = adding a match arm.
fn identify_intrinsic(self_ty: &str, method: &str) -> Option<PrimitiveIntrinsic> {
    match (self_ty, method) {
        ("str", "len") => Some(PrimitiveIntrinsic::StrLen),
        ("str", "is_empty") => Some(PrimitiveIntrinsic::StrIsEmpty),
        ("str", "as_bytes") => Some(PrimitiveIntrinsic::StrAsBytes),
        _ => None,
    }
}

/// Emit MIR for a primitive intrinsic method call.
///
/// This is called AFTER `resolve_inherent_method` succeeds and
/// `lookup_primitive_intrinsic` returns `Some(intrinsic)`. The caller has
/// already set up:
/// - `recv_local`: the local holding the receiver value (e.g., `&str` fat pointer)
/// - `dest`: the destination local (with appropriate type from `query_method_return_type`)
/// - `expr.span`: the source span
///
/// This function emits the MIR statements + terminator that implement the
/// intrinsic behavior. The prelude impl's marker body (`loop {}`) is NEVER
/// lowered — this function intercepts before body lowering.
///
/// Per §1.0 原則 6 (通解>特例): one emit function for all primitive intrinsics.
/// Per §2 原則 9 (正确>妥协): emit real MIR (not placeholder Error).
pub(crate) fn emit_primitive_intrinsic(
    cx: &mut MirLowerCtxt,
    intrinsic: PrimitiveIntrinsic,
    recv_local: LocalId,
    expr: &HirExpr,
) -> LocalId {
    match intrinsic {
        PrimitiveIntrinsic::StrLen => emit_str_len(cx, recv_local, expr.span),
        PrimitiveIntrinsic::StrIsEmpty => emit_str_is_empty(cx, recv_local, expr.span),
        PrimitiveIntrinsic::StrAsBytes => emit_str_as_bytes(cx, recv_local, expr.span),
    }
}

/// `str::len()` → Field(1) projection of the fat pointer (returns usize).
///
/// `&str` is a fat pointer `{ ptr, len: usize }`. The length is field 1.
/// Emit: `dest = &recv.1` (copy of the len field).
///
/// Per §1.0 原則 6 (通解>特例): one fat pointer layout for all &str.
fn emit_str_len(cx: &mut MirLowerCtxt, recv_local: LocalId, span: Span) -> LocalId {
    let dest_ty = Ty::new(TyKind::Uint(crate::ast::UintTy::Usize), span);
    let dest = cx.mir.new_local(dest_ty.clone(), None, span);
    let cont = cx.new_block();
    cx.push_assign(
        Place::local(dest, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(1), dest_ty),
            ),
            span,
        })),
        span,
    );
    cx.terminate_and_goto(
        Terminator {
            kind: TerminatorKind::Goto(cont),
            span,
        },
        cont,
    );
    dest
}

/// `str::is_empty()` → `len() == 0` (returns bool).
///
/// Emit:
/// 1. `len_local = &recv.1` (Field(1) projection)
/// 2. `zero_local = 0` (constant i64)
/// 3. `dest = len_local == zero_local` (BinOp::Eq → bool)
///
/// Per §1.0 原則 6 (通解>特例): reuse the len() Field projection pattern.
fn emit_str_is_empty(cx: &mut MirLowerCtxt, recv_local: LocalId, span: Span) -> LocalId {
    // Step 1: Extract len field.
    let len_ty = Ty::new(TyKind::Uint(crate::ast::UintTy::Usize), span);
    let len_local = cx.mir.new_local(len_ty.clone(), None, span);
    cx.push_assign(
        Place::local(len_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(1), len_ty.clone()),
            ),
            span,
        })),
        span,
    );
    // Step 2: Materialize zero constant.
    let zero_local = cx.mir.new_local(len_ty.clone(), None, span);
    cx.push_assign(
        Place::local(zero_local, span),
        Rvalue::Use(Operand::Constant(Const {
            val: crate::mir::ty::ConstVal::Int(0),
            ty: len_ty,
        })),
        span,
    );
    // Step 3: Compare len == 0 → bool.
    let bool_ty = Ty::new(TyKind::Bool, span);
    let dest = cx.mir.new_local(bool_ty, None, span);
    let cont = cx.new_block();
    cx.push_assign(
        Place::local(dest, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Eq,
            Operand::Copy(Place::local(len_local, span)),
            Operand::Copy(Place::local(zero_local, span)),
        ),
        span,
    );
    cx.terminate_and_goto(
        Terminator {
            kind: TerminatorKind::Goto(cont),
            span,
        },
        cont,
    );
    dest
}

/// `str::as_bytes()` → no-op (return receiver).
///
/// `&str` and `&[u8]` have the SAME LLVM fat pointer layout `{ ptr, usize }`.
/// The conversion is a no-op at the MIR level — just return the receiver
/// local. The type system handles the type change (str → [u8]).
///
/// Per §1.0 原則 6 (通解>特例): one fat pointer layout for all byte slices.
fn emit_str_as_bytes(_cx: &mut MirLowerCtxt, recv_local: LocalId, _span: Span) -> LocalId {
    // No-op: return the receiver local directly. The caller's dest local
    // was already created with the appropriate `&[u8]` type by the caller's
    // `query_method_return_type` lookup on the prelude impl's `as_bytes`
    // signature. We just need to return the same local — the caller's
    // terminator setup uses our returned local as the destination.
    recv_local
}
