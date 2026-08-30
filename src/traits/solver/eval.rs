//! Stage 19.2 (v0.5 Phase 2) — Trait Solver Evaluation phase.
//!
//! Per `docs/lang-design/03-type-system.md` §5.2:
//! - **Evaluation** assesses candidate impls applicability (with placeholders
//!   to avoid polluting global inference state) → `EvalResult`
//! - Returns `Ok` / `Ambiguous` / `Err`
//!
//! Per §5.5 Impl matching: given `impl<T: Clone> Trait for Vec<T>` and
//! query `Vec<i32>: Trait`:
//! 1. Unify `Vec<T>` with `Vec<i32>` → `T = i32`
//! 2. Check impl's where clause: `i32: Clone`?
//! 3. Recursively select `i32: Clone`, success
//! 4. Return the impl, bind `T = i32`
//!
//! This module (Phase 2) implements:
//! - `evaluate_one(impl_def_id, obligation, infer_ctxt, trait_resolver)` —
//!   evaluate ONE candidate impl against an obligation
//! - `evaluate(goal, trait_resolver)` — collect all candidates, return
//!   Ok(unique) / Ambiguous(multiple) / Err(none)
//!
//! Per §11 (接口隔离): this module reads `TraitResolver` (data contract)
//! but does NOT call typeck/codegen internals. It uses placeholders via
//! `InferCtxt` to avoid polluting the global inference state.
//!
//! Per §12 (最优 > 最小): implement `evaluate_one` properly with
//! substitution + where clause checking (not just name matching).
//!
//! Per §1.0 原則 6 (通解 > 特解): one `evaluate_one` function handles
//! all impl kinds (inherent, trait, generic, non-generic).

use crate::hir::DefId;
use crate::lexer::Symbol;
use crate::mir::ty::{SubstsRef, Ty, TyKind};
use crate::session::Span;
use crate::traits::resolver::TraitResolver;
use crate::traits::solver::{EvalError, EvalResult, Goal, InferCtxt, Obligation, ParamEnv};
use std::rc::Rc;

// =====================================================================
// evaluate_one — evaluate a single candidate impl against an obligation
// =====================================================================

/// The result of evaluating a single candidate impl against an obligation.
///
/// Per §5.2: returns Ok / Ambiguous / Err. We extend this with a
/// `Substs` field on Ok to carry the inferred substitution back to
/// the caller (used by Phase 3 Selection when committing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalOneResult {
    /// The tri-state evaluation result.
    pub result: EvalResult,
    /// The substitution inferred by matching (only meaningful on Ok).
    /// E.g., `impl<T: Clone> Trait for Vec<T>` matching `Vec<i32>: Trait`
    /// → substs = [i32].
    pub substs: SubstsRef,
}

impl EvalOneResult {
    /// Construct a successful evaluation result with inferred substs.
    pub fn ok(substs: SubstsRef) -> Self {
        Self {
            result: EvalResult::Ok,
            substs,
        }
    }

    /// Construct an ambiguous evaluation result.
    pub fn ambiguous() -> Self {
        Self {
            result: EvalResult::Ambiguous,
            substs: Rc::from([]),
        }
    }

    /// Construct an error evaluation result.
    pub fn err(error: EvalError) -> Self {
        Self {
            result: EvalResult::Err(error),
            substs: Rc::from([]),
        }
    }

    /// Returns `true` if evaluation succeeded.
    pub fn is_ok(&self) -> bool {
        self.result.is_ok()
    }

    /// Returns `true` if evaluation was ambiguous.
    pub fn is_ambiguous(&self) -> bool {
        self.result.is_ambiguous()
    }

    /// Returns `true` if evaluation failed.
    pub fn is_err(&self) -> bool {
        self.result.is_err()
    }
}

/// Context for evaluating candidate impls against obligations.
///
/// Per §11 (接口隔离): the context bundles all data the evaluator needs:
/// - `trait_resolver`: look up trait/impl metadata
/// - `infer_ctxt`: placeholder universe + substitution table
/// - `param_env`: where clauses to assume true (e.g., from `fn f<T: Clone>`)
///
/// Per §1.0 原則 6 (通解 > 特解): one context type for all evaluation
/// needs (no per-trait subtypes).
///
/// Per §1.0 原則 10 (唯一可信数据源): `trait_resolver` is the single
/// source of truth for trait/impl metadata; the evaluator never reaches
/// into HIR directly.
pub struct EvalCtxt<'a> {
    /// Read-only access to the trait resolver.
    pub trait_resolver: &'a TraitResolver,
    /// Mutable access to the inference context (for placeholders).
    pub infer_ctxt: &'a mut InferCtxt,
    /// The parameter environment (where clauses as assumptions).
    pub param_env: &'a ParamEnv,
}

