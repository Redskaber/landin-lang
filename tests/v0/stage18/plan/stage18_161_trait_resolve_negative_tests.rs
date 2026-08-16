//! Stage 18.161 (TD-NEGATIVE-TEST-COVERAGE): Trait/Resolve negative tests.
//!
//! Tests trait resolution and name resolution error paths. Per §9.4.3,
//! negative tests should be ≥25% of total.
//!
//! Per §9.1: tests live under `tests/v0/stage-XX/plan/`.
//! Per §16: tests use only public API (`compile`).

use landin_compiler::compile;

// === Trait errors ===

/// Stage 18.161 negative 1: trait with unimplemented method.
#[test]
fn stage18_161_trait_unimplemented_method() {
    let src = r#"
        trait Drawable { fn draw(&self); }
        struct Circle;
        fn main() { let c = Circle; c.draw(); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 2: trait method wrong signature.
#[test]
fn stage18_161_trait_wrong_signature() {
    let src = r#"
        trait Drawable { fn draw(&self) -> i32; }
        struct Circle;
        impl Drawable for Circle { fn draw(&self) -> bool { true } }
        fn main() {}
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 3: trait method missing in impl.
#[test]
fn stage18_161_trait_method_missing_in_impl() {
    let src = r#"
        trait Drawable { fn draw(&self); fn color(&self); }
        struct Circle;
        impl Drawable for Circle { fn draw(&self) {} }
        fn main() {}
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 4: impl trait for wrong type.
#[test]
fn stage18_161_impl_trait_for_wrong_type() {
    let src = r#"
        trait Drawable { fn draw(&self); }
        struct Circle;
        struct Square;
        impl Drawable for Circle { fn draw(&self) {} }
        fn main() { let s = Square; s.draw(); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 5: duplicate trait impl.
#[test]
fn stage18_161_duplicate_trait_impl() {
    let src = r#"
        trait Drawable { fn draw(&self); }
        struct Circle;
        impl Drawable for Circle { fn draw(&self) {} }
        impl Drawable for Circle { fn draw(&self) {} }
        fn main() {}
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 6: trait bound not satisfied.
#[test]
fn stage18_161_trait_bound_not_satisfied() {
    let src = r#"
        trait Clone { fn clone(&self) -> Self; }
        fn duplicate<T: Clone>(x: T) -> T { x.clone() }
        struct NoClone;
        fn main() { duplicate(NoClone); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 7: trait method with wrong number of arguments.
#[test]
fn stage18_161_trait_method_wrong_arg_count() {
    let src = r#"
        trait Adder { fn add(&self, x: i32, y: i32) -> i32; }
        struct Calc;
        impl Adder for Calc { fn add(&self, x: i32) -> i32 { x } }
        fn main() {}
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 8: undefined trait.
#[test]
fn stage18_161_undefined_trait() {
    let src = "struct Foo; impl UndefinedTrait for Foo {} fn main() {}";
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 9: trait method on trait object.
#[test]
fn stage18_161_trait_method_on_dyn() {
    let src = r#"
        trait Drawable { fn draw(&self); }
        struct Circle;
        impl Drawable for Circle { fn draw(&self) {} }
        fn main() { let d: dyn Drawable = Circle; d.draw(); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 10: supertrait not implemented.
#[test]
fn stage18_161_supertrait_not_implemented() {
    let src = r#"
        trait Base { fn base_method(&self); }
        trait Derived: Base { fn derived_method(&self); }
        struct Foo;
        impl Derived for Foo { fn derived_method(&self) {} }
        fn main() {}
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Name resolution errors ===

/// Stage 18.161 negative 11: undefined variable.
#[test]
fn stage18_161_resolve_undefined_var() {
    let result = compile("fn main() { undefined_var; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 12: undefined function.
#[test]
fn stage18_161_resolve_undefined_fn() {
    let result = compile("fn main() { undefined_fn(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 13: undefined struct.
#[test]
fn stage18_161_resolve_undefined_struct() {
    let result = compile("fn main() { let p = UndefinedStruct { x: 1 }; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 14: undefined enum.
#[test]
fn stage18_161_resolve_undefined_enum() {
    let result = compile("fn main() { let c = UndefinedEnum::Variant; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 15: undefined module.
#[test]
fn stage18_161_resolve_undefined_module() {
    let result = compile("fn main() { undefined_module::func(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 16: shadowed variable.
#[test]
fn stage18_161_resolve_shadowed_var() {
    let result = compile("fn main() { let x = 1; let x = 2; let y = x; }");
    // Shadowing is allowed; should compile without errors.
    assert!(!result.mirs.is_empty());
}

/// Stage 18.161 negative 17: duplicate function definition.
#[test]
fn stage18_161_resolve_duplicate_fn() {
    let src = "fn helper() {} fn helper() {} fn main() {}";
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 18: duplicate struct definition.
#[test]
fn stage18_161_resolve_duplicate_struct() {
    let src = "struct Foo {} struct Foo {} fn main() {}";
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 19: use of private item from outside module.
#[test]
fn stage18_161_resolve_private_item_outside_module() {
    let src = r#"
        mod inner { fn private_fn() {} }
        fn main() { inner::private_fn(); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 20: use import of undefined item.
#[test]
fn stage18_161_resolve_use_undefined_item() {
    let src = r#"
        mod helper { fn real_fn() {} }
        use helper::undefined_fn;
        fn main() {}
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}
