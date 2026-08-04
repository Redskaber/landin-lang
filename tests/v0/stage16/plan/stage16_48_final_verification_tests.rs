//! Stage 16.48 — Final v0.3 release verification.

#![cfg(test)]
use landin_compiler::compile;

#[test]
fn stage16_48_basic_codegen() {
    let result = compile("fn main() -> i32 { 42 }");
    assert!(!result.has_errors());
}

#[test]
fn stage16_48_closure_codegen() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(10) }");
    assert!(!result.has_errors());
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 1);
}

#[test]
fn stage16_48_triple_nested_closure() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || || x; let _ = f()()(); 42 }");
    assert!(!result.has_errors());
    assert_eq!(result.synthesized_closure_mir_bodies.len(), 3);
}

#[test]
fn stage16_48_mutable_capture() {
    let result = compile("fn main() { let mut x=0; let f=||{while x<3{x+=1;}x}; }");
    assert!(!result.has_errors());
}

#[test]
fn stage16_48_sound_copy() {
    let src = "struct R; impl Drop for R { fn drop(&mut self) {} } fn main() -> i32 { let r = R; let r2 = r; let r3 = r; 0 }";
    let result = compile(src);
    assert!(!result.errors.borrowck.is_empty());
}

#[test]
fn stage16_48_trait_dispatch() {
    let src = "trait Foo { fn bar(&self) -> i32; } struct S; impl Foo for S { fn bar(&self) -> i32 { 42 } } fn main() -> i32 { let s = S; s.bar() }";
    let result = compile(src);
    assert!(!result.has_errors());
}

#[test]
fn stage16_48_dyn_trait() {
    let src = "trait Foo { fn bar(&self) -> i32; } struct S; impl Foo for S { fn bar(&self) -> i32 { 42 } } fn main() -> i32 { let d: dyn Foo = &S; d.bar() }";
    let result = compile(src);
    assert!(!result.has_errors());
}

#[test]
fn stage16_48_complete_program() {
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
