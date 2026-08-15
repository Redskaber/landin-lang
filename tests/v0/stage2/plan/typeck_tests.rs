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
        .unify(
            &ty_int(ast::IntTy::I32),
            &ty_int(ast::IntTy::I32),
            landin_compiler::session::Span::DUMMY
        )
        .is_ok());
}

#[test]
fn unify_i32_with_bool_err() {
    let mut t = UnificationTable::new();
    assert!(t
        .unify(
            &ty_int(ast::IntTy::I32),
            &ty_bool(),
            landin_compiler::session::Span::DUMMY
        )
        .is_err());
}

#[test]
fn unify_bool_with_bool_ok() {
    let mut t = UnificationTable::new();
    assert!(t
        .unify(
            &ty_bool(),
            &ty_bool(),
            landin_compiler::session::Span::DUMMY
        )
        .is_ok());
}

#[test]
fn unify_f64_with_f64_ok() {
    let mut t = UnificationTable::new();
    assert!(t
        .unify(
            &ty_float(ast::FloatTy::F64),
            &ty_float(ast::FloatTy::F64),
            landin_compiler::session::Span::DUMMY
        )
        .is_ok());
}

// === Inference variable binding ===

#[test]
fn infer_var_binds_to_concrete() {
    let mut t = UnificationTable::new();
    let vid = t.new_ty_var();
    let var = Ty::new(TyKind::Infer(InferVar::TyVar(vid)), Span::DUMMY);
    assert!(t
        .unify(
            &var,
            &ty_int(ast::IntTy::I64),
            landin_compiler::session::Span::DUMMY
        )
        .is_ok());
    let resolved = t.resolve(&var);
    assert!(matches!(resolved.kind, TyKind::Int(ast::IntTy::I64)));
}

#[test]
fn infer_int_var_binds_to_concrete() {
    let mut t = UnificationTable::new();
    let vid = t.new_int_var();
    let var = Ty::new(TyKind::Infer(InferVar::IntVar(vid)), Span::DUMMY);
    assert!(t
        .unify(
            &var,
            &ty_int(ast::IntTy::I32),
            landin_compiler::session::Span::DUMMY
        )
        .is_ok());
    let resolved = t.resolve(&var);
    assert!(matches!(resolved.kind, TyKind::Int(ast::IntTy::I32)));
}

#[test]
fn infer_float_var_binds_to_f32() {
    let mut t = UnificationTable::new();
    let vid = t.new_float_var();
    let var = Ty::new(TyKind::Infer(InferVar::FloatVar(vid)), Span::DUMMY);
    assert!(t
        .unify(
            &var,
            &ty_float(ast::FloatTy::F32),
            landin_compiler::session::Span::DUMMY
        )
        .is_ok());
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
    assert!(t
        .unify(&var1, &var2, landin_compiler::session::Span::DUMMY)
        .is_ok());
    assert!(t
        .unify(
            &var2,
            &ty_int(ast::IntTy::I32),
            landin_compiler::session::Span::DUMMY
        )
        .is_ok());
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
    t.unify(
        &var,
        &ty_int(ast::IntTy::I64),
        landin_compiler::session::Span::DUMMY,
    )
    .unwrap();
    t.default_unresolved();
    assert_eq!(t.resolve_int_var(vid), Some(ast::IntTy::I64));
}

// === Tuple unification ===

#[test]
fn unify_tuples_same() {
    let mut t = UnificationTable::new();
    let a = ty_tuple(vec![ty_int(ast::IntTy::I32), ty_bool()]);
    let b = ty_tuple(vec![ty_int(ast::IntTy::I32), ty_bool()]);
    assert!(t
        .unify(&a, &b, landin_compiler::session::Span::DUMMY)
        .is_ok());
}

#[test]
fn unify_tuples_different_len_err() {
    let mut t = UnificationTable::new();
    let a = ty_tuple(vec![ty_int(ast::IntTy::I32)]);
    let b = ty_tuple(vec![ty_int(ast::IntTy::I32), ty_bool()]);
    assert!(t
        .unify(&a, &b, landin_compiler::session::Span::DUMMY)
        .is_err());
}

#[test]
fn unify_tuples_with_infer() {
    let mut t = UnificationTable::new();
    let vid = t.new_ty_var();
    let var = Ty::new(TyKind::Infer(InferVar::TyVar(vid)), Span::DUMMY);
    let a = ty_tuple(vec![var.clone(), ty_bool()]);
    let b = ty_tuple(vec![ty_int(ast::IntTy::I32), ty_bool()]);
    assert!(t
        .unify(&a, &b, landin_compiler::session::Span::DUMMY)
        .is_ok());
    let resolved = t.resolve(&var);
    assert!(matches!(resolved.kind, TyKind::Int(ast::IntTy::I32)));
}

