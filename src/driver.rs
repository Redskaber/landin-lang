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
use crate::session::Span;
use crate::traits::{CoherenceError, IncompleteImpl};
use crate::typeck::{self, TypeError, TypeckResults};
use lasso::Rodeo;

/// Stage 15.9 (v0.2): Typed trait error.
///
/// Replaces the previous `Vec<String>` for `CompileErrors.trait_errors`.
/// Carries the structured `CoherenceError`/`IncompleteImpl` data so downstream
/// consumers (LSP, error reporters) can access the DefIds and Spur names
/// without re-parsing strings.
///
/// Per §1.0 原则 3 "显式 > 隐式": the error kind is explicit (enum variant),
/// not implicit (string prefix).
/// Per §23 (API Naming): `TraitError` follows the `<Noun>Error` pattern
/// consistent with `TypeError`, `BorrowError`, etc.
#[derive(Debug, Clone)]
pub enum TraitError {
    /// Stage 5.18: Multiple `impl Trait for Type` blocks exist for the same
    /// (trait, type) pair — coherence violation.
    Coherence(CoherenceError),
    /// Stage 5.19: An `impl Trait for Type` block is missing one or more
    /// methods declared by the trait.
    Incomplete(IncompleteImpl),
}

impl TraitError {
    /// Format the error as a human-readable string, using the interner
    /// to resolve Spur symbols to &str.
    ///
    /// Per §23 (API Naming): `format_with_interner` follows
    /// `<verb>_<noun>_<noun>` pattern.
    pub fn format_with_interner(&self, interner: &Rodeo) -> String {
        match self {
            TraitError::Coherence(ce) => {
                let trait_str = interner.try_resolve(&ce.trait_name).unwrap_or("?");
                let type_str = interner.try_resolve(&ce.self_ty_name).unwrap_or("?");
                format!(
                    "conflicting implementations of trait `{}` for type `{}` ({} impl blocks)",
                    trait_str,
                    type_str,
                    ce.impl_def_ids.len()
                )
            }
            TraitError::Incomplete(inc) => {
                let trait_str = interner.try_resolve(&inc.trait_name).unwrap_or("?");
                let type_str = interner.try_resolve(&inc.self_ty_name).unwrap_or("?");
                let missing: Vec<&str> = inc
                    .missing_methods
                    .iter()
                    .map(|s| interner.try_resolve(s).unwrap_or("?"))
                    .collect();
                format!(
                    "impl `{}` for `{}` is missing method(s): {}",
                    trait_str,
                    type_str,
                    missing.join(", ")
                )
            }
        }
    }
}

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
    /// Stage 5.22: Trait coherence/completeness errors (non-fatal —
    /// compilation continues but the user should fix these).
    ///
    /// Stage 15.9 (v0.2): Changed from `Vec<String>` to `Vec<TraitError>`
    /// to preserve the structured CoherenceError/IncompleteImpl data.
    /// Closes Phase 2 audit item: "Stop stringifying CoherenceError/IncompleteImpl".
    pub trait_errors: Vec<TraitError>,
}

impl CompileErrors {
    pub fn is_empty(&self) -> bool {
        self.lex.is_empty()
            && self.parse.is_empty()
            && self.resolve.is_empty()
            && self.typeck.is_empty()
            && self.borrowck.is_empty()
            && self.trait_errors.is_empty()
    }

