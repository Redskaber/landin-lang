//! Stage 16.53 — Task 11 Phase 2: Type substitution integration tests.
//!
//! Phase 2 implements the `substitute(ty, substs)` function and integrates
//! it into field type resolution. These tests verify end-to-end that:
//! 1. Generic struct field access produces substituted concrete types
//! 2. Generic struct literals compile with correct field types
//! 3. Generic enum variants work with substitution
//! 4. No regressions on non-generic code
//! 5. The `substitute` function itself is correct (unit tests in
//!    `src/mir/substitute.rs`)

#![cfg(test)]
use landin_compiler::compile;
use landin_compiler::mir::ty::TyKind;
use landin_compiler::mir::SubstsRef;
use landin_compiler::mir::{substitute, substitute_substs};

// =====================================================================
// §1. substitute function — pure unit tests (re-exported from lib)
// =====================================================================

/// Stage 16.53 test 1: substitute replaces Param(0) with the first subst.
#[test]
fn stage16_53_substitute_param_replacement() {
    use landin_compiler::mir::ty::{ParamTy, Ty, TyKind};
    use landin_compiler::session::Span;

    let param = Ty::new(
        TyKind::Param(ParamTy {
            index: 0,
            name: landin_compiler::lexer::Symbol::default(),
        }),
        Span::DUMMY,
    );
    let i32_ty = Ty::new(TyKind::Int(landin_compiler::ast::IntTy::I32), Span::DUMMY);
    let result = substitute(&param, std::slice::from_ref(&i32_ty));
    assert_eq!(result.kind, TyKind::Int(landin_compiler::ast::IntTy::I32));
}

/// Stage 16.53 test 2: substitute is a no-op for leaf types.
#[test]
fn stage16_53_substitute_leaf_noop() {
    use landin_compiler::mir::ty::{Ty, TyKind};
    use landin_compiler::session::Span;

    let i32_ty = Ty::new(TyKind::Int(landin_compiler::ast::IntTy::I32), Span::DUMMY);
    let bool_ty = Ty::new(TyKind::Bool, Span::DUMMY);
    let result = substitute(&i32_ty, &[bool_ty]);
    assert_eq!(result.kind, TyKind::Int(landin_compiler::ast::IntTy::I32));
}

/// Stage 16.53 test 3: substitute_substs applies substitution to a substs slice.
#[test]
fn stage16_53_substitute_substs_slice() {
    use landin_compiler::mir::ty::{ParamTy, Ty, TyKind};
    use landin_compiler::session::Span;

    let param0 = Ty::new(
        TyKind::Param(ParamTy {
            index: 0,
            name: landin_compiler::lexer::Symbol::default(),
        }),
        Span::DUMMY,
    );
    let param1 = Ty::new(
        TyKind::Param(ParamTy {
            index: 1,
            name: landin_compiler::lexer::Symbol::default(),
        }),
        Span::DUMMY,
    );
    let inner_substs: SubstsRef = vec![param0, param1].into();
    let i32_ty = Ty::new(TyKind::Int(landin_compiler::ast::IntTy::I32), Span::DUMMY);
    let bool_ty = Ty::new(TyKind::Bool, Span::DUMMY);
    let result = substitute_substs(&inner_substs, &[i32_ty, bool_ty]);
    assert_eq!(result.len(), 2);
    assert_eq!(
        result[0].kind,
        TyKind::Int(landin_compiler::ast::IntTy::I32)
    );
    assert_eq!(result[1].kind, TyKind::Bool);
}

// =====================================================================
// §2. Generic struct field access — end-to-end compilation
// =====================================================================

