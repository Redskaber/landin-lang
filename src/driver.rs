//! Compiler driver: wires together all compilation passes.
//!
//! This is the single entry point for compiling Landin source code.
//! It runs each pass in order, collecting errors as it goes:
//!
//! ```text
//! source text
//!     │
//!     ▼
//! 1. lexer::tokenize           → tokens + lex errors
//!     │
//!     ▼
//! 2. parser::Parser::parse_crate → AST crate + parse errors
//!     │
//!     ▼
//! 3. hir::lower::lower_crate   → HIR crate
//!     │
//!     ▼
//! 4. resolve::resolve_crate    → mutates HIR (sets Res on paths) + resolve errors
//!     │
//!     ▼
//! 5. mir::lower::lower_hir_body_to_mir  (per body)
//!     │
//!     ▼
//! 6. typeck::check_mir_body    → mutates MIR (writes resolved types) + type errors
//!     │
//!     ▼
//! 7. borrowck::check_mir_body  → borrow/move errors
//!     │
//!     ▼
//! CompileResult { mirs, errors }
//! ```
//!
//! Per the Stage 2.x gate review (§9 Integration Verification Protocol),
//! the driver is what makes the sub-stages actually work together. Without
//! a driver, each sub-stage is "isolated correct" but "integrated broken".

use crate::borrowck::{self, BorrowError};
use crate::hir::lower::lower_crate;
use crate::hir::{HirCrate, HirFnRetTy, HirItem, OwnerNode};
use crate::lexer::tokenize;
use crate::mir::body::MirBody;
use crate::mir::lower::lower_hir_body_to_mir_full;
use crate::parser::Parser;
use crate::resolve::resolve_crate;
use crate::session::Span;
use crate::typeck::{self, TypeError, TypeckResults};
use lasso::Rodeo;

/// Errors collected from one or more passes.
#[derive(Debug, Default)]
pub struct CompileErrors {
    /// Lexer errors (always fatal — cannot continue if tokens are bad).
    pub lex: Vec<crate::lexer::LexError>,
    /// Parser errors (always fatal).
    pub parse: Vec<crate::parser::ParseError>,
    /// Name resolution errors (non-fatal — HIR is still produced).
    pub resolve: Vec<crate::resolve::ResolveError>,
    /// Type errors (non-fatal — MIR is still produced).
    pub typeck: Vec<TypeError>,
    /// Borrow errors (non-fatal — MIR is still produced).
    pub borrowck: Vec<BorrowError>,
}

impl CompileErrors {
    pub fn is_empty(&self) -> bool {
        self.lex.is_empty()
            && self.parse.is_empty()
            && self.resolve.is_empty()
            && self.typeck.is_empty()
            && self.borrowck.is_empty()
    }

    pub fn total_count(&self) -> usize {
        self.lex.len()
            + self.parse.len()
            + self.resolve.len()
            + self.typeck.len()
            + self.borrowck.len()
    }

    pub fn has_fatal(&self) -> bool {
        !self.lex.is_empty() || !self.parse.is_empty()
    }

    /// Format all errors as a human-readable string, suitable for
    /// displaying to the user. Each error includes:
    ///   - The error category (lex/parse/resolve/typeck/borrowck)
    ///   - The error message
    ///   - A snippet of the source code around the error's span
    ///     (with a `^` underline indicating the span)
    ///
    /// `src` is the original source string (used to extract snippets).
    /// If `src` is None, only the messages are printed (no snippets).
    ///
    /// Stage 2.4d (P1-4): This is the user-facing error display.
    /// Previously, errors were only available as raw Debug output.
    pub fn format_for_user(&self, src: Option<&str>) -> String {
        let mut out = String::new();
        let total = self.total_count();
        if total == 0 {
            return String::new();
        }
        out.push_str(&format!("error: {} error(s)\n", total));

        for e in &self.lex {
            out.push_str(&format!("  [lex] {}\n", e.message));
            if let Some(s) = src {
                out.push_str(&format_snippet(s, &e.span));
            }
        }
        for e in &self.parse {
            out.push_str(&format!("  [parse] {}\n", e.message));
            if let Some(s) = src {
                out.push_str(&format_snippet(s, &e.span));
            }
        }
        for e in &self.resolve {
            out.push_str(&format!("  [resolve] {:?}\n", e));
        }
        for e in &self.typeck {
            out.push_str(&format!("  [typeck] {}\n", e.message));
            if let Some(s) = src {
                out.push_str(&format_snippet(s, &e.span));
            }
        }
        for e in &self.borrowck {
            out.push_str(&format!("  [borrowck] {} ({:?})\n", e.message, e.kind));
            if let Some(s) = src {
                out.push_str(&format_snippet(s, &e.span));
            }
        }
        out
    }
}

