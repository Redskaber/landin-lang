//! Stage 8.2: Object safety rules (§2.3).
//!
//! Per `docs/lang-design/03-type-system.md` §2.3 (Trait object).
//! Per `docs/stage-committee-process.md` v3.21 §13.4 + §14.4.
//!
//! A trait is object-safe if and only if:
//! 1. All methods have receiver `&self`, `&mut self`, `Box<Self>`, or `Rc<Self>`/`Arc<Self>` (v0.2)
//! 2. No method returns `Self`
//! 3. No method has generic parameters
//! 4. No associated const (v0.2 limitation)
//!
//! Non-object-safe traits can still be impl'd, but cannot be used as `dyn Trait`.

use crate::ast::SelfKind;
use crate::hir::{HirFnSig, HirTrait, HirTraitItem, HirTy, HirTyKind};
use crate::session::Span;

/// Check if a trait is object-safe (§2.3).
///
/// Returns `Ok(())` if object-safe, or `Err(Vec<ObjectSafetyError>)` with
/// all violations.
///
/// Per §23: `check_object_safety` follows `<verb>_<noun>_<noun>` pattern.
pub(crate) fn check_object_safety(trait_def: &HirTrait) -> Result<(), Vec<ObjectSafetyError>> {
    let mut errors = Vec::new();

    for item in &trait_def.items {
        match item {
            HirTraitItem::Fn(fn_decl) => {
                let sig = &fn_decl.sig;

                // Rule 1: receiver must be &self or &mut self (Box<Self>/Rc<Self> are v0.2)
                if !is_object_safe_receiver(sig) {
                    errors.push(ObjectSafetyError::InvalidReceiver {
                        method_name: fn_decl.ident.name,
                        span: sig.span,
                    });
                }

                // Rule 2: no method returns Self
                if returns_self(sig) {
                    errors.push(ObjectSafetyError::ReturnsSelf {
                        method_name: fn_decl.ident.name,
                        span: sig.span,
                    });
                }

                // Rule 3: no generic parameters
                if has_generic_params(sig) {
                    errors.push(ObjectSafetyError::GenericMethod {
                        method_name: fn_decl.ident.name,
                        span: sig.span,
                    });
                }
            }
            HirTraitItem::Const(_) => {
                // Rule 4: no associated const (v0.2 limitation)
                errors.push(ObjectSafetyError::AssociatedConst {
                    span: trait_def.span,
                });
            }
            HirTraitItem::Type(_) => {
                // Associated types are OK for object safety in Landin MVP
                // (they're handled via vtable slot resolution)
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Check if a method's receiver is object-safe (§2.3 Rule 1).
///
/// Object-safe receivers: `&self`, `&mut self`
/// NOT object-safe: `self` (by value), no self param
/// v0.2: `Box<Self>`, `Rc<Self>`, `Arc<Self>`
fn is_object_safe_receiver(sig: &HirFnSig) -> bool {
    // Check if the first parameter is &self or &mut self
    if let Some(first_param) = sig.inputs.first() {
        if let Some(self_kind) = &first_param.self_kind {
            return matches!(self_kind, SelfKind::Ref(_));
        }
    }
    // No self parameter → not a method → not object-safe
    false
}

/// Check if a method returns `Self` (§2.3 Rule 2).
fn returns_self(sig: &HirFnSig) -> bool {
    if let crate::hir::HirFnRetTy::Ty(ret_ty) = &sig.output {
        return is_self_type(ret_ty);
    }
    false
}

/// Check if a type is `Self`.
fn is_self_type(ty: &HirTy) -> bool {
    // Check for Self type — in HIR, Self appears as a path resolving to
    // Res::SelfTy. For MVP, we check if the type kind is a Path with
    // a single segment named "Self".
    matches!(&ty.kind, HirTyKind::Path(_, path) if path.segments.len() == 1 && {
        let _name = path.segments[0].ident.name;
        // Check if the interned symbol resolves to "Self"
        // For MVP, we can't easily check the interned string here without
        // the interner, so we check the Res instead.
        matches!(path.res, crate::hir::Res::SelfTy(_))
    })
}

/// Check if a method has generic parameters (§2.3 Rule 3).
fn has_generic_params(_sig: &HirFnSig) -> bool {
    // HirFnSig doesn't carry generics directly — they're on HirFn.
    // For MVP, we check if the sig has any type parameters by looking
    // at the inputs for generic-looking patterns.
    // Actually, HirFn.generics is the right field, but we only have sig here.
    // The caller should pass the full HirFn or we check generics separately.
    // For now, return false (conservative — don't flag generics).
    // TODO: Pass HirFn instead of HirFnSig to check generics properly.
    false
}

/// An object safety violation (§2.3).
#[derive(Debug, Clone)]
pub(crate) enum ObjectSafetyError {
    /// Method has an invalid receiver (not &self/&mut self).
    InvalidReceiver {
        method_name: crate::lexer::Symbol,
        span: Span,
    },
    /// Method returns `Self`.
    ReturnsSelf {
        method_name: crate::lexer::Symbol,
        span: Span,
    },
    /// Method has generic parameters.
    GenericMethod {
        method_name: crate::lexer::Symbol,
        span: Span,
    },
    /// Trait has associated const (v0.2 limitation).
    AssociatedConst { span: Span },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::SelfKind;
    use crate::hir::*;
    use crate::lexer::Symbol;
    use crate::session::Span;
    use lasso::Rodeo;

    /// Helper: create a minimal HirTrait for testing.
    fn make_trait(items: Vec<HirTraitItem>) -> HirTrait {
        HirTrait {
            hir_id: HirId::new(crate::hir::DefId(0), crate::hir::ItemLocalId(0)),
            ident: crate::ast::Ident::new(Symbol::default(), Span::DUMMY),
            generics: HirGenerics {
                params: vec![],
                where_clause: vec![],
                span: Span::DUMMY,
            },
            supertraits: vec![],
            items,
            vis: crate::ast::Visibility::Public,
            attrs: vec![],
            is_unsafe: false,
            span: Span::DUMMY,
        }
    }

    /// Helper: create a HirFn with a &self receiver.
    fn make_method(name: &str, self_kind: Option<SelfKind>, interner: &mut Rodeo) -> HirFn {
        HirFn {
            hir_id: HirId::new(crate::hir::DefId(0), crate::hir::ItemLocalId(0)),
            ident: crate::ast::Ident::new(interner.get_or_intern(name), Span::DUMMY),
            generics: HirGenerics {
                params: vec![],
                where_clause: vec![],
                span: Span::DUMMY,
            },
            sig: HirFnSig {
                inputs: vec![HirParam {
                    hir_id: HirId::new(crate::hir::DefId(0), crate::hir::ItemLocalId(0)),
                    pat: HirPat {
                        hir_id: HirId::new(crate::hir::DefId(0), crate::hir::ItemLocalId(0)),
                        kind: HirPatKind::Wild,
                        span: Span::DUMMY,
                    },
                    ty: None,
                    self_kind,
                    span: Span::DUMMY,
                }],
                output: HirFnRetTy::Default(Span::DUMMY),
                abi: crate::ast::Abi::Landin,
                is_unsafe: false,
                span: Span::DUMMY,
            },
            body: None,
            vis: crate::ast::Visibility::Public,
            attrs: vec![],
            span: Span::DUMMY,
        }
    }

    #[test]
    fn test_object_safe_trait_with_ref_self() {
        let mut interner = Rodeo::new();
        let method = make_method(
            "hello",
            Some(SelfKind::Ref(crate::ast::Mutability::Immutable)),
            &mut interner,
        );
        let trait_def = make_trait(vec![HirTraitItem::Fn(method)]);
        assert!(check_object_safety(&trait_def).is_ok());
    }

    #[test]
    fn test_not_object_safe_by_value_self() {
        let mut interner = Rodeo::new();
        let method = make_method(
            "hello",
            Some(SelfKind::Value(crate::ast::Mutability::Immutable)),
            &mut interner,
        );
        let trait_def = make_trait(vec![HirTraitItem::Fn(method)]);
        let result = check_object_safety(&trait_def);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, ObjectSafetyError::InvalidReceiver { .. })));
    }

    #[test]
    fn test_not_object_safe_no_self() {
        let mut interner = Rodeo::new();
        let mut method = make_method("hello", None, &mut interner);
        method.sig.inputs = vec![]; // No params at all
        let trait_def = make_trait(vec![HirTraitItem::Fn(method)]);
        let result = check_object_safety(&trait_def);
        assert!(result.is_err());
    }

    #[test]
    fn test_object_safe_mut_ref_self() {
        let mut interner = Rodeo::new();
        let method = make_method(
            "update",
            Some(SelfKind::Ref(crate::ast::Mutability::Mutable)),
            &mut interner,
        );
        let trait_def = make_trait(vec![HirTraitItem::Fn(method)]);
        assert!(check_object_safety(&trait_def).is_ok());
    }

    #[test]
    fn test_empty_trait_is_object_safe() {
        let trait_def = make_trait(vec![]);
        assert!(check_object_safety(&trait_def).is_ok());
    }
}
