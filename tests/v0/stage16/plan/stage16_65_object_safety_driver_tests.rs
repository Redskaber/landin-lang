//! Stage 16.65 — Task 14 Phase 2: Object safety driver integration tests.
//!
//! These tests verify that the object safety check is correctly wired into
//! the driver pipeline. Non-object-safe traits used as `dyn Trait` should
//! produce compilation errors.

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.65 test 1: Object-safe trait with dyn Trait compiles.
#[test]
fn stage16_65_safe_trait_dyn_compiles() {
    let src = "trait Foo { fn bar(&self) -> i32; } struct S; impl Foo for S { fn bar(&self) -> i32 { 42 } } fn main() { let s: dyn Foo = S; }";
    let result = compile(src);
    // Object-safe trait — should NOT have object safety errors.
    // (May have other errors due to dyn Trait limitations, but no
    // "not object-safe" error.)
    let has_obj_safety_error = result
        .errors
        .typeck
        .iter()
        .any(|e| e.message.contains("not object-safe"));
    assert!(
        !has_obj_safety_error,
        "Object-safe trait should not produce object safety error"
    );
}

/// Stage 16.65 test 2: Non-object-safe trait (Self return) with dyn Trait errors.
#[test]
fn stage16_65_self_return_dyn_errors() {
    let src = "trait Foo { fn bar(&self) -> Self; } struct S; impl Foo for S { fn bar(&self) -> Self { S } } fn main() { let s: dyn Foo = S; }";
    let result = compile(src);
    let has_obj_safety_error = result
        .errors
        .typeck
        .iter()
        .any(|e| e.message.contains("not object-safe"));
    assert!(
        has_obj_safety_error,
        "Non-object-safe trait (Self return) should produce object safety error"
    );
}

/// Stage 16.65 test 3: Non-object-safe trait (generic method) with dyn Trait errors.
#[test]
fn stage16_65_generic_method_dyn_errors() {
    let src = "trait Foo { fn bar<T>(&self, x: T); } struct S; impl Foo for S { fn bar<T>(&self, x: T) {} } fn main() { let s: dyn Foo = S; }";
    let result = compile(src);
    let has_obj_safety_error = result
        .errors
        .typeck
        .iter()
        .any(|e| e.message.contains("not object-safe"));
    assert!(
        has_obj_safety_error,
        "Non-object-safe trait (generic method) should produce object safety error"
    );
}

/// Stage 16.65 test 4: Non-object-safe trait (no receiver) with dyn Trait errors.
#[test]
fn stage16_65_no_receiver_dyn_errors() {
    let src = "trait Foo { fn bar() -> i32; } struct S; impl Foo for S { fn bar() -> i32 { 42 } } fn main() { let s: dyn Foo = S; }";
    let result = compile(src);
    let has_obj_safety_error = result
        .errors
        .typeck
        .iter()
        .any(|e| e.message.contains("not object-safe"));
    assert!(
        has_obj_safety_error,
        "Non-object-safe trait (no receiver) should produce object safety error"
    );
}

/// Stage 16.65 test 5: Non-object-safe trait (by-value self) with dyn Trait errors.
#[test]
fn stage16_65_by_value_self_dyn_errors() {
    let src = "trait Foo { fn bar(self) -> i32; } struct S; impl Foo for S { fn bar(self) -> i32 { 42 } } fn main() { let s: dyn Foo = S; }";
    let result = compile(src);
    let has_obj_safety_error = result
        .errors
        .typeck
        .iter()
        .any(|e| e.message.contains("not object-safe"));
    assert!(
        has_obj_safety_error,
        "Non-object-safe trait (by-value self) should produce object safety error"
    );
}

/// Stage 16.65 test 6: Non-object-safe trait (Self in arg) with dyn Trait errors.
#[test]
fn stage16_65_self_in_arg_dyn_errors() {
    let src = "trait Foo { fn bar(&self, x: Self); } struct S; impl Foo for S { fn bar(&self, x: Self) {} } fn main() { let s: dyn Foo = S; }";
    let result = compile(src);
    let has_obj_safety_error = result
        .errors
        .typeck
        .iter()
        .any(|e| e.message.contains("not object-safe"));
    assert!(
        has_obj_safety_error,
        "Non-object-safe trait (Self in arg) should produce object safety error"
    );
}

/// Stage 16.65 test 7: Object-safe trait (&mut self) with dyn Trait compiles.
#[test]
fn stage16_65_ref_mut_self_dyn_compiles() {
    let src = "trait Foo { fn bar(&mut self); } struct S; impl Foo for S { fn bar(&mut self) {} } fn main() { let s: dyn Foo = S; }";
    let result = compile(src);
    let has_obj_safety_error = result
        .errors
        .typeck
        .iter()
        .any(|e| e.message.contains("not object-safe"));
    assert!(
        !has_obj_safety_error,
        "Object-safe trait (&mut self) should not produce object safety error"
    );
}

/// Stage 16.65 test 8: Empty trait with dyn Trait compiles.
#[test]
fn stage16_65_empty_trait_dyn_compiles() {
    let src = "trait Foo {} struct S; impl Foo for S {} fn main() { let s: dyn Foo = S; }";
    let result = compile(src);
    let has_obj_safety_error = result
        .errors
        .typeck
        .iter()
        .any(|e| e.message.contains("not object-safe"));
    assert!(
        !has_obj_safety_error,
        "Empty trait should not produce object safety error"
    );
}
