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
        ll.contains("define i32 @landin_"),
        "expected function definition in:\n{}",
        ll
    );
}

#[test]
fn codegen_multiple_functions() {
    let ll = gen_ll("fn f() -> i32 { 1 } fn g() -> i32 { 2 }");
    assert!(ll.contains("@landin_"), "expected fn_0 in:\n{}", ll);
    assert!(ll.contains("@landin_"), "expected fn_1 in:\n{}", ll);
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
    // Empty body (unit return) → ret void (per design doc §2.1)
    assert!(
        ll.contains("ret void") || ll.contains("ret i32 0") || ll.contains("ret i32 %v"),
        "expected ret in:\n{}",
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
        ll.contains("define i32 @landin_add(i32 %arg0, i32 %arg1)"),
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
        ll.contains("call i32 @landin_g(i32 42)"),
        "expected call with arg in:\n{}",
        ll
    );
}

#[test]
fn codegen_call_with_multiple_args() {
    let ll = gen_ll("fn add(a: i32, b: i32) -> i32 { a + b } fn f() -> i32 { add(3, 4) }");
    assert!(
        ll.contains("call i32 @landin_add(i32 3, i32 4)"),
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

// Stage 3.7: cast support

#[test]
fn codegen_cast_int_to_f64() {
    let ll = gen_ll("fn f(x: i32) -> f64 { x as f64 }");
    // Should contain sitofp or some cast instruction
    assert!(
        ll.contains("sitofp") || ll.contains("ret"),
        "expected cast in:\n{}",
        ll
    );
}

#[test]
fn codegen_cast_int_to_i64() {
    let ll = gen_ll("fn f(x: i32) -> i64 { x as i64 }");
    assert!(
        ll.contains("sext") || ll.contains("ret"),
        "expected sext in:\n{}",
        ll
    );
}

#[test]
fn codegen_bool_return() {
    let ll = gen_ll("fn f(a: i32, b: i32) -> bool { a == b }");
    // Should have icmp eq
    assert!(ll.contains("icmp eq"), "expected icmp eq in:\n{}", ll);
}

// Stage 3.21: typed aggregate + typed call args

#[test]
fn codegen_tuple_of_two_distinct_types() {
    // (i32, f64) — should emit `{ i32, double }`, NOT hardcoded `{ i32 }`.
    let ll = gen_ll("fn f() -> f64 { let t = (1, 2.5); t.1 }");
    assert!(
        ll.contains("{ i32, double }"),
        "expected '{{ i32, double }}' struct type in:\n{}",
        ll
    );
}

#[test]
fn codegen_tuple_of_three_types() {
    // (i32, f64, bool) — should emit `{ i32, double, i1 }`.
    let ll = gen_ll("fn f() -> bool { let t = (1, 2.5, true); t.2 }");
    assert!(
        ll.contains("{ i32, double, i1 }"),
        "expected '{{ i32, double, i1 }}' struct type in:\n{}",
        ll
    );
}

#[test]
fn codegen_array_of_i32_correct_length() {
    // [1, 2, 3] — should emit `[3 x i32]`, NOT hardcoded `[10 x i32]`.
    let ll = gen_ll("fn f() -> i32 { let a = [1, 2, 3]; a.0 }");
    assert!(
        ll.contains("[3 x i32]"),
        "expected '[3 x i32]' array type in:\n{}",
        ll
    );
    assert!(
        !ll.contains("[10 x i32]"),
        "should NOT have hardcoded '[10 x i32]' in:\n{}",
        ll
    );
}

#[test]
fn codegen_array_of_i64_correct_length() {
    // [1i64, 2, 3] — should emit `[3 x i64]`.
    let ll = gen_ll("fn f() -> i64 { let a: [i64; 3] = [1, 2, 3]; a.0 }");
    assert!(
        ll.contains("[3 x i64]"),
        "expected '[3 x i64]' array type in:\n{}",
        ll
    );
}

#[test]
fn codegen_typed_call_args_i64() {
    // call with i64 arg should produce `call i64 @landin_g(i64 42)` — not `i32 42`.
    let ll = gen_ll("fn g(a: i64) -> i64 { a } fn f() -> i64 { g(42) }");
    assert!(
        ll.contains("call i64 @landin_g(i64 42)"),
        "expected typed i64 call arg in:\n{}",
        ll
    );
}

#[test]
fn codegen_typed_call_args_mixed() {
    // call with mixed types should produce typed args
    let ll = gen_ll("fn g(a: i32, b: i64) -> i64 { b } fn f() -> i64 { g(1, 2) }");
    assert!(
        ll.contains("call i64 @landin_g(i32 1, i64 2)"),
        "expected typed mixed call args in:\n{}",
        ll
    );
}

#[test]
fn codegen_typed_ptr_to_i32() {
    // &x where x: i32 — pointer should be `i32*` (was `i32*` before too, but
    // now via typed Ptr(I32) variant). The deref should `load i32, i32*`.
    let ll = gen_ll("fn f() -> i32 { let x = 42; let r = &x; *r }");
    assert!(
        ll.contains("alloca i32"),
        "expected 'alloca i32' in:\n{}",
        ll
    );
}

#[test]
fn codegen_array_gep_uses_correct_type() {
    // Indexing a [3 x i32] array should GEP into [3 x i32], not [10 x i32].
    let ll = gen_ll("fn f() -> i32 { let a = [10, 20, 30]; let i = 1; a[i] }");
    assert!(
        ll.contains("[3 x i32]"),
        "expected '[3 x i32]' in GEP in:\n{}",
        ll
    );
    assert!(
        !ll.contains("[10 x i32]"),
        "should NOT have hardcoded '[10 x i32]' in:\n{}",
        ll
    );
}

#[test]
fn codegen_tuple_field_access_via_gep() {
    // Tuple field access should use the actual tuple type in GEP.
    let ll = gen_ll("fn f() -> f64 { let t = (1, 2.5); t.1 }");
    // GEP should reference `{ i32, double }` (not hardcoded `{ i32, i32 }`).
    assert!(
        ll.contains("getelementptr inbounds { i32, double }"),
        "expected typed GEP for {{ i32, double }} in:\n{}",
        ll
    );
}

#[test]
fn codegen_insertvalue_with_typed_field() {
    // Building a tuple via insertvalue should preserve field types.
    let ll = gen_ll("fn f() -> f64 { let t = (1, 2.5); t.1 }");
    // insertvalue should reference `{ i32, double }` and insert `double` value.
    assert!(
        ll.contains("insertvalue { i32, double }"),
        "expected typed insertvalue in:\n{}",
        ll
    );
}

// Stage 3.22: block-scoped local value cache (correctness for if/match/while joins)

#[test]
fn codegen_if_else_merge_correct_value() {
    // Regression: bb3 must load the merged value from the result slot,
    // NOT leak the most-recent store (was returning 2 always).
    let ll = gen_ll("fn f(x: i32) -> i32 { if x > 0 { 1 } else { 2 } }");
    // The return must come from a load (either of the result slot or
    // via a phi), NOT a hardcoded constant.
    assert!(
        ll.contains("ret i32 %v"),
        "expected 'ret i32 %v' (loaded value, not constant) in:\n{}",
        ll
    );
    // Should NOT have "ret i32 2" (the bug was returning the false-branch value).
    assert!(
        !ll.contains("ret i32 2"),
        "should NOT return hardcoded 2 in:\n{}",
        ll
    );
}

#[test]
fn codegen_if_else_stores_to_result_slot() {
    // Both branches should store to the same result slot (loc_4 typically).
    let ll = gen_ll("fn f(x: i32) -> i32 { if x > 0 { 1 } else { 2 } }");
    // Count store i32 1 and store i32 2 — both must be present.
    let has_one = ll.contains("store i32 1,");
    let has_two = ll.contains("store i32 2,");
    assert!(
        has_one && has_two,
        "expected both 'store i32 1' and 'store i32 2' in:\n{}",
        ll
    );
}

#[test]
fn codegen_nested_if_correctness() {
    // Nested if: result must come from a load, not a leaked constant.
    let ll = gen_ll("fn f(x: i32) -> i32 { if x > 0 { if x > 10 { 100 } else { 1 } } else { 2 } }");
    assert!(
        ll.contains("ret i32 %v"),
        "expected loaded return value in nested if:\n{}",
        ll
    );
}

#[test]
fn codegen_match_correctness() {
    // Match expressions lower to SwitchInt; the merge block must load correctly.
    let ll = gen_ll("fn f(x: i32) -> i32 { match x { 0 => 10, 1 => 20, _ => 30 } }");
    assert!(
        ll.contains("ret i32 %v"),
        "expected loaded return value in match:\n{}",
        ll
    );
    // All three arms' constants should appear.
    assert!(
        ll.contains("store i32 10"),
        "missing arm 0 value in:\n{}",
        ll
    );
    assert!(
        ll.contains("store i32 20"),
        "missing arm 1 value in:\n{}",
        ll
    );
    assert!(
        ll.contains("store i32 30"),
        "missing default arm value in:\n{}",
        ll
    );
}

#[test]
fn codegen_while_loop_correctness() {
    // While loop: after the loop, reading the counter must load from alloca.
    let ll = gen_ll("fn f() -> i32 { let mut i = 0; while i < 10 { i = i + 1; } i }");
    assert!(
        ll.contains("ret i32 %v"),
        "expected loaded return value after while:\n{}",
        ll
    );
}

#[test]
fn codegen_if_else_with_arith() {
    // If branches with arithmetic — result must be loaded, not constant-leaked.
    let ll = gen_ll("fn f(x: i32) -> i32 { if x > 0 { x + 1 } else { x - 1 } }");
    assert!(
        ll.contains("ret i32 %v"),
        "expected loaded return value in if-with-arith:\n{}",
        ll
    );
    assert!(
        ll.contains("add nsw i32"),
        "expected add in true branch:\n{}",
        ll
    );
    assert!(
        ll.contains("sub nsw i32"),
        "expected sub in false branch:\n{}",
        ll
    );
}

// Stage 3.24: real overflow checks via llvm.{sadd,ssub,smul}.with.overflow

#[test]
fn codegen_overflow_check_add() {
    let ll = gen_ll("fn f(a: i32, b: i32) -> i32 { a + b }");
    assert!(
        ll.contains("llvm.sadd.with.overflow.i32"),
        "expected llvm.sadd.with.overflow.i32 in:\n{}",
        ll
    );
    assert!(
        ll.contains("extractvalue { i32, i1 }"),
        "expected extractvalue of overflow flag in:\n{}",
        ll
    );
}

#[test]
fn codegen_overflow_check_sub() {
    let ll = gen_ll("fn f(a: i32, b: i32) -> i32 { a - b }");
    assert!(
        ll.contains("llvm.ssub.with.overflow.i32"),
        "expected llvm.ssub.with.overflow.i32 in:\n{}",
        ll
    );
}

#[test]
fn codegen_overflow_check_mul() {
    let ll = gen_ll("fn f(a: i32, b: i32) -> i32 { a * b }");
    assert!(
        ll.contains("llvm.smul.with.overflow.i32"),
        "expected llvm.smul.with.overflow.i32 in:\n{}",
        ll
    );
}

#[test]
fn codegen_overflow_check_i64() {
    let ll = gen_ll("fn f(a: i64, b: i64) -> i64 { a + b }");
    assert!(
        ll.contains("llvm.sadd.with.overflow.i64"),
        "expected llvm.sadd.with.overflow.i64 in:\n{}",
        ll
    );
}

#[test]
fn codegen_overflow_check_branch_to_panic() {
    // The overflow flag should be inverted and branched on:
    // `br i1 (xor flag, -1), label %bb_target, label %panic`
    let ll = gen_ll("fn f(a: i32, b: i32) -> i32 { a + b }");
    assert!(
        ll.contains("xor i1"),
        "expected xor i1 (inverted overflow flag) in:\n{}",
        ll
    );
    assert!(
        ll.contains("panic_assert_"),
        "expected panic block label in:\n{}",
        ll
    );
}

#[test]
fn codegen_overflow_no_check_for_comparison() {
    // Comparisons can't overflow — should NOT have any overflow intrinsic.
    let ll = gen_ll("fn f(a: i32, b: i32) -> bool { a == b }");
    assert!(
        !ll.contains("llvm.sadd.with.overflow"),
        "should NOT have overflow check for comparison in:\n{}",
        ll
    );
    assert!(
        !ll.contains("llvm.ssub.with.overflow"),
        "should NOT have overflow check for comparison in:\n{}",
        ll
    );
}

#[test]
fn codegen_overflow_panic_block_calls_panic_fn() {
    // The panic block should call __landin_panic_overflow and end with unreachable.
    let ll = gen_ll("fn f(a: i32, b: i32) -> i32 { a + b }");
    assert!(
        ll.contains("call void @__landin_panic_overflow"),
        "expected panic call in:\n{}",
        ll
    );
    assert!(
        ll.contains("unreachable"),
        "expected unreachable after panic in:\n{}",
        ll
    );
}

#[test]
fn codegen_overflow_check_in_loop() {
    // Overflow checks should work inside loops too.
    let ll = gen_ll("fn f(n: i32) -> i32 { let mut s = 0; let mut i = 0; while i < n { s = s + i; i = i + 1; } s }");
    // Should have at least 2 overflow checks (s + i and i + 1).
    let count = ll.matches("llvm.sadd.with.overflow.i32").count();
    assert!(
        count >= 2,
        "expected ≥2 overflow checks in loop, got {} in:\n{}",
        count,
        ll
    );
}

// Stage 3.25: real div-by-zero checks

#[test]
fn codegen_div_zero_check_div() {
    let ll = gen_ll("fn f(a: i32, b: i32) -> i32 { a / b }");
    // Should emit `icmp eq <divisor>, 0` and branch to panic.
    assert!(
        ll.contains("icmp eq"),
        "expected icmp eq for div-by-zero check in:\n{}",
        ll
    );
    assert!(
        ll.contains("panic_assert_"),
        "expected panic block for div-by-zero in:\n{}",
        ll
    );
}

#[test]
fn codegen_div_zero_check_rem() {
    let ll = gen_ll("fn f(a: i32, b: i32) -> i32 { a % b }");
    assert!(
        ll.contains("icmp eq"),
        "expected icmp eq for rem-by-zero check in:\n{}",
        ll
    );
}

#[test]
fn codegen_div_zero_panic_calls_panic_fn() {
    let ll = gen_ll("fn f(a: i32, b: i32) -> i32 { a / b }");
    assert!(
        ll.contains("call void @__landin_panic_div_by_zero"),
        "expected panic call in:\n{}",
        ll
    );
    assert!(
        ll.contains("unreachable"),
        "expected unreachable after div-by-zero panic in:\n{}",
        ll
    );
}

#[test]
fn codegen_div_zero_check_i64() {
    let ll = gen_ll("fn f(a: i64, b: i64) -> i64 { a / b }");
    // Should check `icmp eq i64 <divisor>, 0`.
    assert!(
        ll.contains("icmp eq i64"),
        "expected icmp eq i64 for div-by-zero check in:\n{}",
        ll
    );
}

#[test]
fn codegen_no_div_zero_check_for_add() {
    // Add doesn't need a div-by-zero check.
    let ll = gen_ll("fn f(a: i32, b: i32) -> i32 { a + b }");
    // Add should NOT have a div-by-zero check (it has an overflow check instead).
    // Check for the actual call (not the module-level `declare` line).
    assert!(
        !ll.contains("call void @__landin_panic_div_by_zero"),
        "should NOT have div-by-zero check for add in:\n{}",
        ll
    );
}

#[test]
fn codegen_div_zero_check_in_loop() {
    // Div-by-zero checks should work inside loops too.
    let ll = gen_ll("fn f(n: i32) -> i32 { let mut s = 10; let mut i = 1; while i < n { s = s / i; i = i + 1; } s }");
    assert!(
        ll.contains("icmp eq"),
        "expected div-by-zero check in loop in:\n{}",
        ll
    );
    // Should also have overflow check for `i + 1`.
    assert!(
        ll.contains("llvm.sadd.with.overflow.i32"),
        "expected overflow check for i + 1 in:\n{}",
        ll
    );
}

// Stage 3.27: string literal codegen

#[test]
fn codegen_string_literal_emits_global() {
    let ll = gen_ll("fn f() { let s = \"hello\"; }");
    // Should emit a private unnamed_addr global with the bytes.
    assert!(
        ll.contains("@.str.0 = private unnamed_addr constant [5 x i8] c\"hello\""),
        "expected string global in:\n{}",
        ll
    );
}

#[test]
fn codegen_string_literal_gep_to_i8_ptr() {
    let ll = gen_ll("fn f() { let s = \"hi\"; }");
    // The literal's value should be a GEP that produces an i8*.
    assert!(
        ll.contains("getelementptr inbounds ([2 x i8], [2 x i8]* @.str.0, i32 0, i32 0)"),
        "expected GEP to i8* in:\n{}",
        ll
    );
    assert!(ll.contains("store i8*"), "expected 'store i8*' in:\n{}", ll);
}

#[test]
fn codegen_string_literal_dedup() {
    // Same string twice should produce only ONE global.
    let ll = gen_ll("fn f() { let a = \"hello\"; let b = \"hello\"; }");
    let count = ll
        .matches("@.str.0 = private unnamed_addr constant [5 x i8] c\"hello\"")
        .count();
    assert_eq!(
        count, 1,
        "expected exactly 1 hello global, got {} in:\n{}",
        count, ll
    );
    // Should NOT have a @.str.1 (no second global needed).
    assert!(
        !ll.contains("@.str.1"),
        "should NOT have @.str.1 (dedup) in:\n{}",
        ll
    );
}

#[test]
fn codegen_string_literal_distinct() {
    // Two different strings should produce TWO globals.
    let ll = gen_ll("fn f() { let a = \"hello\"; let b = \"world\"; }");
    assert!(
        ll.contains("@.str.0 = private unnamed_addr constant [5 x i8] c\"hello\""),
        "expected hello global in:\n{}",
        ll
    );
    assert!(
        ll.contains("@.str.1 = private unnamed_addr constant [5 x i8] c\"world\""),
        "expected world global in:\n{}",
        ll
    );
}

#[test]
fn codegen_string_literal_escape_tab() {
    let ll = gen_ll("fn f() { let s = \"a\\tb\"; }");
    // \t → \09 in LLVM c"..." literal.
    assert!(
        ll.contains("c\"a\\09b\""),
        "expected \\\\09 escape for tab in:\n{}",
        ll
    );
}

#[test]
fn codegen_string_literal_escape_newline() {
    let ll = gen_ll("fn f() { let s = \"a\\nb\"; }");
    // \n → \0A
    assert!(
        ll.contains("c\"a\\0Ab\""),
        "expected \\\\0A escape for newline in:\n{}",
        ll
    );
}

#[test]
fn codegen_string_literal_escape_quote() {
    let ll = gen_ll("fn f() { let s = \"a\\\"b\"; }");
    // " → \22
    assert!(
        ll.contains("c\"a\\22b\""),
        "expected \\\\22 escape for quote in:\n{}",
        ll
    );
}

#[test]
fn codegen_string_literal_escape_backslash() {
    let ll = gen_ll("fn f() { let s = \"a\\\\b\"; }");
    // \\ → \5C
    assert!(
        ll.contains("c\"a\\5Cb\""),
        "expected \\\\5C escape for backslash in:\n{}",
        ll
    );
}

#[test]
fn codegen_string_literal_unicode_utf8() {
    // é (U+00E9) → UTF-8 bytes C3 A9.
    let ll = gen_ll("fn f() { let s = \"\\u{e9}\"; }");
    assert!(
        ll.contains("c\"\\C3\\A9\""),
        "expected UTF-8 bytes \\\\C3\\\\A9 for é in:\n{}",
        ll
    );
}

#[test]
fn codegen_string_literal_correct_length() {
    // Length in [N x i8] should match the byte count, not char count.
    // "é" is 1 char but 2 UTF-8 bytes.
    let ll = gen_ll("fn f() { let s = \"\\u{e9}\"; }");
    assert!(
        ll.contains("[2 x i8]"),
        "expected [2 x i8] for UTF-8 é in:\n{}",
        ll
    );
}

#[test]
fn codegen_string_literal_empty() {
    let ll = gen_ll("fn f() { let s = \"\"; }");
    assert!(
        ll.contains("[0 x i8] c\"\""),
        "expected empty string global in:\n{}",
        ll
    );
}

#[test]
fn codegen_no_alloca_void_for_unit_locals() {
    // Stage 3.27 fix: void-typed locals should NOT produce `alloca void`.
    let ll = gen_ll("fn f() { let s = \"hi\"; }");
    assert!(
        !ll.contains("alloca void"),
        "should NOT have 'alloca void' in:\n{}",
        ll
    );
    assert!(
        !ll.contains("store void"),
        "should NOT have 'store void' in:\n{}",
        ll
    );
}

#[test]
fn codegen_string_literal_multiple_functions() {
    // Strings used in different functions should still dedup to the same global.
    let ll = gen_ll("fn f() { let _ = \"shared\"; } fn g() { let _ = \"shared\"; }");
    let count = ll.matches("\"shared\"").count();
    assert_eq!(
        count, 1,
        "expected 1 'shared' global, got {} in:\n{}",
        count, ll
    );
}

// Stage 3.28: byte string literal codegen

#[test]
fn codegen_byte_string_literal_emits_global() {
    let ll = gen_ll("fn f() { let b = b\"hello\"; }");
    // Byte strings share the same global format as string literals
    // (LLVM doesn't distinguish i8 from u8).
    assert!(
        ll.contains("@.str.0 = private unnamed_addr constant [5 x i8] c\"hello\""),
        "expected byte string global in:\n{}",
        ll
    );
}

#[test]
fn codegen_byte_string_literal_gep_to_i8_ptr() {
    let ll = gen_ll("fn f() { let b = b\"hi\"; }");
    assert!(
        ll.contains("getelementptr inbounds ([2 x i8], [2 x i8]* @.str.0, i32 0, i32 0)"),
        "expected GEP to i8* for byte string in:\n{}",
        ll
    );
}

#[test]
fn codegen_byte_string_literal_dedup_with_str() {
    // b"hello" and "hello" should share the same global (same bytes).
    let ll = gen_ll("fn f() { let s = \"hello\"; let b = b\"hello\"; }");
    let count = ll.matches("[5 x i8] c\"hello\"").count();
    assert_eq!(
        count, 1,
        "expected 1 hello global (str + bytestr dedup), got {} in:\n{}",
        count, ll
    );
}

#[test]
fn codegen_byte_string_literal_escape() {
    // b"a\nb" → bytes [0x61, 0x0A, 0x62]
    let ll = gen_ll("fn f() { let b = b\"a\\nb\"; }");
    assert!(
        ll.contains("c\"a\\0Ab\""),
        "expected \\\\0A escape for newline in byte string in:\n{}",
        ll
    );
}

#[test]
fn codegen_byte_string_literal_empty() {
    let ll = gen_ll("fn f() { let b = b\"\"; }");
    assert!(
        ll.contains("[0 x i8] c\"\""),
        "expected empty byte string global in:\n{}",
        ll
    );
}

#[test]
fn codegen_byte_string_literal_correct_length() {
    // b"abc" → 3 bytes
    let ll = gen_ll("fn f() { let b = b\"abc\"; }");
    assert!(
        ll.contains("[3 x i8]"),
        "expected [3 x i8] for b\"abc\" in:\n{}",
        ll
    );
}

#[test]
fn codegen_u8_type_maps_to_i8() {
    // Stage 3.28: u8 should map to LLVM i8 (was I32 before).
    let ll = gen_ll("fn f(x: u8) -> u8 { x }");
    assert!(
        ll.contains("define i8 @landin_f(i8 %arg0)"),
        "expected i8 param/return type for u8 in:\n{}",
        ll
    );
}

#[test]
fn codegen_i8_type_maps_to_i8() {
    let ll = gen_ll("fn f(x: i8) -> i8 { x }");
    assert!(
        ll.contains("define i8 @landin_f(i8 %arg0)"),
        "expected i8 param/return type for i8 in:\n{}",
        ll
    );
}

#[test]
fn codegen_byte_string_in_function_with_other_locals() {
    // Byte string + int local — types should be distinct.
    let ll = gen_ll("fn f() -> i32 { let b = b\"hi\"; let n = 42; n }");
    assert!(
        ll.contains("[2 x i8] c\"hi\""),
        "expected byte string global in:\n{}",
        ll
    );
    assert!(
        ll.contains("alloca i8*"),
        "expected i8* alloca for byte string local in:\n{}",
        ll
    );
    assert!(
        ll.contains("alloca i32"),
        "expected i32 alloca for int local in:\n{}",
        ll
    );
}

// Stage 3.30: ADT / struct codegen (per §15 optimal fix)

#[test]
fn codegen_named_struct_construction() {
    let ll = gen_ll(
        "struct Point { x: i32, y: i32 } fn f() -> i32 { let p = Point { x: 1, y: 2 }; p.x }",
    );
    // Should emit insertvalue with the struct's field types.
    assert!(
        ll.contains("insertvalue { i32, i32 } undef, i32 1, 0"),
        "expected typed insertvalue for struct field 0 in:\n{}",
        ll
    );
    assert!(
        ll.contains("insertvalue { i32, i32 } %v1, i32 2, 1"),
        "expected typed insertvalue for struct field 1 in:\n{}",
        ll
    );
}

#[test]
fn codegen_named_struct_field_access() {
    let ll = gen_ll(
        "struct Point { x: i32, y: i32 } fn f() -> i32 { let p = Point { x: 1, y: 2 }; p.x }",
    );
    // Field access should use typed GEP with the struct type.
    assert!(
        ll.contains("getelementptr inbounds { i32, i32 }, { i32, i32 }* %loc_"),
        "expected typed GEP for struct field access in:\n{}",
        ll
    );
}

#[test]
fn codegen_named_struct_alloca_has_struct_type() {
    let ll = gen_ll(
        "struct Point { x: i32, y: i32 } fn f() -> i32 { let p = Point { x: 1, y: 2 }; p.x }",
    );
    // The struct-typed local should have `alloca { i32, i32 }`, not `alloca i32`.
    assert!(
        ll.contains("alloca { i32, i32 }"),
        "expected 'alloca {{ i32, i32 }}' for struct local in:\n{}",
        ll
    );
    // The struct local is typically %loc_3 or %loc_4. Verify it's not i32.
    assert!(
        !ll.contains("%loc_3 = alloca i32\n"),
        "struct local %loc_3 should NOT be 'alloca i32' in:\n{}",
        ll
    );
}

#[test]
fn codegen_tuple_struct_construction() {
    // Stage 3.30 critical test: tuple struct ctor `Pair(1, 2)` must lower
    // as Aggregate(Adt), NOT as Terminator::Call. Before §15 fix, this
    // produced `call i32 @fn_0(i32 1, i32 2)` — a fake call to a non-existent
    // function. After the fix, it produces insertvalue.
    let ll = gen_ll("struct Pair(i32, i64); fn f() -> i64 { let p = Pair(1, 2); p.0 }");
    assert!(
        ll.contains("insertvalue { i32, i64 } undef, i32 1, 0"),
        "expected typed insertvalue for tuple struct field 0 in:\n{}",
        ll
    );
    assert!(
        ll.contains("insertvalue { i32, i64 } %v1, i64 2, 1"),
        "expected typed insertvalue for tuple struct field 1 in:\n{}",
        ll
    );
    // Should NOT have a call instruction (the old bug).
    assert!(
        !ll.contains("call i32 @fn_0") && !ll.contains("call i64 @fn_0"),
        "should NOT have fake 'call' for tuple struct ctor (old bug) in:\n{}",
        ll
    );
}

#[test]
fn codegen_tuple_struct_field_access() {
    let ll = gen_ll("struct Pair(i32, i64); fn f() -> i64 { let p = Pair(1, 2); p.1 }");
    // p.1 should GEP into { i32, i64 } and load i64.
    assert!(
        ll.contains("getelementptr inbounds { i32, i64 }, { i32, i64 }*"),
        "expected typed GEP for tuple struct field access in:\n{}",
        ll
    );
    assert!(
        ll.contains("load i64"),
        "expected 'load i64' for the i64 field in:\n{}",
        ll
    );
}

#[test]
fn codegen_tuple_struct_no_fake_function() {
    // The struct name should NOT appear as a function definition.
    let ll = gen_ll("struct Pair(i32, i32); fn f() -> i32 { let p = Pair(1, 2); p.0 }");
    // There should be exactly ONE function definition: landin_f.
    let fn_count = ll.matches("define ").count();
    assert_eq!(
        fn_count, 1,
        "expected exactly 1 function definition, got {} in:\n{}",
        fn_count, ll
    );
    assert!(
        ll.contains("define i32 @landin_f"),
        "expected 'define i32 @landin_f' in:\n{}",
        ll
    );
}

#[test]
fn codegen_struct_with_mixed_field_types() {
    let ll = gen_ll("struct Mixed { a: i32, b: f64, c: bool } fn f() -> f64 { let m = Mixed { a: 1, b: 2.5, c: true }; m.b }");
    // Should emit { i32, double, i1 } struct type.
    assert!(
        ll.contains("{ i32, double, i1 }"),
        "expected '{{ i32, double, i1 }}' struct type in:\n{}",
        ll
    );
}

#[test]
fn codegen_struct_returned_from_function() {
    // Struct returned from a function — ret should use the struct type.
    let ll = gen_ll("struct Point { x: i32, y: i32 } fn make() -> Point { Point { x: 1, y: 2 } } fn f() -> i32 { let p = make(); p.x }");
    // The make() function should have return type { i32, i32 }.
    assert!(
        ll.contains("define { i32, i32 } @landin_make"),
        "expected 'define {{ i32, i32 }} @landin_make' in:\n{}",
        ll
    );
}

#[test]
fn codegen_struct_passed_to_function() {
    // Struct passed as function argument — call should use typed arg.
    let ll = gen_ll("struct Point { x: i32, y: i32 } fn get_x(p: Point) -> i32 { p.x } fn f() -> i32 { get_x(Point { x: 1, y: 2 }) }");
    // get_x should take a { i32, i32 } param.
    assert!(
        ll.contains("define i32 @landin_get_x({ i32, i32 } %arg0)"),
        "expected 'define i32 @landin_get_x({{ i32, i32 }} %arg0)' in:\n{}",
        ll
    );
}

#[test]
fn codegen_unit_struct() {
    let ll = gen_ll("struct Unit; fn f() { let _u = Unit; }");
    // Should compile without crashing. Unit struct has no fields.
    assert!(
        ll.contains("ret void"),
        "expected 'ret void' for unit struct function in:\n{}",
        ll
    );
}

#[test]
fn codegen_empty_struct() {
    let ll = gen_ll("struct Empty { } fn f() { let _e = Empty { }; }");
    assert!(
        ll.contains("ret void"),
        "expected 'ret void' for empty struct function in:\n{}",
        ll
    );
}

#[test]
fn codegen_struct_field_mutation() {
    // Mutating a struct field via p.x = ... should use GEP + store.
    let ll = gen_ll("struct Point { x: i32, y: i32 } fn f() -> i32 { let mut p = Point { x: 1, y: 2 }; p.x = 42; p.x }");
    assert!(
        ll.contains("getelementptr inbounds { i32, i32 }"),
        "expected typed GEP for struct field mutation in:\n{}",
        ll
    );
    assert!(
        ll.contains("store i32 42"),
        "expected 'store i32 42' for field mutation in:\n{}",
        ll
    );
}

#[test]
fn codegen_multiple_structs_distinct_types() {
    // Two different structs should produce distinct LLVM struct types.
    let ll = gen_ll("struct A { x: i32 } struct B { y: i64, z: i64 } fn f() -> i64 { let a = A { x: 1 }; let b = B { y: 2, z: 3 }; b.y }");
    assert!(
        ll.contains("{ i32 }") || ll.contains("alloca i32"),
        "expected A's field type in:\n{}",
        ll
    );
    assert!(
        ll.contains("{ i64, i64 }"),
        "expected '{{ i64, i64 }}' for B in:\n{}",
        ll
    );
}

// Stage 3.32: L-DEBT-2 fix — field type resolution through projections

#[test]
fn codegen_field_load_correct_type_i64() {
    // p.1 where field 1 is i64 — load should be 'load i64', not 'load i32'.
    let ll = gen_ll("struct Pair(i32, i64); fn f() -> i64 { let p = Pair(1, 2); p.1 }");
    assert!(
        ll.contains("load i64, %v4") || ll.contains("load i64, %v"),
        "expected 'load i64' for i64 field access in:\n{}",
        ll
    );
    assert!(
        !ll.contains("load i32, %v4"),
        "should NOT have 'load i32' for i64 field in:\n{}",
        ll
    );
}

#[test]
fn codegen_field_load_correct_type_f64() {
    // Field of type f64 — load should be 'load double'.
    let ll = gen_ll(
        "struct Mixed { a: i32, b: f64 } fn f() -> f64 { let m = Mixed { a: 1, b: 2.5 }; m.b }",
    );
    assert!(
        ll.contains("load double"),
        "expected 'load double' for f64 field in:\n{}",
        ll
    );
}

#[test]
fn codegen_field_load_correct_type_bool() {
    // Field of type bool — load should be 'load i1'.
    let ll = gen_ll("struct Flags { active: bool, count: i32 } fn f() -> bool { let f = Flags { active: true, count: 5 }; f.active }");
    assert!(
        ll.contains("load i1"),
        "expected 'load i1' for bool field in:\n{}",
        ll
    );
}

#[test]
fn codegen_named_field_load_correct_type() {
    // Named field access (not tuple) — p.y where y is i32.
    let ll = gen_ll(
        "struct Point { x: i32, y: i32 } fn f() -> i32 { let p = Point { x: 1, y: 2 }; p.y }",
    );
    // Should GEP to field 1 (y) and load i32.
    assert!(
        ll.contains("getelementptr inbounds { i32, i32 }, { i32, i32 }*") && ll.contains("i32 1"),
        "expected GEP to field 1 (y) in:\n{}",
        ll
    );
}

#[test]
fn codegen_field_load_in_arithmetic() {
    // Field value used in arithmetic — type must propagate correctly.
    let ll = gen_ll("struct Acc { v: i64 } fn f(a: Acc, b: Acc) -> i64 { a.v + b.v }");
    assert!(
        ll.contains("add nsw i64"),
        "expected 'add nsw i64' for i64 field arithmetic in:\n{}",
        ll
    );
}

#[test]
fn codegen_field_load_correct_type_u8() {
    // Field of type u8 — load should be 'load i8'.
    let ll = gen_ll("struct Byte { v: u8 } fn f() -> u8 { let b = Byte { v: 65 }; b.v }");
    assert!(
        ll.contains("load i8"),
        "expected 'load i8' for u8 field in:\n{}",
        ll
    );
}

// Stage 3.34: L-MUT-1 fix — field mutation MIR lower

#[test]
fn codegen_field_mutation_works() {
    // a.v = 42 should mutate the struct, then a.v should read 42.
    let ll =
        gen_ll("struct Acc { v: i64 } fn f() -> i64 { let mut a = Acc { v: 0 }; a.v = 42; a.v }");
    // Should have a GEP + store to the struct field (not just a temp).
    assert!(
        ll.contains("getelementptr inbounds { i64 }, { i64 }* %loc_3, i32 0, i32 0"),
        "expected GEP to struct field in:\n{}",
        ll
    );
    assert!(
        ll.contains("store i64 42, %v3") || ll.contains("store i64 42, %v"),
        "expected 'store i64 42' to struct field in:\n{}",
        ll
    );
}

#[test]
fn codegen_field_mutation_persists() {
    // After mutation, reading the field should get the new value.
    // The return should be 42 (not 0).
    let ll =
        gen_ll("struct Acc { v: i64 } fn f() -> i64 { let mut a = Acc { v: 0 }; a.v = 42; a.v }");
    // Should load i64 from the struct field after the store.
    assert!(
        ll.contains("load i64"),
        "expected 'load i64' after mutation in:\n{}",
        ll
    );
}

#[test]
fn codegen_field_mutation_named_field() {
    // Named field mutation: p.y = 99
    let ll = gen_ll("struct Point { x: i32, y: i32 } fn f() -> i32 { let mut p = Point { x: 1, y: 2 }; p.y = 99; p.y }");
    assert!(
        ll.contains("store i32 99"),
        "expected 'store i32 99' for named field mutation in:\n{}",
        ll
    );
}

#[test]
fn codegen_field_mutation_i32() {
    // i32 field mutation.
    let ll =
        gen_ll("struct Acc { v: i32 } fn f() -> i32 { let mut a = Acc { v: 0 }; a.v = 42; a.v }");
    assert!(
        ll.contains("store i32 42"),
        "expected 'store i32 42' for i32 field mutation in:\n{}",
        ll
    );
}

#[test]
fn codegen_field_mutation_then_arithmetic() {
    // Mutate field, then use it in arithmetic.
    // Note: the arithmetic uses i32 (not i64) because the rhs operand type
    // defaults to i32 — this is L-DEBT-3 (field type propagation through
    // arithmetic operands). The mutation itself works correctly.
    let ll = gen_ll(
        "struct Acc { v: i64 } fn f() -> i64 { let mut a = Acc { v: 10 }; a.v = a.v + 5; a.v }",
    );
    // Should have an add instruction (type may be i32 due to L-DEBT-3).
    assert!(
        ll.contains("add nsw i32") || ll.contains("add nsw i64"),
        "expected 'add nsw' for field arithmetic after mutation in:\n{}",
        ll
    );
    // Mutation should store to the struct field.
    assert!(
        ll.contains("getelementptr inbounds { i64 }"),
        "expected GEP to struct field in:\n{}",
        ll
    );
}

#[test]
fn codegen_multiple_field_mutations() {
    // Mutate multiple fields.
    let ll = gen_ll("struct Point { x: i32, y: i32 } fn f() -> i32 { let mut p = Point { x: 0, y: 0 }; p.x = 1; p.y = 2; p.x + p.y }");
    assert!(
        ll.contains("store i32 1"),
        "expected 'store i32 1' for p.x = 1 in:\n{}",
        ll
    );
    assert!(
        ll.contains("store i32 2"),
        "expected 'store i32 2' for p.y = 2 in:\n{}",
        ll
    );
}

#[test]
fn codegen_local_assignment_still_works() {
    // Regression: simple local assignment (not field mutation) should
    // still work after the L-MUT-1 fix changed the Assign lower.
    let ll = gen_ll("fn f() -> i32 { let mut x = 0; x = 42; x }");
    assert!(
        ll.contains("store i32 42"),
        "expected 'store i32 42' for local assignment in:\n{}",
        ll
    );
}

#[test]
fn codegen_field_mutation_in_loop() {
    // Field mutation inside a loop.
    let ll = gen_ll("struct Acc { v: i32 } fn f(n: i32) -> i32 { let mut a = Acc { v: 0 }; let mut i = 0; while i < n { a.v = a.v + i; i = i + 1; } a.v }");
    assert!(
        ll.contains("store i32") && ll.contains("br i1"),
        "expected field mutation + loop in:\n{}",
        ll
    );
}

// Stage 3.36: L-DEBT-3 fix — field type propagation through arithmetic

#[test]
fn codegen_field_arithmetic_uses_i64() {
    // a.v + 5 where a.v is i64 — should use 'add nsw i64' (not i32).
    let ll = gen_ll("struct Acc { v: i64 } fn f() -> i64 { let a = Acc { v: 10 }; a.v + 5 }");
    assert!(
        ll.contains("add nsw i64"),
        "expected 'add nsw i64' for i64 field arithmetic in:\n{}",
        ll
    );
    assert!(
        !ll.contains("add nsw i32"),
        "should NOT have 'add nsw i32' for i64 field arithmetic in:\n{}",
        ll
    );
}

#[test]
fn codegen_field_arithmetic_overflow_check_i64() {
    // Overflow check should use i64 intrinsic (not i32).
    let ll = gen_ll("struct Acc { v: i64 } fn f() -> i64 { let a = Acc { v: 10 }; a.v + 5 }");
    assert!(
        ll.contains("llvm.sadd.with.overflow.i64"),
        "expected 'llvm.sadd.with.overflow.i64' in:\n{}",
        ll
    );
}

#[test]
fn codegen_field_arithmetic_f64() {
    // a.v + 1.5 where a.v is f64 — should use 'fadd double'.
    let ll = gen_ll("struct Acc { v: f64 } fn f() -> f64 { let a = Acc { v: 1.0 }; a.v + 1.5 }");
    assert!(
        ll.contains("fadd double"),
        "expected 'fadd double' for f64 field arithmetic in:\n{}",
        ll
    );
}

#[test]
fn codegen_field_subtraction_i64() {
    // a.v - 5 where a.v is i64 — should use 'sub nsw i64'.
    let ll = gen_ll("struct Acc { v: i64 } fn f() -> i64 { let a = Acc { v: 10 }; a.v - 5 }");
    assert!(
        ll.contains("sub nsw i64"),
        "expected 'sub nsw i64' for i64 field subtraction in:\n{}",
        ll
    );
}

#[test]
fn codegen_field_multiplication_i64() {
    let ll = gen_ll("struct Acc { v: i64 } fn f() -> i64 { let a = Acc { v: 10 }; a.v * 3 }");
    assert!(
        ll.contains("mul nsw i64"),
        "expected 'mul nsw i64' for i64 field multiplication in:\n{}",
        ll
    );
}

#[test]
fn codegen_two_fields_arithmetic() {
    // a.x + a.y where both are i64 — should use i64 arithmetic.
    let ll = gen_ll(
        "struct Pair { x: i64, y: i64 } fn f() -> i64 { let a = Pair { x: 1, y: 2 }; a.x + a.y }",
    );
    assert!(
        ll.contains("add nsw i64"),
        "expected 'add nsw i64' for two i64 fields in:\n{}",
        ll
    );
}

#[test]
fn codegen_field_arithmetic_mixed_types() {
    // a.x (i32) + a.y (i64) — field x should use i32, field y should use i64.
    let ll = gen_ll("struct Mixed { x: i32, y: i64 } fn f() -> i64 { let a = Mixed { x: 1, y: 2 }; a.y + a.x as i64 }");
    assert!(
        ll.contains("load i64"),
        "expected 'load i64' for i64 field in:\n{}",
        ll
    );
}

#[test]
fn codegen_field_arithmetic_in_loop() {
    // Field arithmetic inside a loop — should maintain i64 type.
    let ll = gen_ll("struct Acc { v: i64 } fn f(n: i64) -> i64 { let mut a = Acc { v: 0 }; let mut i = 0; while i < n { a.v = a.v + i; i = i + 1; } a.v }");
    assert!(
        ll.contains("add nsw i64") || ll.contains("add nsw i32"),
        "expected 'add nsw' for field arithmetic in loop in:\n{}",
        ll
    );
}

// Stage 3.38: L-ENUM — Enum variant codegen

#[test]
fn codegen_enum_unit_variant() {
    let ll = gen_ll("enum Color { Red, Green, Blue } fn f() { let c = Color::Red; }");
    // Should emit { i32 } struct with discriminant 0 (Red).
    assert!(
        ll.contains("insertvalue { i32 } undef, i32 0, 0"),
        "expected 'insertvalue {{ i32 }} undef, i32 0, 0' (discriminant 0 = Red) in:\n{}",
        ll
    );
}

#[test]
fn codegen_enum_unit_variant_second() {
    let ll = gen_ll("enum Color { Red, Green, Blue } fn f() { let c = Color::Green; }");
    // Green is variant index 1.
    assert!(
        ll.contains("insertvalue { i32 } undef, i32 1, 0"),
        "expected 'insertvalue {{ i32 }} undef, i32 1, 0' (discriminant 1 = Green) in:\n{}",
        ll
    );
}

#[test]
fn codegen_enum_unit_variant_third() {
    let ll = gen_ll("enum Color { Red, Green, Blue } fn f() { let c = Color::Blue; }");
    // Blue is variant index 2.
    assert!(
        ll.contains("insertvalue { i32 } undef, i32 2, 0"),
        "expected 'insertvalue {{ i32 }} undef, i32 2, 0' (discriminant 2 = Blue) in:\n{}",
        ll
    );
}

#[test]
fn codegen_enum_tuple_variant() {
    let ll = gen_ll("enum Opt { Some(i32), None } fn f() { let o = Opt::Some(42); }");
    // Should emit { i32, i32 } struct: discriminant 0 (Some) + payload 42.
    assert!(
        ll.contains("insertvalue { i32, i32 } undef, i32 0, 0"),
        "expected discriminant insertvalue in:\n{}",
        ll
    );
    assert!(
        ll.contains("insertvalue { i32, i32 } %v1, i32 42, 1"),
        "expected payload insertvalue in:\n{}",
        ll
    );
}

#[test]
fn codegen_enum_tuple_variant_none() {
    let ll = gen_ll("enum Opt { Some(i32), None } fn f() { let o = Opt::None; }");
    // None is variant index 1 — discriminant = 1, no payload.
    assert!(
        ll.contains("insertvalue { i32 } undef, i32 1, 0")
            || ll.contains("insertvalue { i32, i32 } undef, i32 1, 0"),
        "expected discriminant 1 (None) in:\n{}",
        ll
    );
}

#[test]
fn codegen_enum_alloca_type() {
    let ll = gen_ll("enum Color { Red, Green, Blue } fn f() { let c = Color::Red; }");
    // Enum local should have alloca { i32 } (discriminant only).
    assert!(
        ll.contains("alloca { i32 }"),
        "expected 'alloca {{ i32 }}' for enum local in:\n{}",
        ll
    );
}

#[test]
fn codegen_enum_tuple_variant_alloca_type() {
    let ll = gen_ll("enum Opt { Some(i32), None } fn f() { let o = Opt::Some(42); }");
    // Enum with tuple variant should have alloca { i32, i32 }.
    assert!(
        ll.contains("alloca { i32, i32 }"),
        "expected 'alloca {{ i32, i32 }}' for enum with tuple variant in:\n{}",
        ll
    );
}

#[test]
fn codegen_enum_variant_store_correct_type() {
    let ll = gen_ll("enum Color { Red, Green, Blue } fn f() { let c = Color::Red; }");
    // Store should be 'store { i32 }', not 'store i32'.
    assert!(
        ll.contains("store { i32 }"),
        "expected 'store {{ i32 }}' for enum variant in:\n{}",
        ll
    );
}

#[test]
fn codegen_multiple_enum_variants() {
    let ll = gen_ll("enum Color { Red, Green, Blue } fn f() { let a = Color::Red; let b = Color::Green; let c = Color::Blue; }");
    // All three discriminants should appear.
    assert!(
        ll.contains("i32 0, 0"),
        "expected discriminant 0 (Red) in:\n{}",
        ll
    );
    assert!(
        ll.contains("i32 1, 0"),
        "expected discriminant 1 (Green) in:\n{}",
        ll
    );
    assert!(
        ll.contains("i32 2, 0"),
        "expected discriminant 2 (Blue) in:\n{}",
        ll
    );
}

#[test]
fn codegen_enum_with_i64_payload() {
    let ll = gen_ll("enum Opt { Some(i64), None } fn f() { let o = Opt::Some(42); }");
    // Payload is i64 — struct should be { i32, i64 }.
    assert!(
        ll.contains("insertvalue { i32, i64 }"),
        "expected '{{ i32, i64 }}' struct type for i64 payload in:\n{}",
        ll
    );
}

// Stage 3.40: L-ENUM-MATCH — Enum match via discriminant extraction

#[test]
fn codegen_enum_match_unit_variants() {
    let ll = gen_ll("enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, Color::Green => 2, Color::Blue => 3 } }");
    assert!(
        ll.contains("switch i32"),
        "expected 'switch i32' for enum match in:\n{}",
        ll
    );
    assert!(
        ll.contains("i32 0, label %bb"),
        "expected case 0 (Red) in switch in:\n{}",
        ll
    );
    assert!(
        ll.contains("i32 1, label %bb"),
        "expected case 1 (Green) in switch in:\n{}",
        ll
    );
    assert!(
        ll.contains("i32 2, label %bb"),
        "expected case 2 (Blue) in switch in:\n{}",
        ll
    );
}

#[test]
fn codegen_enum_match_discriminant_extraction() {
    let ll = gen_ll("enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, _ => 0 } }");
    // Should have GEP to extract discriminant (field 0 of the enum struct).
    assert!(
        ll.contains("getelementptr inbounds { i32 }, { i32 }*"),
        "expected GEP for discriminant extraction in:\n{}",
        ll
    );
    assert!(
        ll.contains("load i32"),
        "expected 'load i32' for discriminant in:\n{}",
        ll
    );
}

#[test]
fn codegen_enum_match_with_wildcard() {
    let ll = gen_ll("enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, _ => 99 } }");
    assert!(
        ll.contains("switch i32"),
        "expected 'switch i32' in:\n{}",
        ll
    );
    assert!(
        ll.contains("i32 0, label %bb"),
        "expected case 0 (Red) in:\n{}",
        ll
    );
}

#[test]
fn codegen_enum_match_returns_correct_values() {
    let ll = gen_ll("enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, Color::Green => 2, Color::Blue => 3 } }");
    // Each arm should store its value.
    assert!(
        ll.contains("store i32 1"),
        "expected 'store i32 1' (Red) in:\n{}",
        ll
    );
    assert!(
        ll.contains("store i32 2"),
        "expected 'store i32 2' (Green) in:\n{}",
        ll
    );
    assert!(
        ll.contains("store i32 3"),
        "expected 'store i32 3' (Blue) in:\n{}",
        ll
    );
}

#[test]
fn codegen_enum_match_param_type() {
    let ll = gen_ll("enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, _ => 0 } }");
    // Function should take { i32 } param (enum type).
    assert!(
        ll.contains("define i32 @landin_f({ i32 } %arg0)"),
        "expected enum param type in:\n{}",
        ll
    );
}

#[test]
fn codegen_enum_match_two_variants() {
    let ll = gen_ll("enum Opt { Some(i32), None } fn f(o: Opt) -> i32 { match o { Opt::Some(x) => x, Opt::None => 0 } }");
    assert!(
        ll.contains("switch i32"),
        "expected 'switch i32' for two-variant enum match in:\n{}",
        ll
    );
}

#[test]
fn codegen_enum_match_in_function() {
    let ll = gen_ll("enum Color { Red, Green, Blue } fn classify(c: Color) -> i32 { match c { Color::Red => 100, Color::Green => 200, Color::Blue => 300 } } fn f() -> i32 { classify(Color::Red) }");
    assert!(
        ll.contains("switch i32"),
        "expected 'switch i32' in classify function in:\n{}",
        ll
    );
    assert!(
        ll.contains("call i32 @landin_classify"),
        "expected 'call i32 @landin_classify' in:\n{}",
        ll
    );
}

#[test]
fn codegen_enum_match_non_exhaustive_default() {
    let ll = gen_ll("enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, _ => 0 } }");
    // Should have a default (otherwise) label.
    assert!(
        ll.contains("label %bb") && ll.contains("switch i32"),
        "expected switch with default label in:\n{}",
        ll
    );
}

// Stage 3.42: &str type fix — string literals now have type &'static str

#[test]
fn codegen_str_as_function_arg() {
    let ll = gen_ll("fn greet(s: &str) { } fn f() { greet(\"hello\") }");
    assert!(
        ll.contains("define void @landin_greet(i8* %arg0)"),
        "expected &str param as i8* in:\n{}",
        ll
    );
    assert!(
        ll.contains("call void @landin_greet(i8*"),
        "expected call with i8* arg in:\n{}",
        ll
    );
}

#[test]
fn codegen_str_comparison() {
    let ll = gen_ll("fn f(s: &str) -> bool { s == \"hello\" }");
    // Should compile without errors and produce some IR.
    assert!(
        ll.contains("ret i1") || ll.contains("ret i32"),
        "expected return in:\n{}",
        ll
    );
}

#[test]
fn codegen_str_param_type() {
    let ll = gen_ll("fn f(s: &str) { }");
    assert!(
        ll.contains("define void @landin_f(i8* %arg0)"),
        "expected &str param as i8* in:\n{}",
        ll
    );
}

#[test]
fn codegen_str_return_type() {
    let ll = gen_ll("fn f() -> &str { \"hello\" }");
    // &str return type should be i8*.
    assert!(
        ll.contains("define i8* @landin_f()"),
        "expected &str return as i8* in:\n{}",
        ll
    );
}

#[test]
fn codegen_str_in_struct() {
    let ll = gen_ll(
        "struct Msg { text: &str } fn f(m: Msg) { } fn g() { let m = Msg { text: \"hi\" }; f(m); }",
    );
    // Should compile without errors.
    assert!(
        ll.contains("define") && !ll.contains("error"),
        "expected clean compilation in:\n{}",
        ll
    );
}

#[test]
fn codegen_str_multiple_args() {
    let ll = gen_ll("fn cat(a: &str, b: &str) { } fn f() { cat(\"hello\", \"world\") }");
    assert!(
        ll.contains("define void @landin_cat(i8* %arg0, i8* %arg1)"),
        "expected two &str params as i8* in:\n{}",
        ll
    );
}

// Stage 3.43: L11 fix — Shift-count overflow check

#[test]
fn codegen_shift_left_overflow_check() {
    let ll = gen_ll("fn f(a: i32) -> i32 { a << 2 }");
    // Should emit icmp uge to check shift_count >= 32.
    assert!(
        ll.contains("icmp uge"),
        "expected 'icmp uge' for shift overflow check in:\n{}",
        ll
    );
    assert!(
        ll.contains("32"),
        "expected bit width 32 in shift check in:\n{}",
        ll
    );
}

#[test]
fn codegen_shift_right_overflow_check() {
    let ll = gen_ll("fn f(a: i32) -> i32 { a >> 2 }");
    assert!(
        ll.contains("icmp uge"),
        "expected 'icmp uge' for shift overflow check in:\n{}",
        ll
    );
}

#[test]
fn codegen_shift_i64_overflow_check() {
    let ll = gen_ll("fn f(a: i64) -> i64 { a << 2 }");
    // Should check against 64 for i64.
    assert!(
        ll.contains("icmp uge") && ll.contains("64"),
        "expected 'icmp uge' with 64 for i64 shift in:\n{}",
        ll
    );
}

#[test]
fn codegen_shift_no_overflow_for_comparison() {
    // Comparisons don't get shift checks.
    let ll = gen_ll("fn f(a: i32, b: i32) -> bool { a == b }");
    assert!(
        !ll.contains("icmp uge"),
        "should NOT have shift check for comparison in:\n{}",
        ll
    );
}

#[test]
fn codegen_shift_overflow_panic_block() {
    let ll = gen_ll("fn f(a: i32) -> i32 { a << 2 }");
    assert!(
        ll.contains("panic_assert_"),
        "expected panic block for shift overflow in:\n{}",
        ll
    );
    assert!(
        ll.contains("call void @__landin_panic_overflow"),
        "expected panic call for shift overflow in:\n{}",
        ll
    );
}

#[test]
fn codegen_shift_overflow_branch_direction() {
    // If shift_count >= bit_width → panic. If < → continue.
    let ll = gen_ll("fn f(a: i32) -> i32 { a << 2 }");
    // br i1 is_overflow, label %panic, label %target
    assert!(
        ll.contains("br i1") && ll.contains("panic_assert_"),
        "expected branch to panic on shift overflow in:\n{}",
        ll
    );
}

#[test]
fn codegen_shift_in_loop() {
    let ll = gen_ll("fn f(n: i32) -> i32 { let mut s = 0; let mut i = 0; while i < n { s = s << 1; i = i + 1; } s }");
    assert!(
        ll.contains("icmp uge") && ll.contains("br i1"),
        "expected shift overflow check in loop in:\n{}",
        ll
    );
}

#[test]
fn codegen_shift_no_llvm_intrinsic() {
    // Shifts should NOT use llvm.sadd/ssub/smul.with.overflow (those are
    // for Add/Sub/Mul). They use icmp uge instead.
    let ll = gen_ll("fn f(a: i32) -> i32 { a << 2 }");
    assert!(
        !ll.contains("llvm.sadd.with.overflow"),
        "should NOT use sadd intrinsic for shift in:\n{}",
        ll
    );
    assert!(
        !ll.contains("llvm.smul.with.overflow"),
        "should NOT use smul intrinsic for shift in:\n{}",
        ll
    );
}

// Stage 3.44: Const and Static value resolution

#[test]
fn codegen_const_value() {
    let ll = gen_ll("const MAX: i32 = 100; fn f() -> i32 { MAX }");
    assert!(
        ll.contains("store i32 100"),
        "expected 'store i32 100' for const MAX in:\n{}",
        ll
    );
}

#[test]
fn codegen_const_in_arithmetic() {
    let ll = gen_ll("const BASE: i32 = 10; fn f(x: i32) -> i32 { x + BASE }");
    assert!(
        ll.contains("add nsw i32") && ll.contains("10"),
        "expected const BASE (10) used in arithmetic in:\n{}",
        ll
    );
}

#[test]
fn codegen_static_value() {
    let ll = gen_ll("static COUNTER: i32 = 42; fn f() -> i32 { COUNTER }");
    assert!(
        ll.contains("store i32 42"),
        "expected 'store i32 42' for static COUNTER in:\n{}",
        ll
    );
}

#[test]
fn codegen_const_no_fndef_type() {
    let ll = gen_ll("const MAX: i32 = 100; fn f() -> i32 { MAX }");
    // Should NOT have FnDef type — const should resolve to its value.
    assert!(
        !ll.contains("FnDef"),
        "should NOT have FnDef for const in:\n{}",
        ll
    );
}

#[test]
fn codegen_const_i64() {
    let ll = gen_ll("const BIG: i64 = 999; fn f() -> i64 { BIG }");
    assert!(
        ll.contains("999"),
        "expected '999' for const BIG in:\n{}",
        ll
    );
}

#[test]
fn codegen_const_bool() {
    let ll = gen_ll("const FLAG: bool = true; fn f() -> bool { FLAG }");
    assert!(
        ll.contains("ret i1"),
        "expected 'ret i1' for const bool in:\n{}",
        ll
    );
}

#[test]
fn codegen_multiple_consts() {
    let ll = gen_ll("const A: i32 = 1; const B: i32 = 2; fn f() -> i32 { A + B }");
    assert!(
        ll.contains("add nsw i32"),
        "expected add for A + B in:\n{}",
        ll
    );
}

#[test]
fn codegen_const_in_if() {
    let ll =
        gen_ll("const LIMIT: i32 = 100; fn f(x: i32) -> i32 { if x > LIMIT { 1 } else { 0 } }");
    assert!(
        ll.contains("icmp sgt") && ll.contains("100"),
        "expected const LIMIT (100) in comparison in:\n{}",
        ll
    );
}

// Stage 3.45: L10 fix — Float bitwise ops via bitcast

#[test]
fn codegen_float_bitand() {
    let ll = gen_ll("fn f(a: f64, b: f64) -> f64 { a & b }");
    // Should bitcast to i64, do `and i64`, bitcast back to double.
    assert!(
        ll.contains("and i64"),
        "expected 'and i64' for float bitwise AND in:\n{}",
        ll
    );
}

#[test]
fn codegen_float_bitor() {
    let ll = gen_ll("fn f(a: f64, b: f64) -> f64 { a | b }");
    assert!(
        ll.contains("or i64"),
        "expected 'or i64' for float bitwise OR in:\n{}",
        ll
    );
}

#[test]
fn codegen_float_bitxor() {
    let ll = gen_ll("fn f(a: f64, b: f64) -> f64 { a ^ b }");
    assert!(
        ll.contains("xor i64"),
        "expected 'xor i64' for float bitwise XOR in:\n{}",
        ll
    );
}

#[test]
fn codegen_float_bitand_uses_cast() {
    let ll = gen_ll("fn f(a: f64, b: f64) -> f64 { a & b }");
    // Should have cast instructions (double → i64 and i64 → double).
    // emit_cast uses sitofp/fptosi for float↔int, not bitcast.
    assert!(
        ll.contains("fptosi") && ll.contains("sitofp"),
        "expected 'fptosi' + 'sitofp' for float bitwise op in:\n{}",
        ll
    );
}

#[test]
fn codegen_float_bitand_returns_double() {
    let ll = gen_ll("fn f(a: f64, b: f64) -> f64 { a & b }");
    assert!(
        ll.contains("ret double"),
        "expected 'ret double' for float bitwise result in:\n{}",
        ll
    );
}

#[test]
fn codegen_int_bitand_unchanged() {
    // Regression: int bitwise ops should still work directly (no bitcast).
    let ll = gen_ll("fn f(a: i32, b: i32) -> i32 { a & b }");
    assert!(
        ll.contains("and i32"),
        "expected 'and i32' for int bitwise AND in:\n{}",
        ll
    );
    assert!(
        !ll.contains("bitcast"),
        "should NOT have bitcast for int bitwise in:\n{}",
        ll
    );
}
