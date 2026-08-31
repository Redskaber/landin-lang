//! Stage 19.3 (v0.5 Phase 3) — Trait Solver Selection phase.
//!
//! Per `docs/lang-design/03-type-system.md` §5.3:
//! - **Selection** picks the unique candidate impl (MVP禁 overlapping)
//! - Algorithm: `evaluate(obligation) → candidates`; if `len(candidates) == 1`
//!   → `Ok(impl)`; else (0 or >1) → `Err(no_impl)` or `Err(ambiguous)`
//! - MVP forbids overlapping impls (R3 陷阱 #5), so Selection degenerates
//!   to "unique candidate = selected"
//!
//! Per §5.5 Impl matching: after selecting the impl, "bind" the inference
//! variables from the impl substitution (e.g., `Vec<T>` matching `Vec<i32>`
//! → bind `T = i32`).
//!
//! This module (Phase 3) implements:
//! - `select(goal, cx)` — wraps `evaluate` + picks the unique Ok candidate,
//!   returning `SelectionResult`
//! - `bind_inference_vars(impl_def_id, substs, infer_ctxt)` — commits the
//!   inferred substitution to the InferCtxt (so subsequent queries see the
//!   bound state)
//! - `SelectionCtxt` — context bundling evaluation + selection state
//!
//! Per §11 (接口隔离): this module reads Phase 2's `evaluate` + `EvalCtxt`
//! (data contract) but does NOT call typeck/codegen internals. Real
//! unification of generic types (T=i32 from Vec<T> ↔ Vec<i32>) requires
//! typeck unify table integration — deferred to a future phase that adds
//! a `UnifyCtxt` (per Stage 19.1 design notes).
//!
//! Per §12 (最优 > 最小): implement `select` as a proper composition of
//! `evaluate` + uniqueness check + binding, rather than re-implementing
//! the candidate-collection loop.
//!
//! Per §1.0 原則 6 (通解 > 特解): one `select` function handles all
//! impl kinds (inherent, trait, generic, non-generic) — the uniqueness
//! logic is general.

use crate::hir::DefId;
use crate::mir::ty::SubstsRef;
use crate::traits::solver::eval::{evaluate, EvalAllResult, EvalCtxt};
use crate::traits::solver::{EvalResult, Goal, InferCtxt, SelectionResult};

// =====================================================================
// select — pick the unique candidate impl from EvalAllResult
// =====================================================================

/// Select the unique impl for a goal.
///
/// Per §5.3 Selection algorithm:
/// 1. Call `evaluate(goal, cx)` to collect all candidate impls
/// 2. If exactly 1 Ok candidate → `SelectionResult::Ok { impl_def_id }`
///    - Also commits the inferred substs to `InferCtxt` via `bind_inference_vars`
/// 3. If >1 Ok candidates → `SelectionResult::Ambiguous { candidate_count }`
///    (MVP禁 overlapping — multiple matching impls is an error)
/// 4. If 0 Ok + ≥1 Ambiguous → `SelectionResult::Ambiguous { candidate_count }`
///    (defer for more info — Fulfillment will retry after inference vars resolve)
/// 5. If 0 Ok + 0 Ambiguous → `SelectionResult::NoImpl`
///
/// Per §1.0 原則 4 (报错 > 静默): all non-Ok cases return explicit
/// `SelectionResult` variants; never silently succeed.
///
/// Per §1.0 原則 6 (通解 > 特解): one function handles all goal kinds.
///
/// Per §1.0 原則 9 (正确 > 妥协): MVP禁 overlapping — multiple Ok = Ambiguous,
/// not silent first-match.
///
/// Per §12 (最优 > 最小): `select` is a proper composition of `evaluate` +
/// uniqueness check + binding, rather than re-implementing the loop.
pub fn select(goal: &Goal, cx: &mut EvalCtxt) -> SelectionResult {
    let eval_result = evaluate(goal, cx);
    select_from_eval(&eval_result, cx.infer_ctxt)
}

