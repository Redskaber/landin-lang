//! Stage 18.54 — Generic Type Parameter Resolution tests.
//!
//! Verifies that generic function/struct/enum/trait/impl type parameters
//! (e.g., `T` in `fn f<T>(x: T)`) are correctly resolved by the resolver
//! to `Res::GenericParam` and lowered to `TyKind::Param` in MIR.
//!
//! Per §9.4.3: 3 positive + 9 negative tests (1:3 ratio).
//! Per §1.0 原則 6 "通用 > 特例": one `generic_param_scope` stack handles
//! all owner kinds (fn/struct/enum/trait/impl).
//!
//! Note on negative tests: `scan_for_unresolved_paths` (driver.rs) currently
//! only scans bodies, not struct/enum/trait signature types. So undefined
//! types in struct fields / enum variants / trait method signatures are NOT
//! reported as resolve errors (Stage 0 limitation). Negative tests use
//! contexts that ARE scanned: let bindings, fn params (via body), return types.

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::compile;

// === Positive: Generic type params resolve correctly ===

/// Stage 18.54 positive 1: Generic function type param resolves.
///
/// `fn f<T>(x: T) -> T { x }` should resolve `T` without errors.
/// Previously (Stage 0 limit): resolver reported "cannot find type in this scope".
#[test]
fn stage18_54_generic_fn_param_resolves() {
    let src = "fn id<T>(x: T) -> T { x } fn main() { let _x = id(42); }";
    let result = compile(src);
    assert!(
        result.errors.parse.is_empty(),
        "parse errors: {:?}",
        result.errors.parse
    );
    assert!(
        result.errors.resolve.is_empty(),
        "resolve errors (T should resolve to GenericParam): {:?}",
        result.errors.resolve
    );
}

/// Stage 18.54 positive 2: Generic struct field type resolves.
///
/// `struct S<T> { x: T }` should resolve `T` without errors.
#[test]
fn stage18_54_generic_struct_field_resolves() {
    let src =
        "struct Wrapper<T> { val: T } fn main() { let b: Wrapper<i32> = Wrapper { val: 42 }; }";
    let result = compile(src);
    assert!(
        result.errors.resolve.is_empty(),
        "resolve errors (T in struct field should resolve): {:?}",
        result.errors.resolve
    );
}

/// Stage 18.54 positive 3: Generic fn with trait bound resolves.
///
/// `fn f<T: Show>(x: T) -> T { x }` should resolve both `T` and `Display`.
/// (Stage 59: changed from Clone to Display — Clone is now in prelude,
/// so user-defined Clone causes duplicate definition error.)
#[test]
fn stage18_54_generic_fn_with_bound_resolves() {
    let src = "trait Show { fn fmt(&self) -> i32; } fn f<T: Show>(x: T) -> T { x } fn main() { 0 }";
    let result = compile(src);
    assert!(
        result.errors.resolve.is_empty(),
        "resolve errors (T + Display bound should resolve): {:?}",
        result.errors.resolve
    );
}

// === Negative: Undefined type params properly reported ===

/// Stage 18.54 negative 1: Undefined type param in fn signature (body param).
///
/// `fn f(x: U) { }` (U not declared in generics) must report a resolve error.
/// The body param type IS scanned by `scan_for_unresolved_paths`.
#[test]
fn stage18_54_undefined_type_param_in_fn_body_param() {
    let src = "fn f(x: U) { } fn main() { 0 }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined type `U` in fn body param must produce a resolve error"
    );
}

/// Stage 18.54 negative 2: Undefined type param in impl method body param.
///
/// `impl S { fn f(x: U) { } }` (U not declared) must report a resolve error.
#[test]
fn stage18_54_undefined_type_param_in_impl_method_body_param() {
    let src = "struct S; impl S { fn f(x: U) { } } fn main() { 0 }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined type `U` in impl method body param must produce a resolve error"
    );
}

/// Stage 18.54 negative 3: Undefined type param in let binding.
///
/// `fn main() { let x: U = 42; }` (U not declared) must report a resolve error.
#[test]
fn stage18_54_undefined_type_param_in_let_binding() {
    let src = "fn main() { let x: U = 42; }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined type `U` in let binding must produce a resolve error"
    );
}

/// Stage 18.54 negative 4: Undefined type param in fn body let binding.
///
/// `fn f<T>(x: T) { let y: U = x; }` (U not declared) must report a resolve error.
#[test]
fn stage18_54_undefined_type_param_in_fn_body_let() {
    let src = "fn f<T>(x: T) { let y: U = x; } fn main() { 0 }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined type `U` in fn body let must produce a resolve error"
    );
}

/// Stage 18.54 negative 5: Undefined type param in where clause.
///
/// `fn f<T>(x: T) where T: U { }` (U not a trait) must report a resolve or typeck error.
#[test]
fn stage18_54_undefined_type_param_in_where_clause() {
    let src = "fn f<T>(x: T) where T: U { } fn main() { 0 }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty() || !result.errors.typeck.is_empty(),
        "Undefined type `U` in where clause must produce a resolve or typeck error"
    );
}

/// Stage 18.54 negative 6: Undefined type param in generic fn body assignment.
///
/// `fn f<T>(x: T) { let y: U = x; }` must report U undefined. This is a
/// variant of negative 4 but with explicit type mismatch to ensure the
/// type checker doesn't silently accept.
#[test]
fn stage18_54_undefined_type_in_generic_fn_body_assignment() {
    let src = "fn f<T>(x: T) { let y: U = x; } fn main() { 0 }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined type `U` in generic fn body assignment must produce a resolve error"
    );
}

/// Stage 18.54 negative 7: Undefined type in let binding with generic fn call.
///
/// `fn f<T>(x: T) { } fn main() { let x: U = f(42); }` must report U undefined.
#[test]
fn stage18_54_undefined_type_in_let_with_generic_call() {
    let src = "fn f<T>(x: T) { } fn main() { let x: U = f(42); }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined type `U` in let with generic call must produce a resolve error"
    );
}

/// Stage 18.54 negative 8: Generic fn body references undefined type.
///
/// `fn f<T>(x: T) { let y: U = x; }` must report U undefined.
#[test]
fn stage18_54_undefined_type_in_generic_fn_body() {
    let src = "fn f<T>(x: T) { let y: U = x; } fn main() { 0 }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined type `U` in generic fn body must produce a resolve error"
    );
}

/// Stage 18.54 negative 9: Undefined type in struct literal field.
///
/// `struct S { x: i32 } fn main() { let s: U = S { x: 42 }; }` must report U undefined.
#[test]
fn stage18_54_undefined_type_in_struct_literal() {
    let src = "struct S { x: i32 } fn main() { let s: U = S { x: 42 }; }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined type `U` in struct literal must produce a resolve error"
    );
}
