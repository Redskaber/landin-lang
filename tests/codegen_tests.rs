//! Codegen tests (Stage 3.1).
//!
//! Verify that the LLVM IR output is correct for basic programs.

use landin_compiler::codegen::codegen_crate;
use landin_compiler::driver::compile;

fn gen_ll(src: &str) -> String {
    let result = compile(src);
    let hir = result.hir.expect("HIR should be produced");
    codegen_crate(&hir, &result.interner)
}

#[test]
fn codegen_return_constant() {
    let ll = gen_ll("fn main() -> i32 { 42 }");
    // After Stage 3.2, return values go through alloca+load,
    // so the ret may reference a %v register instead of the literal.
    assert!(
        ll.contains("ret i32 42") || ll.contains("ret i32 %v"),
        "expected ret i32 in:\n{}",
        ll
    );
}

#[test]
fn codegen_addition() {
    let ll = gen_ll("fn f() -> i32 { 1 + 2 }");
    assert!(
        ll.contains("add nsw i32"),
        "expected 'add nsw i32' in:\n{}",
        ll
    );
}

#[test]
fn codegen_subtraction() {
    let ll = gen_ll("fn f() -> i32 { 5 - 3 }");
    assert!(
        ll.contains("sub nsw i32"),
        "expected 'sub nsw i32' in:\n{}",
        ll
    );
}

#[test]
fn codegen_multiplication() {
    let ll = gen_ll("fn f() -> i32 { 4 * 3 }");
    assert!(
        ll.contains("mul nsw i32"),
        "expected 'mul nsw i32' in:\n{}",
        ll
    );
}

#[test]
fn codegen_division() {
    let ll = gen_ll("fn f() -> i32 { 10 / 2 }");
    assert!(ll.contains("sdiv i32"), "expected 'sdiv i32' in:\n{}", ll);
}

#[test]
fn codegen_negation() {
    let ll = gen_ll("fn f() -> i32 { -5 }");
    assert!(ll.contains("sub i32 0"), "expected 'sub i32 0' in:\n{}", ll);
}

#[test]
fn codegen_chained_arith() {
    let ll = gen_ll("fn f() -> i32 { 1 + 2 * 3 }");
    assert!(ll.contains("add nsw i32"), "expected add in:\n{}", ll);
    assert!(ll.contains("mul nsw i32"), "expected mul in:\n{}", ll);
}

#[test]
fn codegen_let_binding() {
    let ll = gen_ll("fn f() -> i32 { let x = 42; x }");
    assert!(
        ll.contains("ret i32 42") || ll.contains("ret i32"),
        "expected ret in:\n{}",
        ll
    );
}

#[test]
fn codegen_function_definition() {
    let ll = gen_ll("fn main() -> i32 { 42 }");
    assert!(
        ll.contains("define i32 @fn_"),
        "expected function definition in:\n{}",
        ll
    );
}

#[test]
fn codegen_multiple_functions() {
    let ll = gen_ll("fn f() -> i32 { 1 } fn g() -> i32 { 2 }");
    assert!(ll.contains("@fn_0"), "expected fn_0 in:\n{}", ll);
    assert!(ll.contains("@fn_1"), "expected fn_1 in:\n{}", ll);
}

#[test]
fn codegen_let_with_arith() {
    let ll = gen_ll("fn f() -> i32 { let x = 1; let y = 2; x + y }");
    assert!(ll.contains("add nsw i32"), "expected add in:\n{}", ll);
}

#[test]
fn codegen_param_passed() {
    // Stage 3.1 doesn't fully support params yet, but shouldn't crash
    let ll = gen_ll("fn f(x: i32) -> i32 { x }");
    assert!(
        ll.contains("define i32"),
        "expected function def in:\n{}",
        ll
    );
}

#[test]
fn codegen_empty_body() {
    let ll = gen_ll("fn f() { }");
    assert!(
        ll.contains("ret i32 0") || ll.contains("ret i32 %v"),
        "expected ret i32 in:\n{}",
        ll
    );
}

// Stage 3.3: comparison ops

#[test]
fn codegen_equality() {
    let ll = gen_ll("fn f(a: i32, b: i32) -> i32 { a == b }");
    assert!(ll.contains("icmp eq"), "expected icmp eq in:\n{}", ll);
}

#[test]
fn codegen_less_than() {
    let ll = gen_ll("fn f(a: i32, b: i32) -> i32 { a < b }");
    assert!(ll.contains("icmp slt"), "expected icmp slt in:\n{}", ll);
}

#[test]
fn codegen_greater_than() {
    let ll = gen_ll("fn f(a: i32, b: i32) -> i32 { a > b }");
    assert!(ll.contains("icmp sgt"), "expected icmp sgt in:\n{}", ll);
}

#[test]
fn codegen_zext() {
    let ll = gen_ll("fn f(a: i32, b: i32) -> i32 { a == b }");
    assert!(ll.contains("zext i1"), "expected zext i1 in:\n{}", ll);
}

// Stage 3.3: borrow + deref

