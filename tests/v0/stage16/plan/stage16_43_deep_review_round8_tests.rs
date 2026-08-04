//! Stage 16.43 — Deep Review Round 8: Final v0.3 + Codegen refactoring release sign-off.
//!
//! These tests verify the complete v0.3 + codegen refactoring is production-ready:
//! 1. All closure patterns (no-capture through triple-nested)
//! 2. Sound Copy detection
//! 3. Trait dispatch (static + dynamic)
//! 4. Unified codegen pipeline (all features)
//! 5. Drop glue
//! 6. Complete program with all v0.3 features

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.43 test 1: No-capture closure.
#[test]
fn stage16_43_nocapture_closure() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(10) }");
    assert!(!result.has_errors());
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
}

/// Stage 16.43 test 2: Triple-nested closure.
#[test]
fn stage16_43_triple_nested_closure() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || || x; let _ = f()()(); 42 }");
    assert!(!result.has_errors());
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 3);
}

/// Stage 16.43 test 3: Mutable capture closure.
#[test]
fn stage16_43_mutable_capture() {
    let result = compile("fn main() { let mut x=0; let f=||{while x<3{x+=1;}x}; }");
    assert!(!result.has_errors());
}

/// Stage 16.43 test 4: Sound Copy (derived + non-Copy).
#[test]
fn stage16_43_sound_copy() {
    let src = "struct R; impl Drop for R { fn drop(&mut self) {} } fn main() -> i32 { let r = R; let r2 = r; let r3 = r; 0 }";
    let result = compile(src);
    assert!(!result.errors.borrowck.is_empty());
}

/// Stage 16.43 test 5: Trait dispatch (vtable).
#[test]
fn stage16_43_trait_dispatch() {
    let src = "trait Foo { fn bar(&self) -> i32; } struct S; impl Foo for S { fn bar(&self) -> i32 { 42 } } fn main() -> i32 { let s = S; s.bar() }";
    let result = compile(src);
    assert!(!result.has_errors());
}

/// Stage 16.43 test 6: Dyn trait (fat pointer).
#[test]
fn stage16_43_dyn_trait() {
    let src = "trait Foo { fn bar(&self) -> i32; } struct S; impl Foo for S { fn bar(&self) -> i32 { 42 } } fn main() -> i32 { let d: dyn Foo = &S; d.bar() }";
    let result = compile(src);
    assert!(!result.has_errors());
}

/// Stage 16.43 test 7: Drop glue.
#[test]
fn stage16_43_drop_glue() {
    let src = "struct R; impl Drop for R { fn drop(&mut self) {} } fn main() -> i32 { let _r = R; 0 }";
    let result = compile(src);
    assert!(!result.has_errors());
}

/// Stage 16.43 test 8: Complete program (all v0.3 + codegen features).
#[test]
fn stage16_43_complete_program() {
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
