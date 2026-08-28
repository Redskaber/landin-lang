//! HIR body → MIR body lowering entry points + helpers.
//!
//! Per `docs/stage-committee-process.md` v6.4 §13.4 J1-J6 (Stage 18.130):
//! Extracted from `mod.rs` to satisfy J6 (科学合理粒度) + J2 (单一职责).
//! This file contains all body lowering entry points (`lower_hir_body_to_mir*`,
//! `lower_body*`, `build_synthesized_closure_mir_body`), elision helpers
//! (`collect_region_vids`, `apply_elision_rules`), and `resolve_self_param_type`.
//!
//! ## Sub-responsibility
//! Body lowering: convert each HIR `Body` (expression tree) into a MIR `MirBody`
//! (control flow graph of basic blocks + statements + terminators), including
//! lifetime elision rules and self parameter type resolution.
//!
//! ## J1-J6 compliance
//! - J1: mir::lower design unchanged (single stage, internal sub-responsibility)
//! - J2: this file has one clear responsibility (body lowering)
//! - J3: no circular deps (body_lower calls ty_lower + siblings; mod.rs calls body_lower)
//! - J4: body lowering sub-responsibility is complete in this file
//! - J5: stays within mir::lower stage
//! - J6: LOC driven by responsibility, not arbitrary slicing

use crate::hir::*;
use crate::mir::body::TerminatorKind;
use crate::mir::body::*;
use crate::mir::dyn_trait::DynTraitMIRPlan;
use crate::mir::place::*;
use crate::mir::ty::*;
use crate::session::Span;
use crate::typeck::unify::UnificationTable;
use lasso::Rodeo;

use super::adt_layout;
use super::lower_expr_to_operand;
use super::pattern_bindings;
use super::MirLowerCtxt;
use super::SynthesizedClosureFunction;
// Stage 18.129: type lowering functions extracted to ty_lower.rs
use super::ty_lower::{
    lower_hir_ty_to_mir_ty, lower_hir_ty_to_mir_ty_with_lifetimes,
    lower_hir_ty_to_mir_ty_with_regions,
};

pub fn lower_hir_body_to_mir(body: &Body, interner: &Rodeo, hir: &HirCrate) -> MirBody {
    lower_hir_body_to_mir_with_return_ty(body, interner, hir, None)
}

/// Lower a HIR body to MIR with an explicit return type (from the fn sig).
///
/// When `return_ty` is `Some(ty)`, the return local (LocalId(0)) is
/// initialized with that type instead of a fresh inference variable.
/// This lets the type checker unify the body's value with the declared
/// return type — fixing the "fn sig not unified with body value type"
/// limitation from Stage 2.4d gate review (fix #3).
pub fn lower_hir_body_to_mir_with_return_ty(
    body: &Body,
    interner: &Rodeo,
    hir: &HirCrate,
    return_ty: Option<HirTy>,
) -> MirBody {
    // Stage 15.12: lower_full now returns 4-tuple (mir, unify, type_errors, closures).
    // The convenience wrappers discard unify + type_errors + closures for
    // callers that only need the MirBody (e.g., tests).
    lower_hir_body_to_mir_full(body, interner, hir, return_ty).0
}

/// Full version of `lower_hir_body_to_mir_with_return_ty` that also
/// returns the UnificationTable used during lowering.
///
/// The unify table contains the IntVar/FloatVar allocated for unsuffixed
/// integer/float literals. The type checker needs this table to properly
/// resolve these variables after type inference (defaulting unresolved
/// int vars to i32, float vars to f64).
///
/// Without returning the unify table, the type checker would create a
/// fresh (empty) table and lose track of the IntVars allocated during
/// lowering — causing literals to stay as unresolved Infer vars even
/// after typeck.
///
/// Stage 15.12: Now returns 3-tuple `(MirBody, UnificationTable, Vec<TypeError>)`.
/// The type_errors were previously stored on `MirBody.lower_type_errors` —
/// this was an architectural smell (IR carrying error collection). Now
/// errors are returned from the lowering function, separating concerns.
pub fn lower_hir_body_to_mir_full(
    body: &Body,
    interner: &Rodeo,
    hir: &HirCrate,
    return_ty: Option<HirTy>,
) -> (
    MirBody,
    UnificationTable,
    Vec<crate::typeck::TypeError>,
    std::collections::HashMap<crate::hir::DefId, SynthesizedClosureFunction>,
) {
    // Stage 5.80: delegate to the new entry point with plan = None.
    // Backward-compatible: all existing callers see identical behavior.
    // Stage 16.85: resolver = None (legacy path, no rich error messages).
    // Stage 18.262 (Phase 2e): fn_sigs = None (legacy path, no expected-ty
    // propagation in call args — soundness hole remains for callers
    // using this legacy entry point).
    lower_hir_body_to_mir_full_with_dyn_trait_plan(body, interner, hir, return_ty, None, None, None)
}

