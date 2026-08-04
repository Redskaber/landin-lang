//! Stage 16.41 — Codegen documentation finalization verification.

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.41 test 1: Basic codegen still works.
#[test]
fn stage16_41_basic_codegen() {
    let result = compile("fn main() -> i32 { 42 }");
    assert!(!result.has_errors());
}

/// Stage 16.41 test 2: Closure codegen still works.
#[test]
fn stage16_41_closure_codegen() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(10) }");
    assert!(!result.has_errors());
}

/// Stage 16.41 test 3: Triple-nested closure still works.
#[test]
fn stage16_41_triple_nested_closure() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || || x; let _ = f()()(); 42 }");
    assert!(!result.has_errors());
}

/// Stage 16.41 test 4: Mutable capture still works.
#[test]
fn stage16_41_mutable_capture() {
    let result = compile("fn main() { let mut x=0; let f=||{while x<3{x+=1;}x}; }");
    assert!(!result.has_errors());
}

/// Stage 16.41 test 5: Trait dispatch still works.
#[test]
fn stage16_41_trait_dispatch() {
    let src = "trait Foo { fn bar(&self) -> i32; } struct S; impl Foo for S { fn bar(&self) -> i32 { 42 } } fn main() -> i32 { let s = S; s.bar() }";
    let result = compile(src);
    assert!(!result.has_errors());
}

/// Stage 16.41 test 6: Complete program.
#[test]
fn stage16_41_complete_program() {
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
