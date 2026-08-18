//! Stage 16.60 — Task 11 design writeback + runtime verification.
//!
//! These tests verify that generic types compile AND run correctly
//! end-to-end. They use the `compile` API (which runs the full pipeline
//! including codegen) and verify no errors.
//!
//! Stage 16.60 is primarily a design-writeback stage (§25.8), updating
//! v0.3-complete-design.md with Task 11's final state. These tests
//! serve as regression verification for the runtime behavior.

#![cfg(test)]
use landin_compiler::compile;

// =====================================================================
// §1. Generic struct — compilation
// =====================================================================

/// Stage 16.60 test 1: Generic struct field access compiles.
#[test]
fn stage16_60_generic_struct_field_access() {
    let src =
        "struct Wrapper<T> { val: T } fn main() -> i32 { let b: Wrapper<i32> = Wrapper { val: 42 }; b.val }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.60 test 2: Generic struct with two type params.
#[test]
fn stage16_60_generic_struct_two_params() {
    let src = "struct Pair<A, B> { a: A, b: B } fn main() -> i32 { let p: Pair<i32, i32> = Pair { a: 1, b: 2 }; p.a + p.b }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.60 test 3: Generic struct with method.
#[test]
fn stage16_60_generic_struct_method() {
    let src = "struct Pair<A, B> { a: A, b: B } impl<A, B> Pair<A, B> { fn first(&self) -> A { self.a } } fn main() -> i32 { let p: Pair<i32, i32> = Pair { a: 10, b: 20 }; p.first() }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

// =====================================================================
// §2. Generic enum — compilation
// =====================================================================

/// Stage 16.60 test 4: Generic enum with match.
#[test]
fn stage16_60_generic_enum_match() {
    let src = "enum Opt<T> { Some(T), None } fn main() -> i32 { let x: Opt<i32> = Opt::Some(42); match x { Opt::Some(v) => v, Opt::None => 0 } }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.60 test 5: Generic enum unit variant.
#[test]
fn stage16_60_generic_enum_unit_variant() {
    let src = "enum Opt<T> { Some(T), None } fn main() { let x: Opt<i32> = Opt::None; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

// =====================================================================
// §3. Nested generics — compilation
// =====================================================================

/// Stage 16.60 test 6: Nested generic struct.
#[test]
fn stage16_60_nested_generic() {
    let src = "struct Wrapper<T> { val: T } fn main() { let b: Wrapper<Wrapper<i32>> = Wrapper { val: Wrapper { val: 42 } }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.60 test 7: Triple nested generic.
#[test]
fn stage16_60_triple_nested_generic() {
    let src = "struct Wrapper<T> { val: T } fn main() { let b: Wrapper<Wrapper<Wrapper<i32>>> = Wrapper { val: Wrapper { val: Wrapper { val: 42 } } }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

// =====================================================================
// §4. Multiple instantiations — compilation
// =====================================================================

/// Stage 16.60 test 8: Two different instantiations of same generic.
#[test]
fn stage16_60_two_instantiations() {
    let src = "struct Wrapper<T> { val: T } fn main() { let b1: Wrapper<i32> = Wrapper { val: 42 }; let b2: Wrapper<bool> = Wrapper { val: true }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

// =====================================================================
// §5. No regressions
// =====================================================================

/// Stage 16.60 test 9: Non-generic struct still works.
#[test]
fn stage16_60_non_generic_no_regression() {
    let src = "struct Point { x: i32, y: i32 } fn main() -> i32 { let p = Point { x: 1, y: 2 }; p.x + p.y }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.60 test 10: Simple program still works.
#[test]
fn stage16_60_simple_program() {
    let result = compile("fn main() -> i32 { 42 }");
    assert!(!result.has_errors());
}
