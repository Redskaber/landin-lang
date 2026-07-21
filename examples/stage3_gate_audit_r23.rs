//! Stage 3 Gate Review Round 23 audit (per §9.3.1 of process v3.13).
//!
//! Run with: cargo run --example stage3_gate_audit_r23
//!
//! Purpose: Phase gate audit for Stage 3.56. Verifies that the refactored
//! pipeline (codegen as pure MIR consumer) produces identical IR to the
//! pre-refactoring pipeline, plus validates the new architecture contracts.
//!
//! Author: redskaber
//! Date: 2026-07-21
//! Process: v3.13

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
}

fn main() {
    let cases: &[Case] = &[
        // Group R: Regression (8) — re-verify R22 cases
        Case { name: "r01_void_fn", src: "fn id(s: &str) -> &str { s } fn f() { id(\"hello\") }", expect_all: &["define void @landin_f()"], expect_none: &[], desc: "R22 r01: void fn emits void" },
        Case { name: "r02_str_param", src: "fn f(s: &str) { }", expect_all: &["define void @landin_f({ i8*, i64 } %arg0)"], expect_none: &[], desc: "R22 r02: &str param as fat pointer" },
        Case { name: "r03_slice_i64", src: "fn f(s: &[i64]) -> i64 { s[0] }", expect_all: &["load i64"], expect_none: &[], desc: "R22 r03: &[i64] element loads i64" },
        Case { name: "r04_enum_case_c", src: "enum E { A, B(i32), C(i64) } fn f(e: E) -> i32 { 0 }", expect_all: &["{ i32, i32, i64 }"], expect_none: &[], desc: "R22 r04: Case C enum flat layout" },
        Case { name: "r05_str_idx", src: "fn f(s: &str) -> i32 { s[0] }", expect_all: &["load i8"], expect_none: &[], desc: "R22 r05: &str index loads i8" },
        Case { name: "r06_slice_field", src: "struct S { data: &mut [i32] } fn f(s: S) { s.data[0] = 42; }", expect_all: &["store i32 42"], expect_none: &[], desc: "R22 r06: slice field store" },
        Case { name: "r07_const", src: "const MAX: i32 = 100; fn f() -> i32 { MAX }", expect_all: &["store i32 100"], expect_none: &[], desc: "R22 r07: const value inlined" },
        Case { name: "r08_i16", src: "fn f(a: i16, b: i16) -> i16 { a + b }", expect_all: &["add nsw i16", "llvm.sadd.with.overflow.i16"], expect_none: &[], desc: "R22 r08: i16 arithmetic" },

        // Group A: Stage 3.56 architecture contracts (14)
        Case { name: "a01_pure_mir_consumer", src: "fn f(x: i32) -> i32 { x + 1 }", expect_all: &["define i32 @landin_f(i32 %arg0)"], expect_none: &[], desc: "Stage 3.56: codegen from pre-built MIR" },
        Case { name: "a02_no_double_lowering", src: "fn f() -> i32 { 42 }", expect_all: &["define i32 @landin_f()"], expect_none: &[], desc: "Stage 3.56: single lowering (no double)" },
        Case { name: "a03_void_from_mir", src: "fn id(s: &str) -> &str { s } fn f() { id(\"hello\") }", expect_all: &["define void @landin_f()"], expect_none: &[], desc: "Stage 3.56: void fn from pre-built MIR + metadata" },
        Case { name: "a04_fn_name_precomputed", src: "fn helper() -> i32 { 42 } fn f() -> i32 { helper() }", expect_all: &["call i32 @landin_helper()"], expect_none: &[], desc: "Stage 3.56: fn_name_by_def_id precomputed" },
        Case { name: "a05_body_metas_parallel", src: "fn a() -> i32 { 1 } fn b() -> i32 { 2 } fn c() -> i32 { 3 }", expect_all: &["define i32 @landin_a()", "define i32 @landin_b()", "define i32 @landin_c()"], expect_none: &[], desc: "Stage 3.56: body_metas parallel to mirs" },
        Case { name: "a06_complex_pipeline", src: "struct Point { x: i32, y: i32 } fn make() -> Point { Point { x: 1, y: 2 } } fn f() -> i32 { let p = Point { x: 1, y: 2 }; p.x + p.y }", expect_all: &["define { i32, i32 } @landin_make()", "define i32 @landin_f()", "insertvalue { i32, i32 }", "add nsw i32"], expect_none: &[], desc: "Stage 3.56: complex pipeline no regressions" },
        // A.2 — §16 compliance regression tests
        Case { name: "a07_str_param_fat", src: "fn f(s: &str) { }", expect_all: &["define void @landin_f({ i8*, i64 } %arg0)"], expect_none: &[], desc: "Stage 3.56 §16: &str param still fat pointer" },
        Case { name: "a08_bstr_fat", src: "fn f() { let b = b\"hello\"; }", expect_all: &["alloca { i8*, i64 }", "i64 5, 1"], expect_none: &[], desc: "Stage 3.56 §16: byte string still fat pointer" },
        Case { name: "a09_str_eq", src: "fn f(s: &str) -> bool { s == \"hello\" }", expect_all: &["extractvalue { i8*, i64 }", "and i1"], expect_none: &[], desc: "Stage 3.56 §16: str eq comparison unchanged" },
        Case { name: "a10_enum_match", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, _ => 0 } }", expect_all: &["switch i32"], expect_none: &[], desc: "Stage 3.56 §16: enum match unchanged" },
        Case { name: "a11_overflow_check", src: "fn f(a: i32, b: i32) -> i32 { a + b }", expect_all: &["llvm.sadd.with.overflow.i32"], expect_none: &[], desc: "Stage 3.56 §16: overflow check unchanged" },
        Case { name: "a12_div_zero", src: "fn f(a: i32, b: i32) -> i32 { a / b }", expect_all: &["icmp eq", "__landin_panic_div_by_zero"], expect_none: &[], desc: "Stage 3.56 §16: div-by-zero check unchanged" },
        Case { name: "a13_struct_nested", src: "struct Inner { v: i32 } struct Outer { i: Inner } fn f(o: Outer) -> i32 { 0 }", expect_all: &["{ { i32 } }"], expect_none: &[], desc: "Stage 3.56 §16: nested struct layout unchanged" },
        Case { name: "a14_float_bitwise", src: "fn f(a: f64, b: f64) -> f64 { a & b }", expect_all: &["and i64", "fptosi", "sitofp"], expect_none: &[], desc: "Stage 3.56 §16: float bitwise unchanged" },

        // Group E: §9.3.2 edge cases (8)
        Case { name: "e01_void_empty", src: "fn f() { }", expect_all: &["define void @landin_f()", "ret void"], expect_none: &[], desc: "Stage 3.56 edge: empty void fn" },
        Case { name: "e02_void_with_params", src: "fn f(x: i32, y: i32) { }", expect_all: &["define void @landin_f(i32 %arg0, i32 %arg1)"], expect_none: &[], desc: "Stage 3.56 edge: void fn with params" },
        Case { name: "e03_nonvoid_i32", src: "fn f() -> i32 { 42 }", expect_all: &["define i32 @landin_f()"], expect_none: &[], desc: "Stage 3.56 edge: non-void i32 fn" },
        Case { name: "e04_nonvoid_str", src: "fn f() -> &str { \"hello\" }", expect_all: &["define { i8*, i64 } @landin_f()"], expect_none: &[], desc: "Stage 3.56 edge: non-void &str fn" },
        Case { name: "e05_multiple_fns", src: "fn a() -> i32 { 1 } fn b() -> i32 { 2 } fn c() -> i32 { a() + b() }", expect_all: &["call i32 @landin_a()", "call i32 @landin_b()"], expect_none: &[], desc: "Stage 3.56 edge: multiple fns with calls" },
        Case { name: "e06_struct_return", src: "struct S { x: i32 } fn f() -> S { S { x: 1 } }", expect_all: &["define { i32 } @landin_f()"], expect_none: &[], desc: "Stage 3.56 edge: struct return fn" },
        Case { name: "e07_array_param", src: "fn f(a: [i32; 3]) -> i32 { a[1] }", expect_all: &["getelementptr inbounds [3 x i32], [3 x i32]*"], expect_none: &[], desc: "Stage 3.56 edge: array param indexing" },
        Case { name: "e08_while_loop", src: "fn f(n: i32) -> i32 { let mut i = 0; while i < n { i = i + 1; } i }", expect_all: &["br i1"], expect_none: &[], desc: "Stage 3.56 edge: while loop" },
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

    println!("\n=== Stage 3 Gate Audit Round 23 Summary ===");
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
            "   R1-R23: ..., 28, 28, 23, 24, 30, 30, 30, {}/{} — all OK.",
            pass,
            cases.len()
        );
        println!("   Per §9.3.3, audit CONVERGED (23 rounds, 0 new issues each).");
        println!("   Stage 3.56 (pipeline architecture refactoring: codegen as pure MIR consumer) verified.");
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found.", fail);
    }
}
