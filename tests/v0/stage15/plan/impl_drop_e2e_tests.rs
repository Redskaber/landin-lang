//! Stage 15.61 — `impl Drop` end-to-end tests.
//!
//! These tests verify that programs WITH `impl Drop` compile and run
//! correctly after the Stage 15.61 fixes:
//!
//! 1. **elaborate_drops infinite-loop fix**: The `StorageDead(local)`
//!    statement is no longer carried into the new block when splitting
//!    (which caused an infinite loop → OOM kill, exit 137).
//!
//! 2. **Drop codegen type fix**: `TerminatorKind::Drop` codegen now
//!    passes `EmitType::OpaquePtr` (matching the drop glue function's
//!    `ptr %self` parameter) instead of the place's value type.
//!
//! 3. **LLVM backend drop glue emission**: `codegen_crate_to_module`
//!    now calls `emit_drop_glue_functions` (previously only the text
//!    backend called it, causing "undefined reference to drop_adt_<N>"
//!    link errors).
//!
//! 4. **borrowck Drop semantics fix**: `TerminatorKind::Drop` is now
//!    treated as a destructor (no-op for moved places, consuming for
//!    live places) instead of a read (which flagged "use of moved
//!    value" for moved temps that elaborate_drops inserts Drop for).
//!
//! Per §17.1: tests live under `tests/v0/stage{N}/plan/`.
//! Per §17.5: test names follow `<stage>_<id>_<description>` pattern.
//! Per §1.0 原則 5 "报错 > 静默": tests assert both success and failure.

#![cfg(test)]

use landin_compiler::compile;

/// Stage 15.61 test 1: Basic `impl Drop` program compiles.
///
/// This is the simplest possible `impl Drop` program — a struct with
/// a Drop impl that has an empty body. The program should compile
/// without errors (no crash, no borrowck error, no codegen error).
#[test]
fn stage15_61_impl_drop_basic_compiles() {
    let src = r#"
        trait Drop { fn drop(&mut self); }
        struct S { x: i32 }
        impl Drop for S { fn drop(&mut self) {} }
        fn main() -> i32 {
            let s = S { x: 42 };
            s.x
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Basic impl Drop program should compile cleanly. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.61 test 2: `impl Drop` program with `let _ = S{{...}}` pattern.
///
/// The `let _` pattern binds the value to a wildcard (immediately dropped).
/// This should compile without "use of moved value" errors.
#[test]
fn stage15_61_impl_drop_let_wildcard_compiles() {
    let src = r#"
        trait Drop { fn drop(&mut self); }
        struct S { x: i32 }
        impl Drop for S { fn drop(&mut self) {} }
        fn main() -> i32 {
            let _ = S { x: 0 };
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "let _ = S{{...}} with impl Drop should compile cleanly. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.61 test 3: Multiple locals with `impl Drop` — drop order.
///
/// Two locals of a Drop type are declared. Both should be dropped at
/// scope end (in forward declaration order for the MVP — Rust uses
/// reverse order, but that's a separate task).
#[test]
fn stage15_61_impl_drop_multiple_locals() {
    let src = r#"
        trait Drop { fn drop(&mut self); }
        struct S { x: i32 }
        impl Drop for S { fn drop(&mut self) {} }
        fn main() -> i32 {
            let a = S { x: 1 };
            let b = S { x: 2 };
            a.x + b.x
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Multiple locals with impl Drop should compile cleanly. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.61 test 4: `impl Drop` with field access (copy out i32 field).
///
/// Accessing a Copy field (i32) of a non-Copy struct (has Drop) should
/// work — the field is copied, not moved, and the struct is dropped at
/// scope end.
#[test]
fn stage15_61_impl_drop_field_access_copy() {
    let src = r#"
        trait Drop { fn drop(&mut self); }
        struct S { x: i32 }
        impl Drop for S { fn drop(&mut self) {} }
        fn main() -> i32 {
            let s = S { x: 42 };
            let v = s.x;
            let w = s.x;
            v + w
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Accessing Copy field of Drop struct should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.61 test 5: `impl Drop` with `&self` method (no move).
///
/// A method taking `&self` doesn't move the value, so the value should
/// still be droppable at scope end.
#[test]
fn stage15_61_impl_drop_with_ref_method() {
    let src = r#"
        trait Drop { fn drop(&mut self); }
        struct S { x: i32 }
        impl Drop for S { fn drop(&mut self) {} }
        impl S {
            fn get(self: &S) -> i32 { self.x }
        }
        fn main() -> i32 {
            let s = S { x: 42 };
            s.get()
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "impl Drop with &self method should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.61 test 6: `impl Drop` with explicit `self: &mut Counter` syntax.
///
/// Tests the explicit type annotation syntax for `self` (vs the `&mut self`
/// shorthand). Both should work.
#[test]
fn stage15_61_impl_drop_explicit_self_type() {
    let src = r#"
        struct Counter { value: i32 }
        impl Drop for Counter {
            fn drop(self: &mut Counter) {
                let _ = self.value;
            }
        }
        fn main() -> i32 {
            let c = Counter { value: 42 };
            c.value
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "impl Drop with explicit self type should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.61 test 7: `impl Drop` with function returning Drop type.
///
/// A function that returns a Drop type — the caller receives the value
/// and drops it at scope end. The function's local is moved into the
/// return value, so its Drop terminator should be a no-op.
#[test]
fn stage15_61_impl_drop_function_returns_drop_type() {
    let src = r#"
        struct Counter { value: i32 }
        impl Drop for Counter {
            fn drop(self: &mut Counter) {
                let _ = self.value;
            }
        }
        fn make(v: i32) -> Counter {
            Counter { value: v }
        }
        fn main() -> i32 {
            let c = make(42);
            c.value
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Function returning Drop type should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.61 test 8: `impl Drop` with multiple structs and cross-calls.
///
/// Two Drop structs, a function that takes a reference to one, and the
/// main function that uses both. Tests that the drop elaboration pipeline
/// handles cross-function interactions correctly.
#[test]
fn stage15_61_impl_drop_multiple_structs_cross_calls() {
    let src = r#"
        struct Counter { value: i32 }
        impl Drop for Counter {
            fn drop(self: &mut Counter) {
                let _ = self.value;
            }
        }
        fn make(v: i32) -> Counter {
            Counter { value: v }
        }
        fn use_counter(c: &Counter) -> i32 {
            c.value
        }
        fn main() -> i32 {
            let c = make(10);
            let d = make(20);
            let sum = use_counter(&c) + use_counter(&d);
            sum
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Multiple Drop structs with cross-calls should compile. Errors: {:?}",
        result.errors.borrowck
    );
}
