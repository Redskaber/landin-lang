//! Stage 3 Gate Review Round 1 audit (per §9.3.1 of process v3.4).
//!
//! Run with: cargo run --example stage3_gate_audit
//!
//! Purpose: Phase gate audit for Stage 3 codegen (sub-stages 3.1-3.22).
//! Verifies that the generated LLVM IR has the expected instructions,
//! types, and structure for each category of input.
//!
//! Groups (38 cases total, ≥30 per §9.3.1):
//!   A: Single-stmt codegen correctness (10)
//!   B: Multi-stmt control flow + calls (10)
//!   C: Complex real-world programs (8)
//!   E: §9.3.2 edge cases for Stage 3.21 + 3.22 fixes (5)
//!   D: Robustness / error recovery (5)
//!
//! Author: redskaber
//! Date: 2026-07-20
//! Process: v3.7

use landin_compiler::codegen::codegen_crate;
use landin_compiler::driver::compile;

/// Audit case: name, source, expected IR substrings (ALL must be present),
/// forbidden IR substrings (NONE may be present), description.
struct Case {
    name: &'static str,
    src: &'static str,
    expect_all: &'static [&'static str],
    expect_none: &'static [&'static str],
    desc: &'static str,
}

fn run_case(c: &Case) -> (bool, String) {
    let result = compile(c.src);
    if let Some(_hir) = result.hir.as_ref() {
        let ll = codegen_crate(&result);
        let mut missing = Vec::new();
        for s in c.expect_all {
            if !ll.contains(s) {
                missing.push(*s);
            }
        }
        let mut forbidden = Vec::new();
        for s in c.expect_none {
            if ll.contains(s) {
                forbidden.push(*s);
            }
        }
        if missing.is_empty() && forbidden.is_empty() {
            (true, format!("OK   — {}", c.desc))
        } else {
            let mut msg = format!("FAIL — {}", c.desc);
            if !missing.is_empty() {
                msg.push_str(&format!("\n       missing: {:?}", missing));
            }
            if !forbidden.is_empty() {
                msg.push_str(&format!("\n       forbidden-found: {:?}", forbidden));
            }
            (false, msg)
        }
    } else {
        (false, format!("FAIL — no HIR produced: {}", c.desc))
    }
}

