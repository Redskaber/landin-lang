//! Round 4 expanded negative-case audit (per §9.3.1 of process v3.2).
//!
//! Run with: cargo run --example round4_audit
//!
//! This audit set satisfies §9.3.1 requirements:
//!   - ≥30 cases total (this: 40)
//!   - 10 single-statement negative tests (basic type system)
//!   - 10 multi-statement/multi-function negative tests (integration)
//!   - 5 complex program negative tests (nested control flow, closures, recursion)
//!   - 5 error recovery tests (one error shouldn't cascade)
//!   - 10 positive cases (regression protection)
//!
//! All 7 categories from §9.1.1 are covered.

use landin_compiler::driver::compile;

fn main() {
    let cases: &[(&str, &str, usize, &str)] = &[
        // ================================================================
        // Group A: Single-statement negative tests (10 cases, basic type system)
        // ================================================================
        ("a01_int_plus_bool", "fn f() { 1 + true; }", 1, "Int + Bool"),
        ("a02_bool_plus_bool", "fn f() { true + false; }", 1, "Bool + Bool"),
        ("a03_negate_bool", "fn f() { -true; }", 1, "-Bool"),
        ("a04_not_float", "fn f() { !3.14; }", 1, "!Float"),
        ("a05_str_plus_int", "fn f() { \"hi\" + 1; }", 1, "Str + Int"),
        ("a06_char_plus_bool", "fn f() { 'a' + true; }", 1, "Char + Bool"),
        ("a07_tuple_plus_int", "fn f() { (1, 2) + 3; }", 1, "Tuple + Int"),
        ("a08_array_plus_int", "fn f() { [1, 2] + 3; }", 1, "Array + Int"),
        ("a09_negate_str", "fn f() { -\"hi\"; }", 1, "-Str"),
        ("a10_negate_tuple", "fn f() { -(1, 2); }", 1, "-Tuple"),

        // ================================================================
        // Group B: Multi-statement/multi-function negative tests (10 cases, integration)
        // ================================================================
        ("b01_let_ascription_mismatch", "fn f() { let x: bool = 42; }", 1, "let x: bool = 42"),
        ("b02_assign_immutable", "fn f() { let x = 1; x = 2; }", 1, "assign immutable"),
        ("b03_use_after_move_str", "fn f() { let s = \"hi\"; let t = s; let u = s; }", 1, "use after move"),
        ("b04_double_mut_borrow", "fn f() { let mut x = 1; let r1 = &mut x; let r2 = &mut x; }", 1, "double mut borrow"),
        ("b05_mut_borrow_immutable", "fn f() { let x = 1; let r = &mut x; }", 1, "mut borrow immutable"),
        ("b06_assign_to_borrowed", "fn f() { let mut x = 1; let r = &x; x = 2; }", 1, "assign to borrowed"),
        ("b07_undefined_fn_call", "fn f() { undefined_fn(); }", 1, "undefined fn"),
        ("b08_wrong_arg_count", "fn add(a: i32, b: i32) -> i32 { a + b } fn main() { add(1); }", 1, "wrong arg count"),
        ("b09_wrong_arg_type", "fn add(a: i32) -> i32 { a } fn main() { add(true); }", 1, "wrong arg type"),
        ("b10_return_type_mismatch", "fn f() -> bool { 42 }", 1, "return type mismatch"),

        // ================================================================
        // Group C: Complex program negative tests (5 cases, nested/closures/recursion)
        // ================================================================
        ("c01_nested_if_type_mismatch",
         "fn f(x: i32) -> i32 { if x > 0 { if x > 10 { 1 } else { true } } else { 2 } }",
         1, "nested if branch mismatch"),
        ("c02_recursive_fn_wrong_return",
         "fn fib(n: i32) -> i32 { if n < 2 { return n; } fib(n-1) + fib(n-2) } fn main() { let r: bool = fib(10); }",
         1, "recursive fn assigned to wrong type"),
        ("c03_closure_call_wrong_args",
         "fn apply(f: fn(i32) -> i32, x: i32) -> i32 { f(x) } fn main() { apply(|a, b| a + b, 1); }",
         1, "closure with wrong arg count"),
        ("c04_while_loop_body_type_mismatch",
         "fn f() -> i32 { let mut i = 0; while i < 10 { i = i + 1; true; } i }",
         0, "while body type ignored (only cond must be bool)"),
        ("c04b_while_cond_must_be_bool",
         "fn f() { let mut i = 0; while i { i = i - 1; } }",
         1, "while cond must be bool (i is i32)"),
        ("c05_match_arm_type_mismatch",
         "fn f(x: i32) -> i32 { match x { 0 => 1, _ => true } }",
         1, "match arm type mismatch"),

        // ================================================================
        // Group D: Error recovery tests (5 cases, one error shouldn't cascade)
        // ================================================================
        ("d01_error_then_valid_let",
         "fn f() { let x: bool = 42; let y = 1; }",
         1, "error in let 1 shouldn't fail let 2"),
        ("d02_two_errors_independent",
         "fn f() { let x: bool = 42; let y: i32 = true; }",
         2, "two independent type errors"),
        ("d03_borrow_error_then_valid",
         "fn f() { let mut x = 1; let r1 = &mut x; let r2 = &mut x; let y = 2; }",
         1, "borrow error then valid let"),
        ("d04_undefined_then_valid_call",
         "fn g() {} fn f() { undefined(); g(); }",
         1, "undefined fn then valid call"),
        ("d05_type_error_in_one_branch",
         "fn f(x: i32) -> i32 { if x > 0 { true } else { 2 } }",
         1, "type error in one if branch"),

        // ================================================================
        // Group E: Positive cases (10 cases, regression protection)
        // ================================================================
        ("e01_simple_let", "fn f() { let x = 1; }", 0, "simple let"),
        ("e02_let_with_annotation", "fn f() { let x: i32 = 42; }", 0, "annotated let"),
        ("e03_shared_borrow", "fn f() { let x = 1; let r = &x; let y = *r; }", 0, "shared borrow"),
        ("e04_mut_borrow_ok", "fn f() { let mut x = 1; let r = &mut x; *r = 2; }", 0, "mut borrow ok"),
        ("e05_recursive_fib", "fn fib(n: i64) -> i64 { if n < 2 { return n; } fib(n-1) + fib(n-2) }", 0, "recursive fib"),
        ("e06_iterative_sum", "fn sum(n: i32) -> i32 { let mut s = 0; let mut i = 0; while i < n { s = s + i; i = i + 1; } s }", 0, "iterative sum"),
        ("e07_nested_if", "fn f(x: i32) -> i32 { if x > 0 { if x > 10 { 1 } else { 2 } } else { 3 } }", 0, "nested if"),
        ("e08_fn_call_ok", "fn g() {} fn f() { g(); }", 0, "fn call ok"),
        ("e09_string_literal", "fn f() { let s = \"hello\"; }", 0, "string literal"),
        ("e10_array_homogeneous", "fn f() { let arr = [1, 2, 3, 4, 5]; }", 0, "homogeneous array"),
    ];

    let mut missed = 0;
    let mut false_positives = 0;
    let mut ok_count = 0;
    let mut category_coverage = std::collections::HashSet::new();

    for (name, src, expected, desc) in cases {
        let result = compile(src);
        let actual = result.errors.total_count();
        let status = if expected > &0 {
            if actual >= *expected {
                "OK"
            } else {
                "MISSED"
            }
        } else {
            if actual == 0 {
                "OK"
            } else {
                "FALSE_POS"
            }
        };
        println!(
            "{:36} {:10} exp>={} got={} — {}",
            name, status, expected, actual, desc
        );
        match status {
            "OK" => {
                ok_count += 1;
                // Track §9.1.1 category coverage
                if name.starts_with('a')
                    || name.starts_with('b')
                    || name.starts_with('c')
                    || name.starts_with('d')
                {
                    if desc.contains("mismatch") || desc.contains("type") {
                        category_coverage.insert("type_mismatch");
                    }
                    if desc.contains("borrow") {
                        category_coverage.insert("borrow_conflict");
                    }
                    if desc.contains("move") {
                        category_coverage.insert("use_after_move");
                    }
                    if desc.contains("undefined") {
                        category_coverage.insert("undefined_name");
                    }
                    if desc.contains("arg count") {
                        category_coverage.insert("wrong_arg_count");
                    }
                    if desc.contains("immutable") {
                        category_coverage.insert("assign_immutable");
                    }
                    if desc.contains("return type") {
                        category_coverage.insert("return_type");
                    }
                }
            }
            "MISSED" => missed += 1,
            "FALSE_POS" => false_positives += 1,
            _ => {}
        }
    }

    println!(
        "\n=== Summary: {} OK, {} missed, {} false_pos ===",
        ok_count, missed, false_positives
    );
    println!(
        "=== §9.1.1 coverage: {}/7 categories ===",
        category_coverage.len()
    );
    for cat in &category_coverage {
        println!("  - {}", cat);
    }
    if category_coverage.len() < 6 {
        println!(
            "\nFAIL: §9.1.1 requires ≥6/7 categories, got {}/7",
            category_coverage.len()
        );
    }
}
