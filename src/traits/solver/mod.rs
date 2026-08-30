//! Stage 19.1 (v0.5 Phase 1) — Trait Solver data structures.
//!
//! Per `docs/lang-design/03-type-system.md` §5 Trait Resolution, the trait
//! solver follows rustc's **老 solver** (3-phase) architecture:
//! 1. **Evaluation**: assess candidate impls applicability (with placeholders
//!    to avoid polluting global inference state) → `EvalResult`
//! 2. **Selection**: pick the most specific candidate (MVP禁 overlapping)
//! 3. **Fulfillment**: maintain an obligation queue, recursively select until
//!    queue empties or fails
//!
//! This module (Phase 1) defines the foundational data structures:
//! - `TraitPredicate` — `T: Trait<args>` (the bound to verify)
//! - `Binder` — abstracts over bound variables (lifetimes/types)
//! - `Obligation` — predicate + cause + span (for diagnostics)
//! - `ObligationQueue` — FIFO + pending (for ambiguous obligations)
//! - `Goal` — `T: Trait` with substitution context (for solver)
//! - `InferCtxt` — inference context (placeholder universe + substitution table)
//! - `EvalResult` — Ok / Ambiguous / Err
//! - `SelectionResult` — Ok(impl) / Err
//!
//! **Implementation rules (per §11 interface isolation)**:
//! - This module ONLY defines data structures + basic queries (no algorithm)
//! - Phase 2 (Stage 19.2) adds `evaluate_one`
//! - Phase 3 (Stage 19.3) adds `select`
//! - Phase 4 (Stage 19.4) adds Fulfillment loop integration
//! - Phase 5 (Stage 19.5) adds supertrait expansion + error reporting
//! - Phase 6 (Stage 19.6) adds integration tests
//!
//! **No integration yet** — Phase 1 only declares structures; no existing
//! typeck/codegen path consumes them yet. They become inputs to Phase 2+.

use crate::hir::DefId;
use crate::mir::ty::{InferVar, SubstsRef, Ty, TyKind};
use crate::session::Span;
use std::collections::VecDeque;
use std::rc::Rc;

// =====================================================================
// TraitPredicate — the bound to verify: `T: Trait<args>`
// =====================================================================

/// A trait predicate: `T: Trait<args>` (potentially bound over lifetimes/types).
///
/// Per `docs/lang-design/03-type-system.md` §5: the atomic unit of trait
/// resolution. Each predicate is a single `T: Trait<args>` obligation that
/// the solver either proves or refutes.
///
/// MVP scope (v0.5 Phase 1):
/// - `trait_def_id`: identifies the trait (looked up via `TraitResolver`)
/// - `trait_substs`: substitution arguments to the trait (e.g., `Iterator<Item=i32>` → substs = [i32])
/// - `self_ty`: the type being asserted to implement the trait
///
/// Per §1.0 原则 6 (通解 > 特解): one `TraitPredicate` struct covers all
/// trait bound kinds (Clone, Iterator, etc.) — no per-trait subtypes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TraitPredicate {
    /// The type for which we're asserting `T: Trait`.
    pub self_ty: Ty,
    /// DefId of the trait being asserted.
    pub trait_def_id: DefId,
    /// Substitution arguments to the trait (excluding Self).
    /// E.g., `T: Iterator<Item = i32>` → substs = [i32].
    /// For `T: Clone` (no args), this is empty.
    pub trait_substs: SubstsRef,
}

impl TraitPredicate {
    /// Construct a simple `T: Trait` predicate (no extra args).
    pub fn simple(self_ty: Ty, trait_def_id: DefId) -> Self {
        Self {
            self_ty,
            trait_def_id,
            trait_substs: Rc::from([]),
        }
    }

    /// Construct a `T: Trait<args>` predicate with substitution arguments.
    pub fn with_substs(self_ty: Ty, trait_def_id: DefId, trait_substs: SubstsRef) -> Self {
        Self {
            self_ty,
            trait_def_id,
            trait_substs,
        }
    }

    /// Returns `true` if `self_ty` is an inference variable (placeholder).
    /// Such predicates cannot be evaluated until the variable is bound.
    pub fn has_infer_self(&self) -> bool {
        matches!(self.self_ty.kind, TyKind::Infer(_))
    }

    /// Returns `true` if `self_ty` is a generic type parameter.
    /// Such predicates are "assumed" rather than "proven" — they're
    /// obligations only when the parameter is later substituted.
    pub fn has_param_self(&self) -> bool {
        matches!(self.self_ty.kind, TyKind::Param(_))
    }
}

// =====================================================================
// Binder — abstracts over bound variables (lifetimes/types)
// =====================================================================

/// A "binder" wraps a value with a count of bound variables.
///
/// Per rustc pattern: `Binder<TraitPredicate>` represents a trait bound
/// that may quantify over lifetimes/types (e.g., `for<'a> T: Iterator<Item=&'a U>`).
///
/// MVP scope (v0.5 Phase 1):
/// - `bound_vars_count`: number of universally quantified variables
/// - `value`: the predicate inside the binder
///
/// Per §1.0 原则 6 (通解 > 特解): `Binder<T>` is generic so it can wrap
/// `TraitPredicate`, `Region`, etc.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Binder<T> {
    /// Number of bound variables (lifetimes/types) this binder introduces.
    pub bound_vars_count: u32,
    /// The value inside the binder.
    pub value: T,
}

