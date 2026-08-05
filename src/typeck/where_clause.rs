//! Stage 16.73: Where clause checking (Phase 1 — trait existence).
//! Stage 16.79: Where clause semantic checking (Phase 2 — concrete type impl verification).
//!
//! This module checks `where` clauses on generic items (fn, struct, enum,
//! trait, impl) to verify that the trait bounds are satisfiable.
//!
//! ## Algorithm
//!
//! For each `HirWherePredicate` (e.g., `T: Clone` in `fn f<T>() where T: Clone`):
//! 1. Resolve the bounded type (e.g., `T` → type param, `S` → concrete DefId)
//! 2. Resolve each bound (e.g., `Clone` → trait DefId)
//! 3. Phase 1: Check if the trait exists (Res::Def vs Res::Unknown)
//! 4. Phase 2: If bounded type is a concrete type (struct/enum), check if
//!    the trait is implemented via `resolver.implements_by_def_ids`
//! 5. If not, emit a typeck error
//!
//! ## Scope
//!
//! - **Concrete types** (struct, enum): full semantic check (Phase 2) ✓
//! - **Type parameters** (T): declarative constraint, not checked (Rust behavior)
//! - **Self type**: deferred (needs trait/impl context)
//! - **Primitive types**: deferred (needs primitive trait impl registration)
//!
//! Per §23: `check_where_clauses` follows `<verb>_<noun>_<noun>` pattern.
//! Per §16: reads HIR + TraitResolver (allowed during driver pre-computation).
//! Per §1.0 原則 4 "报错 > 静默": unsatisfied bounds produce hard errors.
//! Per §1.0 原則 5 "去除兼容思维": type param checking deferred per Rust semantics.

use crate::hir::DefId;
use crate::hir::{
    DefKind, HirCrate, HirItem, HirTraitBound, HirTy, HirTyKind, HirTypeBound, OwnerNode, Res,
};
use lasso::Rodeo;

/// Check all where clauses in a crate.
///
/// Walks all generic items (fn, struct, enum, trait, impl) and verifies
/// that their where clause bounds are satisfiable.
///
/// Per §23: `check_where_clauses` follows `<verb>_<noun>_<noun>` pattern.
pub fn check_where_clauses(
    hir: &HirCrate,
    resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
) -> Vec<crate::typeck::TypeError> {
    let mut errors = Vec::new();

    for (_, owner) in &hir.owners {
        match owner {
            OwnerNode::Item(HirItem::Fn(f)) => {
                check_where_clause_for_generics(
                    &f.generics,
                    &format!("fn {}", interner.try_resolve(&f.ident.name).unwrap_or("fn")),
                    resolver,
                    interner,
                    &mut errors,
                );
            }
            OwnerNode::Item(HirItem::Struct(s)) => {
                check_where_clause_for_generics(
                    &s.generics,
                    &format!(
                        "struct {}",
                        interner.try_resolve(&s.ident.name).unwrap_or("struct")
                    ),
                    resolver,
                    interner,
                    &mut errors,
                );
            }
            OwnerNode::Item(HirItem::Enum(e)) => {
                check_where_clause_for_generics(
                    &e.generics,
                    &format!(
                        "enum {}",
                        interner.try_resolve(&e.ident.name).unwrap_or("enum")
                    ),
                    resolver,
                    interner,
                    &mut errors,
                );
            }
            OwnerNode::Item(HirItem::Trait(t)) => {
                check_where_clause_for_generics(
                    &t.generics,
                    &format!(
                        "trait {}",
                        interner.try_resolve(&t.ident.name).unwrap_or("trait")
                    ),
                    resolver,
                    interner,
                    &mut errors,
                );
            }
            OwnerNode::Item(HirItem::Impl(i)) => {
                check_where_clause_for_generics(
                    &i.generics,
                    "impl block",
                    resolver,
                    interner,
                    &mut errors,
                );
            }
            _ => {}
        }
    }

    errors
}

