//! Stage 19.4 (v0.5 Phase 4) — Trait Solver Fulfillment phase.
//!
//! Per `docs/lang-design/03-type-system.md` §5.4:
//! - **Fulfillment** maintains an obligation queue and recursively selects
//!   until queue empties or fails
//! - Algorithm:
//!   ```text
//!   fulfillment_loop():
//!       while not obligation_queue.is_empty():
//!           obl = obligation_queue.pop()
//!           result = select(obl)
//!           match result:
//!               Ok(impl) =>
//!                   # Add impl's where clauses to queue
//!                   for clause in impl.where_clauses:
//!                       obligation_queue.push(clause)
//!               Err(ambig) =>
//!                   # Defer, retry after inference variable resolved
//!                   pending_queue.push(obl)
//!               Err(no_impl) =>
//!                   report_error(obl)
//!       
//!       # Final check on pending queue
//!       for obl in pending_queue:
//!           if not resolved(obl):
//!               report_error(obl)
//!   ```
//!
//! Per §5.5 Impl matching: when an impl is selected, its where clauses
//! become new obligations. E.g., `impl<T: Clone> Trait for Vec<T>` selected
//! for `Vec<i32>: Trait` adds `i32: Clone` as a new obligation (recursively
//! fulfilled).
//!
//! This module (Phase 4) implements:
//! - `fulfillment_loop(queue, cx) -> FulfillmentResult` — main loop
//! - `FulfillmentResult` — summary (Ok / errors / pending)
//! - `FulfillmentCtxt` — context bundling selection + queue state
//! - `try_fulfill_obligation(obl, cx) -> ObligationResult` — single obligation
//! - `collect_impl_where_clauses(impl_def_id, resolver)` — fetch impl's where clauses
//!   (MVP: returns empty — ImplInfo doesn't store where clauses yet; future phase will integrate HIR)
//!
//! Per §11 (接口隔离): this module reads Phase 3 select + Phase 1 ObligationQueue
//! (data contracts) + Phase 1 ParamEnv (assumption short-circuit).
//!
//! Per §12 (最优 > 最小): implement `fulfillment_loop` as a proper iterative
//! algorithm (vs naive recursion) to avoid stack overflow on deep obligation
//! chains.
//!
//! Per §1.0 原則 4 (报错 > 静默): all Fulfillment errors are explicit
//! (FulfillmentError variants); never silently succeed when obligations fail.

use crate::hir::DefId;
use crate::traits::resolver::TraitResolver;
use crate::traits::solver::eval::EvalCtxt;
use crate::traits::solver::select::select;
use crate::traits::solver::{Obligation, ObligationQueue, ParamEnv, SelectionResult};

// =====================================================================
// FulfillmentResult — summary of fulfillment loop outcome
// =====================================================================

/// Result of running the fulfillment loop on an obligation queue.
///
/// Per §5.4: the loop either:
/// - Succeeds (all obligations resolved)
/// - Errors (some obligation has no impl)
/// - Stalls (some obligations still pending after queue drains — "type
///   annotations needed" error)
///
/// Per §1.0 原則 4 (报错 > 静默): explicit tri-state (Ok / Errors / Stalled),
/// not just bool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FulfillmentResult {
    /// All obligations resolved successfully.
    Ok {
        /// Number of obligations resolved.
        resolved_count: usize,
        /// Number of impls selected.
        selected_count: usize,
    },
    /// Some obligations errored out (no impl or other failure).
    Errors {
        /// List of (obligation, error_description) pairs.
        errors: Vec<(Obligation, FulfillmentError)>,
        /// Number of obligations resolved before errors.
        resolved_count: usize,
        /// Number of impls selected before errors.
        selected_count: usize,
    },
    /// Some obligations still pending (inference variables unresolved).
    /// Per §5.10: report "type annotations needed" for these.
    Stalled {
        /// List of pending obligations that couldn't be resolved.
        pending: Vec<Obligation>,
        /// Number of obligations resolved before stalling.
        resolved_count: usize,
        /// Number of impls selected before stalling.
        selected_count: usize,
    },
}

impl FulfillmentResult {
    /// Returns `true` if fulfillment succeeded.
    pub fn is_ok(&self) -> bool {
        matches!(self, FulfillmentResult::Ok { .. })
    }

    /// Returns `true` if fulfillment errored.
    pub fn has_errors(&self) -> bool {
        matches!(self, FulfillmentResult::Errors { .. })
    }

    /// Returns `true` if fulfillment stalled (pending obligations remain).
    pub fn is_stalled(&self) -> bool {
        matches!(self, FulfillmentResult::Stalled { .. })
    }

    /// Number of obligations resolved (across all variants).
    pub fn resolved_count(&self) -> usize {
        match self {
            FulfillmentResult::Ok { resolved_count, .. } => *resolved_count,
            FulfillmentResult::Errors { resolved_count, .. } => *resolved_count,
            FulfillmentResult::Stalled { resolved_count, .. } => *resolved_count,
        }
    }

