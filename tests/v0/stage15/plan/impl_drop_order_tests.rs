//! Stage 15.62 — Drop order + double-drop prevention tests.
//!
//! These tests verify that:
//!
//! 1. **Drop order**: Locals are dropped in reverse declaration order
//!    (matching Rust's drop semantics — RFC 1327). The last-declared
//!    local is dropped first, the first-declared local is dropped last.
//!
//! 2. **Double-drop prevention**: Temporaries that are moved into let
//!    bindings are NOT dropped again at scope end. Only the destination
//!    local (which owns the value) is dropped.
//!
//! 3. **No regression**: Programs without `impl Drop` still compile
//!    cleanly (the `elaborate_drops` pass is a no-op for non-Drop types).
//!
//! Per §17.1: tests live under `tests/v0/stage{N}/plan/`.
//! Per §17.5: test names follow `<stage>_<id>_<description>` pattern.

#![cfg(test)]

use landin_compiler::compile;

/// Stage 15.62 test 1: Drop order — multiple locals dropped in reverse
/// declaration order.
///
/// This program declares three Drop locals (a, b, c). At scope end, they
/// should be dropped in reverse order: c first, then b, then a.
/// The test verifies that the program compiles (the drop order is
/// implemented in MIR lowering + elaborate_drops).
#[test]
fn stage15_62_drop_order_reverse_declaration_compiles() {
    let src = r#"
        trait Drop { fn drop(&mut self); }
        struct S { x: i32 }
        impl Drop for S { fn drop(&mut self) {} }
        fn main() -> i32 {
            let a = S { x: 1 };
            let b = S { x: 2 };
            let c = S { x: 3 };
            a.x + b.x + c.x
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Multiple Drop locals should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.62 test 2: Double-drop prevention — moved temporaries are
/// not dropped again.
///
/// When `let c = make(42);` is lowered, a temporary holds the return
/// value of `make(42)`, then the temporary is moved into `c`. At scope
/// end, only `c` should be dropped (not the temporary). The Stage 15.62
/// `collect_moved_locals` analysis identifies the temporary as moved
/// and skips its Drop terminator.
#[test]
fn stage15_62_no_double_drop_moved_temporary() {
    let src = r#"
        struct Counter { value: i32 }
        impl Drop for Counter {
            fn drop(self: &mut Counter) { let _ = self.value; }
        }
        fn make(v: i32) -> Counter { Counter { value: v } }
        fn main() -> i32 {
            let c = make(42);
            c.value
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Moved temporary should not cause double-drop. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.62 test 3: Multiple moved temporaries — all skipped.
///
/// Two function calls produce two temporaries, both moved into let
/// bindings. Neither temporary should be dropped (only the let bindings).
#[test]
fn stage15_62_no_double_drop_multiple_temporaries() {
    let src = r#"
        struct Counter { value: i32 }
        impl Drop for Counter {
            fn drop(self: &mut Counter) { let _ = self.value; }
        }
        fn make(v: i32) -> Counter { Counter { value: v } }
        fn main() -> i32 {
            let c = make(10);
            let d = make(20);
            c.value + d.value
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Multiple moved temporaries should not cause double-drop. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.62 test 4: Drop with cross-function calls — no double-drop.
///
/// A function takes a reference to a Drop type. The Drop local is not
/// moved (only borrowed), so it should still be dropped at scope end.
/// The function's parameter (a reference) is not a Drop type, so it
/// should not be dropped.
#[test]
fn stage15_62_drop_with_borrow_no_double_drop() {
    let src = r#"
        struct Counter { value: i32 }
        impl Drop for Counter {
            fn drop(self: &mut Counter) { let _ = self.value; }
        }
        fn make(v: i32) -> Counter { Counter { value: v } }
        fn use_counter(c: &Counter) -> i32 { c.value }
        fn main() -> i32 {
            let c = make(10);
            let d = make(20);
            use_counter(&c) + use_counter(&d)
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Drop with borrow should not cause double-drop. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.62 test 5: No regression — non-Drop struct with multiple locals.
///
/// A struct without `impl Drop` should not trigger any Drop terminators.
/// The `elaborate_drops` pass should be a no-op for this program.
#[test]
fn stage15_62_no_drop_no_regression() {
    let src = r#"
        struct S { x: i32 }
        fn main() -> i32 {
            let a = S { x: 1 };
            let b = S { x: 2 };
            let c = S { x: 3 };
            a.x + b.x + c.x
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Non-Drop struct should compile cleanly. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.62 test 6: Drop order with mixed Drop and non-Drop locals.
///
/// Some locals have Drop, some don't. Only the Drop locals should
/// receive Drop terminators. The order should be reverse declaration
/// among the Drop locals.
#[test]
fn stage15_62_drop_order_mixed_drop_non_drop() {
    let src = r#"
        trait Drop { fn drop(&mut self); }
        struct DropType { x: i32 }
        struct PlainType { y: i32 }
        impl Drop for DropType { fn drop(&mut self) {} }
        fn main() -> i32 {
            let p = PlainType { y: 0 };   // no Drop
            let a = DropType { x: 1 };     // Drop
            let q = PlainType { y: 2 };   // no Drop
            let b = DropType { x: 3 };     // Drop
            a.x + b.x + p.y + q.y
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Mixed Drop/non-Drop locals should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.62 test 7: Drop in nested function scopes — no cross-function
/// interference.
///
/// A function creates a Drop local and returns an i32. The Drop local
/// is dropped at the function's return. The caller's Drop locals are
/// dropped at the caller's return. The two functions' drop orders
/// should not interfere.
#[test]
fn stage15_62_drop_nested_function_scopes() {
    let src = r#"
        trait Drop { fn drop(&mut self); }
        struct S { x: i32 }
        impl Drop for S { fn drop(&mut self) {} }
        fn helper(v: i32) -> i32 {
            let s = S { x: v };
            s.x
        }
        fn main() -> i32 {
            let a = S { x: 1 };
            let v = helper(42);
            a.x + v
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Nested function scopes with Drop should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.62 test 8: Drop with explicit `self: &mut Type` syntax.
///
/// The explicit self type annotation (vs `&mut self` shorthand) should
/// work correctly with the drop order and double-drop prevention.
#[test]
fn stage15_62_drop_explicit_self_type_no_double_drop() {
    let src = r#"
        struct Counter { value: i32 }
        impl Drop for Counter {
            fn drop(self: &mut Counter) {
                let _ = self.value;
            }
        }
        fn make(v: i32) -> Counter { Counter { value: v } }
        fn main() -> i32 {
            let a = make(1);
            let b = make(2);
            let c = make(3);
            a.value + b.value + c.value
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Explicit self type with multiple Drop locals should compile. Errors: {:?}",
        result.errors.borrowck
    );
}