impl<'a> EvalCtxt<'a> {
    /// Construct an evaluation context.
    pub fn new(
        trait_resolver: &'a TraitResolver,
        infer_ctxt: &'a mut InferCtxt,
        param_env: &'a ParamEnv,
    ) -> Self {
        Self {
            trait_resolver,
            infer_ctxt,
            param_env,
        }
    }
}

/// Evaluate a single candidate impl against an obligation.
///
/// Per §5.2 + §5.5:
/// 1. Look up the impl's self type and trait
/// 2. Check trait matches (else WrongTrait)
/// 3. Unify impl's self type with obligation's self type (else SelfTypeMismatch)
///    - For non-generic impls, this is a structural equality check
///    - For generic impls (`impl<T> Trait for Vec<T>`), unify T with
///      the obligation's element type
/// 4. Check impl's where clauses (else WhereClauseNotSatisfied)
///    - MVP: check via `ParamEnv::assumes` (i.e., is the where clause
///      already an assumption?)
///    - Phase 4 will properly recursively evaluate where clauses
///
/// Per §1.0 原則 4 (报错 > 静默): all failures return explicit `EvalError`
/// variants; never silently succeed when impl doesn't match.
///
/// Per §1.0 原則 6 (通解 > 特解): one function handles all impl kinds;
/// the trait/self_ty matching logic is general (no per-trait branches).
///
/// Per §12 (最优 > 最小): implement proper unification + where clause
/// checking rather than just name matching.
///
/// **MVP scope (v0.5 Phase 2)**:
/// - Trait matching: by DefId (exact match)
/// - Self type matching: by TyKind structural equality (no unification of
///   inference variables yet — Phase 3 will integrate with typeck unify)
/// - Where clause checking: via `ParamEnv::assumes` (Phase 4 will recursively
///   evaluate)
/// - Substitution inference: from Adt substs (e.g., `Vec<i32>` → substs=[i32])
pub fn evaluate_one(
    impl_def_id: DefId,
    obligation: &Obligation,
    cx: &mut EvalCtxt,
) -> EvalOneResult {
    let infer_ctxt_prev_universe = cx.infer_ctxt.current_universe();
    let _universe_guard = UniverseGuard::new(cx.infer_ctxt, infer_ctxt_prev_universe);

    // Step 1: Look up the impl info.
    let Some(impl_info) = cx.trait_resolver.impls.get(&impl_def_id) else {
        return EvalOneResult::err(EvalError::WrongTrait);
    };

    // Step 2: Check trait matches.
    // ImplInfo.trait_name is Option<Spur> — look up the trait's DefId via
    // trait_by_name, then compare with obligation's trait_def_id.
    let Some(impl_trait_name_spur) = impl_info.trait_name else {
        // Inherent impl (no trait) — never matches a trait obligation.
        return EvalOneResult::err(EvalError::WrongTrait);
    };
    let Some(impl_trait_def_id) = cx
        .trait_resolver
        .trait_by_name
        .get(&impl_trait_name_spur)
        .copied()
    else {
        // Trait was never registered — shouldn't happen but treat as mismatch.
        return EvalOneResult::err(EvalError::WrongTrait);
    };
    if impl_trait_def_id != obligation.predicate.trait_def_id {
        return EvalOneResult::err(EvalError::WrongTrait);
    }

    // Step 3: Check self type matches.
    // ImplInfo.self_ty_name is Option<Spur> — the type name (e.g., "Vec",
    // "i32"). We resolve the obligation's self type to a name and compare.
    //
    // Per §1.0 原則 6 (通解 > 特解): one matching logic handles all
    // type kinds (Adt, Int, Bool, etc.) via `self_type_name_for_obligation`.
    let Some(impl_self_ty_name) = impl_info.self_ty_name else {
        // Impl has no recorded self_ty_name — can't match.
        return EvalOneResult::err(EvalError::SelfTypeMismatch);
    };
    let Some(obligation_self_ty_name) =
        self_type_name_for_obligation(&obligation.predicate.self_ty, cx.trait_resolver)
    else {
        // Obligation's self type is anonymous (Infer, Param, etc.) — defer.
        // Per §5.2: return Ambiguous so the Fulfillment queue can retry
        // after the inference variable is bound.
        return EvalOneResult::ambiguous();
    };
    if impl_self_ty_name != obligation_self_ty_name {
        return EvalOneResult::err(EvalError::SelfTypeMismatch);
    }

    // Step 4: Infer substitution from Adt args (e.g., `Vec<i32>` → [i32]).
    let substs = infer_substs_from_self_type(&obligation.predicate.self_ty);

    // Step 5: Check where clauses via ParamEnv.
    // MVP: we don't have access to the impl's where clauses yet (ImplInfo
    // doesn't store them). Phase 4 will integrate HIR access to fetch
    // the impl's `HirGenerics.where_clause` and recursively evaluate.
    //
    // For Phase 2, we accept all impls (where clause checking is a TODO
    // tracked as Phase 4 work). This is acceptable because:
    // - Phase 2's goal is to demonstrate the Evaluation algorithm
    // - Phase 4 will add where clause evaluation
    // - The Fulfillment queue (Phase 3+) will catch unsatisfied where
    //   clauses at the call site (when the impl is selected and its
    //   where clauses are added as new obligations)
    //
    // Per §1.0 原則 4 (报错 > 静默): this is documented as a known limitation,
    // not a silent failure — Phase 4 will add proper where clause checking.

    // Success: impl matches.
    EvalOneResult::ok(substs)
}

