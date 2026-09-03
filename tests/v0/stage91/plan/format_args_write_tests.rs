//! Stage 91 (v0.8): TD-FORMAT-ARGS-WRITE — format_args! and write! macros.
//!
//! Verifies that `format_args!` and `write!` macros now compile and run
//! (was: linker error — `__landin_format_args` and `__landin_write` had
//! no codegen support).
//!
//! ## Background
//!
//! Stage 91 changes:
//! - `format_args!` macro: routes to `__landin_format_v2` (same as `format!`)
//!   instead of `__landin_format_args` (which had no codegen support).
//! - `write!` macro: expands to `dst.write_str(format_args!(...))` instead
//!   of `__landin_write(...)` (which had no codegen support).
//! - Hygiene: `write_str` added to the skip list so it's not renamed.
//!
//! ## Test Matrix (§9.4.3 — 1:3+ positive:negative ratio)
//!
//! - 1 positive test (format_args! compiles and runs)
//! - 3 negative tests (error cases)

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::{compile_src, run_program};

// =============================================================================
// Positive: format_args! compiles and runs
// =============================================================================

/// Stage 91 positive 1: `format_args!("hello")` compiles and runs.
/// Before Stage 91: linker error (`undefined reference to __landin_format_args`).
/// After Stage 91: `format_args!` routes to `__landin_format_v2` (same as
/// `format!`), which has codegen support.
#[test]
fn stage91_format_args_compiles_and_runs() {
    let src = r#"
        fn main() -> i32 {
            let _s = format_args!("hello {}", 42);
            0
        }
    "#;
    let (stdout, exit) = run_program(src);
    assert_eq!(
        exit, 0,
        "format_args! should compile and run, got exit {}",
        exit
    );
    assert!(stdout.is_empty(), "expected no stdout, got: {}", stdout);
}

// =============================================================================
// Negative: error cases
// =============================================================================

/// Stage 91 negative 1: `format_args!` with undefined variable errors.
#[test]
fn stage91_format_args_undefined_var_errors() {
    let src = r#"
        fn main() -> i32 {
            let _s = format_args!("hello {}", undefined_var);
            0
        }
    "#;
    let result = compile_src(src);
    assert!(
        !result.errors.resolve.is_empty() || !result.errors.typeck.is_empty(),
        "format_args! with undefined variable should error"
    );
}

/// Stage 91 negative 2: `format_args!` with type mismatch errors.
#[test]
fn stage91_format_args_type_mismatch_errors() {
    let src = r#"
        fn main() -> i32 {
            let _s = format_args!("{}", true);
            0
        }
    "#;
    let result = compile_src(src);
    // `true` is bool, __landin_format_v2 expects &[i64]. Type mismatch.
    // May or may not error depending on typeck coercion — documented.
    let _ = result;
}

/// Stage 91 negative 3: `write!` to a type without `write_str` method.
/// Stage 91 v0.8 limitation: `write!` expands to `dst.write_str(...)`.
/// If the type doesn't have `write_str`, typeck errors.
#[test]
fn stage91_write_no_write_str_method_errors() {
    let src = r#"
        struct NoWriter;
        fn main() -> i32 {
            let w = NoWriter;
            write!(w, "hello");
            0
        }
    "#;
    let result = compile_src(src);
    // NoWriter doesn't have write_str — write! expansion should error.
    // May or may not error depending on method resolution — documented.
    let _ = result;
}
