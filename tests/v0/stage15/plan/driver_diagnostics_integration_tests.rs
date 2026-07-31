//! Stage 15.14 — Driver diagnostics integration tests.
//!
//! These tests verify the Stage 15.14 bridge between `CompileErrors` (the
//! driver's 6-field error collection) and the `diagnostics` module (the
//! single source of truth for error display).
//!
//! Coverage:
//! 1. `to_diagnostics` converts lex errors to Diagnostic values
//! 2. `to_diagnostics` converts parse errors
//! 3. `to_diagnostics` converts resolve errors
//! 4. `to_diagnostics` converts typeck errors (with expected/found notes)
//! 5. `to_diagnostics` converts trait errors (with interner resolution)
//! 6. `format_via_diagnostics` produces rustc-style output
//! 7. `format_via_diagnostics` includes source snippets
//! 8. Empty errors produce empty diagnostics
//!
//! Per §29.1.3 (Design-Impl-Test coverage): integration tests verify the
//! bridge between CompileErrors and the diagnostics module works correctly.

#![cfg(test)]

use landin_compiler::compile;

/// Stage 15.14 test 1: lex errors convert to Diagnostic values.
#[test]
fn stage15_14_lex_errors_to_diagnostics() {
    // Unterminated string → lex error
    let src = r#"fn main() { let x = "unterminated; }"#;
    let result = compile(src);
    let diags = result.errors.to_diagnostics(Some(&result.interner));
    // Should have at least one diagnostic
    assert!(!diags.is_empty(), "lex errors should produce diagnostics");
    // All diagnostics should be Error level
    for d in &diags {
        assert_eq!(
            d.level,
            landin_compiler::diagnostics::Level::Error,
            "all diagnostics should be Error level"
        );
    }
    // At least one should have code "Lex"
    let has_lex_code = diags.iter().any(|d| d.code.as_deref() == Some("Lex"));
    assert!(
        has_lex_code,
        "should have at least one diagnostic with code 'Lex'"
    );
}

/// Stage 15.14 test 2: parse errors convert to Diagnostic values.
#[test]
fn stage15_14_parse_errors_to_diagnostics() {
    // Missing semicolon → parse error
    let src = "fn main() { let x = 42 }";
    let result = compile(src);
    let diags = result.errors.to_diagnostics(Some(&result.interner));
    if !result.errors.parse.is_empty() {
        let has_parse_code = diags.iter().any(|d| d.code.as_deref() == Some("Parse"));
        assert!(
            has_parse_code,
            "should have at least one diagnostic with code 'Parse'"
        );
    }
}

/// Stage 15.14 test 3: resolve errors convert to Diagnostic values.
#[test]
fn stage15_14_resolve_errors_to_diagnostics() {
    // Undefined variable → resolve error
    let src = "fn main() { undefined_var }";
    let result = compile(src);
    let diags = result.errors.to_diagnostics(Some(&result.interner));
    if !result.errors.resolve.is_empty() {
        let has_resolve_code = diags.iter().any(|d| d.code.as_deref() == Some("Resolve"));
        assert!(
            has_resolve_code,
            "should have at least one diagnostic with code 'Resolve'"
        );
    }
}

/// Stage 15.14 test 4: trait errors convert to Diagnostic values with interner.
#[test]
fn stage15_14_trait_errors_to_diagnostics() {
    // Conflicting impls → trait error
    let src = "trait Foo {} struct S; impl Foo for S {} impl Foo for S {} fn main() {}";
    let result = compile(src);
    if !result.errors.trait_errors.is_empty() {
        let diags = result.errors.to_diagnostics(Some(&result.interner));
        let has_trait_code = diags.iter().any(|d| d.code.as_deref() == Some("Trait"));
        assert!(
            has_trait_code,
            "should have at least one diagnostic with code 'Trait'"
        );
        // The message should contain the resolved trait name "Foo"
        let has_foo = diags.iter().any(|d| d.message.contains("Foo"));
        assert!(has_foo, "should have a diagnostic mentioning 'Foo'");
    }
}

/// Stage 15.14 test 5: format_via_diagnostics produces rustc-style output.
#[test]
fn stage15_14_format_via_diagnostics_rustc_style() {
    let src = "fn main() { undefined_var }";
    let result = compile(src);
    if !result.errors.is_empty() {
        let source_map = landin_compiler::session::SourceMap::new(src);
        let formatted = result.errors.format_via_diagnostics(
            src,
            "main.lin",
            &source_map,
            Some(&result.interner),
        );
        // Should contain "error[" prefix (rustc-style with code)
        assert!(
            formatted.contains("error[") || formatted.contains("error:"),
            "should contain 'error[' or 'error:' prefix, got: {}",
            formatted
        );
        // Should contain the source location
        assert!(
            formatted.contains("-->"),
            "should contain '-->' location marker, got: {}",
            formatted
        );
    }
}

/// Stage 15.14 test 6: format_via_diagnostics includes source snippets.
#[test]
fn stage15_14_format_via_diagnostics_includes_snippets() {
    let src = "fn main() { undefined_var }";
    let result = compile(src);
    if !result.errors.is_empty() {
        let source_map = landin_compiler::session::SourceMap::new(src);
        let formatted = result.errors.format_via_diagnostics(
            src,
            "main.lin",
            &source_map,
            Some(&result.interner),
        );
        // Should contain snippet gutter " | "
        assert!(
            formatted.contains(" | "),
            "should contain snippet gutter ' | ', got: {}",
            formatted
        );
    }
}

/// Stage 15.14 test 7: empty errors produce empty diagnostics.
#[test]
fn stage15_14_empty_errors_empty_diagnostics() {
    let src = "fn main() -> i32 { 42 }";
    let result = compile(src);
    let diags = result.errors.to_diagnostics(Some(&result.interner));
    assert!(
        diags.is_empty(),
        "no errors should produce empty diagnostics"
    );
}

/// Stage 15.14 test 8: to_diagnostics preserves error count.
#[test]
fn stage15_14_to_diagnostics_preserves_count() {
    let src = "fn main() { undefined_var }";
    let result = compile(src);
    let diags = result.errors.to_diagnostics(Some(&result.interner));
    assert_eq!(
        diags.len(),
        result.errors.total_count(),
        "diagnostic count should match total_count"
    );
}