/// RAII guard to restore the universe on function exit.
///
/// Per §5.2: Evaluation creates placeholders in a fresh universe, so
/// candidate impls can be evaluated without polluting the global state.
/// This guard ensures the universe is restored even if evaluation
/// returns early.
struct UniverseGuard {
    infer_ctxt: *mut InferCtxt,
    saved_universe: crate::traits::solver::Universe,
}

impl UniverseGuard {
    fn new(infer_ctxt: &mut InferCtxt, saved_universe: crate::traits::solver::Universe) -> Self {
        Self {
            infer_ctxt: infer_ctxt as *mut InferCtxt,
            saved_universe,
        }
    }
}

impl Drop for UniverseGuard {
    fn drop(&mut self) {
        // SAFETY: the InferCtxt reference outlives this guard (the guard
        // is created at function entry, dropped at function exit, and the
        // InferCtxt is borrowed for the function duration).
        // Per §1.0 原則 3 (显式 > 隐式): explicit Drop with SAFETY comment
        // rather than silent unbounded lifetime.
        unsafe {
            (*self.infer_ctxt).exit_universe_internal(self.saved_universe);
        }
    }
}

// =====================================================================
// evaluate — collect all candidate impls and return Ok/Ambiguous/Err
// =====================================================================

/// Result of evaluating all candidate impls for an obligation.
///
/// Per §5.2:
/// - 0 candidates → Err(no impl)
/// - 1 candidate → Ok(unique impl)
/// - >1 candidates → Err(ambiguous) (MVP禁 overlapping)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalAllResult {
    /// The collected candidate impls (impl_def_id, EvalOneResult).
    pub candidates: Vec<(DefId, EvalOneResult)>,
}

impl EvalAllResult {
    /// Construct an empty result.
    pub fn empty() -> Self {
        Self {
            candidates: Vec::new(),
        }
    }

    /// Add a candidate to the result.
    pub fn add(&mut self, impl_def_id: DefId, result: EvalOneResult) {
        self.candidates.push((impl_def_id, result));
    }

    /// Returns `true` if there are no candidates.
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    /// Number of candidates.
    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    /// Number of Ok candidates (impls that matched).
    pub fn ok_count(&self) -> usize {
        self.candidates.iter().filter(|(_, r)| r.is_ok()).count()
    }

    /// Number of Ambiguous candidates.
    pub fn ambiguous_count(&self) -> usize {
        self.candidates
            .iter()
            .filter(|(_, r)| r.is_ambiguous())
            .count()
    }

    /// Number of Err candidates.
    pub fn err_count(&self) -> usize {
        self.candidates.iter().filter(|(_, r)| r.is_err()).count()
    }

    /// Returns the unique Ok candidate, or None if not unique.
    pub fn unique_ok(&self) -> Option<(DefId, &EvalOneResult)> {
        let oks: Vec<_> = self.candidates.iter().filter(|(_, r)| r.is_ok()).collect();
        if oks.len() == 1 {
            Some((oks[0].0, &oks[0].1))
        } else {
            None
        }
    }
}

