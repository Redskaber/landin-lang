//! Stage 16.64 (Task 14 Phase 1): Object safety checking.
//!
//! This module checks whether a trait is object-safe — i.e., whether `dyn Trait`
//! can be used for it. A trait is NOT object-safe if any of its methods:
//!
//! 1. Returns `Self` (the trait's Self type)
//! 2. Has `Self` in any argument position
//! 3. Has generic type parameters (e.g., `fn f<T>(&self, x: T)`)
//! 4. Has no receiver (`self`, `&self`, or `&mut self`) — associated functions
//!    are not callable through `dyn Trait`
//! 5. Has a by-value receiver (`self` or `mut self`) — only `&self` and
//!    `&mut self` are object-safe
//!
//! Per Rust RFC #255, these rules ensure that all methods can be dispatched
//! through a vtable when the concrete type is unknown (erased to `dyn Trait`).
//!
//! Per §23: `check_trait_object_safety` follows `<verb>_<noun>_<noun>_<noun>`
//! pattern.
//! Per §16: reads HIR (trait definition) — allowed during driver setup.
//! Per §1.0 原則 5 "报错 > 静默": non-object-safe traits produce hard errors
//! when used as `dyn Trait`.

use crate::ast::SelfKind;
use crate::hir::{
    HirFn, HirFnRetTy, HirGenericParam, HirTrait, HirTraitItem, HirTy, HirTyKind, Res,
};
use crate::lexer::Symbol;
use crate::session::Span;

/// An object safety violation found in a trait method.
///
/// Each variant carries the method name and span for error reporting.
///
/// Per §23: `ObjectSafetyViolation` follows `<Noun>_<Noun>_<Noun>` pattern.
#[derive(Debug, Clone)]
pub enum ObjectSafetyViolation {
    /// Method returns `Self` type.
    SelfReturn { method: Symbol, span: Span },
    /// Method has `Self` in an argument type.
    SelfInArg {
        method: Symbol,
        arg_idx: usize,
        span: Span,
    },
    /// Method has generic type parameters.
    GenericMethod { method: Symbol, span: Span },
    /// Method has no receiver (associated function, not a method).
    NoReceiver { method: Symbol, span: Span },
    /// Method has a by-value receiver (`self` or `mut self`).
    ByValueReceiver { method: Symbol, span: Span },
}

impl ObjectSafetyViolation {
    /// Format this violation as a human-readable error message.
    ///
    /// The `interner` is needed to resolve the method name `Symbol` to a string.
    pub fn error_message(&self, trait_name: &str, interner: &lasso::Rodeo) -> String {
        let method_str = |sym: Symbol| -> String {
            interner
                .try_resolve(&sym)
                .unwrap_or("<unknown>")
                .to_string()
        };
        match self {
            ObjectSafetyViolation::SelfReturn { method, .. } => {
                format!(
                    "trait `{}` is not object-safe: method `{}` returns `Self`",
                    trait_name,
                    method_str(*method)
                )
            }
            ObjectSafetyViolation::SelfInArg {
                method, arg_idx, ..
            } => {
                format!(
                    "trait `{}` is not object-safe: method `{}` has `Self` in argument {}",
                    trait_name,
                    method_str(*method),
                    arg_idx
                )
            }
            ObjectSafetyViolation::GenericMethod { method, .. } => {
                format!(
                    "trait `{}` is not object-safe: method `{}` has generic type parameters",
                    trait_name,
                    method_str(*method)
                )
            }
            ObjectSafetyViolation::NoReceiver { method, .. } => {
                format!(
                    "trait `{}` is not object-safe: method `{}` has no receiver (associated functions cannot be called through `dyn Trait`)",
                    trait_name, method_str(*method)
                )
            }
            ObjectSafetyViolation::ByValueReceiver { method, .. } => {
                format!(
                    "trait `{}` is not object-safe: method `{}` takes `self` by value (only `&self` and `&mut self` are object-safe)",
                    trait_name, method_str(*method)
                )
            }
        }
    }