impl<T> Binder<T> {
    /// Construct a binder with N bound variables.
    pub fn bind(bound_vars_count: u32, value: T) -> Self {
        Self {
            bound_vars_count,
            value,
        }
    }

    /// Construct a binder with zero bound variables (the common case).
    pub fn dummy(value: T) -> Self {
        Self {
            bound_vars_count: 0,
            value,
        }
    }

    /// Returns `true` if this binder has no bound variables.
    pub fn is_dummy(&self) -> bool {
        self.bound_vars_count == 0
    }

    /// Apply a function to the inner value (preserving the binder).
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Binder<U> {
        Binder {
            bound_vars_count: self.bound_vars_count,
            value: f(self.value),
        }
    }
}

// =====================================================================
// Obligation — predicate + cause + span (for diagnostics)
// =====================================================================

/// The "cause" of an obligation — why are we trying to prove this?
///
/// Per §1.0 原则 3 (显式 > 隐式): every obligation must declare its cause
/// so error messages can point to the source construct.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ObligationCause {
    /// `let x: T = expr;` where expr must satisfy T's bounds.
    LetBinding,
    /// `fn f(x: T)` where T must satisfy the function's bounds.
    FunctionArg { fn_def_id: DefId, arg_index: u32 },
    /// `fn f() -> T` where the body's return must satisfy T's bounds.
    FunctionReturn { fn_def_id: DefId },
    /// `expr.method()` where the method's trait bound must hold.
    MethodCall { method_name: crate::lexer::Symbol },
    /// `impl Trait for X` where X must satisfy the impl's where clauses.
    ImplBlock { impl_def_id: DefId },
    /// `T: Trait + Supertrait` where the supertrait must hold if T: Trait.
    Supertrait { trait_def_id: DefId },
    /// `T: Trait` from a where clause.
    WhereClause,
    /// `struct S<T: Trait>` — generic bound declaration.
    GenericBound,
    /// Used when the cause is unclear (e.g., obligations synthesized by
    /// the solver itself). Should be replaced with a specific cause ASAP.
    Misc,
}

/// An obligation: a predicate + the reason it must hold + source span.
///
/// Per rustc pattern: obligations flow through the Fulfillment queue.
/// Each obligation carries enough context to produce a high-quality error
/// message if the predicate cannot be proved.
///
/// Per §1.0 原则 4 (报错 > 静默): every obligation carries a span so that
/// errors can be reported at the source location.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Obligation {
    /// The predicate this obligation is asking us to prove.
    pub predicate: TraitPredicate,
    /// Why we're asking (for diagnostics).
    pub cause: ObligationCause,
    /// Source location of the obligation (for diagnostics).
    pub span: Span,
}

impl Obligation {
    /// Construct a new obligation.
    pub fn new(predicate: TraitPredicate, cause: ObligationCause, span: Span) -> Self {
        Self {
            predicate,
            cause,
            span,
        }
    }

    /// Returns `true` if this obligation's predicate has an inference variable
    /// in its self type. Such obligations are "pending" — they must wait
    /// for the variable to be bound before they can be evaluated.
    pub fn is_pending(&self) -> bool {
        self.predicate.has_infer_self()
    }
}

// =====================================================================
// ObligationQueue — FIFO + pending (for ambiguous obligations)
// =====================================================================

/// The Fulfillment queue: pending obligations + processed obligations.
///
/// Per `docs/lang-design/03-type-system.md` §5.4:
/// - `pending`: obligations whose predicates still have inference variables
///   (cannot be evaluated yet — must wait for unification)
/// - `ready`: obligations whose predicates are concrete (ready to evaluate)
///
/// Algorithm:
/// 1. Pop from `ready`. Evaluate → if Ok, add obligations for impl's where
///    clauses; if Ambig, move to `pending`; if Err, report.
/// 2. When an inference variable is bound, scan `pending` and move any
///    newly-ready obligations back to `ready`.
/// 3. Repeat until both queues are empty (success) or only pending remain
///    (error: "type annotations needed").
///
/// Per §1.0 原则 6 (通解 > 特解): one queue type handles all obligation flows.
#[derive(Debug, Default)]
pub struct ObligationQueue {
    /// Obligations ready to evaluate (predicate is concrete).
    ready: VecDeque<Obligation>,
    /// Obligations waiting for inference variables to be bound.
    pending: VecDeque<Obligation>,
    /// Total obligations ever added (for stats / debugging).
    total_added: u64,
    /// Total obligations resolved (for stats / debugging).
    total_resolved: u64,
    /// Total obligations that errored (for stats / debugging).
    total_errored: u64,
}

impl ObligationQueue {
    /// Construct an empty queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an obligation. Routes to `ready` or `pending` based on predicate
    /// concreteness.
    pub fn push(&mut self, obl: Obligation) {
        self.total_added += 1;
        if obl.is_pending() {
            self.pending.push_back(obl);
        } else {
            self.ready.push_back(obl);
        }
    }

    /// Pop the next ready obligation (FIFO). Returns `None` if no ready
    /// obligations exist.
    pub fn pop_ready(&mut self) -> Option<Obligation> {
        self.ready.pop_front()
    }