// === Never type ===

#[test]
fn never_unifies_with_anything() {
    let mut t = UnificationTable::new();
    let never = Ty::new(TyKind::Never, Span::DUMMY);
    assert!(t
        .unify(&never, &ty_bool(), landin_compiler::session::Span::DUMMY)
        .is_ok());
    assert!(t
        .unify(
            &ty_int(ast::IntTy::I32),
            &never,
            landin_compiler::session::Span::DUMMY
        )
        .is_ok());
}

// === Error propagation ===

#[test]
fn error_type_does_not_propagate() {
    let mut t = UnificationTable::new();
    let err = Ty::new(TyKind::Error, Span::DUMMY);
    assert!(t
        .unify(&err, &ty_bool(), landin_compiler::session::Span::DUMMY)
        .is_ok());
    assert!(t
        .unify(
            &ty_int(ast::IntTy::I32),
            &err,
            landin_compiler::session::Span::DUMMY
        )
        .is_ok());
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
    assert!(t
        .unify(&a, &b, landin_compiler::session::Span::DUMMY)
        .is_ok());
}

#[test]
fn unify_refs_different_mutability_err() {
    // Stage 14.74: &mut T can now be coerced to &T (immutable reborrow).
    // This test previously asserted that unifying Ref(Immut) with Ref(Mut)
    // is an error. Now it's allowed (subtype coercion).
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
    // Stage 14.74: This now succeeds (was: asserted is_err()).
    assert!(t
        .unify(&a, &b, landin_compiler::session::Span::DUMMY)
        .is_ok());
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
    assert!(t
        .unify(&ty1, &ty2, landin_compiler::session::Span::DUMMY)
        .is_ok());
    // Bind one to i64
    assert!(t
        .unify(
            &ty1,
            &ty_int(ast::IntTy::I64),
            landin_compiler::session::Span::DUMMY
        )
        .is_ok());
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
    assert!(t
        .unify(&ty1, &ty2, landin_compiler::session::Span::DUMMY)
        .is_ok());
    assert!(t
        .unify(
            &ty1,
            &ty_float(ast::FloatTy::F32),
            landin_compiler::session::Span::DUMMY
        )
        .is_ok());
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
    assert!(t
        .unify(&ty1, &ty2, landin_compiler::session::Span::DUMMY)
        .is_ok());
    assert!(t
        .unify(&ty2, &ty3, landin_compiler::session::Span::DUMMY)
        .is_ok());
    // Bind v3 to i32 — v1 and v2 should also resolve to i32
    assert!(t
        .unify(
            &ty3,
            &ty_int(ast::IntTy::I32),
            landin_compiler::session::Span::DUMMY
        )
        .is_ok());
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
    assert!(t
        .unify(&ty1, &ty2, landin_compiler::session::Span::DUMMY)
        .is_ok());
    assert!(t
        .unify(&ty2, &ty3, landin_compiler::session::Span::DUMMY)
        .is_ok());
    // Bind v3 to bool — v1 and v2 should also resolve to bool
    assert!(t
        .unify(&ty3, &ty_bool(), landin_compiler::session::Span::DUMMY)
        .is_ok());
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
    let mut parser = Parser::new(tokens, &mut interner);
    let krate = parser.parse_crate();
    assert!(parser.into_errors().is_empty(), "parse errors");
    let mut hir = lower_crate(&krate, &interner).0;
    let _ = resolve_crate(&mut hir, &mut interner);

    let (mut mir, lower_unify, _, _) =
        lower_hir_body_to_mir_full(&hir.bodies[0].1, &interner, &hir, None);
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

// =================================================================
// Stage 18.98: Adt Substs Soundness Tests
// Per §9.4.3: 1 positive + 2 negative (1:2 ratio)
// =================================================================

/// Stage 18.98 positive: `Vec<i32> = Vec<bool>` must be rejected.
/// Per §2.0 原则 9 "正确 > 妥协": soundness — different substs = different types.
#[test]
fn stage18_98_adt_substs_mismatch_rejected() {
    use landin_compiler::compile;
    let src = r#"
struct Vec<T> { data: T, len: i32 }
fn main() {
    let v1: Vec<i32> = Vec { data: 42, len: 1 };
    let v2: Vec<bool> = Vec { data: true, len: 1 };
    let v3: Vec<i32> = v2;
}
"#;
    let result = compile(src);
    assert!(
        result.has_errors(),
        "Vec<i32> = Vec<bool> must be rejected (soundness)"
    );
}

/// Stage 18.98 negative 1: `Vec<i32> = Vec<i32>` still accepted.
#[test]
fn stage18_98_adt_substs_match_accepted() {
    use landin_compiler::compile;
    let src = r#"
struct Vec<T> { data: T, len: i32 }
fn main() {
    let v1: Vec<i32> = Vec { data: 42, len: 1 };
    let v2: Vec<i32> = v1;
}
"#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Vec<i32> = Vec<i32> should be accepted"
    );
}