/// Stage 5.80: Full lowering entry point with optional `DynTraitMIRPlan`.
///
/// When `plan` is `Some`, attaches it to the `MirLowerCtxt` via
/// `cx.set_dyn_trait_plan(plan.clone())` — this activates the
/// `HirExprKind::MethodCall` dyn Trait path (Stage 5.78). The clone
/// happens once per body (acceptable cost; the plan is small — a few
/// hundred bytes for typical crates).
///
/// When `plan` is `None`, behavior is identical to
/// `lower_hir_body_to_mir_full` (legacy path — no dyn Trait lowering).
///
/// # Driver integration
///
/// The driver (Stage 5.80) builds the plan once via
/// `build_dyn_trait_mir_plan_from_resolver(&trait_resolver, &interner)`
/// before the per-body loop, then passes `Some(&plan)` to this function
/// for each body. This activates end-to-end dyn Trait MIR lowering:
/// HIR `receiver.method(args)` → MIR `TerminatorKind::Call` with Const marker
/// → codegen vtable indirect call IR.
///
/// # §16 compliance
///
/// The plan is built upstream by the driver (which is the sole orchestrator
/// allowed to read TraitResolver). `MirLowerCtxt` does not own a
/// TraitResolver — it receives the plan as data. Data flow:
/// driver → plan → cx → lower → mir::body side-table → codegen.
///
/// # §23 compliance
///
/// `lower_hir_body_to_mir_full_with_dyn_trait_plan` follows the
/// `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>_<noun>_<noun>` pattern.
/// The `_with_dyn_trait_plan` suffix is the Rust API-guidelines convention
/// for "extended variant with additional feature" (mirrors `Vec::with_capacity`,
/// `HashMap::with_hasher`).
///
/// Stage 18.262 (TD-TUPLE-CTOR-CALL-ARG Phase 2e): added `fn_sigs`
/// parameter for call-arg expected-ty propagation. When set,
/// `lower_call_expr` can look up the callee's sig.inputs[i] to thread
/// the expected arg type into each arg's `lower_expr_to_operand`.
/// Per §11.2 (allowed cross-stage access — pre-computed data contract):
/// fn_sigs is built upstream by the driver.
pub fn lower_hir_body_to_mir_full_with_dyn_trait_plan(
    body: &Body,
    interner: &Rodeo,
    hir: &HirCrate,
    return_ty: Option<HirTy>,
    plan: Option<&DynTraitMIRPlan>,
    resolver: Option<&crate::traits::TraitResolver>,
    fn_sigs: Option<&std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>>,
) -> (
    MirBody,
    UnificationTable,
    Vec<crate::typeck::TypeError>,
    std::collections::HashMap<crate::hir::DefId, SynthesizedClosureFunction>,
) {
    let mut cx = MirLowerCtxt::new(interner, body.span);
    cx.hir = Some(hir);

    // Stage 18.105 (S6 fix): Set generic_params from the function's HIR generics.
    // This allows lower_path_generic_args to resolve bare type parameters (e.g., `T`
    // in `Box<T>`) to Param(N) instead of Error.
    let owner_def_id: crate::hir::DefId = body.hir_id.owner;
    cx.generic_params = crate::hir::generics::find_generics(owner_def_id, hir);

    // Stage 16.85: Set resolver for rich error messages (Adt type names).
    if let Some(resolver) = resolver {
        cx.set_resolver(resolver);
    }

    // Stage 18.262 (TD-TUPLE-CTOR-CALL-ARG Phase 2e): Set fn_sigs for
    // call-arg expected-ty propagation. Per §11.2: pre-computed data
    // contract — fn_sigs built upstream by the driver.
    if let Some(fn_sigs) = fn_sigs {
        cx.set_fn_sigs(fn_sigs);
    }

    // Stage 5.80: attach the dyn Trait plan if provided.
    // Per §16: plan was built upstream by the driver via
    // `build_dyn_trait_mir_plan_from_resolver()`. The lower does not
    // query TraitResolver directly.
    if let Some(plan) = plan {
        cx.set_dyn_trait_plan(plan.clone());
    }

    // Stage 15.49: Region counter for assigning fresh RegionVids to
    // reference types during lowering. Each `&T` gets a unique vid,
    // giving the region inference infrastructure real region variables.
    let mut region_counter = 0u32;

    // Stage 15.90: Lifetime elision rule 3 — if the function has exactly
    // one input lifetime (elided or explicit), that lifetime is assigned
    // to all elided output lifetimes.
    //
    // To implement this, we lower params first (collecting their region
    // vids), then lower the return type. If the return type has elided
    // lifetimes, we replace them with the single input lifetime's vid
    // (rule 3) or leave them as fresh vids (rule 1, each gets its own).
    //
    // Rust elision rules (RFC 141):
    //   1. Each elided input lifetime gets its own fresh lifetime.
    //   2. If there's exactly one input lifetime (elided or explicit),
    //      it's assigned to all elided output lifetimes.
    //   3. If there are multiple input lifetimes but one is &self/&mut self,
    //      that lifetime is assigned to all elided output lifetimes.
    //
    // Stage 15.90 implements rule 2 (the most common case). Rule 3 (self)
    // is deferred — requires tracking which param is self.
    //
    // Per §1.0 原則 3 "显式 > 隐式": elision rules are explicitly applied.
    // Per §23: function names follow conventions.

    // Lower param types first, collecting region vids.
    // Stage 15.90/15.91: We need to collect region vids from params
    // for lifetime elision rules 2 and 3.
    // - Rule 2: exactly one input lifetime → use it for output.
    // - Rule 3: multiple input lifetimes, but if one is &self/&mut self,
    //   use the self lifetime for output.
    let mut param_region_vids_collected: Vec<crate::mir::ty::RegionVid> = Vec::new();
    // Stage 15.91: Track the self param's region vid for rule 3.
    let mut self_region_vid: Option<crate::mir::ty::RegionVid> = None;
    // Stage 15.92: Map from lifetime name (Spur) → RegionVid, for explicit
    // lifetime deduplication. References with the same lifetime name share
    // the same vid.
    let mut lifetime_map: std::collections::HashMap<
        crate::lexer::Symbol,
        crate::mir::ty::RegionVid,
    > = std::collections::HashMap::new();
    // Stage 15.90: Store lowered param types so we don't lower them twice
    // (once for elision collection, once for local allocation). Reusing
    // ensures the region vids match.
    let mut lowered_param_types: Vec<Option<Ty>> = Vec::with_capacity(body.params.len());

    // Allocate LocalId(0) as the return value placeholder.
    // We lower the return type AFTER params so elision rules 2/3 can apply.
    let return_mir_ty = {
        // First, lower all param types to collect region vids.
        for param in &body.params {
            if let Some(t) = &param.ty {
                if param.self_kind.is_some() {
                    // Stage 15.91: For &self/&mut self, resolve the self type
                    // and collect its region vid for elision rule 3.
                    let self_ty = resolve_self_param_type(&cx, body, param.self_kind);
                    if let Some(ref mir_ty) = self_ty {
                        // Collect region vids from the self type.
                        let mut self_vids = Vec::new();
                        collect_region_vids(mir_ty, &mut self_vids);
                        if let Some(&vid) = self_vids.first() {
                            self_region_vid = Some(vid);
                            param_region_vids_collected.push(vid);
                        }
                    }
                    lowered_param_types.push(None);
                } else {
                    // Stage 15.92: Use lifetime_map for explicit lifetime
                    // deduplication — references with the same lifetime name
                    // share the same RegionVid.
                    let mir_ty = lower_hir_ty_to_mir_ty_with_lifetimes(
                        t,
                        &mut region_counter,
                        &mut lifetime_map,
                        &cx.generic_params,
                    );
                    // Collect region vids from this param type.
                    collect_region_vids(&mir_ty, &mut param_region_vids_collected);
                    lowered_param_types.push(Some(mir_ty));
                }
            } else {
                lowered_param_types.push(None);
            }
        }
        // Now lower the return type with the accumulated region counter.
        match &return_ty {
            Some(t) => {
                let raw_return_ty = lower_hir_ty_to_mir_ty_with_lifetimes(
                    t,
                    &mut region_counter,
                    &mut lifetime_map,
                    &cx.generic_params,
                );
                // Stage 15.90/15.91: Apply elision rules 2 and 3.
                apply_elision_rules(
                    &raw_return_ty,
                    &param_region_vids_collected,
                    self_region_vid,
                )
            }
            // Stage 18.71 P0-5: For void functions (`fn f() { ... }` with
            // no declared return type), use unit `Tuple([])` as the return
            // local's type — NOT a fresh Infer variable.
            //
            // Previously this used `fresh_infer_ty`, which let
            // `fn f() { return 42; }` unify Infer with Int and silently
            // accept the type mismatch. With explicit unit type, the
            // typeck Assign check fires: place=Tuple([]), rvalue=Int →
            // mismatch error.
            //
            // Per §1.0 原则 3 "显式 > 隐式": void return type is explicit unit.
            // Per §1.0 原则 4 "报错 > 静默": return-with-value in void fn
            // must be reported, not silently accepted.
            None => Ty::new(TyKind::Tuple(vec![]), Span::DUMMY),
        }
    };
    // G5 fix: return_local is assigned multiple times (once per Return
    // terminator path + once at function end), so it must be Mutable.
    // Stage 18.269: clone return_mir_ty before passing to new_local_with_mut
    // because we need to reuse it as expected_ty for body tail expression.
    let return_local = cx.mir.new_local_with_mut(
        return_mir_ty.clone(),
        None,
        Span::DUMMY,
        crate::mir::ty::Mutability::Mutable,
    );
    debug_assert_eq!(return_local, LocalId(0));
    // TEMP DEBUG 18.350 — removed (MirBody has no fn_name field)
    // StorageLive for the return local at function entry.
    cx.mir
        .block_mut(cx.current_block)
        .statements
        .push(Statement {
            kind: StatementKind::StorageLive(return_local),
            span: body.span,
        });

    // Allocate locals for fn params.
    // Stage 15.90: Reuse the lowered param types from the elision pass
    // above (ensures region vids match). Self params are still resolved
    // here because they need the cx context.
    for (param_idx, param) in body.params.iter().enumerate() {
        let ty = if let Some(pre_lowered) =
            lowered_param_types.get(param_idx).and_then(|t| t.as_ref())
        {
            // Reuse the pre-lowered type (non-self params).
            pre_lowered.clone()
        } else {
            match &param.ty {
                Some(t) => {
                    // Stage 13.18: For self params, the parser sets ty to a Path
                    // with "Self" as the segment. This resolves to Res::SelfTy
                    // which lower_hir_ty_to_mir_ty doesn't handle (returns Error).
                    // So for self params, we resolve the type from the impl block's
                    // self_ty directly.
                    // Stage 14.18 (GAP-31): &self/&mut self Ref wrapping was attempted
                    // but reverted — codegen doesn't correctly handle Deref projections
                    // for field access through references. The full fix requires codegen
                    // changes to handle ProjectionElem::Deref in field access paths.
                    // See docs/worklog.md Stage 14.18 for details.
                    if param.self_kind.is_some() {
                        resolve_self_param_type(&cx, body, param.self_kind).unwrap_or_else(|| {
                            lower_hir_ty_to_mir_ty_with_regions(t, &mut region_counter)
                        })
                    } else {
                        lower_hir_ty_to_mir_ty_with_regions(t, &mut region_counter)
                    }
                }
                None => {
                    if param.self_kind.is_some() {
                        resolve_self_param_type(&cx, body, param.self_kind)
                            .unwrap_or_else(|| cx.fresh_infer_ty(Span::DUMMY))
                    } else {
                        cx.fresh_infer_ty(Span::DUMMY)
                    }
                }
            }
        };
        // Stage 15.79 (parser bug fix follow-up): propagate the param
        // pattern's mutability into the local. Previously this used the
        // default `new_local` (Immutable), so `fn f(mut n: i32) { n = 0; }`
        // would fail with AssignImmutable — the param was correctly
        // parsed as `BindingMode::ByValue(Mutable)` but the local was
        // always immutable. Symmetric with the `let mut x` lowering in
        // control_flow.rs (which uses pat_mutability + new_local_with_mut).
        //
        // Per §1.0 原則 3 "显式 > 隐式": mutability is explicitly
        // propagated from pattern to local, not silently dropped.
        // Per §1.0 原則 6 "通用 > 特例": same code path as `let` bindings.
        let mutability = pattern_bindings::pat_mutability(&param.pat);
        let param_local = cx.new_local_with_mut(param.pat.hir_id, ty, None, mutability);
        // StorageLive for each parameter at function entry.
        cx.mir
            .block_mut(cx.current_block)
            .statements
            .push(Statement {
                kind: StatementKind::StorageLive(param_local),
                span: param.span,
            });
    }

    // Lower the body's value expression into the return local.
    // Stage 18.269 (TD-GENERIC-FN-RETURN-EXPECTED-TY Phase 2d): thread
    // the fn's return type as expected_ty into the body's value
    // expression lowering. This closes the soundness hole where
    // `fn make() -> Holder<i32> { Holder(true) }` silently accepted
    // type mismatches because the body's Holder(true) was lowered with
    // expected_ty=None (so Holder's substs stayed as Param(T), which
    // unifies with anything).
    //
    // Per §17.6 (缺陷纳入 — same class as TD-TUPLE-CTOR-CALL-ARG +
    // TD-STRUCT-LITERAL-FIELD-EXPECTED-TY): when one expected-ty
    // propagation bug is found, audit ALL similar paths until no
    // more found.
    // Per §1.0 原則 6 (通解 > 特解): one return_ty-based expected_ty
    // propagation path for all fn body tail expressions.
    // Per §2 原則 9 (正确 > 妥协): proper expected-ty propagation at
    // lower time, not relying on typeck back-propagation.
    let return_is_unit_for_expected =
        matches!(&return_mir_ty.kind, TyKind::Tuple(tys) if tys.is_empty());
    let return_ty_for_expected: Option<Ty> = if return_is_unit_for_expected {
        // For void fns (no declared return type), don't thread expected_ty
        // (unit type would unify with anything, defeating the purpose).
        None
    } else {
        Some(return_mir_ty.clone())
    };
    let value_local = lower_expr_to_operand(&mut cx, &body.value, return_ty_for_expected.as_ref());

    // Stage 14.23: If the current block is already terminated (e.g. by a
    // `return` statement inside the body), skip the assignment to the return
    // local. The return local was already assigned by the `return` expression's
    // lowering. Without this check, we'd emit an assignment AFTER the Return
    // terminator, which is dead code that overwrites the return value with
    // an uninitialized local.
    //
    // Stage 18.71 P0-5: For void functions (`fn f() { ... }` with no declared
    // return type), the return local's type is unit `Tuple([])`. We must NOT
    // assign the body's trailing expression to the return local — instead,
    // the trailing expression is evaluated for side effects (like a statement)
    // and its result is discarded. This matches Rust's behavior: in a void
    // function, the trailing expression is treated as a discarded statement.
    //
    // Why always skip for void fns: The trailing expression's type may be
    // Infer(IntVar) (unsuffixed int literal) which would later resolve to
    // i32. Assigning it to a unit return local would trigger a spurious
    // type mismatch in post_check_statement. By skipping the assign for
    // all void fns, we correctly handle `fn f() { 42 }`, `fn f() { () }`,
    // `fn f() { add(1, 2) }`, etc.
    //
    // For non-void fns (`fn f() -> T { expr }`), the assign happens
    // normally, and post_check_statement catches any type mismatch
    // (e.g., `fn f() -> i32 { true }`).
    //
    // Per §1.0 原則 9 "正确 > 妥协": match Rust's semantics for void fns.
    let return_ty = cx.mir.local(return_local).ty.clone();
    let return_is_unit = matches!(&return_ty.kind, TyKind::Tuple(tys) if tys.is_empty());
    let skip_assign = cx.is_terminated() || return_is_unit;

    // Stage 18.336 (P1 soundness fix): When the return type is `()` (unit) AND
    // the trailing expression has a CONCRETE non-unit type, do NOT skip the
    // assign ONLY if the type is a primitive scalar (Int/Uint/Bool/Float) or
    // Adt (struct/enum). For Ref/Ptr/FnPtr types, keep the skip — Rust's
    // behavior for `fn f() { "hello" }` is to discard with a warning, not
    // a hard error. Only "scalar value where unit is expected" should error.
    //
    // Per §1.0 原則 4 (报错 > 静默): concrete scalar/struct mismatches must
    // be reported (e.g., `fn foo() -> () { 42i64 }`, `fn foo() -> () { true }`).
    // Per §1.0 原則 9 (正确 > 妥协): match Rust's behavior for Ref/Ptr (discard).
    // Per §20 (iterative audit): found via §20 Round 5 audit (TD-TYPECK-ZST-RETURN).
    let skip_assign = if skip_assign && return_is_unit {
        // Check if the trailing value is a concrete non-unit, non-Ref/Ptr type.
        let value_ty = cx.mir.local(value_local).ty.clone();
        let value_is_infer = matches!(&value_ty.kind, TyKind::Infer(_));
        let value_is_unit = matches!(&value_ty.kind, TyKind::Tuple(tys) if tys.is_empty());
        // Ref/Ptr/FnPtr/FnDef/Str are "discardable" — Rust allows them as trailing
        // expressions in void fns (with a warning, not an error).
        let value_is_discardable = matches!(
            &value_ty.kind,
            TyKind::Ref(_, _, _)
                | TyKind::RawPtr(_, _)
                | TyKind::FnPtr(_)
                | TyKind::FnDef(_, _)
                | TyKind::Str
                | TyKind::Slice(_)
        );
        // Skip only if rvalue is Infer, unit, or discardable (Ref/Ptr/etc.).
        // Concrete scalar types (Int/Bool/Float) and Adt (struct/enum) should
        // NOT skip — they're real mismatches.
        value_is_infer || value_is_unit || value_is_discardable
    } else {
        skip_assign
    };

    if !skip_assign {
        // Stage 16.06: Use Operand::Move for the function body's tail
        // expression. The tail value semantically moves into the return
        // slot (LocalId(0)). Using Operand::Copy was unsound for non-Copy
        // types (e.g., structs with `impl Drop`) — the borrow checker
        // would reject "use of moved value: does not implement Copy".
        // With field-level Copy derivation (Stage 16.06), non-Copy types
        // are now correctly identified, so we must use Move for correctness.
        // For Copy types, Move is equivalent to Copy (no move recorded).
        cx.push_assign(
            Place::local(return_local, Span::DUMMY),
            Rvalue::Use(Operand::Move(Place::local(value_local, Span::DUMMY))),
            body.span,
        );
    }

    // Emit StorageDead for all locals (except the return local) before
    // the function returns. This is a conservative approximation —
    // ideally we'd emit StorageDead at each local's scope end, but that
    // requires scope tracking (Stage 3). For now, all locals die at
    // function return.
    //
    // We skip LocalId(0) (the return local) because it's still alive
    // at the point of Return.
    //
    // Stage 15.62: Emit StorageDead in REVERSE declaration order so that
    // `elaborate_drops` produces `Drop` terminators in reverse declaration
    // order — matching Rust's drop semantics (last-declared local is
    // dropped first). Previously, forward emission produced forward drop
    // order, which was incorrect.
    //
    // Per §1.0 原則 6 "通用 > 特例": one rule (reverse iteration) handles
    // all drop-ordering cases — no special-casing per local type.
    // Per §23: no API change (internal MIR lowering detail).
    let local_count = cx.mir.local_decls.len();
    for i in (1..local_count).rev() {
        cx.mir
            .block_mut(cx.current_block)
            .statements
            .push(Statement {
                kind: StatementKind::StorageDead(LocalId(i as u32)),
                span: body.span,
            });
    }

    // Terminate the current block with Return.
    cx.terminate_kind(TerminatorKind::Return);

    // Stage 3.47 (L-PIPE-1 closure per §16): sink ADT layouts from HIR into
    // MIR's `adt_layouts` side-table. This lets codegen resolve
    // `TyKind::Adt(def_id, _)` storage layouts **without reading HIR** —
    // closing the pipeline-coupling debt carried since Stage 3.30.
    //
    // We walk every local's type and register any Adt we encounter. The
    // walk is shallow (we don't recurse into nested Adts — they'll be
    // registered when their own DefId appears in some local's type). This
    // covers all Adt construction paths:
    //   - `lower_hir_ty_to_mir_ty` (free fn — params, returns, let bindings)
    //   - Direct `TyKind::Adt(def_id, …)` construction in lower_expr paths
    //     (struct/enum literals, Call→Aggregate rewrite)
    //   - Field types sunk into `AggregateKind::Adt`
    adt_layout::populate_adt_layouts(&mut cx.mir, hir);

    // Extract the unify table + type_errors before consuming cx.
    // Stage 15.12: type_errors now returned from the lowering function
    // (was stored on MirBody.lower_type_errors — mixed IR + error collection).
    // Stage 16.13: synthesized_closure_functions also returned for codegen.
    let unify = std::mem::take(&mut cx.unify);
    let type_errors = std::mem::take(&mut cx.type_errors);
    let synthesized_closure_functions = std::mem::take(&mut cx.synthesized_closure_functions);
    (cx.mir, unify, type_errors, synthesized_closure_functions)
}

