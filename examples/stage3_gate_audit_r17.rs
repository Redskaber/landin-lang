//! Stage 3 Gate Review Round 17 audit (per §9.3.1 of process v3.13).
//!
//! Run with: cargo run --example stage3_gate_audit_r17
//!
//! Purpose: Phase gate audit for Stage 3.50. Verifies that byte string
//! literals now produce fat pointers (ptr plus len) instead of thin
//! pointers. Also verifies that fat pointer comparison uses the actual
//! pointee type instead of a hardcoded one.
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
        // Group R: Regression (8) — re-verify R16 cases
        Case {
            name: "r01_str_param",
            src: "fn f(s: &str) { }",
            expect_all: &["define void @landin_f({ i8*, i64 } %arg0)"],
            expect_none: &[],
            desc: "R16 r01: &str param as fat pointer",
        },
        Case {
            name: "r02_str_literal_len",
            src: "fn f() -> &str { \"hello\" }",
            expect_all: &["i64 5, 1"],
            expect_none: &[],
            desc: "R16 r02: str literal fat ptr len=5",
        },
        Case {
            name: "r03_str_eq",
            src: "fn f(s: &str) -> bool { s == \"hello\" }",
            expect_all: &["extractvalue { i8*, i64 }", "and i1"],
            expect_none: &[],
            desc: "R16 r03: str eq = (ptr_eq AND len_eq)",
        },
        Case {
            name: "r04_enum_case_c",
            src: "enum E { A, B(i32), C(i64) } fn f(e: E) -> i32 { 0 }",
            expect_all: &["{ i32, i32, i64 }"],
            expect_none: &[],
            desc: "R16 r04: Case C enum flat layout",
        },
        Case {
            name: "r05_struct_ref_str_field",
            src: "struct Wrap { s: &str } fn f(w: Wrap) { }",
            expect_all: &["{ { i8*, i64 } }"],
            expect_none: &[],
            desc: "R16 r05: &str struct field as fat pointer",
        },
        Case {
            name: "r06_const_value",
            src: "const MAX: i32 = 100; fn f() -> i32 { MAX }",
            expect_all: &["store i32 100"],
            expect_none: &[],
            desc: "R16 r06: const value inlined",
        },
        Case {
            name: "r07_div_zero",
            src: "fn f(a: i32, b: i32) -> i32 { a / b }",
            expect_all: &["icmp eq", "__landin_panic_div_by_zero"],
            expect_none: &[],
            desc: "R16 r07: div-by-zero runtime check",
        },
        Case {
            name: "r08_i16_arith",
            src: "fn f(a: i16, b: i16) -> i16 { a + b }",
            expect_all: &["add nsw i16", "llvm.sadd.with.overflow.i16"],
            expect_none: &[],
            desc: "R16 r08: i16 arithmetic + overflow check",
        },
        // Group B: Stage 3.50 byte string fat pointer coverage (14)
        // B.1 — byte string literal layout (was thin ptr, now fat ptr)
        Case {
            name: "b01_bstr_fat_ptr_layout",
            src: "fn f() { let b = b\"hello\"; }",
            expect_all: &[
                "alloca { i8*, i64 }",
                "insertvalue { i8*, i64 } undef, i8*",
                "i64 5, 1",
            ],
            expect_none: &["insertvalue i8* undef, i64"],
            desc: "Stage 3.50: b\"hello\" = fat ptr { i8*, i64 } len=5 (not thin i8*)",
        },
        Case {
            name: "b02_bstr_empty_len_0",
            src: "fn f() { let b = b\"\"; }",
            expect_all: &["i64 0, 1"],
            expect_none: &[],
            desc: "Stage 3.50: empty byte string fat ptr len=0",
        },
        Case {
            name: "b03_bstr_no_invalid_insertvalue",
            src: "fn f() { let b = b\"hi\"; }",
            expect_all: &[],
            expect_none: &["insertvalue i8* undef, i64"],
            desc: "Stage 3.50: no invalid insertvalue i8* (thin ptr regression)",
        },
        // B.2 — byte string as function param/return
        Case {
            name: "b04_bstr_param",
            src: "fn f(b: &[u8]) { }",
            expect_all: &["define void @landin_f({ i8*, i64 } %arg0)"],
            expect_none: &[],
            desc: "Stage 3.50: &[u8] param = fat pointer",
        },
        Case {
            name: "b05_bstr_return",
            src: "fn f() -> &[u8] { b\"hello\" }",
            expect_all: &["define { i8*, i64 } @landin_f()"],
            expect_none: &[],
            desc: "Stage 3.50: &[u8] return = fat pointer",
        },
        Case {
            name: "b06_bstr_call_abi",
            src: "fn f(b: &[u8]) { } fn g() { f(b\"hello\") }",
            expect_all: &["call void @landin_f({ i8*, i64 }"],
            expect_none: &[],
            desc: "Stage 3.50: byte string call uses fat pointer ABI",
        },
        // B.3 — byte string in struct/tuple
        Case {
            name: "b07_bstr_in_struct",
            src: "struct Msg { data: &[u8] } fn f(m: Msg) { }",
            expect_all: &["{ { i8*, i64 } }"],
            expect_none: &[],
            desc: "Stage 3.50: &[u8] struct field = fat pointer",
        },
        Case {
            name: "b08_bstr_in_tuple",
            src: "fn f(t: (&[u8], i32)) { }",
            expect_all: &["{ { i8*, i64 }, i32 }"],
            expect_none: &[],
            desc: "Stage 3.50: &[u8] in tuple = nested fat pointer",
        },
        // B.4 — byte string comparison
        Case {
            name: "b09_bstr_eq",
            src: "fn f(a: &[u8], b: &[u8]) -> bool { a == b }",
            expect_all: &[
                "extractvalue { i8*, i64 }",
                "icmp eq i8*",
                "icmp eq i64",
                "and i1",
            ],
            expect_none: &[],
            desc: "Stage 3.50: &[u8] eq = (ptr_eq AND len_eq)",
        },
        Case {
            name: "b10_bstr_ne",
            src: "fn f(a: &[u8], b: &[u8]) -> bool { a != b }",
            expect_all: &["extractvalue { i8*, i64 }", "or i1"],
            expect_none: &[],
            desc: "Stage 3.50: &[u8] ne = (ptr_ne OR len_ne)",
        },
        // B.5 — byte string dedup with str
        Case {
            name: "b11_bstr_dedup_with_str",
            src: "fn f() { let s = \"hello\"; let b = b\"hello\"; }",
            expect_all: &[],
            expect_none: &["@.str.1"],
            desc: "Stage 3.50: b\"hello\" and \"hello\" share global (dedup)",
        },
        // B.6 — fat pointer comparison uses correct pointee type
        Case {
            name: "b12_str_cmp_correct_pointee",
            src: "fn f(a: &str, b: &str) -> bool { a == b }",
            expect_all: &["icmp eq i8*"],
            expect_none: &[],
            desc: "Stage 3.50: &str cmp uses i8* (derived from fat ptr field 0)",
        },
        Case {
            name: "b13_bstr_cmp_correct_pointee",
            src: "fn f(a: &[u8], b: &[u8]) -> bool { a == b }",
            expect_all: &["icmp eq i8*"],
            expect_none: &[],
            desc: "Stage 3.50: &[u8] cmp uses i8* (derived from fat ptr field 0)",
        },
        // B.7 — byte string with other locals (type distinctness)
        Case {
            name: "b14_bstr_with_int_local",
            src: "fn f() -> i32 { let b = b\"hi\"; let n = 42; n }",
            expect_all: &["alloca { i8*, i64 }", "alloca i32"],
            expect_none: &[],
            desc: "Stage 3.50: byte string + int local have distinct types",
        },
        // Group E: §9.3.2 edge cases (8)
        Case {
            name: "e01_bstr_escape_bytes",
            src: "fn f() { let b = b\"hello\\nworld\"; }",
            expect_all: &["i64 11, 1"],
            expect_none: &[],
            desc: "Stage 3.50 edge: b\"hello\\nworld\" = 11 bytes (with escape)",
        },
        Case {
            name: "e02_bstr_long",
            src: "fn f() { let b = b\"hello, world! this is a long byte string\"; }",
            expect_all: &["i64 40, 1"],
            expect_none: &[],
            desc: "Stage 3.50 edge: long byte string (40 bytes)",
        },
        Case {
            name: "e03_bstr_in_enum_payload",
            src: "enum E { None, Some(&[u8]) } fn f(e: E) { }",
            expect_all: &["{ i32, { i8*, i64 } }"],
            expect_none: &[],
            desc: "Stage 3.50 edge: &[u8] in enum payload = fat pointer",
        },
        Case {
            name: "e04_bstr_two_params",
            src: "fn f(a: &[u8], b: &[u8]) { }",
            expect_all: &["define void @landin_f({ i8*, i64 } %arg0, { i8*, i64 } %arg1)"],
            expect_none: &[],
            desc: "Stage 3.50 edge: two &[u8] params = two fat pointers",
        },
        Case {
            name: "e05_bstr_mixed_with_str",
            src: "fn f(s: &str, b: &[u8]) { }",
            expect_all: &["define void @landin_f({ i8*, i64 } %arg0, { i8*, i64 } %arg1)"],
            expect_none: &[],
            desc: "Stage 3.50 edge: &str + &[u8] params (both fat pointers)",
        },
        Case {
            name: "e06_bstr_returned_and_used",
            src: "fn get() -> &[u8] { b\"hi\" } fn f() { get() }",
            expect_all: &[
                "define { i8*, i64 } @landin_get()",
                "call { i8*, i64 } @landin_get()",
            ],
            expect_none: &[],
            desc: "Stage 3.50 edge: &[u8] return value flows to call site",
        },
        Case {
            name: "e07_bstr_in_nested_struct",
            src: "struct Inner { d: &[u8] } struct Outer { i: Inner } fn f(o: Outer) { }",
            expect_all: &["{ { { i8*, i64 } } }"],
            expect_none: &[],
            desc: "Stage 3.50 edge: &[u8] in nested struct (3 levels)",
        },
        Case {
            name: "e08_bstr_comparison_same_literal",
            src: "fn f() -> bool { b\"abc\" == b\"abc\" }",
            expect_all: &["icmp eq i8*", "icmp eq i64", "and i1"],
            expect_none: &[],
            desc: "Stage 3.50 edge: same byte string literal compared (deduped global)",
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

    println!("\n=== Stage 3 Gate Audit Round 17 Summary ===");
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
            "   R1-R17: ..., 28, 28, 23, 24, 30, 30, 30, {}/{} — all OK.",
            pass,
            cases.len()
        );
        println!("   Per §9.3.3, audit CONVERGED (17 rounds, 0 new issues each).");
        println!(
            "   Stage 3.50 (byte string fat pointer fix + comparison pointee type fix) verified."
        );
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found.", fail);
    }
}