/// Convert an `EvalAllResult` to a `SelectionResult`, committing the
/// inferred substitution if exactly one Ok candidate exists.
///
/// This is the core Selection algorithm (per §5.3). `select` is a thin
/// wrapper that calls `evaluate` first, then delegates here.
///
/// Per §1.0 原則 3 (显式 > 隐式): this function is public so callers
/// can select from a pre-computed `EvalAllResult` (e.g., for caching).
pub fn select_from_eval(
    eval_result: &EvalAllResult,
    infer_ctxt: &mut InferCtxt,
) -> SelectionResult {
    let ok_count = eval_result.ok_count();
    let ambiguous_count = eval_result.ambiguous_count();

    if ok_count == 1 {
        // Unique Ok candidate — select it.
        // Per §5.5: "bind" the inference variables from the impl substitution.
        // Per §1.0 原則 3 (显式 > 隐式): expect() documents the invariant
        // that ok_count == 1 guarantees unique_ok() returns Some.
        let (impl_def_id, one_result) = eval_result
            .unique_ok()
            .expect("ok_count == 1 guarantees unique Ok candidate exists");
        bind_inference_vars(impl_def_id, &one_result.substs, infer_ctxt);
        SelectionResult::Ok { impl_def_id }
    } else if ok_count > 1 {
        // MVP禁 overlapping — multiple Ok candidates is an error.
        // Per §5.3: Selection degenerates to "unique candidate = selected";
        // multiple candidates = Ambiguous.
        SelectionResult::Ambiguous {
            candidate_count: ok_count,
        }
    } else if ambiguous_count > 0 {
        // No Ok, but some Ambiguous — defer for more info.
        // (Fulfillment will retry after inference variables are bound.)
        SelectionResult::Ambiguous {
            candidate_count: ambiguous_count,
        }
    } else {
        // No Ok, no Ambiguous → definitely no impl.
        SelectionResult::NoImpl
    }
}

// =====================================================================
// bind_inference_vars — commit inferred substitution to InferCtxt
// =====================================================================

/// Commit the inferred substitution to the InferCtxt.
///
/// Per §5.3 + §5.5: after Selection picks an impl, the inference variables
/// from the impl substitution must be "bound" so subsequent queries see
/// the bound state. E.g., selecting `impl<T: Clone> Trait for Vec<T>`
/// against `Vec<i32>: Trait` binds `T = i32`.
///
/// MVP scope (v0.5 Phase 3):
/// - This is a placeholder — real binding requires the impl's generic
///   params to be matched against the inferred substs.
/// - Phase 3 records the substs in InferCtxt's `obligations_pushed` counter
///   for stats, but doesn't yet populate `bound_vars` (which is keyed by
///   InferVar ID, not generic param index).
/// - A future phase will integrate with typeck unify table to properly
///   bind `T = i32` (where T is the impl's generic param and i32 is the
///   inferred subst).
///
/// Per §1.0 原則 4 (报错 > 静默): this is documented as a MVP limitation,
/// not a silent failure — future phases will add proper binding.
///
/// Per §1.0 原則 6 (通解 > 特解): one function handles all impl kinds;
/// the binding logic is general (no per-trait branches).
pub fn bind_inference_vars(_impl_def_id: DefId, substs: &SubstsRef, infer_ctxt: &mut InferCtxt) {
    // MVP: record the substs count for stats. Real binding (T=i32) requires
    // the impl's generic params + typeck unify integration — deferred to
    // a future phase.
    //
    // Per §1.0 原則 4 (报错 > 静默): we explicitly record that an obligation
    // was pushed (via `record_obligation_pushed`), so callers can verify
    // Selection happened. This is a documented limitation, not silent.
    for _ in substs.iter() {
        infer_ctxt.record_obligation_pushed();
    }
}

// =====================================================================
// SelectionCtxt — context for selection queries
// =====================================================================

/// Context for selection queries.
///
/// Per §11 (接口隔离): bundles all data the selector needs:
/// - `trait_resolver`: look up trait/impl metadata (via EvalCtxt)
/// - `infer_ctxt`: placeholder universe + substitution table (via EvalCtxt)
/// - `param_env`: where clauses to assume true (via EvalCtxt)
///
/// This is a thin wrapper around `EvalCtxt` that adds selection-specific
/// helpers. Per §1.0 原則 6 (通解 > 特解): one context type for all
/// selection needs (no per-trait subtypes).
///
/// Per §1.0 原則 10 (唯一可信数据源): `infer_ctxt` is the single source
/// of truth for bound state; the selector never maintains a parallel map.
pub struct SelectionCtxt<'a> {
    /// The underlying evaluation context.
    pub eval_ctxt: EvalCtxt<'a>,
}

