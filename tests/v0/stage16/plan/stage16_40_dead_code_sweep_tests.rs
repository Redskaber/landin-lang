//! Stage 16.40 — Dead dyn_trait_emit re-exports cleanup verification.
//!
//! These tests verify that removing the dead re-exports doesn't break
//! any codegen functionality.

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.40 test 1: Basic codegen still works after re-export cleanup.
#[test]
fn stage16_40_basic_codegen() {
    let result = compile("fn main() -> i32 { 42 }");
    assert!(!result.has_errors());
}

/// Stage 16.40 test 2: Closure codegen still works.
#[test]
fn stage16_40_closure_codegen() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(10) }");
    assert!(!result.has_errors());
}

/// Stage 16.40 test 3: Trait dispatch still works (vtable globals).
#[test]
fn stage16_40_trait_dispatch() {
    let src = "trait Foo { fn bar(&self) -> i32; } struct S; impl Foo for S { fn bar(&self) -> i32 { 42 } } fn main() -> i32 { let s = S; s.bar() }";
    let result = compile(src);
    assert!(!result.has_errors());
}

/// Stage 16.40 test 4: Dyn trait still works (fat pointer globals).
#[test]
fn stage16_40_dyn_trait() {
    let src = "trait Foo { fn bar(&self) -> i32; } struct S; impl Foo for S { fn bar(&self) -> i32 { 42 } } fn main() -> i32 { let d: dyn Foo = &S; d.bar() }";
    let result = compile(src);
    assert!(!result.has_errors());
}

/// Stage 16.40 test 5: Triple-nested closure still works.
#[test]
fn stage16_40_triple_nested_closure() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || || x; let _ = f()()(); 42 }");
    assert!(!result.has_errors());
}

/// Stage 16.40 test 6: Mutable capture still works.
#[test]
fn stage16_40_mutable_capture() {
    let result = compile("fn main() { let mut x=0; let f=||{while x<3{x+=1;}x}; }");
    assert!(!result.has_errors());
}

/// Stage 16.40 test 7: dyn_trait_emit module still accessible via full path.
#[test]
fn stage16_40_dyn_trait_emit_accessible() {
    // Verify the module is accessible via the full path (not just re-exports).
    // This is a compile-time check — if the module path changed, this fails.
    use landin_compiler::codegen::dyn_trait_emit;
    // Just reference the function to verify it's accessible.
    let _ = dyn_trait_emit::emit_dyn_trait_fat_ptr_text;
}

/// Stage 16.40 test 8: Complete program with all features.
#[test]
fn stage16_40_complete_program() {
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
