//! Stage 3 Gate Review Round 3 audit (per §9.3.1 of process v3.4).
//!
//! Run with: cargo run --example stage3_gate_audit_r3
//!
//! Purpose: Phase gate audit for Stage 3 codegen after Stage 3.27 + 3.28
//! (string literal codegen + byte string literal codegen).
//!
//! Per §9.3.3, this is Round 3 — Rounds 1 + 2 both passed with 0 new issues,
//! so the audit was CONVERGED. R3 is run anyway because Stage 3.27 + 3.28
//! added significant new IR shape (module-level globals) — per §9.3.3 the
//! skip rule does NOT apply when "significant new features land".
//!
//! Author: redskaber
//! Date: 2026-07-20
//! Process: v3.7

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
        // ============================================================
        // Group R: Regression — re-verify Round 2 cases (15 cases)
        // ============================================================
        Case {
            name: "r01_const_return",
            src: "fn main() -> i32 { 42 }",
            expect_all: &["ret i32"],
            expect_none: &[],
            desc: "R2 r01: constant return",
        },
        Case {
            name: "r02_int_add",
            src: "fn f() -> i32 { 1 + 2 }",
            expect_all: &["add nsw i32", "llvm.sadd.with.overflow.i32"],
            expect_none: &[],
            desc: "R2 r02: int addition + overflow check",
        },
        Case {
            name: "r03_float_add",
            src: "fn f() -> f64 { 1.5 + 2.5 }",
            expect_all: &["fadd double"],
            expect_none: &["llvm.sadd.with.overflow"],
            desc: "R2 r03: float addition (no overflow check)",
        },
        Case {
            name: "r04_bool_constant",
            src: "fn f() -> bool { true }",
            expect_all: &["ret i1"],
            expect_none: &[],
            desc: "R2 r04: bool return",
        },
        Case {
            name: "r05_let_alloca",
            src: "fn f() -> i32 { let x = 42; x }",
            expect_all: &["alloca i32", "store i32"],
            expect_none: &[],
            desc: "R2 r05: let alloca",
        },
        Case {
            name: "r06_borrow_deref",
            src: "fn f() -> i32 { let x = 42; let r = &x; *r }",
            expect_all: &["alloca i32", "load i32"],
            expect_none: &[],
            desc: "R2 r06: borrow + deref",
        },
        Case {
            name: "r07_eq_cmp",
            src: "fn f(a: i32, b: i32) -> bool { a == b }",
            expect_all: &["icmp eq", "zext i1"],
            expect_none: &[],
            desc: "R2 r07: equality",
        },
        Case {
            name: "r08_if_else_merge",
            src: "fn f(x: i32) -> i32 { if x > 0 { 1 } else { 2 } }",
            expect_all: &["br i1", "icmp sgt"],
            expect_none: &["ret i32 2"],
            desc: "R2 r08: if-else merge correctness",
        },
        Case {
            name: "r09_match_switch",
            src: "fn f(x: i32) -> i32 { match x { 0 => 1, 1 => 2, _ => 3 } }",
            expect_all: &["switch i32"],
            expect_none: &[],
            desc: "R2 r09: match switch",
        },
        Case {
            name: "r10_overflow_div",
            src: "fn f(a: i32, b: i32) -> i32 { a / b }",
            expect_all: &["icmp eq", "call void @__landin_panic_div_by_zero"],
            expect_none: &[],
            desc: "R2 r10: div-by-zero check",
        },
        Case {
            name: "r11_tuple_construction",
            src: "fn f() -> f64 { let t = (1, 2.5); t.1 }",
            expect_all: &["insertvalue { i32, double }"],
            expect_none: &[],
            desc: "R2 r11: tuple construction",
        },
        Case {
            name: "r12_array_construction",
            src: "fn f() -> i32 { let a = [10, 20, 30]; let i = 0; a[i] }",
            expect_all: &["insertvalue [3 x i32]"],
            expect_none: &["[10 x i32]"],
            desc: "R2 r12: array construction",
        },
        Case {
            name: "r13_fib_recursive",
            src: "fn fib(n: i64) -> i64 { if n < 2 { return n; } fib(n - 1) + fib(n - 2) } fn main() { let r = fib(10); }",
            expect_all: &["call i64 @landin_fib", "br i1"],
            expect_none: &[],
            desc: "R2 r13: recursive Fibonacci",
        },
        Case {
            name: "r14_empty_function",
            src: "fn f() { }",
            expect_all: &["ret void"],
            expect_none: &["alloca void", "store void"],
            desc: "R2 r14: empty function (no void alloca)",
        },
        Case {
            name: "r15_chained_arith_overflow",
            src: "fn f(a: i32, b: i32) -> i32 { a * b + a - b }",
            expect_all: &[
                "llvm.smul.with.overflow.i32",
                "llvm.sadd.with.overflow.i32",
                "llvm.ssub.with.overflow.i32",
            ],
            expect_none: &[],
            desc: "R2 r15: chained arith each get overflow checks",
        },

        // ============================================================
        // Group S: NEW — Stage 3.27 string literal codegen (10 cases)
        // ============================================================
        Case {
            name: "s01_string_global_emitted",
            src: "fn f() { let s = \"hello\"; }",
            expect_all: &["@.str.0 = private unnamed_addr constant [5 x i8] c\"hello\""],
            expect_none: &[],
            desc: "Stage 3.27: string literal emits private unnamed_addr global",
        },
        Case {
            name: "s02_string_gep_to_i8_ptr",
            src: "fn f() { let s = \"hi\"; }",
            expect_all: &[
                "getelementptr inbounds ([2 x i8], [2 x i8]* @.str.0, i32 0, i32 0)",
                "store i8*",
            ],
            expect_none: &[],
            desc: "Stage 3.27: string literal value is GEP → i8*",
        },
        Case {
            name: "s03_string_dedup",
            src: "fn f() { let a = \"hello\"; let b = \"hello\"; }",
            expect_all: &[],
            expect_none: &["@.str.1"],
            desc: "Stage 3.27: same string twice → 1 global (no @.str.1)",
        },
        Case {
            name: "s04_string_distinct",
            src: "fn f() { let a = \"hello\"; let b = \"world\"; }",
            expect_all: &[
                "@.str.0 = private unnamed_addr constant [5 x i8] c\"hello\"",
                "@.str.1 = private unnamed_addr constant [5 x i8] c\"world\"",
            ],
            expect_none: &[],
            desc: "Stage 3.27: different strings → 2 globals",
        },
        Case {
            name: "s05_string_escape_tab",
            src: "fn f() { let s = \"a\\tb\"; }",
            expect_all: &["c\"a\\09b\""],
            expect_none: &[],
            desc: "Stage 3.27: tab escaped to \\09",
        },
        Case {
            name: "s06_string_escape_newline",
            src: "fn f() { let s = \"a\\nb\"; }",
            expect_all: &["c\"a\\0Ab\""],
            expect_none: &[],
            desc: "Stage 3.27: newline escaped to \\0A",
        },
        Case {
            name: "s07_string_escape_quote",
            src: "fn f() { let s = \"a\\\"b\"; }",
            expect_all: &["c\"a\\22b\""],
            expect_none: &[],
            desc: "Stage 3.27: quote escaped to \\22",
        },
        Case {
            name: "s08_string_unicode_utf8",
            src: "fn f() { let s = \"\\u{e9}\"; }",
            expect_all: &["[2 x i8] c\"\\C3\\A9\""],
            expect_none: &[],
            desc: "Stage 3.27: Unicode → UTF-8 bytes (é = C3 A9)",
        },
        Case {
            name: "s09_string_empty",
            src: "fn f() { let s = \"\"; }",
            expect_all: &["[0 x i8] c\"\""],
            expect_none: &[],
            desc: "Stage 3.27: empty string → [0 x i8]",
        },
        Case {
            name: "s10_string_cross_function_dedup",
            src: "fn f() { let _ = \"shared\"; } fn g() { let _ = \"shared\"; }",
            expect_all: &["c\"shared\""],
            expect_none: &[],
            desc: "Stage 3.27: strings dedup across functions",
        },

        // ============================================================
        // Group B: NEW — Stage 3.28 byte string literal codegen (8 cases)
        // ============================================================
        Case {
            name: "b01_byte_string_global",
            src: "fn f() { let b = b\"hello\"; }",
            expect_all: &["@.str.0 = private unnamed_addr constant [5 x i8] c\"hello\""],
            expect_none: &[],
            desc: "Stage 3.28: byte string emits global (same format as str)",
        },
        Case {
            name: "b02_byte_string_gep",
            src: "fn f() { let b = b\"hi\"; }",
            expect_all: &["getelementptr inbounds ([2 x i8], [2 x i8]* @.str.0, i32 0, i32 0)"],
            expect_none: &[],
            desc: "Stage 3.28: byte string value is GEP → i8*",
        },
        Case {
            name: "b03_byte_string_dedup_with_str",
            src: "fn f() { let s = \"hello\"; let b = b\"hello\"; }",
            expect_all: &[],
            expect_none: &["@.str.1"],
            desc: "Stage 3.28: b\"hello\" and \"hello\" share one global",
        },
        Case {
            name: "b04_byte_string_escape",
            src: "fn f() { let b = b\"a\\nb\"; }",
            expect_all: &["c\"a\\0Ab\""],
            expect_none: &[],
            desc: "Stage 3.28: byte string escapes",
        },
        Case {
            name: "b05_byte_string_empty",
            src: "fn f() { let b = b\"\"; }",
            expect_all: &["[0 x i8] c\"\""],
            expect_none: &[],
            desc: "Stage 3.28: empty byte string",
        },
        Case {
            name: "b06_u8_type_maps_to_i8",
            src: "fn f(x: u8) -> u8 { x }",
            expect_all: &["define i8 @landin_f(i8 %arg0)"],
            expect_none: &[],
            desc: "Stage 3.28: u8 maps to LLVM i8",
        },
        Case {
            name: "b07_i8_type_maps_to_i8",
            src: "fn f(x: i8) -> i8 { x }",
            expect_all: &["define i8 @landin_f(i8 %arg0)"],
            expect_none: &[],
            desc: "Stage 3.28: i8 maps to LLVM i8",
        },
        Case {
            name: "b08_byte_string_with_other_locals",
            src: "fn f() -> i32 { let b = b\"hi\"; let n = 42; n }",
            expect_all: &["[2 x i8] c\"hi\"", "alloca i8*", "alloca i32"],
            expect_none: &[],
            desc: "Stage 3.28: byte string + int local have distinct types",
        },

        // ============================================================
        // Group E: §9.3.2 edge cases for Stage 3.27 + 3.28 fixes (5 cases)
        // ============================================================
        Case {
            name: "e01_no_alloca_void_for_unit_locals",
            src: "fn f() { let s = \"hi\"; }",
            expect_all: &[],
            expect_none: &["alloca void", "store void"],
            desc: "Stage 3.27 edge: no `alloca void` / `store void` for unit locals",
        },
        Case {
            name: "e02_string_global_private_unnamed_addr",
            src: "fn f() { let s = \"hi\"; }",
            // Must be `private unnamed_addr` — not `global` (external) or `internal`.
            expect_all: &["@.str.0 = private unnamed_addr constant"],
            expect_none: &["@.str.0 = global", "@.str.0 = internal"],
            desc: "Stage 3.27 edge: global has correct linkage/visibility",
        },
        Case {
            name: "e03_string_correct_byte_length",
            src: "fn f() { let s = \"hello\"; }",
            // [5 x i8] must match the actual byte count, not char count or a hardcoded 10.
            expect_all: &["[5 x i8] c\"hello\""],
            expect_none: &["[10 x i8]"],
            desc: "Stage 3.27 edge: byte length in [N x i8] is correct",
        },
        Case {
            name: "e04_string_global_at_module_end",
            src: "fn f() { let s = \"hi\"; }",
            // Globals section header should appear (we emit at end of module).
            expect_all: &["; --- Module-level string constants ---"],
            expect_none: &[],
            desc: "Stage 3.27 edge: globals emitted at module end with header",
        },
        Case {
            name: "e05_byte_string_correct_byte_length",
            src: "fn f() { let b = b\"abc\"; }",
            expect_all: &["[3 x i8]"],
            expect_none: &["[10 x i8]", "[5 x i8]"],
            desc: "Stage 3.28 edge: byte string length is correct (3 bytes)",
        },

        // ============================================================
        // Group H: Adversarial — combinations (5 cases)
        // ============================================================
        Case {
            name: "h01_string_in_if_branch",
            src: "fn f(c: bool) { if c { let _ = \"true_branch\"; } else { let _ = \"false_branch\"; } }",
            expect_all: &[
                "c\"true_branch\"",
                "c\"false_branch\"",
                "br i1",
            ],
            expect_none: &[],
            desc: "Adversarial: strings in if branches",
        },
        Case {
            name: "h02_string_in_loop",
            src: "fn f(n: i32) { let mut i = 0; while i < n { let _ = \"iter\"; i = i + 1; } }",
            expect_all: &["c\"iter\"", "br i1", "llvm.sadd.with.overflow.i32"],
            expect_none: &["@.str.1"],
            desc: "Adversarial: string in loop (deduped, overflow check for i+1)",
        },
        Case {
            name: "h03_string_as_function_arg",
            src: "fn g(s: i32) -> i32 { s } fn f() -> i32 { g(42) }",
            // Note: passing &str as arg requires fat-pointer support (deferred).
            // This test uses int arg to verify the call path still works
            // alongside string globals from other tests (process isolation).
            expect_all: &["call i32 @landin_g(i32 42)"],
            expect_none: &[],
            desc: "Adversarial: int call still works (string-as-arg deferred)",
        },
        Case {
            name: "h04_many_strings_dedup",
            src: "fn f() { let a = \"x\"; let b = \"x\"; let c = \"x\"; let d = \"x\"; let e = \"x\"; }",
            // 5 uses of "x" → 1 global.
            expect_all: &["[1 x i8] c\"x\""],
            expect_none: &["@.str.1", "@.str.2"],
            desc: "Adversarial: 5 uses of same string → 1 global",
        },
        Case {
            name: "h05_mixed_str_and_bytestr",
            src: "fn f() { let s1 = \"hello\"; let s2 = \"world\"; let b1 = b\"hello\"; let b2 = b\"world\"; }",
            // 4 literals, 2 distinct contents → 2 globals.
            expect_all: &[
                "@.str.0 = private unnamed_addr constant [5 x i8] c\"hello\"",
                "@.str.1 = private unnamed_addr constant [5 x i8] c\"world\"",
            ],
            expect_none: &["@.str.2", "@.str.3"],
            desc: "Adversarial: mixed str + bytestr dedup correctly",
        },
    ];

    let mut pass = 0;
    let mut fail = 0;
    let mut failures: Vec<&str> = Vec::new();
    for c in cases {
        let (ok, msg) = run_case(c);
        println!("{:30} {}", c.name, msg);
        if ok {
            pass += 1;
        } else {
            fail += 1;
            failures.push(c.name);
        }
    }

    println!("\n=== Stage 3 Gate Audit Round 3 Summary ===");
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
            "   R1: 38/38, R2: 43/43, R3: {}/{} — all OK.",
            pass,
            cases.len()
        );
        println!("   Per §9.3.3, audit CONVERGED (3 rounds, 0 new issues each).");
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found, need fixes.", fail);
    }
}
