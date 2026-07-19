//! Type checking tests (Stage 2.2).
//!
//! Verify that the unification engine correctly infers types,
//! detects mismatches, and defaults unresolved variables.

use landin_compiler::ast;
use landin_compiler::mir::ty::*;
use landin_compiler::session::Span;
use landin_compiler::typeck::TypeError;
use landin_compiler::typeck::UnificationTable;

fn ty_int(i: ast::IntTy) -> Ty {
    Ty::new(TyKind::Int(i), Span::DUMMY)
}
fn ty_bool() -> Ty {
    Ty::new(TyKind::Bool, Span::DUMMY)
}
fn ty_float(f: ast::FloatTy) -> Ty {
    Ty::new(TyKind::Float(f), Span::DUMMY)
}
fn ty_tuple(tys: Vec<Ty>) -> Ty {
    Ty::new(TyKind::Tuple(tys), Span::DUMMY)
}

// === Basic unification ===

#[test]
fn unify_i32_with_i32_ok() {
    let mut t = UnificationTable::new();
    assert!(t
        .unify(&ty_int(ast::IntTy::I32), &ty_int(ast::IntTy::I32))
        .is_ok());
}

#[test]
fn unify_i32_with_bool_err() {
    let mut t = UnificationTable::new();
    assert!(t.unify(&ty_int(ast::IntTy::I32), &ty_bool()).is_err());
}

#[test]
fn unify_bool_with_bool_ok() {
    let mut t = UnificationTable::new();
    assert!(t.unify(&ty_bool(), &ty_bool()).is_ok());
}

#[test]
fn unify_f64_with_f64_ok() {
    let mut t = UnificationTable::new();
    assert!(t
        .unify(&ty_float(ast::FloatTy::F64), &ty_float(ast::FloatTy::F64))
        .is_ok());
}

// === Inference variable binding ===

#[test]
fn infer_var_binds_to_concrete() {
    let mut t = UnificationTable::new();
    let vid = t.new_ty_var();
    let var = Ty::new(TyKind::Infer(InferVar::TyVar(vid)), Span::DUMMY);
    assert!(t.unify(&var, &ty_int(ast::IntTy::I64)).is_ok());
    let resolved = t.resolve(&var);
    assert!(matches!(resolved.kind, TyKind::Int(ast::IntTy::I64)));
}

#[test]
fn infer_int_var_binds_to_concrete() {
    let mut t = UnificationTable::new();
    let vid = t.new_int_var();
    let var = Ty::new(TyKind::Infer(InferVar::IntVar(vid)), Span::DUMMY);
    assert!(t.unify(&var, &ty_int(ast::IntTy::I32)).is_ok());
    let resolved = t.resolve(&var);
    assert!(matches!(resolved.kind, TyKind::Int(ast::IntTy::I32)));
}

#[test]
fn infer_float_var_binds_to_f32() {
    let mut t = UnificationTable::new();
    let vid = t.new_float_var();
    let var = Ty::new(TyKind::Infer(InferVar::FloatVar(vid)), Span::DUMMY);
    assert!(t.unify(&var, &ty_float(ast::FloatTy::F32)).is_ok());
    let resolved = t.resolve(&var);
    assert!(matches!(resolved.kind, TyKind::Float(ast::FloatTy::F32)));
}

// === Variable chaining ===

#[test]
fn var_chain_resolves() {
    let mut t = UnificationTable::new();
    let v1 = t.new_ty_var();
    let v2 = t.new_ty_var();
    let var1 = Ty::new(TyKind::Infer(InferVar::TyVar(v1)), Span::DUMMY);
    let var2 = Ty::new(TyKind::Infer(InferVar::TyVar(v2)), Span::DUMMY);
    // var1 → var2 → i32
    assert!(t.unify(&var1, &var2).is_ok());
    assert!(t.unify(&var2, &ty_int(ast::IntTy::I32)).is_ok());
    let resolved = t.resolve(&var1);
    assert!(matches!(resolved.kind, TyKind::Int(ast::IntTy::I32)));
}

// === Defaulting ===

#[test]
fn default_unresolved_int_to_i32() {
    let mut t = UnificationTable::new();
    let vid = t.new_int_var();
    t.default_unresolved();
    assert_eq!(t.resolve_int_var(vid), Some(ast::IntTy::I32));
}

#[test]
fn default_unresolved_float_to_f64() {
    let mut t = UnificationTable::new();
    let vid = t.new_float_var();
    t.default_unresolved();
    assert_eq!(t.resolve_float_var(vid), Some(ast::FloatTy::F64));
}

