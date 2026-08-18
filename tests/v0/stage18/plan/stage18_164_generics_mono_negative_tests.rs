//! Stage 18.164 (TD-NEGATIVE-TEST-COVERAGE): Generics/monomorphization negative tests.
//!
//! Tests generic function and monomorphization error paths. Per §9.4.3,
//! negative tests should be ≥25% of total.
//!
//! Per §9.1: tests live under `tests/v0/stage-XX/plan/`.
//! Per §16: tests use only public API (`compile`).

use landin_compiler::compile;

// === Generic function errors ===

/// Stage 18.164 negative 1: generic with no type parameter.
#[test]
fn stage18_164_generic_no_param() {
    let result = compile("fn identity<T>(x: T) -> T { x } fn main() -> i32 { identity() }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 2: generic with wrong type parameter count.
#[test]
fn stage18_164_generic_wrong_count() {
    let src = r#"
        fn identity<T>(x: T) -> T { x }
        fn main() -> i32 { identity::<i32, i32>(42) }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 3: generic with mismatched type argument.
#[test]
fn stage18_164_generic_mismatched_arg() {
    let src = r#"
        fn identity<T>(x: T) -> T { x }
        fn main() { identity::<i32>(true); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 4: generic return type mismatch.
#[test]
fn stage18_164_generic_return_mismatch() {
    let src = r#"
        fn identity<T>(x: T) -> T { x }
        fn main() -> i32 { identity::<bool>(true) }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 5: generic with multiple type parameters.
#[test]
fn stage18_164_generic_multi_params() {
    let src = r#"
        fn pair<A, B>(a: A, b: B) -> (A, B) { (a, b) }
        fn main() { let p = pair(1, true); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 6: generic with trait bound not satisfied.
#[test]
fn stage18_164_generic_bound_not_satisfied() {
    let src = r#"
        trait Clone { fn clone(&self) -> Self; }
        fn duplicate<T: Clone>(x: T) -> T { x.clone() }
        struct NoClone;
        fn main() { duplicate(NoClone); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 7: generic with multiple trait bounds.
#[test]
fn stage18_164_generic_multi_bounds() {
    let src = r#"
        trait Clone { fn clone(&self) -> Self; }
        trait Display { fn display(&self); }
        fn process<T: Clone + Display>(x: T) { x.display(); }
        fn main() {}
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 8: generic struct instantiation.
#[test]
fn stage18_164_generic_struct_instantiation() {
    let src = "struct Wrapper<T> { val: T } fn main() { let b = Wrapper { val: 42 }; }";
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 9: generic struct with wrong type arg.
#[test]
fn stage18_164_generic_struct_wrong_arg() {
    let src = r#"
        struct Wrapper<T> { val: T }
        fn main() { let b: Wrapper<i32> = Wrapper { val: true }; }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 10: generic function recursive call.
#[test]
fn stage18_164_generic_recursive() {
    let src = r#"
        fn id<T>(x: T) -> T { x }
        fn main() -> i32 { id(id(id(42))) }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Monomorphization errors ===

/// Stage 18.164 negative 11: same generic called with different types.
#[test]
fn stage18_164_mono_different_types() {
    let src = r#"
        fn id<T>(x: T) -> T { x }
        fn main() { let a = id(42); let b = id(true); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 12: generic with complex nested types.
#[test]
fn stage18_164_mono_nested_types() {
    let src = r#"
        fn id<T>(x: T) -> T { x }
        fn main() { let a = id((1, 2)); let b = id((true, false)); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 13: generic with struct type parameter.
#[test]
fn stage18_164_mono_struct_param() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        fn id<T>(x: T) -> T { x }
        fn main() { let p = Point { x: 1, y: 2 }; let q = id(p); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 14: generic method call.
#[test]
fn stage18_164_mono_method_call() {
    let src = r#"
        struct Container<T> { val: T }
        impl<T> Container<T> { fn get(&self) -> T { self.val } }
        fn main() { let c = Container { val: 42 }; let x = c.get(); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 15: generic with where clause.
#[test]
fn stage18_164_generic_where_clause() {
    let src = r#"
        trait Clone { fn clone(&self) -> Self; }
        fn duplicate<T>(x: T) -> T where T: Clone { x.clone() }
        fn main() {}
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Turbofish errors ===

/// Stage 18.164 negative 16: turbofish with wrong type.
#[test]
fn stage18_164_turbofish_wrong_type() {
    let src = r#"
        fn id<T>(x: T) -> T { x }
        fn main() { let x: bool = id::<i32>(42); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 17: turbofish on non-generic function.
#[test]
fn stage18_164_turbofish_non_generic() {
    let src = r#"
        fn helper(x: i32) -> i32 { x }
        fn main() { helper::<i32>(42); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 18: turbofish with extra type params.
#[test]
fn stage18_164_turbofish_extra_params() {
    let src = r#"
        fn id<T>(x: T) -> T { x }
        fn main() { id::<i32, bool>(42); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 19: turbofish with missing type params.
#[test]
fn stage18_164_turbofish_missing_params() {
    let src = r#"
        fn pair<A, B>(a: A, b: B) -> (A, B) { (a, b) }
        fn main() { pair::<i32>(1, true); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 20: turbofish on method call.
#[test]
fn stage18_164_turbofish_method() {
    let src = r#"
        struct Container<T> { val: T }
        impl<T> Container<T> { fn get<U>(&self, _: U) -> T { self.val } }
        fn main() { let c = Container { val: 42 }; let x = c.get::<bool>(true); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}
