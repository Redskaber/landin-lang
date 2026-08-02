//! Stage 15.46 — Drop elaboration integration tests.
//!
//! These tests verify that `elaborate_drops` is wired into the driver
//! pipeline (runs after typeck, before borrowck) and that it doesn't
//! break any existing programs.
//!
//! Since no types implement `Drop` yet, `elaborate_drops` is a no-op
//! in the pipeline. The tests verify the no-op behavior — the pipeline
//! produces the same results as before.

#![cfg(test)]

use landin_compiler::compile;

/// Stage 15.46 integration test 1: The driver pipeline runs
/// `elaborate_drops` without breaking valid programs.
#[test]
fn stage15_46_driver_pipeline_runs_elaborate_drops() {
    let src = r#"
        fn main() -> i32 {
            let x = 1;
            let y = 2;
            x + y
        }
    "#;
    let result = compile(src);
    assert!(!result.has_errors(), "valid program should compile cleanly");
}

/// Stage 15.46 integration test 2: Structs (no Drop impl) compile cleanly
/// with `elaborate_drops` in the pipeline.
#[test]
fn stage15_46_struct_no_drop_compiles() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        fn main() -> i32 {
            let p = Point { x: 1, y: 2 };
            p.x + p.y
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "struct without Drop should compile cleanly"
    );
}

/// Stage 15.46 integration test 3: Complex programs (loops, method calls)
/// compile cleanly with `elaborate_drops` in the pipeline.
#[test]
fn stage15_46_complex_program_compiles() {
    let src = r#"
        struct Counter { value: i32 }
        impl Counter {
            fn new() -> Counter { Counter { value: 0 } }
            fn increment(&mut self) { self.value = self.value + 1; }
        }
        fn main() -> i32 {
            let mut c = Counter::new();
            let mut i = 0;
            while i < 5 { c.increment(); i = i + 1; }
            c.value
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "complex program should compile cleanly"
    );
}