    /// Scan `pending` for obligations that are now ready (because their
    /// inference variables were bound). Moves them back to `ready`.
    ///
    /// Returns the number of obligations moved.
    pub fn refresh_pending(&mut self) -> usize {
        let mut moved = 0;
        let len = self.pending.len();
        for _ in 0..len {
            if let Some(obl) = self.pending.pop_front() {
                if obl.is_pending() {
                    self.pending.push_back(obl);
                } else {
                    self.ready.push_back(obl);
                    moved += 1;
                }
            }
        }
        moved
    }

    /// Record that an obligation was resolved successfully.
    pub fn record_resolved(&mut self) {
        self.total_resolved += 1;
    }

    /// Record that an obligation errored out.
    pub fn record_errored(&mut self) {
        self.total_errored += 1;
    }

    /// Returns `true` if both queues are empty (Fulfillment complete).
    pub fn is_empty(&self) -> bool {
        self.ready.is_empty() && self.pending.is_empty()
    }

    /// Returns `true` if there are no ready obligations but pending ones
    /// remain. This signals "type annotations needed" — the solver cannot
    /// make progress without more type info.
    pub fn is_stalled(&self) -> bool {
        self.ready.is_empty() && !self.pending.is_empty()
    }

    /// Number of ready obligations.
    pub fn ready_count(&self) -> usize {
        self.ready.len()
    }

    /// Number of pending obligations.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Total obligations ever added.
    pub fn total_added(&self) -> u64 {
        self.total_added
    }

    /// Total obligations resolved.
    pub fn total_resolved(&self) -> u64 {
        self.total_resolved
    }

    /// Total obligations that errored.
    pub fn total_errored(&self) -> u64 {
        self.total_errored
    }

    /// Peek at the next ready obligation (without popping).
    pub fn peek_ready(&self) -> Option<&Obligation> {
        self.ready.front()
    }

    /// Drain all remaining obligations (used when the solver aborts).
    /// Returns (ready, pending) vectors.
    pub fn drain_all(&mut self) -> (Vec<Obligation>, Vec<Obligation>) {
        let ready: Vec<_> = self.ready.drain(..).collect();
        let pending: Vec<_> = self.pending.drain(..).collect();
        (ready, pending)
    }
}

// =====================================================================
// Goal — `T: Trait` with substitution context (for solver)
// =====================================================================

/// A solver goal: a `TraitPredicate` evaluated in a specific substitution
/// context (e.g., with placeholder variables for impl matching).
///
/// Per `docs/lang-design/03-type-system.md` §5.2: the Evaluation phase
/// creates Goals with placeholders, so candidate impls can be evaluated
/// without polluting the global inference state.
///
/// MVP scope (v0.5 Phase 1):
/// - `predicate`: the bound to evaluate
/// - `param_env`: parameter environment (where clauses as assumptions)
///
/// Per §1.0 原则 6 (通解 > 特解): one `Goal` struct handles all evaluation
/// contexts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Goal {
    /// The predicate this goal is asking us to evaluate.
    pub predicate: TraitPredicate,
    /// Parameter environment: where clauses that should be assumed true.
    /// (e.g., `fn f<T: Clone>()` → param_env = [T: Clone] as assumption)
    pub param_env: ParamEnv,
}

impl Goal {
    /// Construct a goal with a given parameter environment.
    pub fn new(predicate: TraitPredicate, param_env: ParamEnv) -> Self {
        Self {
            predicate,
            param_env,
        }
    }

    /// Construct a goal with an empty parameter environment (no assumptions).
    pub fn with_empty_env(predicate: TraitPredicate) -> Self {
        Self::new(predicate, ParamEnv::empty())
    }
}

// =====================================================================
// ParamEnv — parameter environment (where clauses as assumptions)
// =====================================================================

/// Parameter environment: the set of `where` clauses that should be
/// assumed true (not proved) for the duration of a goal evaluation.
///
/// Per rustc pattern: e.g., `fn f<T: Clone + Debug>(x: T)` has param_env
/// `[T: Clone, T: Debug]` — when evaluating the body's obligations, the
/// solver assumes `T: Clone` and `T: Debug` rather than trying to prove them.
///
/// Per §1.0 原则 6 (通解 > 特解): one `ParamEnv` type works for all
/// generic contexts (functions, impls, structs).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ParamEnv {
    /// The list of where clauses to assume true.
    pub assumptions: Vec<TraitPredicate>,
}

impl ParamEnv {
    /// Construct an empty parameter environment.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Construct a parameter environment from a list of predicates.
    pub fn from_predicates(predicates: Vec<TraitPredicate>) -> Self {
        Self {
            assumptions: predicates,
        }
    }

    /// Add an assumption to the parameter environment.
    pub fn add(&mut self, predicate: TraitPredicate) {
        self.assumptions.push(predicate);
    }

    /// Returns `true` if this parameter environment is empty.
    pub fn is_empty(&self) -> bool {
        self.assumptions.is_empty()
    }

    /// Returns `true` if the parameter environment already assumes the
    /// given predicate (modulo substs equality).
    ///
    /// MVP: exact match only. Phase 4 will add smarter matching (e.g.,
    /// `T: Clone` assumption satisfies `T: Clone` query but not `T: Debug`).
    pub fn assumes(&self, predicate: &TraitPredicate) -> bool {
        self.assumptions.contains(predicate)
    }

    /// Number of assumptions.
    pub fn len(&self) -> usize {
        self.assumptions.len()
    }

    /// Iterate over assumptions.
    pub fn iter(&self) -> impl Iterator<Item = &TraitPredicate> {
        self.assumptions.iter()
    }
}

