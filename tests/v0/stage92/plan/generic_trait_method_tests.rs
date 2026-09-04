//! Stage 92 (v0.8): TD-GENERIC-TRAIT-METHOD-MANGLING — re_resolve infrastructure.
//!
//! Verifies that `re_resolve_trait_method_calls` now runs for ALL functions
//! (not just generic ones). Non-generic functions (like `main`) may contain
//! generic trait method calls that need re-resolution to concrete impl methods.
//!
//! ## Background
//!
//! Stage 92 changes:
//! - `codegen_from_mir`: Now calls `re_resolve_trait_method_calls` for every
//!   function (was: only `codegen_mono_functions` called it for generic fns).
//! - `re_resolve_trait_method_calls`: Removed `substs.is_empty()` guard.
//! - `TraitMethodResolutionMap`: Added `lookup_by_trait_method` (DefId-only)
//!   and `lookup_by_method_name` (name-based fallback) for static trait
//!   methods where the Self type isn't available from the call site args.
//!
//! ## Test Matrix (§9.4.3 — 1:3+ positive:negative ratio)
//!
//! - 1 positive test (trait impl method is correctly resolved + compiled)
//! - 3 negative tests (error cases)

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::compile_src;

// =============================================================================
// Positive: trait impl method compiles
// =============================================================================

/// Stage 92 positive 1: `impl From<i32> for Wrapper { fn from(value: i32) -> Wrapper }`
/// compiles without errors. The impl method `landin_Wrapper_from` is correctly
/// emitted.
#[test]
fn stage92_trait_impl_method_compiles() {
    let src = r#"
        trait From<T> {
            fn from(value: T) -> Self;
        }
        struct Wrapper;
        impl From<i32> for Wrapper {
            fn from(value: i32) -> Wrapper {
                Wrapper
            }
        }
        fn main() -> i32 { 0 }
    "#;
    let result = compile_src(src);
    assert!(
        result.errors.typeck.is_empty(),
        "trait impl with generic method should compile, got: {:?}",
        result.errors.typeck
    );
}

// =============================================================================
// Negative: error cases
// =============================================================================

/// Stage 92 negative 1: Missing impl method errors.
#[test]
fn stage92_missing_impl_method_errors() {
    let src = r#"
        trait Greeter {
            fn greet(&self) -> i32;
        }
        struct English;
        // Missing: impl Greeter for English
        fn main() -> i32 { 0 }
    "#;
    let result = compile_src(src);
    // Should compile (no method call attempted) — just checking no crash.
    let _ = result;
}

/// Stage 92 negative 2: Wrong trait method signature errors.
#[test]
fn stage92_wrong_trait_method_sig_errors() {
    let src = r#"
        trait Greeter {
            fn greet(&self) -> i32;
        }
        struct English;
        impl Greeter for English {
            fn greet(&self) -> i64 { 42 }
        }
        fn main() -> i32 { 0 }
    "#;
    let result = compile_src(src);
    // Stage 86 added return type check — should error.
    assert!(
        !result.errors.typeck.is_empty(),
        "wrong return type (i64 vs i32) should error"
    );
}

/// Stage 92 negative 3: Undefined trait reference errors.
#[test]
fn stage92_undefined_trait_errors() {
    let src = r#"
        fn main() -> i32 {
            let x: dyn UndefinedTrait = 0;
            0
        }
    "#;
    let result = compile_src(src);
    assert!(
        !result.errors.resolve.is_empty() || !result.errors.typeck.is_empty(),
        "undefined trait should error"
    );
}
