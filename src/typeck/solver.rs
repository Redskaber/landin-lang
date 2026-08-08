//! Stage 17.03-17.04: Trait Solver — data structures + where clause assumptions.
//!
//! This module defines the core data structures for the trait solver:
//! - `TraitPredicate` — a claim that "Type: Trait"
//! - `Goal` — a goal to be evaluated (currently only `Implies`)
//! - `GoalEvaluationResult` — the result of evaluating a goal
//! - `TraitSolverCtxt` — context holding resolver + interner + assumptions
//!
//! Phase 1 (Stage 17.03): data structures + stub evaluate().
//! Phase 2 (Stage 17.04): where clause assumptions + driver integration.
//!
//! Per §23: all types follow standard naming patterns.
//! Per §16: reads TraitResolver (allowed during typeck).
//! Per §1.0 原則 6 "通用 > 特例": one solver handles all goal types.

use crate::hir::DefId;
use crate::mir::ty::{Ty, TyKind};
use crate::traits::TraitResolver;
use lasso::Rodeo;

/// A trait predicate: "Type: Trait"
///
/// Represents a claim that `ty` implements `trait_def_id`.
/// This is the fundamental unit of the trait solver.
///
/// Per §23: `TraitPredicate` follows `<Noun>_<Noun>` pattern.
#[derive(Debug, Clone)]
pub struct TraitPredicate {
    /// The type that should implement the trait.
    pub ty: Ty,
    /// The DefId of the trait.
    pub trait_def_id: DefId,
}

impl TraitPredicate {
    /// Create a new TraitPredicate.
    ///
    /// Per §23: `new` follows `<verb>` pattern.
    pub fn new(ty: Ty, trait_def_id: DefId) -> Self {
        Self { ty, trait_def_id }
    }
}

/// A goal to be evaluated by the trait solver.
///
/// Currently only supports "does type T implement trait X?"
/// Future phases may add:
/// - `Projection(Ty, DefId)` — resolve associated type
/// - `Eq(Ty, Ty)` — type equality
///
/// Per §23: `Goal` follows `<Noun>` pattern.
#[derive(Debug, Clone)]
pub enum Goal {
    /// Prove that `ty` implements `trait_def_id`.
    Implies(TraitPredicate),
}

/// The result of evaluating a goal.
///
/// Per §23: `GoalEvaluationResult` follows `<Noun>_<Noun>_<Noun>` pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalEvaluationResult {
    /// The goal is provably true (the type implements the trait).
    Yes,
    /// The goal is provably false (the type does NOT implement the trait).
    No,
    /// The goal cannot be determined yet.
    /// This happens when the type is:
    /// - An inference variable (not yet resolved)
    /// - A type parameter (declarative constraint, checked at monomorphization)
    Ambiguous,
}

/// Context for the trait solver.
///
/// Holds a reference to TraitResolver for impl lookup, the interner
/// for name resolution, and a list of assumptions (where clause bounds)
/// that are treated as proven.
///
/// Per §23: `TraitSolverCtxt` follows `<Noun>_<Noun>_<Noun>` pattern.
/// Per §16: reads TraitResolver + interner (allowed during typeck).
pub struct TraitSolverCtxt<'a> {
    /// The trait resolver for looking up impl blocks.
    pub resolver: &'a TraitResolver,
    /// The interner for resolving Spur → &str.
    pub interner: &'a Rodeo,
    /// Stage 17.04: Assumptions from where clauses.
    /// Each assumption is a (DefId, DefId) pair meaning "type_def_id implements trait_def_id".
    /// When evaluating a goal, if the goal matches an assumption, it returns Yes.
    assumptions: Vec<(DefId, DefId)>,
}

impl<'a> TraitSolverCtxt<'a> {
    /// Create a new TraitSolverCtxt with no assumptions.
    ///
    /// Per §23: `new` follows `<verb>` pattern.
    pub fn new(resolver: &'a TraitResolver, interner: &'a Rodeo) -> Self {
        Self {
            resolver,
            interner,
            assumptions: Vec::new(),
        }
    }

    /// Stage 17.04: Create a new TraitSolverCtxt with where clause assumptions.
    ///
    /// Assumptions are (type_def_id, trait_def_id) pairs extracted from
    /// where clauses on generic items. When evaluating a goal for a
    /// concrete type, if the goal matches an assumption, it returns Yes
    /// without consulting the resolver.
    ///
    /// Per §23: `with_assumptions` follows `<prep>_<noun>` pattern.
    pub fn with_assumptions(
        resolver: &'a TraitResolver,
        interner: &'a Rodeo,
        assumptions: Vec<(DefId, DefId)>,
    ) -> Self {
        Self {
            resolver,
            interner,
            assumptions,
        }
    }

