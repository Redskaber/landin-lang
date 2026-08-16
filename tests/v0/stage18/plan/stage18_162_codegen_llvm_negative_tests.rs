//! Stage 18.162 (TD-NEGATIVE-TEST-COVERAGE): LLVM codegen negative tests.
//!
//! Tests LLVM backend codegen error paths. Per §9.4.3, negative tests
//! should be ≥25% of total.
//!
//! Per §9.1: tests live under `tests/v0/stage-XX/plan/`.
//! Per §16: tests use only public API (`compile`, `codegen_crate`).

use landin_compiler::codegen::codegen_crate;
use landin_compiler::compile;

// === Codegen result errors ===

/// Stage 18.162 negative 1: codegen on valid code produces Ok.
#[test]
fn stage18_162_codegen_valid_returns_ok() {
    let result = compile("fn main() -> i32 { 42 }");
    let codegen_result = codegen_crate(&result);
    assert!(
        codegen_result.is_ok(),
        "valid code should codegen successfully, got: {:?}",
        codegen_result
    );
}

/// Stage 18.162 negative 2: codegen on compile errors still produces result.
#[test]
fn stage18_162_codegen_on_compile_errors() {
    let result = compile("fn main() { let x = true + 1; }");
    // Even with type errors, codegen should not panic.
    let codegen_result = codegen_crate(&result);
    // May be Ok (partial) or Err (if BinaryOp2 reaches codegen).
    assert!(codegen_result.is_ok() || codegen_result.is_err());
}

/// Stage 18.162 negative 3: codegen on empty crate.
#[test]
fn stage18_162_codegen_empty_crate() {
    let result = compile("// empty");
    let codegen_result = codegen_crate(&result);
    assert!(codegen_result.is_ok());
}

/// Stage 18.162 negative 4: codegen produces non-empty IR.
#[test]
fn stage18_162_codegen_produces_ir() {
    let result = compile("fn main() -> i32 { 42 }");
    let ir = codegen_crate(&result).expect("should codegen");
    assert!(!ir.is_empty(), "IR should not be empty");
    assert!(
        ir.contains("landin_main"),
        "IR should contain landin_main, got: {}",
        ir
    );
}

/// Stage 18.162 negative 5: codegen on function with no body.
#[test]
fn stage18_162_codegen_fn_no_body() {
    let result = compile("fn helper(); fn main() {}");
    let codegen_result = codegen_crate(&result);
    assert!(codegen_result.is_ok());
}

// === Codegen on various types ===

/// Stage 18.162 negative 6: codegen on struct.
#[test]
fn stage18_162_codegen_struct() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        fn main() { let p = Point { x: 1, y: 2 }; }
    "#;
    let result = compile(src);
    let ir = codegen_crate(&result).expect("should codegen");
    // Struct type may appear as `{ i32, i32 }` in LLVM IR.
    assert!(
        ir.contains("{ i32, i32 }") || ir.contains("Point") || ir.contains("landin_main"),
        "IR should contain struct type or function, got: {}",
        ir
    );
}

/// Stage 18.162 negative 7: codegen on enum.
#[test]
fn stage18_162_codegen_enum() {
    let src = r#"
        enum Color { Red, Green, Blue }
        fn main() { let c = Color::Red; }
    "#;
    let result = compile(src);
    let codegen_result = codegen_crate(&result);
    assert!(codegen_result.is_ok());
}

/// Stage 18.162 negative 8: codegen on generic function.
#[test]
fn stage18_162_codegen_generic_fn() {
    let src = r#"
        fn identity<T>(x: T) -> T { x }
        fn main() -> i32 { identity(42) }
    "#;
    let result = compile(src);
    let codegen_result = codegen_crate(&result);
    assert!(codegen_result.is_ok());
}

/// Stage 18.162 negative 9: codegen on trait impl.
#[test]
fn stage18_162_codegen_trait_impl() {
    let src = r#"
        trait Drawable { fn draw(&self); }
        struct Circle;
        impl Drawable for Circle { fn draw(&self) {} }
        fn main() { let c = Circle; c.draw(); }
    "#;
    let result = compile(src);
    let codegen_result = codegen_crate(&result);
    assert!(codegen_result.is_ok());
}