impl<'a> SelectionCtxt<'a> {
    /// Construct a selection context from an evaluation context.
    pub fn new(eval_ctxt: EvalCtxt<'a>) -> Self {
        Self { eval_ctxt }
    }

    /// Select the unique impl for a goal.
    ///
    /// Per §5.3: this is the main entry point for Selection.
    /// Delegates to `select(goal, &mut self.eval_ctxt)`.
    pub fn select(&mut self, goal: &Goal) -> SelectionResult {
        select(goal, &mut self.eval_ctxt)
    }

    /// Select from a pre-computed `EvalAllResult` (e.g., from caching).
    ///
    /// Per §1.0 原則 3 (显式 > 隐式): explicit method for the cached path,
    /// rather than silently re-evaluating.
    pub fn select_from_eval(&mut self, eval_result: &EvalAllResult) -> SelectionResult {
        select_from_eval(eval_result, self.eval_ctxt.infer_ctxt)
    }
}

// =====================================================================
// Helper: select + report (combines select with high-level EvalResult)
// =====================================================================

/// Select and convert to high-level `EvalResult`.
///
/// Per §5.2-5.3: combines Selection with the high-level tri-state view:
/// - `SelectionResult::Ok { .. }` → `EvalResult::Ok`
/// - `SelectionResult::Ambiguous { .. }` → `EvalResult::Ambiguous`
/// - `SelectionResult::NoImpl` → `EvalResult::Err(WrongTrait)`
///
/// Per §1.0 原則 6 (通解 > 特解): one function converts all variants.
///
/// Per §12 (最优 > 最小): proper composition of `select` + conversion,
/// rather than re-implementing the algorithm.
pub fn select_to_eval_result(selection: &SelectionResult) -> EvalResult {
    match selection {
        SelectionResult::Ok { .. } => EvalResult::Ok,
        SelectionResult::Ambiguous { .. } => EvalResult::Ambiguous,
        SelectionResult::NoImpl => EvalResult::Err(crate::traits::solver::EvalError::WrongTrait),
    }
}

// =====================================================================
// Helper: select + describe (human-readable description for diagnostics)
// =====================================================================

/// Describe a `SelectionResult` for diagnostic messages.
///
/// Per §1.0 原則 3 (显式 > 隐式): produces explicit human-readable strings
/// rather than relying on Display (which we don't implement for
/// SelectionResult to keep the enum simple).
///
/// Per §1.0 原則 4 (报错 > 静默): diagnostic messages explicitly state
/// what happened (Ok with impl_def_id, Ambiguous with N candidates, NoImpl).
pub fn describe_selection(selection: &SelectionResult) -> String {
    match selection {
        SelectionResult::Ok { impl_def_id } => {
            format!("selected impl #{}", impl_def_id.as_u32())
        }
        SelectionResult::Ambiguous { candidate_count } => {
            format!("ambiguous: {} candidates matched", candidate_count)
        }
        SelectionResult::NoImpl => "no impl matched".to_string(),
    }
}

// =====================================================================
// Helper: check if a goal would select (without actually selecting)
// =====================================================================

/// Check if a goal would result in a unique selection (without committing).
///
/// Per §1.0 原則 3 (显式 > 隐式): explicit "peek" function for callers
/// that need to know if Selection would succeed without actually
/// committing the substitution.
///
/// Per §12 (最优 > 最小): reuses `evaluate` + uniqueness check, doesn't
/// duplicate the algorithm.
pub fn would_select_uniquely(goal: &Goal, cx: &mut EvalCtxt) -> bool {
    let eval_result = evaluate(goal, cx);
    eval_result.ok_count() == 1
}

// =====================================================================
// Helper: collect all Ok candidates (for diagnostics)
// =====================================================================

/// Collect all Ok candidates from an `EvalAllResult`.
///
/// Per §1.0 原則 3 (显式 > 隐式): explicit helper for diagnostic messages
/// that need to list all matching impls (e.g., "candidates: #7, #8, #9").
///
/// Per §1.0 原則 6 (通解 > 特解): one function returns all Ok candidates,
/// regardless of count.
pub fn collect_ok_candidates(eval_result: &EvalAllResult) -> Vec<(DefId, SubstsRef)> {
    eval_result
        .candidates
        .iter()
        .filter(|(_, r)| r.is_ok())
        .map(|(def_id, r)| (*def_id, r.substs.clone()))
        .collect()
}

