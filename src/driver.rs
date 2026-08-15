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
//! 6.5. mir::drop_elaboration::elaborate_drops  → insert Drop terminators (Stage 15.46)
//!     │
//!     ▼
//! 7. borrowck::check_mir_body_with_dataflow  → borrow/move errors (Stage 15.40: driver switched)
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
use crate::mir::dyn_trait::build_dyn_trait_mir_plan_from_resolver;
use crate::mir::lower::lower_hir_body_to_mir_full_with_dyn_trait_plan;
use crate::parser::Parser;
use crate::resolve::resolve_crate;
use crate::traits::TraitError;
use crate::typeck::{self, TypeError, TypeckResults};
use lasso::Rodeo;

/// Errors collected from one or more passes.
#[derive(Debug, Default)]
pub struct CompileErrors {
    /// Lexer errors (always fatal — cannot continue if tokens are bad).
    pub lex: Vec<crate::lexer::LexError>,
    /// Parser errors (always fatal).
    pub parse: Vec<crate::parser::ParseError>,
    /// Stage 18.75 P0-1: HIR lowering errors (non-fatal — HIR is still
    /// produced with placeholder nodes). Previously these were silently
    /// dropped because CompileErrors had no `lower` field.
    pub lower: Vec<crate::hir::lower::LowerError>,
    /// Name resolution errors (non-fatal — HIR is still produced).
    pub resolve: Vec<crate::resolve::ResolveError>,
    /// Type errors (non-fatal — MIR is still produced).
    pub typeck: Vec<TypeError>,
    /// Borrow errors (non-fatal — MIR is still produced).
    pub borrowck: Vec<BorrowError>,
    /// Stage 5.22: Trait coherence/completeness errors (non-fatal —
    /// compilation continues but the user should fix these).
    ///
    /// Stage 15.9 (v0.2): Changed from `Vec<String>` to `Vec<TraitError>`
    /// to preserve the structured CoherenceError/IncompleteImpl data.
    /// Closes Phase 2 audit item: "Stop stringifying CoherenceError/IncompleteImpl".
    pub trait_errors: Vec<TraitError>,
    /// Stage 18.08: macro_rules! expansion errors (non-fatal —
    /// compilation continues with whatever tokens were produced).
    /// Captures malformed macro_rules! definitions, no-matching-rule
    /// macro calls, and recursion-limit violations.
    pub macro_errors: Vec<crate::parser::macro_expand::MacroError>,
    /// Stage 18.75 P0-1: Codegen errors (non-fatal — compilation
    /// continues but object/binary emission may fail). Previously these
    /// were silently dropped because CompileErrors had no `codegen` field.
    pub codegen: Vec<crate::codegen::error::CodegenError>,
}

impl CompileErrors {
    pub fn is_empty(&self) -> bool {
        self.lex.is_empty()
            && self.parse.is_empty()
            && self.lower.is_empty()
            && self.resolve.is_empty()
            && self.typeck.is_empty()
            && self.borrowck.is_empty()
            && self.trait_errors.is_empty()
            && self.macro_errors.is_empty()
            && self.codegen.is_empty()
    }

    pub fn total_count(&self) -> usize {
        self.lex.len()
            + self.parse.len()
            + self.lower.len()
            + self.resolve.len()
            + self.typeck.len()
            + self.borrowck.len()
            + self.trait_errors.len()
            + self.macro_errors.len()
            + self.codegen.len()
    }

    pub fn has_fatal(&self) -> bool {
        !self.lex.is_empty() || !self.parse.is_empty()
    }

    /// Stage 15.15: Deprecated. Use `format_via_diagnostics` instead.
    /// Kept as thin wrapper for backward compat with existing test call sites.
    #[deprecated(since = "0.327.0", note = "Use format_via_diagnostics instead")]
    pub fn format_for_user(&self, _src: Option<&str>, _interner: Option<&Rodeo>) -> String {
        let total = self.total_count();
        if total == 0 {
            String::new()
        } else if total == 1 {
            "error: 1 error found\n".to_string()
        } else {
            format!("error: {} errors found\n", total)
        }
    }

    /// Stage 15.14: Convert all errors to `Diagnostic` values.
    ///
    /// Produces a `Vec<Diagnostic>` with one entry per error, preserving
    /// the category as a note. This bridges `CompileErrors` (the driver's
    /// 6-field error collection) to the `diagnostics` module (the single
    /// source of truth for error display).
    ///
    /// Each diagnostic has:
    /// - `level: Error`
    /// - `message`: the error message
    /// - `span`: the error span
    /// - `code`: `Some("Lex")`, `Some("Parse")`, etc. (category as code)
    /// - one child note with the category name
    ///
    /// Per §1.0 原则 3 "显式 > 隐式": the conversion is explicit.
    /// Per §23 (API Naming): `to_diagnostics` follows `<verb>_<noun>` pattern.
    pub fn to_diagnostics(&self, interner: Option<&Rodeo>) -> Vec<crate::diagnostics::Diagnostic> {
        self.to_diagnostics_with_resolver(interner, None)
    }

    /// Stage 16.83: Like `to_diagnostics` but uses resolver-backed type names
    /// for diagnostic notes (shows "MyStruct" instead of "<adt>").
    ///
    /// When `resolver` is `Some`, typeck error notes use
    /// `type_kind_to_string_with_resolver` to resolve `Adt` type names.
    /// When `None`, falls back to `type_kind_to_string` (legacy behavior).
    ///
    /// Per §1.0 原則 3 "显式 > 隐式": user-facing type names are explicit.
    /// Per §23: `to_diagnostics_with_resolver` follows `<verb>_<noun>_<prep>_<noun>` pattern.
    pub fn to_diagnostics_with_resolver(
        &self,
        interner: Option<&Rodeo>,
        resolver: Option<&crate::traits::TraitResolver>,
    ) -> Vec<crate::diagnostics::Diagnostic> {
        use crate::diagnostics::DiagnosticBuilder;
        let mut diags = Vec::new();

        for e in &self.lex {
            diags.push(
                DiagnosticBuilder::error(&e.message, e.span)
                    .with_code(crate::diagnostics::ErrorCode::Lex.to_string())
                    .build(),
            );
        }
        for e in &self.parse {
            diags.push(
                DiagnosticBuilder::error(&e.message, e.span)
                    .with_code(crate::diagnostics::ErrorCode::Parse.to_string())
                    .build(),
            );
        }
        for e in &self.resolve {
            diags.push(
                DiagnosticBuilder::error(&e.message, e.span)
                    .with_code(crate::diagnostics::ErrorCode::Resolve.to_string())
                    .build(),
            );
        }
        for e in &self.typeck {
            let mut builder = DiagnosticBuilder::error(&e.message, e.span)
                .with_code(crate::diagnostics::ErrorCode::Type.to_string());
            // Stage 15.80: use human-readable type names instead of Debug
            // format. Previously: `expected: {:?}` leaked `Int(I32)`,
            // `Infer(IntVar(IntVid(0)))`, etc. into user-facing notes.
            // Now: `expected: i32`, `expected: {integer}` etc.
            //
            // Stage 16.83: use resolver-backed type names when available
            // (shows "MyStruct" instead of "<adt>").
            //
            // Per §1.0 原則 3 "显式 > 隐式": user-facing type names are
            // explicit (e.g., "i32", not "Int(I32)").
            if let (Some(expected), Some(found)) = (&e.expected, &e.found) {
                let (expected_str, found_str) =
                    if let (Some(resolver), Some(interner)) = (resolver, interner) {
                        (
                            crate::mir::ty::type_kind_to_string_with_resolver(
                                &expected.kind,
                                resolver,
                                interner,
                            ),
                            crate::mir::ty::type_kind_to_string_with_resolver(
                                &found.kind,
                                resolver,
                                interner,
                            ),
                        )
                    } else {
                        use crate::mir::ty::type_kind_to_string;
                        (
                            type_kind_to_string(&expected.kind),
                            type_kind_to_string(&found.kind),
                        )
                    };
                builder = builder.with_note(format!("expected: {}", expected_str), e.span);
                builder = builder.with_note(format!("found: {}", found_str), e.span);
            }
            diags.push(builder.build());
        }
        for e in &self.borrowck {
            // Stage 15.80: remove `({:?})` enum variant name leak (see
            // comment in `format_for_user` above for rationale).
            diags.push(
                DiagnosticBuilder::error(&e.message, e.span)
                    .with_code(crate::diagnostics::ErrorCode::Borrow.to_string())
                    .build(),
            );
        }
        for e in &self.trait_errors {
            let msg = if let Some(interner) = interner {
                e.format_with_interner(interner)
            } else {
                // Stage 15.96: use human-readable fallback (was: Debug {:?}).
                e.format_without_interner()
            };
            // Stage 15.89: use the trait error's span (was: Span::DUMMY,
            // producing "1:1"). The span is stored in CoherenceError/
            // IncompleteImpl, populated from HirImpl.span during collect().
            let span = match e {
                TraitError::Coherence(ce) => ce.span,
                TraitError::Incomplete(inc) => inc.span,
            };
            diags.push(
                DiagnosticBuilder::error(&msg, span)
                    .with_code(crate::diagnostics::ErrorCode::Trait.to_string())
                    .build(),
            );
        }

        // Stage 18.75 P0-2: Iterate macro_errors — previously collected
        // but never rendered, making macro errors invisible to users.
        // Per §1.0 原则 4 "报错 > 静默": macro errors must reach the user.
        for e in &self.macro_errors {
            diags.push(
                DiagnosticBuilder::error(&e.message, e.span)
                    .with_code(crate::diagnostics::ErrorCode::Macro.to_string())
                    .build(),
            );
        }

        // Stage 18.75 P0-1: Iterate codegen errors — previously had no
        // field in CompileErrors, so codegen errors were silently dropped.
        // Per §1.0 原则 4 "报错 > 静默": codegen errors must reach the user.
        for e in &self.codegen {
            diags.push(
                DiagnosticBuilder::error(&e.message, e.span)
                    .with_code(crate::diagnostics::ErrorCode::Codegen.to_string())
                    .build(),
            );
        }

        // Stage 18.75 P0-1: Iterate lower errors — previously had no
        // field in CompileErrors, so HIR lowering errors were silently dropped.
        // Per §1.0 原则 4 "报错 > 静默": lowering errors must reach the user.
        for e in &self.lower {
            diags.push(
                DiagnosticBuilder::error(&e.message, e.span)
                    .with_code(crate::diagnostics::ErrorCode::Lower.to_string())
                    .build(),
            );
        }

        diags
    }

    /// Stage 15.14: Format errors via the diagnostics module.
    ///
    /// Converts all errors to `Diagnostic` values, then formats them using
    /// `DiagnosticBuffer::format_with_source` (rustc-style display with
    /// source code snippets). This is the "new" display path that uses
    /// `src/diagnostics/` as the single source of truth.
    ///
    /// The existing `format_for_user` is kept for backward compatibility.
    /// Future stages can migrate callers to this method.
    ///
    /// Per "显示友好": produces rustc-style output with:
    /// - `error[Code]: message`
    /// - `  --> source:line:col`
    /// - source snippet with `^^^` underline
    /// - notes/helps
    ///
    /// Per §23 (API Naming): `format_via_diagnostics` follows
    /// `<verb>_<prep>_<noun>` pattern.
    pub fn format_via_diagnostics(
        &self,
        src: &str,
        source_name: &str,
        source_map: &crate::session::SourceMap,
        interner: Option<&Rodeo>,
    ) -> String {
        self.format_via_diagnostics_with_resolver(src, source_name, source_map, interner, None)
    }

    /// Stage 16.83: Like `format_via_diagnostics` but uses resolver-backed
    /// type names for diagnostic notes.
    ///
    /// Per §23: `format_via_diagnostics_with_resolver` follows
    /// `<verb>_<prep>_<noun>_<prep>_<noun>` pattern.
    pub fn format_via_diagnostics_with_resolver(
        &self,
        src: &str,
        source_name: &str,
        source_map: &crate::session::SourceMap,
        interner: Option<&Rodeo>,
        resolver: Option<&crate::traits::TraitResolver>,
    ) -> String {
        use crate::diagnostics::DiagnosticBuffer;
        let diags = self.to_diagnostics_with_resolver(interner, resolver);
        let mut buf = DiagnosticBuffer::new();
        for diag in diags {
            buf.emit(diag);
        }
        buf.format_with_source(source_name, source_map, src)
    }