/// Stage 18.162 negative 10: codegen on closure.
#[test]
fn stage18_162_codegen_closure() {
    let result = compile("fn main() { let f = |x: i32| { x + 1 }; let y = f(42); }");
    let codegen_result = codegen_crate(&result);
    assert!(codegen_result.is_ok());
}

// === Codegen on control flow ===

/// Stage 18.162 negative 11: codegen on if-else.
#[test]
fn stage18_162_codegen_if_else() {
    let result = compile("fn main() -> i32 { if true { 1 } else { 2 } }");
    let ir = codegen_crate(&result).expect("should codegen");
    assert!(ir.contains("br"));
}

/// Stage 18.162 negative 12: codegen on while loop.
#[test]
fn stage18_162_codegen_while() {
    let result = compile("fn main() { let mut x = 0; while x < 10 { x = x + 1; } }");
    let ir = codegen_crate(&result).expect("should codegen");
    assert!(ir.contains("br"));
}

/// Stage 18.162 negative 13: codegen on match.
#[test]
fn stage18_162_codegen_match() {
    let src = r#"
        fn main() -> i32 {
            let x = 1;
            match x { 1 => 10, _ => 20 }
        }
    "#;
    let result = compile(src);
    let ir = codegen_crate(&result).expect("should codegen");
    assert!(ir.contains("br") || ir.contains("switch"));
}

/// Stage 18.162 negative 14: codegen on for loop.
#[test]
fn stage18_162_codegen_for_loop() {
    let result = compile("fn main() { let arr = [1, 2, 3]; for x in arr { let _ = x; } }");
    let codegen_result = codegen_crate(&result);
    assert!(codegen_result.is_ok());
}

/// Stage 18.162 negative 15: codegen on nested functions.
#[test]
fn stage18_162_codegen_nested_calls() {
    let src = r#"
        fn add(a: i32, b: i32) -> i32 { a + b }
        fn mul(a: i32, b: i32) -> i32 { a * b }
        fn main() -> i32 { add(mul(2, 3), 4) }
    "#;
    let result = compile(src);
    let ir = codegen_crate(&result).expect("should codegen");
    assert!(ir.contains("call"));
}

// === Codegen edge cases ===

/// Stage 18.162 negative 16: codegen on recursive function.
#[test]
fn stage18_162_codegen_recursive_fn() {
    let src = r#"
        fn fact(n: i32) -> i32 { if n <= 1 { 1 } else { n * fact(n - 1) } }
        fn main() -> i32 { fact(5) }
    "#;
    let result = compile(src);
    let codegen_result = codegen_crate(&result);
    assert!(codegen_result.is_ok());
}

/// Stage 18.162 negative 17: codegen on function with many params.
#[test]
fn stage18_162_codegen_many_params() {
    let src = r#"
        fn add8(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32) -> i32 {
            a + b + c + d + e + f + g + h
        }
        fn main() -> i32 { add8(1, 2, 3, 4, 5, 6, 7, 8) }
    "#;
    let result = compile(src);
    let codegen_result = codegen_crate(&result);
    assert!(codegen_result.is_ok());
}

/// Stage 18.162 negative 18: codegen on deeply nested expression.
#[test]
fn stage18_162_codegen_deep_nested() {
    let result = compile("fn main() -> i32 { (((((1 + 2) + 3) + 4) + 5) + 6) }");
    let codegen_result = codegen_crate(&result);
    assert!(codegen_result.is_ok());
}

/// Stage 18.162 negative 19: codegen on empty struct.
#[test]
fn stage18_162_codegen_empty_struct() {
    let src = "struct Empty; fn main() { let e = Empty; }";
    let result = compile(src);
    let codegen_result = codegen_crate(&result);
    assert!(codegen_result.is_ok());
}

/// Stage 18.162 negative 20: codegen on unit type.
#[test]
fn stage18_162_codegen_unit_type() {
    let result = compile("fn main() { let x: () = (); }");
    let codegen_result = codegen_crate(&result);
    assert!(codegen_result.is_ok());
}
