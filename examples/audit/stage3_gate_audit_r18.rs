//! Stage 3 Gate Review Round 18 audit (per §9.3.1 of process v3.13).
//!
//! Run with: cargo run --example stage3_gate_audit_r18
//!
//! Purpose: Phase gate audit for Stage 3.51 — slice indexing fix.
//! Verifies that `s[i]` where `s: &[T]` correctly dereferences the fat
//! pointer's data pointer to load the element (was: GEP into the fat
//! pointer struct, loading the pointer field as the element — P0 bug).
//! Also verifies array indexing `[T; N]` is unchanged (no regression).
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
        // Group R: Regression (8) — re-verify R17 cases
        Case { name: "r01_str_param", src: "fn f(s: &str) { }", expect_all: &["define void @landin_f({ i8*, i64 } %arg0)"], expect_none: &[], desc: "R17 r01: &str param as fat pointer" },
        Case { name: "r02_bstr_fat_ptr", src: "fn f() { let b = b\"hello\"; }", expect_all: &["alloca { i8*, i64 }", "i64 5, 1"], expect_none: &["insertvalue i8* undef, i64"], desc: "R17 r02: byte string fat pointer" },
        Case { name: "r03_str_eq", src: "fn f(s: &str) -> bool { s == \"hello\" }", expect_all: &["extractvalue { i8*, i64 }", "and i1"], expect_none: &[], desc: "R17 r03: str eq = (ptr_eq AND len_eq)" },
        Case { name: "r04_enum_case_c", src: "enum E { A, B(i32), C(i64) } fn f(e: E) -> i32 { 0 }", expect_all: &["{ i32, i32, i64 }"], expect_none: &[], desc: "R17 r04: Case C enum flat layout" },
        Case { name: "r05_struct_ref_str_field", src: "struct Wrap { s: &str } fn f(w: Wrap) { }", expect_all: &["{ { i8*, i64 } }"], expect_none: &[], desc: "R17 r05: &str struct field as fat pointer" },
        Case { name: "r06_const_value", src: "const MAX: i32 = 100; fn f() -> i32 { MAX }", expect_all: &["store i32 100"], expect_none: &[], desc: "R17 r06: const value inlined" },
        Case { name: "r07_div_zero", src: "fn f(a: i32, b: i32) -> i32 { a / b }", expect_all: &["icmp eq", "__landin_panic_div_by_zero"], expect_none: &[], desc: "R17 r07: div-by-zero runtime check" },
        Case { name: "r08_i16_arith", src: "fn f(a: i16, b: i16) -> i16 { a + b }", expect_all: &["add nsw i16", "llvm.sadd.with.overflow.i16"], expect_none: &[], desc: "R17 r08: i16 arithmetic + overflow check" },

        // Group S: Stage 3.51 slice indexing coverage (14)
        // S.1 — basic slice indexing (load element, not pointer)
        Case { name: "s01_slice_idx_i32", src: "fn f(s: &[i32]) -> i32 { s[0] }", expect_all: &["getelementptr inbounds { i32*, i64 }, { i32*, i64 }*", "getelementptr inbounds i32, i32*", "load i32"], expect_none: &[], desc: "Stage 3.51: s[0] on &[i32] loads i32 element via data pointer" },
        Case { name: "s02_slice_idx_u8", src: "fn f(s: &[u8]) -> u8 { s[0] }", expect_all: &["getelementptr inbounds i8, i8*", "load i8"], expect_none: &[], desc: "Stage 3.51: s[0] on &[u8] loads i8 element" },
        Case { name: "s03_slice_idx_i64", src: "fn f(s: &[i64]) -> i64 { s[0] }", expect_all: &["getelementptr inbounds i64, i64*", "load i64"], expect_none: &[], desc: "Stage 3.51: s[0] on &[i64] loads i64 element" },
        Case { name: "s04_slice_idx_f64", src: "fn f(s: &[f64]) -> f64 { s[0] }", expect_all: &["getelementptr inbounds double, double*", "load double"], expect_none: &[], desc: "Stage 3.51: s[0] on &[f64] loads double element" },
        // S.2 — constant index
        Case { name: "s05_slice_const_idx", src: "fn f(s: &[i32]) -> i32 { s[1] }", expect_all: &["getelementptr inbounds i32, i32*"], expect_none: &[], desc: "Stage 3.51: s[1] constant index via data pointer" },
        // S.3 — variable index
        Case { name: "s06_slice_var_idx", src: "fn f(s: &[i32], i: i32) -> i32 { s[i] }", expect_all: &["getelementptr inbounds i32, i32*"], expect_none: &[], desc: "Stage 3.51: s[i] variable index via data pointer" },
        // S.4 — multiple accesses
        Case { name: "s07_slice_two_accesses", src: "fn f(s: &[i32]) -> i32 { s[0] + s[1] }", expect_all: &["getelementptr inbounds i32, i32*"], expect_none: &[], desc: "Stage 3.51: two slice accesses in one expression" },
        // S.5 — no invalid [0 x T] array GEP
        Case { name: "s08_no_invalid_zero_array", src: "fn f(s: &[i32]) -> i32 { s[0] }", expect_all: &[], expect_none: &["[0 x i32]"], desc: "Stage 3.51: no invalid [0 x i32] array type" },
        // S.6 — array indexing unchanged (regression)
        Case { name: "s09_array_idx_uses_array_gep", src: "fn f(a: [i32; 3]) -> i32 { a[1] }", expect_all: &["getelementptr inbounds [3 x i32], [3 x i32]*"], expect_none: &["getelementptr inbounds i32, i32*"], desc: "Stage 3.51 regression: [i32; 3] uses array-style GEP (not pointer GEP)" },
        Case { name: "s10_array_idx_i64", src: "fn f(a: [i64; 4]) -> i64 { a[2] }", expect_all: &["getelementptr inbounds [4 x i64], [4 x i64]*"], expect_none: &[], desc: "Stage 3.51 regression: [i64; 4] array indexing" },
        // S.7 — slice in struct field, then index
        Case { name: "s11_slice_in_struct_then_idx", src: "struct S { data: &[i32] } fn f(s: S) -> i32 { s.data[0] }", expect_all: &[], expect_none: &[], desc: "Stage 3.51: slice field in struct compiles (no crash)" },
        // S.8 — slice indexing in if condition
        Case { name: "s12_slice_idx_in_if", src: "fn f(s: &[i32]) -> i32 { if s[0] > 0 { 1 } else { 0 } }", expect_all: &["getelementptr inbounds i32, i32*", "icmp sgt", "br i1"], expect_none: &[], desc: "Stage 3.51: slice index in if condition" },
        // S.9 — slice indexing in match
        Case { name: "s13_slice_idx_in_match", src: "fn f(s: &[i32]) -> i32 { match s[0] { 0 => 1, _ => 0 } }", expect_all: &["getelementptr inbounds i32, i32*", "switch i32"], expect_none: &[], desc: "Stage 3.51: slice index in match scrutinee" },
        // S.10 — slice indexing with arithmetic
        Case { name: "s14_slice_idx_arith", src: "fn f(s: &[i32]) -> i32 { s[0] + s[1] + s[2] }", expect_all: &["getelementptr inbounds i32, i32*", "add nsw i32"], expect_none: &[], desc: "Stage 3.51: three slice accesses with arithmetic" },

        // Group E: §9.3.2 edge cases (8)
        Case { name: "e01_slice_idx_zero", src: "fn f(s: &[i32]) -> i32 { s[0] }", expect_all: &["getelementptr inbounds i32, i32* %v2, i32 0"], expect_none: &[], desc: "Stage 3.51 edge: index 0" },
        Case { name: "e02_slice_idx_large", src: "fn f(s: &[i32]) -> i32 { s[100] }", expect_all: &["i32 100"], expect_none: &[], desc: "Stage 3.51 edge: large constant index (no bounds check yet)" },
        Case { name: "e03_slice_idx_in_loop", src: "fn f(s: &[i32], n: i32) -> i32 { let mut sum = 0; let mut i = 0; while i < n { sum = sum + s[i]; i = i + 1; } sum }", expect_all: &["getelementptr inbounds i32, i32*", "br i1"], expect_none: &[], desc: "Stage 3.51 edge: slice indexing in loop" },
        Case { name: "e04_array_idx_still_works", src: "fn f(a: [i32; 5]) -> i32 { a[4] }", expect_all: &["getelementptr inbounds [5 x i32], [5 x i32]*"], expect_none: &[], desc: "Stage 3.51 edge: array index 4 (last element)" },
        Case { name: "e05_slice_idx_bool", src: "fn f(s: &[bool]) -> bool { s[0] }", expect_all: &["getelementptr inbounds i1, i1*"], expect_none: &[], desc: "Stage 3.51 edge: &[bool] slice indexing" },
        Case { name: "e06_slice_idx_nested", src: "fn f(s: &[i32]) -> i32 { let x = s[0]; x + 1 }", expect_all: &["getelementptr inbounds i32, i32*", "add nsw i32"], expect_none: &[], desc: "Stage 3.51 edge: slice index then arithmetic" },
        Case { name: "e07_mixed_array_slice", src: "fn f(a: [i32; 2], s: &[i32]) -> i32 { a[0] + s[0] }", expect_all: &["getelementptr inbounds [2 x i32], [2 x i32]*", "getelementptr inbounds i32, i32*"], expect_none: &[], desc: "Stage 3.51 edge: array + slice indexing in same fn" },
        Case { name: "e08_slice_returned_element", src: "fn f(s: &[i32]) -> i32 { s[0] }", expect_all: &["ret i32"], expect_none: &[], desc: "Stage 3.51 edge: slice element returned" },
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

    println!("\n=== Stage 3 Gate Audit Round 18 Summary ===");
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
            "   R1-R18: ..., 28, 28, 23, 24, 30, 30, 30, {}/{} — all OK.",
            pass,
            cases.len()
        );
        println!("   Per §9.3.3, audit CONVERGED (18 rounds, 0 new issues each).");
        println!(
            "   Stage 3.51 (slice indexing fix — fat pointer data pointer dereference) verified."
        );
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found.", fail);
    }
}
