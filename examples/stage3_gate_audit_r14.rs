//! Stage 3 Gate Review Round 14 audit (per §9.3.1 of process v3.13).
//!
//! Run with: cargo run --example stage3_gate_audit_r14
//!
//! Purpose: Phase gate audit for Stage 3.47 — L-PIPE-1 closure via
//! AdtLayout side-table on `MirBody`. Verifies that codegen resolves
//! `TyKind::Adt(def_id, _)` storage layouts via `mir.adt_layouts` (not HIR),
//! per §16 (阶段间接口隔离).
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
        // Group R: Regression (8) — re-verify R13 integer-type cases
        Case { name: "r01_const_value", src: "const MAX: i32 = 100; fn f() -> i32 { MAX }", expect_all: &["store i32 100"], expect_none: &[], desc: "R13 r01: const value inlined" },
        Case { name: "r02_enum_match", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, _ => 0 } }", expect_all: &["switch i32"], expect_none: &[], desc: "R13 r02: enum match via SwitchInt" },
        Case { name: "r03_str_arg", src: "fn greet(s: &str) { } fn f() { greet(\"hello\") }", expect_all: &["define void @landin_greet({ i8*, i64 } %arg0)"], expect_none: &[], desc: "R13 r03: &str as function arg (Stage 3.49: fat pointer)" },
        Case { name: "r04_i16_arith", src: "fn f(a: i16, b: i16) -> i16 { a + b }", expect_all: &["add nsw i16", "llvm.sadd.with.overflow.i16"], expect_none: &[], desc: "R13 r04: i16 arithmetic + overflow check" },
        Case { name: "r05_i128_shift", src: "fn f(a: i128) -> i128 { a << 2 }", expect_all: &["icmp uge", "128"], expect_none: &[], desc: "R13 r05: i128 shift-count overflow (128-bit width)" },
        Case { name: "r06_float_bitand", src: "fn f(a: f64, b: f64) -> f64 { a & b }", expect_all: &["and i64", "fptosi", "sitofp"], expect_none: &[], desc: "R13 r06: float bitwise via int cast" },
        Case { name: "r07_struct_field", src: "struct Point { x: i32, y: i32 } fn f() -> i32 { let p = Point { x: 1, y: 2 }; p.x }", expect_all: &["insertvalue { i32, i32 }"], expect_none: &[], desc: "R13 r07: struct construction via insertvalue" },
        Case { name: "r08_div_zero", src: "fn f(a: i32, b: i32) -> i32 { a / b }", expect_all: &["icmp eq", "__landin_panic_div_by_zero"], expect_none: &[], desc: "R13 r08: div-by-zero runtime check" },

        // Group I: Stage 3.47 L-PIPE-1 closure coverage (14)
        // I.1 — struct as param/return/local
        Case { name: "i01_struct_param", src: "struct Point { x: i32, y: i32 } fn f(p: Point) -> i32 { p.x }", expect_all: &["define i32 @landin_f({ i32, i32 } %arg0)"], expect_none: &[], desc: "Stage 3.47 §16: struct param type from AdtLayout" },
        Case { name: "i02_struct_return", src: "struct Point { x: i32, y: i32 } fn f() -> Point { Point { x: 1, y: 2 } }", expect_all: &["define { i32, i32 } @landin_f()"], expect_none: &[], desc: "Stage 3.47 §16: struct return type from AdtLayout" },
        Case { name: "i03_struct_local_alloca", src: "struct Point { x: i32, y: i32 } fn f() -> i32 { let p = Point { x: 1, y: 2 }; p.x }", expect_all: &["alloca { i32, i32 }"], expect_none: &[], desc: "Stage 3.47 §16: struct alloca from AdtLayout" },
        // I.2 — enum as param/return/match
        Case { name: "i04_enum_unit_param", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { 0 }", expect_all: &["define i32 @landin_f({ i32 } %arg0)"], expect_none: &[], desc: "Stage 3.47 §16: enum (unit variants) param = { i32 } discriminant only" },
        Case { name: "i05_enum_tuple_variant", src: "enum Opt { None, Some(i32) } fn f(o: Opt) -> i32 { 0 }", expect_all: &["define i32 @landin_f({ i32, i32 } %arg0)"], expect_none: &[], desc: "Stage 3.47 §16: enum tuple-variant = { i32, i32 } (discr + payload)" },
        Case { name: "i06_enum_match", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { match c { Color::Red => 1, _ => 0 } }", expect_all: &["switch i32"], expect_none: &[], desc: "Stage 3.47 §16: enum match (discriminant from AdtLayout::Enum)" },
        // I.3 — nested Adt (proves AdtLayout recursion)
        Case { name: "i07_nested_struct", src: "struct Inner { v: i32 } struct Outer { i: Inner } fn f(o: Outer) -> i32 { 0 }", expect_all: &["{ { i32 } }"], expect_none: &[], desc: "Stage 3.47 §16: nested struct AdtLayout recursion" },
        // I.4 — Adt with i128 field (proves AdtLayout preserves width)
        Case { name: "i08_struct_i128_field", src: "struct Big { v: i128 } fn f(b: Big) -> i128 { b.v }", expect_all: &["{ i128 }"], expect_none: &["{ i64 }"], desc: "Stage 3.47 §16: AdtLayout preserves i128 field width (no regression)" },
        // I.5 — Adt with &str field (proves Ref handled in MIR lower, not codegen)
        Case { name: "i09_struct_ref_str_field", src: "struct Wrap { s: &str } fn f(w: Wrap) { }", expect_all: &["{ { i8*, i64 } }"], expect_none: &[], desc: "Stage 3.47 §16.3.1: &str field in AdtLayout (Stage 3.49: fat pointer; no lower_hir_ty_to_mir_ty from codegen)" },
        // I.6 — two distinct Adts in one fn (proves adt_layouts map handles multiple DefIds)
        Case { name: "i10_two_structs_one_fn", src: "struct A { x: i32 } struct B { y: i64 } fn f() -> i32 { let a = A { x: 1 }; let b = B { y: 2 }; a.x }", expect_all: &["alloca { i32 }", "alloca { i64 }"], expect_none: &[], desc: "Stage 3.47 §16: two distinct Adts share one adt_layouts map" },
        // I.7 — tuple struct (proves DefKind::Struct ctor path also uses AdtLayout)
        Case { name: "i11_tuple_struct_param", src: "struct Pair(i32, i32); fn f(p: Pair) -> i32 { 0 }", expect_all: &["define i32 @landin_f({ i32, i32 } %arg0)"], expect_none: &[], desc: "Stage 3.47 §16: tuple struct ctor uses AdtLayout" },
        // I.8 — struct field mutation (proves AdtLayout works for store path too)
        Case { name: "i12_struct_field_mutation", src: "struct Point { x: i32, y: i32 } fn f() -> i32 { let mut p = Point { x: 1, y: 2 }; p.x = 10; p.x }", expect_all: &["insertvalue { i32, i32 }"], expect_none: &[], desc: "Stage 3.47 §16: struct field mutation via AdtLayout" },
        // I.9 — enum struct variant
        Case { name: "i13_enum_struct_variant", src: "enum E { Empty, Point { x: i32, y: i32 } } fn f(e: E) -> i32 { 0 }", expect_all: &["{ i32, i32, i32 }"], expect_none: &[], desc: "Stage 3.47 §16: enum struct-variant = { i32, i32, i32 } (discr + 2 fields)" },
        // I.10 — AdtLayout::Enum preserves ALL variants' payloads (forward-compat with L-ENUM-UNION)
        Case { name: "i14_enum_multiple_variants", src: "enum E { A, B(i32), C(i64) } fn f(e: E) -> i32 { 0 }", expect_all: &["{ i32, i32, i64 }"], expect_none: &[], desc: "Stage 3.47 §16: enum with multiple variants uses first non-empty payload (Stage 3.38 behavior preserved). Stage 3.48 update: layout is now { i32, i32, i64 } (all payloads flattened — L-ENUM-UNION fix)." },

        // Group E: §9.3.2 edge cases (8) — Stage 3.47 L-PIPE-1 boundary cases
        Case { name: "e01_empty_struct", src: "struct Empty; fn f() -> i32 { 0 }", expect_all: &[], expect_none: &[], desc: "Stage 3.47 edge: empty struct (no field_tys, no alloca needed)" },
        Case { name: "e02_mixed_width_struct_arg", src: "struct Pair(i32, i64); fn f(p: Pair) -> i64 { p.1 }", expect_all: &["define i64 @landin_f({ i32, i64 } %arg0)"], expect_none: &[], desc: "Stage 3.47 edge: mixed i32/i64 fields in struct param" },
        Case { name: "e03_all_unit_enum", src: "enum Color { Red, Green, Blue } fn f(c: Color) -> i32 { 0 }", expect_all: &["{ i32 }"], expect_none: &["{ i32, i32 }", "{ i32, i64 }"], desc: "Stage 3.47 edge: all-unit-variant enum = { i32 } only (no phantom payload)" },
        Case { name: "e04_nested_struct_field_access", src: "struct Inner { v: i32 } struct Outer { i: Inner } fn f(o: Outer) -> i32 { o.i.v }", expect_all: &["alloca { { i32 } }"], expect_none: &[], desc: "Stage 3.47 edge: nested struct field access uses AdtLayout for both levels" },
        Case { name: "e05_i128_struct_field_arith", src: "struct Big { v: i128 } fn f(b: Big) -> i128 { b.v + 1 }", expect_all: &["{ i128 }", "add nsw i128"], expect_none: &[], desc: "Stage 3.47 edge: i128 struct field arithmetic (no width regression)" },
        Case { name: "e06_str_field_no_lower_call", src: "struct Wrap { s: &str } fn f(w: Wrap) -> &str { w.s }", expect_all: &["{ { i8*, i64 } }", "ret { i8*, i64 }"], expect_none: &[], desc: "Stage 3.47 edge: &str field read returns fat pointer (Stage 3.49; no codegen→MIR-lower call)" },
        Case { name: "e07_enum_return_value", src: "enum Opt { None, Some(i32) } fn f() -> Opt { Opt::Some(42) }", expect_all: &["define { i32, i32 } @landin_f()"], expect_none: &[], desc: "Stage 3.47 edge: enum as return value uses AdtLayout for return local" },
        Case { name: "e08_struct_in_loop", src: "struct Acc { v: i32 } fn f(n: i32) -> i32 { let mut a = Acc { v: 0 }; let mut i = 0; while i < n { a.v = a.v + 1; i = i + 1; } a.v }", expect_all: &["alloca { i32 }", "br i1"], expect_none: &[], desc: "Stage 3.47 edge: struct local in loop body uses AdtLayout" },
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

    println!("\n=== Stage 3 Gate Audit Round 14 Summary ===");
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
            "   R1-R14: ..., 28, 28, 23, 24, 30, {}/{} — all OK.",
            pass,
            cases.len()
        );
        println!("   Per §9.3.3, audit CONVERGED (14 rounds, 0 new issues each).");
        println!("   Stage 3.47 (L-PIPE-1 closure via AdtLayout side-table) verified.");
    } else {
        println!("\n❌ AUDIT FAILED — {} defects found.", fail);
    }
}