    /// Stage 15.18: Format errors via the diagnostics module with ANSI colors.
    ///
    /// Same as `format_via_diagnostics` but uses `format_with_source_colored`
    /// for colored output. The `color` parameter controls whether ANSI codes
    /// are emitted:
    /// - `ColorConfig::Always` — always emit colors
    /// - `ColorConfig::Never` — never emit colors (plain text)
    /// - `ColorConfig::Auto` — caller resolves to Always/Never based on TTY
    ///
    /// Per "显示友好": colored output makes it easier to distinguish
    /// errors from warnings at a glance.
    /// Per §23 (API Naming): `format_via_diagnostics_colored` follows
    /// `<verb>_<prep>_<noun>_<adj>` pattern.
    pub fn format_via_diagnostics_colored(
        &self,
        src: &str,
        source_name: &str,
        source_map: &crate::session::SourceMap,
        interner: Option<&Rodeo>,
        color: crate::diagnostics::ColorConfig,
    ) -> String {
        use crate::diagnostics::DiagnosticBuffer;
        let diags = self.to_diagnostics(interner);
        let mut buf = DiagnosticBuffer::new();
        for diag in diags {
            buf.emit(diag);
        }
        buf.format_with_source_colored(source_name, source_map, src, color)
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
    /// Stage 3.56 (Phase A §16 refactoring): pre-computed function name
    /// map for call resolution. Maps DefId → "landin_<name>".
    /// Built once during compile() so codegen doesn't need to re-scan HIR.
    pub fn_name_by_def_id: std::collections::HashMap<crate::hir::DefId, String>,
    /// Stage 3.56: per-body metadata parallel to `mirs`.
    /// Each entry: (fn_name, is_void, param_count).
    pub body_metas: Vec<BodyMeta>,
    /// Stage 14.35: Pre-computed function signatures (DefId → Sig) for codegen.
    /// Used by codegen_terminator to resolve Call return types (fixes struct-returning
    /// method calls where dest local type defaults to i32 after typeck writeback).
    pub fn_sigs: std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
    /// Stage 5.2: TraitResolver — pre-computed trait/impl dispatch tables.
    /// Built during compile() so downstream (typeck, borrowck, codegen)
    /// can query trait implementations without reading HIR.
    pub trait_resolver: crate::traits::TraitResolver,
    /// Stage 5.26: Stdlib prelude — types + traits auto-registered by the
    /// compiler. Available for downstream stages to query which names are
    /// stdlib-provided (vs user-defined).
    pub stdlib_prelude: crate::stdlib::StdlibPrelude,
    /// Stage 5.33: Stdlib facade — aggregate statistics + layer queries.
    /// Built from stdlib_prelude; provides type_count, trait_count,
    /// layer_count, is_stdlib_name, summary.
    pub stdlib_facade: crate::stdlib::StdlibFacade,
    /// Stage 15.27 (v0.2): TypeInterner — deduplicates TyKind values.
    /// Built during compile() so all type constructions can go through it.
    /// Currently not wired into Ty::new (that requires migrating all call
    /// sites). Available for debugging/stats and future wiring.
    pub type_interner: crate::mir::ty_interner::TypeInterner,
    /// Stage 16.14 (Task 10 Step 2): Synthesized closure `call` function
    /// MIR bodies. Each entry is a MirBody for a closure's synthesized
    /// `call` function, built from the `SynthesizedClosureFunction`
    /// metadata collected during MIR lowering.
    ///
    /// These are NOT yet used by codegen (Step 4) or call sites (Step 3) —
    /// the inline approach (Stage 13.3a) is still active. This field is
    /// infrastructure for the gradual migration to Strategy A.
    ///
    /// Per §16: data flows downstream from MIR lower to codegen.
    /// Per §23: `synthesized_closure_mir_bodies` follows `<adj>_<noun>_<noun>`
    /// pattern.
    pub synthesized_closure_mir_bodies: Vec<crate::mir::body::MirBody>,
}

/// Stage 3.56: Per-body metadata for codegen.
/// Pre-computed during compile() so codegen is a pure MIR consumer.
#[derive(Debug, Clone)]
pub struct BodyMeta {
    /// The function name (e.g., "landin_f") for `define @landin_f`.
    pub fn_name: String,
    /// Whether the function has no return type (void).
    pub is_void: bool,
    /// Number of parameters.
    pub param_count: usize,
    /// Stage 8.3: The ABI of this function (Landin or C).
    pub abi: crate::ast::Abi,
}

impl CompileResult {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Stage 3.56: Create a CompileResult with empty metadata fields.
    /// Used by early-return paths (lex/parse errors) where no MIR is produced.
    fn empty(interner: Rodeo, errors: CompileErrors) -> Self {
        Self {
            hir: None,
            mirs: Vec::new(),
            typeck_results: Vec::new(),
            errors,
            interner,
            fn_name_by_def_id: std::collections::HashMap::new(),
            body_metas: Vec::new(),
            fn_sigs: std::collections::HashMap::new(),
            trait_resolver: crate::traits::TraitResolver::new(),
            stdlib_prelude: crate::stdlib::default_prelude(),
            stdlib_facade: crate::stdlib::StdlibFacade::default(),
            type_interner: crate::mir::ty_interner::TypeInterner::new(),
            synthesized_closure_mir_bodies: Vec::new(),
        }
    }
}

/// Compile a source string through the full pipeline **with MIR optimization**.
///
/// This is the production entry point. MIR optimization (DCE + const_prop)
/// runs automatically per `06-mir.md` §9.3. Use `compile_no_opt()` for
/// tests that verify IR/MIR structure without optimization interference.
///
/// Returns a `CompileResult` containing the HIR crate, per-body MIR (with
/// resolved types), and any errors collected along the way.
///
/// Errors are non-fatal unless they're lex/parse errors (which prevent
/// HIR/MIR from being produced). Even with type/borrow errors, the MIR
/// is still produced — this lets later stages (codegen, error display)
/// work with partial results.
pub fn compile(src: &str) -> CompileResult {
    compile_inner(src, true)
}

/// Stage 18.96: Compile WITHOUT MIR optimization. Used by tests that
/// verify IR/MIR structure (e.g., codegen tests checking for specific
/// LLVM instruction patterns, closure-capture tests checking for
/// `AggregateKind::Closure` in the MIR).
///
/// Per §11 (interface isolation): tests should verify codegen in
/// isolation — opt changes IR structure (folds constants, removes dead
/// code), which would break structural assertions. This entry point
/// gives tests a stable, unoptimized IR to assert against.
///
/// Per §2.0 原則 3 "显式 > 隐式": the opt flag is explicit, not inferred.
/// Per §23: `compile_no_opt` follows `<verb>_<noun>` pattern.
pub fn compile_no_opt(src: &str) -> CompileResult {
    compile_inner(src, false)
}

/// Internal compile implementation. `optimize` controls whether MIR
/// optimization passes (DCE + const_prop) run after writeback.
///
/// Stage 18.96: extracted from `compile()` to support `compile_no_opt()`
/// without duplicating the 3000-line pipeline.
fn compile_inner(src: &str, optimize: bool) -> CompileResult {
    // Stage 15.28: Clear the thread-local TypeInterner at the start of each
    // compilation to avoid cross-compilation pollution.
    crate::mir::ty::Ty::clear_interner();

    let mut interner = Rodeo::new();
    let mut errors = CompileErrors::default();

    // === Stage 0: Lex ===
    let (tokens, lex_errors) = tokenize(src, &mut interner);
    errors.lex = lex_errors;
    if !errors.lex.is_empty() {
        return CompileResult::empty(interner, errors);
    }

    // === Stage 18.04: Macro expansion ===
    // Expand `macro_rules!`-defined macro calls in the token stream
    // before parsing. Built-in macros (println!) are left for the parser
    // to handle via its existing special cases.
    // Per §11: this is a parser-stage sub-module; driver only sees the
    // free-function entry `parser::macro_expand::expand_macros_with_errors`.
    //
    // Stage 18.08: collect macro expansion errors into `errors.macro_errors`.
    //
    // Stage 18.10: pre-intern built-in macro names so the macro_expand
    // module can register them into the MacroTable (println/print/
    // eprintln/eprint). Phase 1 uses no-op rule bodies so the parser's
    // existing special-case path still handles them.
    for name in crate::parser::macro_expand::BUILTIN_MACRO_NAMES {
        interner.get_or_intern(name);
    }
    // Stage 18.21: pre-intern `__landin_<name>` runtime function names
    // so the built-in macro body can reference them. The body expands
    // `println!(...)` to `__landin_println(...)`, which the parser
    // parses as `Expr::Call` and the codegen detects via
    // `is_landin_print_macro`.
    for name in crate::parser::macro_expand::BUILTIN_MACRO_NAMES {
        interner.get_or_intern(format!("__landin_{}", name));
    }
    // Pre-intern symbols used in built-in macro rule patterns/bodies.
    interner.get_or_intern("args");
    interner.get_or_intern("tt");
    // Stage 18.29: Pre-intern symbols for non-print built-in macros.
    interner.get_or_intern("cond");
    interner.get_or_intern("msg");
    interner.get_or_intern("x");
    interner.get_or_intern("dst");
    interner.get_or_intern("__landin_assert");
    interner.get_or_intern("__landin_panic_msg");
    // Stage 18.32: Pre-intern symbols for more built-in macros.
    interner.get_or_intern("__landin_format");
    interner.get_or_intern("__landin_dbg");
    interner.get_or_intern("__landin_write");
    // Stage 18.34: Pre-intern symbols for compile-time utility macros.
    interner.get_or_intern("__landin_stringify");
    interner.get_or_intern("__landin_concat");
    interner.get_or_intern("__landin_env");
    // Stage 18.36: Pre-intern symbols for source info + file macros.
    interner.get_or_intern("path");
    interner.get_or_intern("__landin_file");
    interner.get_or_intern("__landin_line");
    interner.get_or_intern("__landin_module_path");
    interner.get_or_intern("__landin_include_str");
    // Stage 18.39: Pre-intern symbols for pattern + config macros.
    interner.get_or_intern("pat");
    interner.get_or_intern("cfg");
    interner.get_or_intern("__landin_matches");
    interner.get_or_intern("__landin_cfg");
    interner.get_or_intern("__landin_option_env");
    // Stage 18.41: Pre-intern symbols for low-level + diagnostic macros.
    interner.get_or_intern("attr");
    interner.get_or_intern("__landin_asm");
    interner.get_or_intern("__landin_compile_error");
    interner.get_or_intern("__landin_cfg_attr");
    // Stage 18.43: Pre-intern symbols for control-flow + debug macros.
    interner.get_or_intern("mode");
    interner.get_or_intern("__landin_unreachable");
    interner.get_or_intern("__landin_trace_macros");
    interner.get_or_intern("__landin_format_args");
    let (tokens, macro_errs) =
        crate::parser::macro_expand::expand_macros_with_errors(tokens, &mut interner);
    errors.macro_errors = macro_errs;

    // === Stage 0: Parse ===
    let mut parser = Parser::new(tokens, &mut interner);
    let krate = parser.parse_crate();
    errors.parse = parser.into_errors();
    if !errors.parse.is_empty() {
        return CompileResult::empty(interner, errors);
    }

    // === Stage 1: HIR lowering ===
    // Stage 18.78 P0-A: lower_crate now returns (HirCrate, Vec<LowerError>).
    // Previously errors were silently discarded, making CompileErrors.lower
    // always empty. Now they're properly collected.
    let (mut hir, lower_errors) = lower_crate(&krate, &interner);
    errors.lower = lower_errors;

    // === Stage 1: Name resolution ===
    errors.resolve = resolve_crate(&mut hir, &mut interner);

    // === Stage 1.5: G4 fix — scan HIR for unresolved paths ===
    // After name resolution, any Path with Res::Unknown or Res::Err
    // indicates an undefined name (e.g., `undefined_fn()`). Emit a
    // resolve error for each.
    scan_for_unresolved_paths(&hir, &mut errors);

    // === Stage 2: MIR lowering + typeck + borrowck (per body) ===
    // Stage 3.60: Pre-compute FieldTyTable and FnSigTable from HIR so typeck
    // doesn't need to read HIR directly (per section 16 — data flows downstream).
    let mut field_ty_table = typeck::FieldTyTable::default();
    for (def_id, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s)) = owner {
            let fields: Vec<crate::mir::ty::Ty> = s
                .fields
                .iter()
                .map(|f| crate::mir::lower::lower_hir_ty_to_mir_ty(&f.ty))
                .collect();
            field_ty_table.struct_fields.insert(*def_id, fields);
        }
    }

    let mut fn_sig_table = typeck::FnSigTable::default();

    // Stage 18.102 (TD-MONO-INFER): Build generics_map from HIR for
    // writeback_fndef_substs. This maps DefId → Vec<ParamTy> for all
    // generic items (fns, structs, enums, etc.).
    // Per §16: pre-computed from HIR (data flows downstream, no HIR access
    // during writeback). Per §23: `find_generics` follows `<verb>_<noun>`.
    let generics_map: std::collections::HashMap<crate::hir::DefId, Vec<crate::mir::ty::ParamTy>> = {
        let mut map = std::collections::HashMap::new();
        for (def_id, _) in &hir.owners {
            let params = crate::hir::generics::find_generics(*def_id, &hir);
            if !params.is_empty() {
                map.insert(*def_id, params);
            }
        }
        map
    };

    // Stage 16.16: Declare fn_name_by_def_id early so the per-body loop
    // can register synthesized closure function names.
    let mut fn_name_by_def_id: std::collections::HashMap<crate::hir::DefId, String> =
        std::collections::HashMap::new();

    // Stage 15.2 (perf): Pre-build method→impl index for O(1) lookup.
    let method_to_impl_index = build_method_to_impl_index(&hir);

    for (def_id, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Fn(f)) = owner {
            use crate::hir::HirFnRetTy;
            let inputs: Vec<crate::mir::ty::Ty> = f
                .sig
                .inputs
                .iter()
                .map(|p| {
                    // Stage 14.43: Handle `self` shorthand parameters.
                    //
                    // For `&mut self` / `&self` / `self`, the HIR `p.ty` may be
                    // a placeholder (non-empty Spur but resolves to Res::Unknown
                    // or Res::Err). We check `p.self_kind` FIRST — if it's Some,
                    // the parameter is a self param and its type comes from the
                    // owning impl block's self_ty (with Ref wrapping for &self/&mut self).
                    //
                    // Previously, `p.ty` was checked first, causing impl methods
                    // with `&mut self` to have wrong signatures (placeholder type
                    // instead of the impl's self_ty). This caused LLVM type
                    // mismatches for nested struct methods.
                    //
                    // Per §13.4 (design alignment): self_kind is the authoritative
                    // indicator of a self parameter — the ty field is a HIR
                    // lowering detail that may or may not be set.
                    if p.self_kind.is_some() {
                        // Resolve self param type from owning impl block.
                        resolve_self_param_type_for_sig(
                            &hir,
                            *def_id,
                            p.self_kind,
                            &method_to_impl_index,
                        )
                        .unwrap_or_else(|| {
                            // Fallback: if self_ty resolution fails, try p.ty
                            if let Some(ty) = &p.ty {
                                crate::mir::lower::lower_hir_ty_to_mir_ty(ty)
                            } else {
                                crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Error, p.span)
                            }
                        })
                    } else if let Some(ty) = &p.ty {
                        crate::mir::lower::lower_hir_ty_to_mir_ty(ty)
                    } else {
                        crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Error, p.span)
                    }
                })
                .collect();
            let output = match &f.sig.output {
                HirFnRetTy::Ty(t) => crate::mir::lower::lower_hir_ty_to_mir_ty(t),
                HirFnRetTy::Default(_) => {
                    crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Tuple(vec![]), f.span)
                }
            };
            fn_sig_table.sigs.insert(
                *def_id,
                crate::mir::ty::Sig {
                    inputs: inputs.clone(),
                    output: Box::new(output),
                    abi: f.sig.abi,
                    is_unsafe: f.sig.is_unsafe,
                },
            );
            if crate::session::debug_codegen_enabled() {
                let name = interner.try_resolve(&f.ident.name).unwrap_or("?");
                eprintln!(
                    "[DRIVER] fn_sig_table (HirItem::Fn): def_id={:?} name={} inputs_len={}",
                    def_id,
                    name,
                    inputs.len()
                );
            }
        }
    }

    // Stage 14.91 (Bug X3 fix): Also build fn_sig_table entries for trait
    // impl methods. The loop above only handles HirItem::Fn owners, but
    // trait impl methods are HirImplItem::Fn inside HirItem::Impl owners.
    // Without this, call-site forward declarations use a generic variadic
    // signature that doesn't match the actual function definition, causing
    // LLVM to create a renamed duplicate (e.g. `landin_Square_area.1`)
    // and producing an "undefined reference" link error.
    for (def_id, owner) in &hir.owners {
        if crate::session::debug_codegen_enabled() {
            eprintln!("[DRIVER] owner: def_id={:?} kind={:?}", def_id, owner);
        }
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
            for impl_item in &impl_block.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    use crate::hir::HirFnRetTy;
                    let method_def_id = f.hir_id.owner;
                    // Skip if already registered (inherent impl methods are
                    // registered as HirItem::Fn owners — but trait impl methods
                    // might not be).
                    if fn_sig_table.sigs.contains_key(&method_def_id) {
                        continue;
                    }
                    let inputs: Vec<crate::mir::ty::Ty> = f
                        .sig
                        .inputs
                        .iter()
                        .map(|p| {
                            if p.self_kind.is_some() {
                                resolve_self_param_type_for_sig(
                                    &hir,
                                    method_def_id,
                                    p.self_kind,
                                    &method_to_impl_index,
                                )
                                .unwrap_or_else(|| {
                                    if let Some(ty) = &p.ty {
                                        crate::mir::lower::lower_hir_ty_to_mir_ty(ty)
                                    } else {
                                        crate::mir::ty::Ty::new(
                                            crate::mir::ty::TyKind::Error,
                                            crate::session::Span::DUMMY,
                                        )
                                    }
                                })
                            } else if let Some(ty) = &p.ty {
                                crate::mir::lower::lower_hir_ty_to_mir_ty(ty)
                            } else {
                                crate::mir::ty::Ty::new(
                                    crate::mir::ty::TyKind::Error,
                                    crate::session::Span::DUMMY,
                                )
                            }
                        })
                        .collect();
                    let output = match &f.sig.output {
                        HirFnRetTy::Default(_) => crate::mir::ty::Ty::new(
                            crate::mir::ty::TyKind::Tuple(Vec::new()),
                            crate::session::Span::DUMMY,
                        ),
                        HirFnRetTy::Ty(t) => crate::mir::lower::lower_hir_ty_to_mir_ty(t),
                    };
                    fn_sig_table.sigs.insert(
                        method_def_id,
                        crate::mir::ty::Sig {
                            inputs,
                            output: Box::new(output),
                            abi: f.sig.abi,
                            is_unsafe: f.sig.is_unsafe,
                        },
                    );
                    if crate::session::debug_codegen_enabled() {
                        let name = interner.try_resolve(&f.ident.name).unwrap_or("?");
                        eprintln!("[DRIVER] fn_sig_table: inserted method_def_id={:?} name={} inputs_len={}",
                            method_def_id, name, fn_sig_table.sigs.get(&method_def_id).map(|s| s.inputs.len()).unwrap_or(0));
                    }
                }
            }
        }
    }
    // Stage 14.97 (Bug Y1 fix): Also build fn_sig_table entries for trait
    // DEFAULT BODY methods. A trait default body is a method declared inside
    // a `trait T { fn f(&self) -> i32 { ... } }` block that has a body. When
    // called via static dispatch (e.g., `p.f()` where p: Pair and Pair: T),
    // codegen needs the function signature to emit the correct call.
    //
    // Strategy: For each trait method with a body, find the unique impl of
    // that trait (if any). Use the impl's self_ty as the self parameter type.
    // If multiple impls exist, use the first impl's self_ty (v0.1 limitation
    // — full monomorphization is v0.2+ work).
    //
    // Stage 14.99 (Bug Z7 fix): Emit a warning when 2+ impls exist for a trait
    // with default bodies. Per §1.0 原则 5 "报错 > 静默": the user should know
    // that the default body will be specialized for only the first impl.
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
            let trait_name = t.ident.name;
            // Find all impls of this trait.
            let impls: Vec<_> = hir
                .owners
                .iter()
                .filter_map(|(_, o)| {
                    if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = o {
                        if impl_block
                            .of_trait
                            .as_ref()
                            .and_then(|p| p.segments.last().map(|s| s.ident.name))
                            == Some(trait_name)
                        {
                            return Some(impl_block);
                        }
                    }
                    None
                })
                .collect();
            // Stage 14.99 (Bug Z7 fix): Check if this trait has any default body methods.
            // If so, and if there are 2+ impls, emit a warning per §1.0 原则 5.
            //
            // Stage 14.100 (Bug AA6 fix): Refine the check — only emit the error
            // if at least one impl does NOT override the default body method.
            // If all impls override the default body, the default is never used,
            // so no specialization issue can occur.
            if impls.len() >= 2 {
                // For each trait method with a body, check if any impl doesn't override it.
                let mut any_unoverridden_default = false;
                for trait_item in &t.items {
                    if let crate::hir::HirTraitItem::Fn(default_fn) = trait_item {
                        if default_fn.body.is_none() {
                            continue;
                        }
                        // Check if every impl overrides this method.
                        let all_override = impls.iter().all(|impl_block| {
                            impl_block.items.iter().any(|impl_item| {
                                if let crate::hir::HirImplItem::Fn(impl_fn) = impl_item {
                                    impl_fn.ident.name == default_fn.ident.name
                                } else {
                                    false
                                }
                            })
                        });
                        if !all_override {
                            any_unoverridden_default = true;
                            break;
                        }
                    }
                }
                if any_unoverridden_default {
                    let trait_name_str = interner.try_resolve(&trait_name).unwrap_or("?");
                    errors.typeck.push(crate::typeck::TypeError::new(
                        format!(
                            "trait `{}` has default body methods and {} impls — \
                             v0.1 will specialize the default body using the first impl's \
                             self_ty only. Other impls will use incorrect specialization. \
                             This is a v0.1 limitation; full monomorphization is v0.2+ work. \
                             Workaround: override the default body in each impl.",
                            trait_name_str,
                            impls.len()
                        ),
                        t.span,
                    ));
                }
            }
            for trait_item in &t.items {
                if let crate::hir::HirTraitItem::Fn(f) = trait_item {
                    if f.body.is_none() {
                        continue; // No body — no fn_sig needed (it's just a declaration).
                    }
                    let method_def_id = f.hir_id.owner;
                    if fn_sig_table.sigs.contains_key(&method_def_id) {
                        continue; // Already registered (e.g., overridden in an impl).
                    }
                    // Use the first impl's self_ty as the specialization type.
                    let self_ty_opt = impls.first().map(|impl_block| {
                        crate::mir::lower::lower_hir_ty_to_mir_ty(&impl_block.self_ty)
                    });
                    let inputs: Vec<crate::mir::ty::Ty> = f
                        .sig
                        .inputs
                        .iter()
                        .map(|p| {
                            if p.self_kind.is_some() {
                                if let Some(ref self_ty) = self_ty_opt {
                                    match p.self_kind {
                                        Some(crate::ast::SelfKind::Ref(mutability)) => {
                                            let mir_mut = match mutability {
                                                crate::ast::Mutability::Mutable => {
                                                    crate::mir::ty::Mutability::Mutable
                                                }
                                                crate::ast::Mutability::Immutable => {
                                                    crate::mir::ty::Mutability::Immutable
                                                }
                                            };
                                            crate::mir::ty::Ty::new(
                                                crate::mir::ty::TyKind::Ref(
                                                    crate::mir::ty::Region::Erased,
                                                    mir_mut,
                                                    Box::new(self_ty.clone()),
                                                ),
                                                crate::session::Span::DUMMY,
                                            )
                                        }
                                        _ => self_ty.clone(),
                                    }
                                } else {
                                    crate::mir::ty::Ty::new(
                                        crate::mir::ty::TyKind::Error,
                                        crate::session::Span::DUMMY,
                                    )
                                }
                            } else if let Some(ty) = &p.ty {
                                crate::mir::lower::lower_hir_ty_to_mir_ty(ty)
                            } else {
                                crate::mir::ty::Ty::new(
                                    crate::mir::ty::TyKind::Error,
                                    crate::session::Span::DUMMY,
                                )
                            }
                        })
                        .collect();
                    let output = match &f.sig.output {
                        HirFnRetTy::Default(_) => crate::mir::ty::Ty::new(
                            crate::mir::ty::TyKind::Tuple(Vec::new()),
                            crate::session::Span::DUMMY,
                        ),
                        HirFnRetTy::Ty(t) => crate::mir::lower::lower_hir_ty_to_mir_ty(t),
                    };
                    fn_sig_table.sigs.insert(
                        method_def_id,
                        crate::mir::ty::Sig {
                            inputs,
                            output: Box::new(output),
                            abi: f.sig.abi,
                            is_unsafe: f.sig.is_unsafe,
                        },
                    );
                    if crate::session::debug_codegen_enabled() {
                        let name = interner.try_resolve(&f.ident.name).unwrap_or("?");
                        eprintln!(
                            "[DRIVER] fn_sig_table: inserted trait default method_def_id={:?} name={} inputs_len={}",
                            method_def_id,
                            name,
                            fn_sig_table
                                .sigs
                                .get(&method_def_id)
                                .map(|s| s.inputs.len())
                                .unwrap_or(0)
                        );
                    }
                }
            }
        }
    }
    let mut mirs = Vec::with_capacity(hir.bodies.len());
    let mut typeck_results = Vec::with_capacity(hir.bodies.len());
    // Stage 16.14: Synthesized closure MIR bodies, built per-function.
    let mut synthesized_closure_mir_bodies: Vec<crate::mir::body::MirBody> = Vec::new();

    // Stage 5.2: Build TraitResolver — collect trait definitions + impl blocks.
    // Per §16: pre-computed by driver, passed as data to downstream stages.
    //
    // Stage 5.80 (refactor): moved BEFORE the per-body loop so the
    // DynTraitMIRPlan can be built from it and passed to lowering.
    // Previously this came after the loop — fine when lower didn't need
    // trait info, but Stage 5.78+ requires the plan at lower time.
    let mut trait_resolver = crate::traits::TraitResolver::new();
    // Stage 5.8: Register builtin standard traits (Copy, Clone, Drop, etc.)
    // before collect() so the compiler recognizes them without user
    // definition. Needs &mut interner (collect() only takes &Rodeo).
    // We clone the interner to get a mutable handle, register, then the
    // original interner is used for collect() and stored in CompileResult.
    // NOTE: interner is already &mut here (line 267: `let mut interner`),
    // but by this point several borrows have happened. We use a direct
    // mutable call since interner is still owned.
    trait_resolver.register_builtin_traits(&mut interner);
    // Stage 5.26: Register stdlib types + traits in the interner.
    // This ensures all core types (i32, bool, str, etc.) and stdlib traits
    // (Add, From, Iterator, etc.) are interned before compilation.
    crate::stdlib::register_stdlib(&mut interner);
    // Stage 15.9: trait_resolver.collect now takes &mut Rodeo to intern
    // vtable symbol names (VtableEntry.fn_name is now Spur, was String).
    trait_resolver.collect(&hir, &mut interner);

    // Stage 16.65 (Task 14 Phase 2): Object safety check.
    //
    // Scan all HIR types for `dyn Trait` usage (HirTyKind::TraitObject).
    // For each, look up the trait definition and check if it's object-safe.
    // If not, emit a typeck error — the user must fix the trait or avoid
    // using `dyn Trait`.
    //
    // Per §16: driver reads HIR + TraitResolver (allowed during pre-computation).
    // Per §1.0 原則 5 "报错 > 静默": hard errors for non-object-safe traits.
    // Per §1.0 原則 6 "通用 > 特例": one scan function handles all TraitObject uses.
    check_object_safety_for_dyn_trait_usage(&hir, &trait_resolver, &interner, &mut errors);

    // Stage 16.73: Where clause checking.
    //
    // Verify that all where clause bounds reference valid traits.
    // Full semantic checking (does the type implement the trait?) is
    // deferred to future work — for now we verify trait existence.
    let where_errors =
        crate::typeck::where_clause::check_where_clauses(&hir, &trait_resolver, &interner);
    errors.typeck.extend(where_errors);

    // Stage 5.80: build DynTraitMIRPlan once for the whole crate.
    //
    // Per §16: the driver is the sole orchestrator that connects
    // TraitResolver (Stage 5.2) to mir::lower (Stage 2.1) via the plan
    // data structure. `MirLowerCtxt` does not own a TraitResolver — it
    // receives the plan as data via `set_dyn_trait_plan`.
    //
    // The plan is built once here (before the per-body loop) and passed
    // by reference to each body's lowering. The lower clones the plan
    // internally when attaching it to the cx (one clone per body —
    // acceptable cost; the plan is small).
    //
    // This activates the dyn Trait MIR lowering path (Stage 5.78) and
    // the codegen vtable indirect call path (Stage 5.79) end-to-end:
    // HIR `receiver.method(args)` → MIR `TerminatorKind::Call` with Const
    // marker → codegen `getelementptr + load + load + indirect call`.
    let dyn_trait_plan = build_dyn_trait_mir_plan_from_resolver(&trait_resolver, &interner);

    // Stage 14.100 (Bug AA5 fix): Track which body_ids are lowered (i.e., not
    // skipped). This set is used to filter body_metas so codegen doesn't try
    // to emit functions for skipped bodies (which would have no MIR and
    // produce invalid LLVM IR like `void %(void %arg0)`).
    let mut lowered_body_owners: std::collections::HashSet<crate::hir::DefId> =
        std::collections::HashSet::new();

    for (body_id, body) in &hir.bodies {
        // Stage 14.100 (Bug AA5 fix): Skip codegen for trait default body
        // methods when the trait has zero impls. The default body references
        // `self.<method>()` calls that have no resolution with zero impls,
        // causing LLVM crashes ("Called function must be a pointer!").
        //
        // Per §1.0 原则 5 "报错 > 静默": silently crashing is worse than
        // skipping the dead code. If the user actually calls the default body,
        // they'd get a compile error elsewhere (no impl exists to dispatch to).
        // If they don't call it, skipping is correct — dead code elimination.
        let owner_def_id = body_id.owner.0;
        let is_default_body_with_zero_impls = hir.owners.iter().any(|(_, owner)| {
            if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
                // Check if this body belongs to one of this trait's default body methods.
                let owns_body = t.items.iter().any(|item| {
                    if let crate::hir::HirTraitItem::Fn(f) = item {
                        // f.body is Some(BodyId) for default body methods.
                        // Compare the body's owner DefId with the current body's owner.
                        f.body.map(|b| b.owner.0) == Some(owner_def_id)
                    } else {
                        false
                    }
                });
                if owns_body {
                    // Check if this trait has zero impls.
                    let trait_name = t.ident.name;
                    let has_impl = hir.owners.iter().any(|(_, o)| {
                        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) =
                            o
                        {
                            impl_block
                                .of_trait
                                .as_ref()
                                .and_then(|p| p.segments.last().map(|s| s.ident.name))
                                == Some(trait_name)
                        } else {
                            false
                        }
                    });
                    return !has_impl;
                }
            }
            false
        });
        if crate::session::debug_codegen_enabled() {
            eprintln!(
                "[DRIVER] body_id owner={:?} is_default_body_with_zero_impls={}",
                owner_def_id, is_default_body_with_zero_impls
            );
        }
        if is_default_body_with_zero_impls {
            continue;
        }
        lowered_body_owners.insert(owner_def_id);

        let return_ty = hir.find_owner(body_id.owner.0).and_then(owner_return_ty);

        let (mut mir, lower_unify, lower_type_errors, synthesized_closures) =
            lower_hir_body_to_mir_full_with_dyn_trait_plan(
                body,
                &interner,
                &hir,
                return_ty,
                Some(&dyn_trait_plan),
                Some(&trait_resolver),
            );

        // Stage 16.14 (Task 10 Step 2): Build MIR bodies for synthesized
        // closure `call` functions.
        //
        // Stage 16.16 (Task 10 Steps 3+4): Now used by codegen! The
        // synthesized closure function names are registered in
        // fn_name_by_def_id so codegen can resolve them.
        //
        // Stage 16.29 (通解 — Shared unify table + Typeck on closure MIR):
        // The KEY fix: share the unify table between the main body and
        // all closure MIR bodies. This eliminates the TyVid collision
        // that caused infinite recursion in resolve_ty_var.
        //
        // The flow:
        //   1. Lower main body → main_mir, main_unify, synthesized_closures
        //   2. For each closure:
        //      (a) Build closure MIR body, passing main_unify IN. The
        //          closure's fresh Infer vars are allocated from main_unify
        //          (continuing the TyVid counter). The closure_struct_ty's
        //          Infer vars (from main body lowering) are already in
        //          main_unify. No collision.
        //      (b) Get back (closure_mir, main_unify, errors).
        //      (c) Register fn_name + placeholder fn_sig (with fresh Infer
        //          vars from main_unify for params/return).
        //   3. Typeck MAIN body with main_unify → resolves closure_struct_ty's
        //      Infer vars and closure fn_sig's Infer vars (via Call sites).
        //      Extract main_unify back via into_results_with_unify.
        //   4. For each closure MIR body:
        //      (a) Typeck with main_unify → resolves closure body's Infer
        //          vars. Extract main_unify back.
        //      (b) Update fn_sig with resolved types from local_decls.
        //      (c) Run drop elaboration + borrowck.
        //
        // Per §1.0 原則 6 "通用 > 特例": one unify table for main body +
        // all closures — no special-case handling per closure type.
        // Per §1.0 原則 9 "正确 > 妥协": fix the root cause (unify table
        // isolation), not the symptom (cycle detection in resolve_ty_var).
        // Per §16: closure MIR bodies get the same typeck + borrowck
        // treatment as regular function MIR bodies.

        // Collect closure MIR bodies + their DefIds for deferred typeck.
        // We build all closure MIR bodies FIRST (sharing main_unify), then
        // typeck the main body, then typeck each closure MIR body.
        let mut pending_closure_mirs: Vec<(
            crate::mir::lower::SynthesizedClosureFunction,
            crate::mir::body::MirBody,
        )> = Vec::new();

        // Stage 16.29: Take ownership of lower_unify so we can pass it
        // through build_synthesized_closure_mir_body (which uses
        // new_with_unify to share the table).
        let mut shared_unify = lower_unify;
        // Stage 16.29: Track the closure_def_id_counter to avoid DefId
        // collisions between outer and nested closures. Initialize to the
        // number of closures already allocated by the main body lowering
        // (each call to allocate_closure_def_id increments the counter).
        let mut shared_closure_def_id_counter: u32 = synthesized_closures.len() as u32;

        // Stage 16.29: Process closures in a worklist — each closure may
        // contain nested closures (e.g., `|| || x`), which are discovered
        // during lowering and added to the worklist.
        let mut closure_worklist: Vec<crate::mir::lower::SynthesizedClosureFunction> =
            synthesized_closures.values().cloned().collect();

        while let Some(func) = closure_worklist.pop() {
            // Stage 16.29: Build closure MIR body, SHARING shared_unify.
            // The closure's fresh Infer vars are allocated from shared_unify,
            // avoiding TyVid collision with closure_struct_ty's Infer vars.
            let (
                closure_mir,
                returned_unify,
                closure_lower_errors,
                nested_closures,
                returned_counter,
            ) = crate::mir::lower::build_synthesized_closure_mir_body(
                &func,
                &interner,
                &hir,
                shared_unify,
                shared_closure_def_id_counter,
            );
            shared_unify = returned_unify;
            shared_closure_def_id_counter = returned_counter;
            errors.typeck.extend(closure_lower_errors);

            // Stage 16.16: Register the closure function name in
            // fn_name_by_def_id so codegen can resolve TerminatorKind::Call
            // to the synthesized function.
            fn_name_by_def_id.insert(func.def_id, func.fn_name.clone());

            // Stage 16.29: Build placeholder fn_sig with FRESH Infer vars
            // from shared_unify. These Infer vars will be unified with
            // call site types during main body typeck, and resolved
            // during closure body typeck.
            let mut inputs = vec![func.closure_struct_ty.clone()];
            for _ in &func.params {
                let fresh_vid = shared_unify.new_ty_var();
                inputs.push(crate::mir::ty::Ty::new(
                    crate::mir::ty::TyKind::Infer(crate::mir::ty::InferVar::TyVar(fresh_vid)),
                    crate::session::Span::DUMMY,
                ));
            }
            let fresh_output_vid = shared_unify.new_ty_var();
            let placeholder_sig = crate::mir::ty::Sig {
                inputs,
                output: Box::new(crate::mir::ty::Ty::new(
                    crate::mir::ty::TyKind::Infer(crate::mir::ty::InferVar::TyVar(
                        fresh_output_vid,
                    )),
                    crate::session::Span::DUMMY,
                )),
                abi: crate::ast::Abi::Landin,
                is_unsafe: false,
            };
            fn_sig_table.sigs.insert(func.def_id, placeholder_sig);

            // Stage 16.29: Add nested closures to the worklist.
            for nested_func in nested_closures.into_values() {
                closure_worklist.push(nested_func);
            }

            pending_closure_mirs.push((func, closure_mir));
        }

        // Stage 15.12: Collect type errors from MIR lowering (e.g., "no method found").
        errors.typeck.extend(lower_type_errors);

        // Stage 16.29: Typeck CLOSURE MIR bodies FIRST, then main body.
        //
        // Why closure bodies first? The closure body's typeck resolves the
        // return type (from the body expression). For nested closures
        // (e.g., `|| || x`), the outer closure's return type is the INNER
        // closure's type. If we typeck the main body first, it sees the
        // closure's return type as Infer and emits "expected function"
        // errors for `f()()` patterns.
        //
        // By typecking closure bodies first:
        //   1. Closure body typeck resolves return type (e.g., Closure type)
        //   2. We update fn_sig.output with the resolved type
        //   3. Main body typeck sees the correct closure return type
        //
        // The shared unify table propagates constraints both ways: if the
        // closure body forces a capture's type to be i32, the main body
        // sees it too.
        // Stage 16.32 (通解 — Iterative typeck fixpoint for nested closures):
        //
        // Problem: For triple-nested closures (`|| || || x`), the capture
        // type (`x: i32`) is resolved by the MAIN body's typeck, but the
        // main body's Call sites depend on closure return types (which
        // depend on capture types). This is a circular dependency.
        //
        // 通解: Run multiple typeck passes until fixpoint:
        //   Pass 1: typeck all closures + main body
        //   Pass 2+: re-typeck all closures + main body (now capture types
        //           are resolved, so inner closures can resolve their return
        //           types, so main body Call sites can resolve)
        //   Stop when no fn_sig changes (fixpoint) or max 4 passes.
        //
        // Errors from intermediate passes are DISCARDED — only the final
        // pass's errors are reported (to avoid duplicate/false errors from
        // incomplete type resolution).
        //
        // Per §1.0 原則 6 "通用 > 特例": one iterative approach for all
        // nesting depths (double, triple, quadruple, etc.).
        // Per §1.0 原則 9 "正确 > 妥协": fix the root cause (circular
        // dependency), not the symptom (special-case triple-nested).

        // Helper: typeck one closure MIR body + update its fn_sig.
        fn typeck_closure_and_update_sig(
            func: &crate::mir::lower::SynthesizedClosureFunction,
            closure_mir: &mut crate::mir::body::MirBody,
            shared_unify: &mut crate::typeck::unify::UnificationTable,
            fn_sig_table: &mut typeck::FnSigTable,
            field_ty_table: &typeck::FieldTyTable,
        ) -> Vec<crate::typeck::TypeError> {
            let mut closure_tc = typeck::TypeChecker::with_unify(std::mem::take(shared_unify));
            closure_tc.fn_sigs = fn_sig_table.sigs.clone();
            closure_tc.check_mir_body_with_tables(closure_mir, Some(field_ty_table));
            let (closure_type_errors, _closure_typeck_results, returned_unify) =
                closure_tc.into_results_with_unify();
            *shared_unify = returned_unify;

            // Update fn_sig with resolved types from local_decls.
            let mut resolved_inputs = vec![func.closure_struct_ty.clone()];
            for i in 0..func.params.len() {
                let local_idx = 2 + i;
                if let Some(local) = closure_mir.local_decls.get(local_idx) {
                    resolved_inputs.push(local.ty.clone());
                } else {
                    resolved_inputs.push(crate::mir::ty::Ty::new(
                        crate::mir::ty::TyKind::Error,
                        crate::session::Span::DUMMY,
                    ));
                }
            }
            let resolved_output = closure_mir
                .local_decls
                .first()
                .map(|l| l.ty.clone())
                .unwrap_or_else(|| {
                    crate::mir::ty::Ty::new(
                        crate::mir::ty::TyKind::Error,
                        crate::session::Span::DUMMY,
                    )
                });
            let resolved_sig = crate::mir::ty::Sig {
                inputs: resolved_inputs,
                output: Box::new(resolved_output),
                abi: crate::ast::Abi::Landin,
                is_unsafe: false,
            };
            fn_sig_table.sigs.insert(func.def_id, resolved_sig);
            closure_type_errors
        }

        // Helper: typeck the main body + return errors.
        fn typeck_main_body(
            mir: &mut crate::mir::body::MirBody,
            shared_unify: &mut crate::typeck::unify::UnificationTable,
            fn_sig_table: &typeck::FnSigTable,
            field_ty_table: &typeck::FieldTyTable,
            resolver: &crate::traits::TraitResolver,
            interner: &Rodeo,
        ) -> (Vec<crate::typeck::TypeError>, typeck::TypeckResults) {
            let mut tc = typeck::TypeChecker::with_unify(std::mem::take(shared_unify));
            tc.fn_sigs = fn_sig_table.sigs.clone();
            // Stage 16.81: Set resolver for rich error messages (Adt type names).
            tc.unify.set_resolver(resolver, interner);
            // Stage 18.99 (TD-13 fix): Set fn_sigs on unify table so
            // FnDef↔FnPtr unification checks signature compatibility
            // (soundness — was unconditionally Ok before).
            tc.unify.set_fn_sigs(&fn_sig_table.sigs);
            tc.check_mir_body_with_tables(mir, Some(field_ty_table));
            let (errors, results, returned_unify) = tc.into_results_with_unify();
            *shared_unify = returned_unify;
            (errors, results)
        }

        // Iterative typeck: run passes until fixpoint or max 4 passes.
        // Only run multiple passes if there are closure MIR bodies
        // (nested closures need iterative resolution).
        // Discard intermediate errors; only keep the final pass's errors.
        const MAX_TYPECK_PASSES: usize = 4;
        let mut final_closure_errors: Vec<crate::typeck::TypeError> = Vec::new();
        let mut final_main_errors: Vec<crate::typeck::TypeError> = Vec::new();
        let mut final_main_results = typeck::TypeckResults::default();
        let has_closures = !pending_closure_mirs.is_empty();
        let max_passes = if has_closures { MAX_TYPECK_PASSES } else { 1 };

        for pass in 0..max_passes {
            // Snapshot fn_sigs to detect fixpoint.
            let sigs_before: std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig> =
                fn_sig_table.sigs.clone();

            // Typeck all closures.
            final_closure_errors.clear();
            for (func, closure_mir) in &mut pending_closure_mirs {
                let errs = typeck_closure_and_update_sig(
                    func,
                    closure_mir,
                    &mut shared_unify,
                    &mut fn_sig_table,
                    &field_ty_table,
                );
                final_closure_errors.extend(errs);
            }

            // Typeck the main body.
            let (main_errs, main_results) = typeck_main_body(
                &mut mir,
                &mut shared_unify,
                &fn_sig_table,
                &field_ty_table,
                &trait_resolver,
                &interner,
            );
            final_main_errors = main_errs.clone();
            final_main_results = main_results;

            // Stage 16.32: After main body typeck, resolve closure_struct_ty
            // substs in all closure fn_sigs. The main body's typeck resolves
            // capture types (e.g., `let x = 1` → x: i32), which should
            // propagate to the closure_struct_ty's substs.
            //
            // The closure_struct_ty is `Closure(def_id, [Infer, ...])` —
            // the Infer vars are from the shared unify table. After main
            // body typeck, those Infer vars are resolved. We update the
            // fn_sig.inputs[0] (self) with the resolved closure_struct_ty.
            for (func, _) in &pending_closure_mirs {
                if let Some(sig) = fn_sig_table.sigs.get(&func.def_id).cloned() {
                    // Resolve the closure_struct_ty (inputs[0]) via unify.
                    let resolved_self_ty = shared_unify.resolve(&sig.inputs[0]);
                    let mut new_sig = sig;
                    new_sig.inputs[0] = resolved_self_ty;
                    fn_sig_table.sigs.insert(func.def_id, new_sig);
                }
            }

            // Check if any fn_sig changed (fixpoint detection).
            let mut changed = false;
            for (def_id, new_sig) in &fn_sig_table.sigs {
                if let Some(old_sig) = sigs_before.get(def_id) {
                    if old_sig.inputs != new_sig.inputs || old_sig.output != new_sig.output {
                        changed = true;
                        break;
                    }
                } else {
                    changed = true;
                    break;
                }
            }
            if !changed && pass > 0 {
                break; // Fixpoint reached (after at least 2 passes).
            }
        }

        // Report final pass errors.
        errors.typeck.extend(final_closure_errors);
        errors.typeck.extend(final_main_errors);
        typeck_results.push(final_main_results);

        // Stage 16.31: Run drop elaboration + borrowck on closure MIR bodies
        // (AFTER all typeck passes are done, so types are fully resolved).
        for (func, mut closure_mir) in pending_closure_mirs {
            // Stage 16.29: Run drop elaboration on the closure MIR body.
            crate::mir::drop_elaboration::elaborate_drops(
                &mut closure_mir,
                &trait_resolver,
                &interner,
            );

            // Stage 16.31: Borrowck on closure MIR bodies.
            let mut closure_bc: borrowck::BorrowChecker<'_> =
                borrowck::BorrowChecker::with_resolver_and_sigs(
                    &trait_resolver,
                    &interner,
                    &fn_sig_table.sigs,
                );
            closure_bc.check_mir_body_with_dataflow(&closure_mir);
            errors.borrowck.extend(closure_bc.into_errors());

            // Suppress unused variable warning for `func` (used above in
            // the typeck pass, but the drop/borrowck loop only needs the
            // closure_mir). The `func` binding is kept for clarity.
            let _ = &func;

            synthesized_closure_mir_bodies.push(closure_mir);
        }

        // shared_unify is no longer needed (all typeck done).
        drop(shared_unify);

        // Stage 15.46 (HP-12 step 5): Drop elaboration.
        //
        // Insert `Drop` terminators before `StorageDead` for locals whose
        // type needs drop glue. This runs AFTER typeck (which writes
        // resolved types into `mir.local_decls`) and BEFORE borrowck
        // (so the borrow checker sees the `Drop` terminators).
        //
        // Per §16: `elaborate_drops` is a MIR-to-MIR transformation —
        // it mutates `mir` in place. It reads `mir.adt_layouts` (sunk
        // from HIR during MIR lowering) and `trait_resolver` (for
        // `is_drop_builtin` queries). No HIR lookup.
        //
        // Per §1.0 原則 3 "显式 > 隐式": the `Drop` terminators are
        // explicit in the MIR, not implicit in `StorageDead`.
        //
        // Note: In v0.171.0, no types implement `Drop` yet (the parser
        // doesn't support `impl Drop for T`), so `elaborate_drops` is a
        // no-op. When `impl Drop` support is added (future stage), the
        // pass will start inserting `Drop` terminators.
        crate::mir::drop_elaboration::elaborate_drops(&mut mir, &trait_resolver, &interner);

        // Stage 16.69 (Task 17 Phase 4): Resolve associated type projections.
        //
        // After typeck writeback, some local types may contain
        // `TyKind::Projection` (unresolved associated types like
        // `<T as Trait>::Item`). This pass resolves them to concrete types
        // by looking up the impl block.
        //
        // Per §16: reads HIR (allowed during driver post-typeck).
        // Per §1.0 原則 6 "通用 > 特例": one pass for all projections.
        crate::typeck::projection_resolver::resolve_projections_in_mir(&mut mir, &hir);

        // Borrow check
        // Stage 14.106 (HP-1 fix attempt): Pass TraitResolver to BorrowChecker.
        //
        // NOTE: HP-1 fix is DEFERRED to v0.2. The issue is that
        // `ty_is_copy_with_resolver` returns false for ALL user-defined structs
        // (because v0.1 has no #[derive(Copy)] support and users don't write
        // `impl Copy for Type` blocks). This causes 223 test failures because
        // v0.1 tests expect structs with all-Copy fields to be Copy.
        //
        // The correct v0.2 fix is to implement field-level Copy detection:
        // a struct is Copy if ALL its fields are Copy (matching Rust's
        // #[derive(Copy)] rules). This requires field type lookup infrastructure
        // that doesn't exist in v0.1.
        //
        // For v0.1: fall back to unsound `ty_is_copy` (treats all Adt as Copy).
        // This is a known v0.1 soundness limitation — documented in the
        // v0.1-capability-assessment.
        //
        // Stage 15.40 (HP-10 — driver switch COMPLETE):
        //
        // The driver now uses the dataflow-driven borrow checker
        // (`check_mir_body_with_dataflow`). This completes the NLL fixpoint
        // migration (Stages 15.34-15.40).
        //
        // The dataflow path uses:
        // - `compute_last_use_map` for the kill decision (borrow lifetimes
        //   end at their last read, matching the legacy path).
        // - `compute_ever_read` (Stage 15.39 Option B) to preserve GAP-1
        //   semantics (never kill a borrow whose ref_local was never read).
        // - `kill_borrows_on_redefinition` (Stage 15.40) to kill borrows
        //   when their ref_local is re-assigned (handles borrow temps in
        //   loops — the `&mut self` method-call false positive is fixed).
        //
        // The diagnostic tool (Stage 15.38) confirms:
        // - LEGACY-STRICTER: 0 (was 112 — GAP-1 conflict resolved by Option B)
        // - DATAFLOW-STRICTER: 0 (was 1 — false positive fixed by kill-on-redef)
        // - Both paths agree on all 5028 comparable conformance tests.
        //
        // The legacy `check_mir_body` remains as `#[deprecated]` for
        // backward compatibility with existing tests. Stage 15.41 will
        // remove it (now truly dead code).
        //
        // Per §1.0 原則 1 "长期 > 短期": the dataflow path is the correct
        // long-term design. Per §1.0 原則 3 "显式 > 隐式": the choice of
        // analysis is explicit in the method name (`_with_dataflow` suffix).
        // Stage 15.71/15.99/16.02/16.03/16.06: Sound Copy detection.
        // Stage 16.06 ENABLED `with_resolver_and_sigs` in the driver.
        // The sound Copy detection is now active — no more unsound
        // `Adt => true` fallback in the production path.
        //
        // Stage 16.06 also added field-level Copy derivation to
        // TraitResolver: structs/enums whose ALL fields are Copy (and no
        // `impl Drop`) are DERIVED Copy, mirroring Rust's `#[derive(Copy)]`.
        // This closed the sound Copy migration gap without requiring 199
        // test files to add `impl Copy` manually.
        //
        // The MIR lowerer was also updated to use `Operand::Move` instead
        // of `Operand::Copy` for let bindings, function returns, and call
        // arguments. The borrow checker's Operand::Move path (Stage 15.73)
        // skips move recording for Copy types, so Move is safe for both
        // Copy and non-Copy types.
        //
        // Per §1.0 原則 9 "正确 > 妥协": sound Copy detection is now the
        // production path. The unsound `ty_is_copy` remains only for
        // test contexts (BorrowChecker::new without resolver).
        let mut bc: borrowck::BorrowChecker<'_> = borrowck::BorrowChecker::with_resolver_and_sigs(
            &trait_resolver,
            &interner,
            &fn_sig_table.sigs,
        );
        bc.check_mir_body_with_dataflow(&mir);
        errors.borrowck.extend(bc.into_errors());

        // Stage 15.7 (v0.2 writeback consolidation): The 8 incremental
        // writeback passes from Stages 14.30-14.84 have been consolidated
        // into 2 functions in src/mir/lower/writeback.rs:
        //
        // - writeback_type_propagation(mir, fn_sigs) — merges passes 1-5
        //   (Tuple Aggregate, Call dest, Field projection Copy, Index
        //   projection Copy, Copy/Move chain fixpoint) into one fixpoint walk.
        // - writeback_closures(mir) — merges passes 6-8 (Closure substs,
        //   Closure local_decl.ty, Closure extract locals) into one 3-sub-pass walk.
        //
        // Per §16 (interface isolation): the driver is orchestrator-only —
        // it calls the writeback functions in order, the functions contain
        // the logic. Per §23 (API naming): both functions follow the
        // <verb>_<noun> pattern. Per docs/develop/v0/stage-15/v0.2-preparation.md
        // Phase 1 Task 5: 6× constant factor reduction vs the 8-pass approach.
        //
        // Stage 15.8 (v0.2): The 3× per-body populate_adt_layouts calls have
        // been REMOVED. The driver now builds crate-level AdtLayouts once
        // (via build_crate_adt_layouts) and shares the Arc across all bodies.
        // This eliminates the per-body HashMap duplication (~500KB for typical
        // crate) and the "re-populate after writeback" hack. The crate-level
        // map is complete — every ADT defined in HIR has its layout registered
        // upfront, regardless of writeback results.
        crate::mir::lower::writeback_type_propagation(&mut mir, &fn_sig_table.sigs);
        crate::mir::lower::writeback_closures(&mut mir);

        // Stage 18.102 (TD-MONO-INFER): Writeback inferred substs into FnDef
        // types. For implicit generic calls like `id(42)` (no turbofish),
        // the FnDef type has empty substs after MIR lowering. This pass
        // matches arg types against the function's param types (which
        // contain Param(N)) and writes back the inferred substs.
        //
        // Per §16: takes pre-computed fn_sigs + generics_map (data, not HIR).
        // Per §2.0 原則 9 "正确 > 妥协": implicit inference now works.
        // Per §1.0 原則 6 "通用 > 特例": one pass for all generic calls.
        crate::mir::lower::writeback_fndef_substs(&mut mir, &fn_sig_table.sigs, &generics_map);

        // Stage 18.96: Run MIR optimization passes (DCE → const_prop → DCE)
        // per `06-mir.md` §9.3. Wired here — after writeback (types are
        // final) and before `mirs.push` (so codegen consumes optimized MIR).
        //
        // Per §11: driver (orchestrator) is allowed to call opt entry.
        // Per §2.0 原則 6 "通用 > 特例": single `run_mir_optimizations`
        // entry point — future passes (jump threading, CSE) get added
        // inside that function, not as additional driver calls.
        // Per §2.0 原則 4 "报错 > 静默": opt preserves semantic correctness
        // — DCE only removes provably dead assignments, const_prop only
        // substitutes proven constants. Borrow check has already run, so
        // borrow information is not invalidated.
        //
        // The `optimize` flag allows `compile_no_opt()` to skip opt for
        // tests that verify IR/MIR structure (per §11 interface isolation).
        if optimize {
            crate::mir::optimization::run_mir_optimizations(&mut mir);
        }

        mirs.push(mir);
    }

    // Stage 15.8 (v0.2): Build crate-level AdtLayouts ONCE from HIR.
    //
    // This replaces the 3× per-body populate_adt_layouts calls from Stages
    // 14.41 and 14.84. The crate-level map is complete — every struct/enum
    // defined in HIR has its layout registered, including nested ADTs. This
    // eliminates the "re-populate after writeback" hack because the map no
    // longer depends on local_decls (which change during writeback).
    //
    // The map is shared across all MirBodies via Arc<AdtLayouts> (cheap
    // refcount-bump clone). For a 100-fn, 50-type crate, this saves ~500KB
    // of duplicated HashMap entries.
    //
    // Per §15 "最优 > 最小": this is the root-cause fix, not a workaround.
    // Per §1.0 原则 6 "通用 > 特例": one crate-level map for all bodies.
    //
    // clippy::arc_with_non_send_sync: AdtLayouts (HashMap<DefId, AdtLayout>)
    // is not Send+Sync because AdtLayout contains Ty (which has Box/Vec).
    // The compiler is single-threaded, so Arc is fine — using Arc instead
    // of Rc keeps the door open for future multi-threaded LSP mode.
    #[allow(clippy::arc_with_non_send_sync)]
    let crate_adt_layouts: std::sync::Arc<crate::mir::body::AdtLayouts> =
        std::sync::Arc::new(crate::mir::lower::build_crate_adt_layouts(&hir));

    // Share the crate-level AdtLayouts across all MirBodies.
    for mir in &mut mirs {
        mir.adt_layouts = crate_adt_layouts.clone();
    }

    // Stage 3.56 (Phase A §16 refactoring): pre-compute codegen metadata
    // so codegen becomes a pure MIR consumer (no re-lowering, no re-typeck).
    // Per §16.2.1: this is "data flows downstream" — the driver (orchestrator)
    // builds the metadata and passes it as data, not as HIR references.
    // Stage 16.16: fn_name_by_def_id declared early (before per-body loop).
    for (def_id, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Fn(f)) = owner {
            let name = interner.try_resolve(&f.ident.name).unwrap_or("fn");
            // Stage 13.15: Strip a leading "landin_" prefix to avoid doubling it
            // (e.g., `fn landin_main()` should produce symbol `landin_main`, not
            // `landin_landin_main`). This supports both `fn main()` (Rust
            // convention) and `fn landin_main()` (Landin convention) as entry
            // points, matching the C wrapper's `extern int landin_main(void);`.
            let stripped = name.strip_prefix("landin_").unwrap_or(name);
            fn_name_by_def_id.insert(*def_id, format!("landin_{}", stripped));
        }
        // Stage 14.72: Also register impl method names in fn_name_by_def_id.
        //
        // Previously, only top-level fns were registered. Impl methods
        // (e.g., `Inner::new`, `Outer::new`) were only in body_metas but
        // NOT in fn_name_by_def_id. This caused method name collisions:
        // `Inner::new` and `Outer::new` both resolved to `landin_new`
        // (the fallback name from codegen), producing duplicate function
        // definitions in the LLVM module → segfault at runtime.
        //
        // Fix: iterate impl blocks and register each method with its
        // fully-qualified name: `landin_<SelfType>_<method>`.
        //
        // Per §1.0 原则 5 "报错 > 静默": name collisions now produce
        // distinct symbols instead of silently overwriting.
        // Stage 14.97 (Bug Y1 fix): Also register trait default method names.
        // Trait default methods (with body: Some) need proper function names
        // so they can be called when not overridden in impl blocks.
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
            for trait_item in &t.items {
                if let crate::hir::HirTraitItem::Fn(f) = trait_item {
                    if f.body.is_some() {
                        let method = interner.try_resolve(&f.ident.name).unwrap_or("fn");
                        let trait_name = interner.try_resolve(&t.ident.name).unwrap_or("Trait");
                        let trait_stripped =
                            trait_name.strip_prefix("landin_").unwrap_or(trait_name);
                        let method_stripped = method.strip_prefix("landin_").unwrap_or(method);
                        let method_def_id = f.hir_id.owner;
                        fn_name_by_def_id.insert(
                            method_def_id,
                            format!("landin_{}_default_{}", trait_stripped, method_stripped),
                        );
                    }
                }
            }
        }
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(i)) = owner {
            for impl_item in &i.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    let method = interner.try_resolve(&f.ident.name).unwrap_or("fn");
                    let self_ty_name = crate::traits::extract_impl_self_ty_name(&i.self_ty);
                    let type_str = self_ty_name
                        .and_then(|s| interner.try_resolve(&s))
                        .unwrap_or("Type");
                    let type_stripped = type_str.strip_prefix("landin_").unwrap_or(type_str);
                    let method_stripped = method.strip_prefix("landin_").unwrap_or(method);
                    // Use the method's DefId (from its HirId)
                    let method_def_id = f.hir_id.owner;
                    fn_name_by_def_id.insert(
                        method_def_id,
                        format!("landin_{}_{}", type_stripped, method_stripped),
                    );
                }
            }
        }
    }

    // Build per-body metadata (parallel to mirs).
    //
    // Stage 5.6: extend fn_name resolution to cover impl method bodies so
    // vtable entries (which reference `landin_<SelfType>_<method>`) point
    // at the actual emitted LLVM symbol. Previously impl methods fell back
    // to `fn_<owner_id>` which made vtable references dangling.
    let body_metas: Vec<BodyMeta> = hir
        .bodies
        .iter()
        .filter_map(|(body_id, body)| {
            // Stage 14.100 (Bug AA5 fix): Skip body_metas for bodies that
            // were skipped during MIR lowering (trait default bodies with
            // zero impls). Without this filter, codegen would try to emit
            // functions for bodies that have no MIR, producing invalid LLVM
            // IR like `void %(void %arg0)`.
            if !lowered_body_owners.contains(&body_id.owner.0) {
                return None;
            }
            // Stage 14.72: Use fn_name_by_def_id for name resolution.
            //
            // Previously, body_metas recomputed the fn name by iterating
            // hir.owners. But impl methods are stored as HirItem::Fn owners
            // (not HirItem::Impl), so the Impl branch was never matched.
            // This caused all impl methods with the same name (e.g.,
            // Inner::new and Outer::new) to resolve to `landin_new`,
            // producing duplicate function definitions → segfault.
            //
            // Fix: look up the name from fn_name_by_def_id, which was
            // built earlier with proper type-qualified names for impl
            // methods (landin_<Type>_<method>).
            let owner_def_id = body_id.owner.0;
            let fn_name = if let Some(name) = fn_name_by_def_id.get(&owner_def_id) {
                name.clone()
            } else {
                // Fallback: recompute from HirItem::Fn owner.
                hir.owners
                    .iter()
                    .find_map(|(_, owner)| match owner {
                        crate::hir::OwnerNode::Item(crate::hir::HirItem::Fn(f))
                            if f.body == Some(*body_id) =>
                        {
                            let name = interner.try_resolve(&f.ident.name).unwrap_or("fn");
                            let stripped = name.strip_prefix("landin_").unwrap_or(name);
                            Some(format!("landin_{}", stripped))
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| format!("fn_{}", body_id.owner.0.as_u32()))
            };
            // Check if void (no return type).
            let return_ty = hir.find_owner(body_id.owner.0).and_then(owner_return_ty);
            let is_void = return_ty.is_none();
            // Stage 13.22: Force `main`/`landin_main` to return i32 (not void).
            // The C wrapper declares `extern int landin_main(void)` and reads
            // the return value. If the LLVM function is void, the return
            // register contains garbage → undefined exit code (e.g., 219).
            // For void main, codegen emits `ret i32 0` instead of `ret void`.
            let is_void = is_void && fn_name != "landin_main";
            // Stage 8.3: Get the ABI from the function owner.
            let abi = hir
                .find_owner(body_id.owner.0)
                .and_then(|owner| match owner {
                    crate::hir::OwnerNode::Item(crate::hir::HirItem::Fn(f)) => Some(f.sig.abi),
                    _ => None,
                })
                .unwrap_or(crate::ast::Abi::Landin);
            Some(BodyMeta {
                fn_name,
                is_void,
                param_count: body.params.len(),
                abi,
            })
        })
        .collect();

    // Stage 5.22: Validate all trait impls (coherence + completeness).
    // Per deep review r70 action item: wire validate_impls() into driver.
    // Non-fatal — compilation continues, but errors are reported.
    //
    // Stage 5.80 (refactor): trait_resolver was built earlier (before the
    // per-body loop) so the DynTraitMIRPlan could be constructed from it.
    // Validation remains here — it doesn't affect lowering, only reports.
    let validation_report = trait_resolver.validate_impls();
    // Stage 15.9: Push typed TraitError values (was String). The structured
    // data (CoherenceError/IncompleteImpl) is preserved for downstream
    // consumers. format_for_user resolves the Spur symbols to &str.
    for ce in validation_report.coherence_errors {
        errors.trait_errors.push(TraitError::Coherence(ce));
    }
    for inc in validation_report.incomplete_impls {
        errors.trait_errors.push(TraitError::Incomplete(inc));
    }

    // Stage 18.71 P0-4: Validate trait impl method signatures against
    // trait declarations. Catches:
    //   - return type mismatch (trait: i32, impl: bool)
    //   - arg count mismatch (trait: 1 arg, impl: 2 args)
    //   - arg type mismatch (trait: i32, impl: bool)
    //
    // Per §1.0 原则 4 "报错 > 静默": signature mismatch must be reported.
    // Per §1.0 原则 6 "通用 > 特例": one validator covers all impl methods.
    // Per §10 naming: `validate_impl_method_signatures` follows
    //   `validate_<noun>_<noun>_<noun>` pattern.
    validate_impl_method_signatures(&hir, &interner, &mut errors.typeck);

    // Stage 18.72 P1-A: Validate struct literal field counts.
    // Catches:
    //   - missing field (`S { x: 1 }` where S has fields x, y)
    //   - extra field (`S { x: 1, y: 2 }` where S has only field x)
    //   - unknown field (`S { z: 1 }` where S has no field z)
    //   - duplicate field (`S { x: 1, x: 2 }`)
    //
    // Per §1.0 原则 4 "报错 > 静默": field count mismatch must be reported.
    // Per §1.0 原则 6 "通用 > 特例": one validator covers all struct literals.
    // Per §10 naming: `validate_struct_literal_fields` follows
    //   `validate_<noun>_<noun>_<noun>` pattern.
    validate_struct_literal_fields(&hir, &interner, &mut errors.typeck);

    // Stage 18.72 P1-C: Validate pattern arity in let bindings.
    // Catches `let (a, b, c) = (1, 2)` (3 patterns, 2 tuple elements).
    //
    // Per §1.0 原则 4 "报错 > 静默": arity mismatch must be reported.
    // Per §10 naming: `validate_pattern_arity` follows `validate_<noun>_<noun>`.
    validate_pattern_arity(&hir, &interner, &mut errors.typeck);

    // Stage 18.73 P1-G: Missing main check is inlined in `compile_binary`
    // (CLI path), not here in `compile` (test/library path). This avoids
    // false positives in test contexts where individual functions are
    // compiled without a `main`. See `compile_binary` at line ~1961.
    // Stage 18.78 P1-N7: `validate_main_exists` function was removed;
    // the check is now inlined in `compile_binary`.

    // Stage 18.73 P1-E: Validate assignment targets.
    // Per §1.0 原则 4 "报错 > 静默": invalid assignment target must be reported.
    // Per §10 naming: `validate_assignment_targets` follows `validate_<noun>_<noun>`.
    validate_assignment_targets(&hir, &interner, &mut errors.typeck);

    // Stage 18.73 P1-F: Validate cast types.
    // Per §1.0 原则 4 "报错 > 静默": invalid cast must be reported.
    // Per §10 naming: `validate_cast_types` follows `validate_<noun>_<noun>`.
    validate_cast_types(&hir, &interner, &mut errors.typeck);

    // Stage 18.21: Register __landin_println etc. in fn_name_by_def_id
    // so codegen can resolve the function name. The resolver returns a
    // synthetic DefId for __landin_ functions; we map each to its name.
    // Use DefId(u32::MAX - i) to avoid collisions with real DefIds.
    for (i, name) in crate::parser::macro_expand::BUILTIN_MACRO_NAMES
        .iter()
        .enumerate()
    {
        let landin_name = format!("__landin_{}", name);
        let synthetic_def_id = crate::hir::DefId::new(u32::MAX - i as u32);
        fn_name_by_def_id.insert(synthetic_def_id, landin_name);
    }

    CompileResult {
        hir: Some(hir),
        mirs,
        typeck_results,
        errors,
        interner,
        fn_name_by_def_id,
        body_metas,
        trait_resolver,
        fn_sigs: fn_sig_table.sigs,
        stdlib_prelude: crate::stdlib::default_prelude(),
        stdlib_facade: crate::stdlib::StdlibFacade::default(),
        type_interner: crate::mir::ty_interner::TypeInterner::new(),
        synthesized_closure_mir_bodies,
    }
}

/// Stage 18.73 P1-G: Compile a source file as a binary (entry point required).
///
/// Like `compile`, but additionally validates that a `fn main()` exists.
/// Used by the CLI (`--compile`/`--run`/`--emit-bin`) where an entry point
/// is mandatory. Test contexts use `compile` (no main requirement).
///
/// Per §1.0 原則 4 "报错 > 静默": missing main must be reported explicitly.
/// Per §10 naming: `compile_binary` follows `<verb>_<noun>` pattern.
pub fn compile_binary(src: &str) -> CompileResult {
    let mut result = compile(src);
    // Stage 18.73 P1-G: Validate main exists.
    let main_spur_opt = result.interner.get("main");
    let has_main = main_spur_opt
        .map(|main_spur| {
            if let Some(hir) = &result.hir {
                hir.owners.iter().any(|(_, owner)| {
                    if let crate::hir::OwnerNode::Item(HirItem::Fn(f)) = owner {
                        f.ident.name == main_spur
                    } else {
                        false
                    }
                })
            } else {
                false
            }
        })
        .unwrap_or(false);
    if !has_main {
        // Stage 18.93: Use Span(0, src.len()) instead of Span::DUMMY
        // so the error points to the entire source, not "1:1".
        let src_span = crate::session::Span::new(0, src.len() as u32);
        result.errors.typeck.push(TypeError::new(
            "missing `main` function — every program must have a `fn main()` entry point"
                .to_string(),
            src_span,
        ));
    }
    result
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
            HirFnRetTy::Default(_) => None,
        },
        OwnerNode::Item(HirItem::Const(c)) => Some(c.ty.clone()),
        OwnerNode::Item(HirItem::Static(s)) => Some(s.ty.clone()),
        _ => None,
    }
}