#[test]
fn default_does_not_override_resolved() {
    let mut t = UnificationTable::new();
    let vid = t.new_int_var();
    let var = Ty::new(TyKind::Infer(InferVar::IntVar(vid)), Span::DUMMY);
    t.unify(&var, &ty_int(ast::IntTy::I64)).unwrap();
    t.default_unresolved();
    assert_eq!(t.resolve_int_var(vid), Some(ast::IntTy::I64));
}

// === Tuple unification ===

#[test]
fn unify_tuples_same() {
    let mut t = UnificationTable::new();
    let a = ty_tuple(vec![ty_int(ast::IntTy::I32), ty_bool()]);
    let b = ty_tuple(vec![ty_int(ast::IntTy::I32), ty_bool()]);
    assert!(t.unify(&a, &b).is_ok());
}

#[test]
fn unify_tuples_different_len_err() {
    let mut t = UnificationTable::new();
    let a = ty_tuple(vec![ty_int(ast::IntTy::I32)]);
    let b = ty_tuple(vec![ty_int(ast::IntTy::I32), ty_bool()]);
    assert!(t.unify(&a, &b).is_err());
}

#[test]
fn unify_tuples_with_infer() {
    let mut t = UnificationTable::new();
    let vid = t.new_ty_var();
    let var = Ty::new(TyKind::Infer(InferVar::TyVar(vid)), Span::DUMMY);
    let a = ty_tuple(vec![var.clone(), ty_bool()]);
    let b = ty_tuple(vec![ty_int(ast::IntTy::I32), ty_bool()]);
    assert!(t.unify(&a, &b).is_ok());
    let resolved = t.resolve(&var);
    assert!(matches!(resolved.kind, TyKind::Int(ast::IntTy::I32)));
}

// === Never type ===

#[test]
fn never_unifies_with_anything() {
    let mut t = UnificationTable::new();
    let never = Ty::new(TyKind::Never, Span::DUMMY);
    assert!(t.unify(&never, &ty_bool()).is_ok());
    assert!(t.unify(&ty_int(ast::IntTy::I32), &never).is_ok());
}

// === Error propagation ===

#[test]
fn error_type_does_not_propagate() {
    let mut t = UnificationTable::new();
    let err = Ty::new(TyKind::Error, Span::DUMMY);
    assert!(t.unify(&err, &ty_bool()).is_ok());
    assert!(t.unify(&ty_int(ast::IntTy::I32), &err).is_ok());
}

// === Ref unification ===

#[test]
fn unify_refs_same_inner() {
    let mut t = UnificationTable::new();
    let a = Ty::new(
        TyKind::Ref(
            Region::Static,
            Mutability::Immutable,
            Box::new(ty_int(ast::IntTy::I32)),
        ),
        Span::DUMMY,
    );
    let b = Ty::new(
        TyKind::Ref(
            Region::Static,
            Mutability::Immutable,
            Box::new(ty_int(ast::IntTy::I32)),
        ),
        Span::DUMMY,
    );
    assert!(t.unify(&a, &b).is_ok());
}

#[test]
fn unify_refs_different_mutability_err() {
    let mut t = UnificationTable::new();
    let a = Ty::new(
        TyKind::Ref(
            Region::Static,
            Mutability::Immutable,
            Box::new(ty_int(ast::IntTy::I32)),
        ),
        Span::DUMMY,
    );
    let b = Ty::new(
        TyKind::Ref(
            Region::Static,
            Mutability::Mutable,
            Box::new(ty_int(ast::IntTy::I32)),
        ),
        Span::DUMMY,
    );
    assert!(t.unify(&a, &b).is_err());
}

// === Resolve chain ===

#[test]
fn resolve_follows_chain() {
    let mut t = UnificationTable::new();
    let v1 = t.new_ty_var();
    let v2 = t.new_ty_var();
    t.bind_ty_var(v1, Ty::new(TyKind::Infer(InferVar::TyVar(v2)), Span::DUMMY));
    t.bind_ty_var(v2, ty_int(ast::IntTy::I32));
    let resolved = t.resolve(&Ty::new(TyKind::Infer(InferVar::TyVar(v1)), Span::DUMMY));
    assert!(matches!(resolved.kind, TyKind::Int(ast::IntTy::I32)));
}

// === Error collection ===

#[test]
fn errors_are_collected() {
    let mut t = UnificationTable::new();
    assert!(!t.has_errors());
    t.push_error(TypeError::new("test error", Span::DUMMY));
    assert!(t.has_errors());
    let errors = t.take_errors();
    assert_eq!(errors.len(), 1);
    assert!(!t.has_errors()); // taken
}
