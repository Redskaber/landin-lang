//! Stage 66 (v0.7): TD-IMPL-TRAIT-NO-BOUNDS + TD-IMPL-TRAIT-UNDEFINED-BOUND tests.
//!
//! Verifies two parser/resolver fixes:
//! 1. `impl` with no bounds errors (parser fix)
//! 2. `impl UndefinedTrait` errors (resolver/scanner fix)
//!
//! Per §1.0 原則 4 (报错 > 静默): previously both cases silently compiled.
//! Per §12 (最优 > 最小): root-cause fixes at parser and scanner levels.

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::compile;

// =============================================================================
// TD-IMPL-TRAIT-NO-BOUNDS: parser rejects `impl` with no bounds
// =============================================================================

/// Stage 66 positive 1: `impl` with no bounds errors.
#[test]
fn stage66_impl_no_bounds_errors() {
    let src = r#"
        fn process(x: impl) -> i32 {
            42
        }
        fn main() { let _r = process(7); 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "`impl` with no bounds should produce a parse error"
    );
}

/// Stage 66 positive 2: `impl Clone` with valid bounds compiles.
#[test]
fn stage66_impl_with_bounds_compiles() {
    let src = r#"
        fn process(x: impl Clone) -> i32 {
            42
        }
        fn main() { let _r = process(7); 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "`impl Clone` with valid bounds should compile"
    );
}

/// Stage 66 positive 3: `impl Clone + Display` with multiple bounds compiles.
#[test]
fn stage66_impl_multiple_bounds_compiles() {
    let src = r#"
        fn process(x: impl Clone + Display) -> i32 {
            42
        }
        fn main() { let _r = process(7); 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "`impl Clone + Display` should compile"
    );
}

// =============================================================================
// TD-IMPL-TRAIT-UNDEFINED-BOUND: resolver reports undefined trait bounds
// =============================================================================

/// Stage 66 positive 4: `impl UndefinedTrait` errors.
#[test]
fn stage66_impl_undefined_trait_errors() {
    let src = r#"
        fn process(x: impl UndefinedTrait) -> i32 {
            42
        }
        fn main() { let _r = process(7); 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "`impl UndefinedTrait` should produce a resolve error"
    );
}

/// Stage 66 positive 5: Explicit generic bound with undefined trait errors.
#[test]
fn stage66_explicit_generic_undefined_trait_errors() {
    let src = r#"
        fn process<T: UndefinedTrait>(x: T) -> i32 {
            42
        }
        fn main() { let _r = process(7); 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "`<T: UndefinedTrait>` should produce a resolve error"
    );
}

/// Stage 66 positive 6: Where clause with undefined trait errors.
#[test]
fn stage66_where_clause_undefined_trait_errors() {
    let src = r#"
        fn process<T>(x: T) -> i32 where T: UndefinedTrait {
            42
        }
        fn main() { let _r = process(7); 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "where clause with undefined trait should produce a resolve error"
    );
}

/// Stage 66 positive 7: Valid `impl Clone` with defined trait compiles.
#[test]
fn stage66_impl_defined_trait_compiles() {
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
        fn main() { let _r = process(7); 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "`impl Greet` with defined trait should compile"
    );
}

// =============================================================================
// Negative tests: valid code should still work
// =============================================================================

/// Stage 66 negative 1: Normal function without impl Trait compiles.
#[test]
fn stage66_normal_function_compiles() {
    let src = r#"
        fn process(x: i32) -> i32 {
            42
        }
        fn main() { let _r = process(7); 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Normal function without impl Trait should compile"
    );
}

/// Stage 66 negative 2: Generic function with valid trait bound compiles.
#[test]
fn stage66_generic_with_valid_bound_compiles() {
    let src = r#"
        fn process<T: Clone>(x: T) -> i32 {
            42
        }
        fn main() { let _r = process(7); 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Generic function with valid Clone bound should compile"
    );
}