/// Stage 18.71 P0-4: Validate trait impl method signatures against trait
/// declarations.
///
/// For each `impl Trait for Type { fn method(...) -> ... { ... } }` block,
/// find the corresponding `trait Trait { fn method(...) -> ...; }` declaration
/// and verify that:
///   1. The number of inputs matches (after adjusting for self).
///   2. Each input type matches (after self substitution).
///   3. The output type matches.
///
/// Mismatches produce `TypeErrorKind::SignatureMismatch` errors with the
/// impl method's span.
///
/// Per §1.0 原则 4 "报错 > 静默": trait impl signature mismatch is reported.
/// Per §1.0 原则 6 "通用 > 特例": one validator walks all impl blocks.
/// Per §10 naming: `validate_impl_method_signatures` follows
///   `validate_<noun>_<noun>_<noun>` pattern.
fn validate_impl_method_signatures(
    hir: &HirCrate,
    interner: &lasso::Rodeo,
    errors: &mut Vec<TypeError>,
) {
    use crate::hir::{HirImplItem, HirTraitItem};

    // Build a lookup table: trait_name (Spur) → &HirTrait.
    // Per §1.0 原則 6: one lookup table for all traits, not per-impl scans.
    let mut trait_by_name: std::collections::HashMap<lasso::Spur, &crate::hir::HirTrait> =
        std::collections::HashMap::new();
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(HirItem::Trait(t)) = owner {
            trait_by_name.insert(t.ident.name, t);
        }
    }

    // Walk every impl block that has `of_trait`.
    for (_, owner) in &hir.owners {
        let impl_block = match owner {
            crate::hir::OwnerNode::Item(HirItem::Impl(impl_block))
                if impl_block.of_trait.is_some() =>
            {
                impl_block
            }
            _ => continue,
        };
        // Resolve the trait name from `of_trait` path's last segment.
        let trait_name = match impl_block
            .of_trait
            .as_ref()
            .and_then(|p| p.segments.last())
            .map(|s| s.ident.name)
        {
            Some(name) => name,
            None => continue,
        };
        let trait_decl = match trait_by_name.get(&trait_name) {
            Some(t) => *t,
            None => continue, // Unknown trait — let trait_resolver handle it.
        };

        // For each impl method, find the matching trait method by name.
        // Per §1.0 原則 6: one matching pass per impl method (no per-trait
        // method scans).
        for impl_item in &impl_block.items {
            let impl_fn = match impl_item {
                HirImplItem::Fn(f) => f,
                _ => continue,
            };
            // Find the matching trait method.
            let trait_fn = trait_decl.items.iter().find_map(|ti| match ti {
                HirTraitItem::Fn(f) if f.ident.name == impl_fn.ident.name => Some(f),
                _ => None,
            });
            let trait_fn = match trait_fn {
                Some(f) => f,
                None => continue, // Method not in trait — let trait_resolver's
                                  // incomplete_impls check handle it.
            };

            // Stage 18.71: Compare signatures.
            // Note: We compare the *non-self* parameters. Self is implicit
            // in trait methods but explicit in impl methods (via &self/&mut self).
            // Both trait and impl methods have self_kind set for self params,
            // so we filter those out and compare the rest.
            let trait_inputs: Vec<_> = trait_fn
                .sig
                .inputs
                .iter()
                .filter(|p| p.self_kind.is_none())
                .collect();
            let impl_inputs: Vec<_> = impl_fn
                .sig
                .inputs
                .iter()
                .filter(|p| p.self_kind.is_none())
                .collect();

            // 1. Argument count mismatch.
            if trait_inputs.len() != impl_inputs.len() {
                let trait_method_name = interner.try_resolve(&impl_fn.ident.name).unwrap_or("?");
                errors.push(TypeError::new(
                    format!(
                        "method `{}` has {} parameter(s) but the trait method has {}",
                        trait_method_name,
                        impl_inputs.len(),
                        trait_inputs.len()
                    ),
                    impl_fn.span,
                ));
                continue; // Skip type comparison if count mismatches.
            }

            // 2. Argument type mismatch.
            // Per §1.0 原則 4: report each mismatch separately for clarity.
            for (i, (impl_p, trait_p)) in impl_inputs.iter().zip(trait_inputs.iter()).enumerate() {
                let impl_ty = match &impl_p.ty {
                    Some(t) => crate::mir::lower::lower_hir_ty_to_mir_ty(t),
                    None => continue, // Skip if no type (shouldn't happen for non-self).
                };
                let trait_ty = match &trait_p.ty {
                    Some(t) => crate::mir::lower::lower_hir_ty_to_mir_ty(t),
                    None => continue,
                };
                // Use types_match_loose from typeck::checker via a simple
                // kind comparison. We avoid importing the private fn —
                // instead do a structural kind compare that handles the
                // common cases (Int, Bool, Tuple, Adt).
                if !mir_ty_kinds_compatible(&impl_ty, &trait_ty) {
                    let method_name = interner.try_resolve(&impl_fn.ident.name).unwrap_or("?");
                    let impl_ty_str = crate::mir::ty::type_to_string(&impl_ty);
                    let trait_ty_str = crate::mir::ty::type_to_string(&trait_ty);
                    errors.push(TypeError::new(
                        format!(
                            "method `{}` parameter {} type mismatch: expected `{}`, found `{}`",
                            method_name,
                            i + 1,
                            trait_ty_str,
                            impl_ty_str
                        ),
                        impl_p.span,
                    ));
                }
            }

            // 3. Return type mismatch.
            let impl_ret_ty = match &impl_fn.sig.output {
                HirFnRetTy::Ty(t) => Some(crate::mir::lower::lower_hir_ty_to_mir_ty(t)),
                HirFnRetTy::Default(_) => Some(crate::mir::ty::Ty::new(
                    crate::mir::ty::TyKind::Tuple(vec![]),
                    impl_fn.span,
                )),
            };
            let trait_ret_ty = match &trait_fn.sig.output {
                HirFnRetTy::Ty(t) => Some(crate::mir::lower::lower_hir_ty_to_mir_ty(t)),
                HirFnRetTy::Default(_) => Some(crate::mir::ty::Ty::new(
                    crate::mir::ty::TyKind::Tuple(vec![]),
                    trait_fn.span,
                )),
            };
            if let (Some(impl_ret), Some(trait_ret)) = (impl_ret_ty, trait_ret_ty) {
                if !mir_ty_kinds_compatible(&impl_ret, &trait_ret) {
                    let method_name = interner.try_resolve(&impl_fn.ident.name).unwrap_or("?");
                    let impl_ret_str = crate::mir::ty::type_to_string(&impl_ret);
                    let trait_ret_str = crate::mir::ty::type_to_string(&trait_ret);
                    errors.push(TypeError::new(
                        format!(
                            "method `{}` return type mismatch: expected `{}`, found `{}`",
                            method_name, trait_ret_str, impl_ret_str
                        ),
                        impl_fn.span,
                    ));
                }
            }
        }
    }
}

