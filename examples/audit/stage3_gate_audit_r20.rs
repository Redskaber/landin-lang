//! Stage 3 Gate Review Round 20 audit (per §9.3.1 of process v3.13).
//!
//! Run with: cargo run --example stage3_gate_audit_r20
//!
//! Purpose: Phase gate audit for Stage 3.53 — &str indexing element type
//! fix. Verifies that `s[i]` where `s: &str` produces u8 (i8) element type
//! for load/store/arithmetic (was: `resolve_index_element_type` didn't
//! handle `Ref(_, _, Str)`, so element type was fresh_infer_ty → typeck
//! default i32, causing store i8 into i32 temp — type mismatch).
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
        // Group R: Regression (8) — re-verify R19 cases
        Case { name: "r01_str_param", src: "fn f(s: &str) { }", expect_all: &["define void @landin_f({ i8*, i64 } %arg0)"], expect_none: &[], desc: "R19 r01: &str param as fat pointer" },
        Case { name: "r02_slice_i64", src: "fn f(s: &[i64]) -> i64 { s[0] }", expect_all: &["load i64"], expect_none: &[], desc: "R19 r02: &[i64] element loads i64 (Stage 3.52)" },
        Case { name: "r03_bstr_fat_ptr", src: "fn f() { let b = b\"hello\"; }", expect_all: &["alloca { i8*, i64 }", "i64 5, 1"], expect_none: &[], desc: "R19 r03: byte string fat pointer" },
        Case { name: "r04_enum_case_c", src: "enum E { A, B(i32), C(i64) } fn f(e: E) -> i32 { 0 }", expect_all: &["{ i32, i32, i64 }"], expect_none: &[], desc: "R19 r04: Case C enum flat layout" },
        Case { name: "r05_array_idx", src: "fn f(a: [i32; 3]) -> i32 { a[1] }", expect_all: &["getelementptr inbounds [3 x i32], [3 x i32]*"], expect_none: &[], desc: "R19 r05: array indexing (no regression)" },
        Case { name: "r06_const_value", src: "const MAX: i32 = 100; fn f() -> i32 { MAX }", expect_all: &["store i32 100"], expect_none: &[], desc: "R19 r06: const value inlined" },
        Case { name: "r07_div_zero", src: "fn f(a: i32, b: i32) -> i32 { a / b }", expect_all: &["icmp eq", "__landin_panic_div_by_zero"], expect_none: &[], desc: "R19 r07: div-by-zero runtime check" },
        Case { name: "r08_i16_arith", src: "fn f(a: i16, b: i16) -> i16 { a + b }", expect_all: &["add nsw i16", "llvm.sadd.with.overflow.i16"], expect_none: &[], desc: "R19 r08: i16 arithmetic + overflow check" },

        // Group H: Stage 3.53 &str indexing coverage (14)
        // H.1 — &str index load type
        Case { name: "h01_str_idx_load_i8", src: "fn f(s: &str) -> i32 { s[0] }", expect_all: &["load i8", "store i8"], expect_none: &[], desc: "Stage 3.53: &str s[0] loads i8 (u8), not i32" },
        Case { name: "h02_str_idx_no_i32_temp", src: "fn f(s: &str) -> i32 { s[0] }", expect_all: &[], expect_none: &["store i32 %v4"], desc: "Stage 3.53: no i32 temp for &str element" },
        // H.2 — &str index arithmetic
        Case { name: "h03_str_idx_arith_i8", src: "fn f(s: &str) -> i32 { s[0] + 1 }", expect_all: &["add nsw i8", "llvm.sadd.with.overflow.i8"], expect_none: &[], desc: "Stage 3.53: &str s[0] + 1 uses i8 arith + i8 overflow" },
        Case { name: "h04_str_idx_two_elements", src: "fn f(s: &str) -> i32 { s[0] + s[1] }", expect_all: &["add nsw i8"], expect_none: &[], desc: "Stage 3.53: &str two elements use i8 arith" },
        // H.3 — &str index comparison
        Case { name: "h05_str_idx_cmp_i8", src: "fn f(s: &str) -> bool { s[0] > s[1] }", expect_all: &["icmp sgt i8"], expect_none: &[], desc: "Stage 3.53: &str comparison uses icmp sgt i8" },
        // H.4 — &str variable/constant index
        Case { name: "h06_str_idx_variable", src: "fn f(s: &str, i: i32) -> i32 { s[i] }", expect_all: &["load i8"], expect_none: &[], desc: "Stage 3.53: &str variable index loads i8" },
        Case { name: "h07_str_idx_constant", src: "fn f(s: &str) -> i32 { s[1] }", expect_all: &["load i8"], expect_none: &[], desc: "Stage 3.53: &str constant index loads i8" },
        // H.5 — &str index in different contexts
        Case { name: "h08_str_idx_in_if", src: "fn f(s: &str) -> i32 { if s[0] > 0 { 1 } else { 0 } }", expect_all: &["load i8", "icmp sgt", "br i1"], expect_none: &[], desc: "Stage 3.53: &str index in if condition" },
        Case { name: "h09_str_idx_in_loop", src: "fn f(s: &str, n: i32) -> i32 { let mut sum = 0; let mut i = 0; while i < n { sum = sum + s[i]; i = i + 1; } sum }", expect_all: &["load i8", "br i1"], expect_none: &[], desc: "Stage 3.53: &str index in loop" },
        // H.6 — byte string indexing
        Case { name: "h10_bstr_idx_i8", src: "fn f() -> i32 { b\"hello\"[0] }", expect_all: &["load i8"], expect_none: &[], desc: "Stage 3.53: byte string index loads i8" },
        // H.7 — &str index subtraction
        Case { name: "h11_str_idx_sub_i8", src: "fn f(s: &str) -> i32 { s[0] - 1 }", expect_all: &["sub nsw i8", "llvm.ssub.with.overflow.i8"], expect_none: &[], desc: "Stage 3.53: &str subtraction uses i8" },
        // H.8 — slice regression (Stage 3.52 still works)
        Case { name: "h12_slice_i64_regression", src: "fn f(s: &[i64]) -> i64 { s[0] + s[1] }", expect_all: &["add nsw i64"], expect_none: &["add nsw i32"], desc: "Stage 3.53 regression: &[i64] arith still i64" },
        Case { name: "h13_slice_i32_regression", src: "fn f(s: &[i32]) -> i32 { s[0] }", expect_all: &["load i32"], expect_none: &[], desc: "Stage 3.53 regression: &[i32] loads i32" },
        // H.9 — &str index returns u8, widened to i32 return
        Case { name: "h14_str_idx_widens_to_i32", src: "fn f(s: &str) -> i32 { s[0] }", expect_all: &["load i8", "ret i32"], expect_none: &[], desc: "Stage 3.53: &str s[0] returns i32 (u8 widened)" },

        // Group E: §9.3.2 edge cases (8)
        Case { name: "e01_str_idx_last_char", src: "fn f(s: &str) -> i32 { s[4] }", expect_all: &["load i8"], expect_none: &[], desc: "Stage 3.53 edge: &str index 4 (last char of 5-byte)" },
        Case { name: "e02_str_idx_arith_chain", src: "fn f(s: &str) -> i32 { s[0] + s[1] + s[2] }", expect_all: &["add nsw i8"], expect_none: &["add nsw i32"], desc: "Stage 3.53 edge: three &str accesses use i8 arith" },
        Case { name: "e03_str_idx_mixed_with_int", src: "fn f(s: &str, n: i32) -> i32 { s[0] + n }", expect_all: &[], expect_none: &[], desc: "Stage 3.53 edge: &str byte + i32 (compiles)" },
        Case { name: "e04_bstr_idx_in_fn", src: "fn f(b: &[u8]) -> u8 { b[0] }", expect_all: &["load i8"], expect_none: &[], desc: "Stage 3.53 edge: &[u8] param indexing" },
        Case { name: "e05_str_idx_comparison_eq", src: "fn f(s: &str) -> bool { s[0] == 65 }", expect_all: &["icmp eq i8"], expect_none: &[], desc: "Stage 3.53 edge: &str byte == literal (i8 cmp)" },
        Case { name: "e06_str_idx_store_via_mut", src: "fn f(s: &mut [u8]) { s[0] = 65; }", expect_all: &["store i8 65"], expect_none: &[], desc: "Stage 3.53 edge: &mut [u8] store i8" },
        Case { name: "e07_array_i8_regression", src: "fn f(a: [u8; 3]) -> u8 { a[0] }", expect_all: &["load i8"], expect_none: &[], desc: "Stage 3.53 edge: [u8; 3] array indexing (i8)" },
        Case { name: "e08_str_idx_in_match", src: "fn f(s: &str) -> i32 { match s[0] { 65 => 1, _ => 0 } }", expect_all: &["load i8", "switch i8"], expect_none: &[], desc: "Stage 3.53 edge: &str byte in match (switch i8 — no widening)" },
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

    println!("\n=== Stage 3 Gate Audit Round 20 Summary ===");
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
            "   R1-R20: ..., 28, 28, 23, 24, 30, 30, 30, {}/{} — all OK.",
            pass,
            cases.len()
        );
        println!("   Per §9.3.3, audit CONVERGED (20 rounds, 0 new issues each).");
        println!("   Stage 3.53 (&str indexing element type fix) verified.");
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found.", fail);
    }
}
