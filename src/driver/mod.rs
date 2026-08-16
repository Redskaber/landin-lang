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
use crate::hir::{HirCrate, HirItem};
use crate::lexer::tokenize;
use crate::mir::body::MirBody;
use crate::mir::lower::lower_hir_body_to_mir_full_with_dyn_trait_plan;
use crate::parser::Parser;
use crate::resolve::resolve_crate;
use crate::traits::TraitError;
use crate::typeck::{self, TypeError, TypeckResults};
use lasso::Rodeo;

/// Errors collected from one or more passes.
// Stage 18.134 §13.4 J1-J6: extract sub-responsibilities from driver.rs
mod driver_scan;
mod projection_resolver;
// Stage 18.138 §13.4 J1-J6: extract codegen prep from mod.rs
mod driver_codegen_prep;
mod driver_validations;
// Stage 18.152 (TD-SINGLE-FILE Phase 1): multi-file module loader.
// Per §11: driver-level concern (runs after parse, before HIR lower).
pub mod module_loader;
pub use module_loader::{ModuleLoadError, ModuleLoader};

// Stage 18.134: import extracted functions for use in compile_inner
use driver_scan::scan_for_unresolved_paths;
use driver_validations::owner_return_ty;

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
    /// Stage 18.159 (TD-MODULELOAD-ERROR-FIELD): Module loading errors
    /// (non-fatal — compilation continues with whatever modules were
    /// successfully loaded). Previously these were force-cast to
    /// `LowerError`, losing the `path` field that identifies which file
    /// failed to load.
    ///
    /// Per §2 原則 4 (报错>静默): module load errors preserve structured
    /// `path` info for better diagnostics (shows which file was missing).
    /// Per §1.0 原則 6 (通解>特例): dedicated field instead of overloading
    /// `lower` errors.
    pub module_load: Vec<crate::driver::module_loader::ModuleLoadError>,
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
            && self.module_load.is_empty()
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
            + self.module_load.len()
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

        // Stage 18.159 (TD-MODULELOAD-ERROR-FIELD): Iterate module load
        // errors — previously force-cast to LowerError, losing the `path`
        // field. Now they have a dedicated field and ErrorCode (E850).
        // Per §1.0 原则 4 "报错 > 静默": module load errors must reach the user.
        // Per §2 原则 9 (正确>妥协): preserve structured `path` info as a note.
        for e in &self.module_load {
            let mut builder = DiagnosticBuilder::error(&e.message, e.span)
                .with_code(crate::diagnostics::ErrorCode::ModuleLoad.to_string());
            // Stage 18.159: Add the file path as a note for better UX.
            if let Some(path) = &e.path {
                builder = builder.with_note(format!("module file: {}", path.display()), e.span);
            }
            diags.push(builder.build());
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
    /// Stage 18.104 (S5 fix): Pre-computed type name map for monomorphization.
    /// Maps DefId → Symbol (type name) for all struct/enum items.
    /// Used by codegen to produce correct specialized function names
    /// (e.g., `make_box_Box_i32` instead of `make_box_Adt_0_i32`).
    /// Per §16: pre-computed from HIR (data flows downstream, no HIR in codegen).
    pub type_name_by_def_id: std::collections::HashMap<crate::hir::DefId, crate::lexer::Symbol>,
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
            type_name_by_def_id: std::collections::HashMap::new(),
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
    compile_inner(src, true, None)
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
    compile_inner(src, false, None)
}

