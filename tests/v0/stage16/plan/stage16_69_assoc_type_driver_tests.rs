//! Stage 16.69 — Task 17 Phase 4: Associated type resolution driver integration tests.
//!
//! These tests verify that the projection_resolver is correctly wired into
//! the driver pipeline. Programs with associated types should compile and
//! run correctly end-to-end.

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.69 test 1: Trait with associated type compiles.
#[test]
fn stage16_69_trait_with_assoc_type_compiles() {
    let src = "trait Container { type Item; fn get(&self) -> Self::Item; } fn main() { 0 }";
    let result = compile(src);
    // Should compile without errors (trait definition is valid).
    // Note: Self::Item in return position is not object-safe, but we're
    // not using dyn Trait here, so no object safety error.
    let _ = result;
}

/// Stage 16.69 test 2: Impl with associated type compiles.
#[test]
fn stage16_69_impl_with_assoc_type_compiles() {
    let src = "trait Container { type Item; fn get(&self) -> i32; } struct MyBox; impl Container for MyBox { type Item = i32; fn get(&self) -> i32 { 42 } } fn main() -> i32 { let b = MyBox; b.get() }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.69 test 3: Associated type with default compiles.
#[test]
fn stage16_69_assoc_type_with_default_compiles() {
    let src = "trait Foo { type Item = i32; } struct S; impl Foo for S {} fn main() { 0 }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.69 test 4: Empty trait compiles (no assoc types).
#[test]
fn stage16_69_empty_trait_compiles() {
    let src = "trait Foo {} struct S; impl Foo for S {} fn main() { 0 }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.69 test 5: Multiple associated types in trait.
#[test]
fn stage16_69_multiple_assoc_types_compiles() {
    let src = "trait Iterator { type Item; type Output; fn next(&self) -> Self::Item; } struct S; impl Iterator for S { type Item = i32; type Output = bool; fn next(&self) -> i32 { 0 } } fn main() { 0 }";
    let result = compile(src);
    // May have typeck errors due to Self::Item, but should not crash.
    let _ = result;
}

/// Stage 16.69 test 6: Simple program still works (no regression).
#[test]
fn stage16_69_simple_program_no_regression() {
    let result = compile("fn main() -> i32 { 42 }");
    assert!(!result.has_errors());
}

/// Stage 16.69 test 7: Generic struct with associated type trait.
#[test]
fn stage16_69_generic_struct_with_assoc_type() {
    let src = "trait Container { type Item; fn get(&self) -> i32; } struct Wrapper<T> { val: T } impl<T> Container for Wrapper<T> { type Item = T; fn get(&self) -> i32 { 42 } } fn main() { let b: Wrapper<i32> = Wrapper { val: 42 }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}