    /// Get the span of this violation for error reporting.
    pub fn span(&self) -> Span {
        match self {
            ObjectSafetyViolation::SelfReturn { span, .. }
            | ObjectSafetyViolation::SelfInArg { span, .. }
            | ObjectSafetyViolation::GenericMethod { span, .. }
            | ObjectSafetyViolation::NoReceiver { span, .. }
            | ObjectSafetyViolation::ByValueReceiver { span, .. } => *span,
        }
    }
}

/// Check whether a trait is object-safe.
///
/// Returns a list of `ObjectSafetyViolation`s. If the list is empty, the
/// trait is object-safe and `dyn Trait` can be used for it.
///
/// Per §23: `check_trait_object_safety` follows `<verb>_<noun>_<noun>_<noun>`
/// pattern.
/// Per §16: reads HIR (trait definition) — allowed during driver setup.
/// Per §1.0 原則 6 "通用 > 特例": one function checks all rules.
pub fn check_trait_object_safety(trait_def: &HirTrait) -> Vec<ObjectSafetyViolation> {
    let mut violations = Vec::new();

    for item in &trait_def.items {
        if let HirTraitItem::Fn(f) = item {
            check_method(f, &mut violations);
        }
        // Associated types and consts don't affect object safety in v0.4.
        // (Rust requires them to have defaults, but that's Task 17 territory.)
    }

    violations
}

/// Check a single trait method for object safety violations.
fn check_method(f: &HirFn, violations: &mut Vec<ObjectSafetyViolation>) {
    let method_name = f.ident.name;
    let method_span = f.span;

    // Rule 1: Generic method — has type parameters
    let has_type_params = f
        .generics
        .params
        .iter()
        .any(|p| matches!(p, HirGenericParam::Type(_)));
    if has_type_params {
        violations.push(ObjectSafetyViolation::GenericMethod {
            method: method_name,
            span: method_span,
        });
        return; // Other rules don't matter if the method is generic
    }

    // Rule 2: No receiver (associated function, not a method)
    let first_param = f.sig.inputs.first();
    let has_receiver = first_param.map(|p| p.self_kind.is_some()).unwrap_or(false);

    if !has_receiver {
        violations.push(ObjectSafetyViolation::NoReceiver {
            method: method_name,
            span: method_span,
        });
        return; // No point checking args/return if there's no receiver
    }

    // Rule 3: By-value receiver (self or mut self)
    if let Some(param) = first_param {
        if let Some(SelfKind::Value(_)) = param.self_kind {
            violations.push(ObjectSafetyViolation::ByValueReceiver {
                method: method_name,
                span: method_span,
            });
            return;
        }
    }

    // Rule 4: Self in return type
    if let HirFnRetTy::Ty(ret_ty) = &f.sig.output {
        if ty_contains_self(ret_ty) {
            violations.push(ObjectSafetyViolation::SelfReturn {
                method: method_name,
                span: method_span,
            });
        }
    }

    // Rule 5: Self in argument types (skip the first param, which is self)
    for (i, param) in f.sig.inputs.iter().skip(1).enumerate() {
        if let Some(arg_ty) = &param.ty {
            if ty_contains_self(arg_ty) {
                violations.push(ObjectSafetyViolation::SelfInArg {
                    method: method_name,
                    arg_idx: i + 1,
                    span: param.span,
                });
            }
        }
    }
}