/// Stage 18.98 negative 2: empty-substs inference still works.
/// `let w: Wrapper<i32> = make(42);` where make<T> returns Wrapper<T>.
/// The rvalue's substs may be empty until inference back-propagates.
#[test]
fn stage18_98_adt_empty_substs_inference() {
    use landin_compiler::compile;
    let src = r#"
struct Wrapper<T> { inner: T }
fn make<T>(x: T) -> Wrapper<T> { Wrapper { inner: x } }
fn main() {
    let w: Wrapper<i32> = make(42);
}
"#;
    let result = compile(src);
    assert!(!result.has_errors(), "empty-substs inference should work");
}

// =================================================================
// Stage 18.99: Deep Review P1 Fixes — Soundness Tests
// =================================================================

/// Stage 18.99 positive: nested Adt substs mismatch must be rejected.
/// Exercises the recursive `types_match_loose` call in the Adt arm
/// (checker.rs:1573-1576). `Vec<Vec<i32>>` vs `Vec<Vec<bool>>` should
/// fail because the inner substs differ ([i32] vs [bool]).
#[test]
fn stage18_99_nested_adt_substs_mismatch_rejected() {
    use landin_compiler::compile;
    let src = r#"
struct Vec<T> { data: T, len: i32 }
fn main() {
    let v1: Vec<Vec<i32>> = Vec { data: Vec { data: 42, len: 1 }, len: 1 };
    let v2: Vec<Vec<bool>> = Vec { data: Vec { data: true, len: 1 }, len: 1 };
    let v3: Vec<Vec<i32>> = v2;
}
"#;
    let result = compile(src);
    assert!(
        result.has_errors(),
        "Vec<Vec<i32>> = Vec<Vec<bool>> must be rejected (nested substs soundness)"
    );
}

/// Stage 18.99 negative: nested Adt substs match accepted.
#[test]
fn stage18_99_nested_adt_substs_match_accepted() {
    use landin_compiler::compile;
    let src = r#"
struct Vec<T> { data: T, len: i32 }
fn main() {
    let v1: Vec<Vec<i32>> = Vec { data: Vec { data: 42, len: 1 }, len: 1 };
    let v2: Vec<Vec<i32>> = v1;
}
"#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Vec<Vec<i32>> = Vec<Vec<i32>> should be accepted"
    );
}

/// Stage 18.99 positive: FnDef↔FnPtr with incompatible sigs must be rejected.
/// `fn(i32) -> i32` function assigned to `fn(bool) -> i32` variable —
/// param types differ, must fail (TD-13 soundness fix).
#[test]
fn stage18_99_fndef_fnptr_sig_mismatch_rejected() {
    use landin_compiler::compile;
    let src = r#"
fn add_one(x: i32) -> i32 { x + 1 }
fn main() {
    let f: fn(bool) -> i32 = add_one;
}
"#;
    let result = compile(src);
    assert!(
        result.has_errors(),
        "fn(i32)->i32 assigned to fn(bool)->i32 must be rejected (TD-13)"
    );
}

/// Stage 18.99 negative: FnDef↔FnPtr with compatible sigs accepted.
#[test]
fn stage18_99_fndef_fnptr_sig_match_accepted() {
    use landin_compiler::compile;
    let src = r#"
fn add_one(x: i32) -> i32 { x + 1 }
fn main() {
    let f: fn(i32) -> i32 = add_one;
}
"#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "fn(i32)->i32 assigned to fn(i32)->i32 should be accepted"
    );
}

// =================================================================
// Stage 18.101: Turbofish Monomorphization Tests
// =================================================================

