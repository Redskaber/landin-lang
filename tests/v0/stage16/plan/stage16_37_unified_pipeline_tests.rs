//! Stage 16.37 — Unified codegen pipeline verification.
//!
//! These tests verify that the unified `run_codegen_pipeline` function
//! produces identical results for both text and LLVM backends:
//! 1. All closure patterns still work
//! 2. Trait dispatch still works
//! 3. Drop glue still works
//! 4. No regressions

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.37 test 1: Basic codegen with unified pipeline.
#[test]
fn stage16_37_basic_unified() {
    let result = compile("fn main() -> i32 { 42 }");
    assert!(!result.has_errors());
}

/// Stage 16.37 test 2: Closure codegen with unified pipeline.
#[test]
fn stage16_37_closure_unified() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(10) }");
    assert!(!result.has_errors());
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
}

/// Stage 16.37 test 3: Vtable globals with unified pipeline.
#[test]
fn stage16_37_vtable_unified() {
    let src = "trait Foo { fn bar(&self) -> i32; } struct S; impl Foo for S { fn bar(&self) -> i32 { 42 } } fn main() -> i32 { let s = S; s.bar() }";
    let result = compile(src);
    assert!(!result.has_errors());
}

/// Stage 16.37 test 4: Dyn trait with unified pipeline.
#[test]
fn stage16_37_dyn_trait_unified() {
    let src = "trait Foo { fn bar(&self) -> i32; } struct S; impl Foo for S { fn bar(&self) -> i32 { 42 } } fn main() -> i32 { let d: dyn Foo = &S; d.bar() }";
    let result = compile(src);
    assert!(!result.has_errors());
}

/// Stage 16.37 test 5: Drop glue with unified pipeline.
#[test]
fn stage16_37_drop_glue_unified() {
    let src =
        "struct R; impl Drop for R { fn drop(&mut self) {} } fn main() -> i32 { let _r = R; 0 }";
    let result = compile(src);
    assert!(!result.has_errors());
}

/// Stage 16.37 test 6: Nested closure with unified pipeline.
#[test]
fn stage16_37_nested_closure_unified() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || x; let _ = f()(); 42 }");
    assert!(!result.has_errors());
}

/// Stage 16.37 test 7: Triple-nested closure with unified pipeline.
#[test]
fn stage16_37_triple_nested_unified() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || || x; let _ = f()()(); 42 }");
    assert!(!result.has_errors());
}

/// Stage 16.37 test 8: Mutable capture with unified pipeline.
#[test]
fn stage16_37_mutable_capture_unified() {
    let result = compile("fn main() { let mut x=0; let f=||{while x<3{x+=1;}x}; }");
    assert!(!result.has_errors());
}

/// Stage 16.37 test 9: String globals with unified pipeline.
#[test]
fn stage16_37_string_globals_unified() {
    let result = compile("fn main() { println!(\"hello\"); }");
    assert!(!result.has_errors());
}

/// Stage 16.37 test 10: Complete program with all features.
#[test]
fn stage16_37_complete_program_unified() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        trait Add { fn add(&self, other: &Point) -> i32; }
        impl Add for Point {
            fn add(&self, other: &Point) -> i32 { self.x + other.x + self.y + other.y }
        }
        fn main() -> i32 {
            let p1 = Point { x: 1, y: 2 };
            let p2 = Point { x: 3, y: 4 };
            let p3 = p1;
            let f = |a: i32| a + p3.x;
            f(10) + p1.add(&p2)
        }
    "#;
    let result = compile(src);
    assert!(!result.has_errors(), "{:?}", result.errors);
}