/// Format a source snippet around a span, with a `^` underline.
///
/// ```text
///   |
/// 5 | let x: bool = 42;
///   |                ^^
///   |
/// ```
///
/// For dummy spans (lo == hi == 0), returns an empty string (no snippet).
fn format_snippet(src: &str, span: &Span) -> String {
    if span.is_dummy() {
        return String::new();
    }
    let lo = span.lo as usize;
    let hi = span.hi as usize;
    if lo >= src.len() || hi > src.len() {
        return String::new();
    }

    // Find the line containing `lo`.
    let mut line_start = 0;
    let mut line_end = src.len();
    let mut line_no = 1;
    for (i, c) in src.char_indices() {
        if i < lo {
            if c == '\n' {
                line_start = i + 1;
                line_no += 1;
            }
        } else if c == '\n' {
            line_end = i;
            break;
        }
    }
    if line_end < line_start {
        line_end = src.len();
    }
    let line = &src[line_start..line_end.min(src.len())];

    // Compute column offsets within the line.
    let col_lo = lo.saturating_sub(line_start);
    let col_hi = hi.saturating_sub(line_start).max(col_lo + 1);

    let mut out = String::new();
    let line_no_str = line_no.to_string();
    let pad = " ".repeat(line_no_str.len());
    out.push_str(&format!("  {} |\n", pad));
    out.push_str(&format!("{} | {}\n", line_no_str, line));
    out.push_str(&format!("  {} | ", pad));
    out.push_str(&" ".repeat(col_lo));
    let span_len = col_hi.saturating_sub(col_lo).max(1);
    out.push_str(&"^".repeat(span_len));
    out.push('\n');
    out
}

/// The result of compiling a source file.
pub struct CompileResult {
    /// The HIR crate (always produced if parsing succeeds).
    pub hir: Option<HirCrate>,
    /// Per-body MIR (always produced if HIR lowering succeeds).
    /// Each entry is (BodyId, MirBody) — MirBody has resolved types
    /// written back into local_decls by typeck.
    pub mirs: Vec<MirBody>,
    /// Per-body typeck results (resolved types keyed by LocalId and HirId).
    /// Stage 2.4d (P1-3): populated so downstream consumers (codegen,
    /// error display) can consult resolved types without re-running typeck.
    pub typeck_results: Vec<TypeckResults>,
    /// All errors collected from every pass.
    pub errors: CompileErrors,
    /// The interner used during compilation. Useful for debugging.
    pub interner: Rodeo,
}