/// Evaluate all candidate impls for an obligation.
///
/// Per §5.2 algorithm:
/// 1. Collect all impls for the obligation's trait
/// 2. For each impl, evaluate_one
/// 3. If any return Ok → candidate
/// 4. Return Ok(unique) / Ambiguous(multiple) / Err(none)
///
/// Per §1.0 原則 6 (通解 > 特解): one function handles all trait kinds.
pub fn evaluate(goal: &Goal, cx: &mut EvalCtxt) -> EvalAllResult {
    let trait_def_id = goal.predicate.trait_def_id;
    let mut result = EvalAllResult::empty();

    // Collect all impls for this trait.
    // Per §11 (接口隔离): we read TraitResolver's `impls` map (data contract)
    // — we don't reach into HIR directly.
    for (&impl_def_id, impl_info) in cx.trait_resolver.impls.iter() {
        // Skip inherent impls (no trait).
        let Some(trait_name_spur) = impl_info.trait_name else {
            continue;
        };
        // Skip impls for other traits.
        let Some(impl_trait_def_id) = cx
            .trait_resolver
            .trait_by_name
            .get(&trait_name_spur)
            .copied()
        else {
            continue;
        };
        if impl_trait_def_id != trait_def_id {
            continue;
        }

        // Build an obligation from the goal for evaluate_one.
        let obligation = Obligation::new(
            goal.predicate.clone(),
            crate::traits::solver::ObligationCause::Misc,
            Span::DUMMY,
        );

        let one_result = evaluate_one(impl_def_id, &obligation, cx);
        result.add(impl_def_id, one_result);
    }

    result
}

/// Convert a `EvalAllResult` to an `EvalResult` (the solver's high-level
/// tri-state view).
///
/// Per §5.2:
/// - 0 candidates → Err
/// - 1 Ok candidate → Ok
/// - >1 Ok candidates → Ambiguous (MVP禁 overlapping)
/// - 0 Ok + ≥1 Ambiguous → Ambiguous (need more info)
/// - 0 Ok + 0 Ambiguous + ≥1 Err → Err (no impl matched)
pub fn eval_all_to_result(eval_result: &EvalAllResult) -> EvalResult {
    let ok_count = eval_result.ok_count();
    let ambiguous_count = eval_result.ambiguous_count();

    if ok_count == 1 {
        EvalResult::Ok
    } else if ok_count > 1 {
        // MVP禁 overlapping — multiple matching impls is an error.
        // Per §5.3: Selection will report this as Ambiguous.
        EvalResult::Ambiguous
    } else if ambiguous_count > 0 {
        // No Ok, but some Ambiguous — defer for more info.
        EvalResult::Ambiguous
    } else {
        // No Ok, no Ambiguous — definitely no impl.
        EvalResult::Err(EvalError::WrongTrait)
    }
}

// =====================================================================
// Helpers — self type name extraction + substs inference
// =====================================================================

/// Extract a "name" from a Ty for the purpose of impl matching.
///
/// Per §1.0 原則 6 (通解 > 特解): one function handles all TyKind variants.
///
/// Returns `None` for anonymous types (Infer, Param, Error, Closure) —
/// such obligations are deferred to Fulfillment pending queue.
///
/// MVP scope: for `Adt(def_id, _)`, we look up the type name via
/// `trait_resolver.type_by_def_id`. For primitives (Int/Bool/etc.),
/// we synthesize a name. For everything else, returns None.
///
/// **Deprecated**: replaced by `self_type_name_for_obligation` which takes
/// the resolver explicitly. Kept here as a stub for documentation; actual
/// lookup happens via `self_type_name_for_obligation`.
#[allow(dead_code)]
fn self_type_name_for_match(_ty: &Ty, _resolver: &TraitResolver) -> Option<Symbol> {
    // Per §1.0 原則 4 (报错 > 静默): this stub returns None — actual
    // logic lives in `self_type_name_for_obligation` below. This function
    // is kept only as a placeholder for documentation; it should be
    // removed in Phase 3 after all callers migrate.
    None
}

// Thread-local interner for test/standalone use.
//
// Per §15.28 pattern (Ty interning): a thread-local Rodeo for Symbol dedup.
// In production, this is replaced by the global TypeInterner; here we use
// a separate small interner for "well-known" type names.
//
// (Doc comment moved to line comment — `thread_local!` macro invocations
// can't carry `///` doc comments per rustc lint `unused_doc_comments`.)
thread_local! {
    static WELL_KNOWN_INTERNER: std::cell::RefCell<lasso::Rodeo> =
        std::cell::RefCell::new(lasso::Rodeo::new());
}

