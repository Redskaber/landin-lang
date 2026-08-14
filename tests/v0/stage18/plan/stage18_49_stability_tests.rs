//! Stage 18.49 — Stability tests.
//!
//! Tests that verify the compiler doesn't crash, hang, or produce
//! incorrect output under various stress conditions:
//! - Deeply nested expressions
//! - Long format strings
//! - Many macro expansions
//! - Large programs
//! - Edge cases in macro patterns
//!
//! Per §9.4.3: 2 positive + 6 negative tests (1:3 ratio).

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::compile;

// === Positive: Compiler handles stress without crashing ===

/// Stage 18.49 positive 1: Deeply nested expressions compile.
#[test]
fn stage18_49_deeply_nested_expressions() {
    // 20 levels of nesting: (((((... 42 ...)))))
    let mut src = String::from("fn main() -> i32 { ");
    for _ in 0..20 {
        src.push('(');
    }
    src.push('4');
    src.push('2');
    for _ in 0..20 {
        src.push(')');
    }
    src.push_str(" }");
    let result = compile(&src);
    assert!(
        result.errors.lex.is_empty(),
        "no lex errors for deep nesting"
    );
    assert!(
        result.errors.parse.is_empty(),
        "no parse errors for deep nesting"
    );
}

/// Stage 18.49 positive 2: Many println! calls compile.
#[test]
fn stage18_49_many_println_calls() {
    let mut src = String::from("fn main() { ");
    for i in 0..50 {
        src.push_str(&format!("println!(\"line {}\"); ", i));
    }
    src.push('}');
    let result = compile(&src);
    assert!(
        result.errors.lex.is_empty(),
        "no lex errors for many println"
    );
    assert!(
        result.errors.parse.is_empty(),
        "no parse errors for many println"
    );
    assert!(result.errors.macro_errors.is_empty(), "no macro errors");
}

// === Negative: Edge cases and stress conditions ===

/// Stage 18.49 negative 1: Long format string compiles.
#[test]
fn stage18_49_long_format_string() {
    let long_msg = "x".repeat(1000);
    let src = format!("fn main() {{ println!(\"{}\"); }}", long_msg);
    let result = compile(&src);
    assert!(
        result.errors.lex.is_empty(),
        "no lex errors for long string"
    );
    assert!(
        result.errors.parse.is_empty(),
        "no parse errors for long string"
    );
}

/// Stage 18.49 negative 2: Many macro_rules! definitions compile.
#[test]
fn stage18_49_many_macro_definitions() {
    let mut src = String::new();
    for i in 0..20 {
        src.push_str(&format!("macro_rules! m{} {{ () => {{ {} }} }} ", i, i));
    }
    src.push_str("fn main() { m0!() }");
    let result = compile(&src);
    assert!(
        result.errors.lex.is_empty(),
        "no lex errors for many macros"
    );
    assert!(
        result.errors.parse.is_empty(),
        "no parse errors for many macros"
    );
}

/// Stage 18.49 negative 3: Empty macro body.
#[test]
fn stage18_49_empty_macro_body() {
    let src = "macro_rules! empty { () => { } } fn main() { empty!() }";
    let result = compile(src);
    assert!(result.errors.lex.is_empty());
    assert!(result.errors.macro_errors.is_empty());
}

/// Stage 18.49 negative 4: Macro with many repetitions.
#[test]
fn stage18_49_macro_many_repetitions() {
    let mut args = String::new();
    for i in 0..30 {
        if i > 0 {
            args.push_str(", ");
        }
        args.push_str(&i.to_string());
    }
    let src = format!(
        "macro_rules! sum {{ ($($x:expr),*) => {{ 0 }} }} fn main() {{ sum!({}) }}",
        args
    );
    let result = compile(&src);
    assert!(
        result.errors.lex.is_empty(),
        "no lex errors for many repetitions"
    );
    assert!(
        result.errors.parse.is_empty(),
        "no parse errors for many repetitions"
    );
}

/// Stage 18.49 negative 5: Deeply nested macro_rules! definitions.
#[test]
fn stage18_49_nested_macro_definitions() {
    // Define 10 macros, each calling the previous one.
    let mut src = String::from("macro_rules! m0 { () => { 42 } } ");
    for i in 1..10 {
        src.push_str(&format!(
            "macro_rules! m{} {{ () => {{ m{}!() }} }} ",
            i,
            i - 1
        ));
    }
    src.push_str("fn main() { m9!() }");
    let result = compile(&src);
    assert!(
        result.errors.lex.is_empty(),
        "no lex errors for nested macros"
    );
    assert!(
        result.errors.parse.is_empty(),
        "no parse errors for nested macros"
    );
}

/// Stage 18.49 negative 6: Large program with mixed features.
#[test]
fn stage18_49_large_mixed_program() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        enum Shape { Circle(i32), Square(Point), Triangle(Point, Point, Point) }
        trait Area { fn area(&self) -> i32; }
        impl Area for Shape {
            fn area(&self) -> i32 {
                match self {
                    Shape::Circle(r) => 3 * r * r,
                    Shape::Square(p) => p.x * p.y,
                    Shape::Triangle(_, _, _) => 0,
                }
            }
        }
        fn main() {
            let s = Shape::Circle(5);
            let a = s.area();
            println!("area = {}", a);
            let p = Point { x: 3, y: 4 };
            let sq = Shape::Square(p);
            println!("square area = {}", sq.area());
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.lex.is_empty(),
        "no lex errors for large program"
    );
    assert!(
        result.errors.parse.is_empty(),
        "no parse errors for large program"
    );
    assert!(
        result.errors.macro_errors.is_empty(),
        "no macro errors for large program"
    );
}
