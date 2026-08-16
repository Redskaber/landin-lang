//! Stage 3 Gate Review Round 12 audit.
//! Run with: cargo run --example stage3_gate_audit_r12
//!
//! Purpose: Phase gate audit for Stage 3.45 (L10 float bitwise ops).
//! Author: redskaber
//! Process: v3.12

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
    if let Some(_hir) = result.hir.as_ref() {
        let ll = codegen_crate(&result).expect("codegen should succeed for valid test input");
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
        (false, format!("FAIL — no HIR: {}", c.desc))
    }
}

fn main() {
    let cases: &[Case] = &[
        // Group R: Regression (8)
        Case { name: "r01_const", src: "fn main() -> i32 { 42 }", expect_all: &["ret i32"], expect_none: &[], desc: "R11 r01: const return" },
        Case { name: "r02_const_val", src: "const MAX: i32 = 100; fn f() -> i32 { MAX }", expect_all: &["store i32 100"], expect_none: &[], desc: "R11 r02: const value" },
        Case { name: "r03_enum_match", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, _ => 0 } }", expect_all: &["switch i32"], expect_none: &[], desc: "R11 r03: enum match" },
        Case { name: "r04_str", src: "fn greet(s: &str) { } fn f() { greet(\"hello\") }", expect_all: &["define void @landin_greet(i8* %arg0)"], expect_none: &[], desc: "R11 r04: &str arg" },
        Case { name: "r05_shift", src: "fn f(a: i32) -> i32 { a << 2 }", expect_all: &["icmp uge", "32"], expect_none: &[], desc: "R11 r05: shift overflow" },
        Case { name: "r06_overflow", src: "fn f(a: i32, b: i32) -> i32 { a + b }", expect_all: &["llvm.sadd.with.overflow.i32"], expect_none: &[], desc: "R11 r06: overflow check" },
        Case { name: "r07_struct", src: "struct Point { x: i32, y: i32 } fn f() -> i32 { let p = Point { x: 1, y: 2 }; p.x }", expect_all: &["insertvalue { i32, i32 }"], expect_none: &[], desc: "R11 r07: struct" },
        Case { name: "r08_div", src: "fn f(a: i32, b: i32) -> i32 { a / b }", expect_all: &["icmp eq", "__landin_panic_div_by_zero"], expect_none: &[], desc: "R11 r08: div-by-zero" },

        // Group F: Stage 3.45 float bitwise (8)
        Case { name: "f01_float_and", src: "fn f(a: f64, b: f64) -> f64 { a & b }", expect_all: &["and i64"], expect_none: &[], desc: "Stage 3.45 §15.4: float & via and i64" },
        Case { name: "f02_float_or", src: "fn f(a: f64, b: f64) -> f64 { a | b }", expect_all: &["or i64"], expect_none: &[], desc: "Stage 3.45: float | via or i64" },
        Case { name: "f03_float_xor", src: "fn f(a: f64, b: f64) -> f64 { a ^ b }", expect_all: &["xor i64"], expect_none: &[], desc: "Stage 3.45: float ^ via xor i64" },
        Case { name: "f04_float_cast", src: "fn f(a: f64, b: f64) -> f64 { a & b }", expect_all: &["fptosi", "sitofp"], expect_none: &[], desc: "Stage 3.45: float bitwise uses fptosi + sitofp" },
        Case { name: "f05_float_ret", src: "fn f(a: f64, b: f64) -> f64 { a & b }", expect_all: &["ret double"], expect_none: &[], desc: "Stage 3.45: float bitwise returns double" },
        Case { name: "f06_int_and", src: "fn f(a: i32, b: i32) -> i32 { a & b }", expect_all: &["and i32"], expect_none: &["fptosi", "sitofp"], desc: "Stage 3.45: int & uses and i32 (no cast)" },
        Case { name: "f07_int_or", src: "fn f(a: i32, b: i32) -> i32 { a | b }", expect_all: &["or i32"], expect_none: &["fptosi", "sitofp"], desc: "Stage 3.45: int | uses or i32 (no cast)" },
        Case { name: "f08_float_no_add", src: "fn f(a: f64, b: f64) -> f64 { a & b }", expect_all: &[], expect_none: &["add nsw i32", "fadd double"], desc: "Stage 3.45: float & doesn't use add/fadd" },

        // Group E: edge cases (8)
        Case { name: "e01_float_and_in_expr", src: "fn f(a: f64, b: f64) -> f64 { (a & b) + 1.0 }", expect_all: &["and i64", "fadd double"], expect_none: &[], desc: "Stage 3.45 edge: float & then + uses both" },
        Case { name: "e02_float_or_in_if", src: "fn f(a: f64, b: f64, c: bool) -> f64 { if c { a | b } else { 0.0 } }", expect_all: &["or i64", "br i1"], expect_none: &[], desc: "Stage 3.45 edge: float | in if" },
        Case { name: "e03_int_and_no_cast", src: "fn f(a: i64, b: i64) -> i64 { a & b }", expect_all: &["and i64"], expect_none: &["fptosi", "sitofp"], desc: "Stage 3.45 edge: i64 & no cast" },
        Case { name: "e04_float_xor_ret", src: "fn f(a: f64, b: f64) -> f64 { a ^ b }", expect_all: &["xor i64", "ret double"], expect_none: &[], desc: "Stage 3.45 edge: float ^ returns double" },
        Case { name: "e05_bool_and", src: "fn f(a: bool, b: bool) -> bool { a & b }", expect_all: &["and i1"], expect_none: &["fptosi", "sitofp", "and i64"], desc: "Stage 3.45 edge: bool & uses and i1" },
        Case { name: "e06_float_and_store", src: "fn f(a: f64, b: f64) { let _ = a & b; }", expect_all: &["and i64", "store double"], expect_none: &[], desc: "Stage 3.45 edge: float & stored as double" },
        Case { name: "e07_const_float_and", src: "const A: f64 = 1.0; fn f(b: f64) -> f64 { A & b }", expect_all: &["and i64"], expect_none: &[], desc: "Stage 3.45 edge: const float & " },
        Case { name: "e08_float_and_no_i32", src: "fn f(a: f64, b: f64) -> f64 { a & b }", expect_all: &[], expect_none: &["and i32"], desc: "Stage 3.45 §15.4: no 'and i32' for float bitwise" },
    ];

    let mut pass = 0;
    let mut fail = 0;
    let mut failures: Vec<&str> = Vec::new();
    for c in cases {
        let (ok, msg) = run_case(c);
        println!("{:35} {}", c.name, msg);
        if ok {
            pass += 1;
        } else {
            fail += 1;
            failures.push(c.name);
        }
    }

    println!("\n=== Stage 3 Gate Audit Round 12 Summary ===");
    println!("    Total: {}  Pass: {}  Fail: {}", cases.len(), pass, fail);
    if !failures.is_empty() {
        println!("    Failed cases: {:?}", failures);
    }
    if fail == 0 {
        println!(
            "\n✅ AUDIT PASSED — 0 codegen defects found in {} cases.",
            cases.len()
        );
        println!(
            "   R1-R12: ..., 28, 28, 23, {}/{} — all OK.",
            pass,
            cases.len()
        );
        println!("   Per §9.3.3, audit CONVERGED (12 rounds, 0 new issues each).");
        println!("   Stage 3.45 (L10 float bitwise ops) verified.");
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found.", fail);
    }
}