/// Check whether a `HirTy` contains `Self` (recursively).
///
/// `Self` appears as `HirTyKind::Path(_, path)` where `path.res` is
/// `Res::SelfTy(_)`. This function recursively walks through Ref, Tuple,
/// Array, Slice, FnPtr, TraitObject, ImplTrait, etc.
///
/// Stage 16.71 (Round 10 fix): Added FnPtr, TraitObject, ImplTrait cases
/// that were missing from the original implementation.
fn ty_contains_self(ty: &HirTy) -> bool {
    match &ty.kind {
        HirTyKind::Path(_, path) => {
            matches!(path.res, Res::SelfTy(_))
        }
        HirTyKind::Ref(_, _, inner) => ty_contains_self(inner),
        HirTyKind::Ptr(_, inner) => ty_contains_self(inner),
        HirTyKind::Slice(inner) => ty_contains_self(inner),
        HirTyKind::Array(inner, _) => ty_contains_self(inner),
        HirTyKind::Tuple(tys) => tys.iter().any(ty_contains_self),
        // Stage 16.71: FnPtr — check inputs and output
        HirTyKind::FnPtr { inputs, output, .. } => {
            inputs.iter().any(ty_contains_self) || ty_contains_self(output)
        }
        // Stage 16.71: TraitObject — check bounds for Self.
        // Note: TraitObject bounds carry AST types (not HIR), so we can't
        // fully check for Self here. The Res::SelfTy check on the trait
        // path itself is the primary check.
        HirTyKind::TraitObject { bounds, .. } => bounds.iter().any(|b| match b {
            crate::hir::HirTypeBound::Trait(tc) => {
                matches!(tc.path.res, Res::SelfTy(_))
            }
            _ => false,
        }),
        // Stage 16.71: ImplTrait — check bounds
        HirTyKind::ImplTrait(bounds) => {
            // impl Trait can't contain Self in Rust (it's an opaque type),
            // but we check conservatively.
            bounds.iter().any(|b| {
                if let crate::hir::HirTypeBound::Trait(tc) = b {
                    matches!(tc.path.res, Res::SelfTy(_))
                } else {
                    false
                }
            })
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile;

    /// Stage 16.64 test 1: Object-safe trait (no violations).
    #[test]
    fn stage16_64_safe_trait_no_violations() {
        let src = "trait Foo { fn bar(&self) -> i32; } fn main() { 0 }";
        let result = compile(src);
        let hir = result.hir.as_ref().expect("HIR should be available");
        for (_, owner) in &hir.owners {
            if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
                let violations = check_trait_object_safety(t);
                assert!(
                    violations.is_empty(),
                    "Expected no violations, got: {:?}",
                    violations
                );
                return;
            }
        }
        panic!("No trait found in HIR");
    }

    /// Stage 16.64 test 2: Trait with Self return is not object-safe.
    #[test]
    fn stage16_64_self_return_not_safe() {
        let src = "trait Foo { fn bar(&self) -> Self; } fn main() { 0 }";
        let result = compile(src);
        let hir = result.hir.as_ref().expect("HIR should be available");
        for (_, owner) in &hir.owners {
            if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
                let violations = check_trait_object_safety(t);
                assert!(!violations.is_empty(), "Expected SelfReturn violation");
                assert!(violations
                    .iter()
                    .any(|v| matches!(v, ObjectSafetyViolation::SelfReturn { .. })));
                return;
            }
        }
        panic!("No trait found in HIR");
    }

    /// Stage 16.64 test 3: Trait with generic method is not object-safe.
    #[test]
    fn stage16_64_generic_method_not_safe() {
        let src = "trait Foo { fn bar<T>(&self, x: T); } fn main() { 0 }";
        let result = compile(src);
        let hir = result.hir.as_ref().expect("HIR should be available");
        for (_, owner) in &hir.owners {
            if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
                let violations = check_trait_object_safety(t);
                assert!(!violations.is_empty(), "Expected GenericMethod violation");
                assert!(violations
                    .iter()
                    .any(|v| matches!(v, ObjectSafetyViolation::GenericMethod { .. })));
                return;
            }
        }
        panic!("No trait found in HIR");
    }

    /// Stage 16.64 test 4: Trait with no-receiver method is not object-safe.
    #[test]
    fn stage16_64_no_receiver_not_safe() {
        let src = "trait Foo { fn bar() -> i32; } fn main() { 0 }";
        let result = compile(src);
        let hir = result.hir.as_ref().expect("HIR should be available");
        for (_, owner) in &hir.owners {
            if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
                let violations = check_trait_object_safety(t);
                assert!(!violations.is_empty(), "Expected NoReceiver violation");
                assert!(violations
                    .iter()
                    .any(|v| matches!(v, ObjectSafetyViolation::NoReceiver { .. })));
                return;
            }
        }
        panic!("No trait found in HIR");
    }

    /// Stage 16.64 test 5: Trait with by-value receiver is not object-safe.
    #[test]
    fn stage16_64_by_value_receiver_not_safe() {
        let src = "trait Foo { fn bar(self) -> i32; } fn main() { 0 }";
        let result = compile(src);
        let hir = result.hir.as_ref().expect("HIR should be available");
        for (_, owner) in &hir.owners {
            if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
                let violations = check_trait_object_safety(t);
                assert!(!violations.is_empty(), "Expected ByValueReceiver violation");
                assert!(violations
                    .iter()
                    .any(|v| matches!(v, ObjectSafetyViolation::ByValueReceiver { .. })));
                return;
            }
        }
        panic!("No trait found in HIR");
    }

    /// Stage 16.64 test 6: Trait with Self in argument is not object-safe.
    #[test]
    fn stage16_64_self_in_arg_not_safe() {
        let src = "trait Foo { fn bar(&self, x: Self); } fn main() { 0 }";
        let result = compile(src);
        let hir = result.hir.as_ref().expect("HIR should be available");
        for (_, owner) in &hir.owners {
            if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
                let violations = check_trait_object_safety(t);
                assert!(!violations.is_empty(), "Expected SelfInArg violation");
                assert!(violations
                    .iter()
                    .any(|v| matches!(v, ObjectSafetyViolation::SelfInArg { .. })));
                return;
            }
        }
        panic!("No trait found in HIR");
    }

    /// Stage 16.64 test 7: Empty trait is object-safe.
    #[test]
    fn stage16_64_empty_trait_safe() {
        let src = "trait Foo {} fn main() { 0 }";
        let result = compile(src);
        let hir = result.hir.as_ref().expect("HIR should be available");
        for (_, owner) in &hir.owners {
            if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
                let violations = check_trait_object_safety(t);
                assert!(violations.is_empty(), "Empty trait should be object-safe");
                return;
            }
        }
        panic!("No trait found in HIR");
    }

    /// Stage 16.64 test 8: &mut self is object-safe.
    #[test]
    fn stage16_64_ref_mut_self_safe() {
        let src = "trait Foo { fn bar(&mut self); } fn main() { 0 }";
        let result = compile(src);
        let hir = result.hir.as_ref().expect("HIR should be available");
        for (_, owner) in &hir.owners {
            if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
                let violations = check_trait_object_safety(t);
                assert!(violations.is_empty(), "&mut self should be object-safe");
                return;
            }
        }
        panic!("No trait found in HIR");
    }

    /// Stage 16.64 test 9: Self in return via Ref is not object-safe.
    #[test]
    fn stage16_64_self_in_ref_return_not_safe() {
        let src = "trait Foo { fn bar(&self) -> &Self; } fn main() { 0 }";
        let result = compile(src);
        let hir = result.hir.as_ref().expect("HIR should be available");
        for (_, owner) in &hir.owners {
            if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
                let violations = check_trait_object_safety(t);
                assert!(
                    !violations.is_empty(),
                    "&Self return should not be object-safe"
                );
                assert!(violations
                    .iter()
                    .any(|v| matches!(v, ObjectSafetyViolation::SelfReturn { .. })));
                return;
            }
        }
        panic!("No trait found in HIR");
    }

    /// Stage 16.64 test 10: Multiple violations are all reported.
    #[test]
    fn stage16_64_multiple_violations() {
        let src = "trait Foo { fn bar(&self) -> Self; fn baz<T>(&self, x: T); fn qux() -> i32; } fn main() { 0 }";
        let result = compile(src);
        let hir = result.hir.as_ref().expect("HIR should be available");
        for (_, owner) in &hir.owners {
            if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
                let violations = check_trait_object_safety(t);
                assert_eq!(
                    violations.len(),
                    3,
                    "Expected 3 violations, got: {}",
                    violations.len()
                );
                return;
            }
        }
        panic!("No trait found in HIR");
    }
}