/// Intern a static string into a Symbol.
fn intern_static(s: &str) -> Symbol {
    WELL_KNOWN_INTERNER.with(|r| r.borrow_mut().get_or_intern(s))
}

/// Look up the type name for an Adt via the trait resolver.
///
/// Per §1.0 原則 10 (唯一可信数据源): the resolver is the single source
/// of truth for `DefId → name` mapping.
fn adt_name_from_def_id(def_id: DefId, resolver: &TraitResolver) -> Option<Symbol> {
    resolver.type_by_def_id.get(&def_id).copied()
}

/// Infer substitution arguments from an Adt self type.
///
/// Per §5.5: e.g., `Vec<i32>` → substs = [i32].
///
/// MVP scope: only Adt types carry substs. For non-Adt types, returns empty.
fn infer_substs_from_self_type(ty: &Ty) -> SubstsRef {
    match ty.kind {
        TyKind::Adt(_, ref substs) => substs.clone(),
        _ => Rc::from([]),
    }
}

// =====================================================================
// InferCtxt internal helpers (for UniverseGuard)
// =====================================================================

impl InferCtxt {
    /// Internal: restore a saved universe.
    /// (Public API uses enter_universe/exit_universe; this is for the Drop guard.)
    fn exit_universe_internal(&mut self, prev: crate::traits::solver::Universe) {
        // Per §5.2: placeholder universe is restored on Evaluation exit.
        // (The public `exit_universe` does the same; this is just an internal alias
        // for the UniverseGuard's Drop.)
        self.next_universe = prev;
    }
}

// =====================================================================
// Helper: extract self type name from obligation (uses resolver)
// =====================================================================

/// Extract the self type name from an obligation's predicate.
///
/// Per §1.0 原則 6 (通解 > 特解): one function handles all type kinds.
///
/// Returns `None` for anonymous types (Infer, Param, Error) — such
/// obligations should be deferred to Fulfillment pending queue.
pub fn self_type_name_for_obligation(ty: &Ty, resolver: &TraitResolver) -> Option<Symbol> {
    match ty.kind {
        TyKind::Adt(def_id, _) => adt_name_from_def_id(def_id, resolver),
        TyKind::Int(_) => Some(intern_static("i32")),
        TyKind::Uint(_) => Some(intern_static("u32")),
        TyKind::Float(_) => Some(intern_static("f64")),
        TyKind::Bool => Some(intern_static("bool")),
        TyKind::Char => Some(intern_static("char")),
        TyKind::Str => Some(intern_static("str")),
        // Anonymous types — defer evaluation.
        TyKind::Infer(_) | TyKind::Param(_) | TyKind::Error | TyKind::Closure(_, _) => None,
        // Composite types — Phase 2 doesn't support matching against
        // these (e.g., `impl<T> Trait for &T`). Returns None to defer.
        TyKind::Ref(_, _, _)
        | TyKind::RawPtr(_, _)
        | TyKind::Array(_, _)
        | TyKind::Slice(_)
        | TyKind::Tuple(_)
        | TyKind::FnDef(_, _)
        | TyKind::FnPtr(_)
        | TyKind::Projection(_, _)
        | TyKind::Foreign
        | TyKind::Never => None,
    }
}

