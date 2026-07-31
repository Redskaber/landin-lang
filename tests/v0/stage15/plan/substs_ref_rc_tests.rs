//! Stage 15.10 — SubstsRef Rc<[Ty]> interning tests.
//!
//! These tests verify the `SubstsRef` type change from `Vec<Ty>` to
//! `Rc<[Ty]>` (Stage 15.10). They use `compile()` to run the full pipeline
//! with real HIR, verifying that:
//! 1. Struct construction with empty substs works
//! 2. Enum construction with empty substs works
//! 3. Closure construction with capture substs works
//! 4. Method calls on Adt-typed locals work (writeback resolves substs)
//! 5. Nested struct access works (Adt substs propagate)
//!
//! Per §29.1.3 (Design-Impl-Test coverage): integration tests verify the
//! Rc<[Ty]> type change works correctly with real MIR produced by the
//! full pipeline.

#![cfg(test)]

use landin_compiler::compile;

/// Stage 15.10 test 1: struct construction with empty substs.
///
/// Verifies `TyKind::Adt(def_id, Vec::new().into())` works — the empty
/// Rc<[Ty]> is correctly constructed and consumed by codegen.
#[test]
fn stage15_10_struct_empty_substs() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        fn main() -> i32 {
            let p = Point { x: 1, y: 2 };
            p.x + p.y
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "struct construction must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.10 test 2: enum construction with empty substs.
#[test]
fn stage15_10_enum_empty_substs() {
    let src = r#"
        enum Color { Red, Green, Blue }
        fn main() -> i32 {
            let c = Color::Red;
            0
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "enum construction must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.10 test 3: closure construction with capture substs.
///
/// Verifies `TyKind::Closure(def_id, capture_tys.into())` works — the
/// Rc<[Ty]> is correctly constructed from the capture types and consumed
/// by codegen + writeback.
#[test]
fn stage15_10_closure_with_captures() {
    let src = r#"
        fn main() -> i32 {
            let x = 42;
            let f = || x;
            f()
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "closure with captures must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.10 test 4: method call on Adt-typed local (writeback).
///
/// Verifies the writeback correctly handles Rc<[Ty]> substs — the
/// closure writeback rebuilds the Vec, mutates, and converts back.
#[test]
fn stage15_10_method_call_on_adt() {
    let src = r#"
        struct Counter { v: i32 }
        impl Counter {
            fn new(v: i32) -> Counter { Counter { v: v } }
            fn get(self) -> i32 { self.v }
        }
        fn main() -> i32 {
            let c = Counter::new(42);
            c.get()
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "method call on Adt must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.10 test 5: nested struct access (Adt substs propagate).
#[test]
fn stage15_10_nested_struct_access() {
    let src = r#"
        struct Inner { v: i32 }
        struct Outer { inner: Inner }
        impl Outer {
            fn get_inner(self) -> Inner { self.inner }
        }
        impl Inner {
            fn get(self) -> i32 { self.v }
        }
        fn main() -> i32 {
            let o = Outer { inner: Inner { v: 42 } };
            o.get_inner().get()
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "nested struct access must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.10 test 6: closure capturing struct (Rc<[Ty]> substs with Adt).
///
/// Verifies the closure writeback correctly handles Rc<[Ty]> substs
/// when the captured type is an Adt (not a primitive).
#[test]
fn stage15_10_closure_capturing_struct() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        fn main() -> i32 {
            let p = Point { x: 1, y: 2 };
            let f = || p.x;
            f()
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "closure capturing struct must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.10 test 7: multiple closures with different capture types.
#[test]
fn stage15_10_multiple_closures() {
    let src = r#"
        fn main() -> i32 {
            let x = 10;
            let y = 20;
            let f = || x;
            let g = || y;
            f() + g()
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "multiple closures must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}
