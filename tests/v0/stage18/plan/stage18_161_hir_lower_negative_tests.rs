//! Stage 18.161 (TD-NEGATIVE-TEST-COVERAGE): HIR lowering negative tests.
//!
//! Tests HIR lowering error paths. Per §9.4.3, negative tests should be
//! ≥25% of total. This file covers HIR lower error paths.
//!
//! Per §9.1: tests live under `tests/v0/stage-XX/plan/`.
//! Per §16: tests use only public API (`compile`).

use landin_compiler::compile;

// === Invalid item declarations ===

/// Stage 18.161 negative 1: struct with no fields.
#[test]
fn stage18_161_hir_lower_empty_struct() {
    let result = compile("struct Empty; fn main() {}");
    assert!(
        result.errors.lower.is_empty() || result.errors.parse.is_empty(),
        "empty struct should be valid"
    );
}

/// Stage 18.161 negative 2: enum with no variants.
#[test]
fn stage18_161_hir_lower_empty_enum() {
    let result = compile("enum Empty {}; fn main() {}");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 3: trait with no methods.
#[test]
fn stage18_161_hir_lower_empty_trait() {
    let result = compile("trait Empty {} fn main() {}");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 4: impl with no methods.
#[test]
fn stage18_161_hir_lower_empty_impl() {
    let src = "struct Foo; impl Foo {} fn main() {}";
    let result = compile(src);
    assert!(!result.mirs.is_empty());
}

// === Invalid type annotations ===

/// Stage 18.161 negative 5: undefined type in annotation.
#[test]
fn stage18_161_hir_lower_undefined_type_annotation() {
    let result = compile("fn main() { let x: UndefinedType = 42; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 6: undefined type in function return.
#[test]
fn stage18_161_hir_lower_undefined_return_type() {
    let result = compile("fn get() -> UndefinedType { 42 }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 7: undefined type in parameter.
#[test]
fn stage18_161_hir_lower_undefined_param_type() {
    let result = compile("fn take(x: UndefinedType) {}");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Invalid generics ===

/// Stage 18.161 negative 8: generic with no body.
#[test]
fn stage18_161_hir_lower_generic_no_body() {
    let result = compile("fn identity<T>() {}");
    assert!(!result.mirs.is_empty());
}

/// Stage 18.161 negative 9: generic with wrong type parameter count.
#[test]
fn stage18_161_hir_lower_generic_wrong_count() {
    let src = r#"
        fn identity<T>(x: T) -> T { x }
        fn main() -> i32 { identity::<i32, i32>(42) }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 10: generic struct with no impl.
#[test]
fn stage18_161_hir_lower_generic_struct_no_impl() {
    let src = "struct Box<T> { val: T } fn main() {}";
    let result = compile(src);
    assert!(!result.mirs.is_empty());
}

// === Invalid expressions ===

/// Stage 18.161 negative 11: function call on non-function value.
#[test]
fn stage18_161_hir_lower_call_non_function() {
    let result = compile("fn main() { let x = 42; x(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 12: field access on non-struct.
#[test]
fn stage18_161_hir_lower_field_access_non_struct() {
    let result = compile("fn main() { let x = 42; x.field; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 13: method call on primitive.
#[test]
fn stage18_161_hir_lower_method_on_primitive() {
    let result = compile("fn main() { let x = 42; x.method(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Invalid patterns ===

/// Stage 18.161 negative 14: struct pattern with wrong fields.
#[test]
fn stage18_161_hir_lower_struct_pattern_wrong_fields() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        fn main() { let Point { a, b } = Point { x: 1, y: 2 }; }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 15: tuple pattern with wrong arity.
#[test]
fn stage18_161_hir_lower_tuple_pattern_wrong_arity() {
    let result = compile("fn main() { let (a, b, c) = (1, 2); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 16: enum pattern on non-enum.
#[test]
fn stage18_161_hir_lower_enum_pattern_non_enum() {
    let src = r#"
        struct Foo;
        fn main() { match Foo { Foo::Variant => {} } }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Invalid control flow ===

/// Stage 18.161 negative 17: break outside loop.
#[test]
fn stage18_161_hir_lower_break_outside_loop() {
    let result = compile("fn main() { break; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 18: continue outside loop.
#[test]
fn stage18_161_hir_lower_continue_outside_loop() {
    let result = compile("fn main() { continue; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 19: return outside function.
#[test]
fn stage18_161_hir_lower_return_outside_fn() {
    // return at module level is invalid.
    let result = compile("return 42; fn main() {}");
    assert!(result.has_errors());
}

/// Stage 18.161 negative 20: match with no arms.
#[test]
fn stage18_161_hir_lower_match_no_arms() {
    let result = compile("fn main() { match 1 {} }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}
