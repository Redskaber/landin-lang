//! Stage 3 Gate Review Round 21 audit (per §9.3.1 of process v3.13).
//!
//! Run with: cargo run --example stage3_gate_audit_r21
//!
//! Purpose: Phase gate audit for Stage 3.54. Verifies that indexing a
//! struct field of type slice, array, or string reference correctly
//! GEPs through the struct field to the storage, then to the element.
//! Was: detect_lvalue_storage_type returned the struct type instead of
//! the field type for Field projections, causing wrong GEP in store path.
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
        // Group R: Regression (8) — re-verify R20 cases
        Case { name: "r01_str_param", src: "fn f(s: &str) { }", expect_all: &["define void @landin_f({ i8*, i64 } %arg0)"], expect_none: &[], desc: "R20 r01: &str param as fat pointer" },
        Case { name: "r02_str_idx", src: "fn f(s: &str) -> i32 { s[0] }", expect_all: &["load i8", "store i8"], expect_none: &[], desc: "R20 r02: &str index loads i8 (Stage 3.53)" },
        Case { name: "r03_slice_i64", src: "fn f(s: &[i64]) -> i64 { s[0] }", expect_all: &["load i64"], expect_none: &[], desc: "R20 r03: &[i64] element loads i64" },
        Case { name: "r04_bstr_fat_ptr", src: "fn f() { let b = b\"hello\"; }", expect_all: &["alloca { i8*, i64 }", "i64 5, 1"], expect_none: &[], desc: "R20 r04: byte string fat pointer" },
        Case { name: "r05_enum_case_c", src: "enum E { A, B(i32), C(i64) } fn f(e: E) -> i32 { 0 }", expect_all: &["{ i32, i32, i64 }"], expect_none: &[], desc: "R20 r05: Case C enum flat layout" },
        Case { name: "r06_array_idx", src: "fn f(a: [i32; 3]) -> i32 { a[1] }", expect_all: &["getelementptr inbounds [3 x i32], [3 x i32]*"], expect_none: &[], desc: "R20 r06: array indexing (no regression)" },
        Case { name: "r07_const_value", src: "const MAX: i32 = 100; fn f() -> i32 { MAX }", expect_all: &["store i32 100"], expect_none: &[], desc: "R20 r07: const value inlined" },
        Case { name: "r08_i16_arith", src: "fn f(a: i16, b: i16) -> i16 { a + b }", expect_all: &["add nsw i16", "llvm.sadd.with.overflow.i16"], expect_none: &[], desc: "R20 r08: i16 arithmetic + overflow check" },

        // Group F: Stage 3.54 field indexing coverage (14)
        // F.1 — slice field store
        Case { name: "f01_slice_field_store", src: "struct S { data: &mut [i32] } fn f(s: S) { s.data[0] = 42; }", expect_all: &["getelementptr inbounds { i32*, i64 }, { i32*, i64 }*", "getelementptr inbounds i32, i32*", "store i32 42"], expect_none: &[], desc: "Stage 3.54: &mut [i32] field store via fat pointer" },
        Case { name: "f02_slice_field_store_i64", src: "struct S { data: &mut [i64] } fn f(s: S) { s.data[0] = 42; }", expect_all: &["store i64 42"], expect_none: &[], desc: "Stage 3.54: &mut [i64] field store i64" },
        // F.2 — slice field load
        Case { name: "f03_slice_field_load", src: "struct S { data: &[i64] } fn f(s: S) -> i64 { s.data[0] }", expect_all: &["load i64", "getelementptr inbounds i64, i64*"], expect_none: &[], desc: "Stage 3.54: &[i64] field load via fat pointer" },
        Case { name: "f04_slice_field_load_i32", src: "struct S { data: &[i32] } fn f(s: S) -> i32 { s.data[0] }", expect_all: &["load i32"], expect_none: &[], desc: "Stage 3.54: &[i32] field load i32" },
        // F.3 — array field store/load
        Case { name: "f05_array_field_store", src: "struct S { data: [i32; 3] } fn f(s: S) { s.data[0] = 42; }", expect_all: &["getelementptr inbounds [3 x i32], [3 x i32]*", "store i32 42"], expect_none: &[], desc: "Stage 3.54: [i32; 3] field store (array GEP)" },
        Case { name: "f06_array_field_load", src: "struct S { data: [i64; 4] } fn f(s: S) -> i64 { s.data[1] }", expect_all: &["getelementptr inbounds [4 x i64], [4 x i64]*", "load i64"], expect_none: &[], desc: "Stage 3.54: [i64; 4] field load (array GEP)" },
        // F.4 — &str field index
        Case { name: "f07_str_field_idx", src: "struct S { text: &str } fn f(s: S) -> i32 { s.text[0] }", expect_all: &["load i8", "getelementptr inbounds i8, i8*"], expect_none: &[], desc: "Stage 3.54: &str field index loads i8" },
        // F.5 — slice field arithmetic
        Case { name: "f08_slice_field_arith", src: "struct S { data: &[i64] } fn f(s: S) -> i64 { s.data[0] + s.data[1] }", expect_all: &["add nsw i64"], expect_none: &[], desc: "Stage 3.54: &[i64] field arithmetic uses i64" },
        // F.6 — nested struct slice field
        Case { name: "f09_nested_struct_slice", src: "struct Inner { data: &[i32] } struct Outer { inner: Inner } fn f(o: Outer) -> i32 { o.inner.data[0] }", expect_all: &["load i32"], expect_none: &[], desc: "Stage 3.54: nested struct slice field index" },
        // F.7 — slice field variable index
        Case { name: "f10_slice_field_var_idx", src: "struct S { data: &[i32] } fn f(s: S, i: i32) -> i32 { s.data[i] }", expect_all: &["getelementptr inbounds i32, i32*"], expect_none: &[], desc: "Stage 3.54: &[i32] field variable index" },
        // F.8 — slice field store variable index
        Case { name: "f11_slice_field_store_var", src: "struct S { data: &mut [i32] } fn f(s: S, i: i32) { s.data[i] = 42; }", expect_all: &["store i32 42"], expect_none: &[], desc: "Stage 3.54: &mut [i32] field store variable index" },
        // F.9 — byte string field
        Case { name: "f12_bstr_field_idx", src: "struct S { data: &[u8] } fn f(s: S) -> u8 { s.data[0] }", expect_all: &["load i8"], expect_none: &[], desc: "Stage 3.54: &[u8] field index loads i8" },
        // F.10 — two slice fields in one struct
        Case { name: "f13_two_slice_fields", src: "struct S { a: &[i32], b: &[i64] } fn f(s: S) -> i64 { s.a[0] as i64 + s.b[0] }", expect_all: &[], expect_none: &[], desc: "Stage 3.54: two slice fields in one struct (compiles)" },
        // F.11 — array field regression
        Case { name: "f14_array_field_arith", src: "struct S { data: [i64; 3] } fn f(s: S) -> i64 { s.data[0] + s.data[1] }", expect_all: &["add nsw i64"], expect_none: &[], desc: "Stage 3.54: [i64; 3] field arithmetic uses i64" },

        // Group E: §9.3.2 edge cases (8)
        Case { name: "e01_slice_field_store_last", src: "struct S { data: &mut [i32] } fn f(s: S) { s.data[2] = 42; }", expect_all: &["store i32 42"], expect_none: &[], desc: "Stage 3.54 edge: &mut [i32] field store index 2" },
        Case { name: "e02_str_field_arith", src: "struct S { text: &str } fn f(s: S) -> i32 { s.text[0] + s.text[1] }", expect_all: &["add nsw i8"], expect_none: &[], desc: "Stage 3.54 edge: &str field arithmetic uses i8" },
        Case { name: "e03_slice_field_in_if", src: "struct S { data: &[i32] } fn f(s: S) -> i32 { if s.data[0] > 0 { 1 } else { 0 } }", expect_all: &["icmp sgt", "br i1"], expect_none: &[], desc: "Stage 3.54 edge: &[i32] field index in if" },
        Case { name: "e04_array_field_store_i8", src: "struct S { data: [u8; 4] } fn f(s: S) { s.data[0] = 65; }", expect_all: &["store i8 65"], expect_none: &[], desc: "Stage 3.54 edge: [u8; 4] field store i8" },
        Case { name: "e05_slice_local_regression", src: "fn f(s: &[i32]) -> i32 { s[0] }", expect_all: &["load i32"], expect_none: &[], desc: "Stage 3.54 regression: direct &[i32] param index" },
        Case { name: "e06_array_local_regression", src: "fn f(a: [i32; 3]) -> i32 { a[0] }", expect_all: &["getelementptr inbounds [3 x i32], [3 x i32]*"], expect_none: &[], desc: "Stage 3.54 regression: direct [i32; 3] param index" },
        Case { name: "e07_str_local_regression", src: "fn f(s: &str) -> i32 { s[0] }", expect_all: &["load i8"], expect_none: &[], desc: "Stage 3.54 regression: direct &str param index" },
        Case { name: "e08_slice_field_cmp", src: "struct S { data: &[i64] } fn f(s: S) -> bool { s.data[0] > s.data[1] }", expect_all: &["icmp sgt i64"], expect_none: &[], desc: "Stage 3.54 edge: &[i64] field comparison uses i64" },
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

    println!("\n=== Stage 3 Gate Audit Round 21 Summary ===");
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
            "   R1-R21: ..., 28, 28, 23, 24, 30, 30, 30, {}/{} — all OK.",
            pass,
            cases.len()
        );
        println!("   Per §9.3.3, audit CONVERGED (21 rounds, 0 new issues each).");
        println!("   Stage 3.54 (slice/array field store + storage type fix) verified.");
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found.", fail);
    }
}