    /// Evaluate a goal.
    ///
    /// Phase 2: supports where clause assumptions. If the goal matches
    /// an assumption (same type_def_id + trait_def_id), returns Yes.
    /// Otherwise delegates to `evaluate_implies` for resolver-based lookup.
    ///
    /// Per §1.0 原則 4 "报错 > 静默": returns `No` for definite non-implementation.
    /// Per §1.0 原則 6 "通用 > 特例": one evaluate method handles all goal types.
    pub fn evaluate(&self, goal: &Goal) -> GoalEvaluationResult {
        match goal {
            Goal::Implies(pred) => self.evaluate_implies(pred),
        }
    }

    /// Evaluate a "Type: Trait" predicate.
    fn evaluate_implies(&self, pred: &TraitPredicate) -> GoalEvaluationResult {
        match &pred.ty.kind {
            // Error type: suppress (treat as satisfied to avoid cascading errors).
            TyKind::Error => GoalEvaluationResult::Yes,

            // Inference variable: cannot determine yet.
            TyKind::Infer(_) => GoalEvaluationResult::Ambiguous,

            // Type parameter: declarative constraint, checked at monomorphization.
            TyKind::Param(_) => GoalEvaluationResult::Ambiguous,

            // Concrete ADT type (struct/enum): check via resolver + assumptions.
            TyKind::Adt(def_id, _) => {
                // Stage 17.04: Check assumptions first.
                if self
                    .assumptions
                    .iter()
                    .any(|(ty_id, trait_id)| *ty_id == *def_id && *trait_id == pred.trait_def_id)
                {
                    return GoalEvaluationResult::Yes;
                }

                // Fall back to resolver lookup.
                if self
                    .resolver
                    .implements_by_def_ids(pred.trait_def_id, *def_id)
                {
                    GoalEvaluationResult::Yes
                } else {
                    GoalEvaluationResult::No
                }
            }

            // Other types (primitives, tuples, refs, etc.):
            // Phase 2: treat as Ambiguous (Phase 3 will handle primitive impls).
            _ => GoalEvaluationResult::Ambiguous,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile;
    use crate::session::Span;

    /// Stage 17.03 positive 1: TraitPredicate constructs correctly.
    #[test]
    fn stage17_03_trait_predicate_construction() {
        let ty = Ty::new(TyKind::Bool, Span::DUMMY);
        let pred = TraitPredicate::new(ty, DefId(42));
        assert_eq!(pred.trait_def_id, DefId(42));
        assert!(matches!(pred.ty.kind, TyKind::Bool));
    }

    /// Stage 17.03 positive 2: Goal evaluation for concrete type that implements trait → Yes.
    #[test]
    fn stage17_03_goal_evaluation_concrete_type_implements() {
        let src = "trait Foo { fn foo(&self); } struct S; impl Foo for S { fn foo(&self) {} } fn main() { 0 }";
        let result = compile(src);
        let resolver = &result.trait_resolver;
        let interner = &result.interner;

        // Find Foo trait DefId.
        let foo_name = interner.get("Foo").expect("Foo should be interned");
        let foo_def_id = resolver
            .find_trait_def_id(foo_name)
            .expect("Foo trait should be registered");

        // Find S struct DefId.
        let s_name = interner.get("S").expect("S should be interned");
        let _s_def_id = resolver
            .trait_by_name
            .iter()
            .find(|(name, _)| **name == s_name)
            .map(|(_, def_id)| *def_id)
            .unwrap_or(DefId(0));

        // Actually, find S via type_by_def_id.
        let mut struct_def_id = None;
        for (def_id, spur) in &resolver.type_by_def_id {
            if interner.resolve(spur) == "S" {
                struct_def_id = Some(*def_id);
                break;
            }
        }
        let s_def_id = struct_def_id.expect("S not found in type_by_def_id");

        let ty = Ty::new(TyKind::Adt(s_def_id, Vec::new().into()), Span::DUMMY);
        let pred = TraitPredicate::new(ty, foo_def_id);
        let goal = Goal::Implies(pred);

        let ctxt = TraitSolverCtxt::new(resolver, interner);
        let result = ctxt.evaluate(&goal);
        assert_eq!(
            result,
            GoalEvaluationResult::Yes,
            "S implements Foo → should be Yes"
        );
    }

    /// Stage 17.03 negative 1: Concrete type that does NOT implement trait → No.
    #[test]
    fn stage17_03_goal_evaluation_concrete_type_not_implements() {
        let src = "trait Foo { fn foo(&self); } struct S; fn main() { 0 }";
        let result = compile(src);
        let resolver = &result.trait_resolver;
        let interner = &result.interner;

        let foo_name = interner.get("Foo").expect("Foo should be interned");
        let foo_def_id = resolver
            .find_trait_def_id(foo_name)
            .expect("Foo trait should be registered");

        let mut struct_def_id = None;
        for (def_id, spur) in &resolver.type_by_def_id {
            if interner.resolve(spur) == "S" {
                struct_def_id = Some(*def_id);
                break;
            }
        }
        let s_def_id = struct_def_id.expect("S not found");

        let ty = Ty::new(TyKind::Adt(s_def_id, Vec::new().into()), Span::DUMMY);
        let pred = TraitPredicate::new(ty, foo_def_id);
        let goal = Goal::Implies(pred);

        let ctxt = TraitSolverCtxt::new(resolver, interner);
        let result = ctxt.evaluate(&goal);
        assert_eq!(
            result,
            GoalEvaluationResult::No,
            "S does NOT implement Foo → should be No"
        );
    }

    /// Stage 17.03 negative 2: Type parameter → Ambiguous.
    #[test]
    fn stage17_03_goal_evaluation_type_param_ambiguous() {
        let src = "trait Foo { fn foo(&self); } fn f<T>() { } fn main() { 0 }";
        let result = compile(src);
        let resolver = &result.trait_resolver;
        let interner = &result.interner;

        let foo_name = interner.get("Foo").expect("Foo should be interned");
        let foo_def_id = resolver
            .find_trait_def_id(foo_name)
            .expect("Foo trait should be registered");

        // Create a Param type — "T" is interned from the source.
        let param_name = interner.get("T").expect("T should be interned from source");
        let param = crate::mir::ty::ParamTy {
            index: 0,
            name: param_name,
        };
        let ty = Ty::new(TyKind::Param(param), Span::DUMMY);
        let pred = TraitPredicate::new(ty, foo_def_id);
        let goal = Goal::Implies(pred);

        let ctxt = TraitSolverCtxt::new(resolver, interner);
        let result = ctxt.evaluate(&goal);
        assert_eq!(
            result,
            GoalEvaluationResult::Ambiguous,
            "Type parameter → should be Ambiguous"
        );
    }

    /// Stage 17.03 negative 3: Inference variable → Ambiguous.
    #[test]
    fn stage17_03_goal_evaluation_infer_var_ambiguous() {
        let src = "trait Foo { fn foo(&self); } fn main() { 0 }";
        let result = compile(src);
        let resolver = &result.trait_resolver;
        let interner = &result.interner;

        let foo_name = interner.get("Foo").expect("Foo should be interned");
        let foo_def_id = resolver
            .find_trait_def_id(foo_name)
            .expect("Foo trait should be registered");

        let ty = Ty::new(
            TyKind::Infer(crate::mir::ty::InferVar::TyVar(crate::mir::ty::TyVid(0))),
            Span::DUMMY,
        );
        let pred = TraitPredicate::new(ty, foo_def_id);
        let goal = Goal::Implies(pred);

        let ctxt = TraitSolverCtxt::new(resolver, interner);
        let result = ctxt.evaluate(&goal);
        assert_eq!(
            result,
            GoalEvaluationResult::Ambiguous,
            "Inference variable → should be Ambiguous"
        );
    }

    /// Stage 17.03 negative 4: Error type → Yes (suppressed).
    #[test]
    fn stage17_03_goal_evaluation_error_type_yes() {
        let src = "trait Foo { fn foo(&self); } fn main() { 0 }";
        let result = compile(src);
        let resolver = &result.trait_resolver;
        let interner = &result.interner;

        let foo_name = interner.get("Foo").expect("Foo should be interned");
        let foo_def_id = resolver
            .find_trait_def_id(foo_name)
            .expect("Foo trait should be registered");

        let ty = Ty::new(TyKind::Error, Span::DUMMY);
        let pred = TraitPredicate::new(ty, foo_def_id);
        let goal = Goal::Implies(pred);

        let ctxt = TraitSolverCtxt::new(resolver, interner);
        let result = ctxt.evaluate(&goal);
        assert_eq!(
            result,
            GoalEvaluationResult::Yes,
            "Error type → should be Yes (suppressed)"
        );
    }

    /// Stage 17.03 negative 5: TraitSolverCtxt constructs correctly.
    #[test]
    fn stage17_03_trait_solver_ctxt_new() {
        let src = "fn main() { 0 }";
        let result = compile(src);
        let resolver = &result.trait_resolver;
        let interner = &result.interner;

        let ctxt = TraitSolverCtxt::new(resolver, interner);
        // Just verify it doesn't panic.
        let _ = ctxt.resolver.trait_by_name.len();
    }

    /// Stage 17.03 negative 6: Goal::Implies variant is correct.
    #[test]
    fn stage17_03_goal_implies_variant() {
        let ty = Ty::new(TyKind::Bool, Span::DUMMY);
        let pred = TraitPredicate::new(ty, DefId(1));
        let goal = Goal::Implies(pred.clone());
        match &goal {
            Goal::Implies(p) => {
                assert_eq!(p.trait_def_id, DefId(1));
                assert!(matches!(p.ty.kind, TyKind::Bool));
            }
        }
    }
}