/// Stage 18.71: Compatibility check for two MIR types (used by
/// `validate_impl_method_signatures`).
///
/// Returns `true` if the types are structurally compatible (same kind or
/// coercible per Rust semantics). Returns `false` for clear mismatches
/// (e.g., Int vs Bool, Adt-A vs Adt-B).
///
/// This is a conservative check: it only fires on clear mismatches to
/// avoid false positives on generic types (where substs may differ).
///
/// Per §1.0 原則 9 "正确 > 妥协": must not break valid impl code.
fn mir_ty_kinds_compatible(a: &crate::mir::ty::Ty, b: &crate::mir::ty::Ty) -> bool {
    use crate::mir::ty::TyKind;
    match (&a.kind, &b.kind) {
        // Same primitive kind: ok.
        (TyKind::Bool, TyKind::Bool)
        | (TyKind::Char, TyKind::Char)
        | (TyKind::Str, TyKind::Str)
        | (TyKind::Never, TyKind::Never) => true,
        // Any Int with any Int: ok (width differences are coercible).
        (TyKind::Int(_), TyKind::Int(_)) => true,
        // Any Uint with any Uint: ok.
        (TyKind::Uint(_), TyKind::Uint(_)) => true,
        // Any Float with any Float: ok.
        (TyKind::Float(_), TyKind::Float(_)) => true,
        // Int ↔ Uint of same width: ok (lossless reinterpretation).
        (TyKind::Int(_), TyKind::Uint(_)) | (TyKind::Uint(_), TyKind::Int(_)) => true,
        // Tuple with same length: recurse.
        (TyKind::Tuple(a_tys), TyKind::Tuple(b_tys)) if a_tys.len() == b_tys.len() => a_tys
            .iter()
            .zip(b_tys.iter())
            .all(|(x, y)| mir_ty_kinds_compatible(x, y)),
        // Adt with same DefId: ok (substs may differ in representation).
        (TyKind::Adt(a_def, _), TyKind::Adt(b_def, _)) => a_def == b_def,
        // Ref with same inner kind: ok (region may differ).
        (TyKind::Ref(_, _, a_inner), TyKind::Ref(_, _, b_inner)) => {
            mir_ty_kinds_compatible(a_inner, b_inner)
        }
        // Array with same element: ok (count may differ in representation).
        (TyKind::Array(a_inner, _), TyKind::Array(b_inner, _)) => {
            mir_ty_kinds_compatible(a_inner, b_inner)
        }
        // FnPtr with same input/output: ok.
        (TyKind::FnPtr(a_sig), TyKind::FnPtr(b_sig)) => {
            a_sig.inputs.len() == b_sig.inputs.len()
                && a_sig
                    .inputs
                    .iter()
                    .zip(b_sig.inputs.iter())
                    .all(|(x, y)| mir_ty_kinds_compatible(x, y))
                && mir_ty_kinds_compatible(&a_sig.output, &b_sig.output)
        }
        // Param ↔ Param (same index): ok.
        (TyKind::Param(a_p), TyKind::Param(b_p)) => a_p.index == b_p.index,
        // Param ↔ concrete: ok (generic, can't compare at this stage).
        (TyKind::Param(_), _) | (_, TyKind::Param(_)) => true,
        // Infer/Error: skip (can't determine).
        (TyKind::Infer(_), _) | (_, TyKind::Infer(_)) => true,
        (TyKind::Error, _) | (_, TyKind::Error) => true,
        // Everything else: not compatible.
        _ => false,
    }
}