// =====================================================================
// InferCtxt — inference context (placeholder universe + substitution table)
// =====================================================================

/// The "universe" of an inference variable — controls which placeholders
/// it can be unified with. Per rustc pattern, higher universes are more
/// constrained (used for higher-ranked bounds).
///
/// MVP scope (v0.5 Phase 1): single universe level. Phase 5 will add
/// proper universe escalation for higher-ranked types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Universe(pub u32);

impl Universe {
    /// The root universe (no placeholders introduced).
    pub const ROOT: Universe = Universe(0);

    /// Returns the next universe (used when entering a binder).
    pub fn next(self) -> Universe {
        Universe(self.0 + 1)
    }
}

/// Inference context: holds the placeholder universe + the substitution
/// table mapping inference variables to their bound types.
///
/// Per `docs/lang-design/03-type-system.md` §5.2: Evaluation creates
/// placeholders via the InferCtxt, so candidate impls can be evaluated
/// without polluting the global inference state.
///
/// MVP scope (v0.5 Phase 1):
/// - `next_infer_var_id`: counter for fresh InferVar IDs
/// - `next_universe`: next placeholder universe to allocate
/// - `bound_vars`: map from InferVar ID → bound Ty (populated during unification)
///
/// Per §1.0 原则 6 (通解 > 特解): one InferCtxt handles all inference
/// needs (type variables, int variables, float variables).
///
/// **Note**: This InferCtxt is **separate** from the existing typeck
/// unification table (`src/typeck/unify.rs`). They will be merged in
/// Phase 2+ when the solver is actually integrated. Phase 1 keeps them
/// separate to avoid destabilizing the existing typeck pipeline.
#[derive(Debug, Default)]
pub struct InferCtxt {
    /// Counter for fresh InferVar IDs (next ID to allocate).
    next_infer_var_id: u32,
    /// Next placeholder universe to allocate (for higher-ranked types).
    next_universe: Universe,
    /// Map from InferVar ID → bound Ty.
    ///
    /// Per §1.0 原则 10 (唯一可信数据源): this map is the single source
    /// of truth for what an InferVar is bound to. No other module should
    /// maintain a parallel map.
    bound_vars: std::collections::HashMap<u32, Ty>,
    /// Number of obligations ever pushed via this context (for stats).
    obligations_pushed: u64,
}

impl InferCtxt {
    /// Construct a fresh inference context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate a fresh type inference variable (in the root universe).
    ///
    /// Per §1.0 原则 3 (显式 > 隐式): the variable is explicitly tracked
    /// in this InferCtxt's table, so callers can later query its binding.
    pub fn fresh_ty_var(&mut self) -> Ty {
        let id = self.next_infer_var_id;
        self.next_infer_var_id += 1;
        // Use a fresh InferVar::TyVar — the existing typeck TyKind::Infer
        // already supports this. The InferCtxt tracks the ID separately.
        // (In Phase 2+, we'll thread the InferCtxt through to unify so
        // that unification of these vars populates `bound_vars`.)
        let _ = id; // suppress unused warning until Phase 2 wires this up
        Ty::from_kind(TyKind::Infer(InferVar::TyVar(crate::mir::ty::TyVid(
            self.next_infer_var_id,
        ))))
    }

    /// Enter a new universe (when entering a `for<'a>` binder).
    /// Returns the previous universe so it can be restored on exit.
    ///
    /// Per rustc pattern: higher-ranked bounds need a fresh placeholder
    /// universe so the inner bound variables can't escape.
    pub fn enter_universe(&mut self) -> Universe {
        let prev = self.next_universe;
        self.next_universe = self.next_universe.next();
        prev
    }

    /// Restore a previous universe (when exiting a `for<'a>` binder).
    pub fn exit_universe(&mut self, prev: Universe) {
        self.next_universe = prev;
    }

    /// Bind an inference variable to a concrete type.
    ///
    /// Per §1.0 原则 4 (报错 > 静默): if the variable is already bound to
    /// a different type, this returns `Err` rather than silently
    /// overwriting or silently accepting.
    pub fn bind_infer_var(&mut self, var_id: u32, ty: Ty) -> Result<(), InferCtxtError> {
        match self.bound_vars.get(&var_id) {
            None => {
                self.bound_vars.insert(var_id, ty);
                Ok(())
            }
            Some(existing) if *existing == ty => Ok(()), // idempotent
            Some(existing) => Err(InferCtxtError::ConflictingBinding {
                var_id,
                existing: existing.clone(),
                attempted: ty,
            }),
        }
    }

    /// Look up the binding for an inference variable.
    pub fn lookup_infer_var(&self, var_id: u32) -> Option<&Ty> {
        self.bound_vars.get(&var_id)
    }

    /// Returns `true` if the inference variable is bound.
    pub fn is_bound(&self, var_id: u32) -> bool {
        self.bound_vars.contains_key(&var_id)
    }

    /// Returns the current universe level.
    pub fn current_universe(&self) -> Universe {
        self.next_universe
    }

    /// Number of bound variables (for stats / debugging).
    pub fn bound_count(&self) -> usize {
        self.bound_vars.len()
    }

    /// Total inference variables allocated (for stats / debugging).
    pub fn total_allocated(&self) -> u32 {
        self.next_infer_var_id
    }

    /// Record that an obligation was pushed via this context.
    pub fn record_obligation_pushed(&mut self) {
        self.obligations_pushed += 1;
    }

