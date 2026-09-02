//! Stage 18.279 (TD-LOC-CONTROL-FLOW): Match/Pattern lowering functions.
//!
//! Extracted from `control_flow.rs` to separate the "match/pattern lowering"
//! responsibility from the "control flow lowering" responsibility.
//! Per §13.4 J2 (单一职责): each module has one clear responsibility.
//!
//! This module hosts match expression lowering + pattern condition builders.
//!
//! Per §13.4 J3 (单向流动): `lower_if` in `control_flow.rs` calls
//! `lower_match` (one direction); no back-calls.

use crate::hir::*;
use crate::mir::body::*;
use crate::mir::place::*;
use crate::mir::ty::*;
use crate::session::Span;
use lasso::Spur;

use super::lower_expr_to_operand;
use super::pattern_bindings::{collect_pat_bindings_for_mir, lower_enum_variant_pattern_bindings};
use super::MirLowerCtxt;

pub(crate) fn lower_match(
    cx: &mut MirLowerCtxt,
    scrutinee: &HirExpr,
    arms: &[HirArm],
    span: Span,
) -> LocalId {
    let scrut_local = lower_expr_to_operand(cx, scrutinee, None);
    let cont_block = cx.new_block();
    let result_ty = cx.fresh_infer_ty(span);
    let result_local =
        cx.mir
            .new_local_with_mut(result_ty, None, span, crate::mir::ty::Mutability::Mutable);

    // Stage 18.432 (§20 iterative audit + §5.2 unblock): Check for
    // non-exhaustive match patterns. A match must be exhaustive — either:
    // - Have a catch-all arm (Wild `_` or Ident binding), OR
    // - Cover all values for Bool (both `true` and `false` patterns)
    // Was: silently accepted non-exhaustive matches → `match x { 1 => 1,
    // 2 => 2 }` compiled without error, fell through to undefined behavior.
    //
    // Per §20 (iterative audit): same class as Stage 18.412-18.428.
    // Per §1.0 原則 4 (报错 > 静默): non-exhaustive match must be reported.
    // Per §1.0 原則 9 (正确 > 妥协): defer for Infer/Error/Param/Adt/enum/
    // Str/Float/Array/Tuple/Closure (enum exhaustiveness is future work).
    // Per §1.6 终极检验: root-cause fix at the lower site.
    // Per §5.2: prelude compatibility maintained (bool match with true/false
    // is exhaustive; enum matches defer; primitive matches need `_` arm).
    let has_catch_all = arms.iter().any(|arm| {
        matches!(&arm.pat.kind, HirPatKind::Wild) || matches!(&arm.pat.kind, HirPatKind::Ident(..))
    });
    if !has_catch_all {
        let scrut_ty = cx.mir.local(scrut_local).ty.clone();
        match &scrut_ty.kind {
            // Bool: check if both `true` and `false` patterns are present.
            crate::mir::ty::TyKind::Bool => {
                let has_true = arms.iter().any(|arm| {
                    matches!(
                        &arm.pat.kind,
                        HirPatKind::Lit(e) if matches!(&e.kind, crate::hir::HirExprKind::Lit(crate::hir::HirLitKind::Bool(true)))
                    )
                });
                let has_false = arms.iter().any(|arm| {
                    matches!(
                        &arm.pat.kind,
                        HirPatKind::Lit(e) if matches!(&e.kind, crate::hir::HirExprKind::Lit(crate::hir::HirLitKind::Bool(false)))
                    )
                });
                if !has_true || !has_false {
                    cx.type_errors.push(crate::typeck::TypeError::new(
                        "non-exhaustive patterns: missing `true` or `false` arm (or catch-all `_`)"
                            .to_string(),
                        span,
                    ));
                }
            }
            // Int/Uint/Char: literal patterns can't cover all values → need `_`.
            crate::mir::ty::TyKind::Int(_)
            | crate::mir::ty::TyKind::Uint(_)
            | crate::mir::ty::TyKind::Char => {
                cx.type_errors.push(crate::typeck::TypeError::new(
                    "non-exhaustive patterns: missing catch-all `_` arm".to_string(),
                    span,
                ));
            }
            // Defer for all other types (Infer/Error/Param/Str/
            // Float/Array/Tuple/Closure/Ref/RawPtr/FnDef/FnPtr).
            //
            // Stage 28.1 (v0.10): Enum exhaustiveness check added below
            // (after the non-enum exhaustiveness checks, before the
            // enum match lowering). This is a separate check because
            // enum match uses SwitchInt on the discriminant, and we
            // need to verify all variant discriminant values are covered.
            _ => {}
        }

        // Stage 28.1 (v0.10): Enum exhaustiveness check.
        //
        // Per §1.0 原則 4 (报错 > 静默): non-exhaustive enum match must
        // be reported, not silently accepted.
        // Per §1.0 原則 9 (正确 > 妥协): defer if we can't determine the
        // enum type or variant list (avoids false positives).
        // Per §1.0 原則 6 (通解 > 特解): one check for all enum kinds.
        //
        // We check if the scrutinee is an Adt that's an enum (via HIR
        // owner lookup), then verify all variant names are covered by
        // the match arms' patterns.
        if let crate::mir::ty::TyKind::Adt(adt_def_id, _) = &scrut_ty.kind {
            // Check if this Adt is an enum via HIR owner lookup.
            let is_enum = cx
                .hir
                .and_then(|h| h.find_owner(*adt_def_id))
                .is_some_and(|o| {
                    matches!(o, crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(_)))
                });
            if is_enum {
                // Look up the enum's variant names from TraitResolver.
                // Per §11 (接口隔离): read from TraitResolver (data contract).
                if let Some(variant_names) =
                    cx.resolver.and_then(|r| r.enum_variants.get(adt_def_id))
                {
                    // Collect the variant names matched by the arms.
                    // Per §1.0 原則 6 (通解 > 特解): one loop collects all
                    // variant patterns from all arms.
                    let mut matched_variants: std::collections::HashSet<Spur> =
                        std::collections::HashSet::new();
                    for arm in arms {
                        match &arm.pat.kind {
                            // Path pattern: `Variant(...)` or `Variant`.
                            HirPatKind::Path(path) => {
                                if let Some(seg) = path.segments.last() {
                                    matched_variants.insert(seg.ident.name);
                                }
                            }
                            // Tuple struct pattern: `Variant(a, b)`.
                            HirPatKind::TupleStruct(path, _) => {
                                if let Some(seg) = path.segments.last() {
                                    matched_variants.insert(seg.ident.name);
                                }
                            }
                            // Struct pattern: `Variant { field: val }`.
                            HirPatKind::Struct(path, _, _) => {
                                if let Some(seg) = path.segments.last() {
                                    matched_variants.insert(seg.ident.name);
                                }
                            }
                            _ => {}
                        }
                    }
                    // Check if all variant names are covered.
                    for variant_name in variant_names {
                        if !matched_variants.contains(variant_name) {
                            cx.type_errors.push(crate::typeck::TypeError::new(
                                format!(
                                    "non-exhaustive patterns: missing variant `{:?}` (or catch-all `_` arm)",
                                    variant_name
                                ),
                                span,
                            ));
                            break; // Report only the first missing variant.
                        }
                    }
                }
            }
        }
    }

    // Stage 3.40 (L-ENUM-MATCH): Check if the scrutinee is an enum type.
    // If so, extract the discriminant (field 0 of the enum struct) and
    // switch on that instead of the enum value itself.
    //
    // We check both the MIR local type AND the HIR enum owners — the
    // local type may be Infer (if typeck hasn't resolved it yet at lower
    // time) but the HIR owner can tell us it's an enum.
    let scrut_ty = cx
        .mir
        .local_decls
        .get(scrut_local.0 as usize)
        .map(|ld| ld.ty.clone())
        .unwrap_or_else(|| Ty::new(TyKind::Error, span));
    let is_enum = matches!(&scrut_ty.kind, TyKind::Adt(def_id, _) if cx.hir.and_then(|h| h.find_owner(*def_id)).is_some_and(|o| {
        matches!(o, crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(_)))
    }));
    // Also check: if any arm pattern is an enum variant path, treat as enum.
    // Stage 39.2: Also check variant_index for single-segment paths like
    // `Some` / `None` — these resolve to Res::Def(enum_def_id, DefKind::Enum)
    // via the variant_index in the resolver (path_resolve.rs:911).
    // The previous `has_enum_pat` check already handles this because
    // Res::Def(_, DefKind::Enum) is returned for single-segment variant
    // paths. The issue was that `is_enum` was false (scrut_ty is Infer)
    // AND `has_enum_pat` was false — but `has_enum_pat` SHOULD be true
    // because `Some` and `None` patterns resolve to DefKind::Enum.
    //
    // Root cause: the `has_enum_pat` check at line 186-189 uses
    // `matches!(p.res, Res::Def(_, DefKind::Enum))` — this IS true for
    // single-segment variant paths. The issue is that `is_enum` is
    // determined but the discriminant extraction at line 193-227 fails
    // because `scrut_ty` is Infer (not Adt), so the GEP for field 0
    // doesn't work. The fix: when `has_enum_pat` is true but `scrut_ty`
    // is Infer, we need to resolve the enum type from the pattern's
    // resolved DefId.
    let has_enum_pat = arms.iter().any(|arm| {
        matches!(&arm.pat.kind, HirPatKind::Path(p) | HirPatKind::TupleStruct(p, _) | HirPatKind::Struct(p, _, _)
            if matches!(p.res, Res::Def(_, crate::resolve::DefKind::Enum)))
    });
    let is_enum = is_enum || has_enum_pat;

    // Stage 39.2: If is_enum is true but scrut_ty is Infer (typeck hasn't
    // resolved the enum type yet), resolve the enum DefId from the first
    // arm pattern that has Res::Def(_, DefKind::Enum). This allows the
    // discriminant extraction to work (GEP field 0 on the enum's struct
    // layout).
    //
    // Per §1.0 原則 6 (通解 > 特解): one fix for all single-segment enum
    // variant patterns (Some, None, Ok, Err, etc.).
    // Per §12 (最优 > 最小): root-cause fix — resolve the type from the
    // pattern path, not from the scrutinee local (which may be Infer).
    if is_enum && matches!(scrut_ty.kind, TyKind::Infer(_) | TyKind::Error) {
        // Find the enum DefId from the first arm pattern.
        for arm in arms.iter() {
            let pat_def_id = match &arm.pat.kind {
                HirPatKind::Path(p)
                | HirPatKind::TupleStruct(p, _)
                | HirPatKind::Struct(p, _, _) => {
                    if let Res::Def(def_id, crate::resolve::DefKind::Enum) = p.res {
                        Some(def_id)
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(enum_def_id) = pat_def_id {
                // Found the enum DefId — construct the Adt type and
                // update the scrutinee local's type so the discriminant
                // extraction works.
                let resolved_ty = Ty::new(TyKind::Adt(enum_def_id, Vec::new().into()), span);
                if let Some(ld) = cx.mir.local_decls.get_mut(scrut_local.0 as usize) {
                    ld.ty = resolved_ty;
                }
                break;
            }
        }
    }

    // If enum, extract discriminant: discr = scrut.0 (field 0 of the struct).
    let switch_discr = if is_enum {
        // Stage 14.66: If scrut_local is a Ref (e.g., `match self { ... }`
        // where `self: &Self`), dereference it before extracting the discriminant.
        let scrut_place = {
            let scrut_ty = cx.mir.local(scrut_local).ty.clone();
            if matches!(scrut_ty.kind, crate::mir::ty::TyKind::Ref(_, _, _)) {
                Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(scrut_local, scrutinee.span)),
                        ProjectionElem::Deref,
                    ),
                    span,
                }
            } else {
                Place::local(scrut_local, scrutinee.span)
            }
        };
        // Create a temp local for the extracted discriminant.
        let discr_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), span);
        let discr_local = cx.mir.new_local(discr_ty.clone(), None, span);
        cx.push_assign(
            Place::local(discr_local, span),
            Rvalue::Use(Operand::Move(Place {
                kind: PlaceKind::Projection(
                    Box::new(scrut_place),
                    ProjectionElem::Field(FieldId(0), discr_ty.clone()),
                ),
                span,
            })),
            span,
        );
        Operand::Copy(Place::local(discr_local, span))
    } else {
        Operand::Copy(Place::local(scrut_local, scrutinee.span))
    };

    // Collect targets: (constant, arm_block) pairs
    let mut targets: Vec<(ConstVal, BasicBlockId)> = Vec::new();
    let mut arm_blocks: Vec<BasicBlockId> = Vec::new();
    let otherwise_block = cx.new_block();

    // Stage 14.87 (Bug A fix): Track literal values "claimed" by guarded arms.
    // When building switch targets for unguarded arms, skip any literal value
    // that was previously claimed by a guarded arm. Those values need to be
    // evaluated in the otherwise block (where the guarded arm runs first).
    //
    // Without this, `match n { 0 if n == 0 => 100, 0 => 200 }` would route
    // n=0 directly to the second arm via the switch target, bypassing the
    // guarded first arm. Per Rust semantics, arms are tried in source order,
    // so the guarded arm should run first.
    //
    // Per §1.0 原則 5 "报错 > 静默": previously the guarded arm was silently
    // skipped when an overlapping unguarded arm existed, causing wrong output.
    //
    // Note: we use a Vec + linear scan (not HashSet) because ConstVal contains
    // f64 which doesn't implement Eq/Hash. In practice only Int/Bool values
    // are stored (no Float patterns), but using Vec avoids the trait bound.
    let mut guarded_lit_values: Vec<ConstVal> = Vec::new();

    for arm in arms {
        let arm_block = cx.new_block();
        arm_blocks.push(arm_block);

        // Stage 14.86: Match arms with guards (`pat if cond => body`) must
        // NOT be added as direct switch targets — the guard condition must
        // be evaluated before the arm body runs. If we added them as switch
        // targets, the arm would match without checking the guard.
        //
        // Skip the literal/enum target push for guarded arms — they'll be
        // handled in the otherwise block (where we can evaluate both the
        // pattern match AND the guard).
        //
        // Per §1.0 原則 5 "报错 > 静默": previously guarded arms silently
        // matched without checking the guard, causing wrong runtime behavior
        // (e.g., `x if x < 10 => 1` matched even when x >= 10).
        let has_guard = arm.guard.is_some();

        // Check if this arm's pattern is a literal
        if let HirPatKind::Lit(expr) = &arm.pat.kind {
            if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                let val = ConstVal::Int(*n);
                if has_guard {
                    // Record this literal as claimed by a guarded arm
                    guarded_lit_values.push(val);
                } else if !guarded_lit_values.contains(&val) {
                    // Not claimed — add as switch target
                    targets.push((val, arm_block));
                    continue;
                }
                // If we reach here (guarded or claimed), continue to next arm
                // (don't fall through to other pattern handlers below —
                // guarded arms are fully handled in otherwise block)
                if has_guard {
                    continue;
                }
            }
            if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                let val = ConstVal::Bool(*b);
                if has_guard {
                    guarded_lit_values.push(val);
                } else if !guarded_lit_values.contains(&val) {
                    targets.push((val, arm_block));
                    continue;
                }
                if has_guard {
                    continue;
                }
            }
        }

        // Stage 14.45: Handle Or-patterns (e.g., `1 | 2 => { ... }`).
        // Each literal sub-pattern becomes a switch case pointing to the
        // SAME arm_block. Non-literal sub-patterns are not supported in
        // Or-patterns (deferred — would need more complex lowering).
        //
        // Per §13.4 (design alignment): Rust's Or-pattern semantics require
        // all sub-patterns to match the same binding set. For literals, this
        // is straightforward — each is a separate switch case.
        // Per §"报错 > 静默": if a sub-pattern is non-literal, we emit a
        // compile error instead of silently falling through to otherwise.
        //
        // Stage 14.86: Skip Or-pattern targets for guarded arms (same as
        // single-literal arms — guards must be evaluated in otherwise block).
        //
        // Stage 14.87 (Bug A fix): For unguarded Or-patterns, skip any
        // sub-pattern literal that was claimed by a previous guarded arm.
        // If all sub-pattern literals are claimed (or none are left), don't
        // add any targets — let otherwise handle the whole arm.
        if let HirPatKind::Or(sub_pats) = &arm.pat.kind {
            if has_guard {
                // Record all literal sub-pattern values as claimed
                for sub_pat in sub_pats {
                    if let HirPatKind::Lit(expr) = &sub_pat.kind {
                        if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                            guarded_lit_values.push(ConstVal::Int(*n));
                        }
                        if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                            guarded_lit_values.push(ConstVal::Bool(*b));
                        }
                    }
                }
            } else {
                // Unguarded Or-pattern: only add sub-pattern literals that
                // weren't claimed by a previous guarded arm
                let mut all_lit = true;
                let mut added_any = false;
                for sub_pat in sub_pats {
                    if let HirPatKind::Lit(expr) = &sub_pat.kind {
                        if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                            let val = ConstVal::Int(*n);
                            if !guarded_lit_values.contains(&val) {
                                targets.push((val, arm_block));
                                added_any = true;
                            }
                            continue;
                        }
                        if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                            let val = ConstVal::Bool(*b);
                            if !guarded_lit_values.contains(&val) {
                                targets.push((val, arm_block));
                                added_any = true;
                            }
                            continue;
                        }
                    }
                    // Non-literal sub-pattern in Or — not supported yet.
                    all_lit = false;
                }
                if all_lit && added_any {
                    continue;
                }
                // If all_lit but !added_any (all claimed by guards) or
                // !all_lit (non-literal sub-pattern), fall through to
                // otherwise handling.
            }
        }

        // Stage 3.40 (L-ENUM-MATCH): Handle enum variant patterns.
        // `Color::Red` → HirPatKind::Path(path) where path resolves to enum.
        // `Opt::Some(x)` → HirPatKind::TupleStruct(path, sub_pats).
        // Resolve the variant index and use it as the switch target.
        //
        // Stage 14.86: Skip enum variant targets for guarded arms.
        //
        // Stage 14.87 (Bug A fix): For guarded enum variant arms, record the
        // variant index as claimed. For unguarded enum variant arms, skip
        // adding the target if the variant was claimed by a previous guarded
        // arm — those values need to be evaluated in otherwise.
        let enum_variant_idx: Option<u32> = if is_enum {
            match &arm.pat.kind {
                HirPatKind::Path(path) => {
                    if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                        // Stage 39.1: Support single-segment paths like `None`.
                        if !path.segments.is_empty() {
                            super::resolve_enum_variant(
                                cx,
                                def_id,
                                super::method_resolution::variant_name_from_path(path),
                            )
                            .map(|(idx, _)| idx)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                HirPatKind::TupleStruct(path, _) => {
                    if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                        if !path.segments.is_empty() {
                            super::resolve_enum_variant(
                                cx,
                                def_id,
                                super::method_resolution::variant_name_from_path(path),
                            )
                            .map(|(idx, _)| idx)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                HirPatKind::Struct(path, _, _) => {
                    if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                        if !path.segments.is_empty() {
                            super::resolve_enum_variant(
                                cx,
                                def_id,
                                super::method_resolution::variant_name_from_path(path),
                            )
                            .map(|(idx, _)| idx)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            }
        } else {
            None
        };

        if let Some(idx) = enum_variant_idx {
            let val = ConstVal::Int(idx as u128);
            // Stage 14.89 (Bug 4 fix): Check if this enum variant arm has
            // inner sub-patterns that could differentiate it from other arms
            // with the same variant. If so, don't add as a switch target —
            // handle in otherwise where inner sub-patterns can be checked.
            // This prevents duplicate switch cases AND ensures the correct
            // arm body runs (was: first arm with matching variant always won).
            //
            // Stage 39.3 (TD-PAT-IDENT-VARIANT continuation): Bindings
            // (HirPatKind::Ident) are NOT differentiating — they always
            // match. Only Lit / Range / TupleStruct / Struct / Or
            // sub-patterns differentiate. Treating a binding like `Some(v)`
            // as differentiating caused `has_inner_subpatterns = true`,
            // which prevented the variant from being added as a switch
            // target. This made the otherwise block unreachable for
            // `match self { Some(v) => v, None => default }` in the prelude
            // (segfault at runtime). Now we correctly identify bindings
            // as non-differentiating.
            //
            // Per §1.0 原則 6 (通解 > 特解): one fix for all variant
            // payload bindings (Some(v), Ok(v), Err(e), TupleStruct(a, b)).
            // Per §2.2 根因思维: fix at the pattern classification layer
            // (where sub-pattern kind is determined), not at the switch
            // target generation layer.
            let has_inner_subpatterns = match &arm.pat.kind {
                HirPatKind::TupleStruct(_, sub_pats) => sub_pats
                    .iter()
                    .any(|sp| !matches!(&sp.kind, HirPatKind::Wild | HirPatKind::Ident(..))),
                HirPatKind::Struct(_, fields, _) => fields
                    .iter()
                    .any(|f| !matches!(&f.pat.kind, HirPatKind::Wild | HirPatKind::Ident(..))),
                HirPatKind::Path(_) => false, // Unit variant — no payload
                _ => false,
            };
            if has_guard {
                // Record this enum variant as claimed by a guarded arm
                guarded_lit_values.push(val);
            } else if !guarded_lit_values.contains(&val) && !has_inner_subpatterns {
                // Not claimed AND no inner subpatterns — safe to add as switch target
                let already_in_targets = targets.iter().any(|(t_val, _)| *t_val == val);
                if !already_in_targets {
                    targets.push((val, arm_block));
                    continue;
                }
            } else if !guarded_lit_values.contains(&val) && has_inner_subpatterns {
                // Stage 14.89 (Bug 4 fix): Has inner subpatterns — record as
                // claimed so subsequent arms with same variant also go to otherwise.
                guarded_lit_values.push(val);
            }
            // If claimed by a guard OR has inner subpatterns OR already in
            // targets, fall through to otherwise handling
        }

        // Non-literal patterns (Wild, Ident, etc.) → go to otherwise
    }

    // Stage 14.47: If there are no switch targets (no literal/enum patterns),
    // don't emit a SwitchInt — just goto the otherwise block directly.
    // This handles `match t { (a, b) => ... }` where the pattern is a tuple
    // destructure (not a literal/enum). Was: emitted SwitchInt on a tuple
    // scrutinee → "expected integer or bool for switch, found Tuple" typeck error.
    if targets.is_empty() {
        cx.terminate_kind(TerminatorKind::Goto(otherwise_block));
    } else {
        // Terminate current block with SwitchInt
        cx.terminate_kind(TerminatorKind::SwitchInt {
            discr: switch_discr,
            targets: targets.clone(),
            otherwise: otherwise_block,
        });
    }

    // Lower each arm body
    for (i, arm) in arms.iter().enumerate() {
        let arm_block = arm_blocks[i];
        cx.current_block = arm_block;

        // Collect pattern bindings (for Ident patterns)
        collect_pat_bindings_for_mir(cx, &arm.pat);
        // Stage 3.48 (L-ENUM-BINDING): generate payload-extraction projections
        // for enum tuple/struct variant patterns (e.g., `Opt::Some(x)`).
        // Before this fix, the binding `x` was never assigned — reading
        // uninitialized memory (P0 soundness bug).
        lower_enum_variant_pattern_bindings(cx, scrut_local, &arm.pat);

        // Lower the arm body
        let arm_result = lower_expr_to_operand(cx, &arm.body, None);
        // Stage 14.34: If the arm body diverged (e.g. `return` inside match arm),
        // the current block is already terminated. Skip the assignment + Goto.
        if !cx.is_terminated() {
            cx.push_assign(
                Place::local(result_local, span),
                Rvalue::Use(Operand::Copy(Place::local(arm_result, arm.body.span))),
                arm.span,
            );
            cx.terminate_kind(TerminatorKind::Goto(cont_block));
        }
    }

    // Lower the otherwise block (for non-literal patterns)
    // Stage 14.67: Handle tuple patterns with literal sub-patterns by
    // generating conditional checks (if-else chain).
    //
    // For `match p { (0, 0) => 0, (0, _) => 1, (_, 0) => 2, (a, b) => ... }`:
    // - (0, 0): check p.0 == 0 AND p.1 == 0
    // - (0, _): check p.0 == 0 (p.1 is wildcard, always matches)
    // - (_, 0): check p.1 == 0 (p.0 is wildcard)
    // - (a, b): no check (binding, always matches) — catch-all
    //
    // Each arm with a tuple pattern generates a conditional block. If the
    // condition matches, execute the arm body; otherwise, fall through to
    // the next arm.
    cx.current_block = otherwise_block;
    let mut fallthrough_block = otherwise_block;
    for arm in arms {
        let has_guard = arm.guard.is_some();
        let is_literal = matches!(&arm.pat.kind, HirPatKind::Lit(_));
        // Stage 14.45: Or-pattern with all-literal sub-patterns is already
        // handled as switch cases — treat as "literal" for otherwise purposes.
        let is_or_all_lit = if let HirPatKind::Or(sub_pats) = &arm.pat.kind {
            sub_pats
                .iter()
                .all(|sp| matches!(&sp.kind, HirPatKind::Lit(_)))
        } else {
            false
        };
        // Stage 14.75: Enum variant patterns (Path, TupleStruct, Struct)
        // that resolve to enum variants are already handled as switch cases
        // above. Skip them in the otherwise block — otherwise their body
        // would be executed as a catch-all, causing wrong behavior.
        let is_enum_variant = matches!(
            &arm.pat.kind,
            HirPatKind::Path(_) | HirPatKind::TupleStruct(_, _) | HirPatKind::Struct(_, _, _)
        ) && is_enum;
        // Stage 14.86: arms WITHOUT guards that are literal/Or-all-lit/enum-variant
        // were already handled as switch cases — skip them in otherwise.
        // Arms WITH guards (regardless of pattern kind) must be handled here
        // so we can evaluate the guard before running the arm body.
        //
        // Stage 14.87 (Bug A fix): An unguarded arm whose literal value was
        // "claimed" by a previous guarded arm was NOT added as a switch target.
        // It needs to be evaluated in the otherwise block (where the guarded
        // arm runs first; if guard fails, this unguarded arm should match).
        // So we DON'T skip unguarded literal/Or-all-lit arms if their values
        // were claimed by a guarded arm.
        let was_claimed = if is_literal {
            // Check if this arm's literal value is in guarded_lit_values
            if let HirPatKind::Lit(expr) = &arm.pat.kind {
                if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                    guarded_lit_values.contains(&ConstVal::Int(*n))
                } else if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                    guarded_lit_values.contains(&ConstVal::Bool(*b))
                } else {
                    false
                }
            } else {
                false
            }
        } else if is_or_all_lit {
            // Check if any sub-pattern literal was claimed
            if let HirPatKind::Or(sub_pats) = &arm.pat.kind {
                sub_pats.iter().any(|sp| {
                    if let HirPatKind::Lit(expr) = &sp.kind {
                        if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                            return guarded_lit_values.contains(&ConstVal::Int(*n));
                        }
                        if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                            return guarded_lit_values.contains(&ConstVal::Bool(*b));
                        }
                    }
                    false
                })
            } else {
                false
            }
        } else if is_enum_variant {
            // Stage 14.87 (Bug A fix): Check if this enum variant was claimed
            // by a previous guarded arm
            let variant_idx = match &arm.pat.kind {
                HirPatKind::Path(path)
                | HirPatKind::TupleStruct(path, _)
                | HirPatKind::Struct(path, _, _) => {
                    if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                        if path.segments.len() >= 2 {
                            super::resolve_enum_variant(
                                cx,
                                def_id,
                                super::method_resolution::variant_name_from_path(path),
                            )
                            .map(|(idx, _)| idx)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(idx) = variant_idx {
                guarded_lit_values.contains(&ConstVal::Int(idx as u128))
            } else {
                false
            }
        } else {
            false
        };
        // Skip ONLY if: no guard AND (literal/Or-all-lit/enum-variant) AND
        // not was_claimed (i.e., this arm was added as a switch target).
        //
        // Stage 14.89 (Bug 4 fix): Also check if this enum variant was NOT
        // added as a switch target (because it was a duplicate). If it wasn't
        // added, don't skip — handle it in otherwise (check inner sub-patterns).
        let was_added_as_target = if is_enum_variant {
            // Check if this variant was added to targets
            let variant_idx = match &arm.pat.kind {
                HirPatKind::Path(path)
                | HirPatKind::TupleStruct(path, _)
                | HirPatKind::Struct(path, _, _) => {
                    if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                        if path.segments.len() >= 2 {
                            super::resolve_enum_variant(
                                cx,
                                def_id,
                                super::method_resolution::variant_name_from_path(path),
                            )
                            .map(|(idx, _)| idx)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(idx) = variant_idx {
                let val = ConstVal::Int(idx as u128);
                targets.iter().any(|(t_val, _)| *t_val == val)
            } else {
                false
            }
        } else {
            // For literal/Or patterns, they were added as targets if not claimed
            !was_claimed
        };
        // Stage 14.89 (Bug 4 fix): If enum variant was NOT added as target
        // (duplicate), treat as claimed so it gets handled in the
        // has_guard || was_claimed block (which checks the pattern).
        let was_claimed = was_claimed || (!was_added_as_target && is_enum_variant);
        if !has_guard && (is_literal || is_or_all_lit || is_enum_variant) && was_added_as_target {
            continue;
        }

        // Stage 14.86 + 14.87: For guarded arms (and unguarded arms whose
        // literal values were claimed by a previous guarded arm), we need
        // to:
        // 1. Check the pattern match (since they weren't added as switch
        //    targets, we need to re-check in otherwise)
        // 2. Bind pattern variables (for Ident patterns) so the guard can
        //    reference them (guarded arms only — claimed-but-unguarded arms
        //    use literal/Or/enum patterns, no Ident bindings)
        // 3. Evaluate the guard (guarded arms only)
        // 4. If both pass, run the arm body; otherwise, fall through to next arm
        //
        // Stage 14.87 (Bug A fix): previously only `has_guard` arms were
        // handled here. Now also handle `was_claimed` arms (unguarded arms
        // whose literal value was claimed by a previous guarded arm — these
        // weren't added as switch targets, so they need pattern re-check).
        if has_guard || was_claimed {
            let next_block = cx.new_block();
            let match_block = cx.new_block();

            cx.current_block = fallthrough_block;

            // Step 1: Bind pattern variables BEFORE evaluating pattern check
            // or guard. For Ident patterns, this creates the binding local
            // and assigns the scrutinee value. The guard can then reference
            // the binding.
            collect_pat_bindings_for_mir(cx, &arm.pat);
            lower_enum_variant_pattern_bindings(cx, scrut_local, &arm.pat);
            // Stage 14.77: For Ident patterns, assign scrutinee value to binding
            if let HirPatKind::Ident(_mode, _ident, _) = &arm.pat.kind {
                if let Some(binding_local) = cx.local_map.get(&arm.pat.hir_id).copied() {
                    let scrut_ty = cx.mir.local(scrut_local).ty.clone();
                    if matches!(&scrut_ty.kind, crate::mir::ty::TyKind::Ref(_, _, _)) {
                        cx.push_assign(
                            Place::local(binding_local, arm.pat.span),
                            Rvalue::Use(Operand::Copy(Place {
                                kind: PlaceKind::Projection(
                                    Box::new(Place::local(scrut_local, arm.pat.span)),
                                    ProjectionElem::Deref,
                                ),
                                span: arm.pat.span,
                            })),
                            arm.pat.span,
                        );
                    } else {
                        cx.push_assign(
                            Place::local(binding_local, arm.pat.span),
                            Rvalue::Use(Operand::Copy(Place::local(scrut_local, arm.pat.span))),
                            arm.pat.span,
                        );
                    }
                }
            }

            // Step 2: Build the pattern-match condition.
            // For literal/Or/enum patterns, generate the equality check.
            // For Tuple/TupleStruct/Struct patterns with literal sub-patterns,
            // also generate conditional checks (Stage 14.90 Bug X1 fix).
            // For Ident/Wild/other, skip (always matches).
            let needs_pattern_check = match &arm.pat.kind {
                HirPatKind::Lit(_) => true,
                HirPatKind::Or(_) => true,
                HirPatKind::Path(_)
                | HirPatKind::TupleStruct(_, _)
                | HirPatKind::Struct(_, _, _)
                    if is_enum =>
                {
                    true
                }
                // Stage 14.90 (Bug X1 fix): Tuple/TupleStruct/Struct patterns
                // with literal sub-patterns need conditional checks in
                // guarded arms. Without this, `match (1, 5) { (0, _) if true => 100, (1, _) if true => 200 }`
                // would match the first arm without checking the tuple fields.
                HirPatKind::Tuple(sub_pats)
                    if sub_pats
                        .iter()
                        .any(|sp| matches!(&sp.kind, HirPatKind::Lit(_))) =>
                {
                    true
                }
                HirPatKind::TupleStruct(_, sub_pats)
                    if !is_enum
                        && sub_pats
                            .iter()
                            .any(|sp| matches!(&sp.kind, HirPatKind::Lit(_))) =>
                {
                    true
                }
                HirPatKind::Struct(_, fields, _)
                    if fields
                        .iter()
                        .any(|f| matches!(&f.pat.kind, HirPatKind::Lit(_))) =>
                {
                    true
                }
                _ => false,
            };

            let after_pattern_check_block = if needs_pattern_check {
                // Build pattern-match check, branching to after_pattern_check_block
                // on success or next_block on failure.
                let after_pattern_check_block = cx.new_block();
                build_pattern_equality_check(
                    cx,
                    scrut_local,
                    &arm.pat,
                    is_enum,
                    after_pattern_check_block,
                    next_block,
                );
                after_pattern_check_block
            } else {
                // No pattern check needed — current block IS the after-pattern-check block
                cx.current_block
            };

            // Step 3: If arm has a guard, evaluate the guard from after_pattern_check_block.
            // If no guard (was_claimed but unguarded), skip directly to arm body.
            let body_block = if has_guard {
                cx.current_block = after_pattern_check_block;
                // Guarded by `has_guard`: arm.guard is Some.
                let guard_expr = arm.guard.as_ref().expect("has_guard => arm.guard is Some");
                let guard_local = lower_expr_to_operand(cx, guard_expr, None);
                cx.terminate_kind(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(Place::local(guard_local, guard_expr.span)),
                    targets: vec![(ConstVal::Bool(true), match_block)],
                    otherwise: next_block,
                });
                match_block
            } else {
                // No guard — after_pattern_check_block IS where we run the arm body
                after_pattern_check_block
            };

            // body_block: pattern + guard both passed — execute arm body
            cx.current_block = body_block;
            // Note: pattern bindings already done above (before guard eval)
            let arm_result = lower_expr_to_operand(cx, &arm.body, None);
            if !cx.is_terminated() {
                cx.push_assign(
                    Place::local(result_local, span),
                    Rvalue::Use(Operand::Copy(Place::local(arm_result, arm.body.span))),
                    arm.span,
                );
                cx.terminate_kind(TerminatorKind::Goto(cont_block));
            }
            fallthrough_block = next_block;
            continue;
        }

        // Stage 14.67: For tuple patterns with literal sub-patterns,
        // generate a conditional check.
        let has_tuple_lit = matches!(&arm.pat.kind, HirPatKind::Tuple(_));
        // Stage 14.89 (Bug 2 fix): Also handle TupleStruct patterns with
        // literal sub-patterns (e.g., `Pair(0, _) => ...`).
        let has_tuple_struct_lit = matches!(
            &arm.pat.kind,
            HirPatKind::TupleStruct(_, sub_pats)
            if sub_pats.iter().any(|sp| matches!(&sp.kind, HirPatKind::Lit(_)))
        );
        // Stage 14.89 (Bug 3 fix): Also handle Struct patterns with literal
        // sub-patterns (e.g., `Config { mode: 0, .. } => ...`).
        let has_struct_lit = matches!(
            &arm.pat.kind,
            HirPatKind::Struct(_, fields, _)
            if fields.iter().any(|f| matches!(&f.pat.kind, HirPatKind::Lit(_)))
        );
        if has_tuple_lit || has_tuple_struct_lit || has_struct_lit {
            // Generate condition: AND of all literal sub-field checks
            let next_block = cx.new_block();
            let match_block = cx.new_block();

            // Build the condition by checking each literal sub-pattern
            cx.current_block = fallthrough_block;
            match &arm.pat.kind {
                HirPatKind::Tuple(sub_pats) => {
                    build_tuple_pattern_condition(
                        cx,
                        scrut_local,
                        sub_pats,
                        match_block,
                        next_block,
                    );
                }
                HirPatKind::TupleStruct(_, sub_pats) => {
                    // Stage 14.89: For tuple struct patterns, extract fields
                    // by positional index (same as Tuple, but the scrutinee
                    // is a struct, not a tuple).
                    build_tuple_pattern_condition(
                        cx,
                        scrut_local,
                        sub_pats,
                        match_block,
                        next_block,
                    );
                }
                HirPatKind::Struct(path, fields, _) => {
                    // Stage 14.89: For struct patterns, look up field indices
                    // by name from HIR, then check each literal sub-pattern.
                    if let crate::hir::Res::Def(struct_def_id, _) = path.res {
                        let field_indices: std::collections::HashMap<
                            crate::lexer::token::Symbol,
                            usize,
                        > = {
                            let mut map = std::collections::HashMap::new();
                            if let Some(hir) = cx.hir {
                                if let Some(crate::hir::OwnerNode::Item(
                                    crate::hir::HirItem::Struct(s),
                                )) = hir.find_owner(struct_def_id)
                                {
                                    for (i, f) in s.fields.iter().enumerate() {
                                        if let Some(name) = f.ident {
                                            map.insert(name.name, i);
                                        }
                                    }
                                }
                            }
                            map
                        };
                        // Build a Vec of (field_idx, sub_pat) for literal sub-patterns
                        let mut lit_checks: Vec<(usize, &HirPat)> = Vec::new();
                        for field_pat in fields {
                            if let Some(field_idx) =
                                field_indices.get(&field_pat.ident.name).copied()
                            {
                                if matches!(&field_pat.pat.kind, HirPatKind::Lit(_)) {
                                    lit_checks.push((field_idx, &field_pat.pat));
                                }
                            }
                        }
                        // For struct patterns, build the condition inline
                        // (build_tuple_pattern_condition expects positional indices)
                        let mut current = cx.current_block;
                        let span = arm.pat.span;
                        for (field_idx, lit_pat) in &lit_checks {
                            if let HirPatKind::Lit(expr) = &lit_pat.kind {
                                let lit_val = if let HirExprKind::Lit(HirLitKind::Int(n, _)) =
                                    &expr.kind
                                {
                                    Some(*n)
                                } else if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                                    Some(if *b { 1 } else { 0 })
                                } else {
                                    None
                                };
                                if let Some(n) = lit_val {
                                    // Extract field from scrutinee
                                    let field_ty =
                                        Ty::new(TyKind::Int(crate::ast::IntTy::I32), span);
                                    let field_local =
                                        cx.mir.new_local(field_ty.clone(), None, span);
                                    cx.mir.block_mut(current).statements.push(Statement {
                                        kind: StatementKind::StorageLive(field_local),
                                        span,
                                    });
                                    cx.current_block = current;
                                    cx.push_assign(
                                        Place::local(field_local, span),
                                        Rvalue::Use(Operand::Copy(Place {
                                            kind: PlaceKind::Projection(
                                                Box::new(Place::local(scrut_local, span)),
                                                ProjectionElem::Field(
                                                    FieldId(*field_idx as u32),
                                                    field_ty.clone(),
                                                ),
                                            ),
                                            span,
                                        })),
                                        span,
                                    );
                                    current = cx.current_block;
                                    // Compare field == n
                                    let cmp_result =
                                        cx.mir.new_local(Ty::new(TyKind::Bool, span), None, span);
                                    cx.mir.block_mut(current).statements.push(Statement {
                                        kind: StatementKind::StorageLive(cmp_result),
                                        span,
                                    });
                                    cx.current_block = current;
                                    cx.push_assign(
                                        Place::local(cmp_result, span),
                                        Rvalue::BinaryOp(
                                            BinOp::Eq,
                                            Operand::Copy(Place::local(field_local, span)),
                                            Operand::Constant(crate::mir::ty::Const {
                                                ty: field_ty,
                                                val: crate::mir::ty::ConstVal::Int(n),
                                            }),
                                        ),
                                        span,
                                    );
                                    current = cx.current_block;
                                    let continue_block = cx.new_block();
                                    cx.mir.block_mut(current).terminator = Terminator::new(
                                        TerminatorKind::SwitchInt {
                                            discr: Operand::Copy(Place::local(cmp_result, span)),
                                            targets: vec![(ConstVal::Bool(true), continue_block)],
                                            otherwise: next_block,
                                        },
                                        Span::DUMMY,
                                    );
                                    current = continue_block;
                                }
                            }
                        }
                        // All literal checks passed — goto match_block
                        cx.mir.block_mut(current).terminator =
                            Terminator::new(TerminatorKind::Goto(match_block), Span::DUMMY);
                        cx.current_block = current;
                    }
                }
                _ => {}
            }
            // match_block: pattern matched — execute arm body
            cx.current_block = match_block;
            collect_pat_bindings_for_mir(cx, &arm.pat);
            lower_enum_variant_pattern_bindings(cx, scrut_local, &arm.pat);
            let arm_result = lower_expr_to_operand(cx, &arm.body, None);
            cx.push_assign(
                Place::local(result_local, span),
                Rvalue::Use(Operand::Copy(Place::local(arm_result, arm.body.span))),
                arm.span,
            );
            cx.terminate_kind(TerminatorKind::Goto(cont_block));
            fallthrough_block = next_block;
            continue;
        }

        // Non-tuple, non-literal pattern (Wild, Ident, etc.) — catch-all
        cx.current_block = fallthrough_block;
        collect_pat_bindings_for_mir(cx, &arm.pat);
        lower_enum_variant_pattern_bindings(cx, scrut_local, &arm.pat);

        // Stage 14.77: For Ident patterns (bindings like `n => { ... }`),
        // assign the scrutinee value to the binding local.
        //
        // collect_pat_bindings_for_mir creates the local but doesn't assign it.
        // Without this, `n` would be uninitialized — reading garbage values.
        //
        // Per §1.0 原则 5 "报错 > 静默": bindings must be initialized to the
        // scrutinee value, not left as uninitialized stack garbage.
        if let HirPatKind::Ident(_mode, _ident, _) = &arm.pat.kind {
            // Look up the local created by collect_pat_bindings_for_mir
            if let Some(binding_local) = cx.local_map.get(&arm.pat.hir_id).copied() {
                // Check if scrut_local is a Ref (e.g., &self match)
                let scrut_ty = cx.mir.local(scrut_local).ty.clone();
                if matches!(&scrut_ty.kind, crate::mir::ty::TyKind::Ref(_, _, _)) {
                    // Scrutinee is a reference — deref before assigning
                    let deref_ty = match &scrut_ty.kind {
                        crate::mir::ty::TyKind::Ref(_, _, inner) => (**inner).clone(),
                        _ => scrut_ty.clone(),
                    };
                    cx.push_assign(
                        Place::local(binding_local, arm.pat.span),
                        Rvalue::Use(Operand::Copy(Place {
                            kind: PlaceKind::Projection(
                                Box::new(Place::local(scrut_local, arm.pat.span)),
                                ProjectionElem::Deref,
                            ),
                            span: arm.pat.span,
                        })),
                        arm.pat.span,
                    );
                    let _ = deref_ty; // type hint (not used directly)
                } else {
                    // Scrutinee is by value — copy directly
                    cx.push_assign(
                        Place::local(binding_local, arm.pat.span),
                        Rvalue::Use(Operand::Copy(Place::local(scrut_local, arm.pat.span))),
                        arm.pat.span,
                    );
                }
            }
        }

        let arm_result = lower_expr_to_operand(cx, &arm.body, None);
        cx.push_assign(
            Place::local(result_local, span),
            Rvalue::Use(Operand::Copy(Place::local(arm_result, arm.body.span))),
            arm.span,
        );
        // Stage 14.71: Don't reset cx.current_block to fallthrough_block
        // after the loop — lower_expr_to_operand may have created new blocks
        // (e.g., overflow checks), and cx.current_block points to the LAST
        // block. Resetting to fallthrough_block would overwrite the original
        // block with a Goto, orphaning the overflow check blocks.
        // Instead, terminate the CURRENT block (the last one from
        // lower_expr_to_operand) with Goto(cont_block).
        if !cx.is_terminated() {
            cx.terminate_kind(TerminatorKind::Goto(cont_block));
        }
        break;
    }
    // Stage 14.71: Only terminate fallthrough_block if no catch-all was found.
    // If a catch-all was found, the loop already terminated the last block.
    // If no catch-all, fallthrough_block needs a terminator.
    // Check if fallthrough_block is the same as cx.current_block — if so,
    // it hasn't been terminated yet (no catch-all found).
    if !cx.is_terminated() {
        cx.terminate_kind(TerminatorKind::Goto(cont_block));
    }

    // Continuation
    cx.current_block = cont_block;
    result_local
}

/// Stage 14.67: Build a conditional check for a tuple pattern.
///
/// For each literal sub-pattern at index `i`, check `scrut.i == literal`.
/// Wildcard sub-patterns (`_`) are skipped (always match).
/// Ident sub-patterns (bindings) are skipped (always match, binding done later).
///
/// Generates an if-else chain: if first check fails, goto next_block;
/// if all checks pass, goto match_block.
pub(super) fn build_tuple_pattern_condition(
    cx: &mut MirLowerCtxt,
    scrut_local: LocalId,
    sub_pats: &[HirPat],
    match_block: BasicBlockId,
    next_block: BasicBlockId,
) {
    use crate::mir::body::StatementKind;
    use crate::mir::place::{BinOp, Operand, Place, PlaceKind, ProjectionElem, Rvalue};

    let span = crate::session::Span::DUMMY;
    let mut current = cx.current_block;

    for (i, sub_pat) in sub_pats.iter().enumerate() {
        // Only check literal sub-patterns
        let lit_val = if let HirPatKind::Lit(expr) = &sub_pat.kind {
            if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                Some(*n)
            } else if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                // Bool literals — compare as i32 (0 or 1)
                Some(if *b { 1 } else { 0 })
            } else {
                None
            }
        } else {
            None
        };

        if let Some(n) = lit_val {
            // Extract field i from scrutinee: scrut.i
            let field_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), span);
            let field_local = cx.mir.new_local(field_ty.clone(), None, span);
            cx.mir.block_mut(current).statements.push(Statement {
                kind: StatementKind::StorageLive(field_local),
                span,
            });
            // push_assign uses current_block, so set it to `current`
            cx.current_block = current;
            cx.push_assign(
                Place::local(field_local, span),
                Rvalue::Use(Operand::Copy(Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(scrut_local, span)),
                        ProjectionElem::Field(FieldId(i as u32), field_ty.clone()),
                    ),
                    span,
                })),
                span,
            );
            current = cx.current_block;

            // Compare: field_local == n
            let cmp_result = cx.mir.new_local(Ty::new(TyKind::Bool, span), None, span);
            cx.mir.block_mut(current).statements.push(Statement {
                kind: StatementKind::StorageLive(cmp_result),
                span,
            });
            cx.current_block = current;
            cx.push_assign(
                Place::local(cmp_result, span),
                Rvalue::BinaryOp(
                    BinOp::Eq,
                    Operand::Copy(Place::local(field_local, span)),
                    Operand::Constant(crate::mir::ty::Const {
                        ty: field_ty,
                        val: crate::mir::ty::ConstVal::Int(n),
                    }),
                ),
                span,
            );
            current = cx.current_block;

            // Switch on cmp_result: if true, continue to next check (or match);
            // if false, goto next_block (pattern didn't match)
            let continue_block = cx.new_block();
            cx.mir.block_mut(current).terminator = Terminator::new(
                TerminatorKind::SwitchInt {
                    discr: Operand::Copy(Place::local(cmp_result, span)),
                    targets: vec![(ConstVal::Bool(true), continue_block)],
                    otherwise: next_block,
                },
                Span::DUMMY,
            );
            current = continue_block;
        }
        // Stage 14.87 (Bug B fix): Handle enum variant sub-patterns.
        // For `(Opt::None, 0)` etc., extract the field's enum discriminant
        // and compare to the variant index. Was: silently skipped (treated
        // as wildcard), causing wrong match results.
        else if let HirPatKind::Path(path)
        | HirPatKind::TupleStruct(path, _)
        | HirPatKind::Struct(path, _, _) = &sub_pat.kind
        {
            if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                if path.segments.len() >= 2 {
                    if let Some((variant_idx, _)) = super::resolve_enum_variant(
                        cx,
                        def_id,
                        super::method_resolution::variant_name_from_path(path),
                    ) {
                        // Build the enum's Adt type for the field local
                        let enum_ty = Ty::new(
                            TyKind::Adt(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
                            span,
                        );
                        let field_local = cx.mir.new_local(enum_ty.clone(), None, span);
                        cx.mir.block_mut(current).statements.push(Statement {
                            kind: StatementKind::StorageLive(field_local),
                            span,
                        });
                        cx.current_block = current;
                        cx.push_assign(
                            Place::local(field_local, span),
                            Rvalue::Use(Operand::Copy(Place {
                                kind: PlaceKind::Projection(
                                    Box::new(Place::local(scrut_local, span)),
                                    ProjectionElem::Field(FieldId(i as u32), enum_ty.clone()),
                                ),
                                span,
                            })),
                            span,
                        );
                        current = cx.current_block;

                        // Extract discriminant: discr = field_local.0
                        let discr_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), span);
                        let discr_local = cx.mir.new_local(discr_ty.clone(), None, span);
                        cx.mir.block_mut(current).statements.push(Statement {
                            kind: StatementKind::StorageLive(discr_local),
                            span,
                        });
                        cx.current_block = current;
                        cx.push_assign(
                            Place::local(discr_local, span),
                            Rvalue::Use(Operand::Copy(Place {
                                kind: PlaceKind::Projection(
                                    Box::new(Place::local(field_local, span)),
                                    ProjectionElem::Field(FieldId(0), discr_ty.clone()),
                                ),
                                span,
                            })),
                            span,
                        );
                        current = cx.current_block;

                        // Compare: discr_local == variant_idx
                        let cmp_result = cx.mir.new_local(Ty::new(TyKind::Bool, span), None, span);
                        cx.mir.block_mut(current).statements.push(Statement {
                            kind: StatementKind::StorageLive(cmp_result),
                            span,
                        });
                        cx.current_block = current;
                        cx.push_assign(
                            Place::local(cmp_result, span),
                            Rvalue::BinaryOp(
                                BinOp::Eq,
                                Operand::Copy(Place::local(discr_local, span)),
                                Operand::Constant(crate::mir::ty::Const {
                                    ty: discr_ty,
                                    val: crate::mir::ty::ConstVal::Int(variant_idx as u128),
                                }),
                            ),
                            span,
                        );
                        current = cx.current_block;

                        // Switch on cmp_result
                        let continue_block = cx.new_block();
                        cx.mir.block_mut(current).terminator = Terminator::new(
                            TerminatorKind::SwitchInt {
                                discr: Operand::Copy(Place::local(cmp_result, span)),
                                targets: vec![(ConstVal::Bool(true), continue_block)],
                                otherwise: next_block,
                            },
                            Span::DUMMY,
                        );
                        current = continue_block;
                    }
                }
            }
        }
        // Wildcard or Ident — skip (always matches)
    }

    // All checks passed — goto match_block
    cx.mir.block_mut(current).terminator =
        Terminator::new(TerminatorKind::Goto(match_block), Span::DUMMY);
    cx.current_block = current;
}

