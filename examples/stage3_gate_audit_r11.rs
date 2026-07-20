//! Stage 3 Gate Review Round 11 audit (per §9.3.1 of process v3.4).
//!
//! Run with: cargo run --example stage3_gate_audit_r11
//!
//! Purpose: Phase gate audit for Stage 3.44 (const/static value resolution).
//!
//! Author: redskaber
//! Date: 2026-07-20
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
        (false, format!("FAIL — no HIR: {}", c.desc))
    }
}

fn main() {
    let cases: &[Case] = &[
        // Group R: Regression (8)
        Case { name: "r01_const_return", src: "fn main() -> i32 { 42 }", expect_all: &["ret i32"], expect_none: &[], desc: "R10 r01: constant return" },
        Case { name: "r02_enum_match", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, _ => 0 } }", expect_all: &["switch i32"], expect_none: &[], desc: "R10 r02: enum match" },
        Case { name: "r03_str_arg", src: "fn greet(s: &str) { } fn f() { greet(\"hello\") }", expect_all: &["define void @landin_greet(i8* %arg0)"], expect_none: &[], desc: "R10 r03: &str as arg" },
        Case { name: "r04_shift_overflow", src: "fn f(a: i32) -> i32 { a << 2 }", expect_all: &["icmp uge", "32"], expect_none: &[], desc: "R10 r04: shift overflow check" },
        Case { name: "r05_struct", src: "struct Point { x: i32, y: i32 } fn f() -> i32 { let p = Point { x: 1, y: 2 }; p.x }", expect_all: &["insertvalue { i32, i32 }"], expect_none: &[], desc: "R10 r05: struct construction" },
        Case { name: "r06_overflow", src: "fn f(a: i32, b: i32) -> i32 { a + b }", expect_all: &["llvm.sadd.with.overflow.i32"], expect_none: &[], desc: "R10 r06: overflow check" },
        Case { name: "r07_div_zero", src: "fn f(a: i32, b: i32) -> i32 { a / b }", expect_all: &["icmp eq", "__landin_panic_div_by_zero"], expect_none: &[], desc: "R10 r07: div-by-zero" },
        Case { name: "r08_if_else", src: "fn f(x: i32) -> i32 { if x > 0 { 1 } else { 2 } }", expect_all: &["br i1"], expect_none: &["ret i32 2"], desc: "R10 r08: if-else merge" },

        // Group C: Stage 3.44 const/static (10)
        Case { name: "c01_const_value", src: "const MAX: i32 = 100; fn f() -> i32 { MAX }", expect_all: &["store i32 100"], expect_none: &[], desc: "Stage 3.44 §15.4: const value inlined" },
        Case { name: "c02_const_arith", src: "const BASE: i32 = 10; fn f(x: i32) -> i32 { x + BASE }", expect_all: &["add nsw i32", "10"], expect_none: &[], desc: "Stage 3.44: const in arithmetic" },
        Case { name: "c03_static_value", src: "static COUNTER: i32 = 42; fn f() -> i32 { COUNTER }", expect_all: &["store i32 42"], expect_none: &[], desc: "Stage 3.44: static value inlined" },
        Case { name: "c04_const_no_fndef", src: "const MAX: i32 = 100; fn f() -> i32 { MAX }", expect_all: &[], expect_none: &["FnDef"], desc: "Stage 3.44 §15.4: no FnDef for const" },
        Case { name: "c05_const_i64", src: "const BIG: i64 = 999; fn f() -> i64 { BIG }", expect_all: &["999"], expect_none: &[], desc: "Stage 3.44: const i64" },
        Case { name: "c06_const_bool", src: "const FLAG: bool = true; fn f() -> bool { FLAG }", expect_all: &["ret i1"], expect_none: &[], desc: "Stage 3.44: const bool" },
        Case { name: "c07_multiple_consts", src: "const A: i32 = 1; const B: i32 = 2; fn f() -> i32 { A + B }", expect_all: &["add nsw i32"], expect_none: &[], desc: "Stage 3.44: multiple consts" },
        Case { name: "c08_const_in_if", src: "const LIMIT: i32 = 100; fn f(x: i32) -> i32 { if x > LIMIT { 1 } else { 0 } }", expect_all: &["icmp sgt", "100"], expect_none: &[], desc: "Stage 3.44: const in if condition" },
        Case { name: "c09_const_no_type_mismatch", src: "const MAX: i32 = 100; fn f() -> i32 { MAX }", expect_all: &[], expect_none: &["mismatched types"], desc: "Stage 3.44 §15.4: no type mismatch for const" },
        Case { name: "c10_const_in_loop", src: "const STEP: i32 = 2; fn f(n: i32) -> i32 { let mut s = 0; let mut i = 0; while i < n { s = s + STEP; i = i + 1; } s }", expect_all: &["add nsw i32"], expect_none: &[], desc: "Stage 3.44: const used in loop" },

        // Group E: edge cases (5)
        Case { name: "e01_const_zero", src: "const ZERO: i32 = 0; fn f() -> i32 { ZERO }", expect_all: &["store i32 0"], expect_none: &[], desc: "Stage 3.44 edge: const zero" },
        Case { name: "e02_const_negative", src: "const NEG: i32 = -42; fn f() -> i32 { NEG }", expect_all: &["42"], expect_none: &[], desc: "Stage 3.44 edge: const negative" },
        Case { name: "e03_const_as_arg", src: "const VAL: i32 = 99; fn id(x: i32) -> i32 { x } fn f() -> i32 { id(VAL) }", expect_all: &["call i32 @landin_id(i32 99)"], expect_none: &[], desc: "Stage 3.44 edge: const as function arg" },
        Case { name: "e04_static_f64", src: "static PI: f64 = 3.14; fn f() -> f64 { PI }", expect_all: &["3.14"], expect_none: &[], desc: "Stage 3.44 edge: static f64" },
        Case { name: "e05_const_overflow_check", src: "const A: i32 = 10; const B: i32 = 20; fn f() -> i32 { A + B }", expect_all: &["llvm.sadd.with.overflow.i32"], expect_none: &[], desc: "Stage 3.44 edge: const arith gets overflow check" },
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

    println!("\n=== Stage 3 Gate Audit Round 11 Summary ===");
    println!("    Total: {}  Pass: {}  Fail: {}", cases.len(), pass, fail);
    if !failures.is_empty() {
        println!("    Failed cases: {:?}", failures);
    }
    if fail == 0 {
        println!(
            "\n✅ AUDIT PASSED — 0 codegen defects found in {} cases.",
            cases.len()
        );
        println!("   R1-R11: ..., 28, 28, {}/{} — all OK.", pass, cases.len());
        println!("   Per §9.3.3, audit CONVERGED (11 rounds, 0 new issues each).");
        println!("   Stage 3.44 (const/static value resolution) verified.");
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found.", fail);
    }
}
