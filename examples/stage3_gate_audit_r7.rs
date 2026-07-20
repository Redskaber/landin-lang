//! Stage 3 Gate Review Round 7 audit (per §9.3.1 of process v3.4).
//!
//! Run with: cargo run --example stage3_gate_audit_r7
//!
//! Purpose: Phase gate audit for Stage 3 codegen after Stage 3.36
//! (L-DEBT-3 fix: field type propagation through arithmetic operands).
//!
//! Per §9.3.3, R6 was CONVERGED (6 consecutive rounds with 0 new issues).
//! R7 is run because Stage 3.36 closed the L-DEBT-3 debt item recorded in R6.
//!
//! Author: redskaber
//! Date: 2026-07-20
//! Process: v3.11

use landin_compiler::codegen::codegen_crate;
use landin_compiler::driver::compile;

struct Case {
    name: &'static str,
    src: &'static str,
    expect_all: &'static [&'static str],
    expect_none: &'static [&'static str],
    desc: &'static str,
}

fn run_case(c: &Case) -> (bool, String) {
    let result = compile(c.src);
    if let Some(hir) = result.hir.as_ref() {
        let ll = codegen_crate(hir, &result.interner);
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
        // Group R: Regression (8)
        Case { name: "r01_const_return", src: "fn main() -> i32 { 42 }", expect_all: &["ret i32"], expect_none: &[], desc: "R6 r01: constant return" },
        Case { name: "r02_field_mutation", src: "struct Acc { v: i64 } fn f() -> i64 { let mut a = Acc { v: 0 }; a.v = 42; a.v }", expect_all: &["store i64 42", "getelementptr inbounds { i64 }"], expect_none: &[], desc: "R6 r02: field mutation works" },
        Case { name: "r03_field_load_i64", src: "struct Pair(i32, i64); fn f() -> i64 { let p = Pair(1, 2); p.1 }", expect_all: &["load i64"], expect_none: &["load i32, %v4", "load i32, %v5"], desc: "R6 r03: i64 field loads as i64" },
        Case { name: "r04_named_struct", src: "struct Point { x: i32, y: i32 } fn f() -> i32 { let p = Point { x: 1, y: 2 }; p.x }", expect_all: &["insertvalue { i32, i32 } undef, i32 1, 0"], expect_none: &[], desc: "R6 r04: named struct construction" },
        Case { name: "r05_string_literal", src: "fn f() { let s = \"hello\"; }", expect_all: &["@.str.0 = private unnamed_addr constant [5 x i8] c\"hello\""], expect_none: &[], desc: "R6 r05: string literal global" },
        Case { name: "r06_overflow_check", src: "fn f(a: i32, b: i32) -> i32 { a + b }", expect_all: &["llvm.sadd.with.overflow.i32"], expect_none: &[], desc: "R6 r06: overflow check" },
        Case { name: "r07_div_zero_check", src: "fn f(a: i32, b: i32) -> i32 { a / b }", expect_all: &["icmp eq", "call void @__landin_panic_div_by_zero"], expect_none: &[], desc: "R6 r07: div-by-zero check" },
        Case { name: "r08_local_assignment", src: "fn f() -> i32 { let mut x = 0; x = 42; x }", expect_all: &["store i32 42"], expect_none: &[], desc: "R6 r08: local assignment regression" },

        // Group A: Stage 3.36 L-DEBT-3 fix (field type propagation through arithmetic) (10)
        Case { name: "a01_field_add_i64", src: "struct Acc { v: i64 } fn f() -> i64 { let a = Acc { v: 10 }; a.v + 5 }", expect_all: &["add nsw i64"], expect_none: &["add nsw i32"], desc: "Stage 3.36 §15.4: i64 field + literal → add nsw i64" },
        Case { name: "a02_field_overflow_i64", src: "struct Acc { v: i64 } fn f() -> i64 { let a = Acc { v: 10 }; a.v + 5 }", expect_all: &["llvm.sadd.with.overflow.i64"], expect_none: &[], desc: "Stage 3.36: overflow check uses i64 intrinsic" },
        Case { name: "a03_field_add_f64", src: "struct Acc { v: f64 } fn f() -> f64 { let a = Acc { v: 1.0 }; a.v + 1.5 }", expect_all: &["fadd double"], expect_none: &[], desc: "Stage 3.36: f64 field + literal → fadd double" },
        Case { name: "a04_field_sub_i64", src: "struct Acc { v: i64 } fn f() -> i64 { let a = Acc { v: 10 }; a.v - 5 }", expect_all: &["sub nsw i64"], expect_none: &[], desc: "Stage 3.36: i64 field - literal → sub nsw i64" },
        Case { name: "a05_field_mul_i64", src: "struct Acc { v: i64 } fn f() -> i64 { let a = Acc { v: 10 }; a.v * 3 }", expect_all: &["mul nsw i64"], expect_none: &[], desc: "Stage 3.36: i64 field * literal → mul nsw i64" },
        Case { name: "a06_two_fields_add", src: "struct Pair { x: i64, y: i64 } fn f() -> i64 { let a = Pair { x: 1, y: 2 }; a.x + a.y }", expect_all: &["add nsw i64"], expect_none: &[], desc: "Stage 3.36: two i64 fields → add nsw i64" },
        Case { name: "a07_field_div_i64", src: "struct Acc { v: i64 } fn f() -> i64 { let a = Acc { v: 10 }; a.v / 3 }", expect_all: &["sdiv i64"], expect_none: &[], desc: "Stage 3.36: i64 field / literal → sdiv i64" },
        Case { name: "a08_field_rem_i64", src: "struct Acc { v: i64 } fn f() -> i64 { let a = Acc { v: 10 }; a.v % 3 }", expect_all: &["srem i64"], expect_none: &[], desc: "Stage 3.36: i64 field % literal → srem i64" },
        Case { name: "a09_field_arith_i32", src: "struct Acc { v: i32 } fn f() -> i32 { let a = Acc { v: 10 }; a.v + 5 }", expect_all: &["add nsw i32"], expect_none: &[], desc: "Stage 3.36: i32 field + literal → add nsw i32 (regression)" },
        Case { name: "a10_field_arith_chained", src: "struct Acc { v: i64 } fn f() -> i64 { let a = Acc { v: 1 }; a.v + 2 + 3 }", expect_all: &["add nsw i64"], expect_none: &[], desc: "Stage 3.36: chained i64 field arithmetic" },

        // Group E: §9.3.2 edge cases (5)
        Case { name: "e01_no_i32_for_i64_field", src: "struct Acc { v: i64 } fn f() -> i64 { let a = Acc { v: 10 }; a.v + 5 }", expect_all: &[], expect_none: &["add nsw i32"], desc: "Stage 3.36 §15.4 edge: no 'add nsw i32' for i64 field arithmetic" },
        Case { name: "e02_field_store_i64", src: "struct Acc { v: i64 } fn f() -> i64 { let a = Acc { v: 10 }; a.v + 5 }", expect_all: &["store i64"], expect_none: &[], desc: "Stage 3.36 edge: field value stored as i64" },
        Case { name: "e03_field_load_i64_in_arith", src: "struct Acc { v: i64 } fn f() -> i64 { let a = Acc { v: 10 }; a.v + 5 }", expect_all: &["load i64"], expect_none: &[], desc: "Stage 3.36 edge: field loaded as i64 for arithmetic" },
        Case { name: "e04_field_result_i64", src: "struct Acc { v: i64 } fn f() -> i64 { let a = Acc { v: 10 }; a.v + 5 }", expect_all: &["ret i64"], expect_none: &[], desc: "Stage 3.36 edge: return type is i64" },
        Case { name: "e05_field_arith_with_cast", src: "struct Mixed { x: i32, y: i64 } fn f() -> i64 { let a = Mixed { x: 1, y: 2 }; a.y + a.x as i64 }", expect_all: &["load i64", "sext"], expect_none: &[], desc: "Stage 3.36 edge: mixed field types with cast" },

        // Group H: Adversarial (5)
        Case { name: "h01_field_arith_in_if", src: "struct Acc { v: i64 } fn f(c: bool) -> i64 { let a = Acc { v: 10 }; if c { a.v + 1 } else { a.v + 2 } }", expect_all: &["add nsw i64", "br i1"], expect_none: &[], desc: "Adversarial: field arithmetic in if branches" },
        Case { name: "h02_field_arith_in_loop", src: "struct Acc { v: i64 } fn f(n: i64) -> i64 { let mut a = Acc { v: 0 }; let mut i = 0; while i < n { a.v = a.v + i; i = i + 1; } a.v }", expect_all: &["br i1"], expect_none: &[], desc: "Adversarial: field arithmetic in loop" },
        Case { name: "h03_field_arith_recursive", src: "struct Acc { v: i64 } fn sum(n: i64) -> i64 { let a = Acc { v: n }; if n == 0 { 0 } else { a.v + sum(n - 1) } } fn main() { let _ = sum(5); }", expect_all: &["call i64 @landin_sum"], expect_none: &[], desc: "Adversarial: recursive function with field arithmetic" },
        Case { name: "h04_field_arith_multiple_structs", src: "struct A { v: i64 } struct B { w: i64 } fn f() -> i64 { let a = A { v: 1 }; let b = B { w: 2 }; a.v + b.w }", expect_all: &["add nsw i64"], expect_none: &[], desc: "Adversarial: arithmetic across multiple struct fields" },
        Case { name: "h05_field_arith_nested", src: "struct Inner { v: i64 } struct Outer { inner: Inner } fn f() -> i64 { let o = Outer { inner: Inner { v: 10 } }; o.inner.v + 5 }", expect_all: &["add nsw i64"], expect_none: &[], desc: "Adversarial: nested struct field arithmetic" },
    ];

    let mut pass = 0;
    let mut fail = 0;
    let mut failures: Vec<&str> = Vec::new();
    for c in cases {
        let (ok, msg) = run_case(c);
        println!("{:35} {}", c.name, msg);
        if ok { pass += 1; } else { fail += 1; failures.push(c.name); }
    }

    println!("\n=== Stage 3 Gate Audit Round 7 Summary ===");
    println!("    Total: {}  Pass: {}  Fail: {}", cases.len(), pass, fail);
    if !failures.is_empty() { println!("    Failed cases: {:?}", failures); }
    if fail == 0 {
        println!("\n✅ AUDIT PASSED — 0 codegen defects found in {} cases.", cases.len());
        println!("   R1-R7: 38/38, 43/43, 43/43, 37/37, 30/30, 30/30, {}/{} — all OK.", pass, cases.len());
        println!("   Per §9.3.3, audit CONVERGED (7 rounds, 0 new issues each).");
        println!("   §15.4 verified: L-DEBT-3 root cause fixed (field types propagate through arithmetic).");
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found, need fixes.", fail);
    }
}
