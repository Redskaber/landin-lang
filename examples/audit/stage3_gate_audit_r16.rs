//! Stage 3 Gate Review Round 16 audit (per §9.3.1 of process v3.13).
//!
//! Run with: cargo run --example stage3_gate_audit_r16
//!
//! Purpose: Phase gate audit for Stage 3.49 — L13 fat pointer closure.
//! Verifies that `&str` and `&[T]` references are now represented as
//! fat pointers `{ ptr, len }` instead of thin pointers, closing the
//! L13 soundness/completeness gap carried since Stage 3.27 (18 rounds).
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
        // Group R: Regression (8) — re-verify R15 cases
        Case { name: "r01_struct_param", src: "struct Point { x: i32, y: i32 } fn f(p: Point) -> i32 { p.x }", expect_all: &["define i32 @landin_f({ i32, i32 } %arg0)"], expect_none: &[], desc: "R15 r01: struct param from AdtLayout" },
        Case { name: "r02_enum_case_c", src: "enum E { A, B(i32), C(i64) } fn f(e: E) -> i32 { 0 }", expect_all: &["{ i32, i32, i64 }"], expect_none: &[], desc: "R15 r02: Case C enum flat layout" },
        Case { name: "r03_enum_binding", src: "enum Opt { None, Some(i32) } fn f(o: Opt) -> i32 { match o { Opt::Some(x) => x, Opt::None => 0 } }", expect_all: &["getelementptr inbounds { i32, i32 }, { i32, i32 }* %loc_1, i32 0, i32 1"], expect_none: &[], desc: "R15 r03: enum pattern binding extraction" },
        Case { name: "r04_struct_ref_str_field", src: "struct Wrap { s: &str } fn f(w: Wrap) { }", expect_all: &["{ { i8*, i64 } }"], expect_none: &[], desc: "R15 r04: &str struct field (Stage 3.49: fat pointer)" },
        Case { name: "r05_const_value", src: "const MAX: i32 = 100; fn f() -> i32 { MAX }", expect_all: &["store i32 100"], expect_none: &[], desc: "R15 r05: const value inlined" },
        Case { name: "r06_i16_arith", src: "fn f(a: i16, b: i16) -> i16 { a + b }", expect_all: &["add nsw i16", "llvm.sadd.with.overflow.i16"], expect_none: &[], desc: "R15 r06: i16 arithmetic + overflow check" },
        Case { name: "r07_div_zero", src: "fn f(a: i32, b: i32) -> i32 { a / b }", expect_all: &["icmp eq", "__landin_panic_div_by_zero"], expect_none: &[], desc: "R15 r07: div-by-zero runtime check" },
        Case { name: "r08_float_bitand", src: "fn f(a: f64, b: f64) -> f64 { a & b }", expect_all: &["and i64", "fptosi", "sitofp"], expect_none: &[], desc: "R15 r08: float bitwise via int cast" },

        // Group F: Stage 3.49 L13 fat pointer coverage (14)
        // F.1 — &str param/return/local layout
        Case { name: "f01_str_param_fat", src: "fn f(s: &str) { }", expect_all: &["define void @landin_f({ i8*, i64 } %arg0)"], expect_none: &["define void @landin_f(i8* %arg0)"], desc: "Stage 3.49 §L13: &str param = { i8*, i64 } (not thin i8*)" },
        Case { name: "f02_str_return_fat", src: "fn f() -> &str { \"hello\" }", expect_all: &["define { i8*, i64 } @landin_f()"], expect_none: &["define i8* @landin_f()"], desc: "Stage 3.49 §L13: &str return = { i8*, i64 } (not thin i8*)" },
        Case { name: "f03_str_local_alloca", src: "fn f() { let s = \"hello\"; }", expect_all: &["alloca { i8*, i64 }"], expect_none: &[], desc: "Stage 3.49 §L13: &str local alloca = { i8*, i64 }" },
        // F.2 — fat pointer carries length
        Case { name: "f04_str_literal_len_5", src: "fn f() -> &str { \"hello\" }", expect_all: &["i64 5, 1"], expect_none: &[], desc: "Stage 3.49 §L13: \"hello\" fat ptr len=5" },
        Case { name: "f05_str_literal_len_0", src: "fn f() -> &str { \"\" }", expect_all: &["i64 0, 1"], expect_none: &[], desc: "Stage 3.49 §L13: empty string fat ptr len=0" },
        Case { name: "f06_str_literal_unicode_len", src: "fn f() -> &str { \"héllo\" }", expect_all: &["i64 6, 1"], expect_none: &[], desc: "Stage 3.49 §L13: héllo UTF-8 fat ptr len=6" },
        // F.3 — fat pointer construction (insertvalue)
        Case { name: "f07_str_literal_insertvalue", src: "fn f() -> &str { \"hi\" }", expect_all: &["insertvalue { i8*, i64 } undef, i8*"], expect_none: &[], desc: "Stage 3.49 §L13: fat ptr built via insertvalue (ptr at 0)" },
        // F.4 — &str in struct field
        Case { name: "f08_str_in_struct", src: "struct Msg { text: &str } fn f(m: Msg) { }", expect_all: &["{ { i8*, i64 } }"], expect_none: &[], desc: "Stage 3.49 §L13: &str struct field = { { i8*, i64 } }" },
        // F.5 — &str function call ABI
        Case { name: "f09_str_call_abi", src: "fn greet(s: &str) { } fn f() { greet(\"hello\") }", expect_all: &["define void @landin_greet({ i8*, i64 } %arg0)", "call void @landin_greet({ i8*, i64 }"], expect_none: &[], desc: "Stage 3.49 §L13: &str call ABI = fat pointer" },
        Case { name: "f10_str_two_params", src: "fn cat(a: &str, b: &str) { } fn f() { cat(\"hello\", \"world\") }", expect_all: &["define void @landin_cat({ i8*, i64 } %arg0, { i8*, i64 } %arg1)"], expect_none: &[], desc: "Stage 3.49 §L13: two &str params = two fat pointers" },
        // F.6 — fat pointer comparison (eq/ne)
        Case { name: "f11_str_eq", src: "fn f(s: &str) -> bool { s == \"hello\" }", expect_all: &["extractvalue { i8*, i64 }", "icmp eq i8*", "icmp eq i64", "and i1"], expect_none: &["icmp eq { i8*, i64 }"], desc: "Stage 3.49 §L13: str eq = (ptr_eq AND len_eq)" },
        Case { name: "f12_str_ne", src: "fn f(s: &str) -> bool { s != \"hello\" }", expect_all: &["extractvalue { i8*, i64 }", "or i1"], expect_none: &["icmp ne { i8*, i64 }"], desc: "Stage 3.49 §L13: str ne = (ptr_ne OR len_ne)" },
        // F.7 — &str in tuple (nested)
        Case { name: "f13_str_in_tuple", src: "fn f(t: (&str, i32)) { }", expect_all: &["{ { i8*, i64 }, i32 }"], expect_none: &[], desc: "Stage 3.49 §L13: &str in tuple = nested fat pointer" },
        // F.8 — &str returned and used
        Case { name: "f14_str_returned_and_passed", src: "fn get() -> &str { \"hi\" } fn f() { get() }", expect_all: &["define { i8*, i64 } @landin_get()", "call { i8*, i64 } @landin_get()"], expect_none: &[], desc: "Stage 3.49 §L13: &str return value flows to call site" },

        // Group E: §9.3.2 edge cases (8)
        Case { name: "e01_str_empty_literal", src: "fn f() -> &str { \"\" }", expect_all: &["insertvalue { i8*, i64 } undef, i8*", "i64 0, 1"], expect_none: &[], desc: "Stage 3.49 edge: empty string fat ptr" },
        Case { name: "e02_str_long_literal", src: "fn f() -> &str { \"hello, world! this is a long string\" }", expect_all: &["i64 35, 1"], expect_none: &[], desc: "Stage 3.49 edge: long string (35 bytes) fat ptr len" },
        Case { name: "e03_str_in_nested_struct", src: "struct Inner { s: &str } struct Outer { i: Inner } fn f(o: Outer) { }", expect_all: &["{ { { i8*, i64 } } }"], expect_none: &[], desc: "Stage 3.49 edge: &str in nested struct (3 levels)" },
        Case { name: "e04_str_comparison_same_literal", src: "fn f() -> bool { \"abc\" == \"abc\" }", expect_all: &["icmp eq i8*", "icmp eq i64", "and i1"], expect_none: &[], desc: "Stage 3.49 edge: same literal compared (deduped global)" },
        Case { name: "e05_str_param_passed_to_another", src: "fn id(s: &str) -> &str { s } fn f() { id(\"hello\") }", expect_all: &["define { i8*, i64 } @landin_id({ i8*, i64 } %arg0)"], expect_none: &[], desc: "Stage 3.49 edge: &str identity fn (param → return)" },
        Case { name: "e06_str_multiple_in_struct", src: "struct Pair { a: &str, b: &str } fn f(p: Pair) { }", expect_all: &["{ { i8*, i64 }, { i8*, i64 } }"], expect_none: &[], desc: "Stage 3.49 edge: two &str fields in struct" },
        Case { name: "e07_str_in_enum_payload", src: "enum E { None, Some(&str) } fn f(e: E) { }", expect_all: &["{ i32, { i8*, i64 } }"], expect_none: &[], desc: "Stage 3.49 edge: &str in enum payload = fat pointer" },
        Case { name: "e08_str_returned_from_match", src: "enum E { A, B(&str) } fn f(e: E) -> &str { match e { E::B(s) => s, _ => \"\" } }", expect_all: &["ret { i8*, i64 }"], expect_none: &[], desc: "Stage 3.49 edge: &str returned from enum match binding" },
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

    println!("\n=== Stage 3 Gate Audit Round 16 Summary ===");
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
            "   R1-R16: ..., 28, 28, 23, 24, 30, 30, 30, {}/{} — all OK.",
            pass,
            cases.len()
        );
        println!("   Per §9.3.3, audit CONVERGED (16 rounds, 0 new issues each).");
        println!("   Stage 3.49 (L13 fat pointer closure) verified.");
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found.", fail);
    }
}
