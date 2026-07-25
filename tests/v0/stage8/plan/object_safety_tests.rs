//! Stage 8.2: Object safety verification tests.
//!
//! Per stage-committee-process.md v3.21 §13.4 + §17.1, verifies the
//! object safety module (Stage 8.2, §2.3).

use landin_compiler::driver::compile;

#[test]
fn stage8_2_object_safe_trait_compiles() {
    // A trait with &self method should compile fine (object-safe)
    let result = compile("trait Greet { fn hello(&self) -> i32; } fn main() {}");
    assert!(result.errors.is_empty(), "object-safe trait should compile");
}

#[test]
fn stage8_2_trait_with_mut_self() {
    // &mut self is also object-safe
    let result = compile("trait Update { fn set(&mut self, v: i32); } fn main() {}");
    assert!(result.errors.is_empty(), "&mut self trait should compile");
}

#[test]
fn stage8_2_trait_impl_works() {
    // Trait with &self + impl should work
    let result = compile(
        "
        trait Greet { fn hello(&self) -> i32; }
        struct Person;
        impl Greet for Person { fn hello(&self) -> i32 { 42 } }
        fn main() { let p = Person; let _x = p.hello(); }
    ",
    );
    assert!(result.errors.typeck.is_empty(), "trait impl should work");
}

#[test]
fn stage8_2_empty_trait() {
    let result = compile("trait Empty {} fn main() {}");
    assert!(result.errors.is_empty(), "empty trait should compile");
}

#[test]
fn stage8_2_multiple_object_safe_methods() {
    let result = compile(
        "
        trait Drawable {
            fn draw(&self) -> i32;
            fn area(&self) -> i32;
        }
        fn main() {}
    ",
    );
    assert!(
        result.errors.is_empty(),
        "multiple &self methods should compile"
    );
}
