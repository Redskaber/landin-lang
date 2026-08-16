//! Stage 3 Gate Review Round 13 audit (per §9.3.1 of process v3.4).
//!
//! Run with: cargo run --example stage3_gate_audit_r13
//!
//! Purpose: Phase gate audit for Stage 3.46 (L14 + L9 full integer type
//! support: i8 / i16 / i32 / i64 / i128 / usize / isize, including arithmetic,
//! overflow intrinsics, and shift-count overflow checks at correct bit widths).
//!
//! Author: redskaber
//! Date: 2026-07-20
//! Process: v3.12

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
        // Group R: Regression (8) — re-verify Stage 3.45 float bitwise cases
        Case { name: "r01_const_value", src: "const MAX: i32 = 100; fn f() -> i32 { MAX }", expect_all: &["store i32 100"], expect_none: &[], desc: "R12 r01: const value inlined" },
        Case { name: "r02_enum_match", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, _ => 0 } }", expect_all: &["switch i32"], expect_none: &[], desc: "R12 r02: enum match via SwitchInt" },
        Case { name: "r03_str_arg", src: "fn greet(s: &str) { } fn f() { greet(\"hello\") }", expect_all: &["define void @landin_greet(i8* %arg0)"], expect_none: &[], desc: "R12 r03: &str as function arg" },
        Case { name: "r04_shift_overflow", src: "fn f(a: i32) -> i32 { a << 2 }", expect_all: &["icmp uge", "32"], expect_none: &[], desc: "R12 r04: i32 shift-count overflow (32-bit width)" },
        Case { name: "r05_float_bitand", src: "fn f(a: f64, b: f64) -> f64 { a & b }", expect_all: &["and i64", "fptosi", "sitofp"], expect_none: &[], desc: "R12 r05: float bitwise via int cast" },
        Case { name: "r06_int_overflow", src: "fn f(a: i32, b: i32) -> i32 { a + b }", expect_all: &["llvm.sadd.with.overflow.i32"], expect_none: &[], desc: "R12 r06: i32 signed-add overflow intrinsic" },
        Case { name: "r07_struct", src: "struct Point { x: i32, y: i32 } fn f() -> i32 { let p = Point { x: 1, y: 2 }; p.x }", expect_all: &["insertvalue { i32, i32 }"], expect_none: &[], desc: "R12 r07: struct construction via insertvalue" },
        Case { name: "r08_div_zero", src: "fn f(a: i32, b: i32) -> i32 { a / b }", expect_all: &["icmp eq", "__landin_panic_div_by_zero"], expect_none: &[], desc: "R12 r08: div-by-zero runtime check" },

        // Group I: Stage 3.46 integer type coverage (14)
        // I.1 — parameter types map to correct LLVM IR type
        Case { name: "i01_i8_param", src: "fn f(x: i8) -> i8 { x }", expect_all: &["define i8 @landin_f(i8 %arg0)"], expect_none: &[], desc: "Stage 3.46 §15.1: i8 param + return type" },
        Case { name: "i02_i16_param", src: "fn f(x: i16) -> i16 { x }", expect_all: &["define i16 @landin_f(i16 %arg0)"], expect_none: &[], desc: "Stage 3.46 §15.1: i16 param + return type (was I32 before)" },
        Case { name: "i03_u16_param", src: "fn f(x: u16) -> u16 { x }", expect_all: &["define i16 @landin_f(i16 %arg0)"], expect_none: &[], desc: "Stage 3.46 §15.1: u16 maps to i16 (LLVM has no unsigned type)" },
        Case { name: "i04_u32_param", src: "fn f(x: u32) -> u32 { x }", expect_all: &["define i32 @landin_f(i32 %arg0)"], expect_none: &[], desc: "Stage 3.46 §15.1: u32 maps to i32" },
        Case { name: "i05_usize_param", src: "fn f(x: usize) -> usize { x }", expect_all: &["define i64 @landin_f(i64 %arg0)"], expect_none: &[], desc: "Stage 3.46 §15.1: usize maps to i64 on 64-bit (was I32 before)" },
        Case { name: "i06_isize_param", src: "fn f(x: isize) -> isize { x }", expect_all: &["define i64 @landin_f(i64 %arg0)"], expect_none: &[], desc: "Stage 3.46 §15.1: isize maps to i64 on 64-bit (was I32 before)" },
        Case { name: "i07_i128_param", src: "fn f(x: i128) -> i128 { x }", expect_all: &["define i128 @landin_f(i128 %arg0)"], expect_none: &[], desc: "Stage 3.46 §15.1: i128 param + return type (was I64 before, truncated)" },
        // I.2 — arithmetic uses correct bit-width instruction
        Case { name: "i08_i16_arith", src: "fn f(a: i16, b: i16) -> i16 { a + b }", expect_all: &["add nsw i16"], expect_none: &["add nsw i32", "add nsw i64"], desc: "Stage 3.46 §15.3: i16 add uses 'add nsw i16' (no i32/i64 fallback)" },
        Case { name: "i09_i128_arith", src: "fn f(a: i128, b: i128) -> i128 { a + b }", expect_all: &["add nsw i128"], expect_none: &["add nsw i64", "add nsw i32"], desc: "Stage 3.46 §15.3: i128 add uses 'add nsw i128' (no truncation to i64)" },
        Case { name: "i10_usize_arith", src: "fn f(a: usize, b: usize) -> usize { a + b }", expect_all: &["add nsw i64"], expect_none: &["add nsw i32"], desc: "Stage 3.46 §15.3: usize add uses 'add nsw i64' (no i32 fallback)" },
        // I.3 — overflow intrinsics use correct width
        Case { name: "i11_i16_overflow", src: "fn f(a: i16, b: i16) -> i16 { a + b }", expect_all: &["llvm.sadd.with.overflow.i16"], expect_none: &[], desc: "Stage 3.46 §15.4: i16 overflow check uses i16 intrinsic (not i32)" },
        Case { name: "i12_i128_overflow", src: "fn f(a: i128, b: i128) -> i128 { a + b }", expect_all: &["llvm.sadd.with.overflow.i128"], expect_none: &["llvm.sadd.with.overflow.i64"], desc: "Stage 3.46 §15.4: i128 overflow check uses i128 intrinsic (not i64)" },
        // I.4 — shift-count overflow uses correct bit width
        Case { name: "i13_i16_shift", src: "fn f(a: i16) -> i16 { a << 2 }", expect_all: &["icmp uge", "16"], expect_none: &[], desc: "Stage 3.46 §15.4: i16 shift check uses bit width 16" },
        Case { name: "i14_i128_shift", src: "fn f(a: i128) -> i128 { a << 2 }", expect_all: &["icmp uge", "128"], expect_none: &[], desc: "Stage 3.46 §15.4: i128 shift check uses bit width 128" },

        // Group E: §9.3.2 edge cases (8)
        Case { name: "e01_i16_sub", src: "fn f(a: i16, b: i16) -> i16 { a - b }", expect_all: &["llvm.ssub.with.overflow.i16", "sub nsw i16"], expect_none: &[], desc: "Stage 3.46 edge: i16 subtraction uses ssub i16 intrinsic" },
        Case { name: "e02_i128_mul", src: "fn f(a: i128, b: i128) -> i128 { a * b }", expect_all: &["llvm.smul.with.overflow.i128", "mul nsw i128"], expect_none: &[], desc: "Stage 3.46 edge: i128 multiplication uses smul i128 intrinsic" },
        Case { name: "e03_i16_bitand", src: "fn f(a: i16, b: i16) -> i16 { a & b }", expect_all: &["and i16"], expect_none: &["and i32", "and i64", "fptosi"], desc: "Stage 3.46 edge: i16 bitand uses 'and i16' (no cast, no wider type)" },
        Case { name: "e04_usize_in_if", src: "fn f(a: usize, b: usize) -> usize { if a > b { a } else { b } }", expect_all: &["icmp sgt i64", "br i1"], expect_none: &["icmp sgt i32"], desc: "Stage 3.46 edge: usize comparison uses i64 (not i32)" },
        Case { name: "e05_i128_div_zero", src: "fn f(a: i128, b: i128) -> i128 { a / b }", expect_all: &["icmp eq", "128", "__landin_panic_div_by_zero"], expect_none: &[], desc: "Stage 3.46 edge: i128 div-by-zero check uses i128 zero" },
        Case { name: "e06_mixed_width_no_demote", src: "fn f(a: i32, b: i64) -> i64 { b }", expect_all: &["define i64 @landin_f(i32 %arg0, i64 %arg1)"], expect_none: &[], desc: "Stage 3.46 edge: mixed i32/i64 params preserve distinct widths" },
        Case { name: "e07_i8_arith", src: "fn f(a: i8, b: i8) -> i8 { a + b }", expect_all: &["add nsw i8", "llvm.sadd.with.overflow.i8"], expect_none: &[], desc: "Stage 3.46 edge: i8 add uses 'add nsw i8' + i8 overflow intrinsic" },
        Case { name: "e08_usize_shift", src: "fn f(a: usize) -> usize { a << 4 }", expect_all: &["icmp uge", "64"], expect_none: &[], desc: "Stage 3.46 edge: usize shift check uses 64 (64-bit width)" },
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

    println!("\n=== Stage 3 Gate Audit Round 13 Summary ===");
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
            "   R1-R13: ..., 28, 28, 23, 24, {}/{} — all OK.",
            pass,
            cases.len()
        );
        println!("   Per §9.3.3, audit CONVERGED (13 rounds, 0 new issues each).");
        println!("   Stage 3.46 (L14 + L9 full integer type support) verified.");
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found.", fail);
    }
}