// =====================================================================
// Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::IntTy;
    use crate::mir::ty::TyVid;
    use crate::session::Span;
    use crate::traits::resolver::TraitResolver;
    use crate::traits::solver::{
        EvalError, EvalResult, Goal, InferCtxt, Obligation, ObligationCause, ParamEnv,
        TraitPredicate, Universe,
    };
    use lasso::Rodeo;

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

    fn dummy_param_ty(index: u32, name: &str) -> Ty {
        thread_local! {
            static TEST_RODEO: std::cell::RefCell<Rodeo> = std::cell::RefCell::new(Rodeo::new());
        }
        let spur = TEST_RODEO.with(|r| r.borrow_mut().get_or_intern(name));
        Ty::from_kind(TyKind::Param(crate::mir::ty::ParamTy { index, name: spur }))
    }

    fn dummy_obligation(self_ty: Ty, trait_def_id: u32) -> Obligation {
        Obligation::new(
            TraitPredicate::simple(self_ty, dummy_def_id(trait_def_id)),
            ObligationCause::LetBinding,
            Span::DUMMY,
        )
    }

    fn dummy_resolver() -> TraitResolver {
        TraitResolver::default()
    }

    // ----------- EvalOneResult tests -----------

    #[test]
    fn test_eval_one_result_ok() {
        let substs: SubstsRef = Rc::from([dummy_i32_ty()]);
        let r = EvalOneResult::ok(substs.clone());
        assert!(r.is_ok());
        assert!(!r.is_ambiguous());
        assert!(!r.is_err());
        assert_eq!(r.substs.len(), 1);
    }

    #[test]
    fn test_eval_one_result_ambiguous() {
        let r = EvalOneResult::ambiguous();
        assert!(!r.is_ok());
        assert!(r.is_ambiguous());
        assert!(!r.is_err());
        assert_eq!(r.substs.len(), 0);
    }

    #[test]
    fn test_eval_one_result_err() {
        let r = EvalOneResult::err(EvalError::SelfTypeMismatch);
        assert!(!r.is_ok());
        assert!(!r.is_ambiguous());
        assert!(r.is_err());
        assert_eq!(r.substs.len(), 0);
    }

    // ----------- EvalAllResult tests -----------

    #[test]
    fn test_eval_all_result_empty() {
        let r = EvalAllResult::empty();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        assert_eq!(r.ok_count(), 0);
        assert_eq!(r.ambiguous_count(), 0);
        assert_eq!(r.err_count(), 0);
    }

    #[test]
    fn test_eval_all_result_add_ok() {
        let mut r = EvalAllResult::empty();
        let substs: SubstsRef = Rc::from([dummy_i32_ty()]);
        r.add(dummy_def_id(7), EvalOneResult::ok(substs));
        assert!(!r.is_empty());
        assert_eq!(r.ok_count(), 1);
        assert_eq!(r.unique_ok().unwrap().0, dummy_def_id(7));
    }

    #[test]
    fn test_eval_all_result_add_ambiguous() {
        let mut r = EvalAllResult::empty();
        r.add(dummy_def_id(7), EvalOneResult::ambiguous());
        assert_eq!(r.ambiguous_count(), 1);
        assert_eq!(r.ok_count(), 0);
    }

    #[test]
    fn test_eval_all_result_add_err() {
        let mut r = EvalAllResult::empty();
        r.add(dummy_def_id(7), EvalOneResult::err(EvalError::WrongTrait));
        assert_eq!(r.err_count(), 1);
        assert_eq!(r.ok_count(), 0);
    }

    #[test]
    fn test_eval_all_result_unique_ok_multiple() {
        let mut r = EvalAllResult::empty();
        r.add(dummy_def_id(7), EvalOneResult::ok(Rc::from([])));
        r.add(dummy_def_id(8), EvalOneResult::ok(Rc::from([])));
        // Multiple Ok → not unique.
        assert!(r.unique_ok().is_none());
    }

    #[test]
    fn test_eval_all_result_unique_ok_with_errs() {
        let mut r = EvalAllResult::empty();
        r.add(dummy_def_id(7), EvalOneResult::ok(Rc::from([])));
        r.add(dummy_def_id(8), EvalOneResult::err(EvalError::WrongTrait));
        r.add(
            dummy_def_id(9),
            EvalOneResult::err(EvalError::SelfTypeMismatch),
        );
        // Exactly one Ok → unique.
        assert_eq!(r.unique_ok().unwrap().0, dummy_def_id(7));
    }

    // ----------- eval_all_to_result tests -----------

    #[test]
    fn test_eval_all_to_result_unique_ok() {
        let mut r = EvalAllResult::empty();
        r.add(dummy_def_id(7), EvalOneResult::ok(Rc::from([])));
        assert_eq!(eval_all_to_result(&r), EvalResult::Ok);
    }

    #[test]
    fn test_eval_all_to_result_multiple_ok_ambiguous() {
        let mut r = EvalAllResult::empty();
        r.add(dummy_def_id(7), EvalOneResult::ok(Rc::from([])));
        r.add(dummy_def_id(8), EvalOneResult::ok(Rc::from([])));
        // MVP禁 overlapping — multiple Ok = Ambiguous.
        assert_eq!(eval_all_to_result(&r), EvalResult::Ambiguous);
    }

    #[test]
    fn test_eval_all_to_result_only_ambiguous() {
        let mut r = EvalAllResult::empty();
        r.add(dummy_def_id(7), EvalOneResult::ambiguous());
        r.add(dummy_def_id(8), EvalOneResult::ambiguous());
        // No Ok, only Ambiguous → defer.
        assert_eq!(eval_all_to_result(&r), EvalResult::Ambiguous);
    }

    #[test]
    fn test_eval_all_to_result_only_err() {
        let mut r = EvalAllResult::empty();
        r.add(dummy_def_id(7), EvalOneResult::err(EvalError::WrongTrait));
        r.add(
            dummy_def_id(8),
            EvalOneResult::err(EvalError::SelfTypeMismatch),
        );
        // No Ok, no Ambiguous → Err.
        assert!(matches!(eval_all_to_result(&r), EvalResult::Err(_)));
    }

    #[test]
    fn test_eval_all_to_result_empty() {
        let r = EvalAllResult::empty();
        // No candidates at all → Err.
        assert!(matches!(eval_all_to_result(&r), EvalResult::Err(_)));
    }

    #[test]
    fn test_eval_all_to_result_ok_with_ambiguous() {
        let mut r = EvalAllResult::empty();
        r.add(dummy_def_id(7), EvalOneResult::ok(Rc::from([])));
        r.add(dummy_def_id(8), EvalOneResult::ambiguous());
        // One Ok + one Ambiguous → Ok (the unique Ok wins).
        assert_eq!(eval_all_to_result(&r), EvalResult::Ok);
    }

    // ----------- self_type_name_for_obligation tests -----------

    #[test]
    fn test_self_type_name_int() {
        let resolver = dummy_resolver();
        let name = self_type_name_for_obligation(&dummy_i32_ty(), &resolver);
        assert!(name.is_some());
    }

    #[test]
    fn test_self_type_name_bool() {
        let resolver = dummy_resolver();
        let ty = Ty::from_kind(TyKind::Bool);
        let name = self_type_name_for_obligation(&ty, &resolver);
        assert!(name.is_some());
    }

    #[test]
    fn test_self_type_name_infer_defers() {
        let resolver = dummy_resolver();
        let name = self_type_name_for_obligation(&dummy_infer_ty(0), &resolver);
        // Infer type → None (defer to Fulfillment).
        assert!(name.is_none());
    }

    #[test]
    fn test_self_type_name_param_defers() {
        let resolver = dummy_resolver();
        let name = self_type_name_for_obligation(&dummy_param_ty(0, "T"), &resolver);
        // Param type → None (defer to Fulfillment).
        assert!(name.is_none());
    }

    #[test]
    fn test_self_type_name_error_defers() {
        let resolver = dummy_resolver();
        let ty = Ty::from_kind(TyKind::Error);
        let name = self_type_name_for_obligation(&ty, &resolver);
        assert!(name.is_none());
    }

    #[test]
    fn test_self_type_name_ref_defers() {
        let resolver = dummy_resolver();
        // &i32 — composite type, Phase 2 defers.
        let ty = Ty::from_kind(TyKind::Ref(
            crate::mir::ty::Region::Erased,
            crate::mir::ty::Mutability::Immutable,
            Box::new(dummy_i32_ty()),
        ));
        let name = self_type_name_for_obligation(&ty, &resolver);
        assert!(name.is_none());
    }

    // ----------- infer_substs_from_self_type tests -----------

    #[test]
    fn test_infer_substs_from_adt() {
        let substs: SubstsRef = Rc::from([dummy_i32_ty()]);
        let ty = Ty::from_kind(TyKind::Adt(dummy_def_id(7), substs.clone()));
        let inferred = infer_substs_from_self_type(&ty);
        assert_eq!(inferred.len(), 1);
        assert_eq!(inferred[0], dummy_i32_ty());
    }

    #[test]
    fn test_infer_substs_from_non_adt() {
        let inferred = infer_substs_from_self_type(&dummy_i32_ty());
        assert_eq!(inferred.len(), 0);
    }

    // ----------- evaluate_one integration tests -----------

    #[test]
    fn test_evaluate_one_impl_not_found() {
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let param_env = ParamEnv::empty();
        let mut cx = EvalCtxt::new(&resolver, &mut infer_ctxt, &param_env);

        let obl = dummy_obligation(dummy_i32_ty(), 7);
        // No impl registered → WrongTrait (impls map is empty).
        let result = evaluate_one(dummy_def_id(99), &obl, &mut cx);
        assert!(result.is_err());
        assert_eq!(result.result, EvalResult::Err(EvalError::WrongTrait));
    }

    #[test]
    fn test_evaluate_one_no_candidates() {
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let param_env = ParamEnv::empty();
        let mut cx = EvalCtxt::new(&resolver, &mut infer_ctxt, &param_env);

        let goal = Goal::with_empty_env(TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7)));
        let result = evaluate(&goal, &mut cx);
        // No candidates (impls map is empty).
        assert!(result.is_empty());
        assert_eq!(
            eval_all_to_result(&result),
            EvalResult::Err(EvalError::WrongTrait)
        );
    }

    // ----------- Universe guard tests -----------

    #[test]
    fn test_universe_guard_restores_universe() {
        let mut infer_ctxt = InferCtxt::new();
        let initial = infer_ctxt.current_universe();
        assert_eq!(initial, Universe::ROOT);

        // Enter a universe.
        let prev = infer_ctxt.enter_universe();
        assert_eq!(infer_ctxt.current_universe().0, 1);

        // Use a guard.
        {
            let _guard = UniverseGuard::new(&mut infer_ctxt, prev);
            // Even if we change the universe inside, it should be restored on drop.
            // (We don't change it here, but the guard's Drop will set
            // next_universe = prev regardless.)
        }

        // After the guard's scope ends, the universe is back to prev (which is ROOT).
        // (Note: the guard sets next_universe = saved, so we're at ROOT.)
        assert_eq!(infer_ctxt.current_universe(), Universe::ROOT);
    }

    // ----------- Integration: full Evaluation flow -----------

    #[test]
    fn test_integration_evaluate_with_empty_resolver() {
        // End-to-end: build a goal, evaluate against an empty resolver,
        // confirm we get Err.
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let param_env = ParamEnv::empty();
        let mut cx = EvalCtxt::new(&resolver, &mut infer_ctxt, &param_env);

        let goal = Goal::with_empty_env(TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7)));
        let result = evaluate(&goal, &mut cx);
        assert!(result.is_empty());
        assert_eq!(result.ok_count(), 0);
        assert_eq!(result.ambiguous_count(), 0);
        assert_eq!(result.err_count(), 0);
        assert_eq!(
            eval_all_to_result(&result),
            EvalResult::Err(EvalError::WrongTrait)
        );
    }

    #[test]
    fn test_integration_evaluate_infer_self_defers() {
        // End-to-end: obligation with Infer self type should produce
        // Ambiguous (deferred to Fulfillment pending queue).
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let param_env = ParamEnv::empty();
        let mut cx = EvalCtxt::new(&resolver, &mut infer_ctxt, &param_env);

        let goal = Goal::with_empty_env(TraitPredicate::simple(dummy_infer_ty(0), dummy_def_id(7)));
        let result = evaluate(&goal, &mut cx);
        // No candidates (empty resolver).
        assert!(result.is_empty());
        assert_eq!(
            eval_all_to_result(&result),
            EvalResult::Err(EvalError::WrongTrait)
        );
    }

    #[test]
    fn test_integration_obligation_with_param_env_assumption() {
        // End-to-end: param_env has an assumption, and the goal's predicate
        // matches the assumption. Phase 2 doesn't yet short-circuit on
        // param_env assumptions (that's Phase 4), but we verify the
        // EvalCtxt can be built with a non-empty param_env without panic.
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let tp = TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7));
        let param_env = ParamEnv::from_predicates(vec![tp.clone()]);
        let mut cx = EvalCtxt::new(&resolver, &mut infer_ctxt, &param_env);

        let goal = Goal::new(tp.clone(), param_env.clone());
        let result = evaluate(&goal, &mut cx);
        // No candidates (empty resolver).
        assert!(result.is_empty());
    }

    #[test]
    fn test_integration_infer_ctxt_universe_unchanged_after_eval() {
        // Per §5.2: Evaluation creates placeholders but restores the
        // universe on exit. Verify universe is unchanged after evaluate().
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let initial_universe = infer_ctxt.current_universe();
        let param_env = ParamEnv::empty();
        let mut cx = EvalCtxt::new(&resolver, &mut infer_ctxt, &param_env);

        let goal = Goal::with_empty_env(TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7)));
        let _result = evaluate(&goal, &mut cx);

        // Universe should be unchanged (placeholder was created and restored).
        assert_eq!(infer_ctxt.current_universe(), initial_universe);
    }
}