/// Check where clauses for a single `HirGenerics`.
///
/// Stage 16.73 (Phase 1): Verify trait bounds reference existing traits.
/// Stage 16.79 (Phase 2): For concrete bounded types (struct/enum), verify
/// the type actually implements the trait via `resolver.implements_by_def_ids`.
fn check_where_clause_for_generics(
    generics: &crate::hir::HirGenerics,
    item_name: &str,
    resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    errors: &mut Vec<crate::typeck::TypeError>,
) {
    for pred in &generics.where_clause {
        // Stage 16.79: Resolve the bounded type's DefId (if concrete).
        // Returns None for type params (T), Self, primitives — those are
        // declarative constraints, not checkable assertions.
        let bounded_type_def_id = resolve_bounded_type_def_id(&pred.bounded_ty);

        // Each predicate is `Type: Bound1 + Bound2 + ...`
        // We check that each bound is a valid trait reference.
        for bound in &pred.bounds {
            if let HirTypeBound::Trait(tc) = bound {
                match tc.path.res {
                    Res::Def(trait_def_id, DefKind::Trait) => {
                        // Phase 1: The trait is resolved — the bound is syntactically valid.

                        // Phase 2: If bounded type is concrete, verify implementation.
                        if let Some(type_def_id) = bounded_type_def_id {
                            if !resolver.implements_by_def_ids(trait_def_id, type_def_id) {
                                let type_name = format_hir_ty_name(&pred.bounded_ty, interner);
                                let trait_name = format_trait_name(tc, interner);
                                errors.push(crate::typeck::TypeError::new(
                                    format!(
                                        "where clause error: type `{}` does not implement trait `{}` in {}",
                                        type_name, trait_name, item_name
                                    ),
                                    pred.span,
                                ));
                            }
                        }
                        // If bounded_type_def_id is None (type param T, Self, etc.),
                        // skip — declarative constraint, not checkable.
                    }
                    Res::Def(_, _) => {
                        // The bound resolves to a non-trait definition (e.g., struct).
                        // This is a type error — a where clause bound must be a trait.
                        let trait_name = format_trait_name(tc, interner);
                        errors.push(crate::typeck::TypeError::new(
                            format!(
                                "where clause error: `{}` is not a trait in {}",
                                trait_name, item_name
                            ),
                            pred.span,
                        ));
                    }
                    Res::Unknown | Res::Err => {
                        // Phase 1: Trait not found — emit error.
                        let trait_name = format_trait_name(tc, interner);
                        errors.push(crate::typeck::TypeError::new(
                            format!(
                                "where clause error: trait `{}` not found in {}",
                                trait_name, item_name
                            ),
                            pred.span,
                        ));
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Stage 16.79: Resolve the bounded type in a where clause to a DefId,
/// if it's a concrete type (struct or enum).
///
/// Returns:
/// - `Some(def_id)` for concrete types (struct, enum)
/// - `None` for type parameters (T), Self, primitive types, or unresolvable types
///
/// Per §13.4 J2: single responsibility — type resolution only.
/// Per §1.0 原則 6 "通用 > 特例": one function handles all concrete type cases.
fn resolve_bounded_type_def_id(bounded_ty: &HirTy) -> Option<DefId> {
    if let HirTyKind::Path(_, path) = &bounded_ty.kind {
        if let Res::Def(def_id, DefKind::Struct) | Res::Def(def_id, DefKind::Enum) = path.res {
            return Some(def_id);
        }
    }
    None
}

/// Stage 16.79: Format a `HirTy` as a human-readable name for error messages.
///
/// Per §13.4 J2: single responsibility — name formatting only.
fn format_hir_ty_name(ty: &HirTy, interner: &Rodeo) -> String {
    match &ty.kind {
        HirTyKind::Path(_, path) => path
            .segments
            .last()
            .map(|s| {
                interner
                    .try_resolve(&s.ident.name)
                    .unwrap_or("<unknown>")
                    .to_string()
            })
            .unwrap_or_else(|| "<anonymous>".to_string()),
        HirTyKind::Bool => "bool".to_string(),
        HirTyKind::Char => "char".to_string(),
        HirTyKind::Int(_) => "int".to_string(),
        HirTyKind::Uint(_) => "uint".to_string(),
        HirTyKind::Float(_) => "float".to_string(),
        HirTyKind::Never => "!".to_string(),
        HirTyKind::Tuple(_) => "tuple".to_string(),
        HirTyKind::Array(_, _) => "array".to_string(),
        HirTyKind::Slice(_) => "slice".to_string(),
        HirTyKind::Ref(_, _, _) => "reference".to_string(),
        HirTyKind::Ptr(_, _) => "pointer".to_string(),
        HirTyKind::FnPtr { .. } => "fn pointer".to_string(),
        HirTyKind::TraitObject { .. } => "trait object".to_string(),
        HirTyKind::ImplTrait(_) => "impl trait".to_string(),
        HirTyKind::Infer => "_".to_string(),
    }
}

/// Stage 16.79: Format a trait bound as a human-readable name for error messages.
///
/// Per §13.4 J2: single responsibility — name formatting only.
fn format_trait_name(tc: &HirTraitBound, interner: &Rodeo) -> String {
    tc.path
        .segments
        .last()
        .map(|s| {
            interner
                .try_resolve(&s.ident.name)
                .unwrap_or("<unknown>")
                .to_string()
        })
        .unwrap_or_else(|| "<anonymous trait>".to_string())
}

#[cfg(test)]
mod tests {
    use crate::compile;

    /// Stage 16.73 test 1: Where clause with valid trait compiles.
    #[test]
    fn stage16_73_where_clause_valid_trait() {
        let src =
            "trait Clone { fn clone(&self) -> Self; } fn f<T>() where T: Clone { } fn main() { 0 }";
        let result = compile(src);
        // Should compile — Clone trait exists.
        let has_where_error = result
            .errors
            .typeck
            .iter()
            .any(|e| e.message.contains("not found in where clause"));
        assert!(
            !has_where_error,
            "Valid where clause should not produce error"
        );
    }

    /// Stage 16.73 test 2: Where clause with unknown trait errors.
    #[test]
    fn stage16_73_where_clause_unknown_trait() {
        let src = "fn f<T>() where T: NonExistentTrait { } fn main() { 0 }";
        let result = compile(src);
        let has_where_error = result
            .errors
            .typeck
            .iter()
            .any(|e| e.message.contains("not found"));
        assert!(
            has_where_error,
            "Unknown trait in where clause should produce error"
        );
    }

    /// Stage 16.73 test 3: Where clause on struct compiles.
    #[test]
    fn stage16_73_where_clause_on_struct() {
        let src = "trait Clone { fn clone(&self) -> Self; } struct Foo<T> where T: Clone { x: T } fn main() { 0 }";
        let result = compile(src);
        let has_where_error = result
            .errors
            .typeck
            .iter()
            .any(|e| e.message.contains("not found in where clause"));
        assert!(
            !has_where_error,
            "Valid where clause on struct should not produce error"
        );
    }

    /// Stage 16.73 test 4: No where clause — no error.
    #[test]
    fn stage16_73_no_where_clause() {
        let result = compile("fn main() -> i32 { 42 }");
        assert!(!result.has_errors());
    }

    /// Stage 16.73 test 5: Where clause on impl compiles.
    #[test]
    fn stage16_73_where_clause_on_impl() {
        let src = "trait Foo { fn bar(&self); } struct S<T> { x: T } impl<T> Foo for S<T> where T: Foo { fn bar(&self) {} } fn main() { 0 }";
        let result = compile(src);
        let has_where_error = result
            .errors
            .typeck
            .iter()
            .any(|e| e.message.contains("not found in where clause"));
        assert!(
            !has_where_error,
            "Valid where clause on impl should not produce error"
        );
    }

    // === Stage 16.79 (Where clause Phase 2): Semantic checking tests ===
    // Per §9.4.3: 2 positive + 6 negative tests (1:3 ratio).

    /// Stage 16.79 positive 1: Concrete type that implements trait — no error.
    #[test]
    fn stage16_79_concrete_type_implements_trait() {
        let src = "trait Foo { fn foo(&self); } struct S; impl Foo for S { fn foo(&self) {} } fn f() where S: Foo { } fn main() { 0 }";
        let result = compile(src);
        let has_impl_error = result
            .errors
            .typeck
            .iter()
            .any(|e| e.message.contains("does not implement trait"));
        assert!(
            !has_impl_error,
            "Concrete type that implements trait should not produce error"
        );
    }

    /// Stage 16.79 positive 2: Type parameter T — no error (declarative, not checkable).
    #[test]
    fn stage16_79_type_param_no_error() {
        let src =
            "trait Clone { fn clone(&self) -> Self; } fn f<T>() where T: Clone { } fn main() { 0 }";
        let result = compile(src);
        let has_impl_error = result
            .errors
            .typeck
            .iter()
            .any(|e| e.message.contains("does not implement trait"));
        assert!(
            !has_impl_error,
            "Type parameter where clause should not produce impl error"
        );
    }

    /// Stage 16.79 negative 1: Concrete struct does not implement trait — error.
    #[test]
    fn stage16_79_concrete_struct_does_not_implement() {
        let src = "trait Foo { fn foo(&self); } struct S; fn f() where S: Foo { } fn main() { 0 }";
        let result = compile(src);
        let has_impl_error = result
            .errors
            .typeck
            .iter()
            .any(|e| e.message.contains("does not implement trait") && e.message.contains("S"));
        assert!(
            has_impl_error,
            "Concrete struct not implementing trait should produce error, got errors: {:?}",
            result.errors.typeck
        );
    }

    /// Stage 16.79 negative 2: Concrete enum does not implement trait — error.
    #[test]
    fn stage16_79_concrete_enum_does_not_implement() {
        let src =
            "trait Foo { fn foo(&self); } enum E { A, B } fn f() where E: Foo { } fn main() { 0 }";
        let result = compile(src);
        let has_impl_error = result
            .errors
            .typeck
            .iter()
            .any(|e| e.message.contains("does not implement trait") && e.message.contains("E"));
        assert!(
            has_impl_error,
            "Concrete enum not implementing trait should produce error"
        );
    }

    /// Stage 16.79 negative 3: Multiple bounds, one unsatisfied — error.
    #[test]
    fn stage16_79_multiple_bounds_one_unsatisfied() {
        let src = "trait Foo { fn foo(&self); } trait Bar { fn bar(&self); } struct S; impl Foo for S { fn foo(&self) {} } fn f() where S: Foo + Bar { } fn main() { 0 }";
        let result = compile(src);
        let has_bar_error =
            result.errors.typeck.iter().any(|e| {
                e.message.contains("does not implement trait") && e.message.contains("Bar")
            });
        assert!(
            has_bar_error,
            "S does not implement Bar, should produce error for Bar bound"
        );
    }

    /// Stage 16.79 negative 4: Struct with where clause on different struct — error.
    #[test]
    fn stage16_79_where_clause_on_other_struct() {
        let src = "trait Foo { fn foo(&self); } struct A; struct B; fn f() where A: Foo { } fn main() { 0 }";
        let result = compile(src);
        let has_impl_error = result
            .errors
            .typeck
            .iter()
            .any(|e| e.message.contains("does not implement trait") && e.message.contains("A"));
        assert!(
            has_impl_error,
            "A does not implement Foo, should produce error"
        );
    }

    /// Stage 16.79 negative 5: Phase 1 regression — trait not found still errors.
    #[test]
    fn stage16_79_trait_not_found_phase1_regression() {
        let src = "struct S; fn f() where S: NonExistentTrait { } fn main() { 0 }";
        let result = compile(src);
        let has_not_found = result
            .errors
            .typeck
            .iter()
            .any(|e| e.message.contains("not found"));
        assert!(
            has_not_found,
            "Non-existent trait should still produce 'not found' error"
        );
    }

    /// Stage 16.79 negative 6: Multiple where predicates, one fails — error.
    #[test]
    fn stage16_79_multiple_where_preds_one_fails() {
        let src = "trait Foo { fn foo(&self); } trait Bar { fn bar(&self); } struct S; impl Foo for S { fn foo(&self) {} } fn f() where S: Foo, S: Bar { } fn main() { 0 }";
        let result = compile(src);
        let has_bar_error =
            result.errors.typeck.iter().any(|e| {
                e.message.contains("does not implement trait") && e.message.contains("Bar")
            });
        assert!(
            has_bar_error,
            "S does not implement Bar, should produce error for second where predicate"
        );
    }
}
