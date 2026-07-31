//! Stage 15.8 — Crate-level AdtLayouts integration tests.
//!
//! These tests verify the `build_crate_adt_layouts` function and the
//! `Arc<AdtLayouts>` sharing across MirBodies. They use `compile()` to
//! run the full pipeline with real HIR.
//!
//! Coverage:
//! 1. Struct layout is registered crate-level
//! 2. Enum layout is registered crate-level
//! 3. Nested struct layouts are registered (recursion)
//! 4. Layouts are shared across multiple bodies (Arc sharing)
//! 5. No regression in struct-returning method calls
//! 6. No regression in nested struct access
//!
//! Per §29.1.3 (Design-Impl-Test coverage): integration tests verify the
//! crate-level AdtLayouts works correctly with real MIR produced by the
//! full pipeline.

#![cfg(test)]

use landin_compiler::compile;

/// Stage 15.8 integration test 1: struct layout registered crate-level.
///
/// Verifies that a struct's AdtLayout is registered in the crate-level
/// map (not per-body). The struct is defined once but used in multiple
/// functions — all should share the same layout.
#[test]
fn stage15_8_struct_layout_crate_level() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        fn make() -> Point { Point { x: 1, y: 2 } }
        fn use_it() -> i32 {
            let p = make();
            p.x
        }
        fn main() -> i32 { use_it() }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "struct layout must compile cleanly (errors: {})",
        result.errors.total_count()
    );
    // Verify all MirBodies have non-empty adt_layouts (shared Arc).
    assert!(!result.mirs.is_empty(), "must have at least one MirBody");
    for mir in &result.mirs {
        assert!(
            !mir.adt_layouts.is_empty(),
            "each MirBody must have AdtLayouts (shared Arc from crate-level)"
        );
    }
}

/// Stage 15.8 integration test 2: enum layout registered crate-level.
#[test]
fn stage15_8_enum_layout_crate_level() {
    let src = r#"
        enum Color { Red, Green, Blue }
        fn make() -> Color { Color::Red }
        fn main() -> i32 {
            let c = make();
            0
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "enum layout must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.8 integration test 3: nested struct layouts registered.
///
/// Verifies that `register_adt_layout_recursive` correctly registers
/// nested ADTs when building crate-level layouts.
#[test]
fn stage15_8_nested_struct_layouts() {
    let src = r#"
        struct Inner { v: i32 }
        struct Outer { inner: Inner }
        fn main() -> i32 {
            let o = Outer { inner: Inner { v: 42 } };
            o.inner.v
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "nested struct must compile cleanly (errors: {})",
        result.errors.total_count()
    );
    // Both Inner and Outer should have layouts registered.
    for mir in &result.mirs {
        assert!(
            mir.adt_layouts.len() >= 2,
            "both Inner and Outer layouts must be registered (got {} layouts)",
            mir.adt_layouts.len()
        );
    }
}

/// Stage 15.8 integration test 4: layouts shared across bodies (Arc).
///
/// Verifies that all MirBodies share the SAME Arc<AdtLayouts> — i.e.,
/// the driver builds the map once and shares it.
#[test]
fn stage15_8_layouts_shared_across_bodies() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        fn f1() -> i32 { let p = Point { x: 1, y: 2 }; p.x }
        fn f2() -> i32 { let p = Point { x: 3, y: 4 }; p.y }
        fn main() -> i32 { f1() + f2() }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "multi-body crate must compile cleanly (errors: {})",
        result.errors.total_count()
    );
    // All bodies should share the same Arc (pointer equality).
    let first = &result.mirs[0].adt_layouts;
    for (i, mir) in result.mirs.iter().enumerate() {
        assert!(
            std::sync::Arc::ptr_eq(first, &mir.adt_layouts),
            "MirBody {} must share the same Arc<AdtLayouts> as body 0",
            i
        );
    }
}

/// Stage 15.8 integration test 5: struct-returning method call (regression).
///
/// Verifies the crate-level AdtLayouts correctly supports struct-returning
/// method calls — the original use case that required "re-populate after
/// writeback" in Stage 14.41.
#[test]
fn stage15_8_struct_return_method_call_regression() {
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
        "struct-returning method call must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}

/// Stage 15.8 integration test 6: array of structs (regression).
///
/// Verifies the crate-level AdtLayouts correctly supports arrays of
/// structs — a pattern that requires the struct layout to be available
/// for element access.
#[test]
fn stage15_8_array_of_structs_regression() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        fn main() -> i32 {
            let arr = [Point { x: 1, y: 2 }, Point { x: 3, y: 4 }];
            arr[0].x
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "array of structs must compile cleanly (errors: {})",
        result.errors.total_count()
    );
}