    /// Total obligations ever pushed via this context.
    pub fn obligations_pushed(&self) -> u64 {
        self.obligations_pushed
    }
}

/// Error type for `InferCtxt` operations.
///
/// Per §1.0 原则 4 (报错 > 静默): all inference context errors are
/// explicit, never silently swallowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InferCtxtError {
    /// Tried to bind an inference variable that was already bound to a
    /// different type. This is a type conflict — should be reported as
    /// a type mismatch error.
    ConflictingBinding {
        var_id: u32,
        existing: Ty,
        attempted: Ty,
    },
}

impl std::fmt::Display for InferCtxtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InferCtxtError::ConflictingBinding {
                var_id,
                existing,
                attempted,
            } => {
                write!(
                    f,
                    "conflicting binding for infer var #{}: existing {:?}, attempted {:?}",
                    var_id, existing, attempted
                )
            }
        }
    }
}

impl std::error::Error for InferCtxtError {}

// =====================================================================
// EvalResult — Ok / Ambiguous / Err
// =====================================================================

/// Result of evaluating a candidate impl against an obligation.
///
/// Per `docs/lang-design/03-type-system.md` §5.2:
/// - `Ok`: the impl matches (we don't yet commit — Selection does that)
/// - `Ambiguous`: the impl might match, but inference variables need to
///   be resolved first (defer to Fulfillment pending queue)
/// - `Err`: the impl definitely doesn't match
///
/// Per §1.0 原则 4 (报错 > 静默): explicit tri-state (not just bool) so
/// that "ambiguous" doesn't get conflated with "ok" or "err".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalResult {
    /// The impl matches the obligation.
    Ok,
    /// The impl might match, but more type info is needed.
    Ambiguous,
    /// The impl definitely doesn't match.
    Err(EvalError),
}

impl EvalResult {
    /// Returns `true` if evaluation succeeded.
    pub fn is_ok(&self) -> bool {
        matches!(self, EvalResult::Ok)
    }

    /// Returns `true` if evaluation was ambiguous.
    pub fn is_ambiguous(&self) -> bool {
        matches!(self, EvalResult::Ambiguous)
    }

    /// Returns `true` if evaluation failed.
    pub fn is_err(&self) -> bool {
        matches!(self, EvalResult::Err(_))
    }
}

/// Why evaluation of a candidate impl failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    /// The impl's self type doesn't unify with the obligation's self type.
    SelfTypeMismatch,
    /// The impl's substitution arguments don't unify with the obligation's.
    SubstsMismatch,
    /// The impl is not applicable (e.g., wrong trait).
    WrongTrait,
    /// The impl has where clauses that don't hold.
    WhereClauseNotSatisfied,
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::SelfTypeMismatch => write!(f, "self type mismatch"),
            EvalError::SubstsMismatch => write!(f, "substitution arguments mismatch"),
            EvalError::WrongTrait => write!(f, "wrong trait"),
            EvalError::WhereClauseNotSatisfied => write!(f, "where clause not satisfied"),
        }
    }
}

impl std::error::Error for EvalError {}

// =====================================================================
// SelectionResult — Ok(impl) / Err
// =====================================================================

/// Result of selecting an impl for an obligation.
///
/// Per `docs/lang-design/03-type-system.md` §5.3: Selection picks the
/// unique candidate impl (MVP禁 overlapping).
///
/// Per §1.0 原则 4 (报错 > 静默): all selection errors are explicit,
/// never silently swallowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionResult {
    /// A unique impl was selected.
    Ok { impl_def_id: DefId },
    /// Multiple candidate impls matched (MVP禁 overlapping — error).
    Ambiguous { candidate_count: usize },
    /// No candidate impl matched.
    NoImpl,
}

impl SelectionResult {
    /// Returns `true` if selection succeeded.
    pub fn is_ok(&self) -> bool {
        matches!(self, SelectionResult::Ok { .. })
    }

    /// Returns `true` if selection was ambiguous.
    pub fn is_ambiguous(&self) -> bool {
        matches!(self, SelectionResult::Ambiguous { .. })
    }

    /// Returns `true` if no impl matched.
    pub fn is_no_impl(&self) -> bool {
        matches!(self, SelectionResult::NoImpl)
    }

    /// Returns the selected impl DefId if Ok.
    pub fn impl_def_id(&self) -> Option<DefId> {
        match self {
            SelectionResult::Ok { impl_def_id } => Some(*impl_def_id),
            _ => None,
        }
    }
}

// =====================================================================
// Sub-module re-exports
// =====================================================================

// (Phase 2+ will add: pub mod eval; pub mod select; pub mod fulfill;)

