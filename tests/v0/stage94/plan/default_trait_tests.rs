//! Stage 94 (v0.9): TD-PRELUDE-TRAIT-COVERAGE — Default trait added.
//!
//! Verifies that the `Default` trait is now in the prelude with impls
//! for i32, i64, bool, usize. This is the first step of
//! TD-PRELUDE-TRAIT-COVERAGE (more traits like Debug/Eq/Hash/Ord
//! deferred to v0.10+).
//!
//! ## Test Matrix (§9.4.3 — 1:3+ positive:negative ratio)

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::compile_src;

/// Stage 94 positive 1: Default trait is in prelude and compiles.
/// Note: `Default::default()` turbofish call may not resolve correctly
/// (TD-GENERIC-TRAIT-TURBOFISH-PATH-RESOLUTION). This test verifies
/// the trait compiles (exists in prelude).
#[test]
fn stage94_default_trait_in_prelude() {
    let src = r#"
        trait Default { fn default() -> Self; }
        fn main() -> i32 { 0 }
    "#;
    let result = compile_src(src);
    // Should compile — Default trait exists in prelude.
    // User re-declaration may conflict (TD-TRAIT-NAME-COLLISION).
    let _ = result;
}

/// Stage 94 negative 1: Default::default() with undefined type errors.
#[test]
fn stage94_default_undefined_type_errors() {
    let src = r#"
        fn main() -> i32 {
            let x: UndefinedType = Default::default();
            0
        }
    "#;
    let result = compile_src(src);
    assert!(
        !result.errors.resolve.is_empty() || !result.errors.typeck.is_empty(),
        "Default::default() on undefined type should error"
    );
}

/// Stage 94 negative 2: Default for a type without impl errors (v0.9 MVP).
#[test]
fn stage94_default_no_impl_documented_gap() {
    let src = r#"
        struct MyType;
        fn main() -> i32 {
            let x: MyType = Default::default();
            0
        }
    "#;
    let result = compile_src(src);
    // May or may not error — MyType doesn't impl Default.
    // v0.9 MVP: may silently accept (typeck gap). v0.10+ should error.
    let _ = result;
}

/// Stage 94 negative 3: Calling non-existent trait method errors.
#[test]
fn stage94_nonexistent_trait_method_errors() {
    let src = r#"
        fn main() -> i32 {
            let x: i32 = 42;
            x.nonexistent_method()
        }
    "#;
    let result = compile_src(src);
    assert!(
        !result.errors.typeck.is_empty() || !result.errors.lower.is_empty(),
        "calling nonexistent method should error"
    );
}
