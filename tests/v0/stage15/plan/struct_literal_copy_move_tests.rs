//! Stage 15.64 — Struct literal Copy→Move + field-copy drop prevention tests.
//!
//! These tests verify two fixes:
//!
//! 1. **Struct literal Copy→Move**: When a struct literal has a field whose
//!    type is non-Copy (e.g., a struct with `impl Drop`), the field value
//!    is now moved (not copied) into the struct. This prevents the field's
//!    temporary from being double-dropped.
//!
//! 2. **Field-copy drop prevention**: When a field of a struct is accessed
//!    (e.g., `o.inner`), the intermediate temp that holds the field value
//!    is NOT dropped at scope end — the original struct owns the field and
//!    will drop it via recursive drop glue.
//!
//! Per §17.1: tests live under `tests/v0/stage{N}/plan/`.
//! Per §17.5: test names follow `<stage>_<id>_<description>` pattern.

#![cfg(test)]

use landin_compiler::compile;

/// Stage 15.64 test 1: Struct literal with non-Copy field — no double-drop.
///
/// `Outer { inner: Inner { x: 42 } }` — the Inner temp is moved (not copied)
/// into Outer. At scope end, only Outer is dropped (which recursively drops
/// the inner field). No extra "inner dropped" from the temp.
#[test]
fn stage15_64_struct_literal_non_copy_field_no_double_drop() {
    let src = r#"
        trait Drop { fn drop(&mut self); }
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
        "Struct literal with non-Copy field should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.64 test 2: Field access on Drop struct — no double-drop.
///
/// `o.inner.x` — accessing `o.inner` creates an intermediate temp of type
/// Inner (non-Copy). This temp is NOT dropped at scope end — the original
/// `o` owns the field and drops it via recursive drop glue.
#[test]
fn stage15_64_field_access_no_double_drop() {
    let src = r#"
        trait Drop { fn drop(&mut self); }
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
        "Field access on Drop struct should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.64 test 3: Nested struct literals — all temps moved, no double-drop.
///
/// Three levels of struct literals: `Outer { middle: Middle { inner: Inner { x: 42 } } }`.
/// Each level's temp is moved into the next. No double-drops.
#[test]
fn stage15_64_nested_struct_literals_no_double_drop() {
    let src = r#"
        trait Drop { fn drop(&mut self); }
        struct Inner { x: i32 }
        impl Drop for Inner { fn drop(&mut self) {} }
        struct Middle { inner: Inner }
        impl Drop for Middle { fn drop(&mut self) {} }
        struct Outer { middle: Middle }
        impl Drop for Outer { fn drop(&mut self) {} }
        fn main() -> i32 {
            let o = Outer { middle: Middle { inner: Inner { x: 42 } } };
            o.middle.inner.x
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Nested struct literals should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.64 test 4: Struct literal with Copy fields — no regression.
///
/// `Point { x: 1, y: 2 }` — both fields are i32 (Copy). The struct literal
/// uses Copy for both fields (correct — i32 is Copy). No moves, no drops.
#[test]
fn stage15_64_struct_literal_copy_fields_no_regression() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        fn main() -> i32 {
            let p = Point { x: 1, y: 2 };
            p.x + p.y
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Struct literal with Copy fields should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.64 test 5: Field access on non-Drop struct — no regression.
///
/// `p.x` where p is `Point { x: i32, y: i32 }` (no Drop). The field access
/// creates a temp of type i32 (Copy). i32 doesn't need drop, so no issue.
#[test]
fn stage15_64_field_access_non_drop_no_regression() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        fn main() -> i32 {
            let p = Point { x: 42, y: 0 };
            p.x
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Field access on non-Drop struct should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.64 test 6: Multiple field accesses on Drop struct — no double-drop.
///
/// `o.inner.x + o.inner.y` — two field accesses on `o.inner`. Each creates
/// an intermediate temp of type Inner. Both temps are field-copies and
/// should NOT be dropped.
#[test]
fn stage15_64_multiple_field_accesses_no_double_drop() {
    let src = r#"
        trait Drop { fn drop(&mut self); }
        struct Inner { x: i32, y: i32 }
        impl Drop for Inner { fn drop(&mut self) {} }
        struct Outer { inner: Inner }
        fn main() -> i32 {
            let o = Outer { inner: Inner { x: 10, y: 20 } };
            o.inner.x + o.inner.y
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Multiple field accesses should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.64 test 7: Struct literal with mixed Copy and non-Copy fields.
///
/// `Mixed { copy_field: 42, drop_field: Inner { x: 1 } }` — the Copy field
/// uses Copy (correct), the non-Copy field uses Move (correct). No
/// double-drops.
#[test]
fn stage15_64_struct_literal_mixed_copy_non_copy() {
    let src = r#"
        trait Drop { fn drop(&mut self); }
        struct Inner { x: i32 }
        impl Drop for Inner { fn drop(&mut self) {} }
        struct Mixed { copy_field: i32, drop_field: Inner }
        fn main() -> i32 {
            let m = Mixed { copy_field: 42, drop_field: Inner { x: 1 } };
            m.copy_field + m.drop_field.x
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Mixed Copy/non-Copy struct literal should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.64 test 8: Function returning struct with Drop field — no double-drop.
///
/// A function creates and returns a struct with a Drop field. The caller
/// receives the value and accesses its field. No double-drops.
#[test]
fn stage15_64_function_return_struct_with_drop_field() {
    let src = r#"
        trait Drop { fn drop(&mut self); }
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
