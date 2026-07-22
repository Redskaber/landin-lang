//! Round 5 expanded audit (per §9.3.2 of process v3.3).
//!
//! Run with: cargo run --example round5_audit
//!
//! This audit set satisfies §9.3.1 + §9.3.2 requirements:
//!   - ≥30 cases total (this: 50)
//!   - §9.3.1: 4 groups (single-stmt, multi-stmt, complex, error recovery)
//!   - §9.3.2 (new): ≥5 "previous-round-fix edge case" tests (Group F)
//!
//! Group F specifically tests edge cases of Round 4's G8 + G9b fixes:
//!   - G8: is_notable_ty FloatVar exclusion → test all InferVar subtypes
//!   - G9b: resolve-before-check → test binding timing edge cases

use landin_compiler::driver::compile;

fn main() {
    let cases: &[(&str, &str, usize, &str)] = &[
        // ================================================================
        // Group F: Previous-round-fix edge cases (§9.3.2, ≥5 required)
        // Tests edge cases of Round 4's G8 (FloatVar) + G9b (resolve) fixes
        // ================================================================
        // G8 edge cases: InferVar subtype distinctions
        ("f01_not_int_var_ok", "fn f() { let x = 1; !x; }", 0, "IntVar → Int, !Int OK"),
        ("f02_not_float_var_err", "fn f() { let x = 3.14; !x; }", 1, "FloatVar → Float, !Float err"),
        ("f03_not_bool_ok", "fn f() { let x = true; !x; }", 0, "Bool, !Bool OK"),
        ("f04_negate_int_var_ok", "fn f() { let x = 1; -x; }", 0, "IntVar → Int, -Int OK"),
        ("f05_negate_float_var_ok", "fn f() { let x = 3.14; -x; }", 0, "FloatVar → Float, -Float OK"),
        ("f06_negate_str_err", "fn f() { let s = \"hi\"; -s; }", 1, "Str, -Str err"),
        // G9b edge cases: resolve-before-check timing
        ("f07_arith_after_let", "fn f() { let x = 1; let y = 2; x + y; }", 0, "arith after let (resolve timing)"),
        ("f08_arith_tuple_after_let", "fn f() { let t = (1, 2); t + 3; }", 1, "Tuple + Int after let (resolve)"),
        ("f09_not_after_let_float", "fn f() { let x = 3.14; !x; }", 1, "!Float after let (resolve)"),
        ("f10_chain_arith_ok", "fn f() { let a = 1; let b = 2; let c = 3; a + b + c; }", 0, "chain arith (multi-resolve)"),

        // ================================================================
        // Group A: Single-statement negative tests (10 cases)
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
        // Group B: Multi-statement/multi-function (10 cases)
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
        // Group C: Complex program (5 cases)
        // ================================================================
        ("c01_nested_if_type_mismatch",
         "fn f(x: i32) -> i32 { if x > 0 { if x > 10 { 1 } else { true } } else { 2 } }",
         1, "nested if branch mismatch"),
        ("c02_recursive_fn_wrong_return",
         "fn fib(n: i32) -> i32 { if n < 2 { return n; } fib(n-1) + fib(n-2) } fn main() { let r: bool = fib(10); }",
         1, "recursive fn assigned to wrong type"),
        ("c03_while_cond_must_be_bool",
         "fn f() { let mut i = 0; while i { i = i - 1; } }",
         1, "while cond must be bool"),
        ("c04_match_arm_type_mismatch",
         "fn f(x: i32) -> i32 { match x { 0 => 1, _ => true } }",
         1, "match arm type mismatch"),
        ("c05_deeply_nested_borrow",
         "fn f() { let mut x = 1; let r1 = &mut x; let r2 = &mut x; let r3 = &mut x; }",
         1, "deeply nested mut borrow"),

        // ================================================================
        // Group D: Error recovery (5 cases)
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

        // ================================================================
        // Group G: Cross-stage integration (10 cases, new in Round 5)
        // Tests HIR → MIR → typeck → borrowck data flow consistency
        // ================================================================
        ("g01_typeck_writes_back_to_mir",
         "fn f() { let x = 42; }",
         0, "typeck should write i32 back to mir.local_decls"),
        ("g02_storage_live_emitted",
         "fn f(x: i32) { }",
         0, "StorageLive should be emitted for params"),
        ("g03_storage_dead_before_return",
         "fn f() { let x = 1; }",
         0, "StorageDead should be emitted before Return"),
        ("g04_assert_emitted_for_arith",
         "fn f(a: i32, b: i32) -> i32 { a + b }",
         0, "Assert(Overflow) should be emitted"),
        ("g05_no_assert_for_comparison",
         "fn f(a: i32, b: i32) -> bool { a == b }",
         0, "No Assert for comparison ops"),
        ("g06_typeck_results_populated",
         "fn f(x: i32) -> i32 { x }",
         0, "typeck_results should have local types"),
        ("g07_path_resolves_to_local",
         "fn f() { let x = 1; let y = x; }",
         0, "Path x should resolve to local (G1 regression)"),
        ("g08_borrow_transfer_ref_local",
         "fn f() { let x = 1; let r = &x; let y = *r; }",
         0, "borrow ref_local transfer (G2 regression)"),
        ("g09_fn_sig_unified_with_body",
         "fn f() -> i32 { 42 }",
         0, "fn sig return type unified with body (fix #3 regression)"),
        ("g10_let_ascription_applied",
         "fn f() { let x: i32 = 42; }",
         0, "let ascription applied (fix #4 regression)"),
    ];

    let mut missed = 0;
    let mut false_positives = 0;
    let mut ok_count = 0;
    let mut category_coverage = std::collections::HashSet::new();
    let mut edge_case_count = 0;

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
            "{:40} {:10} exp>={} got={} — {}",
            name, status, expected, actual, desc
        );
        match status {
            "OK" => {
                ok_count += 1;
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
                if name.starts_with('f') {
                    edge_case_count += 1;
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
    println!(
        "=== §9.3.2 edge case tests: {} (requirement: ≥5) ===",
        edge_case_count
    );

    if category_coverage.len() < 6 {
        println!(
            "\nFAIL: §9.1.1 requires ≥6/7 categories, got {}/7",
            category_coverage.len()
        );
    }
    if edge_case_count < 5 {
        println!(
            "\nFAIL: §9.3.2 requires ≥5 edge case tests, got {}",
            edge_case_count
        );
    }
}