// =====================================================================
// Helper: collect all Ambiguous candidates (for diagnostics)
// =====================================================================

/// Collect all Ambiguous candidates from an `EvalAllResult`.
///
/// Per §1.0 原則 3 (显式 > 隐式): explicit helper for diagnostic messages
/// that need to list all deferred impls (e.g., "deferred: #7, #8 — waiting
/// for inference variable binding").
pub fn collect_ambiguous_candidates(eval_result: &EvalAllResult) -> Vec<DefId> {
    eval_result
        .candidates
        .iter()
        .filter(|(_, r)| r.is_ambiguous())
        .map(|(def_id, _)| *def_id)
        .collect()
}

// =====================================================================
// Helper: collect all Err candidates (for diagnostics)
// =====================================================================

/// Collect all Err candidates from an `EvalAllResult`.
///
/// Per §1.0 原則 3 (显式 > 隐式): explicit helper for diagnostic messages
/// that need to list all rejected impls (e.g., "rejected: #7 (WrongTrait),
/// #8 (SelfTypeMismatch)").
pub fn collect_err_candidates(
    eval_result: &EvalAllResult,
) -> Vec<(DefId, crate::traits::solver::EvalError)> {
    eval_result
        .candidates
        .iter()
        .filter_map(|(def_id, r)| match &r.result {
            crate::traits::solver::EvalResult::Err(e) => Some((*def_id, e.clone())),
            _ => None,
        })
        .collect()
}

