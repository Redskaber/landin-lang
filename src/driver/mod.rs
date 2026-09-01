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

use lasso::Rodeo;
// Stage 18.250: Types used in struct definitions — re-exported for mod.rs
// Stage 18.377 (TD-ALLOW-SUPPRESSION): Removed 5 `#[allow(unused_imports)]`
// — all 7 symbols (BorrowError, HirCrate, HirItem, MirBody, TraitError,
// TypeError, TypeckResults) are actually used in CompileErrors struct and
// DriverState. The allows were historical (added when imports were unused)
// but are now stale. Per §1.0 原則 5 (去除兼容思维): remove stale allows.
// Per §1.0 原則 3 (显式 > 隐式): if imports are used, no allow needed.
use crate::borrowck::BorrowError;
use crate::hir::{HirCrate, HirItem};
use crate::mir::body::MirBody;
use crate::traits::TraitError;
use crate::typeck::{TypeError, TypeckResults};

/// Errors collected from one or more passes.
// Stage 18.134 §13.4 J1-J6: extract sub-responsibilities from driver.rs
mod driver_scan;
mod projection_resolver;
// Stage 18.138 §13.4 J1-J6: extract codegen prep from mod.rs
mod driver_codegen_prep;
// Stage 30.22 §13.4 J2/J6: split driver_validations by responsibility.
mod driver_validations;
mod driver_validations_impl;
mod driver_validations_struct;
mod driver_validations_trait_object;
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
                TraitError::InherentConflict(ic) => ic.span,
                TraitError::PrimitiveInherentImpl(pie) => pie.span,
                TraitError::OrphanRule(ore) => ore.span,
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

/// Stage 29.1 (v0.11 TD-SINGLE-FILE Phase 4): Compile a project from a
/// `ProjectManifest` (loaded from `landin.toml`).
///
/// This is the manifest-based entry point for project compilation. It:
/// 1. Reads the entry point from the manifest
/// 2. Compiles via `compile_project_opt` (which runs ModuleLoader)
/// 3. Returns the `CompileResult`
///
/// Per §10: `compile_project_from_manifest` follows `<verb>_<noun>_<prep>_<noun>` pattern.
/// Per §11: driver-level orchestrator (no cross-stage leakage).
/// Per §1.0 原則 6 (通解 > 特解): one function handles all manifest kinds.
/// Per §12 (最优 > 最小): root-cause fix — manifest → entry_point → compile.
pub fn compile_project_from_manifest(manifest: &crate::cargo::ProjectManifest) -> CompileResult {
    compile_project_opt(&manifest.entry_point, true)
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
// Stage 18.250: compile_inner extracted to compile_inner.rs
mod compile_inner;
pub(crate) use compile_inner::compile_inner;

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
///
/// Stage 32.3 (TD-PRELUDE-MONO-ORDER): Now uses
/// `lower_hir_ty_to_mir_ty_with_hir_and_generics` with the impl block's
/// generic params, so `impl<T> Vec<T>` self_ty `Vec<T>` resolves to
/// `Adt(vec_def_id, [Param(0)])` instead of `Adt(vec_def_id, [Error])`.
///
/// Per §1.0 原则 6 (通解 > 特解): one path for all impl blocks (generic
/// and non-generic). Non-generic impls have empty generics → no-op.
/// Per §1.0 原则 3 (显式 > 隐式): generic params are explicitly threaded
/// from the impl block's HIR.
/// Per §1.0 原则 9 (正确 > 妥协): correct type resolution > silent Error.
fn resolve_self_param_type_for_sig(
    hir: &HirCrate,
    method_def_id: crate::hir::DefId,
    self_kind: Option<crate::ast::SelfKind>,
    method_to_impl_index: &std::collections::HashMap<crate::hir::DefId, usize>,
) -> Option<crate::mir::ty::Ty> {
    let owner_idx = *method_to_impl_index.get(&method_def_id)?;
    let (_, owner) = &hir.owners[owner_idx];
    if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
        // Stage 32.3: Extract impl block's generics so the self_ty is lowered
        // with proper Param substitution (e.g., `Vec<T>` → `Adt(Vec, [Param(0)])`).
        let (impl_def_id, _) = &hir.owners[owner_idx];
        let impl_generics = crate::hir::generics::find_generics(*impl_def_id, hir);
        let adt_ty = crate::mir::lower::lower_hir_ty_to_mir_ty_with_hir_and_generics(
            &impl_block.self_ty,
            Some(hir),
            &impl_generics,
        );
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
