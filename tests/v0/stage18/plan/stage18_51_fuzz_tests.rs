//! Stage 18.51 — Fuzz / robustness tests.
//!
//! Tests that verify the compiler doesn't crash (panic) when given
//! malformed or unusual input. These are "blast tests" — random-like
//! inputs that stress the parser, resolver, and type checker.
//!
//! Per §9.4.3: 2 positive + 6 negative tests (1:3 ratio).

#![cfg(all(test, feature = "llvm-backend"))]

use landin_compiler::compile;

// === Positive: Malformed input handled gracefully (no panic) ===

/// Stage 18.51 positive 1: Empty source doesn't crash.
#[test]
fn stage18_51_empty_source_no_crash() {
    let result = compile("");
    // Should produce errors, not panic.
    let _ = result;
}

/// Stage 18.51 positive 2: Single character source doesn't crash.
#[test]
fn stage18_51_single_char_no_crash() {
    let result = compile("x");
    let _ = result;
}

// === Negative: Various malformed inputs ===

/// Stage 18.51 negative 1: Unbalanced braces don't crash.
#[test]
fn stage18_51_unbalanced_braces_no_crash() {
    let result = compile("fn main() { {{{{ }");
    // Should produce parse errors, not panic.
    let _ = result;
}

/// Stage 18.51 negative 2: Unterminated string doesn't crash.
#[test]
fn stage18_51_unterminated_string_no_crash() {
    let result = compile("fn main() { let x = \"hello; }");
    let _ = result;
}

/// Stage 18.51 negative 3: Deeply nested macros don't crash.
#[test]
fn stage18_51_deeply_nested_macros_no_crash() {
    let mut src = String::from("macro_rules! m0 { () => { 42 } } ");
    for i in 1..50 {
        src.push_str(&format!(
            "macro_rules! m{} {{ () => {{ m{}!() }} }} ",
            i,
            i - 1
        ));
    }
    src.push_str("fn main() { m49!() }");
    let result = compile(&src);
    let _ = result;
}

/// Stage 18.51 negative 4: Invalid macro pattern doesn't crash.
#[test]
fn stage18_51_invalid_macro_pattern_no_crash() {
    let result = compile("macro_rules! m { $ } fn main() { m!() }");
    let _ = result;
}

/// Stage 18.51 negative 5: Recursive macro (infinite) doesn't hang.
#[test]
fn stage18_51_recursive_macro_no_hang() {
    let result = compile("macro_rules! m { () => { m!() } } fn main() { m!() }");
    // Should terminate within MAX_EXPANSION_ROUNDS, not hang.
    let _ = result;
}

/// Stage 18.51 negative 6: Garbage tokens don't crash.
#[test]
fn stage18_51_garbage_tokens_no_crash() {
    let result = compile("@#$%^&* fn main() { } ~`|\\");
    let _ = result;
}