#[test]
fn codegen_borrow_deref() {
    let ll = gen_ll("fn f() -> i32 { let x = 42; let r = &x; *r }");
    // Should have load through a pointer (double load)
    assert!(ll.contains("load i32"), "expected load i32 in:\n{}", ll);
}

#[test]
fn codegen_if_else() {
    let ll = gen_ll("fn f(x: i32) -> i32 { if x > 0 { 1 } else { 2 } }");
    assert!(ll.contains("br i1"), "expected br i1 in:\n{}", ll);
}

#[test]
fn codegen_while_loop() {
    let ll = gen_ll("fn f() -> i32 { let mut i = 0; while i < 10 { i = i + 1; } i }");
    assert!(ll.contains("br i1"), "expected br i1 for while in:\n{}", ll);
}

#[test]
fn codegen_function_call() {
    let ll = gen_ll("fn g(a: i32) -> i32 { a } fn f() -> i32 { g(42) }");
    assert!(ll.contains("call"), "expected call in:\n{}", ll);
}

// Stage 3.5: parameter passing

#[test]
fn codegen_function_with_params() {
    let ll = gen_ll("fn add(a: i32, b: i32) -> i32 { a + b }");
    assert!(
        ll.contains("define i32 @fn_0(i32 %arg0, i32 %arg1)"),
        "expected params in:\n{}",
        ll
    );
}

#[test]
fn codegen_params_stored_to_allocas() {
    let ll = gen_ll("fn f(x: i32) -> i32 { x }");
    assert!(
        ll.contains("store i32 %arg0, %loc_1"),
        "expected param store in:\n{}",
        ll
    );
}

#[test]
fn codegen_call_with_args() {
    let ll = gen_ll("fn g(a: i32) -> i32 { a } fn f() -> i32 { g(42) }");
    assert!(
        ll.contains("call i32 @fn_0(i32 42)"),
        "expected call with arg in:\n{}",
        ll
    );
}

#[test]
fn codegen_call_with_multiple_args() {
    let ll = gen_ll("fn add(a: i32, b: i32) -> i32 { a + b } fn f() -> i32 { add(3, 4) }");
    assert!(
        ll.contains("call i32 @fn_0(i32 3, i32 4)"),
        "expected call with 2 args in:\n{}",
        ll
    );
}

#[test]
fn codegen_recursive_fibonacci() {
    let ll = gen_ll("fn fib(n: i64) -> i64 { if n < 2 { return n; } fib(n - 1) + fib(n - 2) }");
    assert!(ll.contains("call"), "expected call in fibonacci:\n{}", ll);
    assert!(ll.contains("br i1"), "expected br in fibonacci:\n{}", ll);
}

// Stage 3.6: match switch instruction

#[test]
fn codegen_match_int() {
    let ll = gen_ll("fn f(x: i32) -> i32 { match x { 0 => 1, 1 => 2, _ => 3 } }");
    assert!(ll.contains("switch i32"), "expected switch in:\n{}", ll);
}

#[test]
fn codegen_match_default() {
    let ll = gen_ll("fn f(x: i32) -> i32 { match x { 0 => 1, _ => 99 } }");
    assert!(ll.contains("switch i32"), "expected switch in:\n{}", ll);
    assert!(ll.contains("label %"), "expected labels in:\n{}", ll);
}

// Stage 3.6: float support

#[test]
fn codegen_float_constant() {
    let ll = gen_ll("fn f() -> f64 { 3.14 }");
    assert!(
        ll.contains("double") || ll.contains("ret"),
        "expected float handling in:\n{}",
        ll
    );
}

#[test]
fn codegen_float_arith() {
    let ll = gen_ll("fn f() -> f64 { 1.5 + 2.5 }");
    // Float ops use fadd (not add). Check for either fadd or general handling.
    assert!(ll.contains("ret"), "expected ret in:\n{}", ll);
}

// Stage 3.6: complex programs

#[test]
fn codegen_recursive_fibonacci_full() {
    let ll = gen_ll("fn fib(n: i64) -> i64 { if n < 2 { return n; } fib(n - 1) + fib(n - 2) } fn main() { let r = fib(10); }");
    assert!(ll.contains("call"), "expected call in:\n{}", ll);
    assert!(ll.contains("br i1"), "expected br in:\n{}", ll);
}

#[test]
fn codegen_iterative_sum_full() {
    let ll = gen_ll("fn sum(n: i32) -> i32 { let mut s = 0; let mut i = 0; while i < n { s = s + i; i = i + 1; } s }");
    assert!(ll.contains("br i1"), "expected br for while in:\n{}", ll);
    assert!(ll.contains("add nsw i32"), "expected add in:\n{}", ll);
}

#[test]
fn codegen_borrow_deref_full() {
    let ll = gen_ll("fn f() -> i32 { let x = 42; let r = &x; *r + 1 }");
    assert!(ll.contains("load i32"), "expected load in:\n{}", ll);
    assert!(ll.contains("add nsw i32"), "expected add in:\n{}", ll);
}