fn main() {
    let cases: &[Case] = &[
        // ============================================================
        // Group A: Single-stmt codegen correctness (10 cases)
        // ============================================================
        Case {
            name: "a01_const_return",
            src: "fn main() -> i32 { 42 }",
            expect_all: &["ret i32"],
            expect_none: &[],
            desc: "constant return produces 'ret i32'",
        },
        Case {
            name: "a02_int_add",
            src: "fn f() -> i32 { 1 + 2 }",
            expect_all: &["add nsw i32"],
            expect_none: &[],
            desc: "int addition emits 'add nsw i32'",
        },
        Case {
            name: "a03_float_add",
            src: "fn f() -> f64 { 1.5 + 2.5 }",
            expect_all: &["fadd double"],
            expect_none: &[],
            desc: "float addition emits 'fadd double'",
        },
        Case {
            name: "a04_bool_constant",
            src: "fn f() -> bool { true }",
            expect_all: &["ret i1"],
            expect_none: &[],
            desc: "bool return uses i1",
        },
        Case {
            name: "a05_let_alloca",
            src: "fn f() -> i32 { let x = 42; x }",
            expect_all: &["alloca i32", "store i32"],
            expect_none: &[],
            desc: "let binding emits alloca + store",
        },
        Case {
            name: "a06_borrow_returns_ptr",
            src: "fn f() -> i32 { let x = 42; let r = &x; *r }",
            expect_all: &["alloca i32", "load i32"],
            expect_none: &[],
            desc: "borrow returns alloca pointer, deref loads",
        },
        Case {
            name: "a07_eq_cmp",
            src: "fn f(a: i32, b: i32) -> bool { a == b }",
            expect_all: &["icmp eq", "zext i1"],
            expect_none: &[],
            desc: "equality emits icmp + zext",
        },
        Case {
            name: "a08_neg_int",
            src: "fn f() -> i32 { -5 }",
            expect_all: &["sub i32 0"],
            expect_none: &[],
            desc: "int negation emits 'sub i32 0'",
        },
        Case {
            name: "a09_not_bool",
            src: "fn f(b: bool) -> bool { !b }",
            expect_all: &["xor i1"],
            expect_none: &[],
            desc: "bool not emits 'xor i1 ..., -1'",
        },
        Case {
            name: "a10_i64_arith",
            src: "fn f(a: i64, b: i64) -> i64 { a + b }",
            expect_all: &["add nsw i64"],
            expect_none: &[],
            desc: "i64 arithmetic uses i64 type",
        },
        // ============================================================
        // Group B: Multi-stmt control flow + calls (10 cases)
        // ============================================================
        Case {
            name: "b01_if_else_branch",
            src: "fn f(x: i32) -> i32 { if x > 0 { 1 } else { 2 } }",
            expect_all: &["br i1", "icmp sgt"],
            expect_none: &["ret i32 2"],
            desc: "if-else emits br i1; merge doesn't leak false-branch value",
        },
        Case {
            name: "b02_while_loop",
            src: "fn f() -> i32 { let mut i = 0; while i < 10 { i = i + 1; } i }",
            expect_all: &["br i1", "icmp slt"],
            expect_none: &[],
            desc: "while loop emits conditional branch",
        },
        Case {
            name: "b03_match_switch",
            src: "fn f(x: i32) -> i32 { match x { 0 => 1, 1 => 2, _ => 3 } }",
            expect_all: &["switch i32"],
            expect_none: &[],
            desc: "match on int emits LLVM switch",
        },
        Case {
            name: "b04_simple_call",
            src: "fn g(a: i32) -> i32 { a } fn f() -> i32 { g(42) }",
            expect_all: &["call i32 @landin_g(i32 42)"],
            expect_none: &[],
            desc: "function call emits 'call i32 @landin_g(i32 42)'",
        },
        Case {
            name: "b05_call_mixed_types",
            src: "fn g(a: i32, b: i64) -> i64 { b } fn f() -> i64 { g(1, 2) }",
            expect_all: &["call i64 @landin_g(i32 1, i64 2)"],
            expect_none: &[],
            desc: "call with mixed-typed args emits correctly typed call",
        },
        Case {
            name: "b06_param_signature",
            src: "fn add(a: i32, b: i32) -> i32 { a + b }",
            expect_all: &["define i32 @landin_add(i32 %arg0, i32 %arg1)"],
            expect_none: &[],
            desc: "params appear in function signature",
        },
        Case {
            name: "b07_recursive_call",
            src: "fn fib(n: i64) -> i64 { if n < 2 { return n; } fib(n - 1) + fib(n - 2) }",
            expect_all: &["call i64 @landin_fib", "br i1"],
            expect_none: &[],
            desc: "recursive call works",
        },
        Case {
            name: "b08_nested_if",
            src: "fn f(x: i32) -> i32 { if x > 0 { if x > 10 { 100 } else { 1 } } else { 2 } }",
            expect_all: &["br i1", "ret i32 %v"],
            expect_none: &["ret i32 2"],
            desc: "nested if has multiple br i1; merge loads result",
        },
        Case {
            name: "b09_tuple_construction",
            src: "fn f() -> f64 { let t = (1, 2.5); t.1 }",
            expect_all: &["insertvalue { i32, double }"],
            expect_none: &[],
            desc: "tuple construction uses typed insertvalue",
        },
        Case {
            name: "b10_array_construction",
            src: "fn f() -> i32 { let a = [10, 20, 30]; let i = 0; a[i] }",
            expect_all: &["insertvalue [3 x i32]"],
            expect_none: &["[10 x i32]"],
            desc: "array construction uses correct length [3 x i32]",
        },
        // ============================================================
        // Group C: Complex real-world programs (8 cases)
        // ============================================================
        Case {
            name: "c01_fib_recursive",
            src: "fn fib(n: i64) -> i64 { if n < 2 { return n; } fib(n - 1) + fib(n - 2) } fn main() { let r = fib(10); }",
            expect_all: &["call i64 @landin_fib", "br i1"],
            expect_none: &[],
            desc: "recursive Fibonacci compiles correctly",
        },
        Case {
            name: "c02_factorial_iterative",
            src: "fn fact(n: i32) -> i32 { let mut r = 1; let mut i = 1; while i <= n { r = r * i; i = i + 1; } r }",
            expect_all: &["br i1", "mul nsw i32", "add nsw i32"],
            expect_none: &[],
            desc: "iterative factorial compiles",
        },
        Case {
            name: "c03_gcd_recursive",
            src: "fn gcd(a: i64, b: i64) -> i64 { if b == 0 { a } else { gcd(b, a % b) } }",
            expect_all: &["call i64 @landin_gcd", "icmp eq", "srem i64"],
            expect_none: &[],
            desc: "recursive GCD compiles",
        },
        Case {
            name: "c04_ackermann",
            src: "fn ack(m: i32, n: i32) -> i32 { if m == 0 { n + 1 } else if n == 0 { ack(m - 1, 1) } else { ack(m - 1, ack(m, n - 1)) } }",
            expect_all: &["call i32 @landin_ack"],
            expect_none: &[],
            desc: "Ackermann (nested recursion) compiles",
        },
        Case {
            name: "c05_nested_match",
            src: "fn f(x: i32, y: i32) -> i32 { match x { 0 => match y { 0 => 0, _ => 1 }, _ => 2 } }",
            expect_all: &["switch i32"],
            expect_none: &[],
            desc: "nested match compiles",
        },
        Case {
            name: "c06_borrow_chain",
            src: "fn f() -> i32 { let x = 42; let r = &x; let r2 = r; *r2 }",
            expect_all: &["load i32"],
            expect_none: &[],
            desc: "chained borrows deref correctly",
        },
        Case {
            name: "c07_tuple_destructure",
            src: "fn f() -> i32 { let t = (1, 2, 3); let a = t.0; let b = t.1; a + b }",
            expect_all: &["getelementptr inbounds { i32, i32, i32 }"],
            expect_none: &[],
            desc: "tuple field access uses typed GEP",
        },
        Case {
            name: "c08_cast_chain",
            src: "fn f(x: i32) -> f64 { let y = x as i64; y as f64 }",
            expect_all: &["sext", "sitofp"],
            expect_none: &[],
            desc: "cast chain i32 → i64 → f64 emits sext + sitofp",
        },
        // ============================================================
        // Group E: §9.3.2 edge cases for Stage 3.21 + 3.22 fixes (5 cases)
        // ============================================================
        Case {
            name: "e01_tuple_mixed_types_321",
            src: "fn f() -> bool { let t = (1, 2.5, true); t.2 }",
            expect_all: &["{ i32, double, i1 }"],
            expect_none: &["{ i32, i32 }"],
            desc: "Stage 3.21: tuple (i32, f64, bool) has correct LLVM struct type",
        },
        Case {
            name: "e02_array_i64_321",
            src: "fn f() -> i64 { let a: [i64; 3] = [1, 2, 3]; let i = 0; a[i] }",
            expect_all: &["[3 x i64]"],
            expect_none: &["[10 x i32]"],
            desc: "Stage 3.21: array of i64 has correct LLVM array type",
        },
        Case {
            name: "e03_typed_call_i64_321",
            src: "fn g(a: i64) -> i64 { a } fn f() -> i64 { g(42) }",
            expect_all: &["call i64 @landin_g(i64 42)"],
            expect_none: &["call i64 @landin_g(i32 42)"],
            desc: "Stage 3.21: i64 call arg has i64 type, not i32",
        },
        Case {
            name: "e04_if_merge_correctness_322",
            src: "fn f(x: i32) -> i32 { if x > 0 { 1 } else { 2 } }",
            expect_all: &["ret i32 %v"],
            expect_none: &["ret i32 1", "ret i32 2"],
            desc: "Stage 3.22: if-else return loads merged value, not constant",
        },
        Case {
            name: "e05_nested_if_merge_322",
            src: "fn f(x: i32) -> i32 { if x > 0 { if x > 10 { 100 } else { 1 } } else { 2 } }",
            expect_all: &["ret i32 %v"],
            expect_none: &["ret i32 1", "ret i32 2", "ret i32 100"],
            desc: "Stage 3.22: nested if-else return loads merged value",
        },
        // ============================================================
        // Group D: Robustness / error recovery (5 cases)
        // ============================================================
        Case {
            name: "d01_empty_function",
            src: "fn f() { }",
            expect_all: &["ret void"],
            expect_none: &[],
            desc: "empty function emits 'ret void'",
        },
        Case {
            name: "d02_unit_return_explicit",
            src: "fn f(x: i32) { let _ = x; }",
            expect_all: &["ret void"],
            expect_none: &[],
            desc: "function with implicit unit return emits 'ret void'",
        },
        Case {
            name: "d03_large_array",
            src: "fn f() -> i32 { let a = [1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16]; let i = 0; a[i] }",
            expect_all: &["[16 x i32]"],
            expect_none: &["[10 x i32]"],
            desc: "large array has correct length (not hardcoded 10)",
        },
        Case {
            name: "d04_deep_match",
            src: "fn f(x: i32) -> i32 { match x { 0 => 0, 1 => 1, 2 => 2, 3 => 3, 4 => 4, 5 => 5, _ => 99 } }",
            expect_all: &["switch i32"],
            expect_none: &[],
            desc: "deep match (7 arms) compiles without crash",
        },
        Case {
            name: "d05_long_arith_chain",
            src: "fn f() -> i32 { 1+2+3+4+5+6+7+8+9+10+11+12+13+14+15+16+17+18+19+20 }",
            expect_all: &["add nsw i32"],
            expect_none: &[],
            desc: "20-element arith chain doesn't crash codegen",
        },
    ];

    let mut pass = 0;
    let mut fail = 0;
    let mut failures: Vec<&str> = Vec::new();
    for c in cases {
        let (ok, msg) = run_case(c);
        println!("{:30} {}", c.name, msg);
        if ok {
            pass += 1;
        } else {
            fail += 1;
            failures.push(c.name);
        }
    }

    println!("\n=== Stage 3 Gate Audit Round 1 Summary ===");
    println!("    Total: {}  Pass: {}  Fail: {}", cases.len(), pass, fail);
    if !failures.is_empty() {
        println!("    Failed cases: {:?}", failures);
    }
    if fail == 0 {
        println!("\n✅ AUDIT PASSED — 0 codegen defects found in 38 cases.");
        println!("   Per §9.3, Stage 3 gate review may proceed to committee vote.");
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found, need fixes.", fail);
    }
}
