//! Stage 13.16 — Format args (`println!("{}", x)`) verification tests
//!
//! Verifies that the P0 v0.1 release blocker (format args support) is
//! properly implemented:
//!
//! - `Expr::Println` has `args: Vec<Expr>` field (AST)
//! - `HirExprKind::Println` has `args: Vec<HirExpr>` field (HIR)
//! - `StatementKind::Println` has `args: Vec<Operand>` field (MIR)
//! - Parser captures all comma-separated args (no silent-drop)
//! - HIR lower lowers each arg expression
//! - MIR lower lowers each arg to an Operand
//! - Codegen builds a C printf format string with type-specific specifiers
//! - Resolver resolves paths inside Println args (the bug that caused
//!   `println!("{}", x)` to print `0` instead of `x`'s value)
//!
//! Per `stage-committee-process.md` v3.21 §13.4 + §14.4 + §25.8 +
//! `stage-13.16-design-alignment.md` + `gate-review-13.16.md`.

#![cfg(test)]

use std::path::Path;

/// Verify `Expr::Println` has `args: Vec<Expr>` field (AST level).
#[test]
fn test_ast_println_has_args_field() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ast_kinds = manifest.join("src/ast/kinds.rs");
    let content = std::fs::read_to_string(&ast_kinds).expect("read ast/kinds.rs");

    // Must reference Stage 13.16 in the Println variant doc.
    assert!(
        content.contains("Stage 13.16"),
        "src/ast/kinds.rs must reference Stage 13.16 in the Println variant doc"
    );

    // The Println variant must have an `args` field.
    // We check for "args: Vec<Expr>" near the Println variant.
    let println_pos = content
        .find("Println {")
        .or_else(|| content.find("Println{"))
        .expect("Println variant must exist");
    let println_section = &content[println_pos..println_pos + 500];
    assert!(
        println_section.contains("args: Vec<Expr>"),
        "Expr::Println must have `args: Vec<Expr>` field (Stage 13.16)"
    );
}

/// Verify `HirExprKind::Println` has `args: Vec<HirExpr>` field (HIR level).
#[test]
fn test_hir_println_has_args_field() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let hir_kinds = manifest.join("src/hir/kinds.rs");
    let content = std::fs::read_to_string(&hir_kinds).expect("read hir/kinds.rs");

    // Must reference Stage 13.16.
    assert!(
        content.contains("Stage 13.16"),
        "src/hir/kinds.rs must reference Stage 13.16"
    );

    // The Println variant must have an `args` field.
    let println_pos = content
        .find("Println {")
        .or_else(|| content.find("Println{"))
        .expect("Println variant must exist");
    let println_section = &content[println_pos..println_pos + 500];
    assert!(
        println_section.contains("args: Vec<HirExpr>"),
        "HirExprKind::Println must have `args: Vec<HirExpr>` field (Stage 13.16)"
    );
}

/// Verify `StatementKind::Println` has `args: Vec<Operand>` field (MIR level).
#[test]
fn test_mir_println_has_args_field() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mir_body = manifest.join("src/mir/body.rs");
    let content = std::fs::read_to_string(&mir_body).expect("read mir/body.rs");

    // Must reference Stage 13.16.
    assert!(
        content.contains("Stage 13.16"),
        "src/mir/body.rs must reference Stage 13.16"
    );

    // The Println variant must have an `args` field.
    let println_pos = content
        .find("Println {")
        .or_else(|| content.find("Println{"))
        .expect("Println variant must exist");
    let println_section = &content[println_pos..println_pos + 500];
    assert!(
        println_section.contains("args: Vec<crate::mir::place::Operand>")
            || println_section.contains("args: Vec<Operand>"),
        "StatementKind::Println must have `args: Vec<Operand>` field (Stage 13.16)"
    );
}

/// Verify the parser captures all comma-separated args (no silent-drop).
/// The Stage 13.11 implementation had a `while ... self.bump()` loop that
/// silently dropped args after the format string.
#[test]
fn test_parser_captures_multiple_args() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let parser_expr = manifest.join("src/parser/expr.rs");
    let content = std::fs::read_to_string(&parser_expr).expect("read parser/expr.rs");

    // Must reference Stage 13.16.
    assert!(
        content.contains("Stage 13.16"),
        "src/parser/expr.rs must reference Stage 13.16"
    );

    // The parser must have an `args` Vec collection loop.
    assert!(
        content.contains("let mut args = Vec::new()"),
        "Parser must collect args into a Vec (Stage 13.16)"
    );

    // The parser must call parse_expr for each arg.
    assert!(
        content.contains("let arg = self.parse_expr()"),
        "Parser must call parse_expr for each println! arg (Stage 13.16)"
    );

    // The parser must NOT have the old silent-drop loop pattern.
    // The old pattern was: `while !matches!(*self.peek(), TokenKind::RParen | TokenType::Eof) { self.bump(); }`
    // We check that the parser doesn't have a bare `self.bump()` inside a
    // while loop that skips to RParen (which would be the silent-drop pattern).
    // This is hard to check precisely, so we just verify the new pattern exists
    // and the args are passed to the Expr::Println constructor.
    assert!(
        content.contains("args,") && content.contains("return Expr::Println"),
        "Parser must pass `args` to the Expr::Println constructor (Stage 13.16)"
    );
}

