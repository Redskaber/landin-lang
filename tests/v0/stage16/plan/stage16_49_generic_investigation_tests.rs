//! Stage 16.49 — Generic parser support investigation verification.

#![cfg(test)]
use landin_compiler::compile;

#[test]
fn stage16_49_basic_codegen() {
    let result = compile("fn main() -> i32 { 42 }");
    assert!(!result.has_errors());
}

#[test]
fn stage16_49_closure_codegen() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(10) }");
    assert!(!result.has_errors());
}

#[test]
fn stage16_49_struct_codegen() {
    let src = "struct Point { x: i32, y: i32 } fn main() -> i32 { let p = Point { x: 1, y: 2 }; p.x + p.y }";
    let result = compile(src);
    assert!(!result.has_errors());
}

#[test]
fn stage16_49_trait_dispatch() {
    let src = "trait Foo { fn bar(&self) -> i32; } struct S; impl Foo for S { fn bar(&self) -> i32 { 42 } } fn main() -> i32 { let s = S; s.bar() }";
    let result = compile(src);
    assert!(!result.has_errors());
}

#[test]
fn stage16_49_triple_nested_closure() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || || x; let _ = f()()(); 42 }");
    assert!(!result.has_errors());
}

#[test]
fn stage16_49_complete_program() {
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