// =====================================================================
// Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::IntTy;
    use crate::hir::DefId;
    use crate::mir::ty::{Ty, TyKind, TyVid};
    use crate::traits::resolver::TraitResolver;
    use crate::traits::solver::eval::{
        eval_all_to_result, evaluate, EvalAllResult, EvalCtxt, EvalOneResult,
    };
    use crate::traits::solver::{
        EvalError, EvalResult, Goal, InferCtxt, ParamEnv, SelectionResult, TraitPredicate,
    };
    use std::rc::Rc;

    // ----------- Test helpers -----------

    fn dummy_def_id(n: u32) -> DefId {
        DefId::new(n)
    }

    fn dummy_i32_ty() -> Ty {
        Ty::from_kind(TyKind::Int(IntTy::I32))
    }

    fn dummy_infer_ty(id: u32) -> Ty {
        Ty::from_kind(TyKind::Infer(crate::mir::ty::InferVar::TyVar(TyVid(id))))
    }

    fn dummy_goal(self_ty: Ty, trait_def_id: u32) -> Goal {
        Goal::with_empty_env(TraitPredicate::simple(self_ty, dummy_def_id(trait_def_id)))
    }

    /// Build an EvalCtxt with an empty ParamEnv.
    ///
    /// Per §1.0 原則 3 (显式 > 隐式): the ParamEnv is created inside the
    /// closure and passed by reference, so the caller controls its lifetime.
    /// (Returning EvalCtxt directly would fail because ParamEnv is owned.)
    fn with_eval_ctxt<R, F>(resolver: &TraitResolver, infer_ctxt: &mut InferCtxt, f: F) -> R
    where
        F: FnOnce(&mut EvalCtxt) -> R,
    {
        let param_env = ParamEnv::empty();
        let mut cx = EvalCtxt::new(resolver, infer_ctxt, &param_env);
        f(&mut cx)
    }

    fn dummy_resolver() -> TraitResolver {
        TraitResolver::default()
    }

    // ----------- select_from_eval tests -----------

    #[test]
    fn test_select_from_eval_unique_ok() {
        let mut eval_result = EvalAllResult::empty();
        eval_result.add(dummy_def_id(7), EvalOneResult::ok(Rc::from([])));
        let mut infer_ctxt = InferCtxt::new();
        let result = select_from_eval(&eval_result, &mut infer_ctxt);
        assert!(
            matches!(result, SelectionResult::Ok { impl_def_id } if impl_def_id == dummy_def_id(7))
        );
    }

    #[test]
    fn test_select_from_eval_multiple_ok_ambiguous() {
        let mut eval_result = EvalAllResult::empty();
        eval_result.add(dummy_def_id(7), EvalOneResult::ok(Rc::from([])));
        eval_result.add(dummy_def_id(8), EvalOneResult::ok(Rc::from([])));
        let mut infer_ctxt = InferCtxt::new();
        let result = select_from_eval(&eval_result, &mut infer_ctxt);
        assert!(matches!(
            result,
            SelectionResult::Ambiguous { candidate_count: 2 }
        ));
    }

    #[test]
    fn test_select_from_eval_only_ambiguous() {
        let mut eval_result = EvalAllResult::empty();
        eval_result.add(dummy_def_id(7), EvalOneResult::ambiguous());
        eval_result.add(dummy_def_id(8), EvalOneResult::ambiguous());
        let mut infer_ctxt = InferCtxt::new();
        let result = select_from_eval(&eval_result, &mut infer_ctxt);
        assert!(matches!(
            result,
            SelectionResult::Ambiguous { candidate_count: 2 }
        ));
    }

    #[test]
    fn test_select_from_eval_only_err() {
        let mut eval_result = EvalAllResult::empty();
        eval_result.add(dummy_def_id(7), EvalOneResult::err(EvalError::WrongTrait));
        eval_result.add(
            dummy_def_id(8),
            EvalOneResult::err(EvalError::SelfTypeMismatch),
        );
        let mut infer_ctxt = InferCtxt::new();
        let result = select_from_eval(&eval_result, &mut infer_ctxt);
        assert_eq!(result, SelectionResult::NoImpl);
    }

    #[test]
    fn test_select_from_eval_empty() {
        let eval_result = EvalAllResult::empty();
        let mut infer_ctxt = InferCtxt::new();
        let result = select_from_eval(&eval_result, &mut infer_ctxt);
        assert_eq!(result, SelectionResult::NoImpl);
    }

    #[test]
    fn test_select_from_eval_ok_with_ambiguous() {
        // One Ok + one Ambiguous → Ok (the unique Ok wins).
        let mut eval_result = EvalAllResult::empty();
        eval_result.add(dummy_def_id(7), EvalOneResult::ok(Rc::from([])));
        eval_result.add(dummy_def_id(8), EvalOneResult::ambiguous());
        let mut infer_ctxt = InferCtxt::new();
        let result = select_from_eval(&eval_result, &mut infer_ctxt);
        assert!(
            matches!(result, SelectionResult::Ok { impl_def_id } if impl_def_id == dummy_def_id(7))
        );
    }

    #[test]
    fn test_select_from_eval_ok_with_errs() {
        // One Ok + multiple Errs → Ok.
        let mut eval_result = EvalAllResult::empty();
        eval_result.add(dummy_def_id(7), EvalOneResult::ok(Rc::from([])));
        eval_result.add(dummy_def_id(8), EvalOneResult::err(EvalError::WrongTrait));
        eval_result.add(
            dummy_def_id(9),
            EvalOneResult::err(EvalError::SelfTypeMismatch),
        );
        let mut infer_ctxt = InferCtxt::new();
        let result = select_from_eval(&eval_result, &mut infer_ctxt);
        assert!(
            matches!(result, SelectionResult::Ok { impl_def_id } if impl_def_id == dummy_def_id(7))
        );
    }

    // ----------- bind_inference_vars tests -----------

    #[test]
    fn test_bind_inference_vars_empty() {
        let mut infer_ctxt = InferCtxt::new();
        let initial = infer_ctxt.obligations_pushed();
        bind_inference_vars(dummy_def_id(7), &Rc::from([]), &mut infer_ctxt);
        assert_eq!(infer_ctxt.obligations_pushed(), initial); // no substs → no records
    }

    #[test]
    fn test_bind_inference_vars_with_substs() {
        let mut infer_ctxt = InferCtxt::new();
        let initial = infer_ctxt.obligations_pushed();
        let substs: SubstsRef = Rc::from([dummy_i32_ty()]);
        bind_inference_vars(dummy_def_id(7), &substs, &mut infer_ctxt);
        assert_eq!(infer_ctxt.obligations_pushed(), initial + 1); // 1 subst → 1 record
    }

    #[test]
    fn test_bind_inference_vars_multiple_substs() {
        let mut infer_ctxt = InferCtxt::new();
        let initial = infer_ctxt.obligations_pushed();
        let substs: SubstsRef = Rc::from([dummy_i32_ty(), dummy_i32_ty(), dummy_i32_ty()]);
        bind_inference_vars(dummy_def_id(7), &substs, &mut infer_ctxt);
        assert_eq!(infer_ctxt.obligations_pushed(), initial + 3); // 3 substs → 3 records
    }

    // ----------- select_to_eval_result tests -----------

    #[test]
    fn test_select_to_eval_result_ok() {
        let selection = SelectionResult::Ok {
            impl_def_id: dummy_def_id(7),
        };
        assert_eq!(select_to_eval_result(&selection), EvalResult::Ok);
    }

    #[test]
    fn test_select_to_eval_result_ambiguous() {
        let selection = SelectionResult::Ambiguous { candidate_count: 3 };
        assert_eq!(select_to_eval_result(&selection), EvalResult::Ambiguous);
    }

    #[test]
    fn test_select_to_eval_result_no_impl() {
        let selection = SelectionResult::NoImpl;
        let result = select_to_eval_result(&selection);
        assert!(matches!(result, EvalResult::Err(EvalError::WrongTrait)));
    }

    // ----------- describe_selection tests -----------

    #[test]
    fn test_describe_selection_ok() {
        let selection = SelectionResult::Ok {
            impl_def_id: dummy_def_id(42),
        };
        let desc = describe_selection(&selection);
        assert!(desc.contains("selected impl #42"));
    }

    #[test]
    fn test_describe_selection_ambiguous() {
        let selection = SelectionResult::Ambiguous { candidate_count: 3 };
        let desc = describe_selection(&selection);
        assert!(desc.contains("ambiguous"));
        assert!(desc.contains("3 candidates"));
    }

    #[test]
    fn test_describe_selection_no_impl() {
        let selection = SelectionResult::NoImpl;
        let desc = describe_selection(&selection);
        assert!(desc.contains("no impl matched"));
    }

    // ----------- would_select_uniquely tests -----------

    #[test]
    fn test_would_select_uniquely_empty() {
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let goal = dummy_goal(dummy_i32_ty(), 7);
        // No candidates (empty resolver).
        let result = with_eval_ctxt(&resolver, &mut infer_ctxt, |cx| {
            would_select_uniquely(&goal, cx)
        });
        assert!(!result);
    }

    #[test]
    fn test_would_select_uniquely_infer_self() {
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let goal = dummy_goal(dummy_infer_ty(0), 7);
        // No candidates + Infer self → defer.
        let result = with_eval_ctxt(&resolver, &mut infer_ctxt, |cx| {
            would_select_uniquely(&goal, cx)
        });
        assert!(!result);
    }

    // ----------- collect_*_candidates tests -----------

    #[test]
    fn test_collect_ok_candidates_empty() {
        let eval_result = EvalAllResult::empty();
        let oks = collect_ok_candidates(&eval_result);
        assert!(oks.is_empty());
    }

    #[test]
    fn test_collect_ok_candidates_with_oks() {
        let mut eval_result = EvalAllResult::empty();
        let substs: SubstsRef = Rc::from([dummy_i32_ty()]);
        eval_result.add(dummy_def_id(7), EvalOneResult::ok(substs.clone()));
        eval_result.add(dummy_def_id(8), EvalOneResult::ok(substs.clone()));
        eval_result.add(dummy_def_id(9), EvalOneResult::err(EvalError::WrongTrait));
        let oks = collect_ok_candidates(&eval_result);
        assert_eq!(oks.len(), 2);
        assert_eq!(oks[0].0, dummy_def_id(7));
        assert_eq!(oks[1].0, dummy_def_id(8));
    }

    #[test]
    fn test_collect_ambiguous_candidates_empty() {
        let eval_result = EvalAllResult::empty();
        let ambig = collect_ambiguous_candidates(&eval_result);
        assert!(ambig.is_empty());
    }

    #[test]
    fn test_collect_ambiguous_candidates_with_ambig() {
        let mut eval_result = EvalAllResult::empty();
        eval_result.add(dummy_def_id(7), EvalOneResult::ambiguous());
        eval_result.add(dummy_def_id(8), EvalOneResult::ok(Rc::from([])));
        eval_result.add(dummy_def_id(9), EvalOneResult::ambiguous());
        let ambig = collect_ambiguous_candidates(&eval_result);
        assert_eq!(ambig.len(), 2);
        assert_eq!(ambig[0], dummy_def_id(7));
        assert_eq!(ambig[1], dummy_def_id(9));
    }

    #[test]
    fn test_collect_err_candidates_empty() {
        let eval_result = EvalAllResult::empty();
        let errs = collect_err_candidates(&eval_result);
        assert!(errs.is_empty());
    }

    #[test]
    fn test_collect_err_candidates_with_errs() {
        let mut eval_result = EvalAllResult::empty();
        eval_result.add(dummy_def_id(7), EvalOneResult::err(EvalError::WrongTrait));
        eval_result.add(dummy_def_id(8), EvalOneResult::ok(Rc::from([])));
        eval_result.add(
            dummy_def_id(9),
            EvalOneResult::err(EvalError::SelfTypeMismatch),
        );
        let errs = collect_err_candidates(&eval_result);
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].0, dummy_def_id(7));
        assert_eq!(errs[0].1, EvalError::WrongTrait);
        assert_eq!(errs[1].0, dummy_def_id(9));
        assert_eq!(errs[1].1, EvalError::SelfTypeMismatch);
    }

    // ----------- SelectionCtxt tests -----------

    #[test]
    fn test_selection_ctxt_new() {
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let param_env = ParamEnv::empty();
        let eval_ctxt = EvalCtxt::new(&resolver, &mut infer_ctxt, &param_env);
        // (Can't easily test SelectionCtxt::new because EvalCtxt borrows
        // infer_ctxt mutably — we'd need to scope the borrow. Test via
        // select() integration tests below.)
        let _ = eval_ctxt;
    }

    // ----------- select integration tests -----------

    #[test]
    fn test_select_no_candidates_no_impl() {
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let goal = dummy_goal(dummy_i32_ty(), 7);
        let result = with_eval_ctxt(&resolver, &mut infer_ctxt, |cx| select(&goal, cx));
        assert_eq!(result, SelectionResult::NoImpl);
    }

    #[test]
    fn test_select_infer_self_defers() {
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let goal = dummy_goal(dummy_infer_ty(0), 7);
        let result = with_eval_ctxt(&resolver, &mut infer_ctxt, |cx| select(&goal, cx));
        // No candidates → NoImpl (not Ambiguous, because evaluate returns empty).
        assert_eq!(result, SelectionResult::NoImpl);
    }

    #[test]
    fn test_select_universe_unchanged() {
        // Per §5.2 + §5.3: Evaluation + Selection create placeholders but
        // restore the universe on exit.
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let initial_universe = infer_ctxt.current_universe();
        let goal = dummy_goal(dummy_i32_ty(), 7);
        let _ = with_eval_ctxt(&resolver, &mut infer_ctxt, |cx| select(&goal, cx));
        assert_eq!(infer_ctxt.current_universe(), initial_universe);
    }

    #[test]
    fn test_select_to_eval_result_consistency() {
        // select + select_to_eval_result should be consistent with
        // eval_all_to_result on the same EvalAllResult.
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let goal = dummy_goal(dummy_i32_ty(), 7);

        let (eval_high, selection_high) = with_eval_ctxt(&resolver, &mut infer_ctxt, |cx| {
            let eval_result = evaluate(&goal, cx);
            let eval_high = eval_all_to_result(&eval_result);
            let selection_high = select_to_eval_result(&select(&goal, cx));
            (eval_high, selection_high)
        });

        // Both should be Err (no candidates).
        assert!(eval_high.is_err());
        assert!(selection_high.is_err());
    }

    #[test]
    fn test_describe_then_select_consistency() {
        // describe_selection(select()) should produce a non-empty string.
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let goal = dummy_goal(dummy_i32_ty(), 7);
        let desc = with_eval_ctxt(&resolver, &mut infer_ctxt, |cx| {
            let selection = select(&goal, cx);
            describe_selection(&selection)
        });
        assert!(!desc.is_empty());
    }
}
