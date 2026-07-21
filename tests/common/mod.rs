//! Shared test helpers for all Landin test suites.
//!
//! Per stage-committee-process.md v3.17 §17.1, shared test utilities live in
//! `tests/common/mod.rs`. Individual test files can use them via:
//!
//! ```ignore
//! mod common;
//! use common::compile_src;
//! ```

use landin_compiler::{compile, CompileResult};

/// Compile a Landin source string and return the CompileResult.
pub fn compile_src(src: &str) -> CompileResult {
    compile(src)
}

/// Compile and return the result without panicking on errors.
/// Useful for negative tests that expect errors.
pub fn compile_silent(src: &str) -> CompileResult {
    compile(src)
}

/// Check if a compiled result has any errors.
pub fn has_errors(result: &CompileResult) -> bool {
    result.has_errors()
}

/// Count the total number of errors in a compiled result.
pub fn error_count(result: &CompileResult) -> usize {
    result.errors.total_count()
}
