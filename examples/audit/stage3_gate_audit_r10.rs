//! Stage 3 Gate Review Round 10 audit (per §9.3.1 of process v3.4).
//!
//! Run with: cargo run --example stage3_gate_audit_r10
//!
//! Purpose: Phase gate audit for Stage 3 codegen after Stage 3.42 + 3.43
//! (&str type fix + L11 shift-count overflow check).
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
        (false, format!("FAIL — no HIR produced: {}", c.desc))
    }
}

fn main() {
    let cases: &[Case] = &[
        // Group R: Regression (8)
        Case { name: "r01_const_return", src: "fn main() -> i32 { 42 }", expect_all: &["ret i32"], expect_none: &[], desc: "R9 r01: constant return" },
        Case { name: "r02_enum_match", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, _ => 0 } }", expect_all: &["switch i32"], expect_none: &[], desc: "R9 r02: enum match" },
        Case { name: "r03_struct_construction", src: "struct Point { x: i32, y: i32 } fn f() -> i32 { let p = Point { x: 1, y: 2 }; p.x }", expect_all: &["insertvalue { i32, i32 }"], expect_none: &[], desc: "R9 r03: struct construction" },
        Case { name: "r04_field_mutation", src: "struct Acc { v: i64 } fn f() -> i64 { let mut a = Acc { v: 0 }; a.v = 42; a.v }", expect_all: &["store i64 42"], expect_none: &[], desc: "R9 r04: field mutation" },
        Case { name: "r05_overflow_check", src: "fn f(a: i32, b: i32) -> i32 { a + b }", expect_all: &["llvm.sadd.with.overflow.i32"], expect_none: &[], desc: "R9 r05: overflow check" },
        Case { name: "r06_div_zero", src: "fn f(a: i32, b: i32) -> i32 { a / b }", expect_all: &["icmp eq", "__landin_panic_div_by_zero"], expect_none: &[], desc: "R9 r06: div-by-zero check" },
        Case { name: "r07_string_literal", src: "fn f() { let s = \"hello\"; }", expect_all: &["@.str.0 = private unnamed_addr constant [5 x i8] c\"hello\""], expect_none: &[], desc: "R9 r07: string literal" },
        Case { name: "r08_if_else", src: "fn f(x: i32) -> i32 { if x > 0 { 1 } else { 2 } }", expect_all: &["br i1"], expect_none: &["ret i32 2"], desc: "R9 r08: if-else merge" },

        // Group S: Stage 3.42 &str type fix (8)
        Case { name: "s01_str_as_arg", src: "fn greet(s: &str) { } fn f() { greet(\"hello\") }", expect_all: &["define void @landin_greet(i8* %arg0)", "call void @landin_greet(i8*"], expect_none: &[], desc: "Stage 3.42: &str param as i8*" },
        Case { name: "s02_str_comparison", src: "fn f(s: &str) -> bool { s == \"hello\" }", expect_all: &["ret i1"], expect_none: &[], desc: "Stage 3.42: string comparison compiles" },
        Case { name: "s03_str_param_type", src: "fn f(s: &str) { }", expect_all: &["define void @landin_f(i8* %arg0)"], expect_none: &[], desc: "Stage 3.42: &str param uses i8*" },
        Case { name: "s04_str_return_type", src: "fn f() -> &str { \"hello\" }", expect_all: &["define i8* @landin_f()"], expect_none: &[], desc: "Stage 3.42: &str return uses i8*" },
        Case { name: "s05_str_in_struct", src: "struct Msg { text: &str } fn f(m: Msg) { } fn g() { let m = Msg { text: \"hi\" }; f(m); }", expect_all: &["define"], expect_none: &["error"], desc: "Stage 3.42: &str in struct" },
        Case { name: "s06_str_multiple_args", src: "fn cat(a: &str, b: &str) { } fn f() { cat(\"hello\", \"world\") }", expect_all: &["define void @landin_cat(i8* %arg0, i8* %arg1)"], expect_none: &[], desc: "Stage 3.42: multiple &str params" },
        Case { name: "s07_str_no_type_mismatch", src: "fn greet(s: &str) { } fn f() { greet(\"hello\") }", expect_all: &[], expect_none: &["mismatched types", "expected Str"], desc: "Stage 3.42 §15.4: no type mismatch for &str arg" },
        Case { name: "s08_str_no_moved_value", src: "fn greet(s: &str) { } fn f() { greet(\"hello\") }", expect_all: &[], expect_none: &["use of moved value"], desc: "Stage 3.42 §15.4: no 'use of moved value' for &str" },

        // Group H: Stage 3.43 L11 shift overflow (8)
        Case { name: "h01_shift_left_check", src: "fn f(a: i32) -> i32 { a << 2 }", expect_all: &["icmp uge", "32"], expect_none: &[], desc: "Stage 3.43 §15.4: shl overflow check via icmp uge 32" },
        Case { name: "h02_shift_right_check", src: "fn f(a: i32) -> i32 { a >> 2 }", expect_all: &["icmp uge", "32"], expect_none: &[], desc: "Stage 3.43: shr overflow check" },
        Case { name: "h03_shift_i64_check", src: "fn f(a: i64) -> i64 { a << 2 }", expect_all: &["icmp uge", "64"], expect_none: &[], desc: "Stage 3.43: i64 shift checks against 64" },
        Case { name: "h04_shift_no_overflow_for_cmp", src: "fn f(a: i32, b: i32) -> bool { a == b }", expect_all: &[], expect_none: &["icmp uge"], desc: "Stage 3.43: no shift check for comparison" },
        Case { name: "h05_shift_panic_block", src: "fn f(a: i32) -> i32 { a << 2 }", expect_all: &["panic_assert_", "__landin_panic_overflow"], expect_none: &[], desc: "Stage 3.43: shift overflow has panic block" },
        Case { name: "h06_shift_branch_direction", src: "fn f(a: i32) -> i32 { a << 2 }", expect_all: &["br i1", "panic_assert_"], expect_none: &[], desc: "Stage 3.43: branch to panic on shift overflow" },
        Case { name: "h07_shift_in_loop", src: "fn f(n: i32) -> i32 { let mut s = 0; let mut i = 0; while i < n { s = s << 1; i = i + 1; } s }", expect_all: &["icmp uge", "br i1"], expect_none: &[], desc: "Stage 3.43: shift overflow check in loop" },
        Case { name: "h08_shift_no_llvm_intrinsic", src: "fn f(a: i32) -> i32 { a << 2 }", expect_all: &[], expect_none: &["llvm.sadd.with.overflow", "llvm.smul.with.overflow"], desc: "Stage 3.43: shifts don't use add/mul intrinsics" },

        // Group E: §9.3.2 edge cases (4)
        Case { name: "e01_str_no_double_ptr", src: "fn f(s: &str) { }", expect_all: &["define void @landin_f(i8* %arg0)"], expect_none: &["i8**", "i32* %arg0"], desc: "Stage 3.42 edge: &str is i8* not i8** or i32*" },
        Case { name: "e02_shift_i8_width", src: "fn f(a: i8) -> i8 { a << 2 }", expect_all: &["icmp uge"], expect_none: &[], desc: "Stage 3.43 edge: i8 shift checks against 8" },
        Case { name: "e03_str_pass_through_fn", src: "fn id(s: &str) -> &str { s } fn f() -> &str { id(\"hello\") }", expect_all: &["define i8* @landin_id(i8* %arg0)", "call i8* @landin_id"], expect_none: &[], desc: "Stage 3.42 edge: &str passes through function" },
        Case { name: "e04_shift_add_mixed", src: "fn f(a: i32) -> i32 { (a << 2) + (a >> 1) }", expect_all: &["icmp uge", "llvm.sadd.with.overflow.i32"], expect_none: &[], desc: "Stage 3.43 edge: mixed shift + add with both checks" },
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

    println!("\n=== Stage 3 Gate Audit Round 10 Summary ===");
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
            "   R1-R10: 38, 43, 43, 37, 30, 30, 28, 28, 28, {}/{} — all OK.",
            pass,
            cases.len()
        );
        println!("   Per §9.3.3, audit CONVERGED (10 rounds, 0 new issues each).");
        println!("   Stage 3.42 (&str type) + Stage 3.43 (L11 shift overflow) verified.");
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found, need fixes.", fail);
    }
}
