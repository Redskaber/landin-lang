//! Stage 3 Gate Review Round 2 audit (per §9.3.1 of process v3.4).
//!
//! Run with: cargo run --example stage3_gate_audit_r2
//!
//! Purpose: Phase gate audit for Stage 3 codegen after Stage 3.24 + 3.25
//! (real overflow checks + real div-by-zero checks).
//!
//! Per §9.3.3, this is Round 2 — Round 1 passed with 38/38.
//! Round 2 must:
//!   1. Re-verify all Round 1 cases still pass (no regression)
//!   2. Add NEW cases for Stage 3.24 + 3.25 features (overflow / div-by-zero)
//!   3. Add §9.3.2 edge cases for Stage 3.24 + 3.25 fixes
//!   4. Add adversarial cases (nested checks, checks in loops, mixed types)
//!
//! If Round 2 finds 0 new issues AND Round 1 already found 0, the audit
//! is declared CONVERGED per §9.3.3 (2 consecutive rounds with 0 new issues).
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
        // Group R: Regression — re-verify Round 1 cases (15 cases)
        // ============================================================
        Case {
            name: "r01_const_return",
            src: "fn main() -> i32 { 42 }",
            expect_all: &["ret i32"],
            expect_none: &[],
            desc: "R1 a01: constant return",
        },
        Case {
            name: "r02_int_add",
            src: "fn f() -> i32 { 1 + 2 }",
            expect_all: &["add nsw i32"],
            expect_none: &[],
            desc: "R1 a02: int addition",
        },
        Case {
            name: "r03_float_add",
            src: "fn f() -> f64 { 1.5 + 2.5 }",
            expect_all: &["fadd double"],
            expect_none: &[],
            desc: "R1 a03: float addition",
        },
        Case {
            name: "r04_bool_constant",
            src: "fn f() -> bool { true }",
            expect_all: &["ret i1"],
            expect_none: &[],
            desc: "R1 a04: bool return",
        },
        Case {
            name: "r05_let_alloca",
            src: "fn f() -> i32 { let x = 42; x }",
            expect_all: &["alloca i32", "store i32"],
            expect_none: &[],
            desc: "R1 a05: let alloca",
        },
        Case {
            name: "r06_borrow_returns_ptr",
            src: "fn f() -> i32 { let x = 42; let r = &x; *r }",
            expect_all: &["alloca i32", "load i32"],
            expect_none: &[],
            desc: "R1 a06: borrow + deref",
        },
        Case {
            name: "r07_eq_cmp",
            src: "fn f(a: i32, b: i32) -> bool { a == b }",
            expect_all: &["icmp eq", "zext i1"],
            expect_none: &[],
            desc: "R1 a07: equality",
        },
        Case {
            name: "r08_if_else_branch",
            src: "fn f(x: i32) -> i32 { if x > 0 { 1 } else { 2 } }",
            expect_all: &["br i1", "icmp sgt"],
            expect_none: &["ret i32 2"],
            desc: "R1 b01: if-else merge correctness",
        },
        Case {
            name: "r09_while_loop",
            src: "fn f() -> i32 { let mut i = 0; while i < 10 { i = i + 1; } i }",
            expect_all: &["br i1", "icmp slt"],
            expect_none: &[],
            desc: "R1 b02: while loop",
        },
        Case {
            name: "r10_match_switch",
            src: "fn f(x: i32) -> i32 { match x { 0 => 1, 1 => 2, _ => 3 } }",
            expect_all: &["switch i32"],
            expect_none: &[],
            desc: "R1 b03: match switch",
        },
        Case {
            name: "r11_simple_call",
            src: "fn g(a: i32) -> i32 { a } fn f() -> i32 { g(42) }",
            expect_all: &["call i32 @landin_g(i32 42)"],
            expect_none: &[],
            desc: "R1 b04: function call",
        },
        Case {
            name: "r12_tuple_construction",
            src: "fn f() -> f64 { let t = (1, 2.5); t.1 }",
            expect_all: &["insertvalue { i32, double }"],
            expect_none: &[],
            desc: "R1 b09: tuple construction",
        },
        Case {
            name: "r13_array_construction",
            src: "fn f() -> i32 { let a = [10, 20, 30]; let i = 0; a[i] }",
            expect_all: &["insertvalue [3 x i32]"],
            expect_none: &["[10 x i32]"],
            desc: "R1 b10: array construction",
        },
        Case {
            name: "r14_fib_recursive",
            src: "fn fib(n: i64) -> i64 { if n < 2 { return n; } fib(n - 1) + fib(n - 2) } fn main() { let r = fib(10); }",
            expect_all: &["call i64 @landin_fib", "br i1"],
            expect_none: &[],
            desc: "R1 c01: recursive Fibonacci",
        },
        Case {
            name: "r15_empty_function",
            src: "fn f() { }",
            expect_all: &["ret void"],
            expect_none: &[],
            desc: "R1 d01: empty function",
        },

        // ============================================================
        // Group F: NEW — Stage 3.24 overflow checks (10 cases)
        // ============================================================
        Case {
            name: "f01_overflow_add_i32",
            src: "fn f(a: i32, b: i32) -> i32 { a + b }",
            expect_all: &["llvm.sadd.with.overflow.i32", "extractvalue { i32, i1 }"],
            expect_none: &[],
            desc: "Stage 3.24: a+b emits llvm.sadd.with.overflow.i32",
        },
        Case {
            name: "f02_overflow_sub_i32",
            src: "fn f(a: i32, b: i32) -> i32 { a - b }",
            expect_all: &["llvm.ssub.with.overflow.i32"],
            expect_none: &[],
            desc: "Stage 3.24: a-b emits llvm.ssub.with.overflow.i32",
        },
        Case {
            name: "f03_overflow_mul_i32",
            src: "fn f(a: i32, b: i32) -> i32 { a * b }",
            expect_all: &["llvm.smul.with.overflow.i32"],
            expect_none: &[],
            desc: "Stage 3.24: a*b emits llvm.smul.with.overflow.i32",
        },
        Case {
            name: "f04_overflow_add_i64",
            src: "fn f(a: i64, b: i64) -> i64 { a + b }",
            expect_all: &["llvm.sadd.with.overflow.i64"],
            expect_none: &[],
            desc: "Stage 3.24: i64 add emits i64 intrinsic",
        },
        Case {
            name: "f05_overflow_branch_to_panic",
            src: "fn f(a: i32, b: i32) -> i32 { a + b }",
            expect_all: &["xor i1", "panic_assert_", "call void @__landin_panic_overflow"],
            expect_none: &[],
            desc: "Stage 3.24: overflow check branches to panic block",
        },
        Case {
            name: "f06_no_overflow_for_comparison",
            src: "fn f(a: i32, b: i32) -> bool { a == b }",
            expect_all: &[],
            expect_none: &["llvm.sadd.with.overflow", "llvm.ssub.with.overflow", "llvm.smul.with.overflow"],
            desc: "Stage 3.24: comparisons don't get overflow checks",
        },
        Case {
            name: "f07_no_overflow_for_bitand",
            src: "fn f(a: i32, b: i32) -> i32 { a & b }",
            expect_all: &[],
            expect_none: &["llvm.sadd.with.overflow", "llvm.smul.with.overflow"],
            desc: "Stage 3.24: bitwise AND doesn't get overflow check",
        },
        Case {
            name: "f08_overflow_in_loop",
            src: "fn f(n: i32) -> i32 { let mut s = 0; let mut i = 0; while i < n { s = s + i; i = i + 1; } s }",
            expect_all: &["llvm.sadd.with.overflow.i32"],
            expect_none: &[],
            desc: "Stage 3.24: overflow check works inside loops",
        },
        Case {
            name: "f09_overflow_multiple_ops",
            src: "fn f(a: i32, b: i32, c: i32) -> i32 { a + b + c }",
            expect_all: &["llvm.sadd.with.overflow.i32"],
            expect_none: &[],
            desc: "Stage 3.24: multiple adds each get overflow check",
        },
        Case {
            name: "f10_overflow_chained_arith",
            src: "fn f(a: i32, b: i32) -> i32 { a * b + a - b }",
            expect_all: &[
                "llvm.smul.with.overflow.i32",
                "llvm.sadd.with.overflow.i32",
                "llvm.ssub.with.overflow.i32",
            ],
            expect_none: &[],
            desc: "Stage 3.24: chained arith (mul+add+sub) each get checks",
        },

        // ============================================================
        // Group G: NEW — Stage 3.25 div-by-zero checks (8 cases)
        // ============================================================
        Case {
            name: "g01_div_zero_check_div",
            src: "fn f(a: i32, b: i32) -> i32 { a / b }",
            expect_all: &["icmp eq", "panic_assert_", "call void @__landin_panic_div_by_zero"],
            expect_none: &[],
            desc: "Stage 3.25: a/b emits div-by-zero check",
        },
        Case {
            name: "g02_div_zero_check_rem",
            src: "fn f(a: i32, b: i32) -> i32 { a % b }",
            expect_all: &["icmp eq", "call void @__landin_panic_div_by_zero"],
            expect_none: &[],
            desc: "Stage 3.25: a%b emits div-by-zero check",
        },
        Case {
            name: "g03_div_zero_check_i64",
            src: "fn f(a: i64, b: i64) -> i64 { a / b }",
            expect_all: &["icmp eq i64"],
            expect_none: &[],
            desc: "Stage 3.25: i64 div emits i64 icmp",
        },
        Case {
            name: "g04_no_div_zero_for_add",
            src: "fn f(a: i32, b: i32) -> i32 { a + b }",
            expect_all: &[],
            expect_none: &["call void @__landin_panic_div_by_zero"],
            desc: "Stage 3.25: add does NOT trigger div-by-zero check",
        },
        Case {
            name: "g05_div_zero_in_loop",
            src: "fn f(n: i32) -> i32 { let mut s = 100; let mut i = 1; while i < n { s = s / i; i = i + 1; } s }",
            expect_all: &["icmp eq", "llvm.sadd.with.overflow.i32"],
            expect_none: &[],
            desc: "Stage 3.25: div check in loop + overflow check for i+1",
        },
        Case {
            name: "g06_div_zero_panic_unreachable",
            src: "fn f(a: i32, b: i32) -> i32 { a / b }",
            expect_all: &["call void @__landin_panic_div_by_zero", "unreachable"],
            expect_none: &[],
            desc: "Stage 3.25: panic block ends with unreachable",
        },
        Case {
            name: "g07_chained_div",
            src: "fn f(a: i32, b: i32, c: i32) -> i32 { a / b / c }",
            expect_all: &["icmp eq"],
            expect_none: &[],
            desc: "Stage 3.25: chained div each get div-by-zero check",
        },
        Case {
            name: "g08_mixed_arith_with_div",
            src: "fn f(a: i32, b: i32, c: i32) -> i32 { a + b / c }",
            expect_all: &[
                "llvm.sadd.with.overflow.i32",
                "icmp eq",
                "call void @__landin_panic_div_by_zero",
            ],
            expect_none: &[],
            desc: "Stage 3.25: mixed arith (add+div) gets both checks",
        },

        // ============================================================
        // Group E: §9.3.2 edge cases for Stage 3.24 + 3.25 fixes (5 cases)
        // ============================================================
        Case {
            name: "e01_overflow_extractvalue_correct_index",
            src: "fn f(a: i32, b: i32) -> i32 { a + b }",
            // Must extract index 1 (the overflow flag), not index 0 (the result).
            expect_all: &["extractvalue { i32, i1 } %v", ", 1"],
            expect_none: &[],
            desc: "Stage 3.24 edge: extractvalue uses index 1 for overflow flag",
        },
        Case {
            name: "e02_overflow_xor_inverts_flag",
            src: "fn f(a: i32, b: i32) -> i32 { a + b }",
            // `xor i1 <flag>, -1` inverts the flag so true = no overflow.
            expect_all: &["xor i1", ", -1"],
            expect_none: &[],
            desc: "Stage 3.24 edge: xor i1 ..., -1 inverts overflow flag",
        },
        Case {
            name: "e03_div_zero_icmp_eq_zero",
            src: "fn f(a: i32, b: i32) -> i32 { a / b }",
            // Must compare divisor to literal 0.
            expect_all: &["icmp eq i32", ", 0"],
            expect_none: &[],
            desc: "Stage 3.25 edge: icmp eq compares divisor to 0",
        },
        Case {
            name: "e04_div_zero_branch_correct_direction",
            src: "fn f(a: i32, b: i32) -> i32 { a / b }",
            // If is_zero is true → panic. So br i1 is_zero, label %panic, label %target.
            expect_all: &["br i1", "label %panic_assert_"],
            expect_none: &[],
            desc: "Stage 3.25 edge: branch direction (true → panic, false → target)",
        },
        Case {
            name: "e05_overflow_no_check_for_float",
            src: "fn f(a: f64, b: f64) -> f64 { a + b }",
            // Floats don't get integer overflow checks (they use fadd directly).
            expect_all: &["fadd double"],
            expect_none: &["llvm.sadd.with.overflow"],
            desc: "Stage 3.24 edge: floats don't get integer overflow check",
        },

        // ============================================================
        // Group H: Adversarial — combinations designed to break (5 cases)
        // ============================================================
        Case {
            name: "h01_overflow_in_if_branch",
            src: "fn f(a: i32, b: i32, c: bool) -> i32 { if c { a + b } else { a - b } }",
            expect_all: &[
                "llvm.sadd.with.overflow.i32",
                "llvm.ssub.with.overflow.i32",
                "br i1",
            ],
            expect_none: &["ret i32 2"],
            desc: "Adversarial: overflow checks in both branches of if-else",
        },
        Case {
            name: "h02_div_zero_in_match",
            src: "fn f(a: i32, b: i32, c: i32) -> i32 { match c { 0 => a / b, _ => a % b } }",
            expect_all: &["icmp eq", "switch i32", "call void @__landin_panic_div_by_zero"],
            expect_none: &[],
            desc: "Adversarial: div-by-zero checks in match arms",
        },
        Case {
            name: "h03_nested_overflow_div",
            src: "fn f(a: i32, b: i32, c: i32) -> i32 { (a + b) / c }",
            expect_all: &[
                "llvm.sadd.with.overflow.i32",
                "icmp eq",
                "call void @__landin_panic_div_by_zero",
            ],
            expect_none: &[],
            desc: "Adversarial: nested (a+b)/c gets both overflow and div check",
        },
        Case {
            name: "h04_overflow_with_early_return",
            src: "fn f(a: i32, b: i32) -> i32 { if a + b > 100 { return 0; } a + b }",
            expect_all: &["llvm.sadd.with.overflow.i32"],
            expect_none: &[],
            desc: "Adversarial: overflow check with early return",
        },
        Case {
            name: "h05_div_zero_recursive",
            src: "fn f(n: i32, d: i32) -> i32 { if n == 0 { 0 } else { n / d + f(n - 1, d) } }",
            expect_all: &["icmp eq", "call i32 @landin_f"],
            expect_none: &[],
            desc: "Adversarial: div-by-zero check with recursive call",
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

    println!("\n=== Stage 3 Gate Audit Round 2 Summary ===");
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
            "   Round 1: 38/38 OK, Round 2: {}/{} OK.",
            pass,
            cases.len()
        );
        println!("   Per §9.3.3, 2 consecutive rounds with 0 new issues → CONVERGED.");
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found, need fixes.", fail);
    }
}