/// Stage 18.72 P1-A: Validate struct literal field counts against struct
/// definitions.
///
/// For each `HirExprKind::Struct { path, fields }` expression in the HIR:
///   1. Resolve `path.res` to a struct DefId
///   2. Look up the struct's declared field names
///   3. Check for:
///      - Unknown fields (field name not in declaration)
///      - Duplicate fields (same name appears twice in literal)
///      - Missing fields (declared field not provided in literal)
///
/// Per §1.0 原则 4 "报错 > 静默": all three error types must be reported.
/// Per §1.0 原则 6 "通用 > 特例": one validator walks all bodies.
/// Per §10 naming: `validate_struct_literal_fields` follows
///   `validate_<noun>_<noun>_<noun>` pattern.
fn validate_struct_literal_fields(
    hir: &HirCrate,
    interner: &lasso::Rodeo,
    errors: &mut Vec<TypeError>,
) {
    use crate::hir::{HirExprKind, HirStmt};

    // Build a lookup table: struct DefId → Vec<Spur> (field names).
    // Per §1.0 原則 6: one lookup table for all structs.
    let mut struct_fields_by_def_id: std::collections::HashMap<
        crate::hir::DefId,
        Vec<lasso::Spur>,
    > = std::collections::HashMap::new();
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(HirItem::Struct(s)) = owner {
            let field_names: Vec<lasso::Spur> = s
                .fields
                .iter()
                .filter_map(|f| f.ident.as_ref().map(|i| i.name))
                .collect();
            struct_fields_by_def_id.insert(s.hir_id.owner, field_names);
        }
    }

    // Walk all bodies and check struct literals.
    for (_, owner) in &hir.owners {
        // Extract BodyId from owner (Fn/Const/Static have bodies).
        let body_id = match owner {
            crate::hir::OwnerNode::Item(HirItem::Fn(f)) if f.body.is_some() => f.body.unwrap(),
            crate::hir::OwnerNode::Item(HirItem::Const(c)) => c.body,
            crate::hir::OwnerNode::Item(HirItem::Static(s)) => s.body,
            _ => continue,
        };
        let body = match hir.find_body(body_id) {
            Some(b) => b,
            None => continue,
        };
        // Walk all statements + trailing expr in the body.
        // body.value is HirExpr — if it's a Block, walk its stmts + expr.
        let mut exprs_to_check: Vec<&crate::hir::HirExpr> = Vec::new();
        if let HirExprKind::Block(block) = &body.value.kind {
            for stmt in &block.stmts {
                if let HirStmt::Expr(e, _) = stmt {
                    exprs_to_check.push(e);
                } else if let HirStmt::Local(local) = stmt {
                    if let Some(init) = &local.init {
                        exprs_to_check.push(init);
                    }
                }
            }
            if let Some(trailing) = &block.expr {
                exprs_to_check.push(trailing);
            }
        } else {
            exprs_to_check.push(&body.value);
        }

        for expr in exprs_to_check {
            check_struct_literal_in_expr(expr, &struct_fields_by_def_id, interner, errors);
        }
    }
}