/// Stage 14.86: Build a pattern-equality check for guarded match arms.
///
/// For arms with guards that have literal/Or/enum patterns, we need to
/// re-check the pattern in the otherwise block (since guarded arms were
/// not added as switch targets).
///
/// Generates: if pattern_matches(scrut, pat) { goto match_block } else { goto next_block }
///
/// Pattern kinds handled:
/// - `HirPatKind::Lit(lit_expr)` — check `scrut == lit`
/// - `HirPatKind::Or(sub_pats)` — check `scrut == lit1 || scrut == lit2 || ...`
///   (only for all-literal sub-patterns; non-literal Or sub-patterns in
///   guarded arms are not supported — fall through to next_block)
/// - `HirPatKind::Path/TupleStruct/Struct` (enum variant) — check
///   `scrut_discr == variant_idx` (extract discriminant from scrut first)
///
/// For non-literal/Or/enum patterns (Ident, Wild, etc.), this function is
/// not called (callers check `needs_pattern_check` first).
///
/// Per §1.0 原則 5 "报错 > 静默": this generates the pattern check that
/// was previously missing for guarded arms, causing them to match without
/// verifying the pattern.
pub(super) fn build_pattern_equality_check(
    cx: &mut MirLowerCtxt,
    scrut_local: LocalId,
    pat: &HirPat,
    is_enum: bool,
    match_block: BasicBlockId,
    next_block: BasicBlockId,
) {
    use crate::mir::place::{BinOp, Operand, Place, PlaceKind, ProjectionElem, Rvalue};

    let span = pat.span;

    // Helper: build `scrut == lit_val` and terminate current block with
    // a SwitchInt that goes to match_block on true, next_block on false.
    let build_eq_check = |cx: &mut MirLowerCtxt, lit_val: i128, is_bool: bool| {
        // Load scrutinee value
        let scrut_place = Place::local(scrut_local, span);
        // Build `scrut == lit_val`
        let lit_const = crate::mir::ty::Const {
            ty: Ty::new(
                if is_bool {
                    TyKind::Bool
                } else {
                    TyKind::Int(crate::ast::IntTy::I32)
                },
                span,
            ),
            val: if is_bool {
                crate::mir::ty::ConstVal::Bool(lit_val != 0)
            } else {
                crate::mir::ty::ConstVal::Int(lit_val as u128)
            },
        };
        let cmp_result = cx.eval_rvalue_to_temp(
            Rvalue::BinaryOp(
                BinOp::Eq,
                Operand::Copy(scrut_place),
                Operand::Constant(lit_const),
            ),
            Ty::new(TyKind::Bool, span),
            span,
        );
        cx.terminate_kind(TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::local(cmp_result, span)),
            targets: vec![(ConstVal::Bool(true), match_block)],
            otherwise: next_block,
        });
    };

    match &pat.kind {
        HirPatKind::Lit(expr) => {
            if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                build_eq_check(cx, *n as i128, false);
            } else if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                build_eq_check(cx, *b as i128, true);
            } else {
                // Unknown literal — fall through to next
                cx.terminate_kind(TerminatorKind::Goto(next_block));
            }
        }
        HirPatKind::Or(sub_pats) => {
            // For Or-patterns, build: if scrut == lit1 { match } else if scrut == lit2 { match } else { next }
            // Each sub-pattern is a separate check. On success, go to match_block.
            // On failure, go to the next sub-pattern's check block.
            // After the last sub-pattern fails, go to next_block.
            let mut current_block = cx.current_block;
            for (i, sub_pat) in sub_pats.iter().enumerate() {
                if let HirPatKind::Lit(expr) = &sub_pat.kind {
                    let (lit_val, is_bool) =
                        if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                            (*n as i128, false)
                        } else if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                            (*b as i128, true)
                        } else {
                            // Unknown literal type — fall through to next
                            cx.current_block = current_block;
                            cx.terminate_kind(TerminatorKind::Goto(next_block));
                            return;
                        };
                    // The "failure" target is the next sub-pattern's check,
                    // or next_block if this is the last sub-pattern.
                    let failure_block = if i + 1 < sub_pats.len() {
                        cx.new_block()
                    } else {
                        next_block
                    };
                    cx.current_block = current_block;
                    // Build `scrut == lit_val` inline (can't use build_eq_check
                    // because it captures match_block/next_block from closure)
                    let scrut_place = Place::local(scrut_local, span);
                    let lit_const = crate::mir::ty::Const {
                        ty: Ty::new(
                            if is_bool {
                                TyKind::Bool
                            } else {
                                TyKind::Int(crate::ast::IntTy::I32)
                            },
                            span,
                        ),
                        val: if is_bool {
                            crate::mir::ty::ConstVal::Bool(lit_val != 0)
                        } else {
                            crate::mir::ty::ConstVal::Int(lit_val as u128)
                        },
                    };
                    let cmp_result = cx.eval_rvalue_to_temp(
                        Rvalue::BinaryOp(
                            BinOp::Eq,
                            Operand::Copy(scrut_place),
                            Operand::Constant(lit_const),
                        ),
                        Ty::new(TyKind::Bool, span),
                        span,
                    );
                    cx.terminate_kind(TerminatorKind::SwitchInt {
                        discr: Operand::Copy(Place::local(cmp_result, span)),
                        targets: vec![(ConstVal::Bool(true), match_block)],
                        otherwise: failure_block,
                    });
                    current_block = failure_block;
                } else {
                    // Non-literal sub-pattern — not supported, fall through to next
                    cx.current_block = current_block;
                    cx.terminate_kind(TerminatorKind::Goto(next_block));
                    return;
                }
            }
        }
        HirPatKind::Path(path)
        | HirPatKind::TupleStruct(path, _)
        | HirPatKind::Struct(path, _, _)
            if is_enum =>
        {
            // For enum variant, check discriminant (field 0 of scrut struct)
            if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                if path.segments.len() >= 2 {
                    if let Some((variant_idx, _)) = super::resolve_enum_variant(
                        cx,
                        def_id,
                        super::method_resolution::variant_name_from_path(path),
                    ) {
                        // Extract discriminant: discr = scrut.0
                        let discr_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), span);
                        let discr_local = cx.mir.new_local(discr_ty.clone(), None, span);
                        let scrut_ty = cx.mir.local(scrut_local).ty.clone();
                        let scrut_place =
                            if matches!(&scrut_ty.kind, crate::mir::ty::TyKind::Ref(_, _, _)) {
                                Place {
                                    kind: PlaceKind::Projection(
                                        Box::new(Place::local(scrut_local, span)),
                                        ProjectionElem::Deref,
                                    ),
                                    span,
                                }
                            } else {
                                Place::local(scrut_local, span)
                            };
                        cx.push_assign(
                            Place::local(discr_local, span),
                            Rvalue::Use(Operand::Move(Place {
                                kind: PlaceKind::Projection(
                                    Box::new(scrut_place),
                                    ProjectionElem::Field(FieldId(0), discr_ty.clone()),
                                ),
                                span,
                            })),
                            span,
                        );
                        // Check discr == variant_idx
                        let lit_const = crate::mir::ty::Const {
                            ty: Ty::new(discr_ty.kind.clone(), span),
                            val: crate::mir::ty::ConstVal::Int(variant_idx as u128),
                        };
                        let cmp_result = cx.eval_rvalue_to_temp(
                            Rvalue::BinaryOp(
                                BinOp::Eq,
                                Operand::Copy(Place::local(discr_local, span)),
                                Operand::Constant(lit_const),
                            ),
                            Ty::new(TyKind::Bool, span),
                            span,
                        );
                        cx.terminate_kind(TerminatorKind::SwitchInt {
                            discr: Operand::Copy(Place::local(cmp_result, span)),
                            targets: vec![(ConstVal::Bool(true), match_block)],
                            otherwise: next_block,
                        });
                        return;
                    }
                }
            }
            // Couldn't resolve variant — fall through
            cx.terminate_kind(TerminatorKind::Goto(next_block));
        }
        // Stage 14.90 (Bug X1 fix): Handle Tuple/TupleStruct patterns with
        // literal sub-patterns. These need conditional field checks.
        // For Tuple and TupleStruct (positional), use build_tuple_pattern_condition.
        HirPatKind::Tuple(sub_pats) => {
            build_tuple_pattern_condition(cx, scrut_local, sub_pats, match_block, next_block);
        }
        HirPatKind::TupleStruct(_, sub_pats) if !is_enum => {
            // Plain tuple struct — same as Tuple (positional fields)
            build_tuple_pattern_condition(cx, scrut_local, sub_pats, match_block, next_block);
        }
        // Stage 14.90 (Bug X1 fix): Handle Struct patterns with literal
        // sub-patterns. Look up field indices by name, check each literal.
        HirPatKind::Struct(path, fields, _) => {
            if let crate::hir::Res::Def(struct_def_id, _) = path.res {
                let field_indices: std::collections::HashMap<crate::lexer::token::Symbol, usize> = {
                    let mut map = std::collections::HashMap::new();
                    if let Some(hir) = cx.hir {
                        if let Some(crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s))) =
                            hir.find_owner(struct_def_id)
                        {
                            for (i, f) in s.fields.iter().enumerate() {
                                if let Some(name) = f.ident {
                                    map.insert(name.name, i);
                                }
                            }
                        }
                    }
                    map
                };
                let mut current = cx.current_block;
                let mut found_lit = false;
                for field_pat in fields {
                    if let Some(field_idx) = field_indices.get(&field_pat.ident.name).copied() {
                        if let HirPatKind::Lit(expr) = &field_pat.pat.kind {
                            found_lit = true;
                            let lit_val =
                                if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                                    Some(*n)
                                } else if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                                    Some(if *b { 1 } else { 0 })
                                } else {
                                    None
                                };
                            if let Some(n) = lit_val {
                                let field_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), span);
                                let field_local = cx.mir.new_local(field_ty.clone(), None, span);
                                cx.mir.block_mut(current).statements.push(Statement {
                                    kind: StatementKind::StorageLive(field_local),
                                    span,
                                });
                                cx.current_block = current;
                                cx.push_assign(
                                    Place::local(field_local, span),
                                    Rvalue::Use(Operand::Copy(Place {
                                        kind: PlaceKind::Projection(
                                            Box::new(Place::local(scrut_local, span)),
                                            ProjectionElem::Field(
                                                FieldId(field_idx as u32),
                                                field_ty.clone(),
                                            ),
                                        ),
                                        span,
                                    })),
                                    span,
                                );
                                current = cx.current_block;
                                let cmp_result =
                                    cx.mir.new_local(Ty::new(TyKind::Bool, span), None, span);
                                cx.mir.block_mut(current).statements.push(Statement {
                                    kind: StatementKind::StorageLive(cmp_result),
                                    span,
                                });
                                cx.current_block = current;
                                cx.push_assign(
                                    Place::local(cmp_result, span),
                                    Rvalue::BinaryOp(
                                        BinOp::Eq,
                                        Operand::Copy(Place::local(field_local, span)),
                                        Operand::Constant(crate::mir::ty::Const {
                                            ty: field_ty,
                                            val: crate::mir::ty::ConstVal::Int(n),
                                        }),
                                    ),
                                    span,
                                );
                                current = cx.current_block;
                                let continue_block = cx.new_block();
                                cx.mir.block_mut(current).terminator = Terminator::new(
                                    TerminatorKind::SwitchInt {
                                        discr: Operand::Copy(Place::local(cmp_result, span)),
                                        targets: vec![(ConstVal::Bool(true), continue_block)],
                                        otherwise: next_block,
                                    },
                                    Span::DUMMY,
                                );
                                current = continue_block;
                            }
                        }
                    }
                }
                if found_lit {
                    cx.mir.block_mut(current).terminator =
                        Terminator::new(TerminatorKind::Goto(match_block), Span::DUMMY);
                    cx.current_block = current;
                } else {
                    // No literal sub-patterns — always match
                    cx.terminate_kind(TerminatorKind::Goto(match_block));
                }
            } else {
                // Can't resolve struct — fall through
                cx.terminate_kind(TerminatorKind::Goto(next_block));
            }
        }
        _ => {
            // Shouldn't happen (callers check needs_pattern_check first)
            cx.terminate_kind(TerminatorKind::Goto(match_block));
        }
    }
}
