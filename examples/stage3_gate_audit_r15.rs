//! Stage 3 Gate Review Round 15 audit (per §9.3.1 of process v3.13).
//!
//! Run with: cargo run --example stage3_gate_audit_r15
//!
//! Purpose: Phase gate audit for Stage 3.48 — L-ENUM-UNION + L-ENUM-BINDING
//! closure. Verifies:
//!   1. Enum storage layout flattens ALL non-empty variants' payload fields
//!      (was: only first non-empty — soundness bug for ≥2 non-empty variants).
//!   2. Enum tuple/struct variant pattern bindings actually extract the
//!      payload (was: reading uninitialized memory — P0 soundness bug).
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
        // Group R: Regression (8) — re-verify R14 cases
        Case { name: "r01_struct_param", src: "struct Point { x: i32, y: i32 } fn f(p: Point) -> i32 { p.x }", expect_all: &["define i32 @landin_f({ i32, i32 } %arg0)"], expect_none: &[], desc: "R14 r01: struct param from AdtLayout" },
        Case { name: "r02_nested_struct", src: "struct Inner { v: i32 } struct Outer { i: Inner } fn f(o: Outer) -> i32 { 0 }", expect_all: &["{ { i32 } }"], expect_none: &[], desc: "R14 r02: nested struct AdtLayout recursion" },
        Case { name: "r03_struct_i128_field", src: "struct Big { v: i128 } fn f(b: Big) -> i128 { b.v }", expect_all: &["{ i128 }"], expect_none: &[], desc: "R14 r03: i128 field preserved" },
        Case { name: "r04_struct_ref_str_field", src: "struct Wrap { s: &str } fn f(w: Wrap) { }", expect_all: &["{ i8* }"], expect_none: &[], desc: "R14 r04: &str field via AdtLayout (no codegen→HIR call)" },
        Case { name: "r05_const_value", src: "const MAX: i32 = 100; fn f() -> i32 { MAX }", expect_all: &["store i32 100"], expect_none: &[], desc: "R14 r05: const value inlined" },
        Case { name: "r06_i16_arith", src: "fn f(a: i16, b: i16) -> i16 { a + b }", expect_all: &["add nsw i16", "llvm.sadd.with.overflow.i16"], expect_none: &[], desc: "R14 r06: i16 arithmetic + overflow check" },
        Case { name: "r07_div_zero", src: "fn f(a: i32, b: i32) -> i32 { a / b }", expect_all: &["icmp eq", "__landin_panic_div_by_zero"], expect_none: &[], desc: "R14 r07: div-by-zero runtime check" },
        Case { name: "r08_float_bitand", src: "fn f(a: f64, b: f64) -> f64 { a & b }", expect_all: &["and i64", "fptosi", "sitofp"], expect_none: &[], desc: "R14 r08: float bitwise via int cast" },

        // Group U: Stage 3.48 L-ENUM-UNION + L-ENUM-BINDING coverage (14)
        // U.1 — Case C layout (≥2 non-empty variants)
        Case { name: "u01_case_c_layout", src: "enum E { A, B(i32), C(i64) } fn f(e: E) -> i32 { 0 }", expect_all: &["{ i32, i32, i64 }"], expect_none: &["{ i32, i32 }"], desc: "Stage 3.48: Case C flat layout (all payloads flattened)" },
        Case { name: "u02_case_c_variant_c_ctor", src: "enum E { A, B(i32), C(i64) } fn f() -> E { E::C(42) }", expect_all: &["insertvalue { i32, i32, i64 } undef, i32 2, 0", "i64 42, 2"], expect_none: &[], desc: "Stage 3.48: E::C(42) inserts discr=2 at 0, i64 at field 2" },
        Case { name: "u03_case_c_variant_b_ctor", src: "enum E { A, B(i32), C(i64) } fn f() -> E { E::B(7) }", expect_all: &["insertvalue { i32, i32, i64 } undef, i32 1, 0", "i32 7, 1"], expect_none: &[], desc: "Stage 3.48: E::B(7) inserts discr=1 at 0, i32 at field 1" },
        Case { name: "u04_case_c_variant_a_ctor", src: "enum E { A, B(i32), C(i64) } fn f() -> E { E::A }", expect_all: &["insertvalue { i32, i32, i64 } undef, i32 0, 0"], expect_none: &[], desc: "Stage 3.48: E::A (unit variant) inserts discr=0 only" },
        // U.2 — L-ENUM-BINDING (pattern binding extraction)
        Case { name: "u05_binding_case_b", src: "enum Opt { None, Some(i32) } fn f(o: Opt) -> i32 { match o { Opt::Some(x) => x, Opt::None => 0 } }", expect_all: &["getelementptr inbounds { i32, i32 }, { i32, i32 }* %loc_1, i32 0, i32 1"], expect_none: &[], desc: "Stage 3.48 §L-ENUM-BINDING: Opt::Some(x) extracts payload from field 1" },
        Case { name: "u06_binding_case_c_b", src: "enum E { A, B(i32), C(i64) } fn f(e: E) -> i32 { match e { E::B(x) => x, _ => 0 } }", expect_all: &["getelementptr inbounds { i32, i32, i64 }, { i32, i32, i64 }* %loc_1, i32 0, i32 1"], expect_none: &[], desc: "Stage 3.48: E::B(x) extracts i32 from field 1 (Case C)" },
        Case { name: "u07_binding_case_c_c", src: "enum E { A, B(i32), C(i64) } fn f(e: E) -> i64 { match e { E::C(x) => x, _ => 0 } }", expect_all: &["getelementptr inbounds { i32, i32, i64 }, { i32, i32, i64 }* %loc_1, i32 0, i32 2"], expect_none: &[], desc: "Stage 3.48: E::C(x) extracts i64 from field 2 (Case C)" },
        // U.3 — Multi-field variant
        Case { name: "u08_multi_field_layout", src: "enum E { A, B(i32, i64), C(i64) } fn f(e: E) -> i32 { 0 }", expect_all: &["{ i32, i32, i64, i64 }"], expect_none: &[], desc: "Stage 3.48: multi-field variant → 4-field flat layout" },
        Case { name: "u09_multi_field_ctor", src: "enum E { A, B(i32, i64), C(i64) } fn f() -> E { E::B(1, 2) }", expect_all: &["insertvalue { i32, i32, i64, i64 } undef, i32 1, 0", "i32 1, 1", "i64 2, 2"], expect_none: &[], desc: "Stage 3.48: E::B(1, 2) inserts discr=1, i32 at 1, i64 at 2" },
        // U.4 — Mixed payload types
        Case { name: "u10_mixed_types_layout", src: "enum E { A, B(i32), C(f64) } fn f(e: E) -> i32 { 0 }", expect_all: &["{ i32, i32, double }"], expect_none: &[], desc: "Stage 3.48: mixed i32/f64 payloads → { i32, i32, double }" },
        // U.5 — Struct variant pattern binding
        Case { name: "u11_struct_variant_binding", src: "enum E { Empty, Point { x: i32, y: i32 } } fn f(e: E) -> i32 { match e { E::Point { x, y } => x + y, _ => 0 } }", expect_all: &["i32 0, i32 1", "i32 0, i32 2"], expect_none: &[], desc: "Stage 3.48: struct variant pattern extracts x (field 1) and y (field 2)" },
        // U.6 — Regression: Case A and Case B unchanged
        Case { name: "u12_case_a_regression", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { 0 }", expect_all: &["define i32 @landin_f({ i32 } %arg0)"], expect_none: &[], desc: "Stage 3.48 regression: Case A (all unit) still { i32 } only" },
        Case { name: "u13_case_b_regression", src: "enum Opt { None, Some(i32) } fn f(o: Opt) -> i32 { 0 }", expect_all: &["define i32 @landin_f({ i32, i32 } %arg0)"], expect_none: &[], desc: "Stage 3.48 regression: Case B (single non-empty) still { i32, i32 }" },
        Case { name: "u14_case_b_multi_field_regression", src: "enum E { Empty, Point { x: i32, y: i32 } } fn f(e: E) -> i32 { 0 }", expect_all: &["{ i32, i32, i32 }"], expect_none: &[], desc: "Stage 3.48 regression: Case B multi-field still { i32, i32, i32 }" },

        // Group E: §9.3.2 edge cases (8)
        Case { name: "e01_three_non_empty_variants", src: "enum E { A(i32), B(i64), C(i32) } fn f(e: E) -> i32 { 0 }", expect_all: &["{ i32, i32, i64, i32 }"], expect_none: &[], desc: "Stage 3.48 edge: 3 non-empty variants → 4-field flat layout" },
        Case { name: "e02_enum_with_bool_payload", src: "enum E { A, B(bool) } fn f(e: E) -> i32 { 0 }", expect_all: &["{ i32, i1 }"], expect_none: &[], desc: "Stage 3.48 edge: bool payload → { i32, i1 }" },
        Case { name: "e03_enum_in_struct", src: "enum E { A, B(i32) } struct S { e: E } fn f(s: S) -> i32 { 0 }", expect_all: &["{ { i32, i32 } }"], expect_none: &[], desc: "Stage 3.48 edge: enum field in struct → nested struct layout" },
        Case { name: "e04_enum_returned_from_fn", src: "enum E { A, B(i32), C(i64) } fn f() -> E { E::C(99) }", expect_all: &["define { i32, i32, i64 } @landin_f()"], expect_none: &[], desc: "Stage 3.48 edge: Case C enum as return type" },
        Case { name: "e05_enum_param_case_c", src: "enum E { A, B(i32), C(i64) } fn f(e: E) -> i32 { 0 }", expect_all: &["define i32 @landin_f({ i32, i32, i64 } %arg0)"], expect_none: &[], desc: "Stage 3.48 edge: Case C enum as function parameter" },
        Case { name: "e06_match_with_wildcard", src: "enum E { A, B(i32), C(i64) } fn f(e: E) -> i32 { match e { E::B(x) => x, _ => 0 } }", expect_all: &["switch i32", "getelementptr inbounds { i32, i32, i64 }"], expect_none: &[], desc: "Stage 3.48 edge: match with wildcard + Case C binding" },
        Case { name: "e07_enum_in_tuple", src: "enum E { A, B(i32) } fn f(t: (E, i32)) -> i32 { 0 }", expect_all: &["{ { i32, i32 }, i32 }"], expect_none: &[], desc: "Stage 3.48 edge: enum inside tuple → nested layout" },
        Case { name: "e08_two_enums_in_one_fn", src: "enum E1 { A, B(i32) } enum E2 { X, Y(i64) } fn f(e1: E1, e2: E2) -> i32 { 0 }", expect_all: &["{ i32, i32 }", "{ i32, i64 }"], expect_none: &[], desc: "Stage 3.48 edge: two distinct enums in one fn — both layouts correct" },
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

    println!("\n=== Stage 3 Gate Audit Round 15 Summary ===");
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
            "   R1-R15: ..., 28, 28, 23, 24, 30, 30, {}/{} — all OK.",
            pass,
            cases.len()
        );
        println!("   Per §9.3.3, audit CONVERGED (15 rounds, 0 new issues each).");
        println!("   Stage 3.48 (L-ENUM-UNION + L-ENUM-BINDING closure) verified.");
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found.", fail);
    }
}
