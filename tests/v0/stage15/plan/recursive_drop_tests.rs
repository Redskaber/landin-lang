//! Stage 15.63 — Recursive drop (fields with Drop) tests.
//!
//! These tests verify that the recursive drop glue works correctly:
//!
//! 1. **Struct without `impl Drop` but with Drop field**: The struct's
//!    drop glue recursively drops the field that needs drop.
//!
//! 2. **Struct with `impl Drop` and Drop field**: The struct's drop glue
//!    calls the user's `Drop::drop` method, then recursively drops the
//!    field.
//!
//! 3. **Deeply nested Drop types**: Three levels of nesting (Outer →
//!    Middle → Inner) where only Inner has `impl Drop`.
//!
//! Per §17.1: tests live under `tests/v0/stage{N}/plan/`.
//! Per §17.5: test names follow `<stage>_<id>_<description>` pattern.

#![cfg(test)]

use landin_compiler::compile;

/// Stage 15.63 test 1: Struct without `impl Drop` but with a Drop field.
///
/// `Outer` has no `impl Drop`, but its field `inner: Inner` has `impl Drop`.
/// `ty_needs_drop(Outer)` returns true (because its field needs drop).
/// `emit_drop_glue_functions` emits `drop_adt_<OuterDefId>` that GEPs to
/// `inner` and calls `drop_adt_<InnerDefId>`.
#[test]
fn stage15_63_recursive_drop_outer_no_drop_inner_drop() {
    let src = r#"
        struct Inner { x: i32 }
        impl Drop for Inner { fn drop(&mut self) {} }
        struct Outer { inner: Inner }
        fn main() -> i32 {
            let o = Outer { inner: Inner { x: 42 } };
            o.inner.x
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Recursive drop (Outer no Drop, Inner Drop) should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.63 test 2: Struct with `impl Drop` AND a Drop field.
///
/// `Outer` has `impl Drop`, and its field `inner: Inner` also has `impl Drop`.
/// The drop glue for `Outer` should:
/// 1. Call `landin_Outer_drop` (user's drop method).
/// 2. Recursively call `drop_adt_<InnerDefId>` on the `inner` field.
#[test]
fn stage15_63_recursive_drop_both_have_drop() {
    let src = r#"
        struct Inner { x: i32 }
        impl Drop for Inner { fn drop(&mut self) {} }
        struct Outer { inner: Inner }
        impl Drop for Outer { fn drop(&mut self) {} }
        fn main() -> i32 {
            let o = Outer { inner: Inner { x: 42 } };
            o.inner.x
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Recursive drop (both have Drop) should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.63 test 3: Deeply nested Drop types (3 levels).
///
/// `Outer` → `Middle` → `Inner`, where only `Inner` has `impl Drop`.
/// `ty_needs_drop` recurses through the fields:
/// - `Outer` needs drop (because `middle` field needs drop).
/// - `Middle` needs drop (because `inner` field needs drop).
/// - `Inner` needs drop (has `impl Drop`).
///
/// The drop glue for `Outer` should GEP to `middle`, call `drop_adt_Middle`,
/// which GEPs to `inner`, calls `drop_adt_Inner`, which calls `landin_Inner_drop`.
#[test]
fn stage15_63_recursive_drop_three_levels() {
    let src = r#"
        struct Inner { x: i32 }
        impl Drop for Inner { fn drop(&mut self) {} }
        struct Middle { inner: Inner }
        struct Outer { middle: Middle }
        fn main() -> i32 {
            let o = Outer { middle: Middle { inner: Inner { x: 42 } } };
            o.middle.inner.x
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Three-level recursive drop should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.63 test 4: Struct with multiple Drop fields.
///
/// `Pair` has two fields, both of Drop types. The drop glue should
/// recursively drop both fields.
#[test]
fn stage15_63_recursive_drop_multiple_drop_fields() {
    let src = r#"
        struct A { x: i32 }
        struct B { y: i32 }
        impl Drop for A { fn drop(&mut self) {} }
        impl Drop for B { fn drop(&mut self) {} }
        struct Pair { a: A, b: B }
        fn main() -> i32 {
            let p = Pair { a: A { x: 1 }, b: B { y: 2 } };
            p.a.x + p.b.y
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Multiple Drop fields should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.63 test 5: Struct with mixed Drop and non-Drop fields.
///
/// `Mixed` has three fields: one Drop, two non-Drop. Only the Drop field
/// should be recursively dropped. The non-Drop fields are skipped.
#[test]
fn stage15_63_recursive_drop_mixed_fields() {
    let src = r#"
        struct DropType { x: i32 }
        impl Drop for DropType { fn drop(&mut self) {} }
        struct Mixed { a: i32, drop_field: DropType, b: i32 }
        fn main() -> i32 {
            let m = Mixed { a: 1, drop_field: DropType { x: 2 }, b: 3 };
            m.a + m.drop_field.x + m.b
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Mixed Drop/non-Drop fields should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.63 test 6: No regression — struct without any Drop types.
///
/// A struct with only primitive fields should not trigger any drop glue.
/// `ty_needs_drop` returns false, so no `Drop` terminators are inserted.
#[test]
fn stage15_63_no_drop_all_primitives_no_regression() {
    let src = r#"
        struct S { a: i32, b: i64, c: bool }
        fn main() -> i32 {
            let s = S { a: 1, b: 2, c: true };
            s.a
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "All-primitive struct should compile cleanly. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.63 test 7: Recursive drop with function returning Drop type.
///
/// A function returns a struct with a Drop field. The caller receives the
/// value and drops it at scope end (recursively dropping the field).
#[test]
fn stage15_63_recursive_drop_function_returns_struct_with_drop_field() {
    let src = r#"
        struct Inner { x: i32 }
        impl Drop for Inner { fn drop(&mut self) {} }
        struct Outer { inner: Inner }
        fn make(v: i32) -> Outer {
            Outer { inner: Inner { x: v } }
        }
        fn main() -> i32 {
            let o = make(42);
            o.inner.x
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Function returning struct with Drop field should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.63 test 8: Recursive drop with explicit self type syntax.
///
/// Tests that the explicit `self: &mut Type` syntax works correctly with
/// recursive drop.
#[test]
fn stage15_63_recursive_drop_explicit_self_type() {
    let src = r#"
        struct Inner { x: i32 }
        impl Drop for Inner {
            fn drop(self: &mut Inner) { let _ = self.x; }
        }
        struct Outer { inner: Inner }
        fn main() -> i32 {
            let o = Outer { inner: Inner { x: 42 } };
            o.inner.x
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Recursive drop with explicit self type should compile. Errors: {:?}",
        result.errors.borrowck
    );
}