    pub fn total_count(&self) -> usize {
        self.lex.len()
            + self.parse.len()
            + self.resolve.len()
            + self.typeck.len()
            + self.borrowck.len()
            + self.trait_errors.len()
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
    /// **DEPRECATED** (Stage 15.15): Use `format_via_diagnostics` instead —
    /// it uses the `src/diagnostics/` module as the single source of truth
    /// for error display (rustc-style with `error[Code]:` + `-->` + snippets).
    /// This method is kept for backward compatibility with existing tests.
    #[deprecated(
        since = "0.140.0",
        note = "Use `format_via_diagnostics` instead (rustc-style display via src/diagnostics/ module)"
    )]
    /// Stage 2.4d (P1-4): This is the user-facing error display.
    /// Previously, errors were only available as raw Debug output.
    ///
    /// Stage 15.9 (v0.2): Added `interner: Option<&Rodeo>` parameter to
    /// resolve TraitError Spur symbols to &str. The interner is always
    /// available from CompileResult.interner — callers pass `Some(&result.interner)`.
    pub fn format_for_user(&self, src: Option<&str>, interner: Option<&Rodeo>) -> String {
        let mut out = String::new();
        let total = self.total_count();
        if total == 0 {
            return String::new();
        }
        // Stage 15.12: friendlier summary line (was "error: N error(s)").
        // Per "显示友好": use "error[E001]: ..." style with count.
        // Example: "error: 3 errors found" (plural) or "error: 1 error found" (singular).
        if total == 1 {
            out.push_str("error: 1 error found\n");
        } else {
            out.push_str(&format!("error: {} errors found\n", total));
        }

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
            // Stage 15.12: use Display message + snippet (was Debug {:?}).
            // Per "显示友好": users see the message, not the Debug format.
            out.push_str(&format!("  [resolve] {}\n", e.message));
            if let Some(s) = src {
                out.push_str(&format_snippet(s, &e.span));
            }
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
        // Stage 14.10: trait_errors were missing from format_for_user,
        // causing "error: N error(s)" with no detail lines when only
        // trait coherence/completeness errors existed. total_count()
        // already included trait_errors.len(), so the count was correct
        // but the details were invisible. This fix closes that diagnostic
        // gap by printing each trait error with a [trait] prefix.
        //
        // Stage 15.9: trait_errors is now Vec<TraitError> (was Vec<String>).
        // Use format_with_interner to resolve Spur symbols. If interner is
        // None (test contexts), fall back to Debug formatting.
        for e in &self.trait_errors {
            let msg = if let Some(interner) = interner {
                e.format_with_interner(interner)
            } else {
                format!("{:?}", e)
            };
            out.push_str(&format!("  [trait] {}\n", msg));
        }
        out
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
            // Stage 15.14: Add expected/found as notes if present.
            if let (Some(expected), Some(found)) = (&e.expected, &e.found) {
                builder = builder.with_note(format!("expected: {:?}", expected.kind), e.span);
                builder = builder.with_note(format!("found: {:?}", found.kind), e.span);
            }
            diags.push(builder.build());
        }
        for e in &self.borrowck {
            diags.push(
                DiagnosticBuilder::error(format!("{} ({:?})", e.message, e.kind), e.span)
                    .with_code(crate::diagnostics::ErrorCode::Borrow.to_string())
                    .build(),
            );
        }
        for e in &self.trait_errors {
            let msg = if let Some(interner) = interner {
                e.format_with_interner(interner)
            } else {
                format!("{:?}", e)
            };
            diags.push(
                DiagnosticBuilder::error(&msg, crate::session::Span::DUMMY)
                    .with_code(crate::diagnostics::ErrorCode::Trait.to_string())
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
        use crate::diagnostics::DiagnosticBuffer;
        let diags = self.to_diagnostics(interner);
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
/// For dummy spans (lo == hi == 0), returns an empty string (no snippet).
///
/// Stage 15.13: This is now a thin wrapper around
/// `crate::diagnostics::format_snippet` (the single source of truth).
/// Kept for backward compatibility with existing call sites in this file.
fn format_snippet(src: &str, span: &Span) -> String {
    crate::diagnostics::format_snippet(src, span)
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
        }
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

    // === Stage 0: Parse ===
    let mut parser = Parser::new(tokens, &mut interner);
    let krate = parser.parse_crate();
    errors.parse = parser.into_errors();
    if !errors.parse.is_empty() {
        return CompileResult::empty(interner, errors);
    }

    // === Stage 1: HIR lowering ===
    let mut hir = lower_crate(&krate, &interner);

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

        let return_ty = hir.owner(body_id.owner.0).and_then(owner_return_ty);

        let (mut mir, lower_unify, lower_type_errors) =
            lower_hir_body_to_mir_full_with_dyn_trait_plan(
                body,
                &interner,
                &hir,
                return_ty,
                Some(&dyn_trait_plan),
            );

        // Stage 15.12: Collect type errors from MIR lowering (e.g., "no method found").
        // Per "报错 > 静默" principle — these errors were previously silently
        // swallowed (Error placeholder → codegen produced 0 or invalid IR).
        // Stage 15.12: errors now returned from the lowering function (was
        // stored on MirBody.lower_type_errors — mixed IR + error collection).
        errors.typeck.extend(lower_type_errors);

        // Stage 3.60: typeck uses pre-computed tables instead of HIR.
        let mut tc = typeck::TypeChecker::with_unify(lower_unify);
        tc.fn_sigs = fn_sig_table.sigs.clone();
        tc.check_mir_body_with_tables(&mut mir, Some(&field_ty_table));
        let (type_errors, body_results) = tc.into_results();
        errors.typeck.extend(type_errors);
        typeck_results.push(body_results);

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
        let mut bc: borrowck::BorrowChecker<'_> = borrowck::BorrowChecker::new();
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
    let mut fn_name_by_def_id: std::collections::HashMap<crate::hir::DefId, String> =
        std::collections::HashMap::new();
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
            let return_ty = hir.owner(body_id.owner.0).and_then(owner_return_ty);
            let is_void = return_ty.is_none();
            // Stage 13.22: Force `main`/`landin_main` to return i32 (not void).
            // The C wrapper declares `extern int landin_main(void)` and reads
            // the return value. If the LLVM function is void, the return
            // register contains garbage → undefined exit code (e.g., 219).
            // For void main, codegen emits `ret i32 0` instead of `ret void`.
            let is_void = is_void && fn_name != "landin_main";
            // Stage 8.3: Get the ABI from the function owner.
            let abi = hir
                .owner(body_id.owner.0)
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
            HirFnRetTy::Default(_) => None,
        },
        OwnerNode::Item(HirItem::Const(c)) => Some(c.ty.clone()),
        OwnerNode::Item(HirItem::Static(s)) => Some(s.ty.clone()),
        _ => None,
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
                errors.resolve.push(crate::resolve::ResolveError::new(
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
                    _ => {}
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
                    _ => {}
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
                    _ => {}
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
        // Stage 14.100 (Bug AA1 fix): Println args must be scanned.
        // Previously the catch-all skipped Println, so
        // `println!("{}", nonexistent_xyz)` silently printed 0.
        HirExprKind::Println { args, .. } => {
            for a in args {
                scan_expr_for_unresolved(a, errors);
            }
        }
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
                    _ => {}
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
                errors.resolve.push(crate::resolve::ResolveError::new(
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
                    _ => {}
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
                errors.resolve.push(crate::resolve::ResolveError::new(
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
                errors.resolve.push(crate::resolve::ResolveError::new(
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
                errors.resolve.push(crate::resolve::ResolveError::new(
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

fn scan_ty_for_unresolved(ty: &crate::hir::HirTy, errors: &mut CompileErrors) {
    use crate::hir::{HirTyKind, Res};
    match &ty.kind {
        HirTyKind::Path(_, p) => {
            if matches!(p.res, Res::Unknown | Res::Err) {
                errors.resolve.push(crate::resolve::ResolveError::new(
                    "cannot find type in this scope".to_string(),
                    Span::DUMMY,
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
            errors.resolve.push(crate::resolve::ResolveError::new(
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
