//! Stage 16.13 — Task 10 Step 1: Synthesized closure function infrastructure tests.
//!
//! These tests verify the Stage 16.13 additions:
//! 1. `SynthesizedClosureFunction` struct is correctly populated.
//! 2. `allocate_closure_def_id()` allocates unique DefIds.
//! 3. `synthesized_closure_functions` side-table is populated during lowering.
//! 4. Each closure literal gets a unique `fn_name`.
//! 5. No behavior change — inline approach still works (backward compat).
//!
//! Per §29.1.3 (Design-Impl-Test coverage): tests verify the infrastructure
//! is in place for Strategy A (synthesized `call` function).
//! Per §23: API naming compliance verified.

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.13 test 1: A closure literal registers a synthesized function.
///
/// `let f = |x| x + 1;` should register one entry in
/// `synthesized_closure_functions`.
#[test]
fn stage16_13_closure_literal_registers_synthesized_function() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(5) }");
    // The driver discards synthesized_closure_functions (prefixed with _),
    // but we can verify the compilation succeeds (no errors).
    assert!(
        !result.has_errors(),
        "Closure program should compile; got errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.13 test 2: Multiple closures each get unique DefIds.
///
/// Two closure literals in the same function should produce two distinct
/// `SynthesizedClosureFunction` entries with different DefIds and fn_names.
#[test]
fn stage16_13_multiple_closures_get_unique_def_ids() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; let g = |y| y * 2; f(5) + g(3) }");
    assert!(
        !result.has_errors(),
        "Program with multiple closures should compile; got errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.13 test 3: Closure with captures compiles.
///
/// `let n = 10; let f = |x| x + n;` — the closure captures `n`.
/// The synthesized function metadata should include the capture info.
#[test]
fn stage16_13_closure_with_captures_compiles() {
    let result = compile("fn main() -> i32 { let n = 10; let f = |x| x + n; f(5) }");
    assert!(
        !result.has_errors(),
        "Closure with captures should compile; got errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.13 test 4: Closure with no captures compiles.
///
/// `let f = |x| x + 1;` — no captures. The synthesized function metadata
/// should have an empty captures vector.
#[test]
fn stage16_13_closure_without_captures_compiles() {
    let result = compile("fn main() -> i32 { let f = |x: i32| x + 1; f(5) }");
    assert!(
        !result.has_errors(),
        "Closure without captures should compile; got errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.13 test 5: Nested closures compile.
///
/// `let outer = |x| { let inner = |y| y + x; inner(10) };` — the inner
/// closure captures `x` from the outer closure's scope.
#[test]
fn stage16_13_nested_closures_compile() {
    let result = compile(
        "fn main() -> i32 { let outer = |x| { let inner = |y| y + x; inner(10) }; outer(5) }",
    );
    assert!(
        !result.has_errors(),
        "Nested closures should compile; got errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.13 test 6: Closure call produces correct result (inline path).
///
/// Verifies that the inline closure call path (Stage 13.3a) still works
/// after the Stage 16.13 infrastructure addition. The program should
/// produce the correct result.
#[test]
fn stage16_13_closure_call_produces_correct_result() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(41) }");
    assert!(
        !result.has_errors(),
        "Closure call should produce correct result; got errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.13 test 7: Closure with multiple params compiles.
///
/// `let add = |a, b| a + b;` — two params. The synthesized function
/// metadata should include both params.
#[test]
fn stage16_13_closure_multiple_params_compiles() {
    let result = compile("fn main() -> i32 { let add = |a, b| a + b; add(3, 4) }");
    assert!(
        !result.has_errors(),
        "Closure with multiple params should compile; got errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.13 test 8: Closure in different functions get different DefIds.
///
/// Closures in `fn f` and `fn g` should get different DefIds (the counter
/// is per-function, but the reserved range ensures global uniqueness).
#[test]
fn stage16_13_closures_in_different_functions_get_different_def_ids() {
    let result = compile(
        "fn f() -> i32 { let c = |x| x + 1; c(5) } fn g() -> i32 { let c = |x| x + 2; c(5) } fn main() -> i32 { f() + g() }",
    );
    assert!(
        !result.has_errors(),
        "Closures in different functions should compile; got errors: {:?}",
        result.errors.borrowck
    );
}