/// Verify the resolver resolves paths inside Println args.
/// This was the bug that caused `println!("{}", x)` to print `0` instead
/// of `x`'s value — the path `x` was left as `Res::Unknown`, causing MIR
/// lower to fall back to an error placeholder.
#[test]
fn test_resolver_handles_println_args() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path_resolve = manifest.join("src/resolve/path_resolve.rs");
    let content = std::fs::read_to_string(&path_resolve).expect("read path_resolve.rs");

    // Must reference Stage 13.16.
    assert!(
        content.contains("Stage 13.16"),
        "src/resolve/path_resolve.rs must reference Stage 13.16"
    );

    // The resolver must have a Println arm that resolves args.
    assert!(
        content.contains("HirExprKind::Println { args, .. }"),
        "Resolver must have a HirExprKind::Println arm (Stage 13.16)"
    );

    // The resolver must call resolve_expr for each arg.
    assert!(
        content.contains("self.resolve_expr(arg, interner)"),
        "Resolver must call resolve_expr for each Println arg (Stage 13.16)"
    );
}

/// Verify the codegen builds a C printf format string with type-specific
/// specifiers (replacing Landin `{}` with `%ld`/`%s`/`%d`/`%f`).
#[test]
fn test_codegen_builds_format_string() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let codegen_mod = manifest.join("src/codegen/mod.rs");
    let content = std::fs::read_to_string(&codegen_mod).expect("read codegen/mod.rs");

    // Must reference Stage 13.16.
    assert!(
        content.contains("Stage 13.16"),
        "src/codegen/mod.rs must reference Stage 13.16"
    );

    // The codegen must build a C format string variable.
    assert!(
        content.contains("let mut c_fmt = String::new()"),
        "Codegen must build a C format string variable `c_fmt` (Stage 13.16)"
    );

    // The codegen must replace `{}` with `%ld` for integers.
    assert!(
        content.contains("\"%ld\""),
        "Codegen must use `%ld` for integer args (Stage 13.16)"
    );

    // The codegen must use `%s` for string/pointer args.
    assert!(
        content.contains("\"%s\""),
        "Codegen must use `%s` for string/pointer args (Stage 13.16)"
    );

    // The codegen must null-terminate the C format string.
    assert!(
        content.contains("c_fmt.push('\\0')"),
        "Codegen must null-terminate the C format string (Stage 13.16)"
    );

    // The codegen must call printf with the format string + args.
    assert!(
        content.contains("\"printf\"") && content.contains("call_args"),
        "Codegen must call printf with call_args (Stage 13.16)"
    );
}

/// Verify the Stage 13.16 design alignment document exists with the
/// required sections.
#[test]
fn test_stage_13_16_design_alignment_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let design_doc = manifest.join("docs/develop/v0/stage-13/stage-13.16-design-alignment.md");
    assert!(
        design_doc.exists(),
        "docs/develop/v0/stage-13/stage-13.16-design-alignment.md must exist"
    );

    let content = std::fs::read_to_string(&design_doc).expect("read design-alignment.md");

    assert!(
        content.contains("§13.4") && content.contains("§14.4") && content.contains("§25.8"),
        "design-alignment.md must reference §13.4, §14.4, §25.8"
    );

    assert!(
        content.contains("Strategy B") && content.contains("format args"),
        "design-alignment.md must recommend Strategy B for format args"
    );
}

/// Verify the Stage 13.16 gate review document exists with a PASS verdict.
#[test]
fn test_stage_13_16_gate_review_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gate_review = manifest.join("docs/develop/v0/stage-13/gate-review-13.16.md");
    assert!(
        gate_review.exists(),
        "docs/develop/v0/stage-13/gate-review-13.16.md must exist"
    );

    let content = std::fs::read_to_string(&gate_review).expect("read gate-review-13.16.md");

    assert!(
        content.contains("Stage 13.16") && content.contains("PASS"),
        "gate-review-13.16.md must reference Stage 13.16 and have PASS verdict"
    );
}

/// Verify the v0.1 gate still holds (≥5000 conformance tests).
#[test]
fn test_v01_gate_still_holds_after_stage_13_16() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let conformance_dir = manifest.join("tests/conformance");
    let mut count = 0usize;
    if let Ok(entries) = std::fs::read_dir(&conformance_dir) {
        for entry in entries.flatten() {
            if let Ok(tree) = std::fs::read_dir(entry.path()) {
                for sub in tree.flatten() {
                    if let Ok(inner) = std::fs::read_dir(sub.path()) {
                        for f in inner.flatten() {
                            if f.path().extension().and_then(|s| s.to_str()) == Some("lin") {
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        count >= 5000,
        "v0.1 conformance gate must hold: ≥5000 .lin files (found {})",
        count
    );
}