/// Stage 18.152 (TD-SINGLE-FILE Phase 1): Compile a multi-file project.
///
/// Reads `entry_path` (e.g., `src/main.lin`), parses it, then runs
/// `ModuleLoader::load_module_tree` to recursively load all `mod foo;`
/// declarations from disk (`foo.lin` or `foo/mod.lin`).
///
/// After loading, the merged AST is lowered to HIR + MIR via the standard
/// `compile_inner` pipeline.
///
/// # Module resolution
///
/// For `mod foo;` declared in `entry_path`:
/// 1. Try `<entry_dir>/foo.lin` (single-file module)
/// 2. Else try `<entry_dir>/foo/mod.lin` (directory module)
/// 3. Else report a module-load error
///
/// `entry_dir` is the parent directory of `entry_path`.
///
/// # Errors
///
/// Module-load errors (file not found, parse error in submodule, circular
/// dependency) are surfaced in `CompileResult.errors` as `LowerError`
/// entries (the closest existing error category). Future: add a dedicated
/// `ModuleLoadError` variant to `CompileErrors`.
///
/// Per §10: `compile_project` follows `<verb>_<noun>` pattern (entry function).
/// Per §11: driver-level orchestrator (no cross-stage leakage).
/// Per §12 (最优>最小): root-cause fix for TD-SINGLE-FILE — multi-file
/// compilation is a first-class API, not a workaround.
pub fn compile_project(entry_path: &std::path::Path) -> CompileResult {
    // Stage 18.152 (TD-SINGLE-FILE Phase 1): Multi-file project compilation.
    //
    // Reads `entry_path`, parses it, runs `ModuleLoader::load_module_tree`
    // to populate `mod foo;` declarations from disk, then continues the
    // standard pipeline via `compile_inner`.
    //
    // Per §10: `compile_project` follows `<verb>_<noun>` pattern (entry function).
    // Per §11: driver-level orchestrator (no cross-stage leakage).
    // Per §12 (最优>最小): root-cause fix — `compile_inner` accepts an
    // optional `entry_path` so ModuleLoader can run between parse and HIR lower.
    //
    // Stage 18.155: delegates to `compile_project_opt` with `optimize=true`
    // (default debug build runs MIR optimization). Use `compile_project_opt`
    // directly to control optimization (e.g., `--release` → `optimize=false`
    // is WRONG; `--release` should enable MORE optimization, but currently
    // `compile_inner(optimize=true)` is the only optimization level —
    // `optimize=false` disables DCE+const_prop entirely).
    //
    // Per §2 原則 3 (显式>隐式): `compile_project` is the simple default;
    // `compile_project_opt` is the explicit control variant.
    compile_project_opt(entry_path, true)
}

/// Stage 18.155 (TD-SINGLE-FILE Phase 4): Compile a multi-file project with
/// explicit optimization control.
///
/// `optimize=true` runs MIR optimization passes (DCE + const_prop) after
/// writeback. `optimize=false` skips them (useful for tests that verify
/// unoptimized IR structure, or for `--release` builds that defer optimization
/// to LLVM).
///
/// Per §10: `compile_project_opt` follows `<verb>_<noun>_<adj>` pattern.
/// Per §2 原則 3 (显式>隐式): the optimize flag is explicit, not inferred.
/// Per §1.0 原則 6 (通解>特例): one function handles both debug and release.
pub fn compile_project_opt(entry_path: &std::path::Path, optimize: bool) -> CompileResult {
    let src = match std::fs::read_to_string(entry_path) {
        Ok(s) => s,
        Err(e) => {
            // File not readable — return an empty CompileResult with a lex error.
            let interner = Rodeo::new();
            let mut errors = CompileErrors::default();
            errors.lex.push(crate::lexer::reader::LexError {
                message: format!("cannot read entry file {}: {}", entry_path.display(), e),
                span: crate::session::Span::DUMMY,
                kind: crate::lexer::reader::LexErrorKind::Generic,
            });
            return CompileResult::empty(interner, errors);
        }
    };
    compile_inner(&src, optimize, Some(entry_path))
}

