//! Stage 16.38 — Emitter trait split attempt verification.
//!
//! These tests verify that the Emitter trait (with documentation groups)
//! still works correctly after the attempted split and revert.

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.38 test 1: Basic codegen with documentation-grouped trait.
#[test]
fn stage16_38_basic_codegen() {
    let result = compile("fn main() -> i32 { 42 }");
    assert!(!result.has_errors());
}

/// Stage 16.38 test 2: Closure codegen.
#[test]
fn stage16_38_closure_codegen() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(10) }");
    assert!(!result.has_errors());
}

/// Stage 16.38 test 3: Vtable globals.
#[test]
fn stage16_38_vtable_globals() {
    let src = "trait Foo { fn bar(&self) -> i32; } struct S; impl Foo for S { fn bar(&self) -> i32 { 42 } } fn main() -> i32 { let s = S; s.bar() }";
    let result = compile(src);
    assert!(!result.has_errors());
}

/// Stage 16.38 test 4: Nested closure.
#[test]
fn stage16_38_nested_closure() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || x; let _ = f()(); 42 }");
    assert!(!result.has_errors());
}

/// Stage 16.38 test 5: Triple-nested closure.
#[test]
fn stage16_38_triple_nested_closure() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || || x; let _ = f()()(); 42 }");
    assert!(!result.has_errors());
}

/// Stage 16.38 test 6: Mutable capture.
#[test]
fn stage16_38_mutable_capture() {
    let result = compile("fn main() { let mut x=0; let f=||{while x<3{x+=1;}x}; }");
    assert!(!result.has_errors());
}

/// Stage 16.38 test 7: EmitType helpers.
#[test]
fn stage16_38_emit_type_helpers() {
    use landin_compiler::codegen::EmitType;
    let ptr = EmitType::ptr_to(EmitType::I32);
    assert!(ptr.is_ptr());
    assert_eq!(ptr.pointee(), EmitType::I32);
}

/// Stage 16.38 test 8: Fat pointer type.
#[test]
fn stage16_38_fat_ptr_type() {
    use landin_compiler::codegen::{emit_fat_ptr_type, EmitType};
    let fp = emit_fat_ptr_type(EmitType::I8);
    match fp {
        EmitType::Struct(fields) => {
            assert_eq!(fields.len(), 2);
            assert!(fields[0].is_ptr());
        }
        _ => panic!("expected Struct"),
    }
}

/// Stage 16.38 test 9: String globals.
#[test]
fn stage16_38_string_globals() {
    let result = compile("fn main() { println!(\"hello\"); }");
    assert!(!result.has_errors());
}

/// Stage 16.38 test 10: Complete program.
#[test]
fn stage16_38_complete_program() {
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
