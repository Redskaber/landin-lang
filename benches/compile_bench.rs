//! Stage 4.11: Performance benchmark suite
//!
//! Lightweight compile-time benchmarks using std::time::Instant.
//! No external dependencies (criterion not available in this environment).
//!
//! Run with: cargo bench
//! Or: cargo test --bench compile_bench -- --nocapture

use landin_compiler::compile;
use std::time::Instant;

/// Benchmark: compile a small program (just `fn main() {}`).
#[test]
fn bench_compile_small() {
    let src = "fn main() {}";
    let start = Instant::now();
    let result = compile(src);
    let elapsed = start.elapsed();
    assert!(!result.mirs.is_empty(), "should produce MIR");
    println!("bench_compile_small: {:?}", elapsed);
}

/// Benchmark: compile a medium program (struct + fns + control flow).
#[test]
fn bench_compile_medium() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        fn add(a: i32, b: i32) -> i32 { a + b }
        fn main() {
            let p = Point { x: 1, y: 2 };
            let s = add(p.x, p.y);
            if s > 0 { s } else { 0 }
        }
    "#;
    let start = Instant::now();
    let result = compile(src);
    let elapsed = start.elapsed();
    assert!(!result.mirs.is_empty(), "should produce MIR");
    println!("bench_compile_medium: {:?}", elapsed);
}

/// Benchmark: compile a program with closures.
#[test]
fn bench_compile_closure() {
    let src = r#"
        fn main() {
            let y = 10;
            let f = |x: i32| x + y;
            f(42);
        }
    "#;
    let start = Instant::now();
    let result = compile(src);
    let elapsed = start.elapsed();
    assert!(!result.mirs.is_empty(), "should produce MIR");
    println!("bench_compile_closure: {:?}", elapsed);
}

/// Benchmark: compile a program with macros.
#[test]
fn bench_compile_macros() {
    let src = r#"
        fn main() {
            println!("hello");
            assert!(1 == 1);
            let s = stringify!(x);
        }
    "#;
    let start = Instant::now();
    let result = compile(src);
    let elapsed = start.elapsed();
    assert!(!result.mirs.is_empty(), "should produce MIR");
    println!("bench_compile_macros: {:?}", elapsed);
}

/// Benchmark: compile a program with nested modules.
#[test]
fn bench_compile_nested_modules() {
    let src = r#"
        mod inner { pub fn inner_fn() {} }
        fn main() { inner::inner_fn(); }
    "#;
    let start = Instant::now();
    let result = compile(src);
    let elapsed = start.elapsed();
    assert!(!result.mirs.is_empty(), "should produce MIR");
    println!("bench_compile_nested_modules: {:?}", elapsed);
}