/// Stage 16.14 (Task 10 Step 2): Build a MIR body for a synthesized closure
/// `call` function.
///
/// Given the `SynthesizedClosureFunction` metadata (collected during the
/// main function's MIR lowering), this function builds a separate MirBody
/// representing the closure's `call` function:
///
/// ```text
/// fn closure_call_fn_N(self: Closure_N, param1: T1, param2: T2, ...) -> Ret {
///     // Extract captures from self:
///     local_cap_0 = Projection(self, Field(0, cap_ty_0))
///     local_cap_1 = Projection(self, Field(1, cap_ty_1))
///     ...
///     // Lower closure body (references to captures resolve to local_cap_i)
///     <body>
///     return <body_result>
/// }
/// ```
///
/// Stage 16.29 (通解 — Typeck on synthesized closure MIR bodies):
/// This function now takes `unify: UnificationTable` as input (the SHARED
/// unify table from the main body) and returns it back. The closure MIR
/// body's fresh Infer vars are allocated from this shared table, so they
/// don't collide with the closure_struct_ty's Infer vars (which were
/// created during main body lowering).
///
/// Stage 16.29 (nested closures): This function ALSO returns any nested
/// `synthesized_closure_functions` discovered while lowering the closure
/// body (e.g., `|| || x` — the outer closure's body contains an inner
/// closure literal). The driver processes these recursively.
///
/// The driver flow:
///   1. Lower main body → main_mir, main_unify, synthesized_closures
///   2. For each closure: pass main_unify into this function, get back
///      (closure_mir, main_unify, errors, nested_closures). main_unify is
///      updated with the closure's fresh Infer vars.
///   3. Typeck main body with main_unify → resolves closure_struct_ty's
///      Infer vars.
///   4. Typeck closure MIR bodies with main_unify → resolves closure
///      body's Infer vars.
///   5. Recursively process nested closures (from step 2).
///
/// Per §1.0 原則 6 "通用 > 特例": one unify table for main body + all
/// closures (including nested) — no special-case handling per closure type.
/// Per §1.0 原則 9 "正确 > 妥协": fix the root cause (unify table
/// isolation), not the symptom (cycle detection in resolve_ty_var).
/// Per §16: this function reads HIR (the closure body) — allowed during
/// MIR lowering.
/// Per §23: `build_synthesized_closure_mir_body` follows
/// `<verb>_<adj>_<noun>_<noun>` pattern.
pub fn build_synthesized_closure_mir_body(
    func: &SynthesizedClosureFunction,
    interner: &Rodeo,
    hir: &HirCrate,
    unify: UnificationTable,
    closure_def_id_counter: u32,
) -> (
    MirBody,
    UnificationTable,
    Vec<crate::typeck::TypeError>,
    std::collections::HashMap<crate::hir::DefId, SynthesizedClosureFunction>,
    u32,
) {
    let mut cx =
        MirLowerCtxt::new_with_unify(interner, func.body.span, unify, closure_def_id_counter);
    cx.hir = Some(hir);

    // Stage 16.20: MirBody::new() creates an empty local_decls vec.
    // We need to explicitly create LocalId(0) as the return local FIRST,
    // then LocalId(1) as `self`, then LocalId(2+) as closure params.
    //
    // LocalId(0): return local (fresh infer type — will be resolved
    // from the body expression type by typeck writeback).
    //
    // Stage 16.31 (通解 — return local mutability): The return local
    // is Mutable, matching the main body's lowering (G5 fix). This
    // allows `return expr;` inside closure bodies to assign to
    // LocalId(0) without borrowck flagging "cannot assign twice to
    // immutable variable" (the first assign is the body result, the
    // second is the early return — both are valid writes to the
    // mutable return local).
    let return_ty = cx.fresh_infer_ty(func.body.span);
    let return_local = cx.mir.new_local_with_mut(
        return_ty,
        None,
        func.body.span,
        crate::mir::ty::Mutability::Mutable,
    );
    debug_assert_eq!(return_local, crate::mir::place::LocalId(0));

    // LocalId(1): `self` parameter — the closure struct.
    let self_local = cx
        .mir
        .new_local(func.closure_struct_ty.clone(), None, func.body.span);
    // Note: LocalId(0) is the return local, LocalId(1) is `self`.

    // LocalId(2), (3), ...: closure parameters.
    let mut param_locals: Vec<crate::mir::place::LocalId> = Vec::new();
    for param in &func.params {
        let ty = cx.fresh_infer_ty(param.pat.span);
        let local = cx.mir.new_local(ty, None, param.pat.span);
        // Register param's hir_id → local in local_map.
        cx.local_map.insert(param.pat.hir_id, local);
        param_locals.push(local);
    }

    // Extract captures from `self` and register their hir_ids.
    // Stage 16.23: `self` is passed as a pointer (OpaquePtr) in codegen.
    // To access capture fields, we need to first Deref the pointer, then
    // project the field. This generates GEP in LLVM:
    //   getelementptr inbounds { ty0, ty1, ... }, ptr %self, i32 0, i32 field_idx
    //
    // Stage 16.31 (通解 — capture mutability): The extract local is
    // created with the captured variable's mutability (from the outer
    // scope). This allows the closure body to mutate the captured
    // variable (e.g., `x += 1` where `x` is a captured `mut`).
    // Without this, borrowck would flag the assignment as
    // "cannot assign twice to immutable variable".
    for (cap_hir_id, field_idx, cap_ty, cap_mutability) in &func.captures {
        let extract_local =
            cx.mir
                .new_local_with_mut(cap_ty.clone(), None, func.body.span, *cap_mutability);
        // Assign: extract_local = Copy(Projection(Projection(self, Deref), Field(field_idx, cap_ty)))
        cx.push_assign(
            crate::mir::place::Place::local(extract_local, func.body.span),
            crate::mir::place::Rvalue::Use(crate::mir::place::Operand::Copy(
                crate::mir::place::Place {
                    kind: crate::mir::place::PlaceKind::Projection(
                        Box::new(crate::mir::place::Place {
                            kind: crate::mir::place::PlaceKind::Projection(
                                Box::new(crate::mir::place::Place::local(
                                    self_local,
                                    func.body.span,
                                )),
                                crate::mir::place::ProjectionElem::Deref,
                            ),
                            span: func.body.span,
                        }),
                        crate::mir::place::ProjectionElem::Field(
                            crate::mir::place::FieldId(*field_idx),
                            cap_ty.clone(),
                        ),
                    ),
                    span: func.body.span,
                },
            )),
            func.body.span,
        );
        // Register captured binding's hir_id → extract_local.
        cx.local_map.insert(*cap_hir_id, extract_local);
    }

    // Lower the closure body expression into a local.
    let body_result_local = lower_expr_to_operand(&mut cx, &func.body, None);

    // Assign the body result to the return local (LocalId(0)).
    if !cx.is_terminated() {
        cx.push_assign(
            crate::mir::place::Place::local(
                crate::mir::place::LocalId(0),
                crate::session::Span::DUMMY,
            ),
            crate::mir::place::Rvalue::Use(crate::mir::place::Operand::Move(
                crate::mir::place::Place::local(body_result_local, crate::session::Span::DUMMY),
            )),
            func.body.span,
        );
    }

    // Terminate with Return.
    cx.terminate_kind(crate::mir::body::TerminatorKind::Return);

    // Populate adt_layouts (same as main function lowering).
    adt_layout::populate_adt_layouts(&mut cx.mir, hir);

    // Stage 16.17: Set the DefId on the MirBody so codegen can resolve
    // the function name via fn_name_by_def_id.
    cx.mir.def_id = Some(func.def_id);

    // Stage 16.29 (通解): Return the unify table and type errors so the
    // driver can run TypeChecker::with_unify + check_mir_body_with_tables
    // on this MIR body. This resolves all Infer types (return type, param
    // types) — eliminating the typeck gap that forced the
    // `has_complex_captures` special-case routing.
    //
    // Stage 16.29 (nested closures): Also return any nested
    // synthesized_closure_functions discovered while lowering the closure
    // body. The driver processes these recursively.
    let unify = std::mem::take(&mut cx.unify);
    let type_errors = std::mem::take(&mut cx.type_errors);
    let nested_closures = std::mem::take(&mut cx.synthesized_closure_functions);
    let closure_def_id_counter = cx.closure_def_id_counter();
    (
        cx.mir,
        unify,
        type_errors,
        nested_closures,
        closure_def_id_counter,
    )
}

