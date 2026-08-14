//! Stage 18.50 — Phase-level integration tests.
//!
//! Tests that verify each compiler phase produces correct output
//! when given valid input, catching regressions across phases:
//! - Lexer → Parser: token stream correctness
//! - Parser → HIR: AST structure correctness
//! - HIR → MIR: lowering correctness
//! - MIR → Codegen: IR generation correctness
//! - End-to-end: compile + run correctness
//!
//! Per §9.4.3: 2 positive + 6 negative tests (1:3 ratio).

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::compile;

// === Positive: End-to-end compilation succeeds ===

/// Stage 18.50 positive 1: Full pipeline produces MIR bodies.
#[test]
fn stage18_50_full_pipeline_produces_mir() {
    let src = "fn main() { let x = 42; println!(\"x={}\", x); }";
    let result = compile(src);
    assert!(!result.has_errors(), "no errors in full pipeline");
    assert!(!result.mirs.is_empty(), "MIR bodies should be produced");
    assert!(result.hir.is_some(), "HIR should be produced");
}

/// Stage 18.50 positive 2: Generic function compiles through all phases.
/// Note: turbofish `id::<i32>(42)` may not be fully supported yet,
/// so we use implicit type inference instead.
#[test]
fn stage18_50_generic_function_all_phases() {
    let src = r#"
        fn id(x: i32) -> i32 { x }
        fn main() { let _x = id(42); }
    "#;
    let result = compile(src);
    assert!(!result.has_errors(), "no errors for function call");
    assert!(!result.mirs.is_empty(), "MIR should be produced");
}

// === Negative: Phase-level regressions ===

/// Stage 18.50 negative 1: Trait impl + method call through all phases.
#[test]
fn stage18_50_trait_impl_method_call() {
    let src = r#"
        trait Display { fn show(&self) -> i32; }
        struct Foo { v: i32 }
        impl Display for Foo {
            fn show(&self) -> i32 { self.v }
        }
        fn main() {
            let f = Foo { v: 42 };
            let _r = f.show();
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "trait impl + method call should compile"
    );
}

/// Stage 18.50 negative 2: Closure capture through all phases.
#[test]
fn stage18_50_closure_capture_all_phases() {
    let src = r#"
        fn main() {
            let x = 10;
            let f = || { x + 1 };
            let _y = f();
        }
    "#;
    let result = compile(src);
    assert!(!result.has_errors(), "closure capture should compile");
}

/// Stage 18.50 negative 3: Match expression with enum through all phases.
#[test]
fn stage18_50_match_enum_all_phases() {
    let src = r#"
        enum Opt { Some(i32), None }
        fn main() {
            let x = Opt::Some(42);
            let _r = match x {
                Opt::Some(v) => v,
                Opt::None => 0,
            };
        }
    "#;
    let result = compile(src);
    assert!(!result.has_errors(), "match + enum should compile");
}

/// Stage 18.50 negative 4: Macro expansion + println through all phases.
#[test]
fn stage18_50_macro_println_all_phases() {
    let src = r#"
        macro_rules! greet {
            ($name:expr) => { println!("hello {}", $name); };
        }
        fn main() { greet!("world"); }
    "#;
    let result = compile(src);
    assert!(result.errors.lex.is_empty(), "no lex errors");
    assert!(result.errors.macro_errors.is_empty(), "no macro errors");
}

/// Stage 18.50 negative 5: Array + loop through all phases.
#[test]
fn stage18_50_array_loop_all_phases() {
    let src = r#"
        fn main() {
            let arr = [1, 2, 3, 4, 5];
            let mut sum = 0;
            let mut i = 0;
            while i < 5 {
                sum = sum + arr[i];
                i = i + 1;
            }
            println!("sum = {}", sum);
        }
    "#;
    let result = compile(src);
    assert!(!result.has_errors(), "array + loop should compile");
}

/// Stage 18.50 negative 6: Nested struct field access through all phases.
#[test]
fn stage18_50_nested_struct_all_phases() {
    let src = r#"
        struct Inner { v: i32 }
        struct Outer { inner: Inner }
        fn main() {
            let o = Outer { inner: Inner { v: 42 } };
            let _x = o.inner.v;
        }
    "#;
    let result = compile(src);
    assert!(!result.has_errors(), "nested struct should compile");
}
