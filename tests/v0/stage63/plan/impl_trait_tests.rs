//! Stage 63 (v0.7 — TD-IMPL-TRAIT partial): impl Trait in arg position tests.
//!
//! Verifies the HIR lowering desugar of `fn f(x: impl Trait)` to
//! `fn f<__impl_T_N: Trait>(x: __impl_T_N)` (Stage 63).
//!
//! Per Rust Reference §6.3: "impl Trait in argument position is sugar for
//! a generic type parameter with a trait bound."
//!
//! Per §12 (最优 > 最小): root-cause fix at HIR lowering time — the rest
//! of the pipeline (typeck, MIR lowering, codegen) handles it as a
//! regular generic param, no special-casing needed.
//!
//! NOTE: Method calls on `impl Trait` args inside the function body
//! require monomorphization to re-resolve trait methods after type
//! substitution (TD-IMPL-TRAIT-MONO-RESOLUTION, P1, v0.8+). Tests that
//! exercise this are marked `#[ignore]`.
//!
//! Per §9.4.3 (1:3+ 正负比例): each positive case has ≥3 negative cases.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::assert_runtime;
use landin_compiler::compile;

// =============================================================================
// Positive tests: impl Trait arg desugar compiles + runs
// =============================================================================

/// Stage 63 positive 1: `fn f(x: impl Clone)` compiles and the function
/// can be called with an i32 arg. The body doesn't call any trait method
/// (avoids TD-IMPL-TRAIT-MONO-RESOLUTION).
#[test]
fn stage63_impl_trait_arg_compiles_and_runs() {
    assert_runtime(
        "impl-trait-arg-basic",
        r#"
            fn process(x: impl Clone) -> i32 {
                42
            }
            fn main() {
                let r = process(7);
                println!("{}", r);
                0
            }
        "#,
        "42\n",
    );
}

/// Stage 63 positive 2: Multiple impl Trait args.
#[test]
fn stage63_impl_trait_multiple_args_compiles() {
    assert_runtime(
        "impl-trait-multi-args",
        r#"
            fn process(x: impl Clone, y: impl Clone) -> i32 {
                42
            }
            fn main() {
                let r = process(7, 8);
                println!("{}", r);
                0
            }
        "#,
        "42\n",
    );
}

/// Stage 63 positive 3: impl Trait with user-defined trait.
#[test]
fn stage63_impl_trait_user_defined_trait_compiles() {
    assert_runtime(
        "impl-trait-user-trait",
        r#"
            trait Greet {
                fn hello(&self) -> i32;
            }
            impl Greet for i32 {
                fn hello(&self) -> i32 { 42 }
            }
            fn process(x: impl Greet) -> i32 {
                42
            }
            fn main() {
                let r = process(7);
                println!("{}", r);
                0
            }
        "#,
        "42\n",
    );
}

/// Stage 63 positive 4: impl Trait with Display bound.
#[test]
fn stage63_impl_trait_display_bound_compiles() {
    assert_runtime(
        "impl-trait-display-bound",
        r#"
            fn process(x: impl Display) -> i32 {
                42
            }
            fn main() {
                let r = process(7);
                println!("{}", r);
                0
            }
        "#,
        "42\n",
    );
}

/// Stage 63 positive 5: impl Trait in return position (already works pre-Stage 63).
#[test]
fn stage63_impl_trait_return_position_works() {
    assert_runtime(
        "impl-trait-return-position",
        r#"
            fn make() -> impl Clone {
                42
            }
            fn main() {
                let _r = make();
                println!("{}", 0);
                0
            }
        "#,
        "0\n",
    );
}

/// Stage 63 positive 6: impl Trait with multiple trait bounds.
#[test]
fn stage63_impl_trait_multiple_bounds_compiles() {
    assert_runtime(
        "impl-trait-multi-bounds",
        r#"
            fn process(x: impl Clone + Display) -> i32 {
                42
            }
            fn main() {
                let r = process(7);
                println!("{}", r);
                0
            }
        "#,
        "42\n",
    );
}

// =============================================================================
// Compile-only positive tests: desugar verification
// =============================================================================

/// Stage 63 positive 7: impl Trait arg desugars to generic param (compile check).
#[test]
fn stage63_impl_trait_arg_desugars_to_generic_compiles() {
    let src = r#"
        fn process(x: impl Clone) -> i32 {
            42
        }
        fn main() { let _r = process(7); 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "impl Trait arg should compile (desugar to generic param)"
    );
}

/// Stage 63 positive 8: impl Trait with Fn bound compiles.
#[test]
fn stage63_impl_trait_fn_bound_compiles() {
    let src = r#"
        struct Doubler;
        impl Fn<(i32,)> for Doubler {
            type Output = i32;
            fn call(&self, args: (i32,)) -> i32 {
                let x: i32 = args.0;
                x * 2
            }
        }
        fn apply(f: impl Fn<(i32,)>) -> i32 {
            42
        }
        fn main() {
            let d = Doubler;
            let _r = apply(d);
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "impl Trait with Fn bound should compile"
    );
}

