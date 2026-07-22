//! Cross-stage audit: Stage 0.x (lexer/parser) + Stage 1.x (HIR/resolve) + Stage 2.x (MIR/typeck/borrowck).
//!
//! Run with: cargo run --example cross_stage_audit
//!
//! This audit tests all 3 completed stages with negative cases:
//!   - Stage 0: lex errors, parse errors, AST structure
//!   - Stage 1: HIR lowering, name resolution, scope
//!   - Stage 2: type checking, borrow checking (already well-tested)
//!
//! Per §9.3.1: ≥30 cases, 4 groups, all 7 §9.1.1 categories.

use landin_compiler::driver::compile;

fn main() {
    let cases: &[(&str, &str, usize, &str)] = &[
        // ================================================================
        // Group S0: Stage 0 — Lexer/Parser (10 cases)
        // ================================================================
        ("s0_01_unterminated_string", "fn f() { let s = \"unterminated; }", 1, "lex: unterminated string"),
        ("s0_02_unterminated_char", "fn f() { let c = 'a; }", 1, "lex: unterminated char"),
        ("s0_03_missing_semicolon", "fn f() { let x = 1 }", 1, "parse: missing semicolon"),
        ("s0_04_missing_closing_brace", "fn f() { let x = 1;", 1, "parse: missing closing brace"),
        ("s0_05_missing_closing_paren", "fn f() { let x = (1; }", 1, "parse: missing closing paren"),
        ("s0_06_empty_function_body", "fn f()", 1, "parse: empty body (no braces)"),
        ("s0_07_invalid_token", "fn f() { let x = @; }", 1, "lex: invalid token @"),
        ("s0_08_missing_fn_keyword", "f() { let x = 1; }", 1, "parse: missing fn keyword"),
        ("s0_09_missing_fn_name", "fn () { }", 1, "parse: missing fn name"),
        ("s0_10_duplicate_let", "fn f() { let x = 1; let x = 2; }", 0, "parse: duplicate let (shadow OK)"),

        // ================================================================
        // Group S1: Stage 1 — HIR/Resolve (10 cases)
        // ================================================================
        ("s1_01_undefined_variable", "fn f() { let y = x; }", 1, "resolve: undefined variable"),
        ("s1_02_undefined_function", "fn f() { undefined_fn(); }", 1, "resolve: undefined function"),
        ("s1_03_undefined_type", "fn f() { let x: Foo = 1; }", 1, "resolve: undefined type"),
        ("s1_04_use_before_decl", "fn f() { x = 1; let x = 2; }", 1, "resolve: use before decl"),
        ("s1_05_scope_shadow_ok", "fn f() { let x = 1; let x = true; }", 0, "resolve: shadow OK"),
        ("s1_06_block_scope", "fn f() { { let x = 1; } x; }", 1, "resolve: x out of scope"),
        ("s1_07_param_visible", "fn f(x: i32) { let y = x; }", 0, "resolve: param visible"),
        ("s1_08_nested_fn_scope", "fn f() { let x = 1; { let y = x; } }", 0, "resolve: nested scope OK"),
        ("s1_09_self_ref_invalid", "fn f() { let x = x; }", 1, "resolve: self-reference (forward ref)"),
        ("s1_10_function_visible", "fn g() {} fn f() { g(); }", 0, "resolve: function visible"),

        // ================================================================
        // Group S2: Stage 2 — Typeck/Borrowck (10 cases, regression)
        // ================================================================
        ("s2_01_type_mismatch", "fn f() { let x: bool = 42; }", 1, "typeck: type mismatch"),
        ("s2_02_borrow_conflict", "fn f() { let mut x = 1; let r1 = &mut x; let r2 = &mut x; }", 1, "borrowck: conflict"),
        ("s2_03_use_after_move", "fn f() { let s = \"hi\"; let t = s; let u = s; }", 1, "borrowck: use after move"),
        ("s2_04_assign_immutable", "fn f() { let x = 1; x = 2; }", 1, "borrowck: assign immutable"),
        ("s2_05_wrong_arg_count", "fn g(a: i32, b: i32) -> i32 { a+b } fn f() { g(1); }", 1, "typeck: wrong arg count"),
        ("s2_06_return_mismatch", "fn f() -> bool { 42 }", 1, "typeck: return mismatch"),
        ("s2_07_int_plus_bool", "fn f() { 1 + true; }", 1, "typeck: int + bool"),
        ("s2_08_negate_bool", "fn f() { -true; }", 1, "typeck: -bool"),
        ("s2_09_if_cond_not_bool", "fn f() { if 42 { 1; } }", 1, "typeck: if cond not bool"),
        ("s2_10_mut_borrow_imm", "fn f() { let x = 1; let r = &mut x; }", 1, "borrowck: mut borrow imm"),

        // ================================================================
        // Group X: Cross-stage integration (10 cases)
        // ================================================================
        ("x01_lex_then_parse", "fn f() { let x = 1; let y = ; }", 1, "lex OK, parse fails"),
        ("x02_parse_then_resolve", "fn f() { let x: UndefinedType = 1; }", 1, "parse OK, resolve fails"),
        ("x03_resolve_then_typeck", "fn f() { let x: i32 = true; }", 1, "typeck: i32 = true mismatch"),
        ("x03b_resolve_then_typeck_v2", "fn f() { let x: i32 = true; }", 1, "typeck: i32 = true mismatch"),
        ("x04_full_pipeline_ok", "fn f(x: i32) -> i32 { x + 1 }", 0, "full pipeline OK"),
        ("x05_lex_error_aborts", "fn f() { \"unterminated; let x = 1; }", 1, "lex error aborts (no Hir)"),
        ("x06_parse_error_aborts", "fn f() { let x = ; let y = 1; }", 1, "parse error aborts"),
        ("x07_multi_error_recovery", "fn f() { let x: bool = 42; let y: i32 = true; }", 2, "two independent errors"),
        ("x08_error_then_success", "fn g() {} fn f() { undefined(); g(); }", 1, "error then success"),
        ("x09_complex_program", "fn fib(n: i64) -> i64 { if n < 2 { return n; } fib(n-1) + fib(n-2) } fn main() { let r = fib(10); }", 0, "complex program OK"),
        ("x10_struct_def", "struct P { x: i32, y: i32 } fn f() { let p = P { x: 1, y: 2 }; }", 0, "struct definition + literal"),

        // ================================================================
        // Group P: Positive regression (10 cases)
        // ================================================================
        ("p01_simple_let", "fn f() { let x = 1; }", 0, "simple let"),
        ("p02_annotated_let", "fn f() { let x: i32 = 42; }", 0, "annotated let"),
        ("p03_shared_borrow", "fn f() { let x = 1; let r = &x; let y = *r; }", 0, "shared borrow"),
        ("p04_mut_borrow_ok", "fn f() { let mut x = 1; let r = &mut x; *r = 2; }", 0, "mut borrow ok"),
        ("p05_recursive_fib", "fn fib(n: i64) -> i64 { if n < 2 { return n; } fib(n-1) + fib(n-2) }", 0, "recursive fib"),
        ("p06_iterative_sum", "fn sum(n: i32) -> i32 { let mut s = 0; let mut i = 0; while i < n { s = s + i; i = i + 1; } s }", 0, "iterative sum"),
        ("p07_nested_if", "fn f(x: i32) -> i32 { if x > 0 { if x > 10 { 1 } else { 2 } } else { 3 } }", 0, "nested if"),
        ("p08_string_literal", "fn f() { let s = \"hello\"; }", 0, "string literal"),
        ("p09_array_literal", "fn f() { let arr = [1, 2, 3]; }", 0, "array literal"),
        ("p10_match_expr", "fn f(x: i32) -> i32 { match x { 0 => 1, _ => 2 } }", 0, "match expression"),
    ];

    let mut missed = 0;
    let mut false_positives = 0;
    let mut ok_count = 0;
    let mut stage0_pass = 0;
    let mut stage1_pass = 0;
    let mut stage2_pass = 0;
    let mut cross_pass = 0;
    let mut positive_pass = 0;

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
                if name.starts_with("s0") {
                    stage0_pass += 1;
                } else if name.starts_with("s1") {
                    stage1_pass += 1;
                } else if name.starts_with("s2") {
                    stage2_pass += 1;
                } else if name.starts_with('x') {
                    cross_pass += 1;
                } else if name.starts_with('p') {
                    positive_pass += 1;
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
    println!("  Stage 0 (lexer/parser): {}/10", stage0_pass);
    println!("  Stage 1 (HIR/resolve):  {}/10", stage1_pass);
    println!("  Stage 2 (typeck/borrow): {}/10", stage2_pass);
    println!("  Cross-stage:            {}/10", cross_pass);
    println!("  Positive regression:    {}/10", positive_pass);
    if missed == 0 && false_positives == 0 {
        println!("\n✅ CROSS-STAGE AUDIT CONVERGED");
    } else {
        println!("\n❌ Issues found — need fixes");
    }
}
