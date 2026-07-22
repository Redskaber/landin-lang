//! Stage 2.4d audit: run the full pipeline on real-world programs and
//! report any errors. Used to find remaining P0/P1 issues before
//! declaring Stage 2.x done.
//!
//! Run with: cargo run --example stage2_4d_audit

use landin_compiler::driver::compile;

const PROGRAMS: &[(&str, &str)] = &[
    (
        "recursive_fibonacci",
        r#"
        fn fib(n: i64) -> i64 {
            if n < 2 { return n; }
            fib(n - 1) + fib(n - 2)
        }
        fn main() {
            let r = fib(30);
        }
        "#,
    ),
    (
        "iterative_fibonacci",
        r#"
        fn fib(n: i32) -> i32 {
            let mut a = 0;
            let mut b = 1;
            let mut i = 0;
            while i < n {
                let t = a + b;
                a = b;
                b = t;
                i = i + 1;
            }
            a
        }
        "#,
    ),
    (
        "mutual_recursion",
        r#"
        fn is_even(n: i32) -> bool {
            if n == 0 { return true; }
            is_odd(n - 1)
        }
        fn is_odd(n: i32) -> bool {
            if n == 0 { return false; }
            is_even(n - 1)
        }
        "#,
    ),
    (
        "shared_borrow",
        r#"
        fn read_ref(r: &i32) -> i32 {
            *r
        }
        fn main() {
            let x = 42;
            let r = &x;
            read_ref(r)
        }
        "#,
    ),
    (
        "tuple_and_array",
        r#"
        fn make_pair(a: i32, b: bool) -> (i32, bool) {
            (a, b)
        }
        fn make_array() {
            let arr = [1, 2, 3, 4, 5];
        }
        "#,
    ),
    (
        "match_expression",
        r#"
        fn classify(n: i32) -> i32 {
            match n {
                0 => 100,
                1 => 200,
                _ => 300,
            }
        }
        "#,
    ),
    (
        "let_with_type_annotations",
        r#"
        fn typed() {
            let x: i32 = 42;
            let y: bool = true;
            let z: u64 = 100;
            let w: f64 = 3.14;
        }
        "#,
    ),
    (
        "short_circuit_and_or",
        r#"
        fn in_range(x: i32, lo: i32, hi: i32) -> bool {
            x >= lo && x <= hi
        }
        fn either(a: bool, b: bool) -> bool {
            a || b
        }
        "#,
    ),
    (
        "string_literal",
        r#"
        fn greet() {
            let s = "hello";
        }
        "#,
    ),
    (
        "negative_arithmetic",
        r#"
        fn math(a: i32, b: i32) -> i32 {
            -a + b
        }
        "#,
    ),
    (
        "nested_loops",
        r#"
        fn matrix_sum(rows: i32, cols: i32) -> i32 {
            let mut i = 0;
            let mut sum = 0;
            while i < rows {
                let mut j = 0;
                while j < cols {
                    sum = sum + i * j;
                    j = j + 1;
                }
                i = i + 1;
            }
            sum
        }
        "#,
    ),
    (
        "struct_definition",
        r#"
        struct Point {
            x: i32,
            y: i32,
        }
        fn make_point(x: i32, y: i32) -> Point {
            Point { x: x, y: y }
        }
        "#,
    ),
    (
        "enum_definition",
        r#"
        enum Shape {
            Circle(i32),
            Square(i32, i32),
            Triangle,
        }
        fn area(s: Shape) -> i32 {
            match s {
                Circle(r) => r * r,
                Square(w, h) => w * h,
                Triangle => 0,
            }
        }
        "#,
    ),
    (
        "error_case_type_mismatch",
        // Intentional type error to demonstrate error display.
        r#"
        fn bad() -> i32 {
            let x: bool = 42;
            x
        }
        "#,
    ),
    (
        "error_case_lex",
        // Intentional lex error.
        r#"
        fn bad() {
            let s = "unterminated;
        }
        "#,
    ),
];

fn main() {
    let mut total_errors = 0;
    let mut total_programs = 0;
    let mut clean_programs = 0;

    for (name, src) in PROGRAMS {
        total_programs += 1;
        let result = compile(src);
        let error_count = result.errors.total_count();
        println!("\n=== {} ===", name);
        if error_count == 0 {
            println!("  OK clean ({} bodies)", result.mirs.len());
            clean_programs += 1;
        } else {
            total_errors += error_count;
            println!("  FAIL {} error(s):", error_count);
            // Use the new format_for_user method to print pretty errors.
            println!("{}", result.errors.format_for_user(Some(src)));
        }
    }

    println!("\n=== Summary ===");
    println!(
        "Programs: {} ({} clean, {} with errors)",
        total_programs,
        clean_programs,
        total_programs - clean_programs
    );
    println!("Total errors: {}", total_errors);
}