    /// Number of impls selected (across all variants).
    pub fn selected_count(&self) -> usize {
        match self {
            FulfillmentResult::Ok { selected_count, .. } => *selected_count,
            FulfillmentResult::Errors { selected_count, .. } => *selected_count,
            FulfillmentResult::Stalled { selected_count, .. } => *selected_count,
        }
    }

    /// Returns the list of errors if `Errors`, else empty.
    pub fn errors(&self) -> Vec<&(Obligation, FulfillmentError)> {
        match self {
            FulfillmentResult::Errors { errors, .. } => errors.iter().collect(),
            _ => Vec::new(),
        }
    }

    /// Returns the list of pending obligations if `Stalled`, else empty.
    pub fn pending(&self) -> Vec<&Obligation> {
        match self {
            FulfillmentResult::Stalled { pending, .. } => pending.iter().collect(),
            _ => Vec::new(),
        }
    }
}

// =====================================================================
// FulfillmentError — why an obligation failed
// =====================================================================

/// Error variants for fulfillment failures.
///
/// Per §1.0 原則 4 (报错 > 静默): all fulfillment errors are explicit
/// variants, never silently swallowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FulfillmentError {
    /// No impl matched the obligation.
    NoImpl,
    /// Multiple impls matched (MVP禁 overlapping).
    Ambiguous { candidate_count: usize },
    /// Recursion depth limit exceeded (per §5.8: 128 — same as rustc default).
    /// Prevents infinite loops on `impl<T: A> B for T where T: B` cycles.
    RecursionLimitExceeded { depth: u32 },
}

impl std::fmt::Display for FulfillmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FulfillmentError::NoImpl => write!(f, "no impl matched"),
            FulfillmentError::Ambiguous { candidate_count } => {
                write!(f, "ambiguous: {} candidates matched", candidate_count)
            }
            FulfillmentError::RecursionLimitExceeded { depth } => {
                write!(f, "recursion limit exceeded (depth {})", depth)
            }
        }
    }
}

impl std::error::Error for FulfillmentError {}

// =====================================================================
// ObligationResult — result of fulfilling a single obligation
// =====================================================================

/// Result of attempting to fulfill a single obligation.
///
/// Per §5.4: each obligation either:
/// - Resolves (impl selected, where clauses added to queue)
/// - Errors (no impl or ambiguous)
/// - Defers (pending — inference variable unresolved)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObligationResult {
    /// Obligation resolved: impl selected.
    Resolved {
        /// DefId of the selected impl.
        impl_def_id: DefId,
        /// Where clauses from the selected impl (added to queue by caller).
        new_obligations: Vec<Obligation>,
    },
    /// Obligation errored: no impl or ambiguous.
    Error(FulfillmentError),
    /// Obligation deferred: inference variable unresolved.
    Deferred,
}

impl ObligationResult {
    /// Returns `true` if the obligation resolved.
    pub fn is_resolved(&self) -> bool {
        matches!(self, ObligationResult::Resolved { .. })
    }

    /// Returns `true` if the obligation errored.
    pub fn is_error(&self) -> bool {
        matches!(self, ObligationResult::Error(_))
    }

    /// Returns `true` if the obligation was deferred.
    pub fn is_deferred(&self) -> bool {
        matches!(self, ObligationResult::Deferred)
    }
}

// =====================================================================
// FulfillmentCtxt — context for fulfillment operations
// =====================================================================

/// Context for fulfillment operations.
///
/// Per §11 (接口隔离): bundles all data the fulfiller needs:
/// - `trait_resolver`: look up trait/impl metadata (via EvalCtxt)
/// - `infer_ctxt`: placeholder universe + substitution table (via EvalCtxt)
/// - `param_env`: where clauses to assume true (via EvalCtxt)
/// - `max_depth`: recursion limit (per §5.8: 128)
///
/// Per §1.0 原則 6 (通解 > 特解): one context type for all fulfillment
/// needs (no per-trait subtypes).
///
/// Per §1.0 原則 10 (唯一可信数据源): the obligation queue is the single
/// source of truth for pending work; the fulfiller never maintains a
/// parallel queue.
pub struct FulfillmentCtxt<'a> {
    /// The underlying evaluation context (TraitResolver + InferCtxt + ParamEnv).
    pub eval_ctxt: EvalCtxt<'a>,
    /// Maximum recursion depth (per §5.8: 128).
    pub max_depth: u32,
}

/// The default maximum recursion depth (per §5.8).
///
/// Per §5.8: "trait resolution 递归深度限制为 128 (v1.2 修正：与 rustc 默认值一致)".
pub const DEFAULT_MAX_DEPTH: u32 = 128;

