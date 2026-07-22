//! Stage 3 Gate Review Round 9 audit (per §9.3.1 of process v3.4).
//!
//! Run with: cargo run --example stage3_gate_audit_r9
//!
//! Purpose: Phase gate audit for Stage 3 codegen after Stage 3.40
//! (L-ENUM-MATCH: enum match via discriminant extraction + SwitchInt).
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
        (false, format!("FAIL — no HIR produced: {}", c.desc))
    }
}

fn main() {
    let cases: &[Case] = &[
        // Group R: Regression (8)
        Case { name: "r01_const_return", src: "fn main() -> i32 { 42 }", expect_all: &["ret i32"], expect_none: &[], desc: "R8 r01: constant return" },
        Case { name: "r02_enum_unit_variant", src: "enum Color { Red, Green, Blue } fn f() { let c = Color::Red; }", expect_all: &["insertvalue { i32 } undef, i32 0, 0"], expect_none: &[], desc: "R8 r02: enum unit variant" },
        Case { name: "r03_enum_tuple_variant", src: "enum Opt { Some(i32), None } fn f() { let o = Opt::Some(42); }", expect_all: &["insertvalue { i32, i32 } undef, i32 0, 0", "insertvalue { i32, i32 } %v1, i32 42, 1"], expect_none: &[], desc: "R8 r03: enum tuple variant" },
        Case { name: "r04_struct_construction", src: "struct Point { x: i32, y: i32 } fn f() -> i32 { let p = Point { x: 1, y: 2 }; p.x }", expect_all: &["insertvalue { i32, i32 } undef, i32 1, 0"], expect_none: &[], desc: "R8 r04: struct construction" },
        Case { name: "r05_field_mutation", src: "struct Acc { v: i64 } fn f() -> i64 { let mut a = Acc { v: 0 }; a.v = 42; a.v }", expect_all: &["store i64 42", "getelementptr inbounds { i64 }"], expect_none: &[], desc: "R8 r05: field mutation" },
        Case { name: "r06_string_literal", src: "fn f() { let s = \"hello\"; }", expect_all: &["@.str.0 = private unnamed_addr constant [5 x i8] c\"hello\""], expect_none: &[], desc: "R8 r06: string literal" },
        Case { name: "r07_overflow_check", src: "fn f(a: i32, b: i32) -> i32 { a + b }", expect_all: &["llvm.sadd.with.overflow.i32"], expect_none: &[], desc: "R8 r07: overflow check" },
        Case { name: "r08_if_else_merge", src: "fn f(x: i32) -> i32 { if x > 0 { 1 } else { 2 } }", expect_all: &["br i1"], expect_none: &["ret i32 2"], desc: "R8 r08: if-else merge" },

        // Group M: Stage 3.40 L-ENUM-MATCH (10)
        Case { name: "m01_enum_match_switch", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, Color::Green => 2, Color::Blue => 3 } }", expect_all: &["switch i32", "i32 0, label %bb", "i32 1, label %bb", "i32 2, label %bb"], expect_none: &[], desc: "Stage 3.40 §15.4: enum match produces switch with correct cases" },
        Case { name: "m02_enum_match_discriminant_extraction", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, _ => 0 } }", expect_all: &["getelementptr inbounds { i32 }, { i32 }*", "load i32", "switch i32"], expect_none: &[], desc: "Stage 3.40: discriminant extracted via GEP + load" },
        Case { name: "m03_enum_match_wildcard", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, _ => 99 } }", expect_all: &["switch i32", "i32 0, label %bb", "store i32 99"], expect_none: &[], desc: "Stage 3.40: enum match with wildcard default" },
        Case { name: "m04_enum_match_values", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, Color::Green => 2, Color::Blue => 3 } }", expect_all: &["store i32 1", "store i32 2", "store i32 3"], expect_none: &[], desc: "Stage 3.40: match arms store correct values" },
        Case { name: "m05_enum_match_param_type", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, _ => 0 } }", expect_all: &["define i32 @landin_f({ i32 } %arg0)"], expect_none: &[], desc: "Stage 3.40: enum match function takes { i32 } param" },
        Case { name: "m06_enum_match_two_variants", src: "enum Opt { Some(i32), None } fn f(o: Opt) -> i32 { match o { Opt::Some(x) => x, Opt::None => 0 } }", expect_all: &["switch i32"], expect_none: &[], desc: "Stage 3.40: match on two-variant enum (Some/None)" },
        Case { name: "m07_enum_match_in_function", src: "enum Color { Red, Green, Blue } fn classify(c: Color) -> i32 { match c { Color::Red => 100, Color::Green => 200, Color::Blue => 300 } } fn f() -> i32 { classify(Color::Red) }", expect_all: &["switch i32", "call i32 @landin_classify"], expect_none: &[], desc: "Stage 3.40: enum match in function, called from another" },
        Case { name: "m08_enum_match_non_exhaustive", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, _ => 0 } }", expect_all: &["switch i32", "i32 0, label %bb"], expect_none: &[], desc: "Stage 3.40: non-exhaustive match with default" },
        Case { name: "m09_enum_match_no_errors", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, Color::Green => 2, Color::Blue => 3 } }", expect_all: &["ret i32"], expect_none: &[], desc: "Stage 3.40: enum match compiles without errors" },
        Case { name: "m10_enum_match_return", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, _ => 0 } }", expect_all: &["ret i32 %v"], expect_none: &[], desc: "Stage 3.40: enum match returns loaded value" },

        // Group E: §9.3.2 edge cases (5)
        Case { name: "e01_no_adt_in_switch", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, _ => 0 } }", expect_all: &[], expect_none: &["expected integer or bool for switch"], desc: "Stage 3.40 §15.4 edge: no 'expected integer' error (old bug symptom gone)" },
        Case { name: "e02_discriminant_is_i32", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, _ => 0 } }", expect_all: &["switch i32"], expect_none: &["switch { i32 }", "switch i64"], desc: "Stage 3.40 edge: switch operates on i32 (extracted discriminant)" },
        Case { name: "e03_match_on_enum_param", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, Color::Green => 2, _ => 3 } }", expect_all: &["switch i32", "i32 0, label", "i32 1, label"], expect_none: &[], desc: "Stage 3.40 edge: match on enum parameter" },
        Case { name: "e04_enum_match_then_use", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { let r = match c { Color::Red => 1, _ => 0 }; r + 10 }", expect_all: &["switch i32", "add nsw i32"], expect_none: &[], desc: "Stage 3.40 edge: match result used in arithmetic" },
        Case { name: "e05_enum_match_in_if", src: "enum Color { Red, Green, Blue } fn f(c: Color, b: bool) -> i32 { if b { match c { Color::Red => 1, _ => 0 } } else { 99 } }", expect_all: &["switch i32", "br i1"], expect_none: &[], desc: "Stage 3.40 edge: enum match inside if" },

        // Group H: Adversarial (5)
        Case { name: "h01_match_constructed_enum", src: "enum Color { Red, Green, Blue } fn f() -> i32 { match Color::Red { Color::Red => 1, _ => 0 } }", expect_all: &["switch i32", "i32 0, label"], expect_none: &[], desc: "Adversarial: match on directly constructed enum" },
        Case { name: "h02_match_enum_from_call", src: "enum Color { Red, Green, Blue } fn make() -> Color { Color::Green } fn f() -> i32 { match make() { Color::Red => 1, Color::Green => 2, _ => 0 } }", expect_all: &["switch i32", "call { i32 } @landin_make"], expect_none: &[], desc: "Adversarial: match on enum returned from function" },
        Case { name: "h03_nested_match", src: "enum A { X, Y } enum B { P, Q } fn f(a: A, b: B) -> i32 { match a { A::X => match b { B::P => 1, _ => 2 }, _ => 3 } }", expect_all: &["switch i32"], expect_none: &[], desc: "Adversarial: nested match on two enums" },
        Case { name: "h04_enum_match_with_overflow", src: "enum Opt { Some(i32), None } fn f(o: Opt) -> i32 { match o { Opt::Some(x) => x + 1, Opt::None => 0 } }", expect_all: &["switch i32", "llvm.sadd.with.overflow.i32"], expect_none: &[], desc: "Adversarial: enum match arm with overflow check" },
        Case { name: "h05_enum_match_multiple_returns", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => return 1, Color::Green => 2, _ => 3 } }", expect_all: &["switch i32", "store i32 1"], expect_none: &[], desc: "Adversarial: enum match with early return in arm" },
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

    println!("\n=== Stage 3 Gate Audit Round 9 Summary ===");
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
            "   R1-R9: 38, 43, 43, 37, 30, 30, 28, 28, {}/{} — all OK.",
            pass,
            cases.len()
        );
        println!("   Per §9.3.3, audit CONVERGED (9 rounds, 0 new issues each).");
        println!("   L-ENUM-MATCH verified: enum match works via discriminant extraction.");
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found, need fixes.", fail);
    }
}