// ================================================================
// Stage 3.65: convenience aliases
// ================================================================
//
// Per `docs/develop/v0/api-naming-standard.md` §2.2, each stage should
// expose a `<verb>_<noun>` free-function entry point. The MIR lower
// stage historically used the verbose `lower_hir_body_to_mir_*` names
// (which are explicit but break the verb-object pattern set by
// `lower_crate` / `resolve_crate` / `codegen_crate`). These thin
// wrappers provide the short form without removing the long form.

/// Stage 3.65: convenience alias for `lower_hir_body_to_mir`.
///
/// Mirrors the entry-point style of `hir::lower::lower_crate` (verb_noun).
/// The long-form `lower_hir_body_to_mir` remains available for callers
/// who prefer the explicit name.
pub fn lower_body(body: &Body, interner: &Rodeo, hir: &HirCrate) -> MirBody {
    lower_hir_body_to_mir(body, interner, hir)
}

/// Stage 3.65: convenience alias for `lower_hir_body_to_mir_full`.
///
/// Returns both the `MirBody` and the `UnificationTable` (the latter is
/// passed to `TypeChecker::with_unify` so typeck can resolve inference
/// variables created during lowering).
///
/// Stage 15.12: Now also returns `Vec<TypeError>` (was stored on MirBody).
pub fn lower_body_full(
    body: &Body,
    interner: &Rodeo,
    hir: &HirCrate,
    return_ty: Option<HirTy>,
) -> (
    MirBody,
    UnificationTable,
    Vec<crate::typeck::TypeError>,
    std::collections::HashMap<crate::hir::DefId, SynthesizedClosureFunction>,
) {
    lower_hir_body_to_mir_full(body, interner, hir, return_ty)
}