/// Internal compile implementation. `optimize` controls whether MIR
/// optimization passes (DCE + const_prop) run after writeback.
///
/// Stage 18.152: `entry_path` controls multi-file module loading.
/// - `None`: single-file mode (legacy `compile(src)` path). `mod foo;`
///   declarations are NOT loaded from disk (HIRModKind::Loaded stays empty).
/// - `Some(path)`: multi-file mode (`compile_project` path). After parsing,
///   `ModuleLoader::load_module_tree` runs to populate `mod foo;` items
///   from disk (`foo.lin` or `foo/mod.lin`), relative to `path.parent()`.
///
/// Per §1.0 原則 6 (通解>特例): one `compile_inner` handles both modes,
/// parameterized by `entry_path`. No separate multi-file pipeline.
/// Per §2.0 原則 3 (显式>隐式): the `entry_path` parameter is explicit.
fn compile_inner(src: &str, optimize: bool, entry_path: Option<&std::path::Path>) -> CompileResult {
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

    // Stage 18.141 §13.4 J2: extracted to driver_codegen_prep.rs
    driver_codegen_prep::pre_intern_macro_symbols(&mut interner);
    let (tokens, macro_errs) =
        crate::parser::macro_expand::expand_macros_with_errors(tokens, &mut interner);
    errors.macro_errors = macro_errs;

    // === Stage 0: Parse ===
    let mut parser = Parser::new(tokens, &mut interner);
    let mut krate = parser.parse_crate();
    errors.parse = parser.into_errors();
    if !errors.parse.is_empty() {
        return CompileResult::empty(interner, errors);
    }

    // === Stage 18.165: Inject built-in prelude types (Option, Result) ===
    // Per §11: prelude injection is a driver-level concern (after parse,
    // before HIR lower). Per §1.0 原則 6 (通解>特例): one injection
    // mechanism for all built-in types.
    crate::stdlib::prelude::inject_prelude(&mut krate, &mut interner);

    // === Stage 18.152: Multi-file module loading (only in project mode) ===
    // Per §11: ModuleLoader runs after parse, before HIR lower.
    // Per §2 原则 4 (报错>静默): load errors are collected, not silently ignored.
    if let Some(entry) = entry_path {
        let base_dir = entry.parent().unwrap_or_else(|| std::path::Path::new("."));
        let mut loader = ModuleLoader::new();
        let load_errors = loader.load_module_tree(&mut krate, base_dir, &mut interner);
        if !load_errors.is_empty() {
            // Stage 18.159 (TD-MODULELOAD-ERROR-FIELD): Module load errors
            // now go to the dedicated `module_load` field (was: force-cast
            // to LowerError, losing the `path` field).
            //
            // Per §2 原則 4 (报错>静默): errors are collected, not silently ignored.
            // Per §1.0 原則 6 (通解>特例): dedicated field preserves structured info.
            errors.module_load.extend(load_errors);
            // Don't return early — let HIR lower run on the partial AST so
            // downstream errors are also surfaced (better UX: user sees all
            // errors at once, not one at a time).
        }
    }

    // === Stage 1: HIR lowering ===
    // Stage 18.78 P0-A: lower_crate now returns (HirCrate, Vec<LowerError>).
    // Previously errors were silently discarded, making CompileErrors.lower
    // always empty. Now they're properly collected.
    let (mut hir, lower_errors) = lower_crate(&krate, &interner);
    errors.lower.extend(lower_errors);

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
    // Stage 18.142 §13.4 J2: extracted to driver_codegen_prep.rs
    let generics_map = driver_codegen_prep::build_generics_map(&hir);

    // Stage 16.16: Declare fn_name_by_def_id early so the per-body loop
    // can register synthesized closure function names.
    let mut fn_name_by_def_id: std::collections::HashMap<crate::hir::DefId, String> =
        std::collections::HashMap::new();

    // Stage 15.2 (perf): Pre-build method→impl index for O(1) lookup.
    let method_to_impl_index = build_method_to_impl_index(&hir);

    for (def_id, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Fn(f)) = owner {
            use crate::hir::HirFnRetTy;
            // Stage 18.105 (S6 fix): Build generic_params from f.generics so
            // bare type parameters (e.g., `T` in `Box<T>`) resolve to Param(N)
            // instead of Error in fn_sig_table.
            let generic_params: Vec<crate::mir::ty::ParamTy> =
                crate::hir::generics::find_generics(*def_id, &hir);
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
                        crate::mir::lower::lower_hir_ty_to_mir_ty_with_hir_and_generics(
                            ty,
                            Some(&hir),
                            &generic_params,
                        )
                    } else {
                        crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Error, p.span)
                    }
                })
                .collect();
            let output = match &f.sig.output {
                HirFnRetTy::Ty(t) => {
                    crate::mir::lower::lower_hir_ty_to_mir_ty_with_hir_and_generics(
                        t,
                        Some(&hir),
                        &generic_params,
                    )
                }
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
                                            p.span,
                                        )
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
                        HirFnRetTy::Default(_) => crate::mir::ty::Ty::new(
                            crate::mir::ty::TyKind::Tuple(Vec::new()),
                            f.span,
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
    // Stage 18.144: extracted to driver_codegen_prep.rs
    driver_codegen_prep::populate_trait_default_fn_sigs(
        &hir,
        &interner,
        &mut fn_sig_table,
        &mut errors,
    );
    let mut mirs = Vec::with_capacity(hir.bodies.len());
    let mut typeck_results = Vec::with_capacity(hir.bodies.len());
    // Stage 16.14: Synthesized closure MIR bodies, built per-function.
    let mut synthesized_closure_mir_bodies: Vec<crate::mir::body::MirBody> = Vec::new();

    // Stage 18.143 §13.4 J2: extracted to driver_codegen_prep.rs
    let (trait_resolver, dyn_trait_plan) =
        driver_codegen_prep::build_trait_resolver_and_plan(&hir, &mut interner, &mut errors);

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

        // Stage 18.103 (TD-MONO-CODEGEN): Set def_id on MirBody so codegen
        // can find the generic MIR body by DefId for monomorphization.
        // Per §16: data carried on the IR, not looked up from HIR.
        mir.def_id = Some(owner_def_id);

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
        projection_resolver::resolve_projections_in_mir(&mut mir, &hir);

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

    // so codegen becomes a pure MIR consumer (no re-lowering, no re-typeck).
    // Per §16.2.1: this is "data flows downstream" — the driver (orchestrator)
    // builds the metadata and passes it as data, not as HIR references.
    // Stage 18.138 §13.4 J2: extracted to driver_codegen_prep.rs
    driver_codegen_prep::populate_fn_name_by_def_id(&hir, &interner, &mut fn_name_by_def_id);

    // Build per-body metadata (parallel to mirs).
    //
    // Stage 5.6: extend fn_name resolution to cover impl method bodies so
    // vtable entries (which reference `landin_<SelfType>_<method>`) point
    // at the actual emitted LLVM symbol. Previously impl methods fell back
    // to `fn_<owner_id>` which made vtable references dangling.
    // Stage 18.139 §13.4 J2: extracted to driver_codegen_prep.rs
    let body_metas = driver_codegen_prep::build_body_metas(
        &interner,
        &hir,
        &lowered_body_owners,
        &fn_name_by_def_id,
    );

    // Stage 18.140 §13.4 J2: extracted to driver_codegen_prep.rs
    driver_validations::run_post_typeck_validations(
        &hir,
        &interner,
        &mut errors,
        &trait_resolver,
        &mut fn_name_by_def_id,
    );

    // Stage 18.104 (S5 fix): Build type name map from HIR for codegen.
    // Maps DefId → Symbol for all struct/enum items.
    // Per §16: pre-computed from HIR (data flows downstream, no HIR in codegen).
    // Built BEFORE hir is moved into CompileResult.
    // Stage 18.138 §13.4 J2: extracted to driver_codegen_prep.rs
    let type_name_by_def_id = driver_codegen_prep::build_type_name_by_def_id(&hir);

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
        type_name_by_def_id,
    }
}

/// Stage 18.73 P1-G: Compile a source file as a binary (entry point required).
///
/// Build a map from method DefId → impl block owner index.
/// Used by `resolve_self_param_type_for_sig` for O(1) self type lookup.
fn build_method_to_impl_index(
    hir: &HirCrate,
) -> std::collections::HashMap<crate::hir::DefId, usize> {
    let mut index = std::collections::HashMap::new();
    for (i, (_, owner)) in hir.owners.iter().enumerate() {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
            for impl_item in &impl_block.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    index.insert(f.hir_id.owner, i);
                }
            }
        }
    }
    index
}

/// Resolve the self param type for a method signature using the pre-built index.
fn resolve_self_param_type_for_sig(
    hir: &HirCrate,
    method_def_id: crate::hir::DefId,
    self_kind: Option<crate::ast::SelfKind>,
    method_to_impl_index: &std::collections::HashMap<crate::hir::DefId, usize>,
) -> Option<crate::mir::ty::Ty> {
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
            _ => Some(adt_ty),
        };
    }
    None
}

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

// Stage 18.137: re-export test helpers from tests.rs
pub use driver_tests::compile_expect_ok;

mod driver_tests;