/// Stage 18.101 positive: turbofish generic call produces MonoItem.
/// `id::<i32>(42)` should produce MonoItem::Fn { def_id: id, substs: [i32] }.
/// This verifies that FnDef substs are propagated from path turbofish args.
#[test]
fn stage18_101_turbofish_produces_mono_item() {
    use landin_compiler::compile;
    use landin_compiler::mir::collect_mono_items;
    let src = r#"
fn id<T>(x: T) -> T { x }
fn main() {
    let a: i32 = id::<i32>(42);
    let b: bool = id::<bool>(true);
}
"#;
    let result = compile(src);
    assert!(!result.has_errors());
    let items = collect_mono_items(&result.mirs);
    // Should collect 2 MonoItems: id<i32> and id<bool>
    let fn_items: Vec<_> = items
        .iter()
        .filter(|i| matches!(i, landin_compiler::mir::MonoItem::Fn { .. }))
        .collect();
    assert_eq!(
        fn_items.len(),
        2,
        "turbofish calls should produce 2 MonoItems, got {}: {:?}",
        fn_items.len(),
        fn_items
    );
}

/// Stage 18.101 negative: non-generic call produces no Fn MonoItems.
#[test]
fn stage18_101_non_generic_no_mono_items() {
    use landin_compiler::compile;
    use landin_compiler::mir::collect_mono_items;
    let src = r#"
fn add(x: i32, y: i32) -> i32 { x + y }
fn main() {
    let a = add(1, 2);
}
"#;
    let result = compile(src);
    assert!(!result.has_errors());
    let items = collect_mono_items(&result.mirs);
    let fn_items: Vec<_> = items
        .iter()
        .filter(|i| matches!(i, landin_compiler::mir::MonoItem::Fn { .. }))
        .collect();
    assert_eq!(
        fn_items.len(),
        0,
        "non-generic call should produce 0 Fn MonoItems, got {}",
        fn_items.len()
    );
}

// =================================================================
// Stage 18.102: Implicit Generic Inference Tests (TD-MONO-INFER)
// =================================================================

/// Stage 18.102 positive: implicit generic call (no turbofish) produces MonoItem.
/// `id(42)` without `::<i32>` should infer T=i32 from the argument type and
/// produce MonoItem::Fn { def_id: id, substs: [i32] }.
/// This verifies the writeback_fndef_substs pass.
#[test]
fn stage18_102_implicit_inference_produces_mono_item() {
    use landin_compiler::compile;
    use landin_compiler::mir::collect_mono_items;
    let src = r#"
fn id<T>(x: T) -> T { x }
fn main() {
    let a: i32 = id(42);
    let b: bool = id(true);
}
"#;
    let result = compile(src);
    assert!(!result.has_errors());
    let items = collect_mono_items(&result.mirs);
    let fn_items: Vec<_> = items
        .iter()
        .filter(|i| matches!(i, landin_compiler::mir::MonoItem::Fn { .. }))
        .collect();
    assert_eq!(
        fn_items.len(),
        2,
        "implicit calls should produce 2 MonoItems (id<i32> + id<bool>), got {}: {:?}",
        fn_items.len(),
        fn_items
    );
}

/// Stage 18.102 negative 1: non-generic implicit call produces no Fn MonoItems.
#[test]
fn stage18_102_non_generic_implicit_no_mono_items() {
    use landin_compiler::compile;
    use landin_compiler::mir::collect_mono_items;
    let src = r#"
fn add(x: i32, y: i32) -> i32 { x + y }
fn main() {
    let a = add(1, 2);
}
"#;
    let result = compile(src);
    assert!(!result.has_errors());
    let items = collect_mono_items(&result.mirs);
    let fn_items: Vec<_> = items
        .iter()
        .filter(|i| matches!(i, landin_compiler::mir::MonoItem::Fn { .. }))
        .collect();
    assert_eq!(
        fn_items.len(),
        0,
        "non-generic call should produce 0 Fn MonoItems, got {}",
        fn_items.len()
    );
}

/// Stage 18.102 negative 2: mixed turbofish + implicit both work.
#[test]
fn stage18_102_mixed_turbofish_and_implicit() {
    use landin_compiler::compile;
    use landin_compiler::mir::collect_mono_items;
    let src = r#"
fn id<T>(x: T) -> T { x }
fn main() {
    let a: i32 = id::<i32>(42);     // turbofish
    let b: bool = id(true);          // implicit
}
"#;
    let result = compile(src);
    assert!(!result.has_errors());
    let items = collect_mono_items(&result.mirs);
    let fn_items: Vec<_> = items
        .iter()
        .filter(|i| matches!(i, landin_compiler::mir::MonoItem::Fn { .. }))
        .collect();
    assert_eq!(
        fn_items.len(),
        2,
        "mixed turbofish + implicit should produce 2 MonoItems, got {}: {:?}",
        fn_items.len(),
        fn_items
    );
}