/// Stage 15.90: Collect all `RegionVid`s from a `Ty`'s reference types.
///
/// Recursively walks the type and collects every `Region::Var(vid)` found
/// in `TyKind::Ref` variants. Used to gather input lifetime vids for
/// lifetime elision rule 2 (output lifetime = input lifetime).
fn collect_region_vids(ty: &Ty, vids: &mut Vec<crate::mir::ty::RegionVid>) {
    use crate::mir::ty::{Region, TyKind};
    match &ty.kind {
        TyKind::Ref(region, _, inner) => {
            if let Region::Var(vid) = region {
                vids.push(*vid);
            }
            collect_region_vids(inner, vids);
        }
        TyKind::RawPtr(_, inner) => {
            collect_region_vids(inner, vids);
        }
        TyKind::Array(inner, _) | TyKind::Slice(inner) => {
            collect_region_vids(inner, vids);
        }
        TyKind::Tuple(tys) => {
            for t in tys {
                collect_region_vids(t, vids);
            }
        }
        TyKind::FnPtr(sig) => {
            for t in &sig.inputs {
                collect_region_vids(t, vids);
            }
            collect_region_vids(&sig.output, vids);
        }
        _ => {}
    }
}

/// Stage 15.90/15.91: Apply lifetime elision rules to a return type.
///
/// Implements RFC 141 elision rules:
///   - Rule 2: If there's exactly one input lifetime (elided or explicit),
///     it's assigned to all elided output lifetimes.
///   - Rule 3: If there are multiple input lifetimes but one is `&self`/
///     `&mut self`, that lifetime is assigned to all elided output lifetimes.
///
/// This function replaces all `Region::Var(vid)` in the return type with
/// the selected input lifetime's vid.
///
/// Per §1.0 原則 3 "显式 > 隐式": elision rules are explicitly applied.
/// Per §23: function name follows `<verb>_<noun>_<noun>` pattern.
fn apply_elision_rules(
    return_ty: &Ty,
    input_vids: &[crate::mir::ty::RegionVid],
    self_vid: Option<crate::mir::ty::RegionVid>,
) -> Ty {
    use crate::mir::ty::{Region, RegionVid, TyKind};

    // Determine which input lifetime to use for the output.
    let target_vid = if input_vids.len() == 1 {
        // Rule 2: exactly one input lifetime → use it.
        Some(input_vids[0])
    } else if input_vids.len() > 1 {
        // Rule 3: multiple input lifetimes, but if one is &self/&mut self,
        // use the self lifetime.
        self_vid
    } else {
        // No input lifetimes → no elision (keep fresh output vids).
        None
    };

    match target_vid {
        None => return_ty.clone(),
        Some(target_vid) => {
            // Recursively replace all region vids in the return type.
            fn replace_regions(ty: &Ty, target_vid: RegionVid) -> Ty {
                let span = crate::session::Span::DUMMY;
                match &ty.kind {
                    TyKind::Ref(_, mutability, inner) => Ty::new(
                        TyKind::Ref(
                            Region::Var(target_vid),
                            *mutability,
                            Box::new(replace_regions(inner, target_vid)),
                        ),
                        span,
                    ),
                    TyKind::RawPtr(mutability, inner) => Ty::new(
                        TyKind::RawPtr(*mutability, Box::new(replace_regions(inner, target_vid))),
                        span,
                    ),
                    TyKind::Array(inner, count) => Ty::new(
                        TyKind::Array(Box::new(replace_regions(inner, target_vid)), count.clone()),
                        span,
                    ),
                    TyKind::Slice(inner) => Ty::new(
                        TyKind::Slice(Box::new(replace_regions(inner, target_vid))),
                        span,
                    ),
                    TyKind::Tuple(tys) => Ty::new(
                        TyKind::Tuple(tys.iter().map(|t| replace_regions(t, target_vid)).collect()),
                        span,
                    ),
                    TyKind::FnPtr(sig) => Ty::new(
                        TyKind::FnPtr(crate::mir::ty::Sig {
                            inputs: sig
                                .inputs
                                .iter()
                                .map(|t| replace_regions(t, target_vid))
                                .collect(),
                            output: Box::new(replace_regions(&sig.output, target_vid)),
                            abi: sig.abi,
                            is_unsafe: sig.is_unsafe,
                        }),
                        span,
                    ),
                    _ => ty.clone(),
                }
            }
            replace_regions(return_ty, target_vid)
        }
    }
}