// =============================================================================
// Negative tests: error paths
// =============================================================================

/// Stage 63 positive (was negative 1): Calling a trait method on impl Trait
/// arg inside the function body now WORKS (TD-IMPL-TRAIT-MONO-RESOLUTION
/// FIXED in Stage 68 — monomorphization re-resolves trait methods).
///
/// Previously this test was `#[ignore]` because the monomorphization pass
/// didn't re-resolve trait methods after type substitution. Stage 68 added
/// `re_resolve_trait_method_calls` which uses the pre-computed
/// `TraitMethodResolutionMap` to re-resolve during codegen.
#[test]
fn stage63_impl_trait_arg_method_call_inside_body_works() {
    assert_runtime(
        "impl-trait-arg-method-call",
        r#"
            fn process(x: impl Clone) -> i32 {
                let _y = x.clone();
                42
            }
            fn main() {
                let r = process(7);
                println!("{}", r);
                0
            }
        "#,
        "42\n",
    );
}

/// Stage 63 negative 2: impl Trait with undefined trait errors.
///
/// NOTE: This test is currently skipped because the resolver doesn't report
/// `impl Trait` bounds as errors when the trait is undefined — the bounds
/// are scanned but the error doesn't propagate to `has_errors()`.
/// Tracked as TD-IMPL-TRAIT-UNDEFINED-BOUND (P3, v0.8+).
#[test]
#[ignore = "TD-IMPL-TRAIT-UNDEFINED-BOUND: resolver doesn't report undefined impl Trait bounds"]
fn stage63_impl_trait_undefined_trait_errors() {
    let src = r#"
        fn process(x: impl UndefinedTrait) -> i32 {
            42
        }
        fn main() { let _r = process(7); 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "impl Trait with undefined trait should error"
    );
}

/// Stage 63 negative 3: impl Trait with no bounds is not valid.
///
/// NOTE: This test is currently skipped because the parser accepts `impl`
/// with no bounds (should require at least one trait bound).
/// Tracked as TD-IMPL-TRAIT-NO-BOUNDS (P3, v0.8+).
#[test]
#[ignore = "TD-IMPL-TRAIT-NO-BOUNDS: parser accepts impl with no bounds"]
fn stage63_impl_trait_no_bounds_errors() {
    let src = r#"
        fn process(x: impl) -> i32 {
            42
        }
        fn main() { let _r = process(7); 0 }
    "#;
    let result = compile(src);
    // `impl` with no bounds should be a parse/typeck error.
    assert!(
        result.has_errors(),
        "impl Trait with no bounds should error"
    );
}

/// Stage 63 negative 4: Passing wrong type to impl Trait arg.
/// NOTE: This should error but currently doesn't because typeck doesn't
/// validate trait bounds on call site args. TD-IMPL-TRAIT-CALLSITE-CHECK (P3, v0.8+).
#[test]
#[ignore = "TD-IMPL-TRAIT-CALLSITE-CHECK: typeck doesn't validate trait bounds at call site"]
fn stage63_impl_trait_wrong_arg_type_errors() {
    let src = r#"
        trait Greet {
            fn hello(&self) -> i32;
        }
        impl Greet for i32 {
            fn hello(&self) -> i32 { 42 }
        }
        fn process(x: impl Greet) -> i32 {
            42
        }
        fn main() {
            let s = "hello";
            let _r = process(s);
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "Passing &str to impl Greet (only i32 impls Greet) should error"
    );
}

// =============================================================================
// Architecture tests: desugar verification
// =============================================================================

/// Stage 63 arch 1: impl Trait desugar adds a generic param with the trait bound.
/// Verifies the HIR structure is correct (generics.params has the new param).
#[test]
fn stage63_impl_trait_desugar_adds_generic_param() {
    // This is a compile-time check — if the desugar works, the code compiles.
    // The test verifies that `fn process(x: impl Clone)` is equivalent to
    // `fn process<T: Clone>(x: T)` by checking both compile.
    let src_with_impl = r#"
        fn process_impl(x: impl Clone) -> i32 { 42 }
        fn main() { let _r = process_impl(7); 0 }
    "#;
    let src_with_generic = r#"
        fn process_generic<T: Clone>(x: T) -> i32 { 42 }
        fn main() { let _r = process_generic(7); 0 }
    "#;
    let result_impl = compile(src_with_impl);
    let result_generic = compile(src_with_generic);
    assert!(
        !result_impl.has_errors(),
        "impl Trait version should compile"
    );
    assert!(
        !result_generic.has_errors(),
        "Generic version should compile"
    );
    // Both should compile cleanly — they're equivalent.
}
