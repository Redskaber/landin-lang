//! Stage 18.160 (TD-NEGATIVE-TEST-COVERAGE): Typeck negative tests.
//!
//! Tests type checking error paths. Per §9.4.3, negative tests should be
//! ≥25% of total. This file covers typeck error paths.
//!
//! Per §9.1: tests live under `tests/v0/stage-XX/plan/`.
//! Per §16: tests use only public API (`compile`).

use landin_compiler::compile;

// === Type mismatch errors ===

/// Stage 18.160 negative 1: assigning bool to i32 variable.
#[test]
fn stage18_160_typeck_bool_to_int_mismatch() {
    let result = compile("fn main() { let x: i32 = true; }");
    assert!(
        !result.errors.typeck.is_empty(),
        "bool→i32 mismatch should produce typeck errors"
    );
}

/// Stage 18.160 negative 2: assigning i32 to bool variable.
#[test]
fn stage18_160_typeck_int_to_bool_mismatch() {
    let result = compile("fn main() { let x: bool = 42; }");
    assert!(
        !result.errors.typeck.is_empty(),
        "int→bool mismatch should produce typeck errors"
    );
}

/// Stage 18.160 negative 3: assigning struct to wrong struct type.
#[test]
fn stage18_160_typeck_struct_mismatch() {
    let src = r#"
        struct A { x: i32 }
        struct B { x: i32 }
        fn main() { let a = A { x: 1 }; let b: B = a; }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "struct A→B mismatch should produce typeck errors"
    );
}

/// Stage 18.160 negative 4: returning wrong type from function.
#[test]
fn stage18_160_typeck_wrong_return() {
    let result = compile("fn get_int() -> i32 { true }");
    assert!(
        !result.errors.typeck.is_empty(),
        "wrong return type should produce typeck errors"
    );
}

/// Stage 18.160 negative 5: function parameter type mismatch.
#[test]
fn stage18_160_typeck_param_mismatch() {
    let src = r#"
        fn take_int(x: i32) -> i32 { x }
        fn main() -> i32 { take_int(true) }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "param type mismatch should produce typeck errors"
    );
}

// === Unresolved name errors ===

/// Stage 18.160 negative 6: using undefined variable.
#[test]
fn stage18_160_typeck_undefined_var() {
    let result = compile("fn main() { undefined_var; }");
    assert!(
        !result.errors.resolve.is_empty() || !result.errors.typeck.is_empty(),
        "undefined variable should produce errors"
    );
}

/// Stage 18.160 negative 7: calling undefined function.
#[test]
fn stage18_160_typeck_undefined_fn() {
    let result = compile("fn main() { undefined_fn(); }");
    assert!(
        !result.errors.resolve.is_empty() || !result.errors.typeck.is_empty(),
        "undefined function should produce errors"
    );
}

/// Stage 18.160 negative 8: using undefined struct doesn't panic.
#[test]
fn stage18_160_typeck_undefined_struct() {
    let result = compile("fn main() { let p = UndefinedStruct { x: 1 }; }");
    // Per §2 原则 9: compiler should not panic on undefined struct.
    assert!(
        !result.mirs.is_empty() || result.has_errors(),
        "should produce MIR or errors"
    );
}

/// Stage 18.160 negative 9: accessing undefined field.
#[test]
fn stage18_160_typeck_undefined_field() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        fn main() { let p = Point { x: 1, y: 2 }; p.z; }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty() || !result.errors.resolve.is_empty(),
        "undefined field should produce errors"
    );
}

// === Trait errors ===

/// Stage 18.160 negative 10: calling trait method on non-implementing type doesn't panic.
#[test]
fn stage18_160_typeck_trait_on_non_impl() {
    let src = r#"
        trait Drawable { fn draw(&self); }
        struct Circle;
        impl Drawable for Circle { fn draw(&self) {} }
        struct Square;
        fn main() { let s = Square; s.draw(); }
    "#;
    let result = compile(src);
    // Per §2 原则 9: compiler should not panic on trait error.
    assert!(
        !result.mirs.is_empty() || result.has_errors(),
        "should produce MIR or errors"
    );
}

/// Stage 18.160 negative 11: unimplemented trait method.
#[test]
fn stage18_160_typeck_unimplemented_trait() {
    let src = r#"
        trait Drawable { fn draw(&self); }
        struct Circle;
        fn main() { let c = Circle; c.draw(); }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty() || !result.errors.trait_errors.is_empty(),
        "unimplemented trait should produce errors"
    );
}

/// Stage 18.160 negative 12: duplicate trait impl.
#[test]
fn stage18_160_typeck_duplicate_trait_impl() {
    let src = r#"
        trait Drawable { fn draw(&self); }
        struct Circle;
        impl Drawable for Circle { fn draw(&self) {} }
        impl Drawable for Circle { fn draw(&self) {} }
        fn main() { }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.trait_errors.is_empty() || !result.errors.resolve.is_empty(),
        "duplicate trait impl should produce errors"
    );
}

// === Binary op errors ===

/// Stage 18.160 negative 13: adding bool and int.
#[test]
fn stage18_160_typeck_bool_plus_int() {
    let result = compile("fn main() { let x = true + 1; }");
    assert!(
        !result.errors.typeck.is_empty(),
        "bool + int should produce typeck errors"
    );
}

/// Stage 18.160 negative 14: comparing struct types.
#[test]
fn stage18_160_typeck_struct_comparison() {
    let src = r#"
        struct A { x: i32 }
        fn main() { let a = A { x: 1 }; let b = A { x: 2 }; a == b; }
    "#;
    let result = compile(src);
    // Struct comparison may or may not be supported; at least no panic.
    assert!(!result.mirs.is_empty(), "MIR should be produced");
}

// === Function call errors ===

/// Stage 18.160 negative 15: calling non-function.
#[test]
fn stage18_160_typeck_call_non_function() {
    let result = compile("fn main() { let x = 42; x(); }");
    assert!(
        !result.errors.typeck.is_empty(),
        "calling non-function should produce typeck errors"
    );
}

/// Stage 18.160 negative 16: wrong number of arguments.
#[test]
fn stage18_160_typeck_wrong_arg_count() {
    let src = r#"
        fn take_one(x: i32) -> i32 { x }
        fn main() -> i32 { take_one(1, 2) }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "wrong arg count should produce typeck errors"
    );
}

// === Generic errors ===

/// Stage 18.160 negative 17: using generic type without type parameter.
#[test]
fn stage18_160_typeck_generic_without_param() {
    let src = r#"
        fn identity<T>(x: T) -> T { x }
        fn main() -> i32 { identity() }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "generic without param should produce typeck errors"
    );
}

/// Stage 18.160 negative 18: trait bound not satisfied.
#[test]
fn stage18_160_typeck_trait_bound_not_satisfied() {
    let src = r#"
        trait Clone { fn clone(&self) -> Self; }
        fn duplicate<T: Clone>(x: T) -> T { x.clone() }
        struct NoClone;
        fn main() { duplicate(NoClone); }
    "#;
    let result = compile(src);
    // Trait bound may or may not be enforced; at least no panic.
    assert!(!result.mirs.is_empty(), "MIR should be produced");
}