/// Stage 15.92: Lower a HIR type to MIR type with explicit lifetime tracking.
///
/// This is a wrapper around `lower_hir_ty_to_mir_ty_with_regions` that
/// adds explicit lifetime deduplication via `lifetime_map`. When an
/// explicit lifetime is encountered (e.g., `'a`), the function looks up
/// the lifetime name in `lifetime_map`. If found, reuses the existing
/// vid; if not found, creates a fresh vid and records it in the map.
///
/// This ensures references with the same explicit lifetime name share
/// the same region vid, which is what the region inference needs to
/// enforce lifetime constraints correctly.
///
/// Per §23: `_with_lifetimes` suffix follows convention.
/// Per §1.0 原則 3 "显式 > 隐式": explicit lifetimes are tracked by name.
fn resolve_self_param_type(
    cx: &MirLowerCtxt,
    body: &Body,
    self_kind: Option<crate::ast::SelfKind>,
) -> Option<crate::mir::ty::Ty> {
    let hir = cx.hir?;
    // The body's owner DefId — for impl methods, this is the HirFn's owner.
    let _owner_def_id = body.hir_id.owner;

    // Helper: wrap an ADT type as &T/&mut T based on self_kind.
    let wrap_with_ref = |adt_ty: crate::mir::ty::Ty| -> crate::mir::ty::Ty {
        match self_kind {
            Some(crate::ast::SelfKind::Ref(mutability)) => {
                let mir_mut = match mutability {
                    crate::ast::Mutability::Mutable => crate::mir::ty::Mutability::Mutable,
                    crate::ast::Mutability::Immutable => crate::mir::ty::Mutability::Immutable,
                };
                crate::mir::ty::Ty::new(
                    crate::mir::ty::TyKind::Ref(
                        crate::mir::ty::Region::Erased,
                        mir_mut,
                        Box::new(adt_ty),
                    ),
                    body.span,
                )
            }
            // self by value — no wrapping
            _ => adt_ty,
        }
    };

    // Search all owners for an Impl block that contains this method.
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
            // Check if this impl block contains a method whose body matches.
            for impl_item in &impl_block.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    if f.body
                        == Some(crate::hir::BodyId {
                            owner: crate::hir::OwnerId(body.hir_id.owner),
                        })
                    {
                        // Found the owning impl block! Lower its self_ty.
                        // Stage 14.19 (GAP-31): For &self/&mut self, wrap the
                        // type in TyKind::Ref so the self param is a reference.
                        // This makes mutations propagate to the caller.
                        // The codegen Deref+Field handling has been fixed in
                        // mir_translation.rs to support this correctly.
                        let adt_ty = lower_hir_ty_to_mir_ty(&impl_block.self_ty);
                        return Some(wrap_with_ref(adt_ty));
                    }
                }
            }
        }
    }

    // Stage 14.97 (Bug Y1 fix): Trait default body methods.
    //
    // If no impl block owns this body, check if a Trait block owns it
    // (i.e., this is a trait default body). For trait default bodies, the
    // self type is `Self` — a type parameter that's unknown without
    // monomorphization. For v0.1, we use a single-impl heuristic: if exactly
    // one impl of the trait exists in the program, use that impl's self_ty
    // as the specialization type. This is correct for the common case of
    // `trait T { fn f(&self) {...} } impl T for Type { ... }` with one impl.
    //
    // Limitation: If multiple impls exist, we use the first impl's self_ty.
    // This is wrong for the other impls but is a v0.1 limitation (full
    // monomorphization is v0.2+ work). The alternative (returning None and
    // leaving self as Infer) causes worse failures (LLVM crashes).
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
            for trait_item in &t.items {
                if let crate::hir::HirTraitItem::Fn(f) = trait_item {
                    if f.body
                        == Some(crate::hir::BodyId {
                            owner: crate::hir::OwnerId(body.hir_id.owner),
                        })
                    {
                        // Found the owning trait! Find impls of this trait.
                        let trait_name = t.ident.name;
                        let impls: Vec<_> = hir
                            .owners
                            .iter()
                            .filter_map(|(_, o)| {
                                if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(
                                    impl_block,
                                )) = o
                                {
                                    if impl_block
                                        .of_trait
                                        .as_ref()
                                        .and_then(|p| p.segments.last().map(|s| s.ident.name))
                                        == Some(trait_name)
                                    {
                                        return Some(impl_block);
                                    }
                                }
                                None
                            })
                            .collect();

                        // Use the first impl's self_ty as the specialization type.
                        if let Some(impl_block) = impls.first() {
                            let adt_ty = lower_hir_ty_to_mir_ty(&impl_block.self_ty);
                            return Some(wrap_with_ref(adt_ty));
                        }
                        // No impls exist — fall through to return None.
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod stage15_90_tests {
    use super::*;
    use crate::mir::ty::{Mutability, Region, RegionVid, TyKind};

    /// Stage 15.90: Verify `collect_region_vids` collects vids from Ref types.
    #[test]
    fn collect_region_vids_basic() {
        // &i32 with Region::Var(5)
        let ty = Ty::new(
            TyKind::Ref(
                Region::Var(RegionVid(5)),
                Mutability::Immutable,
                Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
            ),
            Span::DUMMY,
        );
        let mut vids = Vec::new();
        collect_region_vids(&ty, &mut vids);
        assert_eq!(vids, vec![RegionVid(5)]);
    }

    /// Stage 15.90: Verify `collect_region_vids` collects from nested types.
    #[test]
    fn collect_region_vids_nested() {
        // &(&i32, &i32) with regions 1 and 2
        let inner1 = Ty::new(
            TyKind::Ref(
                Region::Var(RegionVid(1)),
                Mutability::Immutable,
                Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
            ),
            Span::DUMMY,
        );
        let inner2 = Ty::new(
            TyKind::Ref(
                Region::Var(RegionVid(2)),
                Mutability::Immutable,
                Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
            ),
            Span::DUMMY,
        );
        let tuple = Ty::new(TyKind::Tuple(vec![inner1, inner2]), Span::DUMMY);
        let mut vids = Vec::new();
        collect_region_vids(&tuple, &mut vids);
        assert_eq!(vids, vec![RegionVid(1), RegionVid(2)]);
    }

    /// Stage 15.90: Verify `apply_elision_rules` with single input lifetime (rule 2).
    #[test]
    fn apply_elision_rule_2_single_input() {
        // Return type: &i32 with Region::Var(10) (fresh output vid)
        let return_ty = Ty::new(
            TyKind::Ref(
                Region::Var(RegionVid(10)),
                Mutability::Immutable,
                Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
            ),
            Span::DUMMY,
        );
        // Input: single lifetime vid 3
        let input_vids = vec![RegionVid(3)];
        let result = apply_elision_rules(&return_ty, &input_vids, None);
        // The output lifetime should be replaced with vid 3.
        match &result.kind {
            TyKind::Ref(region, _, _) => {
                assert_eq!(region, &Region::Var(RegionVid(3)));
            }
            _ => panic!("expected Ref"),
        }
    }

    /// Stage 15.90: Verify `apply_elision_rules` with multiple input lifetimes
    /// and no self → does NOT apply (keeps original output lifetime).
    #[test]
    fn apply_elision_rule_2_multiple_inputs_no_self() {
        let return_ty = Ty::new(
            TyKind::Ref(
                Region::Var(RegionVid(10)),
                Mutability::Immutable,
                Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
            ),
            Span::DUMMY,
        );
        // Input: multiple lifetime vids, no self
        let input_vids = vec![RegionVid(1), RegionVid(2)];
        let result = apply_elision_rules(&return_ty, &input_vids, None);
        // The output lifetime should NOT be replaced (keeps vid 10).
        match &result.kind {
            TyKind::Ref(region, _, _) => {
                assert_eq!(region, &Region::Var(RegionVid(10)));
            }
            _ => panic!("expected Ref"),
        }
    }

    /// Stage 15.91: Verify `apply_elision_rules` with multiple input lifetimes
    /// AND self lifetime (rule 3) → uses self lifetime for output.
    #[test]
    fn apply_elision_rule_3_self_lifetime() {
        let return_ty = Ty::new(
            TyKind::Ref(
                Region::Var(RegionVid(10)),
                Mutability::Immutable,
                Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
            ),
            Span::DUMMY,
        );
        // Input: multiple lifetime vids (1=self, 2=other param)
        let input_vids = vec![RegionVid(1), RegionVid(2)];
        // Self lifetime is vid 1
        let self_vid = Some(RegionVid(1));
        let result = apply_elision_rules(&return_ty, &input_vids, self_vid);
        // Rule 3: the output lifetime should be replaced with self's vid 1.
        match &result.kind {
            TyKind::Ref(region, _, _) => {
                assert_eq!(region, &Region::Var(RegionVid(1)));
            }
            _ => panic!("expected Ref"),
        }
    }

    /// Stage 15.90: Verify `apply_elision_rules` with no input lifetimes
    /// does NOT apply (keeps original output lifetime).
    #[test]
    fn apply_elision_rule_2_no_inputs() {
        let return_ty = Ty::new(
            TyKind::Ref(
                Region::Var(RegionVid(10)),
                Mutability::Immutable,
                Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
            ),
            Span::DUMMY,
        );
        let input_vids: Vec<RegionVid> = vec![];
        let result = apply_elision_rules(&return_ty, &input_vids, None);
        match &result.kind {
            TyKind::Ref(region, _, _) => {
                assert_eq!(region, &Region::Var(RegionVid(10)));
            }
            _ => panic!("expected Ref"),
        }
    }
}