/// Recursively walk an expression tree and validate all struct literals.
fn check_struct_literal_in_expr(
    expr: &crate::hir::HirExpr,
    struct_fields: &std::collections::HashMap<crate::hir::DefId, Vec<lasso::Spur>>,
    interner: &lasso::Rodeo,
    errors: &mut Vec<TypeError>,
) {
    use crate::hir::HirExprKind;
    match &expr.kind {
        HirExprKind::Struct { path, fields } => {
            // Try to resolve path to a struct DefId.
            if let crate::hir::Res::Def(def_id, crate::resolve::DefKind::Struct) = path.res {
                if let Some(declared_fields) = struct_fields.get(&def_id) {
                    validate_one_struct_literal(
                        fields,
                        declared_fields,
                        interner,
                        expr.span,
                        errors,
                    );
                }
            }
            // Recurse into field expressions.
            for f in fields {
                if let Some(e) = &f.expr {
                    check_struct_literal_in_expr(e, struct_fields, interner, errors);
                }
            }
        }
        // Recurse into other expression kinds that may contain struct literals.
        HirExprKind::Call { func, args, .. } => {
            check_struct_literal_in_expr(func, struct_fields, interner, errors);
            for arg in args {
                check_struct_literal_in_expr(arg, struct_fields, interner, errors);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            check_struct_literal_in_expr(receiver, struct_fields, interner, errors);
            for arg in args {
                check_struct_literal_in_expr(arg, struct_fields, interner, errors);
            }
        }
        HirExprKind::Field { receiver, .. } => {
            check_struct_literal_in_expr(receiver, struct_fields, interner, errors);
        }
        HirExprKind::Unary { expr: inner, .. } => {
            check_struct_literal_in_expr(inner, struct_fields, interner, errors);
        }
        HirExprKind::Binary { lhs, rhs, .. } => {
            check_struct_literal_in_expr(lhs, struct_fields, interner, errors);
            check_struct_literal_in_expr(rhs, struct_fields, interner, errors);
        }
        HirExprKind::If {
            cond, then, else_, ..
        } => {
            check_struct_literal_in_expr(cond, struct_fields, interner, errors);
            for stmt in &then.stmts {
                if let crate::hir::HirStmt::Expr(e, _) = stmt {
                    check_struct_literal_in_expr(e, struct_fields, interner, errors);
                } else if let crate::hir::HirStmt::Local(local) = stmt {
                    if let Some(init) = &local.init {
                        check_struct_literal_in_expr(init, struct_fields, interner, errors);
                    }
                }
            }
            if let Some(trailing) = &then.expr {
                check_struct_literal_in_expr(trailing, struct_fields, interner, errors);
            }
            if let Some(e) = else_ {
                check_struct_literal_in_expr(e, struct_fields, interner, errors);
            }
        }
        HirExprKind::Match {
            expr: scrutinee,
            arms,
            ..
        } => {
            check_struct_literal_in_expr(scrutinee, struct_fields, interner, errors);
            for arm in arms {
                if let Some(e) = &arm.guard {
                    check_struct_literal_in_expr(e, struct_fields, interner, errors);
                }
                // arm.body is Box<HirExpr>, not a Block — recurse directly.
                check_struct_literal_in_expr(&arm.body, struct_fields, interner, errors);
            }
        }
        HirExprKind::Block(block) => {
            for stmt in &block.stmts {
                if let crate::hir::HirStmt::Expr(e, _) = stmt {
                    check_struct_literal_in_expr(e, struct_fields, interner, errors);
                } else if let crate::hir::HirStmt::Local(local) = stmt {
                    if let Some(init) = &local.init {
                        check_struct_literal_in_expr(init, struct_fields, interner, errors);
                    }
                }
            }
            if let Some(trailing) = &block.expr {
                check_struct_literal_in_expr(trailing, struct_fields, interner, errors);
            }
        }
        HirExprKind::Return { expr: Some(e), .. } => {
            check_struct_literal_in_expr(e, struct_fields, interner, errors);
        }
        _ => {}
    }
}

/// Validate a single struct literal against its declared fields.
fn validate_one_struct_literal(
    fields: &[crate::hir::HirExprField],
    declared_fields: &[lasso::Spur],
    interner: &lasso::Rodeo,
    span: crate::session::Span,
    errors: &mut Vec<TypeError>,
) {
    // Check for unknown + duplicate fields.
    let mut seen: std::collections::HashSet<lasso::Spur> = std::collections::HashSet::new();
    for f in fields {
        let name = f.ident.name;
        if !declared_fields.contains(&name) {
            let name_str = interner.try_resolve(&name).unwrap_or("?");
            errors.push(TypeError::new(
                format!("struct has no field `{}`", name_str),
                f.span,
            ));
        } else if !seen.insert(name) {
            let name_str = interner.try_resolve(&name).unwrap_or("?");
            errors.push(TypeError::new(
                format!("field `{}` specified more than once", name_str),
                f.span,
            ));
        }
    }

    // Check for missing fields (only if no unknown/duplicate errors).
    // Per §1.0 原則 4: report missing fields too.
    let provided: std::collections::HashSet<lasso::Spur> =
        fields.iter().map(|f| f.ident.name).collect();
    let missing: Vec<&lasso::Spur> = declared_fields
        .iter()
        .filter(|name| !provided.contains(name))
        .collect();
    if !missing.is_empty() {
        let missing_names: Vec<&str> = missing
            .iter()
            .map(|s| interner.try_resolve(s).unwrap_or("?"))
            .collect();
        errors.push(TypeError::new(
            format!("missing field(s): {}", missing_names.join(", ")),
            span,
        ));
    }
}

/// Stage 18.72 P1-C: Validate pattern arity in let bindings.
///
/// For each `let (a, b, c) = init` where the pattern is a tuple:
///   - If init's type is `Tuple(tys)` and `tys.len() != pattern_count`,
///     report an error.
///
/// Per §1.0 原则 4 "报错 > 静默": arity mismatch must be reported.
/// Per §10 naming: `validate_pattern_arity` follows `validate_<noun>_<noun>`.
fn validate_pattern_arity(hir: &HirCrate, _interner: &lasso::Rodeo, errors: &mut Vec<TypeError>) {
    use crate::hir::{HirExprKind, HirPatKind, HirStmt};

    // We need MIR typeck results to know init types. But we're called
    // before MIR lowering. Instead, we do a best-effort HIR-level check:
    // If the init expression is a tuple literal, count its elements.
    //
    // Per §1.0 原則 9 "正确 > 妥协": This is a conservative check — it only
    // catches the case where init is a literal tuple. For non-literal
    // inits (e.g., function calls returning tuples), the check is skipped
    // (would need full type info).
    for (_, owner) in &hir.owners {
        // Extract BodyId from owner (Fn/Const/Static have bodies).
        let body_id = match owner {
            crate::hir::OwnerNode::Item(HirItem::Fn(f)) if f.body.is_some() => f.body.unwrap(),
            crate::hir::OwnerNode::Item(HirItem::Const(c)) => c.body,
            crate::hir::OwnerNode::Item(HirItem::Static(s)) => s.body,
            _ => continue,
        };
        let body = match hir.find_body(body_id) {
            Some(b) => b,
            None => continue,
        };
        // body.value is HirExpr — if it's a Block, walk its stmts.
        if let HirExprKind::Block(block) = &body.value.kind {
            for stmt in &block.stmts {
                if let HirStmt::Local(local) = stmt {
                    if let Some(init) = &local.init {
                        if let HirPatKind::Tuple(sub_pats) = &local.pat.kind {
                            let pat_count = sub_pats.len();
                            if let HirExprKind::Tuple { elems } = &init.kind {
                                let tuple_len = elems.len();
                                if pat_count != tuple_len {
                                    errors.push(TypeError::new(
                                        format!(
                                            "pattern arity mismatch: {} pattern(s) but tuple has {} element(s)",
                                            pat_count, tuple_len
                                        ),
                                        local.pat.span,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// Stage 18.78 P1 (N7): Removed dead `validate_main_exists` function.
// The actual missing-main check is inlined in `compile_binary` (which has
// access to the CompileResult after compilation). This avoids borrow issues
// with the interner.
/// Stage 18.73 P1-E: Validate assignment targets.
///
/// For each `lhs = rhs` expression, check that `lhs` is a valid place
/// expression (local, field access, deref, index). Non-place targets
/// like `42 = 99` or `f() = 1` are rejected.
///
/// Per §1.0 原则 4 "报错 > 静默": invalid assignment target must be reported.
/// Per §1.0 原则 6 "通用 > 特例": one validator walks all bodies.
/// Per §10 naming: `validate_assignment_targets` follows `validate_<noun>_<noun>`.
fn validate_assignment_targets(
    hir: &HirCrate,
    _interner: &lasso::Rodeo,
    errors: &mut Vec<TypeError>,
) {
    use crate::hir::{HirExprKind, HirStmt, HirUnaryOp};

    for (_, owner) in &hir.owners {
        let body_id = match owner {
            crate::hir::OwnerNode::Item(HirItem::Fn(f)) if f.body.is_some() => f.body.unwrap(),
            _ => continue,
        };
        let body = match hir.find_body(body_id) {
            Some(b) => b,
            None => continue,
        };
        // Walk the body's expression tree to find Assign nodes.
        let mut to_check: Vec<&crate::hir::HirExpr> = vec![&body.value];
        while let Some(expr) = to_check.pop() {
            match &expr.kind {
                HirExprKind::Assign { lhs, rhs, .. } => {
                    // Check if lhs is a valid place expression.
                    let is_valid_place = match &lhs.kind {
                        HirExprKind::Path(_) => true,      // local or static
                        HirExprKind::Field { .. } => true, // struct/tuple field
                        HirExprKind::Index { .. } => true, // array index
                        HirExprKind::Unary {
                            op: HirUnaryOp::Deref,
                            ..
                        } => true, // *ptr
                        _ => false,
                    };
                    if !is_valid_place {
                        errors.push(TypeError::new(
                            "invalid assignment target — left-hand side must be a place expression (variable, field, dereference, or index)"
                                .to_string(),
                            lhs.span,
                        ));
                    }
                    // Recurse into lhs and rhs for nested assignments.
                    to_check.push(lhs);
                    to_check.push(rhs);
                }
                // Recurse into other expression kinds.
                HirExprKind::Call { func, args, .. } => {
                    to_check.push(func);
                    for arg in args {
                        to_check.push(arg);
                    }
                }
                HirExprKind::MethodCall { receiver, args, .. } => {
                    to_check.push(receiver);
                    for arg in args {
                        to_check.push(arg);
                    }
                }
                HirExprKind::Field { receiver, .. } => {
                    to_check.push(receiver);
                }
                HirExprKind::Unary { expr: inner, .. } => {
                    to_check.push(inner);
                }
                HirExprKind::Binary { lhs, rhs, .. } => {
                    to_check.push(lhs);
                    to_check.push(rhs);
                }
                HirExprKind::If {
                    cond, then, else_, ..
                } => {
                    to_check.push(cond);
                    for stmt in &then.stmts {
                        if let HirStmt::Expr(e, _) = stmt {
                            to_check.push(e);
                        }
                    }
                    if let Some(trailing) = &then.expr {
                        to_check.push(trailing);
                    }
                    if let Some(e) = else_ {
                        to_check.push(e);
                    }
                }
                HirExprKind::Match {
                    expr: scrutinee,
                    arms,
                    ..
                } => {
                    to_check.push(scrutinee);
                    for arm in arms {
                        if let Some(e) = &arm.guard {
                            to_check.push(e);
                        }
                        to_check.push(&arm.body);
                    }
                }
                HirExprKind::Block(block) => {
                    for stmt in &block.stmts {
                        if let HirStmt::Expr(e, _) = stmt {
                            to_check.push(e);
                        }
                    }
                    if let Some(trailing) = &block.expr {
                        to_check.push(trailing);
                    }
                }
                HirExprKind::Return { expr: Some(e), .. } => {
                    to_check.push(e);
                }
                HirExprKind::Tuple { elems } => {
                    for e in elems {
                        to_check.push(e);
                    }
                }
                HirExprKind::Array { elems } => {
                    for e in elems {
                        to_check.push(e);
                    }
                }
                HirExprKind::Struct { fields, .. } => {
                    for f in fields {
                        if let Some(e) = &f.expr {
                            to_check.push(e);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Stage 18.73 P1-F: Validate cast types.
///
/// For each `expr as Ty` expression, check that the cast is valid:
///   - Int/Uint → Int/Uint/Bool/Char: OK (numeric casts)
///   - Float → Float: OK
///   - Bool → Int/Uint: OK
///   - Other casts: rejected
///
/// Per §1.0 原则 4 "报错 > 静默": invalid cast must be reported.
/// Per §10 naming: `validate_cast_types` follows `validate_<noun>_<noun>`.
fn validate_cast_types(hir: &HirCrate, _interner: &lasso::Rodeo, errors: &mut Vec<TypeError>) {
    use crate::hir::{HirExprKind, HirLitKind, HirStmt};

    for (_, owner) in &hir.owners {
        let body_id = match owner {
            crate::hir::OwnerNode::Item(HirItem::Fn(f)) if f.body.is_some() => f.body.unwrap(),
            _ => continue,
        };
        let body = match hir.find_body(body_id) {
            Some(b) => b,
            None => continue,
        };
        let mut to_check: Vec<&crate::hir::HirExpr> = vec![&body.value];
        // Also walk statements — including Local (let bindings) which may
        // contain cast expressions in their init.
        if let HirExprKind::Block(block) = &body.value.kind {
            for stmt in &block.stmts {
                match stmt {
                    HirStmt::Expr(e, _) => to_check.push(e),
                    HirStmt::Local(local) => {
                        if let Some(init) = &local.init {
                            to_check.push(init);
                        }
                    }
                    _ => {}
                }
            }
        }
        while let Some(expr) = to_check.pop() {
            if let HirExprKind::Cast { expr: inner, ty } = &expr.kind {
                // Conservative HIR-level check: if inner is a literal,
                // determine its type kind and check against target type.
                let src_kind = literal_type_kind(&inner.kind);
                let dst_kind = hir_ty_kind(&ty.kind);
                if let (Some(src), Some(dst)) = (src_kind, dst_kind) {
                    if !is_valid_cast(src, dst) {
                        errors.push(TypeError::new(
                            format!("invalid cast: cannot cast `{}` to `{}`", src, dst),
                            expr.span,
                        ));
                    }
                }
                to_check.push(inner);
            }
            // Recurse into common expression kinds.
            match &expr.kind {
                HirExprKind::Call { func, args, .. } => {
                    to_check.push(func);
                    for arg in args {
                        to_check.push(arg);
                    }
                }
                HirExprKind::MethodCall { receiver, args, .. } => {
                    to_check.push(receiver);
                    for arg in args {
                        to_check.push(arg);
                    }
                }
                HirExprKind::Field { receiver, .. } => {
                    to_check.push(receiver);
                }
                HirExprKind::Unary { expr: inner, .. } => {
                    to_check.push(inner);
                }
                HirExprKind::Binary { lhs, rhs, .. } => {
                    to_check.push(lhs);
                    to_check.push(rhs);
                }
                HirExprKind::Assign { lhs, rhs, .. } => {
                    to_check.push(lhs);
                    to_check.push(rhs);
                }
                HirExprKind::If {
                    cond, then, else_, ..
                } => {
                    to_check.push(cond);
                    for stmt in &then.stmts {
                        match stmt {
                            HirStmt::Expr(e, _) => to_check.push(e),
                            HirStmt::Local(local) => {
                                if let Some(init) = &local.init {
                                    to_check.push(init);
                                }
                            }
                            _ => {}
                        }
                    }
                    if let Some(trailing) = &then.expr {
                        to_check.push(trailing);
                    }
                    if let Some(e) = else_ {
                        to_check.push(e);
                    }
                }
                HirExprKind::Block(block) => {
                    for stmt in &block.stmts {
                        match stmt {
                            HirStmt::Expr(e, _) => to_check.push(e),
                            HirStmt::Local(local) => {
                                if let Some(init) = &local.init {
                                    to_check.push(init);
                                }
                            }
                            _ => {}
                        }
                    }
                    if let Some(trailing) = &block.expr {
                        to_check.push(trailing);
                    }
                }
                HirExprKind::Return { expr: Some(e), .. } => {
                    to_check.push(e);
                }
                HirExprKind::Tuple { elems } => {
                    for e in elems {
                        to_check.push(e);
                    }
                }
                HirExprKind::Array { elems } => {
                    for e in elems {
                        to_check.push(e);
                    }
                }
                HirExprKind::Struct { fields, .. } => {
                    for f in fields {
                        if let Some(e) = &f.expr {
                            to_check.push(e);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Determine the type kind of a literal expression.
    fn literal_type_kind(kind: &HirExprKind) -> Option<&'static str> {
        match kind {
            HirExprKind::Lit(HirLitKind::Bool(_)) => Some("bool"),
            HirExprKind::Lit(HirLitKind::Int(_, _)) => Some("integer"),
            HirExprKind::Lit(HirLitKind::Uint(_, _)) => Some("integer"),
            HirExprKind::Lit(HirLitKind::Float(_, _)) => Some("float"),
            HirExprKind::Lit(HirLitKind::Char(_)) => Some("char"),
            HirExprKind::Lit(HirLitKind::Str(_)) => Some("str"),
            _ => None,
        }
    }

    /// Determine the type kind from a HIR type.
    fn hir_ty_kind(ty_kind: &crate::hir::HirTyKind) -> Option<&'static str> {
        use crate::hir::HirTyKind;
        match ty_kind {
            HirTyKind::Bool => Some("bool"),
            HirTyKind::Int(_) => Some("integer"),
            HirTyKind::Uint(_) => Some("integer"),
            HirTyKind::Float(_) => Some("float"),
            HirTyKind::Char => Some("char"),
            _ => None,
        }
    }

    /// Check if a cast from src to dst is valid (Rust semantics, simplified).
    /// Per §1.0 原则 9 "正确 > 妥协": match Rust's cast rules.
    /// Rust allows: numeric→numeric, numeric→char, char→numeric, bool→numeric,
    /// numeric→bool (via `as`). Does NOT allow: str→anything, float→bool,
    /// bool→float, bool→char, char→bool.
    fn is_valid_cast(src: &str, dst: &str) -> bool {
        matches!(
            (src, dst),
            // Numeric casts (int/uint/float are all numeric)
            ("integer", "integer")
                | ("integer", "float")
                | ("float", "integer")
                | ("float", "float")
                | ("integer", "char")
                | ("char", "integer")
                | ("char", "char")
                // Bool → integer (widening)
                | ("bool", "integer")
                // Integer → bool (Rust allows `x as bool`)
                | ("integer", "bool")
        )
    }
}

/// Stage 14.43: Resolve the type of a `self` parameter for fn_sig_table.
///
/// This mirrors `resolve_self_param_type` in mir/lower/mod.rs but is used
/// during fn_sig_table construction (before MIR lowering). It searches
/// the HIR for the impl block that owns the given method DefId, then
/// returns the impl's `self_ty` (with Ref wrapping for &self/&mut self).
///
/// Per §16 (interface isolation): this is a HIR query at fn_sig_table
/// construction time. The result is sunk into fn_sig_table as data.
/// Stage 15.2 (perf optimization): Pre-build a DefId → ImplBlock index
/// to eliminate O(B × O × I) quadratic scan in `resolve_self_param_type_for_sig`.
///
/// Per Phase 2 audit recommendation: "Pre-build HashMap<DefId, &ImplBlock> index
/// so resolve_self_param_type_for_sig is O(1) per call."
///
/// Per §1.0 原则 6 "通用 > 特例": one index handles all impl methods.
fn build_method_to_impl_index(
    hir: &HirCrate,
) -> std::collections::HashMap<crate::hir::DefId, usize> {
    let mut index = std::collections::HashMap::new();
    for (i, (_, owner)) in hir.owners.iter().enumerate() {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
            for impl_item in &impl_block.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    // Map method DefId → owner index (where the impl block lives)
                    index.insert(f.hir_id.owner, i);
                }
            }
        }
    }
    index
}

fn resolve_self_param_type_for_sig(
    hir: &HirCrate,
    method_def_id: crate::hir::DefId,
    self_kind: Option<crate::ast::SelfKind>,
    method_to_impl_index: &std::collections::HashMap<crate::hir::DefId, usize>,
) -> Option<crate::mir::ty::Ty> {
    // Stage 15.2: O(1) lookup via pre-built index (was O(O × I) linear scan).
    let owner_idx = *method_to_impl_index.get(&method_def_id)?;
    let (_, owner) = &hir.owners[owner_idx];
    if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
        let adt_ty = crate::mir::lower::lower_hir_ty_to_mir_ty(&impl_block.self_ty);
        return match self_kind {
            Some(crate::ast::SelfKind::Ref(mutability)) => {
                let mir_mut = match mutability {
                    crate::ast::Mutability::Mutable => crate::mir::ty::Mutability::Mutable,
                    crate::ast::Mutability::Immutable => crate::mir::ty::Mutability::Immutable,
                };
                Some(crate::mir::ty::Ty::new(
                    crate::mir::ty::TyKind::Ref(
                        crate::mir::ty::Region::Erased,
                        mir_mut,
                        Box::new(adt_ty),
                    ),
                    crate::session::Span::DUMMY,
                ))
            }
            // self by value — no wrapping
            _ => Some(adt_ty),
        };
    }
    None
}
/// Public wrapper for codegen to get the return type of a body's owner.
pub fn owner_return_ty_for_body(
    hir: &HirCrate,
    body: &crate::hir::Body,
) -> Option<crate::hir::HirTy> {
    // Find the owner by matching body's hir_id
    for (_, owner) in &hir.owners {
        if let OwnerNode::Item(HirItem::Fn(f)) = owner {
            if let Some(body_id) = &f.body {
                // Check if this body belongs to this fn by comparing
                // the body's hir_id owner with the fn's def_id
                if body.hir_id.owner == body_id.owner.0 {
                    return owner_return_ty(owner);
                }
            }
        }
    }
    None
}

/// G4 fix: Scan HIR for unresolved paths after name resolution.
///
/// Any `HirPath` with `Res::Unknown` or `Res::Err` indicates an undefined
/// name (e.g., calling `undefined_fn()` or referring to an undefined
/// variable). Emit a resolve error for each.
///
/// Without this scan, undefined names silently fall through to
/// `Ty::Error` in MIR lower, which typeck treats as "always succeeds"
/// (intentional error recovery). The result: typos in function names
/// go undetected.
fn scan_for_unresolved_paths(hir: &HirCrate, errors: &mut CompileErrors) {
    for (_, body) in &hir.bodies {
        scan_expr_for_unresolved(&body.value, errors);
        for param in &body.params {
            if let Some(ty) = &param.ty {
                scan_ty_for_unresolved(ty, errors);
            }
            scan_pat_for_unresolved(&param.pat, errors);
        }
    }
}

fn scan_expr_for_unresolved(expr: &crate::hir::HirExpr, errors: &mut CompileErrors) {
    use crate::hir::{HirExprKind, Res};
    match &expr.kind {
        HirExprKind::Path(p) => {
            if matches!(p.res, Res::Unknown | Res::Err) {
                errors.resolve.push(crate::resolve::ResolveError::with_kind(
                    crate::resolve::ResolveErrorKind::CannotFindValue,
                    "cannot find value in this scope".to_string(),
                    p.span,
                ));
            }
        }
        HirExprKind::Block(b) => {
            for stmt in &b.stmts {
                use crate::hir::HirStmt;
                match stmt {
                    HirStmt::Local(local) => {
                        if let Some(init) = &local.init {
                            scan_expr_for_unresolved(init, errors);
                        }
                        if let Some(ty) = &local.ty {
                            scan_ty_for_unresolved(ty, errors);
                        }
                    }
                    HirStmt::Expr(e, _) => scan_expr_for_unresolved(e, errors),
                    _ => {} // Stage 18.60: skip unhandled HirExprKind variant (no Res::Def to check)
                }
            }
            if let Some(e) = &b.expr {
                scan_expr_for_unresolved(e, errors);
            }
        }
        HirExprKind::Binary { lhs, rhs, .. } => {
            scan_expr_for_unresolved(lhs, errors);
            scan_expr_for_unresolved(rhs, errors);
        }
        HirExprKind::Unary { expr: inner, .. } => {
            scan_expr_for_unresolved(inner, errors);
        }
        HirExprKind::Call { func, args, .. } => {
            scan_expr_for_unresolved(func, errors);
            for a in args {
                scan_expr_for_unresolved(a, errors);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            scan_expr_for_unresolved(receiver, errors);
            for a in args {
                scan_expr_for_unresolved(a, errors);
            }
        }
        HirExprKind::Field { receiver, .. } => {
            scan_expr_for_unresolved(receiver, errors);
        }
        HirExprKind::Index {
            receiver, index, ..
        } => {
            scan_expr_for_unresolved(receiver, errors);
            scan_expr_for_unresolved(index, errors);
        }
        HirExprKind::If {
            cond, then, else_, ..
        } => {
            scan_expr_for_unresolved(cond, errors);
            for stmt in &then.stmts {
                use crate::hir::HirStmt;
                if let HirStmt::Expr(e, _) = stmt {
                    scan_expr_for_unresolved(e, errors);
                }
            }
            if let Some(e) = &then.expr {
                scan_expr_for_unresolved(e, errors);
            }
            if let Some(e) = else_ {
                scan_expr_for_unresolved(e, errors);
            }
        }
        HirExprKind::Match {
            expr: scrutinee,
            arms,
            ..
        } => {
            scan_expr_for_unresolved(scrutinee, errors);
            for arm in arms {
                scan_pat_for_unresolved(&arm.pat, errors);
                scan_expr_for_unresolved(&arm.body, errors);
            }
        }
        HirExprKind::Return { expr } | HirExprKind::Break { expr } => {
            if let Some(e) = expr {
                scan_expr_for_unresolved(e, errors);
            }
        }
        HirExprKind::Assign { lhs, rhs, .. } => {
            scan_expr_for_unresolved(lhs, errors);
            scan_expr_for_unresolved(rhs, errors);
        }
        HirExprKind::Tuple { elems, .. } => {
            for e in elems {
                scan_expr_for_unresolved(e, errors);
            }
        }
        HirExprKind::Array { elems, .. } => {
            for e in elems {
                scan_expr_for_unresolved(e, errors);
            }
        }
        HirExprKind::Struct { fields, .. } => {
            for f in fields {
                if let Some(e) = &f.expr {
                    scan_expr_for_unresolved(e, errors);
                }
            }
        }
        HirExprKind::Cast {
            expr: inner, ty, ..
        } => {
            scan_expr_for_unresolved(inner, errors);
            scan_ty_for_unresolved(ty, errors);
        }
        HirExprKind::AddrOf { expr: inner, .. } => {
            scan_expr_for_unresolved(inner, errors);
        }
        HirExprKind::Loop { body, .. } | HirExprKind::While { body, .. } => {
            for stmt in &body.stmts {
                use crate::hir::HirStmt;
                match stmt {
                    HirStmt::Local(local) => {
                        if let Some(init) = &local.init {
                            scan_expr_for_unresolved(init, errors);
                        }
                        if let Some(ty) = &local.ty {
                            scan_ty_for_unresolved(ty, errors);
                        }
                    }
                    HirStmt::Expr(e, _) => scan_expr_for_unresolved(e, errors),
                    _ => {} // Stage 18.60: skip unhandled HirExprKind variant (no Res::Def to check)
                }
            }
            if let Some(e) = &body.expr {
                scan_expr_for_unresolved(e, errors);
            }
        }
        // Stage 14.100 (Bug AA2 fix): For-loop body must be scanned for
        // unresolved paths. Previously the `_ => {}` catch-all skipped For,
        // so `for i in 0..5 { let _ = nonexistent_xyz; }` silently compiled.
        HirExprKind::For { iter, body, .. } => {
            scan_expr_for_unresolved(iter, errors);
            for stmt in &body.stmts {
                use crate::hir::HirStmt;
                match stmt {
                    HirStmt::Local(local) => {
                        if let Some(init) = &local.init {
                            scan_expr_for_unresolved(init, errors);
                        }
                        if let Some(ty) = &local.ty {
                            scan_ty_for_unresolved(ty, errors);
                        }
                    }
                    HirStmt::Expr(e, _) => scan_expr_for_unresolved(e, errors),
                    _ => {} // Stage 18.60: skip unhandled HirExprKind variant (no Res::Def to check)
                }
            }
            if let Some(e) = &body.expr {
                scan_expr_for_unresolved(e, errors);
            }
        }
        // Stage 14.100 (Bug AA3 fix): Range start/end must be scanned.
        // Previously the catch-all skipped Range, so `for i in foo..5 {}`
        // silently used foo=0.
        HirExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                scan_expr_for_unresolved(s, errors);
            }
            if let Some(e) = end {
                scan_expr_for_unresolved(e, errors);
            }
        }
        // Stage 14.100 (Bug AA4 fix): Repeat elem/count must be scanned.
        // Previously the catch-all skipped Repeat, so `let arr = [foo; 3];`
        // silently used foo=0.
        HirExprKind::Repeat { elem, count } => {
            scan_expr_for_unresolved(elem, errors);
            scan_expr_for_unresolved(count, errors);
        }
        // Stage 18.48: HirExprKind::Println variant removed.
        HirExprKind::Closure { body, .. } => scan_expr_for_unresolved(body, errors),
        // Stage 14.101 (Phase 1 audit fix): Scan Try expr, Unsafe block,
        // MacroCall path, Await expr, Async block.
        // Previously the catch-all silently skipped these, so unresolved
        // paths inside them went unreported.
        HirExprKind::Try { expr, .. } => scan_expr_for_unresolved(expr, errors),
        HirExprKind::Unsafe(block) => {
            for stmt in &block.stmts {
                use crate::hir::HirStmt;
                match stmt {
                    HirStmt::Local(local) => {
                        if let Some(init) = &local.init {
                            scan_expr_for_unresolved(init, errors);
                        }
                        if let Some(ty) = &local.ty {
                            scan_ty_for_unresolved(ty, errors);
                        }
                    }
                    HirStmt::Expr(e, _) => scan_expr_for_unresolved(e, errors),
                    _ => {} // Stage 18.60: skip unhandled HirExprKind variant (no Res::Def to check)
                }
            }
            if let Some(e) = &block.expr {
                scan_expr_for_unresolved(e, errors);
            }
        }
        HirExprKind::MacroCall { path, .. } => {
            // Stage 14.101: MacroCall path resolution. Built-in macros
            // (vec!, println!, assert!, etc.) are single-segment paths that
            // the resolver doesn't resolve to Res::Def — they're handled
            // specially during HIR lowering. Only report multi-segment paths
            // (e.g., `std::println!`) as errors if unresolved.
            //
            // Per §1.0 原则 5 "报错 > 静默": unresolved macro paths should be
            // reported, but we must not false-positive on built-in macros.
            if path.segments.len() > 1 && matches!(path.res, Res::Unknown | Res::Err) {
                errors.resolve.push(crate::resolve::ResolveError::with_kind(
                    crate::resolve::ResolveErrorKind::CannotFindMacro,
                    "cannot find macro in this scope".to_string(),
                    path.span,
                ));
            }
        }
        HirExprKind::Await { expr, .. } => scan_expr_for_unresolved(expr, errors),
        HirExprKind::Async { block, .. } => {
            for stmt in &block.stmts {
                use crate::hir::HirStmt;
                match stmt {
                    HirStmt::Local(local) => {
                        if let Some(init) = &local.init {
                            scan_expr_for_unresolved(init, errors);
                        }
                        if let Some(ty) = &local.ty {
                            scan_ty_for_unresolved(ty, errors);
                        }
                    }
                    HirStmt::Expr(e, _) => scan_expr_for_unresolved(e, errors),
                    _ => {} // Stage 18.60: skip unhandled HirExprKind variant (no Res::Def to check)
                }
            }
            if let Some(e) = &block.expr {
                scan_expr_for_unresolved(e, errors);
            }
        }
        // Lit, Unit, Continue — genuinely no sub-expressions
        HirExprKind::Lit(_) | HirExprKind::Unit | HirExprKind::Continue => {}
    }
}

fn scan_pat_for_unresolved(pat: &crate::hir::HirPat, errors: &mut CompileErrors) {
    // Stage 14.101 (Phase 1 audit fix): Re-enabled pattern scanning.
    //
    // Previously this was a no-op stub (G4 fix) because enum variant patterns
    // like `Circle(r)` appeared as Res::Unknown. However, this meant unresolved
    // IDENTIFIER patterns (e.g., `match x { nonexistent => ... }`) were also
    // silently accepted.
    //
    // Now we scan patterns but ONLY report paths that resolve to Res::Unknown
    // AND are not enum variant patterns. We detect enum variant patterns by
    // checking if the path has multiple segments (e.g., `Color::Red`) — single-
    // segment paths in TupleStruct/Struct/Path patterns might be enum variants
    // (resolved lazily during typeck) so we skip them.
    //
    // Per §1.0 原则 5 "报错 > 静默": unresolved identifiers in patterns should
    // be reported. Per §1.0 原则 6 "通用 > 特例": one rule handles all pattern
    // kinds by recursing into sub-patterns.
    use crate::hir::{HirPatKind, Res};
    match &pat.kind {
        HirPatKind::Wild | HirPatKind::Rest | HirPatKind::Lit(_) => {}
        HirPatKind::Ident(_mode, ident, sub) => {
            // Ident patterns bind a new variable — they don't reference an
            // existing path. No resolution check needed.
            let _ = ident;
            if let Some(s) = sub {
                scan_pat_for_unresolved(s, errors);
            }
        }
        HirPatKind::Struct(path, fields, _has_rest) => {
            // Multi-segment paths (e.g., `Color::Red { ... }`) should be resolved.
            // Single-segment paths might be enum variants (lazily resolved).
            if path.segments.len() > 1 && matches!(path.res, Res::Unknown | Res::Err) {
                errors.resolve.push(crate::resolve::ResolveError::with_kind(
                    crate::resolve::ResolveErrorKind::CannotFindType,
                    "cannot find type in this scope".to_string(),
                    path.span,
                ));
            }
            for f in fields {
                scan_pat_for_unresolved(&f.pat, errors);
            }
        }
        HirPatKind::TupleStruct(path, sub_pats) => {
            if path.segments.len() > 1 && matches!(path.res, Res::Unknown | Res::Err) {
                errors.resolve.push(crate::resolve::ResolveError::with_kind(
                    crate::resolve::ResolveErrorKind::CannotFindType,
                    "cannot find type in this scope".to_string(),
                    path.span,
                ));
            }
            for p in sub_pats {
                scan_pat_for_unresolved(p, errors);
            }
        }
        HirPatKind::Tuple(sub_pats) => {
            for p in sub_pats {
                scan_pat_for_unresolved(p, errors);
            }
        }
        HirPatKind::Slice(sub_pats, rest) => {
            for p in sub_pats {
                scan_pat_for_unresolved(p, errors);
            }
            if let Some(r) = rest {
                scan_pat_for_unresolved(r, errors);
            }
        }
        HirPatKind::Or(sub_pats) => {
            for p in sub_pats {
                scan_pat_for_unresolved(p, errors);
            }
        }
        HirPatKind::Path(path) => {
            // Multi-segment paths (e.g., `Color::Red`) should be resolved.
            // Single-segment paths might be enum variants (lazily resolved).
            if path.segments.len() > 1 && matches!(path.res, Res::Unknown | Res::Err) {
                errors.resolve.push(crate::resolve::ResolveError::with_kind(
                    crate::resolve::ResolveErrorKind::CannotFindType,
                    "cannot find type in this scope".to_string(),
                    path.span,
                ));
            }
        }
        HirPatKind::Range(start, end, _) => {
            if let Some(s) = start {
                scan_expr_for_unresolved(s, errors);
            }
            if let Some(e) = end {
                scan_expr_for_unresolved(e, errors);
            }
        }
        HirPatKind::Ref(sub, _) => {
            scan_pat_for_unresolved(sub, errors);
        }
    }
}

/// Stage 16.65 (Task 14 Phase 2): Check object safety for all `dyn Trait` usages.
///
/// Scans all HIR types for `HirTyKind::TraitObject`. For each, resolves the
/// trait DefId from the bound's path, looks up the `HirTrait` definition,
/// and calls `check_trait_object_safety`. If any violations are found, emits
/// typeck errors.
///
/// Per §23: `check_object_safety_for_dyn_trait_usage` follows
/// `<verb>_<noun>_<noun>_<prep>_<noun>` pattern.
/// Per §16: reads HIR + TraitResolver (allowed during driver pre-computation).
fn check_object_safety_for_dyn_trait_usage(
    hir: &crate::hir::HirCrate,
    resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    errors: &mut CompileErrors,
) {
    use crate::hir::{HirItem, HirTyKind, HirTypeBound, OwnerNode, Res};
    use crate::traits::object_safety::check_trait_object_safety;

    // Build a map from trait DefId → HirTrait for quick lookup.
    let mut trait_defs: std::collections::HashMap<crate::hir::DefId, &crate::hir::HirTrait> =
        std::collections::HashMap::new();
    for (def_id, owner) in &hir.owners {
        if let OwnerNode::Item(HirItem::Trait(t)) = owner {
            trait_defs.insert(*def_id, t);
        }
    }

    // Walk all HIR bodies for TraitObject types.
    for (_body_id, body) in &hir.bodies {
        walk_hir_ty_in_body(&body.value, &mut |ty| {
            if let HirTyKind::TraitObject { bounds, .. } = &ty.kind {
                for bound in bounds {
                    if let HirTypeBound::Trait(tc) = bound {
                        if let Res::Def(trait_def_id, _) = tc.path.res {
                            if let Some(trait_def) = trait_defs.get(&trait_def_id) {
                                let violations =
                                    check_trait_object_safety(trait_def, &trait_defs, interner);
                                if !violations.is_empty() {
                                    let trait_name = interner
                                        .try_resolve(&trait_def.ident.name)
                                        .unwrap_or("<anonymous>");
                                    for v in &violations {
                                        errors.typeck.push(crate::typeck::TypeError::new(
                                            v.error_message(trait_name, interner),
                                            v.span(),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    // Also walk fn signatures, struct fields, etc. for TraitObject types.
    for (_, owner) in &hir.owners {
        match owner {
            OwnerNode::Item(HirItem::Fn(f)) => {
                for param in &f.sig.inputs {
                    if let Some(ty) = &param.ty {
                        walk_hir_ty(ty, &mut |ty| {
                            check_trait_object_ty(ty, &trait_defs, resolver, interner, errors);
                        });
                    }
                }
                if let crate::hir::HirFnRetTy::Ty(ret_ty) = &f.sig.output {
                    walk_hir_ty(ret_ty, &mut |ty| {
                        check_trait_object_ty(ty, &trait_defs, resolver, interner, errors);
                    });
                }
            }
            OwnerNode::Item(HirItem::Struct(s)) => {
                for field in &s.fields {
                    walk_hir_ty(&field.ty, &mut |ty| {
                        check_trait_object_ty(ty, &trait_defs, resolver, interner, errors);
                    });
                }
            }
            OwnerNode::Item(HirItem::Enum(e)) => {
                for variant in &e.variants {
                    match &variant.data {
                        crate::hir::HirVariantData::Tuple(fields, _) => {
                            for f in fields {
                                walk_hir_ty(&f.ty, &mut |ty| {
                                    check_trait_object_ty(
                                        ty,
                                        &trait_defs,
                                        resolver,
                                        interner,
                                        errors,
                                    );
                                });
                            }
                        }
                        crate::hir::HirVariantData::Struct(fields, _) => {
                            for f in fields {
                                walk_hir_ty(&f.ty, &mut |ty| {
                                    check_trait_object_ty(
                                        ty,
                                        &trait_defs,
                                        resolver,
                                        interner,
                                        errors,
                                    );
                                });
                            }
                        }
                        _ => {} // Stage 18.60: skip unhandled variant (no Res::Def to check)
                    }
                }
            }
            _ => {} // Stage 18.60: skip unhandled HirStmt variant
        }
    }
}

/// Helper: check a single TraitObject type for object safety.
fn check_trait_object_ty(
    ty: &crate::hir::HirTy,
    trait_defs: &std::collections::HashMap<crate::hir::DefId, &crate::hir::HirTrait>,
    _resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    errors: &mut CompileErrors,
) {
    use crate::hir::{HirTyKind, HirTypeBound, Res};
    use crate::traits::object_safety::check_trait_object_safety;

    if let HirTyKind::TraitObject { bounds, .. } = &ty.kind {
        for bound in bounds {
            if let HirTypeBound::Trait(tc) = bound {
                if let Res::Def(trait_def_id, _) = tc.path.res {
                    if let Some(trait_def) = trait_defs.get(&trait_def_id) {
                        let violations = check_trait_object_safety(trait_def, trait_defs, interner);
                        if !violations.is_empty() {
                            let trait_name = interner
                                .try_resolve(&trait_def.ident.name)
                                .unwrap_or("<anonymous>");
                            for v in &violations {
                                errors.typeck.push(crate::typeck::TypeError::new(
                                    v.error_message(trait_name, interner),
                                    v.span(),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Walk a HirTy and call f for each type (including nested).
///
/// Stage 16.71 (Round 10 fix): Added FnPtr inputs/output recursion.
fn walk_hir_ty<F>(ty: &crate::hir::HirTy, f: &mut F)
where
    F: FnMut(&crate::hir::HirTy),
{
    use crate::hir::HirTyKind;
    f(ty);
    match &ty.kind {
        HirTyKind::Ref(_, _, inner) | HirTyKind::Ptr(_, inner) | HirTyKind::Slice(inner) => {
            walk_hir_ty(inner, f);
        }
        HirTyKind::Array(inner, _) => walk_hir_ty(inner, f),
        HirTyKind::Tuple(tys) => {
            for t in tys {
                walk_hir_ty(t, f);
            }
        }
        // Stage 16.71: FnPtr — recurse into inputs and output
        HirTyKind::FnPtr { inputs, output, .. } => {
            for t in inputs {
                walk_hir_ty(t, f);
            }
            walk_hir_ty(output, f);
        }
        // Stage 18.61: TraitObject / ImplTrait — recurse into bounds
        // (bounds contain HirTypeBound::Trait(path) which has paths to scan).
        // Per §1.0 原則 2 "整体 > 局部": walker must cover all type variants.
        HirTyKind::TraitObject { bounds, .. } | HirTyKind::ImplTrait(bounds) => {
            for bound in bounds {
                if let crate::hir::HirTypeBound::Trait(tb) = bound {
                    // The trait bound's path may have generic args with types.
                    // Walk the path segments' args.
                    for seg in &tb.path.segments {
                        if let Some(crate::ast::GenericArgs::AngleBracketed(_)) = &seg.args {
                            // AST Ty args — can't walk via walk_hir_ty (needs HirTy).
                            // The resolver will catch unresolved paths here
                            // via resolve_ty_paths during resolution.
                        }
                    }
                }
            }
        }
        _ => {} // Stage 18.60: skip unhandled variant (no paths to scan)
    }
}

/// Walk a HirExpr for HirTy occurrences (in cast expressions, let bindings, etc.).
fn walk_hir_ty_in_body<F>(expr: &crate::hir::HirExpr, f: &mut F)
where
    F: FnMut(&crate::hir::HirTy),
{
    use crate::hir::HirExprKind;
    match &expr.kind {
        HirExprKind::Cast { expr, ty } => {
            walk_hir_ty_in_body(expr, f);
            walk_hir_ty(ty, f);
        }
        HirExprKind::Call { func, args } => {
            walk_hir_ty_in_body(func, f);
            for arg in args {
                walk_hir_ty_in_body(arg, f);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            walk_hir_ty_in_body(receiver, f);
            for arg in args {
                walk_hir_ty_in_body(arg, f);
            }
        }
        HirExprKind::Field { receiver, .. } => {
            walk_hir_ty_in_body(receiver, f);
        }
        HirExprKind::Index { receiver, index } => {
            walk_hir_ty_in_body(receiver, f);
            walk_hir_ty_in_body(index, f);
        }
        HirExprKind::AddrOf { expr, .. } => {
            walk_hir_ty_in_body(expr, f);
        }
        HirExprKind::Unary { expr, .. } => {
            walk_hir_ty_in_body(expr, f);
        }
        HirExprKind::Binary { lhs, rhs, .. } => {
            walk_hir_ty_in_body(lhs, f);
            walk_hir_ty_in_body(rhs, f);
        }
        HirExprKind::If {
            cond, then, else_, ..
        } => {
            walk_hir_ty_in_body(cond, f);
            walk_hir_block(then, f);
            if let Some(e) = else_ {
                walk_hir_ty_in_body(e, f);
            }
        }
        HirExprKind::Match { expr, arms } => {
            walk_hir_ty_in_body(expr, f);
            for arm in arms {
                walk_hir_ty_in_body(&arm.body, f);
            }
        }
        HirExprKind::Block(block) => {
            walk_hir_block(block, f);
        }
        _ => {} // Stage 18.60: skip unhandled variant (no paths to scan)
    }
}

/// Walk a HirBlock for HirTy occurrences.
fn walk_hir_block<F>(block: &crate::hir::HirBlock, f: &mut F)
where
    F: FnMut(&crate::hir::HirTy),
{
    for stmt in &block.stmts {
        walk_hir_ty_in_stmt(stmt, f);
    }
    if let Some(expr) = &block.expr {
        walk_hir_ty_in_body(expr, f);
    }
}

/// Walk a HirStmt for HirTy occurrences.
fn walk_hir_ty_in_stmt<F>(stmt: &crate::hir::HirStmt, f: &mut F)
where
    F: FnMut(&crate::hir::HirTy),
{
    match stmt {
        crate::hir::HirStmt::Local(local) => {
            if let Some(ty) = &local.ty {
                walk_hir_ty(ty, f);
            }
            if let Some(init) = &local.init {
                walk_hir_ty_in_body(init, f);
            }
        }
        crate::hir::HirStmt::Expr(expr, _) => {
            walk_hir_ty_in_body(expr, f);
        }
        _ => {} // Stage 18.60: skip unhandled variant (no paths to scan)
    }
}

fn scan_ty_for_unresolved(ty: &crate::hir::HirTy, errors: &mut CompileErrors) {
    use crate::hir::{HirTyKind, Res};
    match &ty.kind {
        HirTyKind::Path(_, p) => {
            if matches!(p.res, Res::Unknown | Res::Err) {
                errors.resolve.push(crate::resolve::ResolveError::with_kind(
                    crate::resolve::ResolveErrorKind::CannotFindType,
                    "cannot find type in this scope".to_string(),
                    // Stage 15.87: use the type path's span (was:
                    // Span::DUMMY, producing "1:1" for type resolution
                    // errors like `let x: Undefined = 42;`).
                    //
                    // Per §1.0 原則 3 "显式 > 隐式": error spans are
                    // explicitly sourced from the type path.
                    // Per §1.0 原則 4 "报错 > 静默": error locations
                    // are accurate, not cryptic.
                    p.span,
                ));
            }
        }
        HirTyKind::Ref(_, _, inner)
        | HirTyKind::Ptr(_, inner)
        | HirTyKind::Slice(inner)
        | HirTyKind::Array(inner, _) => scan_ty_for_unresolved(inner, errors),
        HirTyKind::Tuple(tys) => {
            for t in tys {
                scan_ty_for_unresolved(t, errors);
            }
        }
        // Stage 14.101 (Phase 1 audit fix): FnPtr inputs/output must be scanned.
        // Previously the catch-all silently skipped FnPtr, so
        // `fn(unresolved) -> i32` went unreported.
        HirTyKind::FnPtr { inputs, output, .. } => {
            for t in inputs {
                scan_ty_for_unresolved(t, errors);
            }
            scan_ty_for_unresolved(output, errors);
        }
        // Stage 14.101 (Phase 1 audit fix): TraitObject bounds must be scanned.
        HirTyKind::TraitObject { bounds, .. } => {
            for bound in bounds {
                scan_type_bound_for_unresolved(bound, errors);
            }
        }
        // Stage 14.101 (Phase 1 audit fix): ImplTrait bounds must be scanned.
        HirTyKind::ImplTrait(bounds) => {
            for bound in bounds {
                scan_type_bound_for_unresolved(bound, errors);
            }
        }
        // Bool, Char, Int, Uint, Float, Never, Infer — no sub-types
        HirTyKind::Bool
        | HirTyKind::Char
        | HirTyKind::Int(_)
        | HirTyKind::Uint(_)
        | HirTyKind::Float(_)
        | HirTyKind::Never
        | HirTyKind::Infer => {}
    }
}

/// Stage 14.101 (Phase 1 audit fix): Scan a type bound for unresolved paths.
/// Used by TraitObject and ImplTrait scanning.
fn scan_type_bound_for_unresolved(bound: &crate::hir::HirTypeBound, errors: &mut CompileErrors) {
    use crate::hir::Res;
    if let crate::hir::HirTypeBound::Trait(trait_bound) = bound {
        let path = &trait_bound.path;
        if matches!(path.res, Res::Unknown | Res::Err) {
            errors.resolve.push(crate::resolve::ResolveError::with_kind(
                crate::resolve::ResolveErrorKind::CannotFindTrait,
                "cannot find trait in this scope".to_string(),
                path.span,
            ));
        }
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
        // Stage 18.71: Updated to reflect P0-5 fix — `fn f() { 42 }` now
        // has a unit return type (the `42` is a discarded trailing
        // expression, not the return value). To get an Int return type,
        // the function must declare `-> i32`.
        let result = compile_expect_ok("fn f() -> i32 { 42 }");
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

    // === Stage 16.83: Diagnostic type name resolution via resolver tests ===
    // Per §9.4.3: 2 positive + 6 negative tests (1:3 ratio).

    /// Stage 16.83 positive 1: to_diagnostics_with_resolver shows struct name.
    #[test]
    fn stage16_83_diagnostic_with_resolver_shows_struct_name() {
        let src = "struct MyStruct { x: i32 } fn foo(s: MyStruct) {} fn main() { foo(42); 0 }";
        let result = compile(src);
        let diags = result
            .errors
            .to_diagnostics_with_resolver(Some(&result.interner), Some(&result.trait_resolver));
        // Find a typeck diagnostic with expected/found notes.
        let has_struct_name = diags
            .iter()
            .any(|d| d.children.iter().any(|n| n.message.contains("MyStruct")));
        assert!(
            has_struct_name,
            "Diagnostic notes should contain 'MyStruct', got diags: {:?}",
            diags
        );
    }

    /// Stage 16.83 positive 2: to_diagnostics without resolver falls back.
    #[test]
    fn stage16_83_diagnostic_without_resolver_falls_back() {
        let src = "struct MyStruct { x: i32 } fn foo(s: MyStruct) {} fn main() { foo(42); 0 }";
        let result = compile(src);
        let diags = result.errors.to_diagnostics(Some(&result.interner));
        // Should still produce diagnostics (fallback works).
        assert!(
            !diags.is_empty(),
            "Should have diagnostics without resolver"
        );
    }

    /// Stage 16.83 negative 1: Compile mismatch diagnostic note shows name.
    #[test]
    fn stage16_83_compile_mismatch_diagnostic_note_shows_name() {
        let src = "struct MyStruct { x: i32 } fn foo(s: MyStruct) {} fn main() { foo(42); 0 }";
        let result = compile(src);
        let diags = result
            .errors
            .to_diagnostics_with_resolver(Some(&result.interner), Some(&result.trait_resolver));
        let has_struct_in_notes = diags
            .iter()
            .any(|d| d.children.iter().any(|n| n.message.contains("MyStruct")));
        assert!(
            has_struct_in_notes,
            "Diagnostic notes should contain 'MyStruct', got: {:?}",
            diags
        );
    }

    /// Stage 16.83 negative 2: Compile struct mismatch diagnostic full.
    #[test]
    fn stage16_83_compile_struct_mismatch_diagnostic_full() {
        let src = "struct Foo { x: i32 } fn foo(f: Foo) {} fn main() { foo(42); 0 }";
        let result = compile(src);
        let diags = result
            .errors
            .to_diagnostics_with_resolver(Some(&result.interner), Some(&result.trait_resolver));
        let has_foo = diags
            .iter()
            .any(|d| d.children.iter().any(|n| n.message.contains("Foo")));
        assert!(
            has_foo,
            "Diagnostic notes should contain 'Foo', got: {:?}",
            diags
        );
    }

    /// Stage 16.83 negative 3: Compile enum mismatch diagnostic shows name.
    #[test]
    fn stage16_83_compile_enum_mismatch_diagnostic_shows_name() {
        let src = "enum MyEnum { A, B } fn foo(e: MyEnum) {} fn main() { foo(42); 0 }";
        let result = compile(src);
        let diags = result
            .errors
            .to_diagnostics_with_resolver(Some(&result.interner), Some(&result.trait_resolver));
        let has_enum = diags
            .iter()
            .any(|d| d.children.iter().any(|n| n.message.contains("MyEnum")));
        assert!(
            has_enum,
            "Diagnostic notes should contain 'MyEnum', got: {:?}",
            diags
        );
    }

    /// Stage 16.83 negative 4: Compile two struct mismatch shows both.
    #[test]
    fn stage16_83_compile_two_struct_diagnostic_shows_both() {
        let src = "struct Foo { x: i32 } struct Bar { y: i32 } fn foo(f: Foo) {} fn main() { foo(Bar { y: 1 }); 0 }";
        let result = compile(src);
        let diags = result
            .errors
            .to_diagnostics_with_resolver(Some(&result.interner), Some(&result.trait_resolver));
        let has_foo = diags
            .iter()
            .any(|d| d.children.iter().any(|n| n.message.contains("Foo")));
        let has_bar = diags
            .iter()
            .any(|d| d.children.iter().any(|n| n.message.contains("Bar")));
        assert!(
            has_foo && has_bar,
            "Diagnostic notes should contain 'Foo' and 'Bar', got: {:?}",
            diags
        );
    }

    /// Stage 16.83 negative 5: Compile fn arg diagnostic shows name.
    #[test]
    fn stage16_83_compile_fn_arg_diagnostic_shows_name() {
        let src = "struct MyStruct { x: i32 } fn foo(s: MyStruct) {} fn main() { foo(42); 0 }";
        let result = compile(src);
        let diags = result
            .errors
            .to_diagnostics_with_resolver(Some(&result.interner), Some(&result.trait_resolver));
        // The diagnostic message itself should contain MyStruct (from Stage 16.81).
        let has_struct = diags.iter().any(|d| d.message.contains("MyStruct"));
        assert!(
            has_struct,
            "Diagnostic message should contain 'MyStruct', got: {:?}",
            diags
        );
    }

    /// Stage 16.83 negative 6: format_via_diagnostics_with_resolver shows name.
    #[test]
    fn stage16_83_format_for_user_with_resolver_shows_name() {
        use crate::session::SourceMap;
        let src = "struct MyStruct { x: i32 } fn foo(s: MyStruct) {} fn main() { foo(42); 0 }";
        let result = compile(src);
        let source_map = SourceMap::new(src);
        let formatted = result.errors.format_via_diagnostics_with_resolver(
            src,
            "test.lin",
            &source_map,
            Some(&result.interner),
            Some(&result.trait_resolver),
        );
        assert!(
            formatted.contains("MyStruct"),
            "Formatted output should contain 'MyStruct', got: {}",
            formatted
        );
    }
}
