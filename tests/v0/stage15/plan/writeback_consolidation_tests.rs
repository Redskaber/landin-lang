//! Stage 15.7 — Consolidated writeback integration tests.
//!
//! These tests verify the consolidated `writeback_type_propagation` and
//! `writeback_closures` functions against real HIR produced by `compile()`.
//! They complement the unit tests in `src/mir/lower/writeback.rs` (which
//! test the functions in isolation with synthetic MIR).
//!
//! Coverage:
//! 1. Method call chains (Call dest writeback) — `a.b().c()`
//! 2. Tuple literals (Tuple Aggregate writeback) — `let t = (1, true);`
//! 3. Field access on tuples (Field projection writeback) — `t.0`
//! 4. Array indexing (Index projection writeback) — `arr[0]`
//! 5. Copy/Move chains (fixpoint convergence) — `let a = b; let c = a;`
//! 6. Closures (closure writeback) — `let f = || x;`
//!
//! Per §29.1.3 (Design-Impl-Test coverage): integration tests verify the
//! writeback functions work correctly with real MIR produced by the full
//! pipeline (lexer → parser → HIR → MIR lower → typeck → writeback).
//!
//! Per §23 (API Naming): test module name follows
//! `<feature>_<noun>_tests` pattern.

#![cfg(test)]

use landin_compiler::compile;

/// Stage 15.7 integration test 1: method call chain compiles and runs.
///
/// Verifies Call dest writeback works for chained method calls:
/// `Counter::new(10).add(5).get()` — each call's dest local must get
/// the correct return type so the next method resolves.
#[test]
fn stage15_7_method_chain_writeback_integration() {
    let src = r#"
        struct Counter { v: i32 }
        impl Counter {
            fn new(v: i32) -> Counter { Counter { v: v } }
            fn add(self, x: i32) -> Counter { Counter { v: self.v + x } }
            fn get(self) -> i32 { self.v }
        }
        fn main() -> i32 {
            Counter::new(10).add(5).get()
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "method chain must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.7 integration test 2: tuple literal + field access.
///
/// Verifies Tuple Aggregate writeback + Field projection writeback:
/// `let t = (42, true); t.0` — t's type must resolve to Tuple([i32, bool])
/// so the field access t.0 can resolve to i32.
#[test]
fn stage15_7_tuple_field_writeback_integration() {
    let src = r#"
        fn main() -> i32 {
            let t = (42, true);
            t.0
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "tuple field access must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.7 integration test 3: array indexing.
///
/// Verifies Index projection writeback: `arr[0]` where arr is `[i32; 3]`
/// — the indexed element's type must resolve to i32.
#[test]
fn stage15_7_array_index_writeback_integration() {
    let src = r#"
        fn main() -> i32 {
            let arr = [10, 20, 30];
            arr[1]
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "array index must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.7 integration test 4: Copy/Move chain (fixpoint convergence).
///
/// Verifies the fixpoint loop converges for chains:
/// `let a = 42; let b = a; let c = b;` — c's type must resolve to i32
/// through two hops of Copy propagation.
#[test]
fn stage15_7_copy_chain_writeback_integration() {
    let src = r#"
        fn main() -> i32 {
            let a = 42;
            let b = a;
            let c = b;
            c
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "copy chain must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.7 integration test 5: closure with capture.
///
/// Verifies closure writeback (all 3 sub-passes):
/// `let x = 42; let f = || x;` — f's type must resolve to
/// `Closure(_, [i32])` so the closure struct has the correct layout.
#[test]
fn stage15_7_closure_writeback_integration() {
    let src = r#"
        fn main() -> i32 {
            let x = 42;
            let f = || x;
            f()
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "closure with capture must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.7 integration test 6: struct-returning method call.
///
/// Verifies Call dest writeback for struct return types:
/// `let p = Point::new(1, 2); p.x()` — p's type must resolve to
/// `Adt(Point, [])` so the method `x` can be resolved on it.
#[test]
fn stage15_7_struct_return_writeback_integration() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        impl Point {
            fn new(x: i32, y: i32) -> Point { Point { x: x, y: y } }
            fn x(self) -> i32 { self.x }
        }
        fn main() -> i32 {
            let p = Point::new(1, 2);
            p.x()
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "struct-returning method call must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.7 integration test 7: generic method (should error, not hang).
///
/// This is the regression test for the infinite-loop bug found during
/// Stage 15.7. The compiler doesn't support generics (v0.1 limitation),
/// so this should produce a compile error — NOT hang.
///
/// Per §1.0 原则 5 "报错 > 静默": errors are better than silent hangs.
///
/// Stage 18.54 update: generic method now compiles successfully (no longer
/// a v0.1 limitation). The fixpoint converges and the method's return type
/// (generic Param) is correctly handled. Test now verifies no hang + no
/// errors (previously asserted errors were present).
#[test]
fn stage15_7_generic_method_no_hang_regression() {
    let src = r#"
        struct S;
        impl S { fn f<T>(&self, x: T) -> T { x } }
        fn main() -> i32 { let s = S; s.f(42) }
    "#;
    let result = compile(src);
    // Stage 18.54: Generic method now resolves correctly (T → Param → unify with i32).
    // Must NOT hang and must NOT produce errors.
    assert!(
        result.errors.is_empty(),
        "generic method should compile without errors (Stage 18.54 fix), got: {:?}",
        result.errors
    );
}