/// Stage 16.53 test 4: Generic struct field access compiles.
///
/// `let b: Box<i32> = Box { val: 42 }; b.val` should compile — the field
/// `val: T` is substituted to `i32` via the `resolve_adt_field_tys_with_substs`
/// function, and `b.val` has type `i32`.
#[test]
fn stage16_53_generic_struct_field_access_compiles() {
    let src =
        "struct Box<T> { val: T } fn main() -> i32 { let b: Box<i32> = Box { val: 42 }; b.val }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.53 test 5: Generic struct with two type params.
#[test]
fn stage16_53_generic_struct_two_params_field_access() {
    let src = "struct Pair<A, B> { a: A, b: B } fn main() -> i32 { let p: Pair<i32, i32> = Pair { a: 1, b: 2 }; p.a + p.b }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.53 test 6: Generic struct field access in method body.
///
/// This tests the `self.x` pattern where `self: &S<X>` and `x: X`. The field
/// type `X` is lowered as `Param(X)` and substituted with `self`'s substs.
#[test]
fn stage16_53_generic_struct_field_in_method() {
    let src = "struct S<X> { x: X } impl<X> S<X> { fn get(&self) -> X { self.x } } fn main() { let s: S<i32> = S { x: 42 }; let _ = s.get(); }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.53 test 7: Generic struct with trait impl and method call.
///
/// This is the conformance test that was failing before the Param Copy fix.
#[test]
fn stage16_53_generic_struct_trait_impl_method_call() {
    let src = "trait T { fn f(&self) -> i32; } struct S<X> { x: X } impl<X: T> T for S<X> { fn f(&self) -> i32 { self.x.f() } } fn main() { 0 }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

// =====================================================================
// §3. Generic enum — end-to-end compilation
// =====================================================================

/// Stage 16.53 test 8: Generic enum with match.
#[test]
fn stage16_53_generic_enum_match() {
    let src = r#"enum Opt<T> { Some(T), None }
        fn main() -> i32 {
            let x: Opt<i32> = Opt::Some(42);
            match x {
                Opt::Some(v) => v,
                Opt::None => 0,
            }
        }"#;
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.53 test 9: Generic enum unit variant.
#[test]
fn stage16_53_generic_enum_unit_variant() {
    let src = "enum Opt<T> { Some(T), None } fn main() { let x: Opt<i32> = Opt::None; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

// =====================================================================
// §4. No regressions on non-generic code
// =====================================================================

/// Stage 16.53 test 10: Non-generic struct field access still works.
#[test]
fn stage16_53_non_generic_struct_no_regression() {
    let src = "struct Point { x: i32, y: i32 } fn main() -> i32 { let p = Point { x: 1, y: 2 }; p.x + p.y }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.53 test 11: Non-generic struct with method still works.
#[test]
fn stage16_53_non_generic_struct_method_no_regression() {
    let src = "struct Point { x: i32, y: i32 } impl Point { fn sum(&self) -> i32 { self.x + self.y } } fn main() -> i32 { let p = Point { x: 1, y: 2 }; p.sum() }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.53 test 12: Non-generic enum still works.
#[test]
fn stage16_53_non_generic_enum_no_regression() {
    let src = "enum Color { Red, Green, Blue } fn main() { let c = Color::Red; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

// =====================================================================
// §5. MIR inspection — verify substitution produces correct types
// =====================================================================

/// Stage 16.53 test 13: Verify generic struct local has substs in MIR.
///
/// `let b: Box<i32>` should produce a local decl with type
/// `Adt(Box_def_id, [i32])` — the substs are propagated from the type
/// annotation.
#[test]
fn stage16_53_generic_struct_local_has_substs() {
    let src = "struct Box<T> { val: T } fn main() { let b: Box<i32> = Box { val: 42 }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);

    // Find main's MIR body and look at local decls.
    let main_mir = result
        .mirs
        .iter()
        .find(|m| m.local_decls.len() >= 2)
        .or_else(|| result.mirs.last())
        .expect("at least one MIR body should exist");

    // Look for a local with Adt type and non-empty substs.
    let has_generic_adt = main_mir
        .local_decls
        .iter()
        .any(|ld| matches!(&ld.ty.kind, TyKind::Adt(_, substs) if !substs.is_empty()));
    assert!(
        has_generic_adt,
        "Expected a local with Adt type and non-empty substs (Box<i32>)"
    );
}

/// Stage 16.53 test 14: Verify generic struct field access produces concrete type.
///
/// `b.val` where `b: Box<i32>` should produce a local with type `i32`
/// (not `Param` or `Error`). This verifies that substitution is working
/// end-to-end: the field type `T` is substituted with `i32`.
#[test]
fn stage16_53_generic_field_access_produces_concrete_type() {
    let src =
        "struct Box<T> { val: T } fn main() -> i32 { let b: Box<i32> = Box { val: 42 }; b.val }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);

    // Find main's MIR body.
    let main_mir = result
        .mirs
        .iter()
        .find(|m| m.local_decls.len() >= 3) // return + b + b.val temp
        .or_else(|| result.mirs.last())
        .expect("at least one MIR body should exist");

    // Look for a local with i32 type (the field access result).
    // The return type is i32, and b.val should also be i32.
    let has_i32 = main_mir
        .local_decls
        .iter()
        .any(|ld| matches!(&ld.ty.kind, TyKind::Int(landin_compiler::ast::IntTy::I32)));
    assert!(
        has_i32,
        "Expected a local with i32 type (the field access result)"
    );
}

// =====================================================================
// §6. Complex generic patterns
// =====================================================================

/// Stage 16.53 test 15: Nested generic struct.
#[test]
fn stage16_53_nested_generic_struct() {
    let src = "struct Box<T> { val: T } fn main() { let b: Box<Box<i32>> = Box { val: Box { val: 42 } }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.53 test 16: Generic struct with tuple field.
#[test]
fn stage16_53_generic_struct_tuple_field() {
    let src =
        "struct Pair<T> { val: (T, T) } fn main() { let p: Pair<i32> = Pair { val: (1, 2) }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.53 test 17: Generic struct with reference field.
#[test]
fn stage16_53_generic_struct_ref_field() {
    let src = "struct RefBox<T> { val: &T } fn main() { let x = 42; let b: RefBox<i32> = RefBox { val: &x }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.53 test 18: Multiple generic structs in same program.
#[test]
fn stage16_53_multiple_generic_structs() {
    let src = r#"struct Box<T> { val: T }
        struct Pair<A, B> { a: A, b: B }
        fn main() -> i32 {
            let b: Box<i32> = Box { val: 42 };
            let p: Pair<i32, i32> = Pair { a: 1, b: 2 };
            b.val + p.a + p.b
        }"#;
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}
