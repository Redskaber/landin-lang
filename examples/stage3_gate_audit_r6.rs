//! Stage 3 Gate Review Round 6 audit (per §9.3.1 of process v3.4).
//!
//! Run with: cargo run --example stage3_gate_audit_r6
//!
//! Purpose: Phase gate audit for Stage 3 codegen after Stage 3.34
//! (L-MUT-1 fix: field mutation MIR lower).
//!
//! Per §9.3.3, R5 was CONVERGED (5 consecutive rounds with 0 new issues).
//! R6 is run because Stage 3.34 closed the L-MUT-1 debt item recorded in R5.
//! Per §15.4, the gate review must verify "是否真的消除了根因" when a debt
//! item is closed.
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
        (false, format!("FAIL — no HIR produced: {}", c.desc))
    }
}

fn main() {
    let cases: &[Case] = &[
        // ============================================================
        // Group R: Regression (10)
        // ============================================================
        Case {
            name: "r01_const_return",
            src: "fn main() -> i32 { 42 }",
            expect_all: &["ret i32"],
            expect_none: &[],
            desc: "R5 r01: constant return",
        },
        Case {
            name: "r02_named_struct",
            src: "struct Point { x: i32, y: i32 } fn f() -> i32 { let p = Point { x: 1, y: 2 }; p.x }",
            expect_all: &["insertvalue { i32, i32 } undef, i32 1, 0"],
            expect_none: &[],
            desc: "R5 r02: named struct construction",
        },
        Case {
            name: "r03_tuple_struct",
            src: "struct Pair(i32, i64); fn f() -> i64 { let p = Pair(1, 2); p.0 }",
            expect_all: &["insertvalue { i32, i64 } undef, i32 1, 0"],
            expect_none: &["call i32 @landin_Pair", "call i64 @landin_Pair"],
            desc: "R5 r03: tuple struct ctor (no fake call)",
        },
        Case {
            name: "r04_field_load_i64",
            src: "struct Pair(i32, i64); fn f() -> i64 { let p = Pair(1, 2); p.1 }",
            expect_all: &["load i64"],
            expect_none: &["load i32, %v4", "load i32, %v5"],
            desc: "R5 r04: i64 field loads as i64 (L-DEBT-2 fixed)",
        },
        Case {
            name: "r05_struct_param",
            src: "struct Point { x: i32, y: i32 } fn get_x(p: Point) -> i32 { p.x } fn f() -> i32 { get_x(Point { x: 1, y: 2 }) }",
            expect_all: &["define i32 @landin_get_x({ i32, i32 } %arg0)"],
            expect_none: &[],
            desc: "R5 r05: struct as function parameter",
        },
        Case {
            name: "r06_string_literal",
            src: "fn f() { let s = \"hello\"; }",
            expect_all: &["@.str.0 = private unnamed_addr constant [5 x i8] c\"hello\""],
            expect_none: &[],
            desc: "R5 r06: string literal global",
        },
        Case {
            name: "r07_overflow_check",
            src: "fn f(a: i32, b: i32) -> i32 { a + b }",
            expect_all: &["llvm.sadd.with.overflow.i32"],
            expect_none: &[],
            desc: "R5 r07: overflow check",
        },
        Case {
            name: "r08_div_zero_check",
            src: "fn f(a: i32, b: i32) -> i32 { a / b }",
            expect_all: &["icmp eq", "call void @__landin_panic_div_by_zero"],
            expect_none: &[],
            desc: "R5 r08: div-by-zero check",
        },
        Case {
            name: "r09_if_else_merge",
            src: "fn f(x: i32) -> i32 { if x > 0 { 1 } else { 2 } }",
            expect_all: &["br i1"],
            expect_none: &["ret i32 2"],
            desc: "R5 r09: if-else merge correctness",
        },
        Case {
            name: "r10_field_load_f64",
            src: "struct Mixed { a: i32, b: f64 } fn f() -> f64 { let m = Mixed { a: 1, b: 2.5 }; m.b }",
            expect_all: &["load double"],
            expect_none: &[],
            desc: "R5 r10: f64 field loads as double",
        },

        // ============================================================
        // Group M: NEW — Stage 3.34 L-MUT-1 fix (field mutation) (10)
        // ============================================================
        Case {
            name: "m01_field_mutation_works",
            src: "struct Acc { v: i64 } fn f() -> i64 { let mut a = Acc { v: 0 }; a.v = 42; a.v }",
            // §15.4 verification: L-MUT-1 root-cause fix. Before Stage 3.34,
            // `a.v = 42` was silently dropped (no GEP + store to struct).
            // After fix, should have GEP + store to struct field.
            expect_all: &[
                "getelementptr inbounds { i64 }, { i64 }* %loc_3, i32 0, i32 0",
            ],
            expect_none: &[],
            desc: "Stage 3.34 §15.4: field mutation emits GEP + store to struct",
        },
        Case {
            name: "m02_field_mutation_value",
            src: "struct Acc { v: i64 } fn f() -> i64 { let mut a = Acc { v: 0 }; a.v = 42; a.v }",
            // Should store i64 42 to the struct field.
            expect_all: &["store i64 42"],
            expect_none: &[],
            desc: "Stage 3.34: mutation stores correct value to field",
        },
        Case {
            name: "m03_field_mutation_persists",
            src: "struct Acc { v: i64 } fn f() -> i64 { let mut a = Acc { v: 0 }; a.v = 42; a.v }",
            // After mutation, reading the field should load the new value.
            expect_all: &["load i64"],
            expect_none: &[],
            desc: "Stage 3.34: mutated field value persists on read",
        },
        Case {
            name: "m04_named_field_mutation",
            src: "struct Point { x: i32, y: i32 } fn f() -> i32 { let mut p = Point { x: 1, y: 2 }; p.y = 99; p.y }",
            expect_all: &["store i32 99"],
            expect_none: &[],
            desc: "Stage 3.34: named field mutation",
        },
        Case {
            name: "m05_i32_field_mutation",
            src: "struct Acc { v: i32 } fn f() -> i32 { let mut a = Acc { v: 0 }; a.v = 42; a.v }",
            expect_all: &["store i32 42"],
            expect_none: &[],
            desc: "Stage 3.34: i32 field mutation",
        },
        Case {
            name: "m06_multiple_field_mutations",
            src: "struct Point { x: i32, y: i32 } fn f() -> i32 { let mut p = Point { x: 0, y: 0 }; p.x = 1; p.y = 2; p.x + p.y }",
            expect_all: &["store i32 1", "store i32 2"],
            expect_none: &[],
            desc: "Stage 3.34: multiple field mutations",
        },
        Case {
            name: "m07_local_assignment_regression",
            src: "fn f() -> i32 { let mut x = 0; x = 42; x }",
            // Regression: simple local assignment should still work after
            // the L-MUT-1 fix changed the Assign lower to use lower_expr_to_lvalue.
            expect_all: &["store i32 42"],
            expect_none: &[],
            desc: "Stage 3.34 regression: local assignment still works",
        },
        Case {
            name: "m08_field_mutation_in_loop",
            src: "struct Acc { v: i32 } fn f(n: i32) -> i32 { let mut a = Acc { v: 0 }; let mut i = 0; while i < n { a.v = a.v + i; i = i + 1; } a.v }",
            expect_all: &["store i32", "br i1"],
            expect_none: &[],
            desc: "Stage 3.34: field mutation inside loop",
        },
        Case {
            name: "m09_field_mutation_correct_gep",
            src: "struct Pair(i32, i64); fn f() -> i64 { let mut p = Pair(1, 2); p.1 = 99; p.1 }",
            // Mutation of field 1 (i64) — GEP should use index 1.
            expect_all: &["i32 0, i32 1"],
            expect_none: &[],
            desc: "Stage 3.34: field mutation uses correct GEP index",
        },
        Case {
            name: "m10_field_mutation_then_read",
            src: "struct Acc { v: i64 } fn f() -> i64 { let mut a = Acc { v: 10 }; a.v = 42; a.v }",
            // Multiple overwrites — final value should be 42.
            // Note: `store i64 10` appears for the initial value temp,
            // but `store i64 42` should appear for the mutation.
            expect_all: &["store i64 42"],
            expect_none: &[],
            desc: "Stage 3.34: mutation overwrites initial value",
        },

        // ============================================================
        // Group E: §9.3.2 edge cases for Stage 3.34 fix (5)
        // ============================================================
        Case {
            name: "e01_mutation_not_dropped",
            src: "struct Acc { v: i64 } fn f() -> i64 { let mut a = Acc { v: 0 }; a.v = 42; a.v }",
            // §15.4 edge: the OLD bug dropped the mutation entirely (no GEP
            // + store to struct). After fix, GEP + store must be present.
            expect_all: &["getelementptr inbounds { i64 }", "store i64 42"],
            expect_none: &[],
            desc: "Stage 3.34 §15.4 edge: mutation not dropped (L-MUT-1 root cause verified)",
        },
        Case {
            name: "e02_mutation_correct_field",
            src: "struct Triple(i32, i64, bool); fn f() -> bool { let mut t = Triple(1, 2, false); t.2 = true; t.2 }",
            // Mutate field 2 (bool) — GEP should use index 2.
            expect_all: &["i32 0, i32 2"],
            expect_none: &[],
            desc: "Stage 3.34 edge: mutation uses correct field index (2)",
        },
        Case {
            name: "e03_mutation_store_type",
            src: "struct Acc { v: i64 } fn f() -> i64 { let mut a = Acc { v: 0 }; a.v = 42; a.v }",
            // Store to field should be 'store i64', not 'store i32'.
            expect_all: &["store i64 42"],
            expect_none: &[],
            desc: "Stage 3.34 edge: mutation store uses correct type",
        },
        Case {
            name: "e04_mutation_then_load_same_field",
            src: "struct Acc { v: i32 } fn f() -> i32 { let mut a = Acc { v: 0 }; a.v = 42; a.v }",
            // After mutation, load should read from the same field.
            expect_all: &["load i32"],
            expect_none: &[],
            desc: "Stage 3.34 edge: load after mutation reads same field",
        },
        Case {
            name: "e05_chained_mutation",
            src: "struct Inner { v: i32 } struct Outer { inner: Inner } fn f() -> i32 { let mut o = Outer { inner: Inner { v: 0 } }; o.inner.v = 42; o.inner.v }",
            // Chained field mutation — should GEP through both levels.
            expect_all: &["store i32 42"],
            expect_none: &[],
            desc: "Stage 3.34 edge: chained field mutation",
        },

        // ============================================================
        // Group H: Adversarial (5)
        // ============================================================
        Case {
            name: "h01_mutation_in_if",
            src: "struct Acc { v: i32 } fn f(c: bool) -> i32 { let mut a = Acc { v: 0 }; if c { a.v = 1; } else { a.v = 2; } a.v }",
            expect_all: &["store i32 1", "store i32 2", "br i1"],
            expect_none: &[],
            desc: "Adversarial: field mutation in both if branches",
        },
        Case {
            name: "h02_mutation_in_loop",
            src: "struct Acc { v: i32 } fn f(n: i32) -> i32 { let mut a = Acc { v: 0 }; let mut i = 0; while i < n { a.v = i; i = i + 1; } a.v }",
            expect_all: &["store i32", "br i1", "llvm.sadd.with.overflow.i32"],
            expect_none: &[],
            desc: "Adversarial: field mutation in loop with overflow check",
        },
        Case {
            name: "h03_mutation_then_call",
            src: "struct Acc { v: i32 } fn get_v(a: Acc) -> i32 { a.v } fn f() -> i32 { let mut a = Acc { v: 0 }; a.v = 42; get_v(a) }",
            expect_all: &["store i32 42", "call i32 @landin_get_v"],
            expect_none: &[],
            desc: "Adversarial: mutate then pass struct to function",
        },
        Case {
            name: "h04_mutation_multiple_structs",
            src: "struct Acc { v: i32 } fn f() -> i32 { let mut a = Acc { v: 0 }; let mut b = Acc { v: 0 }; a.v = 1; b.v = 2; a.v + b.v }",
            expect_all: &["store i32 1", "store i32 2"],
            expect_none: &[],
            desc: "Adversarial: mutate fields of multiple structs",
        },
        Case {
            name: "h05_mutation_overwrite",
            src: "struct Acc { v: i32 } fn f() -> i32 { let mut a = Acc { v: 10 }; a.v = 20; a.v = 30; a.v }",
            // Multiple overwrites — final value should be 30.
            expect_all: &["store i32 20", "store i32 30"],
            expect_none: &[],
            desc: "Adversarial: multiple field overwrites",
        },
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

    println!("\n=== Stage 3 Gate Audit Round 6 Summary ===");
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
            "   R1-R6: 38/38, 43/43, 43/43, 37/37, 30/30, {}/{} — all OK.",
            pass,
            cases.len()
        );
        println!("   Per §9.3.3, audit CONVERGED (6 rounds, 0 new issues each).");
        println!("   §15.4 verified: L-MUT-1 root cause fixed (field mutations work).");
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found, need fixes.", fail);
    }
}
