//! Round 3 expanded negative-case audit.
//!
//! Run with: cargo run --example round3_audit
//!
//! This script tests ~30 negative cases to find any remaining
//! soundness holes after Stage 2.4e.

use landin_compiler::driver::compile;

fn main() {
    let cases: &[(&str, &str, usize, &str)] = &[
        // === Type system ===
        // expected = minimum error count (≥1 means "should error")
        (
            "int_plus_bool",
            "fn f() { 1 + true; }",
            1,
            "Int + Bool mismatch",
        ),
        (
            "int_minus_str_via_var",
            "fn f() { let s = \"hi\"; 1 - s; }",
            1,
            "Int - Str via var",
        ),
        (
            "int_minus_str_literal",
            "fn f() { 1 - \"hi\"; }",
            1,
            "Int - Str literal",
        ),
        (
            "bool_plus_bool",
            "fn f() { true + false; }",
            1,
            "Bool + Bool (not arithmetic)",
        ),
        (
            "ref_mismatch",
            "fn f() { let x: &i32 = 42; }",
            1,
            "&i32 annotation with int value",
        ),
        (
            "tuple_mismatch",
            "fn f() { let x: (i32, bool) = (1, 2); }",
            1,
            "Tuple field type mismatch",
        ),
        (
            "array_type_mismatch",
            "fn f() { let x: [i32; 2] = [1, true]; }",
            1,
            "Array elem type mismatch",
        ),
        (
            "array_elem_mismatch",
            "fn f() { let x = [1, true, 2]; }",
            1,
            "Array elem mismatch (no annotation)",
        ),
        (
            "char_plus_int",
            "fn f() { 'a' + 1; }",
            1,
            "Char + Int (not supported)",
        ),
        (
            "float_as_bool",
            "fn f() { let x: bool = 3.14; }",
            1,
            "Float as Bool",
        ),
        ("negate_bool", "fn f() { -true; }", 1, "Negate Bool"),
        ("not_int", "fn f() { !42; }", 0, "Not Int is OK (bitwise)"),
        (
            "not_bool",
            "fn f() { !true; }",
            0,
            "Not Bool is OK (logical)",
        ),
        // === Borrow checker ===
        (
            "double_mut_borrow",
            "fn f() { let mut x = 1; let r1 = &mut x; let r2 = &mut x; }",
            1,
            "Two mut borrows",
        ),
        (
            "shared_then_mut",
            "fn f() { let mut x = 1; let r1 = &x; let r2 = &mut x; }",
            1,
            "Shared then mut",
        ),
        (
            "assign_to_borrowed",
            "fn f() { let mut x = 1; let r = &x; x = 2; }",
            1,
            "Assign to borrowed",
        ),
        (
            "move_borrowed",
            "fn f() { let s = \"hi\"; let r = &s; let t = s; }",
            1,
            "Move borrowed",
        ),
        (
            "use_after_move_str",
            "fn f() { let s = \"hi\"; let t = s; let u = s; }",
            1,
            "Use after move (Str)",
        ),
        (
            "borrow_after_move",
            "fn f() { let s = \"hi\"; let t = s; let r = &s; }",
            1,
            "Borrow after move",
        ),
        // deref_null_ish removed — raw ptr type parsing is Stage 3 work

        // === Mutability ===
        (
            "assign_immutable",
            "fn f() { let x = 1; x = 2; }",
            1,
            "Assign to immutable",
        ),
        (
            "assign_mutable_ok",
            "fn f() { let mut x = 1; x = 2; }",
            0,
            "Assign to mutable OK",
        ),
        (
            "mut_borrow_immutable",
            "fn f() { let x = 1; let r = &mut x; }",
            1,
            "Mut borrow of immutable",
        ),
        // === Function calls ===
        (
            "undefined_fn",
            "fn f() { undefined_fn(); }",
            1,
            "Undefined function",
        ),
        (
            "wrong_arg_count",
            "fn add(a: i32, b: i32) -> i32 { a + b } fn main() { add(1); }",
            1,
            "Too few args",
        ),
        (
            "wrong_arg_count_extra",
            "fn add(a: i32) -> i32 { a } fn main() { add(1, 2); }",
            1,
            "Too many args",
        ),
        (
            "wrong_arg_type",
            "fn add(a: i32) -> i32 { a } fn main() { add(true); }",
            1,
            "Wrong arg type",
        ),
        (
            "return_type_mismatch",
            "fn f() -> bool { 42 }",
            1,
            "Return type mismatch",
        ),
        (
            "call_non_function",
            "fn f() { let x = 1; x(); }",
            1,
            "Call non-function",
        ),
        // === Control flow ===
        (
            "if_branch_mismatch",
            "fn f() -> i32 { if true { 1 } else { true } }",
            1,
            "If branch mismatch",
        ),
        (
            "if_cond_not_bool",
            "fn f() { if 42 { 1 } else { 2 } }",
            1,
            "If cond not bool",
        ),
        (
            "while_cond_not_bool",
            "fn f() { while 42 { 1; } }",
            1,
            "While cond not bool",
        ),
        (
            "match_arm_mismatch",
            "fn f(x: i32) -> i32 { match x { 0 => 1, _ => true } }",
            1,
            "Match arm mismatch",
        ),
        // === Let bindings ===
        (
            "let_ascription_mismatch",
            "fn f() { let x: bool = 42; }",
            1,
            "Let ascription mismatch",
        ),
        (
            "let_ascription_u64_ok",
            "fn f() { let z: u64 = 100; }",
            0,
            "Let ascription u64 OK",
        ),
        (
            "let_ascription_f64_ok",
            "fn f() { let w: f64 = 3.14; }",
            0,
            "Let ascription f64 OK",
        ),
        // === Variable scope ===
        (
            "use_before_decl",
            "fn f() { x = 1; let x = 2; }",
            1,
            "Use before declaration",
        ),
        (
            "undefined_variable",
            "fn f() { let y = x; }",
            1,
            "Undefined variable",
        ),
        // === Positive cases (should compile cleanly) ===
        ("simple_let", "fn f() { let x = 1; }", 0, "Simple let"),
        (
            "let_with_annotation",
            "fn f() { let x: i32 = 42; }",
            0,
            "Annotated let",
        ),
        (
            "shared_borrow_ok",
            "fn f() { let x = 1; let r = &x; let y = *r; }",
            0,
            "Shared borrow OK",
        ),
        (
            "if_branch_ok",
            "fn f() -> i32 { if true { 1 } else { 2 } }",
            0,
            "Matching branches",
        ),
        (
            "fn_call_ok",
            "fn g() {} fn f() { g(); }",
            0,
            "Defined fn call",
        ),
        (
            "recursive_fib",
            "fn fib(n: i64) -> i64 { if n < 2 { return n; } fib(n-1) + fib(n-2) }",
            0,
            "Recursive fibonacci",
        ),
        (
            "string_literal_ok",
            "fn f() { let s = \"hello\"; }",
            0,
            "String literal OK",
        ),
    ];

    let mut missed = 0;
    let mut false_positives = 0;
    let mut ok_count = 0;
    for (name, src, expected, desc) in cases {
        let result = compile(src);
        let actual = result.errors.total_count();
        // expected = minimum error count.
        // If expected > 0: actual must be >= expected (≥1 error = detected).
        // If expected == 0: actual must be 0 (no false positives).
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
            "{:32} {:12} exp>={} got={} — {}",
            name, status, expected, actual, desc
        );
        match status {
            "OK" => ok_count += 1,
            "MISSED" => missed += 1,
            "FALSE_POS" => false_positives += 1,
            _ => {}
        }
    }
    println!(
        "\n=== Summary: {} OK, {} missed, {} false_pos ===",
        ok_count, missed, false_positives
    );
}
