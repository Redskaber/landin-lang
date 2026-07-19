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

// === Stage 2.4c: Union-find propagation regression tests ===
// These test that binding one variable propagates to all variables
// unified with it (the "shallow merge" bug from gate review P0-9).

#[test]
fn unify_two_int_vars_propagates() {
    let mut t = UnificationTable::new();
    let v1 = t.new_int_var();
    let v2 = t.new_int_var();
    let ty1 = Ty::new(TyKind::Infer(InferVar::IntVar(v1)), Span::DUMMY);
    let ty2 = Ty::new(TyKind::Infer(InferVar::IntVar(v2)), Span::DUMMY);
    // Unify two unbound int vars — should create a link
    assert!(t.unify(&ty1, &ty2).is_ok());
    // Bind one to i64
    assert!(t.unify(&ty1, &ty_int(ast::IntTy::I64)).is_ok());
    // The other should resolve to i64 via the link
    let resolved = t.resolve(&ty2);
    assert!(
        matches!(resolved.kind, TyKind::Int(ast::IntTy::I64)),
        "expected i64, got {:?}",
        resolved.kind
    );
}

#[test]
fn unify_two_float_vars_propagates() {
    let mut t = UnificationTable::new();
    let v1 = t.new_float_var();
    let v2 = t.new_float_var();
    let ty1 = Ty::new(TyKind::Infer(InferVar::FloatVar(v1)), Span::DUMMY);
    let ty2 = Ty::new(TyKind::Infer(InferVar::FloatVar(v2)), Span::DUMMY);
    assert!(t.unify(&ty1, &ty2).is_ok());
    assert!(t.unify(&ty1, &ty_float(ast::FloatTy::F32)).is_ok());
    let resolved = t.resolve(&ty2);
    assert!(
        matches!(resolved.kind, TyKind::Float(ast::FloatTy::F32)),
        "expected f32, got {:?}",
        resolved.kind
    );
}

#[test]
fn unify_int_var_chain_propagates() {
    let mut t = UnificationTable::new();
    let v1 = t.new_int_var();
    let v2 = t.new_int_var();
    let v3 = t.new_int_var();
    let ty1 = Ty::new(TyKind::Infer(InferVar::IntVar(v1)), Span::DUMMY);
    let ty2 = Ty::new(TyKind::Infer(InferVar::IntVar(v2)), Span::DUMMY);
    let ty3 = Ty::new(TyKind::Infer(InferVar::IntVar(v3)), Span::DUMMY);
    // Chain: v1 ~ v2 ~ v3
    assert!(t.unify(&ty1, &ty2).is_ok());
    assert!(t.unify(&ty2, &ty3).is_ok());
    // Bind v3 to i32 — v1 and v2 should also resolve to i32
    assert!(t.unify(&ty3, &ty_int(ast::IntTy::I32)).is_ok());
    assert!(matches!(t.resolve(&ty1).kind, TyKind::Int(ast::IntTy::I32)));
    assert!(matches!(t.resolve(&ty2).kind, TyKind::Int(ast::IntTy::I32)));
}

#[test]
fn unify_ty_var_chain_propagates() {
    let mut t = UnificationTable::new();
    let v1 = t.new_ty_var();
    let v2 = t.new_ty_var();
    let v3 = t.new_ty_var();
    let ty1 = Ty::new(TyKind::Infer(InferVar::TyVar(v1)), Span::DUMMY);
    let ty2 = Ty::new(TyKind::Infer(InferVar::TyVar(v2)), Span::DUMMY);
    let ty3 = Ty::new(TyKind::Infer(InferVar::TyVar(v3)), Span::DUMMY);
    // Chain: v1 ~ v2 ~ v3
    assert!(t.unify(&ty1, &ty2).is_ok());
    assert!(t.unify(&ty2, &ty3).is_ok());
    // Bind v3 to bool — v1 and v2 should also resolve to bool
    assert!(t.unify(&ty3, &ty_bool()).is_ok());
    assert!(matches!(t.resolve(&ty1).kind, TyKind::Bool));
    assert!(matches!(t.resolve(&ty2).kind, TyKind::Bool));
}

// === Stage 2.4c: Type writeback regression tests ===
// Verify that resolved types are written back to local_decls.

use landin_compiler::hir::lower::lower_crate;
use landin_compiler::lexer::tokenize;
use landin_compiler::mir::body::MirBody;
use landin_compiler::mir::lower::lower_hir_body_to_mir_full;
use landin_compiler::mir::ty::InferVar;
use landin_compiler::parser::Parser;
use landin_compiler::resolve::resolve_crate;
use lasso::Rodeo;

#[test]
fn type_writeback_resolves_infer_var() {
    // After typeck, the local decl's ty should be the resolved
    // concrete type, not an Infer var.
    let src = "fn f() { let x = 42; }";
    let mut interner = Rodeo::new();
    interner.get_or_intern("Self");
    interner.get_or_intern("self");
    interner.get_or_intern("crate");
    interner.get_or_intern("super");
    let (tokens, _) = tokenize(src, &mut interner);
    let mut parser = Parser::new(tokens, &interner);
    let krate = parser.parse_crate();
    assert!(parser.into_errors().is_empty(), "parse errors");
    let mut hir = lower_crate(&krate, &interner);
    let _ = resolve_crate(&mut hir, &mut interner);

    let (mut mir, lower_unify) = lower_hir_body_to_mir_full(&hir.bodies[0].1, &interner, None);
    let mut tc = landin_compiler::typeck::TypeChecker::with_unify(lower_unify);
    tc.check_mir_body(&mut mir);
    let errors: Vec<_> = tc.into_errors();
    assert!(errors.is_empty(), "typeck errors: {:?}", errors);

    // At least one local should now have a concrete Int type (the
    // temp holding 42). Look for any local with a non-Infer type.
    let has_concrete_int = mir
        .local_decls
        .iter()
        .any(|ld| matches!(&ld.ty.kind, landin_compiler::mir::ty::TyKind::Int(_)));
    assert!(
        has_concrete_int,
        "expected at least one local with concrete Int type after writeback"
    );
}

#[test]
fn type_writeback_defaults_unresolved_int_to_i32() {
    // Locals with unconstrained int inference vars should default to i32.
    let mut mir = MirBody::new(Span::DUMMY);
    let _ = mir.new_block();
    let temp = mir.new_local(
        Ty::new(
            TyKind::Infer(InferVar::IntVar(landin_compiler::mir::ty::IntVid(0))),
            Span::DUMMY,
        ),
        None,
        Span::DUMMY,
    );
    // No statements — the IntVar stays unresolved. typeck should default
    // it to i32.
    // We need to also allocate the IntVar in the unification table.
    // The simplest way: call the type checker manually.
    let mut tc = landin_compiler::typeck::TypeChecker::new();
    // Allocate the IntVar properly so the unification table knows about it.
    let _ = tc.unify.new_int_var(); // IntVid(0)
    tc.check_mir_body(&mut mir);
    let resolved = &mir.local(temp).ty;
    assert!(
        matches!(resolved.kind, TyKind::Int(ast::IntTy::I32)),
        "expected default i32, got {:?}",
        resolved.kind
    );
}