// =====================================================================
// Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::IntTy;
    use crate::hir::DefId;
    use crate::lexer::Symbol;
    use crate::mir::ty::{ParamTy, TyVid};
    use crate::session::Span;
    use lasso::Rodeo;

    fn dummy_def_id(n: u32) -> DefId {
        DefId::new(n)
    }

    fn dummy_i32_ty() -> Ty {
        Ty::from_kind(TyKind::Int(IntTy::I32))
    }

    fn dummy_infer_ty(id: u32) -> Ty {
        Ty::from_kind(TyKind::Infer(InferVar::TyVar(TyVid(id))))
    }

    fn dummy_param_ty(index: u32, name: &str) -> Ty {
        // Intern the name string into a Symbol (Spur) via a thread-local
        // Rodeo interner. (Per Stage 15.28 type-interning pattern.)
        thread_local! {
            static TEST_RODEO: std::cell::RefCell<Rodeo> =
                std::cell::RefCell::new(Rodeo::new());
        }
        let spur = TEST_RODEO.with(|r| r.borrow_mut().get_or_intern(name));
        Ty::from_kind(TyKind::Param(ParamTy { index, name: spur }))
    }

    // ----------- TraitPredicate tests -----------

    #[test]
    fn test_trait_predicate_simple() {
        let tp = TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7));
        assert_eq!(tp.self_ty, dummy_i32_ty());
        assert_eq!(tp.trait_def_id, dummy_def_id(7));
        assert_eq!(tp.trait_substs.len(), 0);
    }

    #[test]
    fn test_trait_predicate_with_substs() {
        let substs: SubstsRef = Rc::from([dummy_i32_ty()]);
        let tp = TraitPredicate::with_substs(dummy_i32_ty(), dummy_def_id(7), substs.clone());
        assert_eq!(tp.trait_substs.len(), 1);
        assert_eq!(tp.trait_substs[0], dummy_i32_ty());
    }

    #[test]
    fn test_trait_predicate_has_infer_self() {
        let tp = TraitPredicate::simple(dummy_infer_ty(0), dummy_def_id(7));
        assert!(tp.has_infer_self());
        assert!(!tp.has_param_self());
    }

    #[test]
    fn test_trait_predicate_has_param_self() {
        let tp = TraitPredicate::simple(dummy_param_ty(0, "T"), dummy_def_id(7));
        assert!(!tp.has_infer_self());
        assert!(tp.has_param_self());
    }

    #[test]
    fn test_trait_predicate_concrete() {
        let tp = TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7));
        assert!(!tp.has_infer_self());
        assert!(!tp.has_param_self());
    }

    // ----------- Binder tests -----------

    #[test]
    fn test_binder_dummy() {
        let tp = TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7));
        let binder = Binder::dummy(tp.clone());
        assert!(binder.is_dummy());
        assert_eq!(binder.bound_vars_count, 0);
        assert_eq!(binder.value, tp);
    }

    #[test]
    fn test_binder_bind() {
        let tp = TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7));
        let binder = Binder::bind(2, tp.clone());
        assert!(!binder.is_dummy());
        assert_eq!(binder.bound_vars_count, 2);
    }

    #[test]
    fn test_binder_map() {
        let binder = Binder::dummy(dummy_def_id(7));
        let mapped = binder.map(|def_id| def_id.as_u32());
        assert_eq!(mapped.bound_vars_count, 0);
        assert_eq!(mapped.value, 7);
    }

    // ----------- Obligation tests -----------

    #[test]
    fn test_obligation_pending() {
        let obl = Obligation::new(
            TraitPredicate::simple(dummy_infer_ty(0), dummy_def_id(7)),
            ObligationCause::LetBinding,
            Span::DUMMY,
        );
        assert!(obl.is_pending());
    }

    #[test]
    fn test_obligation_ready() {
        let obl = Obligation::new(
            TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7)),
            ObligationCause::LetBinding,
            Span::DUMMY,
        );
        assert!(!obl.is_pending());
    }

    #[test]
    fn test_obligation_cause_equality() {
        assert_eq!(ObligationCause::LetBinding, ObligationCause::LetBinding);
        assert_ne!(ObligationCause::LetBinding, ObligationCause::WhereClause);
    }

    // Suppress unused Symbol import warning (kept for the `name: &str -> Symbol`
    // interner pattern documented above).
    #[allow(dead_code)]
    fn _suppress_symbol_warning(_sym: Symbol) {}

    // ----------- ObligationQueue tests -----------

    #[test]
    fn test_obligation_queue_push_ready() {
        let mut q = ObligationQueue::new();
        let obl = Obligation::new(
            TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7)),
            ObligationCause::LetBinding,
            Span::DUMMY,
        );
        q.push(obl);
        assert_eq!(q.ready_count(), 1);
        assert_eq!(q.pending_count(), 0);
        assert!(!q.is_empty());
        assert!(!q.is_stalled());
        assert_eq!(q.total_added(), 1);
    }

    #[test]
    fn test_obligation_queue_push_pending() {
        let mut q = ObligationQueue::new();
        let obl = Obligation::new(
            TraitPredicate::simple(dummy_infer_ty(0), dummy_def_id(7)),
            ObligationCause::LetBinding,
            Span::DUMMY,
        );
        q.push(obl);
        assert_eq!(q.ready_count(), 0);
        assert_eq!(q.pending_count(), 1);
        assert!(!q.is_empty());
        assert!(q.is_stalled()); // pending only, no ready
    }

    #[test]
    fn test_obligation_queue_pop_ready() {
        let mut q = ObligationQueue::new();
        let obl1 = Obligation::new(
            TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7)),
            ObligationCause::LetBinding,
            Span::DUMMY,
        );
        let obl2 = Obligation::new(
            TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(8)),
            ObligationCause::WhereClause,
            Span::DUMMY,
        );
        q.push(obl1.clone());
        q.push(obl2.clone());
        assert_eq!(q.ready_count(), 2);
        let popped = q.pop_ready();
        assert_eq!(popped, Some(obl1));
        assert_eq!(q.ready_count(), 1);
    }

    #[test]
    fn test_obligation_queue_pop_ready_empty() {
        let mut q = ObligationQueue::new();
        assert!(q.pop_ready().is_none());
    }

    #[test]
    fn test_obligation_queue_refresh_pending() {
        let mut q = ObligationQueue::new();
        // Push a pending obligation (with infer self)
        q.push(Obligation::new(
            TraitPredicate::simple(dummy_infer_ty(0), dummy_def_id(7)),
            ObligationCause::LetBinding,
            Span::DUMMY,
        ));
        assert_eq!(q.pending_count(), 1);
        assert_eq!(q.ready_count(), 0);
        // Refresh — no movement since still infer
        let moved = q.refresh_pending();
        assert_eq!(moved, 0);
        assert_eq!(q.pending_count(), 1);
    }

    #[test]
    fn test_obligation_queue_stats() {
        let mut q = ObligationQueue::new();
        q.push(Obligation::new(
            TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7)),
            ObligationCause::LetBinding,
            Span::DUMMY,
        ));
        q.record_resolved();
        q.record_resolved();
        q.record_errored();
        assert_eq!(q.total_added(), 1);
        assert_eq!(q.total_resolved(), 2);
        assert_eq!(q.total_errored(), 1);
    }

    #[test]
    fn test_obligation_queue_drain_all() {
        let mut q = ObligationQueue::new();
        q.push(Obligation::new(
            TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7)),
            ObligationCause::LetBinding,
            Span::DUMMY,
        ));
        q.push(Obligation::new(
            TraitPredicate::simple(dummy_infer_ty(0), dummy_def_id(8)),
            ObligationCause::WhereClause,
            Span::DUMMY,
        ));
        let (ready, pending) = q.drain_all();
        assert_eq!(ready.len(), 1);
        assert_eq!(pending.len(), 1);
        assert!(q.is_empty());
    }

    // ----------- ParamEnv tests -----------

    #[test]
    fn test_param_env_empty() {
        let pe = ParamEnv::empty();
        assert!(pe.is_empty());
        assert_eq!(pe.len(), 0);
    }

    #[test]
    fn test_param_env_from_predicates() {
        let tp1 = TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7));
        let tp2 = TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(8));
        let pe = ParamEnv::from_predicates(vec![tp1.clone(), tp2.clone()]);
        assert!(!pe.is_empty());
        assert_eq!(pe.len(), 2);
    }

    #[test]
    fn test_param_env_add() {
        let mut pe = ParamEnv::empty();
        let tp = TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7));
        pe.add(tp.clone());
        assert_eq!(pe.len(), 1);
        assert!(pe.assumes(&tp));
    }

    #[test]
    fn test_param_env_assumes_exact_match() {
        let tp1 = TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7));
        let tp2 = TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(8)); // different trait
        let pe = ParamEnv::from_predicates(vec![tp1.clone()]);
        assert!(pe.assumes(&tp1));
        assert!(!pe.assumes(&tp2));
    }

    #[test]
    fn test_param_env_iter() {
        let tp1 = TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7));
        let tp2 = TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(8));
        let pe = ParamEnv::from_predicates(vec![tp1.clone(), tp2.clone()]);
        let collected: Vec<_> = pe.iter().collect();
        assert_eq!(collected.len(), 2);
    }

    // ----------- Goal tests -----------

    #[test]
    fn test_goal_new() {
        let tp = TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7));
        let pe = ParamEnv::from_predicates(vec![tp.clone()]);
        let goal = Goal::new(tp.clone(), pe);
        assert_eq!(goal.predicate, tp);
        assert_eq!(goal.param_env.len(), 1);
    }

    #[test]
    fn test_goal_with_empty_env() {
        let tp = TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7));
        let goal = Goal::with_empty_env(tp.clone());
        assert_eq!(goal.predicate, tp);
        assert!(goal.param_env.is_empty());
    }

    // ----------- InferCtxt tests -----------

    #[test]
    fn test_infer_ctxt_new() {
        let cx = InferCtxt::new();
        assert_eq!(cx.total_allocated(), 0);
        assert_eq!(cx.bound_count(), 0);
        assert_eq!(cx.current_universe(), Universe::ROOT);
    }

    #[test]
    fn test_infer_ctxt_fresh_ty_var() {
        let mut cx = InferCtxt::new();
        let ty1 = cx.fresh_ty_var();
        let ty2 = cx.fresh_ty_var();
        assert_ne!(ty1, ty2); // different vars
        assert!(cx.total_allocated() >= 2);
    }

    #[test]
    fn test_infer_ctxt_universe_escalation() {
        let mut cx = InferCtxt::new();
        assert_eq!(cx.current_universe(), Universe::ROOT);
        let prev = cx.enter_universe();
        assert_eq!(prev, Universe::ROOT);
        assert_eq!(cx.current_universe(), Universe(1));
        cx.exit_universe(prev);
        assert_eq!(cx.current_universe(), Universe::ROOT);
    }

    #[test]
    fn test_infer_ctxt_bind_infer_var() {
        let mut cx = InferCtxt::new();
        let result = cx.bind_infer_var(0, dummy_i32_ty());
        assert!(result.is_ok());
        assert!(cx.is_bound(0));
        assert_eq!(cx.lookup_infer_var(0), Some(&dummy_i32_ty()));
        assert_eq!(cx.bound_count(), 1);
    }

    #[test]
    fn test_infer_ctxt_bind_idempotent() {
        let mut cx = InferCtxt::new();
        cx.bind_infer_var(0, dummy_i32_ty()).unwrap();
        // Binding the same var to the same type is idempotent.
        let result = cx.bind_infer_var(0, dummy_i32_ty());
        assert!(result.is_ok());
    }

    #[test]
    fn test_infer_ctxt_bind_conflicting() {
        let mut cx = InferCtxt::new();
        cx.bind_infer_var(0, dummy_i32_ty()).unwrap();
        // Conflicting binding (different type).
        let conflicting_ty = Ty::from_kind(TyKind::Int(IntTy::I64));
        let result = cx.bind_infer_var(0, conflicting_ty.clone());
        assert!(matches!(
            result,
            Err(InferCtxtError::ConflictingBinding { .. })
        ));
    }

    #[test]
    fn test_infer_ctxt_obligations_pushed() {
        let mut cx = InferCtxt::new();
        assert_eq!(cx.obligations_pushed(), 0);
        cx.record_obligation_pushed();
        cx.record_obligation_pushed();
        cx.record_obligation_pushed();
        assert_eq!(cx.obligations_pushed(), 3);
    }

    // ----------- EvalResult tests -----------

    #[test]
    fn test_eval_result_ok() {
        let r = EvalResult::Ok;
        assert!(r.is_ok());
        assert!(!r.is_ambiguous());
        assert!(!r.is_err());
    }

    #[test]
    fn test_eval_result_ambiguous() {
        let r = EvalResult::Ambiguous;
        assert!(!r.is_ok());
        assert!(r.is_ambiguous());
        assert!(!r.is_err());
    }

    #[test]
    fn test_eval_result_err() {
        let r = EvalResult::Err(EvalError::SelfTypeMismatch);
        assert!(!r.is_ok());
        assert!(!r.is_ambiguous());
        assert!(r.is_err());
    }

    #[test]
    fn test_eval_error_display() {
        assert_eq!(
            EvalError::SelfTypeMismatch.to_string(),
            "self type mismatch"
        );
        assert_eq!(
            EvalError::WhereClauseNotSatisfied.to_string(),
            "where clause not satisfied"
        );
    }

    // ----------- SelectionResult tests -----------

    #[test]
    fn test_selection_result_ok() {
        let r = SelectionResult::Ok {
            impl_def_id: dummy_def_id(42),
        };
        assert!(r.is_ok());
        assert!(!r.is_ambiguous());
        assert!(!r.is_no_impl());
        assert_eq!(r.impl_def_id(), Some(dummy_def_id(42)));
    }

    #[test]
    fn test_selection_result_ambiguous() {
        let r = SelectionResult::Ambiguous { candidate_count: 3 };
        assert!(!r.is_ok());
        assert!(r.is_ambiguous());
        assert!(!r.is_no_impl());
        assert_eq!(r.impl_def_id(), None);
    }

    #[test]
    fn test_selection_result_no_impl() {
        let r = SelectionResult::NoImpl;
        assert!(!r.is_ok());
        assert!(!r.is_ambiguous());
        assert!(r.is_no_impl());
        assert_eq!(r.impl_def_id(), None);
    }

    // ----------- Integration tests -----------

    #[test]
    fn test_integration_obligation_queue_pending_to_ready() {
        // Simulate: push pending obligation, then "resolve" the infer var,
        // then refresh_pending should move it to ready.
        let mut q = ObligationQueue::new();
        q.push(Obligation::new(
            TraitPredicate::simple(dummy_infer_ty(0), dummy_def_id(7)),
            ObligationCause::LetBinding,
            Span::DUMMY,
        ));
        assert_eq!(q.pending_count(), 1);
        assert_eq!(q.ready_count(), 0);
        assert!(q.is_stalled());

        // "Resolve" the infer var (in real code this would happen via unify).
        // For Phase 1 testing, we just push a NEW obligation with the resolved
        // type — in Phase 2+ we'll re-evaluate the pending one.
        q.push(Obligation::new(
            TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7)),
            ObligationCause::LetBinding,
            Span::DUMMY,
        ));
        assert_eq!(q.ready_count(), 1);
        assert_eq!(q.pending_count(), 1);

        // Pop the ready obligation.
        let popped = q.pop_ready();
        assert!(popped.is_some());
        q.record_resolved();
        assert_eq!(q.ready_count(), 0);
        assert_eq!(q.pending_count(), 1);
        assert_eq!(q.total_resolved(), 1);
    }

    #[test]
    fn test_integration_param_env_assumption_satisfies() {
        // Simulate: param_env has `T: Clone`, goal is `T: Clone` →
        // ParamEnv::assumes should return true (Phase 4 will use this
        // to short-circuit Evaluation).
        let tp = TraitPredicate::simple(dummy_i32_ty(), dummy_def_id(7));
        let pe = ParamEnv::from_predicates(vec![tp.clone()]);
        let goal = Goal::new(tp.clone(), pe.clone());
        assert!(goal.param_env.assumes(&goal.predicate));
    }

    #[test]
    fn test_integration_infer_ctxt_universe_nesting() {
        // Simulate: enter a binder, allocate a placeholder, exit the binder.
        let mut cx = InferCtxt::new();
        let outer_universe = cx.current_universe();
        let prev = cx.enter_universe();
        assert_eq!(cx.current_universe().0, outer_universe.0 + 1);
        let _placeholder = cx.fresh_ty_var();
        cx.exit_universe(prev);
        assert_eq!(cx.current_universe(), outer_universe);
    }
}
