//! Stage 16.36 — Emitter trait cleanup: remove dead `emit_output`.
//!
//! These tests verify that removing `emit_output` from the Emitter trait
//! doesn't break any codegen functionality:
//! 1. TextEmitter still produces correct output (via output_with_globals)
//! 2. LLVMSysEmitter still compiles correctly (via to_module)
//! 3. No regressions on any codegen feature

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.36 test 1: Basic codegen still works after emit_output removal.
#[test]
fn stage16_36_basic_codegen() {
    let result = compile("fn main() -> i32 { 42 }");
    assert!(!result.has_errors());
}

/// Stage 16.36 test 2: Closure codegen still works.
#[test]
fn stage16_36_closure_codegen() {
    let result = compile("fn main() -> i32 { let f = |x| x + 1; f(10) }");
    assert!(!result.has_errors());
}

/// Stage 16.36 test 3: String globals still work (emit_string_global).
#[test]
fn stage16_36_string_globals() {
    let result = compile("fn main() { println!(\"hello\"); }");
    assert!(!result.has_errors());
}

/// Stage 16.36 test 4: Vtable globals still work (emit_vtable_global).
#[test]
fn stage16_36_vtable_globals() {
    let src = "trait Foo { fn bar(&self) -> i32; } struct S; impl Foo for S { fn bar(&self) -> i32 { 42 } } fn main() -> i32 { let s = S; s.bar() }";
    let result = compile(src);
    assert!(!result.has_errors());
}

/// Stage 16.36 test 5: Dyn trait const still works (emit_dyn_trait_const).
#[test]
fn stage16_36_dyn_trait_const() {
    let src = "trait Foo { fn bar(&self) -> i32; } struct S; impl Foo for S { fn bar(&self) -> i32 { 42 } } fn main() -> i32 { let d: dyn Foo = &S; d.bar() }";
    let result = compile(src);
    assert!(!result.has_errors());
}

/// Stage 16.36 test 6: All EmitType helpers still work.
#[test]
fn stage16_36_emit_type_helpers() {
    use landin_compiler::codegen::EmitType;
    let ptr = EmitType::ptr_to(EmitType::I32);
    assert!(ptr.is_ptr());
    assert_eq!(ptr.pointee(), EmitType::I32);
}

/// Stage 16.36 test 7: Fat pointer type still works.
#[test]
fn stage16_36_fat_ptr_type() {
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

/// Stage 16.36 test 8: Nested closure codegen still works.
#[test]
fn stage16_36_nested_closure() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || x; let _ = f()(); 42 }");
    assert!(!result.has_errors());
}

/// Stage 16.36 test 9: Triple-nested closure codegen still works.
#[test]
fn stage16_36_triple_nested_closure() {
    let result = compile("fn main() -> i32 { let x = 1; let f = || || || x; let _ = f()()(); 42 }");
    assert!(!result.has_errors());
}

/// Stage 16.36 test 10: Mutable capture closure codegen still works.
#[test]
fn stage16_36_mutable_capture() {
    let result = compile("fn main() { let mut x=0; let f=||{while x<3{x+=1;}x}; }");
    assert!(!result.has_errors());
}
