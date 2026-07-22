//! Round 6 final convergence audit (per §9.3.3 of process v3.4).
//!
//! Run with: cargo run --example round6_audit
//!
//! Purpose: Verify that Round 5's "0 new issues" conclusion is stable
//! by using a FRESH set of test cases (not reusing R5 cases). If R6
//! also finds 0 new issues, the audit is declared CONVERGED and Stage 3
//! may begin per §9.3.3.
//!
//! This audit focuses on:
//!   - Adversarial patterns (combinations designed to break edge cases)
//!   - Real-world code patterns (not just minimal test cases)
//!   - Stress tests (many locals, deep nesting, long chains)
//!   - Idempotency (same program compiled twice gives same result)

use landin_compiler::driver::compile;

fn main() {
    let cases: &[(&str, &str, usize, &str)] = &[
        // ================================================================
        // Group H: Adversarial patterns (10 cases, designed to break edges)
        // ================================================================
        ("h01_nested_not", "fn f() { !!true; }", 0, "double not (Bool)"),
        ("h02_nested_neg", "fn f() { --5; }", 0, "double neg (Int)"),
        ("h03_mixed_arith", "fn f() { 1 + 2 * 3 - 4 / 5; }", 0, "mixed arith precedence"),
        ("h04_chain_compare", "fn f(a: i32, b: i32) -> bool { a < b && b < 10 }", 0, "chain comparison"),
        ("h05_tuple_arith", "fn f() { let t = (1, 2); let u = (3, 4); t + u; }", 1, "tuple + tuple"),
        ("h06_array_in_tuple", "fn f() { let x = ([1, 2], 3); }", 0, "array in tuple"),
        ("h07_borrow_in_arith", "fn f() { let x = 1; let r = &x; *r + 1; }", 0, "borrow in arith"),
        ("h08_move_in_arith", "fn f() { let s = \"hi\"; s + 1; }", 1, "move in arith (Str + Int)"),
        ("h09_let_chain", "fn f() { let a = 1; let b = a; let c = b; let d = c; d }", 0, "let chain (propagation)"),
        ("h10_scope_shadow", "fn f() { let x = 1; let x = true; x }", 0, "scope shadowing"),

        // ================================================================
        // Group I: Real-world patterns (10 cases)
        // ================================================================
        ("i01_factorial",
         "fn fact(n: i64) -> i64 { if n <= 1 { 1 } else { n * fact(n - 1) } }",
         0, "recursive factorial"),
        ("i02_gcd",
         "fn gcd(a: i64, b: i64) -> i64 { if b == 0 { a } else { gcd(b, a % b) } }",
         0, "recursive GCD"),
        ("i03_swap",
         "fn swap(a: i32, b: i32) -> (i32, i32) { (b, a) }",
         0, "swap via tuple"),
        ("i04_iterative_factorial",
         "fn fact(n: i32) -> i32 { let mut r = 1; let mut i = 1; while i <= n { r = r * i; i = i + 1; } r }",
         0, "iterative factorial"),
        ("i05_ackermann",
         "fn ack(m: i32, n: i32) -> i32 { if m == 0 { n + 1 } else if n == 0 { ack(m - 1, 1) } else { ack(m - 1, ack(m, n - 1)) } }",
         0, "Ackermann (nested recursion)"),
        ("i06_boolean_logic",
         "fn f(a: bool, b: bool, c: bool) -> bool { (a || b) && !c }",
         0, "boolean logic"),
        ("i07_conditional_assign",
         "fn f(x: i32) -> i32 { let mut r = 0; if x > 0 { r = x; } else { r = -x; } r }",
         0, "conditional assign"),
        ("i08_match_exhaustive",
         "fn f(x: i32) -> i32 { match x { 0 => 0, 1 => 1, _ => x } }",
         0, "exhaustive match"),
        ("i09_string_use",
         "fn f() { let s = \"hello\"; let t = \"world\"; let _ = s; }",
         0, "string use (borrow, not move)"),
        ("i10_arith_overflow_pattern",
         "fn f(a: i32, b: i32) -> i32 { let s = a + b; let d = a - b; s * d }",
         0, "arith with overflow checks"),

        // ================================================================
        // Group J: Stress tests (5 cases)
        // ================================================================
        ("j01_many_locals",
         "fn f() { let a = 1; let b = 2; let c = 3; let d = 4; let e = 5; let f = 6; let g = 7; let h = 8; let i = 9; let j = 10; a+b+c+d+e+f+g+h+i+j }",
         0, "10 locals + chain arith"),
        ("j02_deep_if",
         "fn f(x: i32) -> i32 { if x > 0 { if x > 1 { if x > 2 { if x > 3 { 1 } else { 2 } } else { 3 } } else { 4 } } else { 5 } }",
         0, "deeply nested if (4 levels)"),
        ("j03_long_chain_arith",
         "fn f() -> i32 { 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10 + 11 + 12 + 13 + 14 + 15 + 16 + 17 + 18 + 19 + 20 }",
         0, "20-element arith chain"),
        ("j04_many_params",
         "fn f(a: i32, b: i32, c: i32, d: i32, e: i32) -> i32 { a + b + c + d + e }",
         0, "5-param function"),
        ("j05_complex_match",
         "fn f(x: i32) -> i32 { match x { 0 => 1, 1 => 2, 2 => 3, 3 => 4, _ => 5 } }",
         0, "5-arm match"),

        // ================================================================
        // Group K: Idempotency / determinism (5 cases)
        // Same program compiled multiple times should give same result.
        // These are positive cases — just verify no crash.
        // ================================================================
        ("k01_compile_twice_1", "fn f() { let x = 1; }", 0, "compile #1"),
        ("k02_compile_twice_2", "fn f() { let x = 1; }", 0, "compile #2 (idempotent)"),
        ("k03_compile_twice_3", "fn f(a: i32) -> i32 { a + 1 }", 0, "compile #3"),
        ("k04_compile_twice_4", "fn f(a: i32) -> i32 { a + 1 }", 0, "compile #4 (idempotent)"),
        ("k05_compile_complex", "fn fib(n: i64) -> i64 { if n < 2 { return n; } fib(n-1) + fib(n-2) }", 0, "complex compile"),

        // ================================================================
        // Group L: Negative regression (10 cases, ensure no false positive)
        // ================================================================
        ("l01_neg_let_mismatch", "fn f() { let x: bool = 42; }", 1, "let mismatch (neg)"),
        ("l02_neg_assign_imm", "fn f() { let x = 1; x = 2; }", 1, "assign imm (neg)"),
        ("l03_neg_double_mut", "fn f() { let mut x = 1; let r1 = &mut x; let r2 = &mut x; }", 1, "double mut (neg)"),
        ("l04_neg_undefined", "fn f() { undefined(); }", 1, "undefined fn (neg)"),
        ("l05_neg_wrong_args", "fn g(a: i32, b: i32) -> i32 { a + b } fn f() { g(1); }", 1, "wrong args (neg)"),
        ("l06_neg_return_mismatch", "fn f() -> bool { 42 }", 1, "return mismatch (neg)"),
        ("l07_neg_int_plus_bool", "fn f() { 1 + true; }", 1, "int + bool (neg)"),
        ("l08_neg_negate_bool", "fn f() { -true; }", 1, "-bool (neg)"),
        ("l09_neg_use_after_move", "fn f() { let s = \"hi\"; let t = s; let u = s; }", 1, "use after move (neg)"),
        ("l10_neg_mut_borrow_imm", "fn f() { let x = 1; let r = &mut x; }", 1, "mut borrow imm (neg)"),
    ];

    let mut missed = 0;
    let mut false_positives = 0;
    let mut ok_count = 0;

    for (name, src, expected, desc) in cases {
        let result = compile(src);
        let actual = result.errors.total_count();
        let status = if expected > &0 {
            if actual >= *expected {
                "OK"
            } else {
                "MISSED"
            }
        } else {
            if actual == 0 {
                "OK"
            } else {
                "FALSE_POS"
            }
        };
        println!(
            "{:40} {:10} exp>={} got={} — {}",
            name, status, expected, actual, desc
        );
        match status {
            "OK" => ok_count += 1,
            "MISSED" => missed += 1,
            "FALSE_POS" => false_positives += 1,
            _ => {}
        }
    }

    println!(
        "\n=== Summary: {} OK, {} missed, {} false_pos ===",
        ok_count, missed, false_positives
    );
    if missed == 0 && false_positives == 0 {
        println!("\n✅ AUDIT CONVERGED — 0 new issues found.");
        println!("   Per §9.3.3, Stage 3 may begin (pending committee vote).");
    } else {
        println!("\n❌ AUDIT NOT CONVERGED — issues found, need another round.");
    }
}