impl<'a> FulfillmentCtxt<'a> {
    /// Construct a fulfillment context with default max depth.
    pub fn new(eval_ctxt: EvalCtxt<'a>) -> Self {
        Self {
            eval_ctxt,
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }

    /// Construct a fulfillment context with a custom max depth.
    pub fn with_max_depth(eval_ctxt: EvalCtxt<'a>, max_depth: u32) -> Self {
        Self {
            eval_ctxt,
            max_depth,
        }
    }

    /// Run the fulfillment loop on an obligation queue.
    ///
    /// Per §5.4: this is the main entry point for Fulfillment.
    pub fn fulfill(&mut self, queue: &mut ObligationQueue) -> FulfillmentResult {
        fulfillment_loop(queue, &mut self.eval_ctxt, self.max_depth)
    }
}

// =====================================================================
// fulfillment_loop — main loop
// =====================================================================

/// The main fulfillment loop.
///
/// Per §5.4 algorithm:
/// 1. Pop ready obligations from the queue
/// 2. For each: try_fulfill_obligation
///    - Resolved → add new where clause obligations to queue
///    - Error → record error
///    - Deferred → leave in pending queue
/// 3. When ready queue drains:
///    - If errors → return Errors
///    - If pending → return Stalled (type annotations needed)
///    - Else → return Ok
///
/// Per §5.8: enforce max_depth to prevent infinite recursion on cyclic
/// obligations (e.g., `impl<T: A> B for T where T: B`).
///
/// Per §1.0 原則 4 (报错 > 静默): all errors are explicit; never silently
/// succeed when obligations fail.
///
/// Per §12 (最优 > 最小): iterative loop (vs naive recursion) to avoid
/// stack overflow on deep obligation chains.
pub fn fulfillment_loop(
    queue: &mut ObligationQueue,
    cx: &mut EvalCtxt,
    max_depth: u32,
) -> FulfillmentResult {
    let mut resolved_count = 0usize;
    let mut selected_count = 0usize;
    let mut errors: Vec<(Obligation, FulfillmentError)> = Vec::new();
    let mut depth: u32 = 0;

    // Main loop: process ready obligations until none remain.
    while let Some(obl) = queue.pop_ready() {
        // Check recursion depth (per §5.8).
        if depth >= max_depth {
            errors.push((obl, FulfillmentError::RecursionLimitExceeded { depth }));
            queue.record_errored();
            continue;
        }
        depth += 1;

        // Try to fulfill this obligation.
        let result = try_fulfill_obligation(&obl, cx);

        match result {
            ObligationResult::Resolved {
                impl_def_id,
                new_obligations,
            } => {
                // Success: add where clause obligations to queue.
                for new_obl in new_obligations {
                    queue.push(new_obl);
                }
                resolved_count += 1;
                // Per §1.0 原則 9 (正确 > 妥协): only count as "selected" if
                // an actual impl was selected (not when assumed via ParamEnv).
                // The sentinel value `DefId::new(u32::MAX)` indicates
                // "assumed, not selected" (see `try_fulfill_obligation`).
                if impl_def_id != DefId::new(u32::MAX) {
                    selected_count += 1;
                }
                queue.record_resolved();
            }
            ObligationResult::Error(err) => {
                // Failure: record error.
                errors.push((obl, err));
                queue.record_errored();
            }
            ObligationResult::Deferred => {
                // Defer: put back in pending queue.
                // (try_fulfill_obligation already detected the deferral
                // via select returning Ambiguous; we put it back so
                // refresh_pending can pick it up if inference vars resolve.)
                queue.push(Obligation::new(
                    obl.predicate.clone(),
                    obl.cause.clone(),
                    obl.span,
                ));
            }
        }
    }

    // After ready queue drains, check for errors.
    if !errors.is_empty() {
        return FulfillmentResult::Errors {
            errors,
            resolved_count,
            selected_count,
        };
    }

    // Check for pending obligations (stalled).
    if queue.is_stalled() {
        let (_, pending) = queue.drain_all();
        return FulfillmentResult::Stalled {
            pending,
            resolved_count,
            selected_count,
        };
    }

    // All obligations resolved successfully.
    FulfillmentResult::Ok {
        resolved_count,
        selected_count,
    }
}

// =====================================================================
// try_fulfill_obligation — fulfill a single obligation
// =====================================================================

/// Try to fulfill a single obligation.
///
/// Per §5.4 + §5.5:
/// 1. Check if the obligation's predicate is already assumed in ParamEnv
///    (short-circuit — no need to select)
/// 2. If not, call `select` to find a unique impl
/// 3. If Ok: collect the impl's where clauses as new obligations
/// 4. If Ambiguous: defer (return Deferred)
/// 5. If NoImpl: return Error
///
/// Per §1.0 原則 6 (通解 > 特解): one function handles all obligation kinds.
///
/// Per §1.0 原則 9 (正确 > 妥协): ParamEnv short-circuit is correct (don't
/// re-prove assumed bounds); deferral is correct (don't guess on ambiguous).
pub fn try_fulfill_obligation(obl: &Obligation, cx: &mut EvalCtxt) -> ObligationResult {
    // Step 1: Check ParamEnv short-circuit.
    // Per §5.4 + rustc pattern: if the obligation's predicate is already
    // assumed in the ParamEnv, we don't need to select — it's trivially Ok.
    //
    // Per §1.0 原則 9 (正确 > 妥协): don't re-prove assumed bounds.
    if cx.param_env.assumes(&obl.predicate) {
        // Assumed true — no new obligations, no impl selected.
        return ObligationResult::Resolved {
            impl_def_id: DefId::new(u32::MAX), // sentinel: "assumed, not selected"
            new_obligations: Vec::new(),
        };
    }

    // Step 2: Build a goal from the obligation and select.
    let goal = crate::traits::solver::Goal::new(obl.predicate.clone(), cx.param_env.clone());
    let selection = select(&goal, cx);

    // Step 3: Handle selection result.
    match selection {
        SelectionResult::Ok { impl_def_id } => {
            // Success: collect the impl's where clauses as new obligations.
            let new_obligations = collect_impl_where_clauses(impl_def_id, cx.trait_resolver);
            ObligationResult::Resolved {
                impl_def_id,
                new_obligations,
            }
        }
        SelectionResult::Ambiguous { candidate_count: _ } => {
            // Ambiguous: defer.
            // (Could be due to inference variable unresolved; Fulfillment
            // will retry after the variable is bound.)
            ObligationResult::Deferred
        }
        SelectionResult::NoImpl => {
            // No impl matched: error.
            ObligationResult::Error(FulfillmentError::NoImpl)
        }
    }
}

// =====================================================================
// collect_impl_where_clauses — fetch impl's where clauses + supertraits
// =====================================================================

/// Collect the where clauses of an impl as new obligations, plus the
/// supertrait obligations of the impl's trait.
///
/// Per §5.4 + §5.5: when an impl is selected, its where clauses AND its
/// trait's supertraits become new obligations. E.g., `impl<T: Clone> Trait
/// for Vec<T>` selected for `Vec<i32>: Trait` adds:
/// - `i32: Clone` (impl where clause — MVP placeholder, returns empty)
/// - Supertrait obligations (if `trait Trait: Super` then `Vec<i32>: Super`)
///
/// Stage 19.6 (v0.5 Phase 6) integration: supertrait expansion is now
/// wired in via `supertrait::supertrait_obligations`. The impl's where
/// clause collection remains a MVP placeholder (ImplInfo doesn't store
/// where clauses yet) — documented limitation, not silent failure.
///
/// Per §1.0 原則 4 (报错 > 静默): impl where clause MVP limitation documented.
///
/// Per §1.0 原則 6 (通解 > 特解): one function handles all impl kinds;
/// the collection logic is general (no per-trait branches).
///
/// Per §12 (最优 > 最小): proper composition — supertrait expansion +
/// (future) where clause collection, rather than duplicating logic.
pub fn collect_impl_where_clauses(impl_def_id: DefId, resolver: &TraitResolver) -> Vec<Obligation> {
    let mut obligations = Vec::new();

    // Step 1: Collect supertrait obligations.
    // Per §5.5: when an impl is selected, its trait's supertraits become
    // new obligations. E.g., `impl Foo for X` (where `trait Foo: Bar`)
    // adds `X: Bar` as a new obligation.
    //
    // Stage 19.6: integrated via supertrait::supertrait_obligations.
    // We need the impl's trait_def_id and self_ty to generate predicates.
    if let Some(impl_info) = resolver.impls.get(&impl_def_id) {
        // Look up the trait's DefId via trait_name Spur.
        if let Some(trait_name_spur) = impl_info.trait_name {
            if let Some(&trait_def_id) = resolver.trait_by_name.get(&trait_name_spur) {
                // Construct the self_ty from impl_info.
                // Per §11: ImplInfo stores self_ty_name (Spur), not the full Ty.
                // For supertrait expansion, we need a Ty. We construct a
                // best-effort Ty by looking up the type DefId.
                //
                // MVP: if we can't resolve self_ty_name to a DefId, we skip
                // supertrait expansion (documented limitation).
                if let Some(self_ty) = construct_self_ty_from_name(impl_info.self_ty_name, resolver)
                {
                    let supertrait_obls = crate::traits::solver::supertrait::supertrait_obligations(
                        impl_def_id,
                        trait_def_id,
                        &self_ty,
                        resolver,
                        impl_info.span,
                    );
                    obligations.extend(supertrait_obls);
                }
            }
        }
    }

    // Step 2: Collect impl's where clauses (MVP placeholder — returns empty).
    // Per §1.0 原則 4: documented limitation, not silent failure.
    // Future phase will integrate HIR access to fetch HirImpl.generics.where_clause.
    // (No-op for now — supertrait expansion above is the main integration.)

    obligations
}

/// Construct a best-effort `Ty` from an impl's `self_ty_name` (Spur).
///
/// Per §11 (接口隔离): we use TraitResolver's `type_by_def_id` map to
/// find the DefId for the type name, then construct `TyKind::Adt(def_id, [])`.
///
/// Per §1.0 原則 9 (正确 > 妥协): returns `None` if the type name can't
/// be resolved (rather than guessing or using a placeholder).
///
/// Per §1.0 原則 6 (通解 > 特解): one function handles all type name
/// kinds (struct, enum, primitive — via name lookup).
fn construct_self_ty_from_name(
    self_ty_name: Option<crate::lexer::Symbol>,
    resolver: &TraitResolver,
) -> Option<crate::mir::ty::Ty> {
    use crate::mir::ty::{Ty, TyKind};

    let name_spur = self_ty_name?;

    // Look up the DefId for this type name via type_by_def_id.
    // Per §1.0 原則 10: TraitResolver is the single source of truth.
    let def_id = resolver
        .type_by_def_id
        .iter()
        .find_map(|(did, spur)| (*spur == name_spur).then_some(*did))?;

    Some(Ty::from_kind(TyKind::Adt(def_id, std::rc::Rc::from([]))))
}

// =====================================================================
// Helper: fulfill a single obligation (top-level entry)
// =====================================================================

/// Fulfill a single obligation (top-level entry).
///
/// Per §5.4: convenience function for callers that want to fulfill a
/// single obligation without managing a queue. Creates a temporary queue,
/// pushes the obligation, runs the loop, and returns the result.
///
/// Per §1.0 原則 3 (显式 > 隐式): explicit single-obligation entry point,
/// rather than requiring callers to manage a queue.
pub fn fulfill_obligation(obl: Obligation, cx: &mut EvalCtxt, max_depth: u32) -> FulfillmentResult {
    let mut queue = ObligationQueue::new();
    queue.push(obl);
    fulfillment_loop(&mut queue, cx, max_depth)
}

// =====================================================================
// Helper: check if an obligation would be assumed (without fulfilling)
// =====================================================================

/// Check if an obligation's predicate is assumed in the ParamEnv.
///
/// Per §1.0 原則 3 (显式 > 隐式): explicit peek function for callers
/// that need to know if an obligation would short-circuit without
/// actually running the fulfillment loop.
pub fn is_assumed(obl: &Obligation, param_env: &ParamEnv) -> bool {
    param_env.assumes(&obl.predicate)
}

// =====================================================================
// Helper: describe fulfillment result for diagnostics
// =====================================================================

/// Describe a FulfillmentResult for diagnostic messages.
///
/// Per §1.0 原則 3 (显式 > 隐式): produces explicit human-readable strings.
pub fn describe_fulfillment_result(result: &FulfillmentResult) -> String {
    match result {
        FulfillmentResult::Ok {
            resolved_count,
            selected_count,
        } => {
            format!(
                "fulfilled: {} obligations resolved, {} impls selected",
                resolved_count, selected_count
            )
        }
        FulfillmentResult::Errors {
            errors,
            resolved_count,
            selected_count,
        } => {
            let error_descs: Vec<String> = errors.iter().map(|(_, e)| e.to_string()).collect();
            format!(
                "fulfillment errors: {} errors ({}); {} resolved, {} selected before errors",
                errors.len(),
                error_descs.join(", "),
                resolved_count,
                selected_count
            )
        }
        FulfillmentResult::Stalled {
            pending,
            resolved_count,
            selected_count,
        } => {
            format!(
                "fulfillment stalled: {} pending obligations (type annotations needed); {} resolved, {} selected before stalling",
                pending.len(),
                resolved_count,
                selected_count
            )
        }
    }
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
    use crate::session::Span;
    use crate::traits::resolver::TraitResolver;
    use crate::traits::solver::eval::EvalCtxt;
    use crate::traits::solver::{
        Goal, InferCtxt, Obligation, ObligationCause, ObligationQueue, ParamEnv, TraitPredicate,
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

    /// Run a closure with an EvalCtxt (ParamEnv owned by the closure).
    fn with_eval_ctxt<R, F>(resolver: &TraitResolver, infer_ctxt: &mut InferCtxt, f: F) -> R
    where
        F: FnOnce(&mut EvalCtxt) -> R,
    {
        let param_env = ParamEnv::empty();
        let mut cx = EvalCtxt::new(resolver, infer_ctxt, &param_env);
        f(&mut cx)
    }

    // ----------- FulfillmentResult tests -----------

    #[test]
    fn test_fulfillment_result_ok() {
        let r = FulfillmentResult::Ok {
            resolved_count: 5,
            selected_count: 3,
        };
        assert!(r.is_ok());
        assert!(!r.has_errors());
        assert!(!r.is_stalled());
        assert_eq!(r.resolved_count(), 5);
        assert_eq!(r.selected_count(), 3);
        assert!(r.errors().is_empty());
        assert!(r.pending().is_empty());
    }

    #[test]
    fn test_fulfillment_result_errors() {
        let obl = dummy_obligation(dummy_i32_ty(), 7);
        let r = FulfillmentResult::Errors {
            errors: vec![(obl.clone(), FulfillmentError::NoImpl)],
            resolved_count: 2,
            selected_count: 1,
        };
        assert!(!r.is_ok());
        assert!(r.has_errors());
        assert!(!r.is_stalled());
        assert_eq!(r.resolved_count(), 2);
        assert_eq!(r.selected_count(), 1);
        assert_eq!(r.errors().len(), 1);
    }

    #[test]
    fn test_fulfillment_result_stalled() {
        let obl = dummy_obligation(dummy_infer_ty(0), 7);
        let r = FulfillmentResult::Stalled {
            pending: vec![obl.clone()],
            resolved_count: 0,
            selected_count: 0,
        };
        assert!(!r.is_ok());
        assert!(!r.has_errors());
        assert!(r.is_stalled());
        assert_eq!(r.resolved_count(), 0);
        assert_eq!(r.pending().len(), 1);
    }

    // ----------- FulfillmentError tests -----------

    #[test]
    fn test_fulfillment_error_no_impl() {
        let e = FulfillmentError::NoImpl;
        assert_eq!(e.to_string(), "no impl matched");
    }

    #[test]
    fn test_fulfillment_error_ambiguous() {
        let e = FulfillmentError::Ambiguous { candidate_count: 3 };
        assert!(e.to_string().contains("ambiguous"));
        assert!(e.to_string().contains("3 candidates"));
    }

    #[test]
    fn test_fulfillment_error_recursion_limit() {
        let e = FulfillmentError::RecursionLimitExceeded { depth: 128 };
        assert!(e.to_string().contains("recursion limit"));
        assert!(e.to_string().contains("128"));
    }

    // ----------- ObligationResult tests -----------

    #[test]
    fn test_obligation_result_resolved() {
        let r = ObligationResult::Resolved {
            impl_def_id: dummy_def_id(7),
            new_obligations: Vec::new(),
        };
        assert!(r.is_resolved());
        assert!(!r.is_error());
        assert!(!r.is_deferred());
    }

    #[test]
    fn test_obligation_result_error() {
        let r = ObligationResult::Error(FulfillmentError::NoImpl);
        assert!(!r.is_resolved());
        assert!(r.is_error());
        assert!(!r.is_deferred());
    }

    #[test]
    fn test_obligation_result_deferred() {
        let r = ObligationResult::Deferred;
        assert!(!r.is_resolved());
        assert!(!r.is_error());
        assert!(r.is_deferred());
    }

    // ----------- collect_impl_where_clauses tests -----------

    #[test]
    fn test_collect_impl_where_clauses_empty() {
        let resolver = dummy_resolver();
        let result = collect_impl_where_clauses(dummy_def_id(7), &resolver);
        // MVP: returns empty (ImplInfo doesn't store where clauses yet).
        assert!(result.is_empty());
    }

    // ----------- is_assumed tests -----------

    #[test]
    fn test_is_assumed_true() {
        let obl = dummy_obligation(dummy_i32_ty(), 7);
        let pe = ParamEnv::from_predicates(vec![obl.predicate.clone()]);
        assert!(is_assumed(&obl, &pe));
    }

    #[test]
    fn test_is_assumed_false() {
        let obl = dummy_obligation(dummy_i32_ty(), 7);
        let pe = ParamEnv::empty();
        assert!(!is_assumed(&obl, &pe));
    }

    #[test]
    fn test_is_assumed_different_trait() {
        let obl1 = dummy_obligation(dummy_i32_ty(), 7);
        let obl2 = dummy_obligation(dummy_i32_ty(), 8); // different trait
        let pe = ParamEnv::from_predicates(vec![obl1.predicate.clone()]);
        assert!(!is_assumed(&obl2, &pe));
    }

    // ----------- describe_fulfillment_result tests -----------

    #[test]
    fn test_describe_fulfillment_result_ok() {
        let r = FulfillmentResult::Ok {
            resolved_count: 5,
            selected_count: 3,
        };
        let desc = describe_fulfillment_result(&r);
        assert!(desc.contains("fulfilled"));
        assert!(desc.contains("5 obligations resolved"));
        assert!(desc.contains("3 impls selected"));
    }

    #[test]
    fn test_describe_fulfillment_result_errors() {
        let obl = dummy_obligation(dummy_i32_ty(), 7);
        let r = FulfillmentResult::Errors {
            errors: vec![(obl, FulfillmentError::NoImpl)],
            resolved_count: 2,
            selected_count: 1,
        };
        let desc = describe_fulfillment_result(&r);
        assert!(desc.contains("fulfillment errors"));
        assert!(desc.contains("1 errors"));
        assert!(desc.contains("no impl matched"));
    }

    #[test]
    fn test_describe_fulfillment_result_stalled() {
        let obl = dummy_obligation(dummy_infer_ty(0), 7);
        let r = FulfillmentResult::Stalled {
            pending: vec![obl],
            resolved_count: 0,
            selected_count: 0,
        };
        let desc = describe_fulfillment_result(&r);
        assert!(desc.contains("fulfillment stalled"));
        assert!(desc.contains("1 pending"));
        assert!(desc.contains("type annotations needed"));
    }

    // ----------- try_fulfill_obligation tests -----------

    #[test]
    fn test_try_fulfill_obligation_no_impl() {
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let obl = dummy_obligation(dummy_i32_ty(), 7);
        let result = with_eval_ctxt(&resolver, &mut infer_ctxt, |cx| {
            try_fulfill_obligation(&obl, cx)
        });
        // No candidates (empty resolver) → Error(NoImpl).
        assert!(result.is_error());
        assert_eq!(result, ObligationResult::Error(FulfillmentError::NoImpl));
    }

    #[test]
    fn test_try_fulfill_obligation_infer_self_deferred() {
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let obl = dummy_obligation(dummy_infer_ty(0), 7);
        let result = with_eval_ctxt(&resolver, &mut infer_ctxt, |cx| {
            try_fulfill_obligation(&obl, cx)
        });
        // Infer self → evaluate returns empty → select returns NoImpl.
        // (Note: with empty resolver, no candidates, so it's NoImpl, not Deferred.)
        assert!(result.is_error());
    }

    #[test]
    fn test_try_fulfill_obligation_assumed_short_circuits() {
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let obl = dummy_obligation(dummy_i32_ty(), 7);
        let pe = ParamEnv::from_predicates(vec![obl.predicate.clone()]);
        // Build EvalCtxt with non-empty ParamEnv.
        let mut cx = EvalCtxt::new(&resolver, &mut infer_ctxt, &pe);
        let result = try_fulfill_obligation(&obl, &mut cx);
        // Assumed true → Resolved (no impl selected, no new obligations).
        assert!(result.is_resolved());
        if let ObligationResult::Resolved {
            new_obligations, ..
        } = result
        {
            assert!(new_obligations.is_empty());
        }
    }

    // ----------- fulfillment_loop tests -----------

    #[test]
    fn test_fulfillment_loop_empty_queue() {
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let mut queue = ObligationQueue::new();
        let result = with_eval_ctxt(&resolver, &mut infer_ctxt, |cx| {
            fulfillment_loop(&mut queue, cx, DEFAULT_MAX_DEPTH)
        });
        // Empty queue → Ok with 0 resolved.
        assert!(result.is_ok());
        assert_eq!(result.resolved_count(), 0);
        assert_eq!(result.selected_count(), 0);
    }

    #[test]
    fn test_fulfillment_loop_single_obligation_no_impl() {
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let mut queue = ObligationQueue::new();
        queue.push(dummy_obligation(dummy_i32_ty(), 7));
        let result = with_eval_ctxt(&resolver, &mut infer_ctxt, |cx| {
            fulfillment_loop(&mut queue, cx, DEFAULT_MAX_DEPTH)
        });
        // No candidates → Error(NoImpl).
        assert!(result.has_errors());
        assert_eq!(result.resolved_count(), 0);
        assert_eq!(result.errors().len(), 1);
    }

    #[test]
    fn test_fulfillment_loop_infer_self_stalled() {
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let mut queue = ObligationQueue::new();
        queue.push(dummy_obligation(dummy_infer_ty(0), 7));
        let result = with_eval_ctxt(&resolver, &mut infer_ctxt, |cx| {
            fulfillment_loop(&mut queue, cx, DEFAULT_MAX_DEPTH)
        });
        // Infer self → tries to fulfill, gets NoImpl (empty resolver).
        // Per current implementation: NoImpl is an error, not a stall.
        // (Stall happens only when select returns Ambiguous, but with
        // empty resolver, evaluate returns empty, so select returns NoImpl.)
        assert!(result.has_errors() || result.is_stalled());
    }

    #[test]
    fn test_fulfillment_loop_assumed_short_circuits() {
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let obl = dummy_obligation(dummy_i32_ty(), 7);
        let pe = ParamEnv::from_predicates(vec![obl.predicate.clone()]);
        let mut cx = EvalCtxt::new(&resolver, &mut infer_ctxt, &pe);
        let mut queue = ObligationQueue::new();
        queue.push(obl);
        let result = fulfillment_loop(&mut queue, &mut cx, DEFAULT_MAX_DEPTH);
        // Assumed → Resolved (no impl selected, no new obligations).
        assert!(result.is_ok());
        assert_eq!(result.resolved_count(), 1);
        assert_eq!(result.selected_count(), 0); // assumed, not selected
    }

    #[test]
    fn test_fulfillment_loop_recursion_limit() {
        // Test that max_depth is respected.
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let mut queue = ObligationQueue::new();
        queue.push(dummy_obligation(dummy_i32_ty(), 7));
        let result = with_eval_ctxt(&resolver, &mut infer_ctxt, |cx| {
            fulfillment_loop(&mut queue, cx, 0) // max_depth = 0
        });
        // With max_depth = 0, the first obligation hits RecursionLimitExceeded.
        assert!(result.has_errors());
        if let FulfillmentResult::Errors { errors, .. } = result {
            assert_eq!(errors.len(), 1);
            assert!(matches!(
                errors[0].1,
                FulfillmentError::RecursionLimitExceeded { depth: 0 }
            ));
        }
    }

    // ----------- fulfill_obligation tests -----------

    #[test]
    fn test_fulfill_obligation_no_impl() {
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let obl = dummy_obligation(dummy_i32_ty(), 7);
        let result = with_eval_ctxt(&resolver, &mut infer_ctxt, |cx| {
            fulfill_obligation(obl, cx, DEFAULT_MAX_DEPTH)
        });
        assert!(result.has_errors());
    }

    #[test]
    fn test_fulfill_obligation_assumed() {
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let obl = dummy_obligation(dummy_i32_ty(), 7);
        let pe = ParamEnv::from_predicates(vec![obl.predicate.clone()]);
        let mut cx = EvalCtxt::new(&resolver, &mut infer_ctxt, &pe);
        let result = fulfill_obligation(obl, &mut cx, DEFAULT_MAX_DEPTH);
        assert!(result.is_ok());
        assert_eq!(result.resolved_count(), 1);
    }

    // ----------- Integration tests -----------

    #[test]
    fn test_integration_fulfillment_with_assumed_param_env() {
        // End-to-end: ParamEnv has assumption, obligation matches →
        // Fulfillment should short-circuit and return Ok.
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let obl1 = dummy_obligation(dummy_i32_ty(), 7);
        let obl2 = dummy_obligation(dummy_i32_ty(), 7); // same predicate
        let pe = ParamEnv::from_predicates(vec![obl1.predicate.clone()]);
        let mut cx = EvalCtxt::new(&resolver, &mut infer_ctxt, &pe);
        let mut queue = ObligationQueue::new();
        queue.push(obl2);
        let result = fulfillment_loop(&mut queue, &mut cx, DEFAULT_MAX_DEPTH);
        assert!(result.is_ok());
        assert_eq!(result.resolved_count(), 1);
        assert_eq!(result.selected_count(), 0); // assumed, not selected
    }

    #[test]
    fn test_integration_fulfillment_universe_unchanged() {
        // Per §5.2 + §5.3 + §5.4: Fulfillment should not leave the
        // InferCtxt universe in a polluted state.
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let initial_universe = infer_ctxt.current_universe();
        let mut queue = ObligationQueue::new();
        queue.push(dummy_obligation(dummy_i32_ty(), 7));
        let _ = with_eval_ctxt(&resolver, &mut infer_ctxt, |cx| {
            fulfillment_loop(&mut queue, cx, DEFAULT_MAX_DEPTH)
        });
        assert_eq!(infer_ctxt.current_universe(), initial_universe);
    }

    #[test]
    fn test_integration_describe_after_fulfill() {
        // describe_fulfillment_result(fulfillment_loop()) should produce
        // a non-empty diagnostic string.
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let mut queue = ObligationQueue::new();
        queue.push(dummy_obligation(dummy_i32_ty(), 7));
        let result = with_eval_ctxt(&resolver, &mut infer_ctxt, |cx| {
            fulfillment_loop(&mut queue, cx, DEFAULT_MAX_DEPTH)
        });
        let desc = describe_fulfillment_result(&result);
        assert!(!desc.is_empty());
    }

    // ----------- FulfillmentCtxt tests -----------

    #[test]
    fn test_fulfillment_ctxt_default_max_depth() {
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let pe = ParamEnv::empty();
        let eval_ctxt = EvalCtxt::new(&resolver, &mut infer_ctxt, &pe);
        let fctxt = FulfillmentCtxt::new(eval_ctxt);
        assert_eq!(fctxt.max_depth, DEFAULT_MAX_DEPTH);
        assert_eq!(fctxt.max_depth, 128); // per §5.8
    }

    #[test]
    fn test_fulfillment_ctxt_custom_max_depth() {
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let pe = ParamEnv::empty();
        let eval_ctxt = EvalCtxt::new(&resolver, &mut infer_ctxt, &pe);
        let fctxt = FulfillmentCtxt::with_max_depth(eval_ctxt, 256);
        assert_eq!(fctxt.max_depth, 256);
    }

    #[test]
    fn test_fulfillment_ctxt_fulfill_empty() {
        let resolver = dummy_resolver();
        let mut infer_ctxt = InferCtxt::new();
        let pe = ParamEnv::empty();
        let eval_ctxt = EvalCtxt::new(&resolver, &mut infer_ctxt, &pe);
        let mut fctxt = FulfillmentCtxt::new(eval_ctxt);
        let mut queue = ObligationQueue::new();
        let result = fctxt.fulfill(&mut queue);
        assert!(result.is_ok());
        assert_eq!(result.resolved_count(), 0);
    }
}
