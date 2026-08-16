//! Stage 3 Gate Review Round 8 audit (per §9.3.1 of process v3.4).
//!
//! Run with: cargo run --example stage3_gate_audit_r8
//!
//! Purpose: Phase gate audit for Stage 3 codegen after Stage 3.38
//! (L-ENUM: Enum variant codegen).
//!
//! Per §9.3.3, R7 was CONVERGED (7 consecutive rounds). R8 is run because
//! Stage 3.38 closed the L-ENUM feature gap — significant new IR shape.
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
        (false, format!("FAIL — no HIR produced: {}", c.desc))
    }
}

fn main() {
    let cases: &[Case] = &[
        // Group R: Regression (8)
        Case { name: "r01_const_return", src: "fn main() -> i32 { 42 }", expect_all: &["ret i32"], expect_none: &[], desc: "R7 r01: constant return" },
        Case { name: "r02_struct_construction", src: "struct Point { x: i32, y: i32 } fn f() -> i32 { let p = Point { x: 1, y: 2 }; p.x }", expect_all: &["insertvalue { i32, i32 } undef, i32 1, 0"], expect_none: &[], desc: "R7 r02: struct construction" },
        Case { name: "r03_field_mutation", src: "struct Acc { v: i64 } fn f() -> i64 { let mut a = Acc { v: 0 }; a.v = 42; a.v }", expect_all: &["store i64 42", "getelementptr inbounds { i64 }"], expect_none: &[], desc: "R7 r03: field mutation works" },
        Case { name: "r04_field_arithmetic_i64", src: "struct Acc { v: i64 } fn f() -> i64 { let a = Acc { v: 10 }; a.v + 5 }", expect_all: &["add nsw i64"], expect_none: &["add nsw i32"], desc: "R7 r04: i64 field arithmetic" },
        Case { name: "r05_string_literal", src: "fn f() { let s = \"hello\"; }", expect_all: &["@.str.0 = private unnamed_addr constant [5 x i8] c\"hello\""], expect_none: &[], desc: "R7 r05: string literal" },
        Case { name: "r06_overflow_check", src: "fn f(a: i32, b: i32) -> i32 { a + b }", expect_all: &["llvm.sadd.with.overflow.i32"], expect_none: &[], desc: "R7 r06: overflow check" },
        Case { name: "r07_div_zero_check", src: "fn f(a: i32, b: i32) -> i32 { a / b }", expect_all: &["icmp eq", "call void @__landin_panic_div_by_zero"], expect_none: &[], desc: "R7 r07: div-by-zero check" },
        Case { name: "r08_if_else_merge", src: "fn f(x: i32) -> i32 { if x > 0 { 1 } else { 2 } }", expect_all: &["br i1"], expect_none: &["ret i32 2"], desc: "R7 r08: if-else merge" },

        // Group E: Stage 3.38 L-ENUM — Enum variant codegen (10)
        Case { name: "e01_enum_unit_variant_red", src: "enum Color { Red, Green, Blue } fn f() { let c = Color::Red; }", expect_all: &["insertvalue { i32 } undef, i32 0, 0"], expect_none: &[], desc: "Stage 3.38: unit variant Red (discriminant 0)" },
        Case { name: "e02_enum_unit_variant_green", src: "enum Color { Red, Green, Blue } fn f() { let c = Color::Green; }", expect_all: &["insertvalue { i32 } undef, i32 1, 0"], expect_none: &[], desc: "Stage 3.38: unit variant Green (discriminant 1)" },
        Case { name: "e03_enum_unit_variant_blue", src: "enum Color { Red, Green, Blue } fn f() { let c = Color::Blue; }", expect_all: &["insertvalue { i32 } undef, i32 2, 0"], expect_none: &[], desc: "Stage 3.38: unit variant Blue (discriminant 2)" },
        Case { name: "e04_enum_tuple_variant_some", src: "enum Opt { Some(i32), None } fn f() { let o = Opt::Some(42); }", expect_all: &["insertvalue { i32, i32 } undef, i32 0, 0", "insertvalue { i32, i32 } %v1, i32 42, 1"], expect_none: &[], desc: "Stage 3.38: tuple variant Some(42) (discriminant 0 + payload)" },
        Case { name: "e05_enum_tuple_variant_none", src: "enum Opt { Some(i32), None } fn f() { let o = Opt::None; }", expect_all: &["insertvalue { i32 } undef, i32 1, 0"], expect_none: &[], desc: "Stage 3.38: unit variant None (discriminant 1)" },
        Case { name: "e06_enum_alloca_type", src: "enum Color { Red, Green, Blue } fn f() { let c = Color::Red; }", expect_all: &["alloca { i32 }"], expect_none: &[], desc: "Stage 3.38: enum alloca has { i32 } type" },
        Case { name: "e07_enum_tuple_alloca_type", src: "enum Opt { Some(i32), None } fn f() { let o = Opt::Some(42); }", expect_all: &["alloca { i32, i32 }"], expect_none: &[], desc: "Stage 3.38: enum with tuple variant has { i32, i32 } alloca" },
        Case { name: "e08_enum_store_correct_type", src: "enum Color { Red, Green, Blue } fn f() { let c = Color::Red; }", expect_all: &["store { i32 }"], expect_none: &[], desc: "Stage 3.38: enum variant store uses { i32 } type" },
        Case { name: "e09_enum_i64_payload", src: "enum Opt { Some(i64), None } fn f() { let o = Opt::Some(42); }", expect_all: &["insertvalue { i32, i64 }"], expect_none: &[], desc: "Stage 3.38: enum with i64 payload has { i32, i64 } struct" },
        Case { name: "e10_enum_multiple_variants", src: "enum Color { Red, Green, Blue } fn f() { let a = Color::Red; let b = Color::Green; let c = Color::Blue; }", expect_all: &["i32 0, 0", "i32 1, 0", "i32 2, 0"], expect_none: &[], desc: "Stage 3.38: multiple enum variants with correct discriminants" },

        // Group X: §9.3.2 edge cases (5)
        Case { name: "x01_no_i32_store_for_enum", src: "enum Color { Red, Green, Blue } fn f() { let c = Color::Red; }", expect_all: &[], expect_none: &["store i32 0, %loc_1"], desc: "Stage 3.38 edge: no raw 'store i32' for enum variant" },
        Case { name: "x02_enum_discriminant_correct", src: "enum Opt { Some(i32), None } fn f() { let o = Opt::Some(42); }", expect_all: &["i32 0, 0"], expect_none: &["i32 1, 0\n  %v1"], desc: "Stage 3.38 edge: Some variant has discriminant 0" },
        Case { name: "x03_enum_none_discriminant", src: "enum Opt { Some(i32), None } fn f() { let o = Opt::None; }", expect_all: &["i32 1, 0"], expect_none: &[], desc: "Stage 3.38 edge: None variant has discriminant 1" },
        Case { name: "x04_enum_struct_type", src: "enum Color { Red, Green, Blue } fn f() { let c = Color::Red; }", expect_all: &["alloca { i32 }", "store { i32 }"], expect_none: &["alloca i32\n  %loc_1"], desc: "Stage 3.38 edge: enum type is { i32 } struct" },
        Case { name: "x05_enum_with_float_payload", src: "enum Val { F(f64), N } fn f() { let v = Val::F(3.14); }", expect_all: &["insertvalue { i32, double }"], expect_none: &[], desc: "Stage 3.38 edge: enum with f64 payload has { i32, double } struct" },

        // Group H: Adversarial (5)
        Case { name: "h01_enum_in_if", src: "enum Color { Red, Green, Blue } fn f(c: bool) { if c { let _ = Color::Red; } else { let _ = Color::Green; } }", expect_all: &["insertvalue { i32 } undef, i32 0, 0", "insertvalue { i32 } undef, i32 1, 0", "br i1"], expect_none: &[], desc: "Adversarial: enum variants in if branches" },
        Case { name: "h02_enum_in_function", src: "enum Color { Red, Green, Blue } fn make() -> Color { Color::Red } fn f() { let c = make(); }", expect_all: &["define { i32 } @landin_make", "call { i32 } @landin_make"], expect_none: &[], desc: "Adversarial: enum as function return type" },
        Case { name: "h03_enum_as_param", src: "enum Color { Red, Green, Blue } fn identify(c: Color) { } fn f() { identify(Color::Red); }", expect_all: &["define void @landin_identify({ i32 } %arg0)"], expect_none: &[], desc: "Adversarial: enum as function parameter" },
        Case { name: "h04_enum_multiple_enums", src: "enum A { X, Y } enum B { P, Q } fn f() { let a = A::X; let b = B::P; }", expect_all: &["insertvalue { i32 } undef, i32 0, 0"], expect_none: &[], desc: "Adversarial: multiple enums in same function" },
        Case { name: "h05_enum_with_struct_variant", src: "enum Shape { Circle { r: f64 }, Square { s: f64 } } fn f() { let s = Shape::Circle { r: 1.0 }; }", expect_all: &["alloca { i32, double }", "insertvalue { i32, double }"], expect_none: &[], desc: "Adversarial: enum with struct variant" },
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

    println!("\n=== Stage 3 Gate Audit Round 8 Summary ===");
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
            "   R1-R8: 38/38, 43/43, 43/43, 37/37, 30/30, 30/30, 28/28, {}/{} — all OK.",
            pass,
            cases.len()
        );
        println!("   Per §9.3.3, audit CONVERGED (8 rounds, 0 new issues each).");
        println!("   L-ENUM feature verified: enum variant codegen works.");
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found, need fixes.", fail);
    }
}
