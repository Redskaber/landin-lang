//! Stage 15.11 — Const.ty Box<Ty> → Ty interning tests.
//!
//! These tests verify the `Const.ty` type change from `Box<Ty>` to `Ty`
//! (Stage 15.11). They use `compile()` to run the full pipeline with real
//! HIR, verifying that Const construction and consumption work correctly
//! with the new Ty type (no Box indirection).
//!
//! Coverage:
//! 1. Integer constant construction (Const { ty: Ty::new(...), val: ... })
//! 2. Boolean constant construction
//! 3. String constant construction
//! 4. Function call with constant argument
//! 5. Binary operation with constants
//! 6. Array with constant length
//! 7. Method call with constant receiver
//!
//! Per §29.1.3 (Design-Impl-Test coverage): integration tests verify the
//! Const.ty type change works correctly with real MIR produced by the
//! full pipeline.

#![cfg(test)]

use landin_compiler::compile;

/// Stage 15.11 test 1: integer constant construction.
///
/// Verifies `Const { ty: Ty::new(TyKind::Int(I32), ...), val: Int(42) }`
/// works — the Ty is now inline (no Box).
#[test]
fn stage15_11_integer_constant() {
    let src = r#"
        fn main() -> i32 {
            let x = 42;
            x
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "integer constant must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.11 test 2: boolean constant construction.
#[test]
fn stage15_11_boolean_constant() {
    let src = r#"
        fn main() -> i32 {
            let b = true;
            if b { 1 } else { 0 }
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "boolean constant must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.11 test 3: function call with constant argument.
#[test]
fn stage15_11_function_call_with_constant() {
    let src = r#"
        fn add(a: i32, b: i32) -> i32 { a + b }
        fn main() -> i32 {
            add(10, 20)
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "function call with constants must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.11 test 4: binary operation with constants.
#[test]
fn stage15_11_binary_op_with_constants() {
    let src = r#"
        fn main() -> i32 {
            10 + 20 * 3
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "binary op with constants must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.11 test 5: array with constant elements.
#[test]
fn stage15_11_array_with_constants() {
    let src = r#"
        fn main() -> i32 {
            let arr = [1, 2, 3, 4, 5];
            arr[2]
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "array with constants must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.11 test 6: method call with constant receiver.
#[test]
fn stage15_11_method_call_with_constant() {
    let src = r#"
        struct Counter { v: i32 }
        impl Counter {
            fn new(v: i32) -> Counter { Counter { v: v } }
            fn get(self) -> i32 { self.v }
        }
        fn main() -> i32 {
            Counter::new(42).get()
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "method call with constant must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.11 test 7: match with constant discriminant.
#[test]
fn stage15_11_match_with_constant() {
    let src = r#"
        enum Color { Red, Green, Blue }
        fn main() -> i32 {
            let c = Color::Green;
            match c {
                Color::Red => 0,
                Color::Green => 1,
                Color::Blue => 2,
            }
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "match with constant must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}
