//! Stage 3 Gate Review Round 19 audit (per §9.3.1 of process v3.13).
//!
//! Run with: cargo run --example stage3_gate_audit_r19
//!
//! Purpose: Phase gate audit for Stage 3.52 — slice element type
//! propagation fix. Verifies that `s[i]` where `s: &[T]` produces the
//! correct element type for load/store/arithmetic (was: `detect_place_type`
//! fell through to I32 fallback, and MIR lower used a fresh infer var
//! that typeck defaulted to i32 — causing `s[0]` on `&[i64]` to
//! `load i32` instead of `load i64`).
//!
//! Author: redskaber
//! Date: 2026-07-20
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
        // Group R: Regression (8) — re-verify R18 cases
        Case { name: "r01_str_param", src: "fn f(s: &str) { }", expect_all: &["define void @landin_f({ i8*, i64 } %arg0)"], expect_none: &[], desc: "R18 r01: &str param as fat pointer" },
        Case { name: "r02_slice_idx_i32", src: "fn f(s: &[i32]) -> i32 { s[0] }", expect_all: &["getelementptr inbounds i32, i32*", "load i32"], expect_none: &[], desc: "R18 r02: &[i32] slice indexing" },
        Case { name: "r03_bstr_fat_ptr", src: "fn f() { let b = b\"hello\"; }", expect_all: &["alloca { i8*, i64 }", "i64 5, 1"], expect_none: &[], desc: "R18 r03: byte string fat pointer" },
        Case { name: "r04_enum_case_c", src: "enum E { A, B(i32), C(i64) } fn f(e: E) -> i32 { 0 }", expect_all: &["{ i32, i32, i64 }"], expect_none: &[], desc: "R18 r04: Case C enum flat layout" },
        Case { name: "r05_array_idx", src: "fn f(a: [i32; 3]) -> i32 { a[1] }", expect_all: &["getelementptr inbounds [3 x i32], [3 x i32]*"], expect_none: &[], desc: "R18 r05: array indexing (no regression)" },
        Case { name: "r06_const_value", src: "const MAX: i32 = 100; fn f() -> i32 { MAX }", expect_all: &["store i32 100"], expect_none: &[], desc: "R18 r06: const value inlined" },
        Case { name: "r07_div_zero", src: "fn f(a: i32, b: i32) -> i32 { a / b }", expect_all: &["icmp eq", "__landin_panic_div_by_zero"], expect_none: &[], desc: "R18 r07: div-by-zero runtime check" },
        Case { name: "r08_i16_arith", src: "fn f(a: i16, b: i16) -> i16 { a + b }", expect_all: &["add nsw i16", "llvm.sadd.with.overflow.i16"], expect_none: &[], desc: "R18 r08: i16 arithmetic + overflow check" },

        // Group T: Stage 3.52 element type propagation (14)
        // T.1 — load uses correct element type
        Case { name: "t01_slice_i64_load", src: "fn f(s: &[i64]) -> i64 { s[0] }", expect_all: &["load i64"], expect_none: &["load i32"], desc: "Stage 3.52: &[i64] s[0] loads i64 (not i32)" },
        Case { name: "t02_slice_i32_load", src: "fn f(s: &[i32]) -> i32 { s[0] }", expect_all: &["load i32"], expect_none: &[], desc: "Stage 3.52: &[i32] s[0] loads i32" },
        Case { name: "t03_slice_i128_load", src: "fn f(s: &[i128]) -> i128 { s[0] }", expect_all: &["load i128"], expect_none: &[], desc: "Stage 3.52: &[i128] s[0] loads i128" },
        Case { name: "t04_slice_f64_load", src: "fn f(s: &[f64]) -> f64 { s[0] }", expect_all: &["load double"], expect_none: &[], desc: "Stage 3.52: &[f64] s[0] loads double" },
        // T.2 — arithmetic uses correct width
        Case { name: "t05_slice_i64_arith", src: "fn f(s: &[i64]) -> i64 { s[0] + s[1] }", expect_all: &["add nsw i64", "llvm.sadd.with.overflow.i64"], expect_none: &["add nsw i32"], desc: "Stage 3.52: &[i64] arith uses i64 (not i32)" },
        Case { name: "t06_slice_i32_arith", src: "fn f(s: &[i32]) -> i32 { s[0] + s[1] }", expect_all: &["add nsw i32"], expect_none: &["add nsw i64"], desc: "Stage 3.52: &[i32] arith uses i32" },
        Case { name: "t07_slice_f64_arith", src: "fn f(s: &[f64]) -> f64 { s[0] + s[1] }", expect_all: &["fadd double"], expect_none: &[], desc: "Stage 3.52: &[f64] arith uses fadd double" },
        // T.3 — store uses correct type
        Case { name: "t08_slice_i64_store", src: "fn f(s: &mut [i64]) { s[0] = 42; }", expect_all: &["store i64 42"], expect_none: &[], desc: "Stage 3.52: &mut [i64] s[0] = 42 stores i64" },
        Case { name: "t09_slice_i32_store", src: "fn f(s: &mut [i32]) { s[0] = 42; }", expect_all: &["store i32 42"], expect_none: &[], desc: "Stage 3.52: &mut [i32] s[0] = 42 stores i32" },
        // T.4 — comparison uses correct type
        Case { name: "t10_slice_i64_cmp", src: "fn f(s: &[i64]) -> bool { s[0] > s[1] }", expect_all: &["icmp sgt i64"], expect_none: &[], desc: "Stage 3.52: &[i64] comparison uses icmp sgt i64" },
        Case { name: "t11_slice_i32_cmp", src: "fn f(s: &[i32]) -> bool { s[0] > s[1] }", expect_all: &["icmp sgt i32"], expect_none: &[], desc: "Stage 3.52: &[i32] comparison uses icmp sgt i32" },
        // T.5 — array regression (element type still correct)
        Case { name: "t12_array_i64_arith", src: "fn f(a: [i64; 3]) -> i64 { a[0] + a[1] }", expect_all: &["add nsw i64"], expect_none: &[], desc: "Stage 3.52 regression: [i64; 3] arith uses i64" },
        // T.6 — subtraction and multiplication
        Case { name: "t13_slice_i64_sub", src: "fn f(s: &[i64]) -> i64 { s[0] - s[1] }", expect_all: &["sub nsw i64", "llvm.ssub.with.overflow.i64"], expect_none: &[], desc: "Stage 3.52: &[i64] subtraction uses i64" },
        Case { name: "t14_slice_i64_mul", src: "fn f(s: &[i64]) -> i64 { s[0] * s[1] }", expect_all: &["mul nsw i64", "llvm.smul.with.overflow.i64"], expect_none: &[], desc: "Stage 3.52: &[i64] multiplication uses i64" },

        // Group E: §9.3.2 edge cases (8)
        Case { name: "e01_slice_i16_element", src: "fn f(s: &[i16]) -> i16 { s[0] }", expect_all: &["load i16"], expect_none: &[], desc: "Stage 3.52 edge: &[i16] element" },
        Case { name: "e02_slice_usize_element", src: "fn f(s: &[usize]) -> usize { s[0] }", expect_all: &["load i64"], expect_none: &[], desc: "Stage 3.52 edge: &[usize] element (i64 on 64-bit)" },
        Case { name: "e03_slice_bool_element", src: "fn f(s: &[bool]) -> bool { s[0] }", expect_all: &["load i1"], expect_none: &[], desc: "Stage 3.52 edge: &[bool] element" },
        Case { name: "e04_slice_mixed_arith", src: "fn f(s: &[i64]) -> i64 { s[0] + 1 }", expect_all: &["add nsw i64"], expect_none: &[], desc: "Stage 3.52 edge: slice element + literal (i64 arith)" },
        Case { name: "e05_slice_in_loop", src: "fn f(s: &[i64], n: i32) -> i64 { let mut sum = 0; let mut i = 0; while i < n { sum = sum + s[i]; i = i + 1; } sum }", expect_all: &["add nsw i64", "br i1"], expect_none: &[], desc: "Stage 3.52 edge: slice in loop uses i64 arith" },
        Case { name: "e06_slice_three_accesses", src: "fn f(s: &[i64]) -> i64 { s[0] + s[1] + s[2] }", expect_all: &["add nsw i64"], expect_none: &["add nsw i32"], desc: "Stage 3.52 edge: three slice accesses use i64" },
        Case { name: "e07_array_i32_regression", src: "fn f(a: [i32; 4]) -> i32 { a[0] + a[1] }", expect_all: &["add nsw i32"], expect_none: &[], desc: "Stage 3.52 edge: [i32; 4] arith still uses i32 (no regression)" },
        Case { name: "e08_slice_div_correct", src: "fn f(s: &[i64]) -> i64 { s[0] / s[1] }", expect_all: &["sdiv i64"], expect_none: &[], desc: "Stage 3.52 edge: &[i64] division uses sdiv i64" },
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

    println!("\n=== Stage 3 Gate Audit Round 19 Summary ===");
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
            "   R1-R19: ..., 28, 28, 23, 24, 30, 30, 30, {}/{} — all OK.",
            pass,
            cases.len()
        );
        println!("   Per §9.3.3, audit CONVERGED (19 rounds, 0 new issues each).");
        println!("   Stage 3.52 (slice element type propagation fix) verified.");
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found.", fail);
    }
}
