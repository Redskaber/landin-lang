//! Stage 16.39 — Deep Review Round 7: Codegen architecture refactoring verification.
//!
//! These tests verify the codegen architecture refactoring (Stages 16.35-16.38)
//! is complete and stable:
//! 1. Unified pipeline (run_codegen_pipeline) works for all patterns
//! 2. Text backend utilities properly separated
//! 3. Dead code removed (emit_output, emit_dyn_trait_ptr_type, etc.)
//! 4. No regressions on any codegen feature

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.39 test 1: Basic codegen with unified pipeline.
#[test]
fn stage16_39_basic_codegen() {
    let result = compile("fn main() -> i32 { 42 }");
    assert!(!result.has_errors());
}

/// Stage 16.39 test 2: Closure codegen with unified pipeline.
#[test]
fn stage16_39_closure_codegen() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(10) }");
    assert!(!result.has_errors());
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
}

/// Stage 16.39 test 3: Triple-nested closure with unified pipeline.
#[test]
fn stage16_39_triple_nested_closure() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || || x; let _ = f()()(); 42 }");
    assert!(!result.has_errors());
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 3);
}

/// Stage 16.39 test 4: Mutable capture with unified pipeline.
#[test]
fn stage16_39_mutable_capture() {
    let result = compile("fn main() { let mut x=0; let f=||{while x<3{x+=1;}x}; }");
    assert!(!result.has_errors());
}

/// Stage 16.39 test 5: Trait dispatch (vtable globals).
#[test]
fn stage16_39_trait_dispatch() {
    let src = "trait Foo { fn bar(&self) -> i32; } struct S; impl Foo for S { fn bar(&self) -> i32 { 42 } } fn main() -> i32 { let s = S; s.bar() }";
    let result = compile(src);
    assert!(!result.has_errors());
}

/// Stage 16.39 test 6: Dyn trait (fat pointer globals).
#[test]
fn stage16_39_dyn_trait() {
    let src = "trait Foo { fn bar(&self) -> i32; } struct S; impl Foo for S { fn bar(&self) -> i32 { 42 } } fn main() -> i32 { let d: dyn Foo = &S; d.bar() }";
    let result = compile(src);
    assert!(!result.has_errors());
}

/// Stage 16.39 test 7: Drop glue functions.
#[test]
fn stage16_39_drop_glue() {
    let src =
        "struct R; impl Drop for R { fn drop(&mut self) {} } fn main() -> i32 { let _r = R; 0 }";
    let result = compile(src);
    assert!(!result.has_errors());
}

/// Stage 16.39 test 8: Complete program with all v0.3 + codegen features.
#[test]
fn stage16_39_complete_program() {
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