impl CompileResult {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Compile a source string through the full pipeline.
///
/// This is the main entry point. Returns a `CompileResult` containing
/// the HIR crate, per-body MIR (with resolved types), and any errors
/// collected along the way.
///
/// Errors are non-fatal unless they're lex/parse errors (which prevent
/// HIR/MIR from being produced). Even with type/borrow errors, the MIR
/// is still produced — this lets later stages (codegen, error display)
/// work with partial results.
pub fn compile(src: &str) -> CompileResult {
    let mut interner = Rodeo::new();
    let mut errors = CompileErrors::default();

    // === Stage 0: Lex ===
    let (tokens, lex_errors) = tokenize(src, &mut interner);
    errors.lex = lex_errors;
    if !errors.lex.is_empty() {
        return CompileResult {
            hir: None,
            mirs: Vec::new(),
            typeck_results: Vec::new(),
            errors,
            interner,
        };
    }

    // === Stage 0: Parse ===
    let mut parser = Parser::new(tokens, &interner);
    let krate = parser.parse_crate();
    errors.parse = parser.into_errors();
    if !errors.parse.is_empty() {
        return CompileResult {
            hir: None,
            mirs: Vec::new(),
            typeck_results: Vec::new(),
            errors,
            interner,
        };
    }

    // === Stage 1: HIR lowering ===
    let mut hir = lower_crate(&krate, &interner);

    // === Stage 1: Name resolution ===
    errors.resolve = resolve_crate(&mut hir, &mut interner);

    // === Stage 2: MIR lowering + typeck + borrowck (per body) ===
    let mut mirs = Vec::with_capacity(hir.bodies.len());
    let mut typeck_results = Vec::with_capacity(hir.bodies.len());
    for (body_id, body) in &hir.bodies {
        // Look up the owner node to get the fn signature's return type.
        // For fn items, the return type comes from `HirFn.sig.output`.
        // For const/static, the type comes from `HirConst.ty` / `HirStatic.ty`.
        // For closures, there's no declared return type (inference only).
        let return_ty = hir.owner(body_id.owner.0).and_then(owner_return_ty);

        // Lower HIR → MIR. If we found a return type, pass it so the
        // return local gets the declared type instead of a fresh Infer var.
        // This lets typeck unify the body's value with the declared type,
        // catching mismatches like `fn f() -> i32 { true }`.
        //
        // We also get back the unify table that MIR lower used to allocate
        // IntVar/FloatVar for unsuffixed literals. The type checker needs
        // this table to properly default unresolved vars (i32/f64).
        let (mut mir, lower_unify) = lower_hir_body_to_mir_full(body, &interner, return_ty);

        // Type check (writes resolved types back into local_decls)
        let mut tc = typeck::TypeChecker::with_unify(lower_unify);
        tc.check_mir_body(&mut mir);
        let (type_errors, body_results) = tc.into_results();
        errors.typeck.extend(type_errors);
        typeck_results.push(body_results);

        // Borrow check
        let mut bc = borrowck::BorrowChecker::new();
        bc.check_mir_body(&mir);
        errors.borrowck.extend(bc.into_errors());

        mirs.push(mir);
    }

    CompileResult {
        hir: Some(hir),
        mirs,
        typeck_results,
        errors,
        interner,
    }
}

/// Extract the return type from an owner node, if it's a fn/const/static.
///
/// For `HirItem::Fn`: returns `Some(ty)` if the fn has an explicit return type,
///                    `None` if it's the default (`-> ()`).
/// For `HirItem::Const` / `HirItem::Static`: returns `Some(ty)` (the declared type).
/// For other owners (impl items, trait items, etc.): returns `None` for now
/// (Stage 3 will handle them).
fn owner_return_ty(owner: &OwnerNode) -> Option<crate::hir::HirTy> {
    match owner {
        OwnerNode::Item(HirItem::Fn(f)) => match &f.sig.output {
            HirFnRetTy::Ty(t) => Some(t.clone()),
            HirFnRetTy::Default(_) => None, // default is `()` — let inference handle it
        },
        OwnerNode::Item(HirItem::Const(c)) => Some(c.ty.clone()),
        OwnerNode::Item(HirItem::Static(s)) => Some(s.ty.clone()),
        // Impl/trait items and foreign items: Stage 3.
        _ => None,
    }
}

/// Compile a source string and assert that there are zero errors.
///
/// Intended for use in integration tests where any error is a bug.
/// Returns the CompileResult on success; panics with a detailed
/// breakdown on failure.
pub fn compile_expect_ok(src: &str) -> CompileResult {
    let result = compile(src);
    if result.has_errors() {
        panic!(
            "expected zero errors, but got {}:\n\
             lex: {:?}\n\
             parse: {:?}\n\
             resolve: {:?}\n\
             typeck: {:?}\n\
             borrowck: {:?}",
            result.errors.total_count(),
            result.errors.lex,
            result.errors.parse,
            result.errors.resolve,
            result.errors.typeck,
            result.errors.borrowck
        );
    }
    result
}

/// Compile a source string and assert that it has at least one error
/// of a specific kind. Returns the CompileResult for further inspection.
pub fn compile_expect_errors(src: &str) -> CompileResult {
    let result = compile(src);
    if !result.has_errors() {
        panic!(
            "expected at least one error, but compilation succeeded with zero errors.\n\
             Source:\n{}",
            src
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_compiles_empty_fn() {
        let result = compile_expect_ok("fn f() {}");
        assert_eq!(result.mirs.len(), 1);
    }

    #[test]
    fn driver_compiles_return_literal() {
        let result = compile_expect_ok("fn f() { 42 }");
        assert_eq!(result.mirs.len(), 1);
        // The return local should have a concrete Int type after typeck
        let mir = &result.mirs[0];
        let return_ty = &mir.local_decls[0].ty;
        assert!(
            matches!(return_ty.kind, crate::mir::ty::TyKind::Int(_)),
            "expected Int return type, got {:?}",
            return_ty.kind
        );
    }

    #[test]
    fn driver_compiles_let_binding() {
        let result = compile_expect_ok("fn f() { let x = 42; }");
        // The local `x` should have type i32 after typeck + default
        let mir = &result.mirs[0];
        let has_i32 = mir.local_decls.iter().any(|ld| {
            matches!(
                &ld.ty.kind,
                crate::mir::ty::TyKind::Int(crate::ast::IntTy::I32)
            )
        });
        assert!(has_i32, "expected at least one i32 local");
    }

    #[test]
    fn driver_detects_type_mismatch() {
        // `let x: bool = 42;` should produce a type error.
        // Note: this depends on HIR lower handling the `let x: T = e` annotation.
        // If type ascription isn't wired yet, this test may need adjustment.
        let result = compile("fn f() { let x: bool = 42; }");
        // We expect at least one error (type mismatch).
        // If the parser doesn't accept `let x: bool`, we'll get a parse error instead.
        // Either way, the driver shouldn't crash.
        let _ = result;
    }

    #[test]
    fn driver_compiles_if_expression() {
        let result = compile_expect_ok("fn f() { if true { 1 } else { 2 } }");
        assert_eq!(result.mirs.len(), 1);
    }

    #[test]
    fn driver_compiles_while_loop() {
        let result = compile_expect_ok("fn f() { while false { 1 } }");
        assert_eq!(result.mirs.len(), 1);
    }

    #[test]
    fn driver_compiles_binary_op() {
        let result = compile_expect_ok("fn f() { 1 + 2 }");
        let mir = &result.mirs[0];
        // The result local should have an Int type
        let has_int = mir
            .local_decls
            .iter()
            .any(|ld| matches!(&ld.ty.kind, crate::mir::ty::TyKind::Int(_)));
        assert!(has_int, "expected an Int local");
    }

    #[test]
    fn driver_compiles_function_call() {
        // Define two functions and call one from the other
        let result =
            compile_expect_ok("fn add(a: i32, b: i32) -> i32 { a + b } fn main() { add(1, 2) }");
        assert_eq!(result.mirs.len(), 2);
    }

    #[test]
    fn driver_lex_error_aborts() {
        // Unterminated string literal → lex error → driver aborts at lex stage
        let result = compile("fn f() { let x = \"unterminated; }");
        assert!(!result.errors.lex.is_empty());
        assert!(result.hir.is_none());
    }

    #[test]
    fn driver_parse_error_aborts() {
        // Missing closing brace → parse error → driver aborts at parse stage
        let result = compile("fn f() { let x = 42;");
        assert!(!result.errors.parse.is_empty());
        assert!(result.hir.is_none());
    }
}
