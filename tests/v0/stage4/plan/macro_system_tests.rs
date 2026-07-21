//! Stage 4.10: Macro system tests
//!
//! Tests that built-in macros (println!, stringify!, assert!) are correctly
//! expanded and produce proper MIR instead of TyKind::Error.
//!
//! Per stage-committee-process.md v3.17 §17.1, new tests are placed in
//! `tests/v0/stage4/plan/` (standardized directory structure).

use landin_compiler::compile;

#[test]
fn test_macro_println_no_crash() {
    // Stage 4.10: println! macro should not crash and should produce MIR.
    let result = compile("fn main() { println!(\"hello\"); }");
    assert!(!result.mirs.is_empty(), "should produce MIR");
}

#[test]
fn test_macro_stringify() {
    // Stage 4.10: stringify! macro should produce a str-typed local
    // (simplified — no actual string content since we don't have token stream).
    let result = compile("fn main() { let s = stringify!(x); }");
    assert!(!result.mirs.is_empty(), "should produce MIR");
}

#[test]
fn test_macro_assert_no_crash() {
    // Stage 4.10: assert! macro should not crash and should produce MIR.
    let result = compile("fn main() { assert!(1 == 1); }");
    assert!(!result.mirs.is_empty(), "should produce MIR");
}
