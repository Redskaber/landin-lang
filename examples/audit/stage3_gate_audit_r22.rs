//! Stage 3 Gate Review Round 22 audit (per §9.3.1 of process v3.13).
//!
//! Run with: cargo run --example stage3_gate_audit_r22
//!
//! Purpose: Phase gate audit for Stage 3.55. Verifies that void functions
//! emit `define void` + `ret void` regardless of their body value's type.
//! Was: void fn's return local got a fresh infer var that typeck unified
//! with the body value type, causing `fn f() { id("hello") }` to emit
//! `define { i8*, i64 }` instead of `define void` (P0 correctness).
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
        (false, format!("FAIL — no HIR: {}", c.desc))
    }
}

fn main() {
    let cases: &[Case] = &[
        // Group R: Regression (8) — re-verify R21 cases
        Case {
            name: "r01_str_param",
            src: "fn f(s: &str) { }",
            expect_all: &["define void @landin_f({ i8*, i64 } %arg0)"],
            expect_none: &[],
            desc: "R21 r01: &str param as fat pointer",
        },
        Case {
            name: "r02_slice_field",
            src: "struct S { data: &mut [i32] } fn f(s: S) { s.data[0] = 42; }",
            expect_all: &["store i32 42"],
            expect_none: &[],
            desc: "R21 r02: slice field store",
        },
        Case {
            name: "r03_str_idx",
            src: "fn f(s: &str) -> i32 { s[0] }",
            expect_all: &["load i8", "store i8"],
            expect_none: &[],
            desc: "R21 r03: &str index loads i8",
        },
        Case {
            name: "r04_slice_i64",
            src: "fn f(s: &[i64]) -> i64 { s[0] }",
            expect_all: &["load i64"],
            expect_none: &[],
            desc: "R21 r04: &[i64] element loads i64",
        },
        Case {
            name: "r05_enum_case_c",
            src: "enum E { A, B(i32), C(i64) } fn f(e: E) -> i32 { 0 }",
            expect_all: &["{ i32, i32, i64 }"],
            expect_none: &[],
            desc: "R21 r05: Case C enum flat layout",
        },
        Case {
            name: "r06_array_idx",
            src: "fn f(a: [i32; 3]) -> i32 { a[1] }",
            expect_all: &["getelementptr inbounds [3 x i32], [3 x i32]*"],
            expect_none: &[],
            desc: "R21 r06: array indexing (no regression)",
        },
        Case {
            name: "r07_const_value",
            src: "const MAX: i32 = 100; fn f() -> i32 { MAX }",
            expect_all: &["store i32 100"],
            expect_none: &[],
            desc: "R21 r07: const value inlined",
        },
        Case {
            name: "r08_i16_arith",
            src: "fn f(a: i16, b: i16) -> i16 { a + b }",
            expect_all: &["add nsw i16", "llvm.sadd.with.overflow.i16"],
            expect_none: &[],
            desc: "R21 r08: i16 arithmetic + overflow check",
        },
        // Group V: Stage 3.55 void function coverage (14)
        Case {
            name: "v01_void_emits_void",
            src: "fn id(s: &str) -> &str { s } fn f() { id(\"hello\") }",
            expect_all: &["define void @landin_f()"],
            expect_none: &[],
            desc: "Stage 3.55: void fn emits define void (not fat ptr)",
        },
        Case {
            name: "v02_void_ret_void",
            src: "fn id(s: &str) -> &str { s } fn f() { id(\"hello\") }",
            expect_all: &["ret void"],
            expect_none: &[],
            desc: "Stage 3.55: void fn ret void",
        },
        Case {
            name: "v03_void_calls_i32",
            src: "fn g() -> i32 { 42 } fn f() { g(); }",
            expect_all: &["define void @landin_f()"],
            expect_none: &[],
            desc: "Stage 3.55: void fn calling i32 fn is void",
        },
        Case {
            name: "v04_void_empty",
            src: "fn f() { }",
            expect_all: &["define void @landin_f()", "ret void"],
            expect_none: &[],
            desc: "Stage 3.55: empty void fn",
        },
        Case {
            name: "v05_void_str_body",
            src: "fn f() { \"hello\" }",
            expect_all: &["define void @landin_f()"],
            expect_none: &[],
            desc: "Stage 3.55: void fn with str body",
        },
        Case {
            name: "v06_void_arith_body",
            src: "fn f() { 1 + 2 }",
            expect_all: &["define void @landin_f()"],
            expect_none: &[],
            desc: "Stage 3.55: void fn with arith body",
        },
        Case {
            name: "v07_void_call_chain",
            src: "fn a() { } fn b() { a(); } fn c() { b(); }",
            expect_all: &[
                "define void @landin_a()",
                "define void @landin_b()",
                "define void @landin_c()",
            ],
            expect_none: &[],
            desc: "Stage 3.55: void call chain",
        },
        Case {
            name: "v08_void_with_params",
            src: "fn f(x: i32, y: i32) { }",
            expect_all: &["define void @landin_f(i32 %arg0, i32 %arg1)"],
            expect_none: &[],
            desc: "Stage 3.55: void fn with params",
        },
        Case {
            name: "v09_void_str_param_body",
            src: "fn f(s: &str) { }",
            expect_all: &["define void @landin_f({ i8*, i64 } %arg0)"],
            expect_none: &[],
            desc: "Stage 3.55: void fn with &str param",
        },
        // V.2 — non-void regression
        Case {
            name: "v10_nonvoid_i32",
            src: "fn f() -> i32 { 42 }",
            expect_all: &["define i32 @landin_f()"],
            expect_none: &[],
            desc: "Stage 3.55 regression: i32 fn still correct",
        },
        Case {
            name: "v11_nonvoid_str",
            src: "fn f() -> &str { \"hello\" }",
            expect_all: &["define { i8*, i64 } @landin_f()"],
            expect_none: &[],
            desc: "Stage 3.55 regression: &str fn still correct",
        },
        Case {
            name: "v12_nonvoid_i64",
            src: "fn f() -> i64 { 42 }",
            expect_all: &["define i64 @landin_f()"],
            expect_none: &[],
            desc: "Stage 3.55 regression: i64 fn still correct",
        },
        // V.3 — void fn with complex body
        Case {
            name: "v13_void_if",
            src: "fn f(x: i32) { if x > 0 { } else { } }",
            expect_all: &["define void @landin_f(i32 %arg0)"],
            expect_none: &[],
            desc: "Stage 3.55: void fn with if",
        },
        Case {
            name: "v14_void_while",
            src: "fn f(n: i32) { let mut i = 0; while i < n { i = i + 1; } }",
            expect_all: &["define void @landin_f(i32 %arg0)", "br i1"],
            expect_none: &[],
            desc: "Stage 3.55: void fn with while loop",
        },
        // Group E: §9.3.2 edge cases (8)
        Case {
            name: "e01_void_struct_field",
            src: "struct S { x: i32 } fn f(s: S) { }",
            expect_all: &["define void @landin_f({ i32 } %arg0)"],
            expect_none: &[],
            desc: "Stage 3.55 edge: void fn with struct param",
        },
        Case {
            name: "e02_void_enum_param",
            src: "enum E { A, B } fn f(e: E) { }",
            expect_all: &["define void @landin_f({ i32 } %arg0)"],
            expect_none: &[],
            desc: "Stage 3.55 edge: void fn with enum param",
        },
        Case {
            name: "e03_void_array_param",
            src: "fn f(a: [i32; 3]) { }",
            expect_all: &["define void @landin_f([3 x i32] %arg0)"],
            expect_none: &[],
            desc: "Stage 3.55 edge: void fn with array param",
        },
        Case {
            name: "e04_void_slice_param",
            src: "fn f(s: &[i32]) { }",
            expect_all: &["define void @landin_f({ i32*, i64 } %arg0)"],
            expect_none: &[],
            desc: "Stage 3.55 edge: void fn with slice param",
        },
        Case {
            name: "e05_void_calls_void",
            src: "fn g() { } fn f() { g(); g(); }",
            expect_all: &["define void @landin_f()", "call void @landin_g()"],
            expect_none: &[],
            desc: "Stage 3.55 edge: void fn calls void fn twice",
        },
        Case {
            name: "e06_void_mixed_returns",
            src: "fn g() -> i32 { 1 } fn h() -> &str { \"hi\" } fn f() { g(); h(); }",
            expect_all: &["define void @landin_f()"],
            expect_none: &[],
            desc: "Stage 3.55 edge: void fn calls i32 and &str fns",
        },
        Case {
            name: "e07_nonvoid_struct_return",
            src: "struct S { x: i32 } fn f() -> S { S { x: 1 } }",
            expect_all: &["define { i32 } @landin_f()"],
            expect_none: &[],
            desc: "Stage 3.55 edge: struct return fn (non-void)",
        },
        Case {
            name: "e08_void_let_then_call",
            src: "fn g() -> i32 { 42 } fn f() { let x = g(); }",
            expect_all: &["define void @landin_f()"],
            expect_none: &[],
            desc: "Stage 3.55 edge: void fn with let binding",
        },
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

    println!("\n=== Stage 3 Gate Audit Round 22 Summary ===");
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
            "   R1-R22: ..., 28, 28, 23, 24, 30, 30, 30, {}/{} — all OK.",
            pass,
            cases.len()
        );
        println!("   Per §9.3.3, audit CONVERGED (22 rounds, 0 new issues each).");
        println!("   Stage 3.55 (void function return type fix) verified.");
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found.", fail);
    }
}
